//! Database layer for keva storage.
//!
//! This module handles all redb operations including:
//! - Main key-value storage (Key → VersionedValue)
//! - TTL tracking tables for garbage collection

use crate::storage::db::error::DatabaseError;
use crate::storage::db::ttl_table::TtlTable;
use crate::types::value::versioned_value::VersionedValue;
use crate::types::value::versioned_value::latest_value::{
    ClipData, FileData, LifecycleState, Metadata, Value as PersistedValue,
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

/// TTL table for tracking when Active keys should move to Trash.
const TRASHED_TTL: TtlTable = TtlTable::new("ttl_trashed");

/// TTL table for tracking when Trashed keys should be purged.
const PURGED_TTL: TtlTable = TtlTable::new("ttl_purged");

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
            TRASHED_TTL.init(&write_txn)?;
            PURGED_TTL.init(&write_txn)?;
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
    /// This always overwrites - the caller (Storage) is responsible for checking
    /// whether overwriting is allowed based on lifecycle state.
    ///
    /// Handles TTL table cleanup for any existing entry.
    pub fn insert(
        &mut self,
        key: &Key,
        now: SystemTime,
        clip_data: ClipData,
    ) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Check if key already exists and clean up TTL tables
            let existing_state = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .map(|v| {
                    (
                        v.metadata.lifecycle_state,
                        v.metadata.updated_at,
                        v.metadata.trashed_at,
                    )
                });

            if let Some((state, updated_at, trashed_at)) = existing_state {
                match state {
                    LifecycleState::Active => {
                        let old_ttl_key = TtlKey {
                            timestamp: updated_at,
                            key: key.clone(),
                        };
                        TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;
                    }
                    LifecycleState::Trash => {
                        if let Some(trashed_at) = trashed_at {
                            let old_ttl_key = TtlKey {
                                timestamp: trashed_at,
                                key: key.clone(),
                            };
                            PURGED_TTL.remove(&write_txn, &old_ttl_key)?;
                        }
                    }
                    LifecycleState::Purge => {
                        // No TTL entries to clean up
                    }
                }
            }

            // Build the new value
            let new_value = PersistedValue {
                metadata: Metadata {
                    created_at: now,
                    updated_at: now,
                    trashed_at: None,
                    lifecycle_state: LifecycleState::Active,
                },
                clip_data,
            };

            // Add TTL entry
            let ttl_key = TtlKey {
                timestamp: now,
                key: key.clone(),
            };
            TRASHED_TTL.insert(&write_txn, &ttl_key)?;

            // Store the value
            main_table.insert(key, &VersionedValue::V1(new_value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Appends files to an existing key's value.
    ///
    /// - Updates `updated_at` to `now`
    /// - Adds files to existing Files entry
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

            // Get existing value
            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Can only append to Active keys
            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            // Can only append to Files entries
            let existing_files = match &mut value.clip_data {
                ClipData::Files(f) => f,
                ClipData::Text(_) => return Err(DatabaseError::TypeMismatch),
            };

            // Remove old TTL entry
            let old_ttl_key = TtlKey {
                timestamp: value.metadata.updated_at,
                key: key.clone(),
            };
            TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;

            // Update timestamp
            value.metadata.updated_at = now;

            // Append files
            existing_files.extend(files);

            // Add new TTL entry
            let new_ttl_key = TtlKey {
                timestamp: now,
                key: key.clone(),
            };
            TRASHED_TTL.insert(&write_txn, &new_ttl_key)?;

            // Store updated value
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Updates the `updated_at` timestamp without modifying the value.
    ///
    /// This is used to prevent a key from being garbage collected when accessed.
    /// Returns `Err(NotFound)` if the key doesn't exist or is not Active.
    pub fn touch(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Only touch Active keys
            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            // Remove old TTL entry
            let old_ttl_key = TtlKey {
                timestamp: value.metadata.updated_at,
                key: key.clone(),
            };
            TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;

            // Update timestamp
            value.metadata.updated_at = now;

            // Add new TTL entry
            let new_ttl_key = TtlKey {
                timestamp: now,
                key: key.clone(),
            };
            TRASHED_TTL.insert(&write_txn, &new_ttl_key)?;

            // Store updated value
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Soft-deletes a key by moving it to Trash state.
    ///
    /// - Sets `lifecycle_state` to `Trash`
    /// - Sets `trashed_at` to `now`
    /// - Removes from trashed TTL table, adds to purged TTL table
    ///
    /// Returns `Err(NotFound)` if the key doesn't exist or is already trashed/purged.
    pub fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            let mut value = main_table
                .get(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Can only trash Active keys
            if value.metadata.lifecycle_state != LifecycleState::Active {
                return Err(DatabaseError::NotFound);
            }

            // Remove from trashed TTL table
            let old_ttl_key = TtlKey {
                timestamp: value.metadata.updated_at,
                key: key.clone(),
            };
            TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;

            // Update state
            value.metadata.lifecycle_state = LifecycleState::Trash;
            value.metadata.trashed_at = Some(now);

            // Add to purged TTL table
            let new_ttl_key = TtlKey {
                timestamp: now,
                key: key.clone(),
            };
            PURGED_TTL.insert(&write_txn, &new_ttl_key)?;

            // Store updated value
            main_table.insert(key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Permanently deletes a key from the database.
    ///
    /// This removes the key from both the main table and any TTL tables.
    /// Returns `Err(NotFound)` if the key doesn't exist.
    pub fn purge(&mut self, key: &Key) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Get and remove the value
            let value = main_table
                .remove(key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Remove from appropriate TTL table based on state
            match value.metadata.lifecycle_state {
                LifecycleState::Active => {
                    let ttl_key = TtlKey {
                        timestamp: value.metadata.updated_at,
                        key: key.clone(),
                    };
                    TRASHED_TTL.remove(&write_txn, &ttl_key)?;
                }
                LifecycleState::Trash => {
                    if let Some(trashed_at) = value.metadata.trashed_at {
                        let ttl_key = TtlKey {
                            timestamp: trashed_at,
                            key: key.clone(),
                        };
                        PURGED_TTL.remove(&write_txn, &ttl_key)?;
                    }
                }
                LifecycleState::Purge => {
                    // Already marked for purge, nothing in TTL tables
                }
            }
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Renames a key, overwriting any existing entry at new_key.
    ///
    /// Returns `Err(NotFound)` if the old key doesn't exist.
    ///
    /// This always overwrites new_key if it exists - the caller (Storage) is
    /// responsible for checking whether overwriting is allowed.
    ///
    /// Handles TTL table cleanup for both old_key and any existing new_key.
    pub fn rename(&mut self, old_key: &Key, new_key: &Key) -> Result<(), DatabaseError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Check if new key exists and clean up its TTL tables
            let new_key_state = main_table
                .get(new_key)?
                .map(|g| Self::extract_latest(g.value()))
                .map(|v| {
                    (
                        v.metadata.lifecycle_state,
                        v.metadata.updated_at,
                        v.metadata.trashed_at,
                    )
                });

            if let Some((state, updated_at, trashed_at)) = new_key_state {
                match state {
                    LifecycleState::Active => {
                        let ttl_key = TtlKey {
                            timestamp: updated_at,
                            key: new_key.clone(),
                        };
                        TRASHED_TTL.remove(&write_txn, &ttl_key)?;
                    }
                    LifecycleState::Trash => {
                        if let Some(trashed_at) = trashed_at {
                            let ttl_key = TtlKey {
                                timestamp: trashed_at,
                                key: new_key.clone(),
                            };
                            PURGED_TTL.remove(&write_txn, &ttl_key)?;
                        }
                    }
                    LifecycleState::Purge => {
                        // No TTL entries to clean up
                    }
                }
                // Remove the existing entry from main table
                main_table.remove(new_key)?;
            }

            // Get and remove old entry, extract value immediately to release borrow
            let value = main_table
                .remove(old_key)?
                .map(|g| Self::extract_latest(g.value()))
                .ok_or(DatabaseError::NotFound)?;

            // Update TTL table (remove old key, insert new key)
            match value.metadata.lifecycle_state {
                LifecycleState::Active => {
                    let old_ttl_key = TtlKey {
                        timestamp: value.metadata.updated_at,
                        key: old_key.clone(),
                    };
                    let new_ttl_key = TtlKey {
                        timestamp: value.metadata.updated_at,
                        key: new_key.clone(),
                    };
                    TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;
                    TRASHED_TTL.insert(&write_txn, &new_ttl_key)?;
                }
                LifecycleState::Trash => {
                    if let Some(trashed_at) = value.metadata.trashed_at {
                        let old_ttl_key = TtlKey {
                            timestamp: trashed_at,
                            key: old_key.clone(),
                        };
                        let new_ttl_key = TtlKey {
                            timestamp: trashed_at,
                            key: new_key.clone(),
                        };
                        PURGED_TTL.remove(&write_txn, &old_ttl_key)?;
                        PURGED_TTL.insert(&write_txn, &new_ttl_key)?;
                    }
                }
                LifecycleState::Purge => {
                    // No TTL entries
                }
            }

            // Insert with new key
            main_table.insert(new_key, &VersionedValue::V1(value))?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Returns all keys in the database.
    pub fn keys(&self) -> Result<Vec<Key>, DatabaseError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MAIN_TABLE)?;

        let mut keys = Vec::new();
        for entry in table.iter()? {
            let (key, _) = entry?;
            keys.push(key.value());
        }

        Ok(keys)
    }

    /// Performs garbage collection.
    ///
    /// This method:
    /// 1. Finds all keys that have exceeded their TTL
    /// 2. Moves Active keys to Trash
    /// 3. Permanently deletes Trashed keys
    ///
    /// Returns `GcResult` containing the keys that were trashed and purged,
    /// so the caller can perform filesystem cleanup for blob files.
    pub fn gc(&mut self, now: SystemTime) -> Result<GcResult, DatabaseError> {
        // First, find expired keys using read transaction
        let (to_trash, to_purge) = {
            let read_txn = self.db.begin_read()?;
            let to_trash = TRASHED_TTL.expired_keys(&read_txn, now, self.config.saved.trash_ttl)?;
            let to_purge = PURGED_TTL.expired_keys(&read_txn, now, self.config.saved.purge_ttl)?;
            (to_trash, to_purge)
        };

        if to_trash.is_empty() && to_purge.is_empty() {
            return Ok(GcResult::default());
        }

        let write_txn = self.db.begin_write()?;
        let mut result = GcResult::default();

        {
            let mut main_table = write_txn.open_table(MAIN_TABLE)?;

            // Process keys to trash
            for key in to_trash {
                let value_opt = main_table
                    .get(&key)?
                    .map(|guard| Self::extract_latest(guard.value()));

                if let Some(mut value) = value_opt {
                    // Only process if still Active
                    if value.metadata.lifecycle_state == LifecycleState::Active {
                        // Remove from trashed TTL table
                        let old_ttl_key = TtlKey {
                            timestamp: value.metadata.updated_at,
                            key: key.clone(),
                        };
                        TRASHED_TTL.remove(&write_txn, &old_ttl_key)?;

                        // Update state
                        value.metadata.lifecycle_state = LifecycleState::Trash;
                        value.metadata.trashed_at = Some(now);

                        // Add to purged TTL table
                        let new_ttl_key = TtlKey {
                            timestamp: now,
                            key: key.clone(),
                        };
                        PURGED_TTL.insert(&write_txn, &new_ttl_key)?;

                        // Store updated value
                        main_table.insert(&key, &VersionedValue::V1(value))?;

                        result.trashed.push(key);
                    }
                }
            }

            // Process keys to purge
            for key in to_purge {
                let value_opt = main_table
                    .remove(&key)?
                    .map(|guard| Self::extract_latest(guard.value()));

                if let Some(value) = value_opt {
                    // Remove from purged TTL table
                    if let Some(trashed_at) = value.metadata.trashed_at {
                        let ttl_key = TtlKey {
                            timestamp: trashed_at,
                            key: key.clone(),
                        };
                        PURGED_TTL.remove(&write_txn, &ttl_key)?;
                    }

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

#[cfg(test)]
mod tests;
