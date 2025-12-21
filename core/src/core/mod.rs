//! Storage layer for keva.
//!
//! This module provides the main KevaCore struct that coordinates:
//! - Database operations (metadata, lifecycle, TTL)
//! - File storage operations (blob files)

use crate::core::db::GcResult;
use crate::core::db::error::DatabaseError;
use crate::core::file::FileStorage;
use crate::core::file::error::FileStorageError;
use crate::types::value::versioned_value::latest_value::{
    BlobStoredFileData, ClipData, FileData, LifecycleState, TextData, Value,
};
use crate::types::{Config, Key};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

pub(crate) mod db;
pub(crate) mod file;

pub mod error {
    use super::*;
    use crate::clipboard::ClipboardError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum StorageError {
        #[error("Database error: {0}")]
        Database(#[from] DatabaseError),

        #[error("File storage error: {0}")]
        FileStorage(#[from] FileStorageError),

        #[error("Clipboard error: {0}")]
        Clipboard(#[from] ClipboardError),

        #[error("Key is in Trash state - restore it first")]
        KeyIsTrashed,

        #[error("Key is already trashed")]
        AlreadyTrashed,

        #[error("Destination key already exists")]
        DestinationExists,
    }
}

use error::StorageError;

/// Converts a key to a filesystem-safe path by hashing it.
///
/// Keys can be up to 256 characters and contain `/`, which would create
/// unwanted subdirectories. We use BLAKE3 hash (hex) for a fixed-length,
/// safe directory name.
fn key_to_path(key: &Key) -> PathBuf {
    let hash = blake3_v1::hash(key.as_str().as_bytes());
    PathBuf::from(hash.to_hex().as_str())
}

pub struct KevaCore {
    db: db::Database,
    file: FileStorage,
}

impl KevaCore {
    /// Opens or creates a storage at the configured path.
    pub fn open(config: Config) -> Result<Self, StorageError> {
        let file = FileStorage {
            base_path: config.blob_path(),
            inline_threshold_bytes: config.saved.inline_threshold_bytes,
        };
        let db = db::Database::new(config)?;
        Ok(Self { db, file })
    }
}

/// Read operations.
impl KevaCore {
    /// Retrieves a value by key.
    ///
    /// Returns the raw value from the database. Stale entries (past TTL) are
    /// still returned - only GC transitions lifecycle states.
    pub fn get(&self, key: &Key) -> Result<Option<Value>, StorageError> {
        Ok(self.db.get(key)?)
    }

    /// Returns all Active keys efficiently by iterating the TTL table.
    pub fn active_keys(&self) -> Result<Vec<Key>, StorageError> {
        Ok(self.db.active_keys()?)
    }

    /// Returns all Trash keys efficiently by iterating the TTL table.
    pub fn trashed_keys(&self) -> Result<Vec<Key>, StorageError> {
        Ok(self.db.trashed_keys()?)
    }

    /// Resolves text content (handles BlobStored case).
    fn resolve_text(&self, key: &Key, text_data: &TextData) -> Result<String, StorageError> {
        match text_data {
            TextData::Inlined(s) => Ok(s.clone()),
            TextData::BlobStored => {
                let key_path = key_to_path(key);
                let text_path = self
                    .file
                    .base_path
                    .join(&key_path)
                    .join(file::TEXT_FILE_NAME);
                std::fs::read_to_string(&text_path).map_err(|e| FileStorageError::Io(e).into())
            }
        }
    }
}

/// Write operations.
impl KevaCore {
    /// Creates or updates a text value at the given key.
    ///
    /// If the existing value was blob-stored text (exceeding inline threshold) and the
    /// new text is small enough to be inlined, the old blob file is automatically removed.
    pub fn upsert_text(
        &mut self,
        key: &Key,
        text: &str,
        now: SystemTime,
    ) -> Result<(), StorageError> {
        let key_path = key_to_path(key);

        // Determine whether we need to clean up an existing blob-stored text file.
        // If the previous value was blob-stored text and the new representation is inlined,
        // remove the old blob file to avoid leaving orphaned `text.txt` on disk.
        let remove_old_blob_text = match self.get(key)? {
            Some(v)
                if v.metadata.lifecycle_state == LifecycleState::Active
                    && matches!(v.clip_data, ClipData::Text(TextData::BlobStored)) =>
            {
                // We'll compute the new representation next; if it becomes inlined, remove old blob.
                true
            }
            _ => false,
        };

        let text_data = self.file.store_text(&key_path, Cow::Borrowed(text))?;

        if remove_old_blob_text && matches!(text_data, TextData::Inlined(_)) {
            self.file.remove_blob_stored_text(&key_path)?;
        }

        match self.get(key)? {
            None => {
                self.db.insert(key, now, ClipData::Text(text_data))?;
            }
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                self.db.update(key, now, ClipData::Text(text_data))?;
            }
            Some(_) => {
                // Key exists but is trashed - must restore first
                return Err(StorageError::KeyIsTrashed);
            }
        }
        Ok(())
    }

    /// Adds files to a key.
    ///
    /// - If the key doesn't exist, creates a new Files entry
    /// - If the key exists and is Active with Files, appends to existing files
    /// - If the key exists and is Active with Text, returns TypeMismatch error
    /// - If the key exists and is Trash, returns KeyIsTrashed error (must restore first)
    pub fn add_files(
        &mut self,
        key: &Key,
        file_paths: impl IntoIterator<Item = impl AsRef<std::path::Path>>,
        now: SystemTime,
    ) -> Result<(), StorageError> {
        let key_path = key_to_path(key);
        let files: Vec<_> = file_paths
            .into_iter()
            .map(|p| self.file.store_file(&key_path, p.as_ref()))
            .collect::<Result<_, _>>()?;

        if files.is_empty() {
            return Ok(());
        }

        match self.get(key)? {
            None => {
                self.db.insert(key, now, ClipData::Files(files))?;
            }
            Some(v)
                if v.metadata.lifecycle_state == LifecycleState::Active
                    && matches!(v.clip_data, ClipData::Files(_)) =>
            {
                self.db.append_files(key, now, files)?;
            }
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Trash => {
                // Key is trashed - must restore first
                return Err(StorageError::KeyIsTrashed);
            }
            Some(_) => {
                // Key exists with Text (and is Active) - type mismatch
                return Err(DatabaseError::TypeMismatch.into());
            }
        }
        Ok(())
    }

    /// Removes a single file entry from a Files value by index.
    ///
    /// - Only works on Active keys
    /// - If the removed entry is blob-stored, deletes its blob file on disk
    /// - If the last file is removed, the value becomes empty text (consistent with emptying text)
    pub fn remove_file_at(
        &mut self,
        key: &Key,
        index: usize,
        now: SystemTime,
    ) -> Result<(), StorageError> {
        // DB mutation returns the removed entry so we can clean up blob storage.
        let removed = self.db.remove_file_at(key, now, index)?;

        if let FileData::BlobStored(BlobStoredFileData { file_name, hash }) = removed {
            let key_path = key_to_path(key);
            self.file
                .remove_blob_stored_file(&key_path, &BlobStoredFileData { file_name, hash })?;
        }

        Ok(())
    }

    /// Updates `last_accessed` without modifying the value.
    ///
    /// This should be called when the value is actually accessed (e.g. shown in the UI),
    /// not when keys are merely enumerated or searched.
    pub fn touch(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        Ok(self.db.touch(key, now)?)
    }
}

/// Lifecycle operations.
impl KevaCore {
    /// Soft-deletes a key by moving it to Trash state.
    ///
    /// Only works on Active keys. Returns error if key is already trashed.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let value = self.get(key)?;
        match value {
            None => Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                self.db.trash(key, now)?;
                Ok(())
            }
            Some(_) => Err(StorageError::AlreadyTrashed),
        }
    }

    /// Restores a trashed key to Active state.
    ///
    /// - If Trash → move to Active
    /// - If Active → no-op (idempotent)
    /// - If None → Error
    pub fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let value = self.get(key)?;
        match value {
            None => Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                // Already active - no-op
                Ok(())
            }
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Trash => {
                self.db.restore(key, now)?;
                Ok(())
            }
            Some(_) => {
                // Purge state - should not happen as keys are deleted by GC
                Err(DatabaseError::NotFound.into())
            }
        }
    }

    /// Permanently deletes a key, bypassing trash.
    ///
    /// This removes the key from the database and cleans up blob files.
    pub fn purge(&mut self, key: &Key) -> Result<(), StorageError> {
        self.db.purge(key)?;
        let key_path = key_to_path(key);
        self.file.remove_all(&key_path)?;
        Ok(())
    }
}

/// Key management operations.
impl KevaCore {
    /// Renames a key.
    ///
    /// Only works on Active source keys. If `overwrite` is false and destination
    /// exists (Active or Trash), returns `DestinationExists` error.
    pub fn rename(
        &mut self,
        old_key: &Key,
        new_key: &Key,
        overwrite: bool,
    ) -> Result<(), StorageError> {
        // Guardrail: renaming a key to itself is a no-op.
        if old_key == new_key {
            return Ok(());
        }

        // Check source key is Active
        let value = self.get(old_key)?;
        match value {
            None => return Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state != LifecycleState::Active => {
                return Err(StorageError::KeyIsTrashed);
            }
            Some(_) => {}
        }

        // Check destination doesn't exist (unless overwrite is true)
        let destination_exists = self.get(new_key)?.is_some();
        if !overwrite && destination_exists {
            return Err(StorageError::DestinationExists);
        }

        self.db.rename(old_key, new_key)?;

        let old_path = key_to_path(old_key);
        let new_path = key_to_path(new_key);
        self.file.rename(&old_path, &new_path)?;
        Ok(())
    }
}

/// Maintenance operations.
impl KevaCore {
    /// Runs garbage collection and blob cleanup.
    ///
    /// This is the intended hook for periodic maintenance, avoiding heavy work during
    /// active UI interaction.
    pub fn maintenance(&mut self, now: SystemTime) -> Result<GcResult, StorageError> {
        let result = self.db.gc(now)?;

        // Clean up blob files for purged keys
        for key in &result.purged {
            self.file.remove_all(&key_to_path(key))?;
        }

        self.cleanup_orphan_blobs()?;
        Ok(result)
    }

    /// Detects and removes orphan blob directories.
    ///
    /// Orphan blobs can occur if the process crashes after database purge but before
    /// filesystem cleanup. This finds blob directories without corresponding keys.
    fn cleanup_orphan_blobs(&self) -> Result<(), StorageError> {
        let active_paths: HashSet<_> = self
            .db
            .active_keys()?
            .into_iter()
            .map(|k| key_to_path(&k))
            .collect();
        let trashed_paths: HashSet<_> = self
            .db
            .trashed_keys()?
            .into_iter()
            .map(|k| key_to_path(&k))
            .collect();

        for dir in self.file.list_blob_dirs()? {
            if !active_paths.contains(&dir) && !trashed_paths.contains(&dir) {
                self.file.remove_all(&dir)?;
            }
        }

        Ok(())
    }
}

/// Clipboard operations.
impl KevaCore {
    /// Import clipboard content to a key.
    ///
    /// Files take priority over text when both are present.
    pub fn import_clipboard(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let content = crate::clipboard::read_clipboard()?;

        match content {
            crate::clipboard::ClipboardContent::Text(text) => self.upsert_text(key, &text, now),
            crate::clipboard::ClipboardContent::Files(paths) => self.add_files(key, paths, now),
        }
    }

    /// Copy key's value to clipboard and update access time.
    pub fn copy_to_clipboard(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let value = self.get(key)?.ok_or(DatabaseError::NotFound)?;

        // Only copy Active keys
        if value.metadata.lifecycle_state != LifecycleState::Active {
            return Err(StorageError::KeyIsTrashed);
        }

        match &value.clip_data {
            ClipData::Text(text_data) => {
                let text = self.resolve_text(key, text_data)?;
                crate::clipboard::write_text(&text)?;
            }
            ClipData::Files(files) => {
                let key_path = key_to_path(key);
                let paths: Vec<PathBuf> = files
                    .iter()
                    .map(|f| self.file.ensure_file_path(&key_path, f))
                    .collect::<Result<_, _>>()?;
                crate::clipboard::write_files(&paths)?;
            }
        }

        // Update last_accessed time
        self.db.touch(key, now)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
