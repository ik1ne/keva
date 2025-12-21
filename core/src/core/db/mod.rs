//! Database layer for keva storage.
//!
//! This module handles all redb operations including:
//! - Main key-value storage (Key → VersionedValue)
//! - TTL tracking tables for garbage collection

use crate::core::db::error::DatabaseError;
use crate::core::db::ttl_table::TtlTable;
use crate::types::value::versioned_value::VersionedValue;
use crate::types::value::versioned_value::latest_value::{
    ClipData, FileData, LifecycleState, Metadata, TextData, Value as PersistedValue,
};
use crate::types::{Config, Key, TtlKey};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use std::time::SystemTime;

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

        #[error("Key not found")]
        NotFound,

        #[error("Cannot append files to text entry")]
        TypeMismatch,
    }
}

mod ttl_table;

/// Main table: Key → VersionedValue
const MAIN_TABLE: TableDefinition<Key, VersionedValue> = TableDefinition::new("main");

/// TTL table tracking when Active keys expire to Trash.
const ACTIVE_EXPIRY: TtlTable = TtlTable::new("ttl_trashed");

/// TTL table tracking when Trash keys expire to Purge.
const TRASH_EXPIRY: TtlTable = TtlTable::new("ttl_purged");

/// The main database struct wrapping redb.
pub struct Database {
    db: redb::Database,
    config: Config,
}

/// Result of garbage collection, containing keys that were modified.
#[derive(Debug, Default)]
pub struct GcResult {
    /// Keys that were moved from Active → Trash
    pub trashed: Vec<Key>,
    /// Keys that were permanently deleted (for filesystem cleanup)
    pub purged: Vec<Key>,
}

impl GcResult {
    pub fn is_empty(&self) -> bool {
        self.trashed.is_empty() && self.purged.is_empty()
    }
}

impl Database {
    /// Creates or opens a database using paths and settings from the config.
    pub fn new(config: Config) -> Result<Self, DatabaseError> {
        let db = redb::Database::create(config.db_path())?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(MAIN_TABLE)?;
            ACTIVE_EXPIRY.init(&write_txn)?;
            TRASH_EXPIRY.init(&write_txn)?;
        }
        write_txn.commit()?;

        Ok(Self { db, config })
    }

    /// Retrieves a value by key.
    ///
    /// Returns `None` if the key doesn't exist.
    ///
    /// Note: This returns `PersistedValue` which may contain `BlobStored` markers.
    /// The caller (Storage) is responsible for resolving these to actual content.
    pub fn get(&self, key: &Key) -> Result<Option<PersistedValue>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MAIN_TABLE)?;

        match table.get(key)? {
            Some(guard) => {
                let versioned = guard.value();
                Ok(Some(Self::extract_latest(versioned)))
            }
            None => Ok(None),
        }
    }

    /// Inserts a key-value pair, overwriting any existing entry.
    ///
    /// If the key already exists (in any lifecycle state), its TTL entry is removed
    /// before inserting the new value. This prevents stale TTL entries.
    pub fn insert(
        &mut self,
        key: &Key,
        now: SystemTime,
        clip_data: ClipData,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Clean up TTL tables if key already exists
            if let Some(existing) = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
            {
                Self::remove_ttl_entry(&write_txn, key, &existing.metadata)?;
            }

            // Build and store the new value
            let new_value = PersistedValue {
                metadata: Metadata {
                    created_at: now,
                    updated_at: now,
                    last_accessed: now,
                    trashed_at: None,
                    lifecycle_state: LifecycleState::Active,
                },
                clip_data,
            };

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(new_value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Appends files to an existing Files entry.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is not Active.
    /// Returns `Err(TypeMismatch)` if the key contains Text instead of Files.
    pub fn append_files(
        &mut self,
        key: &Key,
        now: SystemTime,
        files: Vec<FileData>,
    ) -> Result<(), DatabaseError> {
        if files.is_empty() {
            return Ok(());
        }

        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            let existing_files = match &mut value.clip_data {
                ClipData::Files(f) => f,
                ClipData::Text(_) => return Err(DatabaseError::TypeMismatch),
            };

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            value.metadata.updated_at = now;
            value.metadata.last_accessed = now;
            existing_files.extend(files);

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Updates `last_accessed` timestamp to prevent garbage collection.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is not Active.
    pub fn touch(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            value.metadata.last_accessed = now;

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Updates an existing key's clip_data, preserving created_at.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is not Active.
    pub fn update(
        &mut self,
        key: &Key,
        now: SystemTime,
        clip_data: ClipData,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            let new_value = PersistedValue {
                metadata: Metadata {
                    created_at: value.metadata.created_at,
                    updated_at: now,
                    last_accessed: now,
                    trashed_at: None,
                    lifecycle_state: LifecycleState::Active,
                },
                clip_data,
            };

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(new_value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Soft-deletes a key by moving it from Active to Trash state.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is not Active.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            value.metadata.lifecycle_state = LifecycleState::Trash;
            value.metadata.trashed_at = Some(now);

            Self::insert_trash_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Restores a key from Trash to Active state.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is not in Trash state.
    pub fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Trash {
                return Err(DatabaseError::NotFound);
            }

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            value.metadata.lifecycle_state = LifecycleState::Active;
            value.metadata.trashed_at = None;
            value.metadata.updated_at = now;
            value.metadata.last_accessed = now;

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Permanently deletes a key from the database.
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist.
    pub fn purge(&mut self, key: &Key) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let value = main_table
                .remove(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Removes a single file entry from a Files value by index.
    ///
    /// When removing the last file, converts to empty text (empty value = empty text).
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist, is not Active, or index is out of bounds.
    /// Returns `Err(TypeMismatch)` if the key contains Text instead of Files.
    ///
    /// Returns the removed file entry so the caller can clean up blob storage.
    pub fn remove_file_at(
        &mut self,
        key: &Key,
        now: SystemTime,
        index: usize,
    ) -> Result<FileData, DatabaseError> {
        let write_txn = self.db.begin_write()?;

        let removed_file = {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            let files = match &mut value.clip_data {
                ClipData::Files(files) => files,
                ClipData::Text(_) => return Err(DatabaseError::TypeMismatch),
            };

            if index >= files.len() {
                return Err(DatabaseError::NotFound);
            }

            Self::remove_ttl_entry(&write_txn, key, &value.metadata)?;

            let removed = files.remove(index);

            if files.is_empty() {
                value.clip_data = ClipData::Text(TextData::Inlined(String::new()));
            }

            value.metadata.updated_at = now;
            value.metadata.last_accessed = now;

            Self::insert_active_ttl(&write_txn, key, now)?;
            main_table.insert(key, &VersionedValue::V1(value))?;

            removed
        };

        write_txn.commit()?;
        Ok(removed_file)
    }

    /// Renames a key, overwriting any existing entry at new_key.
    ///
    /// If new_key exists (in any lifecycle state), it is fully removed before the rename.
    /// The source key's TTL entry is transferred with the same timestamp (preserving expiration).
    ///
    /// Returns `Err(NotFound)` if old_key doesn't exist.
    pub fn rename(&mut self, old_key: &Key, new_key: &Key) -> Result<(), DatabaseError> {
        if old_key == new_key {
            return Ok(());
        }

        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Extract destination metadata before mutating (to avoid borrow conflict)
            let dest_metadata = main_table
                .get(new_key)?
                .map(|g| Self::extract_latest(g.value()).metadata);

            // Clean up destination if it exists
            if let Some(metadata) = &dest_metadata {
                Self::remove_ttl_entry(&write_txn, new_key, metadata)?;
                main_table.remove(new_key)?;
            }

            // Get and remove source
            let value = main_table
                .remove(old_key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Transfer TTL entry from old_key to new_key
            Self::transfer_ttl_entry(&write_txn, old_key, new_key, &value.metadata)?;

            main_table.insert(new_key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Returns all Active keys by iterating the ACTIVE_EXPIRY table.
    ///
    /// This is more efficient than iterating the main table and decoding values.
    pub fn active_keys(&self) -> Result<Vec<Key>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        ACTIVE_EXPIRY.all_keys(&read_txn)
    }

    /// Returns all Trash keys by iterating the TRASH_EXPIRY table.
    ///
    /// This is more efficient than iterating the main table and decoding values.
    pub fn trashed_keys(&self) -> Result<Vec<Key>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        TRASH_EXPIRY.all_keys(&read_txn)
    }

    /// Performs garbage collection: moves expired Active keys to Trash, deletes expired Trash keys.
    ///
    /// Returns `GcResult` with keys that were trashed/purged for filesystem cleanup.
    pub fn gc(&mut self, now: SystemTime) -> Result<GcResult, DatabaseError> {
        let (to_trash, to_purge) = {
            let read_txn = self.db.begin_read()?;
            let to_trash =
                ACTIVE_EXPIRY.expired_keys(&read_txn, now, self.config.saved.trash_ttl)?;
            let to_purge =
                TRASH_EXPIRY.expired_keys(&read_txn, now, self.config.saved.purge_ttl)?;
            (to_trash, to_purge)
        };

        if to_trash.is_empty() && to_purge.is_empty() {
            return Ok(GcResult::default());
        }

        let write_txn = self.db.begin_write()?;
        let mut result = GcResult::default();

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Move expired Active keys to Trash
            for key in to_trash {
                // Extract value to avoid borrow conflict with insert
                let value_opt = main_table
                    .get(&key)?
                    .map(|guard| Self::extract_latest(guard.value()));

                if let Some(mut value) = value_opt
                    && value.metadata.lifecycle_state == LifecycleState::Active
                {
                    Self::remove_ttl_entry(&write_txn, &key, &value.metadata)?;

                    value.metadata.lifecycle_state = LifecycleState::Trash;
                    value.metadata.trashed_at = Some(now);

                    Self::insert_trash_ttl(&write_txn, &key, now)?;
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
                    Self::remove_ttl_entry(&write_txn, &key, &value.metadata)?;
                    result.purged.push(key);
                }
            }
        }

        write_txn.commit()?;
        Ok(result)
    }

    /// Extracts the latest value format from a VersionedValue.
    fn extract_latest(versioned: VersionedValue) -> PersistedValue {
        match versioned {
            VersionedValue::V1(v) => v,
        }
    }
}

/// TTL table helpers for managing key expiration entries.
///
/// These helpers reduce duplication when manipulating TTL entries across
/// lifecycle state transitions (Active → Trash → Purge).
impl Database {
    /// Removes a key's TTL entry based on its lifecycle state.
    fn remove_ttl_entry(
        txn: &redb::WriteTransaction,
        key: &Key,
        metadata: &Metadata,
    ) -> Result<(), DatabaseError> {
        match metadata.lifecycle_state {
            LifecycleState::Active => {
                let ttl_key = TtlKey {
                    timestamp: metadata.last_accessed,
                    key: key.clone(),
                };
                ACTIVE_EXPIRY.remove(txn, &ttl_key)?;
            }
            LifecycleState::Trash => {
                if let Some(trashed_at) = metadata.trashed_at {
                    let ttl_key = TtlKey {
                        timestamp: trashed_at,
                        key: key.clone(),
                    };
                    TRASH_EXPIRY.remove(txn, &ttl_key)?;
                }
            }
            LifecycleState::Purge => {
                // No TTL entries to clean up
            }
        }
        Ok(())
    }

    /// Inserts a TTL entry for an Active key (tracks Active -> Trash expiration).
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

    /// Inserts a TTL entry for a Trashed key (tracks Trash -> Purge expiration).
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

    /// Transfers a key's TTL entry to a new key, preserving the same timestamp.
    fn transfer_ttl_entry(
        txn: &redb::WriteTransaction,
        old_key: &Key,
        new_key: &Key,
        metadata: &Metadata,
    ) -> Result<(), DatabaseError> {
        match metadata.lifecycle_state {
            LifecycleState::Active => {
                let old_ttl = TtlKey {
                    timestamp: metadata.last_accessed,
                    key: old_key.clone(),
                };
                let new_ttl = TtlKey {
                    timestamp: metadata.last_accessed,
                    key: new_key.clone(),
                };
                ACTIVE_EXPIRY.remove(txn, &old_ttl)?;
                ACTIVE_EXPIRY.insert(txn, &new_ttl)?;
            }
            LifecycleState::Trash => {
                if let Some(trashed_at) = metadata.trashed_at {
                    let old_ttl = TtlKey {
                        timestamp: trashed_at,
                        key: old_key.clone(),
                    };
                    let new_ttl = TtlKey {
                        timestamp: trashed_at,
                        key: new_key.clone(),
                    };
                    TRASH_EXPIRY.remove(txn, &old_ttl)?;
                    TRASH_EXPIRY.insert(txn, &new_ttl)?;
                }
            }
            LifecycleState::Purge => {
                // No TTL entries to transfer
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
