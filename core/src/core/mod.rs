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
    ClipData, LifecycleState, TextData, Value,
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
    use crate::search::SearchError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum StorageError {
        #[error("Database error: {0}")]
        Database(#[from] DatabaseError),

        #[error("File storage error: {0}")]
        FileStorage(#[from] FileStorageError),

        #[error("Clipboard error: {0}")]
        Clipboard(#[from] ClipboardError),

        #[error("Search error: {0}")]
        Search(#[from] SearchError),

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
    config: Config,
    search_engine: crate::search::SearchEngine,
}

impl KevaCore {
    /// Opens or creates a storage at the configured path.
    pub fn open(
        config: Config,
        search_config: crate::search::SearchConfig,
    ) -> Result<Self, StorageError> {
        let file = FileStorage {
            base_path: config.blob_path(),
            inline_threshold_bytes: config.saved.inline_threshold_bytes,
        };
        let db = db::Database::new(config.clone())?;
        let search_engine =
            crate::search::SearchEngine::new(db.active_keys()?, db.trashed_keys()?, search_config);
        Ok(Self {
            db,
            file,
            config,
            search_engine,
        })
    }

    /// Computes the effective lifecycle state based on TTL expiration.
    ///
    /// The DB stores the state at the time of the last operation, but the effective
    /// state may have changed due to TTL expiration:
    /// - Active key with `last_accessed + trash_ttl <= now` → effective Trash
    /// - Trash key with `trashed_at + purge_ttl <= now` → effective Purge
    fn effective_lifecycle_state(&self, value: &Value, now: SystemTime) -> LifecycleState {
        match value.metadata.lifecycle_state {
            LifecycleState::Active => {
                let expires_at = value.metadata.last_accessed + self.config.saved.trash_ttl;
                if expires_at <= now {
                    LifecycleState::Trash
                } else {
                    LifecycleState::Active
                }
            }
            LifecycleState::Trash => {
                if let Some(trashed_at) = value.metadata.trashed_at {
                    let expires_at = trashed_at + self.config.saved.purge_ttl;
                    if expires_at <= now {
                        LifecycleState::Purge
                    } else {
                        LifecycleState::Trash
                    }
                } else {
                    // trashed_at should always be set for Trash keys, but be defensive
                    LifecycleState::Trash
                }
            }
            LifecycleState::Purge => LifecycleState::Purge,
        }
    }

    /// Retrieves a value by key.
    ///
    /// Returns the value with its effective lifecycle state:
    /// - Active keys are returned
    /// - Trash keys are returned (GUI can display them)
    /// - Purge keys (effective) return `None`
    ///
    /// The effective state is computed from TTL expiration, so an Active key
    /// in the DB may be returned as Trash if its TTL has expired.
    pub fn get(&self, key: &Key, now: SystemTime) -> Result<Option<Value>, StorageError> {
        let Some(mut value) = self.db.get(key)? else {
            return Ok(None);
        };

        let effective_state = self.effective_lifecycle_state(&value, now);

        match effective_state {
            LifecycleState::Purge => Ok(None), // Treat as deleted
            _ => {
                // Update lifecycle_state to effective state
                value.metadata.lifecycle_state = effective_state;
                Ok(Some(value))
            }
        }
    }

    /// Creates or updates a text value at the given key.
    ///
    /// - If the key doesn't exist, creates a new entry
    /// - If the key exists and is Active, updates the value (preserving created_at)
    /// - If the key exists and is Trash, returns KeyIsTrashed error (must restore first)
    pub fn upsert_text(
        &mut self,
        key: &Key,
        text: &str,
        now: SystemTime,
    ) -> Result<(), StorageError> {
        let key_path = key_to_path(key);
        let text_data = self.file.store_text(&key_path, Cow::Borrowed(text))?;

        match self.get(key, now)? {
            None => {
                self.db.insert(key, now, ClipData::Text(text_data))?;
                self.search_engine.add_active(key.clone());
            }
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                self.db.update(key, now, ClipData::Text(text_data))?;
                // Key already exists in search index, no update needed
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

        match self.get(key, now)? {
            None => {
                self.db.insert(key, now, ClipData::Files(files))?;
                self.search_engine.add_active(key.clone());
            }
            Some(v)
                if v.metadata.lifecycle_state == LifecycleState::Active
                    && matches!(v.clip_data, ClipData::Files(_)) =>
            {
                self.db.append_files(key, now, files)?;
                // Key already exists in search index, no update needed
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

    /// Soft-deletes a key by moving it to Trash state.
    ///
    /// Only works on Active keys. Returns error if key is already trashed.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let value = self.get(key, now)?;
        match value {
            None => Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                self.db.trash(key, now)?;
                self.search_engine.trash(key);
                Ok(())
            }
            Some(_) => Err(StorageError::AlreadyTrashed),
        }
    }

    /// Restores a trashed key to Active state.
    ///
    /// - If Trash → move to Active
    /// - If Active → no-op (idempotent)
    /// - If Purge/None → Error
    pub fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError> {
        let value = self.get(key, now)?;
        match value {
            None => Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Active => {
                // Already active - no-op
                Ok(())
            }
            Some(v) if v.metadata.lifecycle_state == LifecycleState::Trash => {
                self.db.restore(key, now)?;
                self.search_engine.restore(key);
                Ok(())
            }
            Some(_) => {
                // Purge state - should not happen since get() returns None for Purge
                Err(DatabaseError::NotFound.into())
            }
        }
    }

    /// Permanently deletes a key, bypassing trash.
    ///
    /// This removes the key from the database and cleans up blob files.
    pub fn purge(&mut self, key: &Key) -> Result<(), StorageError> {
        self.search_engine.remove(key);
        self.db.purge(key)?;
        let key_path = key_to_path(key);
        self.file.remove_all(&key_path)?;
        Ok(())
    }

    /// Renames a key.
    ///
    /// Only works on Active source keys. If `overwrite` is false and destination
    /// exists (Active or Trash), returns `DestinationExists` error.
    pub fn rename(
        &mut self,
        old_key: &Key,
        new_key: &Key,
        overwrite: bool,
        now: SystemTime,
    ) -> Result<(), StorageError> {
        // Check source key is Active
        let value = self.get(old_key, now)?;
        match value {
            None => return Err(DatabaseError::NotFound.into()),
            Some(v) if v.metadata.lifecycle_state != LifecycleState::Active => {
                return Err(StorageError::KeyIsTrashed);
            }
            Some(_) => {}
        }

        // Check destination doesn't exist (unless overwrite is true)
        if !overwrite && self.get(new_key, now)?.is_some() {
            return Err(StorageError::DestinationExists);
        }

        self.db.rename(old_key, new_key)?;
        self.search_engine.rename(old_key, new_key.clone());
        let old_path = key_to_path(old_key);
        let new_path = key_to_path(new_key);
        self.file.rename(&old_path, &new_path)?;
        Ok(())
    }

    /// Returns all Active keys efficiently by iterating the TTL table.
    pub fn active_keys(&self) -> Result<Vec<Key>, StorageError> {
        Ok(self.db.active_keys()?)
    }

    /// Returns all Trash keys efficiently by iterating the TTL table.
    pub fn trashed_keys(&self) -> Result<Vec<Key>, StorageError> {
        Ok(self.db.trashed_keys()?)
    }

    /// Performs garbage collection.
    ///
    /// This:
    /// 1. Moves expired Active keys to Trash
    /// 2. Permanently deletes expired Trash keys
    /// 3. Cleans up blob files for purged keys
    /// 4. Removes orphan blob directories (blobs without corresponding keys)
    pub fn gc(&mut self, now: SystemTime) -> Result<GcResult, StorageError> {
        let result = self.db.gc(now)?;

        // Update search engine for trashed keys
        for key in &result.trashed {
            self.search_engine.trash(key);
        }

        // Update search engine for purged keys
        for key in &result.purged {
            self.search_engine.remove(key);
        }

        // Clean up blob files for purged keys
        for key in &result.purged {
            let key_path = key_to_path(key);
            self.file.remove_all(&key_path)?;
        }

        // Clean up orphan blob directories
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

        Ok(result)
    }

    /// Import clipboard content to a key.
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
        let value = self.get(key, now)?.ok_or(DatabaseError::NotFound)?;

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

    /// Helper: resolve text content (handles BlobStored case)
    fn resolve_text(&self, key: &Key, text_data: &TextData) -> Result<String, StorageError> {
        match text_data {
            TextData::Inlined(s) => Ok(s.clone()),
            TextData::BlobStored => {
                let key_path = key_to_path(key);
                let text_path = self
                    .file
                    .base_path
                    .join(&key_path)
                    .join(crate::core::file::TEXT_FILE_NAME);
                std::fs::read_to_string(&text_path).map_err(|e| FileStorageError::Io(e).into())
            }
        }
    }

    /// Searches for keys matching the query.
    ///
    /// Returns results sorted by score (fuzzy) or in arbitrary order (regex).
    pub fn search(
        &mut self,
        query: crate::search::SearchQuery,
        timeout_ms: u64,
    ) -> Result<Vec<crate::search::SearchResult>, StorageError> {
        Ok(self.search_engine.search(query, timeout_ms)?)
    }
}

#[cfg(test)]
mod tests;
