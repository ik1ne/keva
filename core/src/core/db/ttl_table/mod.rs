use crate::core::db::error::DatabaseError;
use crate::types::{Key, TtlKey};
use redb::{ReadTransaction, ReadableTable, TableDefinition, WriteTransaction};
use std::time::{Duration, SystemTime};

/// Stores `TtlKey { timestamp, key }` entries. Expiration is calculated at
/// query time as `timestamp + ttl_duration`, allowing TTL changes to immediately
/// affect all keys.
pub struct TtlTable {
    definition: TableDefinition<'static, TtlKey, ()>,
}

impl TtlTable {
    pub const fn new(name: &'static str) -> Self {
        Self {
            definition: TableDefinition::new(name),
        }
    }

    pub fn init(&self, txn: &WriteTransaction) -> Result<(), DatabaseError> {
        txn.open_table(self.definition)?;
        Ok(())
    }

    pub fn insert(&self, txn: &WriteTransaction, ttl_key: &TtlKey) -> Result<(), DatabaseError> {
        let mut table = txn.open_table(self.definition)?;
        table.insert(ttl_key, &())?;
        Ok(())
    }

    /// Returns `true` if the key was present.
    pub fn remove(&self, txn: &WriteTransaction, ttl_key: &TtlKey) -> Result<bool, DatabaseError> {
        let mut table = txn.open_table(self.definition)?;
        Ok(table.remove(ttl_key)?.is_some())
    }

    /// Returns keys where `timestamp + ttl_duration < now`, oldest first.
    pub fn expired_keys(
        &self,
        txn: &ReadTransaction,
        now: SystemTime,
        ttl_duration: Duration,
    ) -> Result<Vec<Key>, DatabaseError> {
        let Some(expires_at) = now.checked_sub(ttl_duration) else {
            // If ttl_duration is greater than now, no keys can be expired
            return Ok(vec![]);
        };

        let table = txn.open_table(self.definition)?;

        table
            .range(
                ..TtlKey {
                    timestamp: expires_at,
                    // SAFETY: This key is only used for range querying, so the empty value is not stored.
                    key: unsafe { Key::new_unchecked(String::new()) },
                },
            )?
            .map(|ttl_key| {
                let (ttl_key_guard, _) = ttl_key?;
                Ok(ttl_key_guard.value().key.clone())
            })
            .collect()
    }

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

#[cfg(test)]
mod tests;
