//! Core storage implementation combining database and file storage.

use crate::core::db::Database;
use crate::core::db::error::DatabaseError;
use crate::core::file_storage::FileStorage;
use crate::core::file_storage::error::FileStorageError;
use crate::types::value::PublicValue as Value;
use crate::types::value::versioned_value::latest_value;
use crate::types::value::versioned_value::latest_value::Attachment;
use crate::types::{Config, Key};
use error::KevaError;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) mod db;
pub(crate) mod file_storage;

pub mod error {
    use super::*;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum KevaError {
        #[error("Database error: {0}")]
        Database(#[from] DatabaseError),

        #[error("File storage error: {0}")]
        FileStorage(#[from] FileStorageError),

        #[error("Destination key already exists")]
        DestinationExists,
    }
}

fn key_to_path(key: &Key) -> PathBuf {
    let hash = blake3::hash(key.as_str().as_bytes());
    PathBuf::from(hash.to_hex().as_str())
}

pub struct KevaCore {
    base_path: PathBuf,
    db: Database,
    file: FileStorage,
}

#[derive(Debug, Default)]
pub struct MaintenanceOutcome {
    pub keys_trashed: Vec<Key>,
    pub keys_purged: Vec<Key>,
    pub orphaned_files_removed: usize,
}

impl KevaCore {
    pub fn open(config: Config) -> Result<Self, KevaError> {
        let base_path = config.base_path.clone();
        let file = FileStorage {
            content_path: config.content_path(),
            blobs_path: config.blobs_path(),
            thumbnails_path: config.thumbnails_path(),
        };

        let db = Database::new(config)?;
        Ok(Self {
            base_path,
            db,
            file,
        })
    }

    /// Returns the base data directory path.
    pub fn data_dir(&self) -> &Path {
        &self.base_path
    }

    /// Computes the absolute path to an attachment blob file.
    ///
    /// This is a static method that doesn't require a KevaCore instance,
    /// useful for code that needs to resolve attachment paths independently
    /// (e.g., drag-drop handlers on the main thread).
    pub fn attachment_blob_path(base_path: &Path, key: &Key, filename: &str) -> PathBuf {
        let key_hash = key_to_path(key);
        base_path.join("blobs").join(key_hash).join(filename)
    }
}

/// Read operations.
impl KevaCore {
    pub fn get(&self, key: &Key) -> Result<Option<Value>, KevaError> {
        let value = self.db.get(key)?;
        Ok(value.map(Value::from_latest_value))
    }

    pub fn active_keys(&self) -> Result<Vec<Key>, KevaError> {
        Ok(self.db.active_keys()?)
    }

    pub fn trashed_keys(&self) -> Result<Vec<Key>, KevaError> {
        Ok(self.db.trashed_keys()?)
    }
}

/// Content operations.
impl KevaCore {
    pub fn create(&mut self, key: &Key, now: SystemTime) -> Result<Value, KevaError> {
        let key_hash = key_to_path(key);

        let value: latest_value::Value = self.db.create(key, now)?;

        self.file.create_content(&key_hash)?;
        Ok(Value::from_latest_value(value))
    }

    pub fn content_path(&self, key: &Key) -> PathBuf {
        let key_hash = key_to_path(key);
        self.file.content_file_path(&key_hash)
    }

    /// Updates last_accessed timestamp.
    pub fn touch(&mut self, key: &Key, now: SystemTime) -> Result<Value, KevaError> {
        Ok(Value::from_latest_value(self.db.touch(key, now)?))
    }
}

/// Attachment operations.
impl KevaCore {
    pub fn attachment_path(&self, key: &Key, filename: &str) -> PathBuf {
        let key_hash = key_to_path(key);
        self.file.attachment_path(&key_hash, filename)
    }

    /// Add attachments with explicit target filenames.
    /// If a file with the same name exists, it will be overwritten.
    pub fn add_attachments(
        &mut self,
        key: &Key,
        files: Vec<(PathBuf, String)>,
        now: SystemTime,
    ) -> Result<(), KevaError> {
        let key_hash = key_to_path(key);

        for (source_path, target_filename) in files {
            // Remove existing attachment if present (overwrite behavior)
            let _ = self.remove_attachment(key, &target_filename, now);

            self.add_attachment_with_thumbnail(key, &key_hash, source_path, target_filename, now)?;
        }
        Ok(())
    }

    fn add_attachment_with_thumbnail(
        &mut self,
        key: &Key,
        key_hash: &Path,
        source_path: PathBuf,
        filename: String,
        now: SystemTime,
    ) -> Result<u64, KevaError> {
        let size = self
            .file
            .add_attachment(key_hash, &source_path, &filename)?;

        if FileStorage::is_supported_image(&filename) {
            self.file
                .generate_thumbnail(key_hash, &filename)
                .map_err(KevaError::from)?;
        }

        self.db
            .add_attachment(key, Attachment { filename, size }, now)
            .map_err(KevaError::from)?;

        Ok(size)
    }

    pub fn remove_attachment(
        &mut self,
        key: &Key,
        filename: &str,
        now: SystemTime,
    ) -> Result<(), KevaError> {
        let key_hash = key_to_path(key);

        self.db.remove_attachment(key, filename, now)?;

        self.file.remove_attachment(&key_hash, filename)?;
        self.file.remove_thumbnail(&key_hash, filename)?;
        Ok(())
    }

    pub fn rename_attachment(
        &mut self,
        key: &Key,
        old_filename: &str,
        new_filename: &str,
        now: SystemTime,
    ) -> Result<(), KevaError> {
        if old_filename == new_filename {
            return Ok(());
        }

        let value = self.db.get(key)?.ok_or(DatabaseError::NotFound)?;
        if value.attachments.iter().any(|a| a.filename == new_filename) {
            return Err(KevaError::DestinationExists);
        }

        let key_hash = key_to_path(key);

        self.db
            .rename_attachment(key, old_filename, new_filename, now)?;

        self.file
            .rename_attachment(&key_hash, old_filename, new_filename)?;

        self.file
            .rename_thumbnail(&key_hash, old_filename, new_filename)?;

        Ok(())
    }
}

/// Thumbnail operations.
impl KevaCore {
    /// Returns filename -> thumbnail relative path map for attachments with thumbnails.
    /// Paths are relative to the thumbnails directory (`data_dir()/thumbnails`).
    pub fn thumbnail_paths(&mut self, key: &Key) -> Result<HashMap<String, PathBuf>, KevaError> {
        let key_hash = key_to_path(key);
        let value = self.db.get(key)?.ok_or(DatabaseError::NotFound)?;
        let mut result = HashMap::new();

        // Regenerate all thumbnails if version is outdated
        for attachment in value.attachments {
            if FileStorage::is_supported_image(&attachment.filename) {
                if value.thumb_version < FileStorage::THUMB_VER {
                    let _ = self
                        .file
                        .generate_thumbnail(&key_hash, &attachment.filename);
                }

                result.insert(
                    attachment.filename.clone(),
                    FileStorage::thumbnail_rel_path(&key_hash, &attachment.filename),
                );
            }
        }

        if value.thumb_version < FileStorage::THUMB_VER {
            self.db.update_thumb_version(key, FileStorage::THUMB_VER)?;
        }
        Ok(result)
    }
}

/// Key management operations.
impl KevaCore {
    pub fn rename(
        &mut self,
        old_key: &Key,
        new_key: &Key,
        now: SystemTime,
    ) -> Result<(), KevaError> {
        if old_key == new_key {
            return Ok(());
        }

        if self.db.get(new_key)?.is_some() {
            return Err(KevaError::DestinationExists);
        }

        let old_hash = key_to_path(old_key);
        let new_hash = key_to_path(new_key);

        // Rename in database
        self.db.rename(old_key, new_key, now)?;

        // Rename files
        self.file.rename_all(&old_hash, &new_hash)?;

        Ok(())
    }
}

/// Trash operations.
impl KevaCore {
    /// Moves a key to trash.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), KevaError> {
        self.db.trash(key, now)?;
        Ok(())
    }

    /// Restores a key from trash.
    pub fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), KevaError> {
        self.db.restore(key, now)?;
        Ok(())
    }

    /// Permanently deletes a key.
    pub fn purge(&mut self, key: &Key) -> Result<(), KevaError> {
        let key_hash = key_to_path(key);
        self.db.purge(key)?;
        self.file.remove_all(&key_hash)?;
        Ok(())
    }
}

/// Maintenance operations.
impl KevaCore {
    /// Performs garbage collection and orphan cleanup.
    pub fn maintenance(&mut self, now: SystemTime) -> Result<MaintenanceOutcome, KevaError> {
        let gc_result = self.db.gc(now)?;

        // Clean up files for purged keys
        for key in &gc_result.purged {
            let key_hash = key_to_path(key);
            self.file.remove_all(&key_hash)?;
        }

        // Clean up orphan blobs (files without database entries)
        let valid_key_hashes: HashSet<_> = self
            .db
            .active_keys()?
            .iter()
            .chain(self.db.trashed_keys()?.iter())
            .map(key_to_path)
            .collect();

        let mut orphaned_files_removed = 0;

        // Check blob directories
        for key_hash in self.file.list_blob_key_hashes()? {
            if !valid_key_hashes.contains(&key_hash) {
                self.file.remove_all(&key_hash)?;
                orphaned_files_removed += 1;
            }
        }

        // Check content files
        for key_hash in self.file.list_content_key_hashes()? {
            if !valid_key_hashes.contains(&key_hash) {
                self.file.remove_content(&key_hash)?;
                orphaned_files_removed += 1;
            }
        }

        Ok(MaintenanceOutcome {
            keys_trashed: gc_result.trashed,
            keys_purged: gc_result.purged,
            orphaned_files_removed,
        })
    }
}

#[cfg(test)]
mod tests;
