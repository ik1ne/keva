use crate::storage::db::error::DatabaseError;
use crate::storage::db::ttl::purged::PurgedTable;
use crate::storage::db::ttl::trashed::TrashedTable;
use crate::types::value::versioned_value::latest_value;
use crate::types::{Key, Value};
use std::borrow::Cow;
use std::path::Path;
use std::time::SystemTime;

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum DatabaseError {
        #[error("Database error: {0}")]
        Redb(#[from] redb::DatabaseError),

        #[error("Key not found")]
        NotFound,
    }
}
pub mod ttl {
    pub mod common {
        //! Shared generic table definition for TTL storage
    }
    pub mod trashed {
        pub struct TrashedTable;
    }
    pub mod purged {
        pub struct PurgedTable;
    }
}

pub struct Database {
    redb: redb::Database,
    trashed_table: TrashedTable,
    purged_table: PurgedTable,
}

pub struct InsertOperation {
    pub value: Value,
    pub is_overwrite: bool,
}

pub struct GcTargets {
    trashed_keys: Vec<Key>,
    purged_keys: Vec<Key>,
}

impl GcTargets {
    pub fn trashed_keys(&self) -> impl IntoIterator<Item = &Key> {
        self.trashed_keys.iter()
    }

    pub fn purged_keys(&self) -> impl IntoIterator<Item = &Key> {
        self.purged_keys.iter()
    }
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, DatabaseError> {
        // initialize redb, trashed_table, purged_table
    }

    pub fn get(&self, key: &Key) -> Result<Option<Value>, DatabaseError> {}

    /// Inserts value into main table.
    /// TTL tracking (trashed/purged tables) is updated internally based on lifecycle state.
    pub fn insert(
        &mut self,
        key: &Key,
        now: SystemTime,
        plain_text: Option<Cow<'_, str>>,
        files: Vec<latest_value::BlobStoredFileData>,
    ) -> Result<InsertOperation, DatabaseError> {
    }
    pub fn remove(&mut self, key: &Key) -> Result<Value, DatabaseError> {}
    pub fn rename(&mut self, old_key: &Key, new_key: &Key) -> Result<(), DatabaseError> {}
    pub fn keys(&self) -> Result<impl IntoIterator<Item = Key>, DatabaseError> {}
    pub fn gc_targets(&self, now: SystemTime) -> Result<GcTargets, DatabaseError> {}
    pub fn gc_finalize(&mut self, targets: GcTargets) -> Result<(), DatabaseError> {}
}
