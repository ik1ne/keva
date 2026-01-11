//! Database layer for keva storage.
//!
//! This module handles all redb operations including:
//! - Main key-value storage (Key → VersionedValue)
//! - TTL tracking tables for garbage collection
//! - Metadata storage (JSON strings)

use crate::core::db::error::DatabaseError;
use crate::core::db::ttl_table::TtlTable;
use crate::core::file_storage::FileStorage;
use crate::types::metadata::MaintenanceMetadata;
use crate::types::value::versioned_value::latest_value::{
    Attachment, LifecycleState, Metadata, Value,
};
use crate::types::value::versioned_value::VersionedValue;
use crate::types::{Config, GcConfig, Key, TtlKey};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use std::time::{Duration, SystemTime};

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum DatabaseError {
        #[error("Database error: {0}")]
        Redb(#[from] redb::DatabaseError),

        #[error("Table error: {0}")]
        TableError(#[from] redb::TableError),

        #[error("Storage error: {0}")]
        StorageError(#[from] redb::StorageError),

        #[error("Transaction error: {0}")]
        TransactionError(#[from] redb::TransactionError),

        #[error("Commit error: {0}")]
        CommitError(#[from] redb::CommitError),

        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("Key not found")]
        NotFound,

        #[error("Key is trashed")]
        Trashed,

        #[error("Key is not trashed")]
        NotTrashed,

        #[error("Key already exists")]
        AlreadyExists,

        #[error("Attachment not found: {0}")]
        AttachmentNotFound(String),

        #[error("Attachment already exists: {0}")]
        AttachmentExists(String),
    }
}

mod ttl_table;

/// Main table: Key → VersionedValue
const MAIN_TABLE: TableDefinition<Key, VersionedValue> = TableDefinition::new("main");

/// Metadata table: &str → JSON string
const METADATA_TABLE: TableDefinition<&str, &str> = TableDefinition::new("metadata");

/// Metadata key for maintenance tracking.
const METADATA_KEY_MAINTENANCE: &str = "maintenance";

/// TTL table tracking when Active keys expire to Trash.
const ACTIVE_EXPIRY: TtlTable = TtlTable::new("ttl_trashed");

/// TTL table tracking when Trash keys expire to Purge.
const TRASH_EXPIRY: TtlTable = TtlTable::new("ttl_purged");

/// The main database struct wrapping redb.
pub struct Database {
    db: redb::Database,
}

/// Result of garbage collection.
#[derive(Debug, Default)]
pub struct GcResult {
    /// Keys that were moved from Active → Trash
    pub trashed: Vec<Key>,
    /// Keys that were permanently deleted
    pub purged: Vec<Key>,
}

impl Database {
    /// Creates or opens a database using paths and settings from the config.
    pub fn new(config: Config) -> Result<Self, DatabaseError> {
        std::fs::create_dir_all(&config.base_path)?;

        let db = redb::Database::create(config.db_path())?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(MAIN_TABLE)?;
            let _ = write_txn.open_table(METADATA_TABLE)?;
            ACTIVE_EXPIRY.init(&write_txn)?;
            TRASH_EXPIRY.init(&write_txn)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }
}

/// Create operations.
impl Database {
    /// Creates a new key with empty attachments.
    ///
    /// Returns `Err(AlreadyExists)` if the key already exists.
    pub fn create(&mut self, key: &Key, now: SystemTime) -> Result<Value, DatabaseError> {
        let write_txn = self.db.begin_write()?;

        let new_value = Value {
            metadata: Metadata {
                lifecycle_state: LifecycleState::Active { last_accessed: now },
            },
            attachments: vec![],
            thumb_version: FileStorage::THUMB_VER,
        };

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            if main_table.get(key)?.is_some() {
                return Err(DatabaseError::AlreadyExists);
            }

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(new_value.clone()))?;
        }

        write_txn.commit()?;
        Ok(new_value)
    }
}

/// Read operations.
impl Database {
    /// Retrieves a value by key.
    pub fn get(&self, key: &Key) -> Result<Option<Value>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MAIN_TABLE)?;

        match table.get(key)? {
            None => Ok(None),
            Some(guard) => Ok(Some(Self::extract_latest(guard.value()))),
        }
    }

    /// Returns all Active keys.
    pub fn active_keys(&self) -> Result<Vec<Key>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        ACTIVE_EXPIRY.all_keys(&read_txn)
    }

    /// Returns all Trash keys.
    pub fn trashed_keys(&self) -> Result<Vec<Key>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        TRASH_EXPIRY.all_keys(&read_txn)
    }
}

/// Update operations.
impl Database {
    /// Updates `last_accessed` timestamp only.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    /// Returns `Err(Trashed)` if the key is trashed.
    pub fn touch(&mut self, key: &Key, now: SystemTime) -> Result<Value, DatabaseError> {
        let write_txn = self.db.begin_write()?;

        let mut value;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::Trashed);
            };

            Self::remove_active_ttl(&write_txn, key, last_accessed)?;
            Self::insert_active_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };

            main_table.insert(key, &VersionedValue::V1(value.clone()))?;
        }

        write_txn.commit()?;
        Ok(value)
    }

    /// Adds an attachment to a key.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    /// Returns `Err(Trashed)` if the key is trashed.
    pub fn add_attachment(
        &mut self,
        key: &Key,
        attachment: Attachment,
        now: SystemTime,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::Trashed);
            };

            value.attachments.push(attachment);

            Self::remove_active_ttl(&write_txn, key, last_accessed)?;
            Self::insert_active_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };

            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Removes an attachment by filename.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    /// Returns `Err(Trashed)` if the key is trashed.
    /// Returns `Err(AttachmentNotFound)` if the attachment doesn't exist.
    pub fn remove_attachment(
        &mut self,
        key: &Key,
        filename: &str,
        now: SystemTime,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::Trashed);
            };

            let pos = value
                .attachments
                .iter()
                .position(|a| a.filename == filename)
                .ok_or_else(|| DatabaseError::AttachmentNotFound(filename.to_string()))?;

            value.attachments.remove(pos);

            Self::remove_active_ttl(&write_txn, key, last_accessed)?;
            Self::insert_active_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };

            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Renames an attachment.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    /// Returns `Err(Trashed)` if the key is trashed.
    /// Returns `Err(AttachmentNotFound)` if the source attachment doesn't exist.
    ///
    /// If `new_filename` already exists, that entry is removed (overwritten).
    pub fn rename_attachment(
        &mut self,
        key: &Key,
        old_filename: &str,
        new_filename: &str,
        now: SystemTime,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::Trashed);
            };

            if let Some(dst_pos) = value
                .attachments
                .iter()
                .position(|a| a.filename == new_filename)
            {
                value.attachments.remove(dst_pos);
            }

            let attachment = value
                .attachments
                .iter_mut()
                .find(|a| a.filename == old_filename)
                .ok_or_else(|| DatabaseError::AttachmentNotFound(old_filename.to_string()))?;

            attachment.filename = new_filename.to_string();

            Self::remove_active_ttl(&write_txn, key, last_accessed)?;
            Self::insert_active_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };

            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Updates the thumb_version for a key.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    pub fn update_thumb_version(&mut self, key: &Key, version: u32) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            value.thumb_version = version;
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Renames a key, optionally overwriting destination.
    ///
    /// Returns `Err(NotFound)` if src doesn't exist.
    pub fn rename(&mut self, src: &Key, dst: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Extract destination lifecycle state before mutating
            let dest_state = main_table
                .get(dst)?
                .map(|g| Self::extract_latest(g.value()).metadata.lifecycle_state);

            // Clean up destination if it exists
            if let Some(state) = dest_state {
                match state {
                    LifecycleState::Active { last_accessed } => {
                        Self::remove_active_ttl(&write_txn, dst, last_accessed)?;
                    }
                    LifecycleState::Trash { trashed_at } => {
                        Self::remove_trash_ttl(&write_txn, dst, trashed_at)?;
                    }
                }
                main_table.remove(dst)?;
            }

            // Get and remove source
            let mut value = main_table
                .remove(src)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Remove old TTL entry and insert new one
            match value.metadata.lifecycle_state {
                LifecycleState::Active { last_accessed } => {
                    Self::remove_active_ttl(&write_txn, src, last_accessed)?;
                    Self::insert_active_ttl(&write_txn, dst, now)?;
                    value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };
                }
                LifecycleState::Trash { trashed_at } => {
                    Self::remove_trash_ttl(&write_txn, src, trashed_at)?;
                    Self::insert_trash_ttl(&write_txn, dst, trashed_at)?;
                }
            }

            main_table.insert(dst, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }
}

/// Delete operations.
impl Database {
    /// Soft-deletes a key by moving it from Active to Trash state.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::Trashed);
            };

            Self::remove_active_ttl(&write_txn, key, last_accessed)?;
            Self::insert_trash_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Trash { trashed_at: now };

            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Restores a key from Trash to Active state.
    pub fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            let LifecycleState::Trash { trashed_at } = value.metadata.lifecycle_state else {
                return Err(DatabaseError::NotTrashed);
            };

            Self::remove_trash_ttl(&write_txn, key, trashed_at)?;
            Self::insert_active_ttl(&write_txn, key, now)?;

            value.metadata.lifecycle_state = LifecycleState::Active { last_accessed: now };

            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Permanently deletes a key from the database.
    pub fn purge(&mut self, key: &Key) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let value = main_table
                .remove(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            match value.metadata.lifecycle_state {
                LifecycleState::Active { last_accessed } => {
                    Self::remove_active_ttl(&write_txn, key, last_accessed)?;
                }
                LifecycleState::Trash { trashed_at } => {
                    Self::remove_trash_ttl(&write_txn, key, trashed_at)?;
                }
            }
        }

        write_txn.commit()?;
        Ok(())
    }
}

/// Maintenance operations.
impl Database {
    /// Performs garbage collection and updates last_run_at timestamp.
    pub fn gc(&mut self, now: SystemTime, gc_config: GcConfig) -> Result<GcResult, DatabaseError> {
        let (to_trash, to_purge) = {
            let read_txn = self.db.begin_read()?;
            let to_trash =
                ACTIVE_EXPIRY.expired_keys(&read_txn, now, gc_config.trash_ttl)?;
            let to_purge =
                TRASH_EXPIRY.expired_keys(&read_txn, now, gc_config.purge_ttl)?;
            (to_trash, to_purge)
        };

        if to_trash.is_empty() && to_purge.is_empty() {
            self.set_maintenance_metadata(&MaintenanceMetadata {
                last_run_at: Some(now),
            })?;
            return Ok(GcResult::default());
        }

        let write_txn = self.db.begin_write()?;
        let mut result = GcResult::default();

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Move expired Active keys to Trash
            for key in to_trash {
                let value_opt = main_table
                    .get(&key)?
                    .map(|guard| Self::extract_latest(guard.value()));

                if let Some(mut value) = value_opt
                    && let LifecycleState::Active { last_accessed } = value.metadata.lifecycle_state
                {
                    Self::remove_active_ttl(&write_txn, &key, last_accessed)?;
                    Self::insert_trash_ttl(&write_txn, &key, now)?;

                    value.metadata.lifecycle_state = LifecycleState::Trash { trashed_at: now };

                    main_table.insert(&key, &VersionedValue::V1(value))?;
                    result.trashed.push(key);
                }
            }

            // Permanently delete expired Trash keys
            for key in to_purge {
                if let Some(value) = main_table
                    .remove(&key)?
                    .map(|guard| Self::extract_latest(guard.value()))
                {
                    let LifecycleState::Trash { trashed_at } = value.metadata.lifecycle_state
                    else {
                        continue;
                    };
                    Self::remove_trash_ttl(&write_txn, &key, trashed_at)?;
                    result.purged.push(key);
                }
            }

            // Update maintenance timestamp
            let metadata = MaintenanceMetadata {
                last_run_at: Some(now),
            };
            let json = serde_json::to_string(&metadata).expect("serialization failed");
            let mut meta_table = write_txn.open_table(METADATA_TABLE)?;
            meta_table.insert(METADATA_KEY_MAINTENANCE, json.as_str())?;
        }

        write_txn.commit()?;
        Ok(result)
    }
}

/// Internal helpers.
impl Database {
    fn extract_latest(versioned: VersionedValue) -> Value {
        match versioned {
            VersionedValue::V1(v) => v,
        }
    }
}

/// TTL table helpers.
impl Database {
    fn remove_active_ttl(
        txn: &redb::WriteTransaction,
        key: &Key,
        last_accessed: SystemTime,
    ) -> Result<(), DatabaseError> {
        let ttl_key = TtlKey {
            timestamp: last_accessed,
            key: key.clone(),
        };
        ACTIVE_EXPIRY.remove(txn, &ttl_key)?;
        Ok(())
    }

    fn remove_trash_ttl(
        txn: &redb::WriteTransaction,
        key: &Key,
        trashed_at: SystemTime,
    ) -> Result<(), DatabaseError> {
        let ttl_key = TtlKey {
            timestamp: trashed_at,
            key: key.clone(),
        };
        TRASH_EXPIRY.remove(txn, &ttl_key)?;
        Ok(())
    }

    fn insert_active_ttl(
        txn: &redb::WriteTransaction,
        key: &Key,
        timestamp: SystemTime,
    ) -> Result<(), DatabaseError> {
        let ttl_key = TtlKey {
            timestamp,
            key: key.clone(),
        };
        ACTIVE_EXPIRY.insert(txn, &ttl_key)?;
        Ok(())
    }

    fn insert_trash_ttl(
        txn: &redb::WriteTransaction,
        key: &Key,
        timestamp: SystemTime,
    ) -> Result<(), DatabaseError> {
        let ttl_key = TtlKey {
            timestamp,
            key: key.clone(),
        };
        TRASH_EXPIRY.insert(txn, &ttl_key)?;
        Ok(())
    }
}

/// Metadata operations.
impl Database {
    fn get_maintenance_metadata(&self) -> Option<MaintenanceMetadata> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(METADATA_TABLE).ok()?;
        let guard = table.get(METADATA_KEY_MAINTENANCE).ok()??;
        serde_json::from_str(guard.value()).ok()
    }

    fn set_maintenance_metadata(
        &mut self,
        metadata: &MaintenanceMetadata,
    ) -> Result<(), DatabaseError> {
        let json = serde_json::to_string(metadata).expect("serialization failed");
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert(METADATA_KEY_MAINTENANCE, json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn last_maintenance_at(&self) -> Option<SystemTime> {
        self.get_maintenance_metadata()?.last_run_at
    }

    /// Returns true if maintenance should run (never run or interval elapsed).
    pub fn should_run_maintenance(&self, now: SystemTime, interval: Duration) -> bool {
        match self.last_maintenance_at() {
            None => true,
            Some(last) => now.duration_since(last).map(|d| d >= interval).unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests;
