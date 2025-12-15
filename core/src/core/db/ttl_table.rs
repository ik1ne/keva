//! Generic TTL table abstraction for lifecycle management.
//!
//! This module provides a reusable TTL table that can track keys with timestamps
//! for garbage collection purposes.

use crate::core::db::error::DatabaseError;
use crate::types::{Key, TtlKey};
use redb::{ReadTransaction, ReadableTable, TableDefinition, WriteTransaction};
use std::time::{Duration, SystemTime};

/// A generic TTL (Time-To-Live) table for tracking key expiration.
///
/// TTL tables store `TtlKey { timestamp, key }` entries where:
/// - `timestamp` is the base time for TTL calculation (e.g., `updated_at` or `trashed_at`)
/// - Expiration is calculated at query time as `timestamp + ttl_duration`
///
/// This allows TTL duration changes to immediately affect all keys.
pub struct TtlTable {
    definition: TableDefinition<'static, TtlKey, ()>,
}

impl TtlTable {
    /// Creates a new TTL table with the given name.
    pub const fn new(name: &'static str) -> Self {
        Self {
            definition: TableDefinition::new(name),
        }
    }

    /// Initializes the table in the database (creates if not exists).
    pub fn init(&self, txn: &WriteTransaction) -> Result<(), DatabaseError> {
        txn.open_table(self.definition)?;
        Ok(())
    }

    /// Inserts a key with its timestamp into the TTL table.
    pub fn insert(&self, txn: &WriteTransaction, ttl_key: &TtlKey) -> Result<(), DatabaseError> {
        let mut table = txn.open_table(self.definition)?;
        table.insert(ttl_key, &())?;
        Ok(())
    }

    /// Removes a key from the TTL table.
    ///
    /// Returns `true` if the key was present, `false` otherwise.
    pub fn remove(&self, txn: &WriteTransaction, ttl_key: &TtlKey) -> Result<bool, DatabaseError> {
        let mut table = txn.open_table(self.definition)?;
        Ok(table.remove(ttl_key)?.is_some())
    }

    /// Finds all keys that have expired based on the given TTL duration.
    ///
    /// A key is considered expired if `timestamp + ttl_duration <= now`.
    ///
    /// Returns keys in timestamp order (oldest first).
    pub fn expired_keys(
        &self,
        txn: &ReadTransaction,
        now: SystemTime,
        ttl_duration: Duration,
    ) -> Result<Vec<Key>, DatabaseError> {
        let table = txn.open_table(self.definition)?;
        let mut expired = Vec::new();

        for entry in table.iter()? {
            let (ttl_key_guard, _) = entry?;
            let ttl_key = ttl_key_guard.value();

            let expires_at = ttl_key.timestamp + ttl_duration;
            if expires_at <= now {
                expired.push(ttl_key.key);
            } else {
                // Table is sorted by timestamp, so we can stop early
                break;
            }
        }

        Ok(expired)
    }

    /// Returns all keys in this TTL table.
    ///
    /// This iterates the entire table and collects all keys.
    pub fn all_keys(&self, txn: &ReadTransaction) -> Result<Vec<Key>, DatabaseError> {
        let table = txn.open_table(self.definition)?;
        let mut keys = Vec::new();

        for entry in table.iter()? {
            let (ttl_key_guard, _) = entry?;
            keys.push(ttl_key_guard.value().key);
        }

        Ok(keys)
    }
}
