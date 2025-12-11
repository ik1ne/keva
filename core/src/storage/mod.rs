use crate::types::config::Config;
use crate::types::key::Key;
use crate::types::value::schema::VersionedValue;
use redb::{ReadableDatabase, TableDefinition};
use std::fs::create_dir_all;

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum StorageError {
        #[error(transparent)]
        IoError(#[from] std::io::Error),
        #[error(transparent)]
        RedbDbError(#[from] redb::DatabaseError),
        #[error(transparent)]
        RedbTransactionError(#[from] redb::TransactionError),
    }
}

pub type Result<T> = std::result::Result<T, error::StorageError>;

#[must_use = "feed this to SearchIndex to keep it in sync"]
pub enum StorageEvent {
    None,
    KeyAdded(Key),
    KeyRemoved(Key),
    KeyTrashed(Key),
    KeyRestored(Key),
    KeyRenamed { from: Key, to: Key },
}

pub struct Storage {
    redb: redb::Database,
    config: Config,
}

const KV_TABLE: TableDefinition<Key, VersionedValue> = TableDefinition::new("kv_table");

impl Storage {
    pub fn new(config: Config) -> Result<Self> {
        create_dir_all(&config.storage_path)?;
        let redb = redb::Database::open(&config.storage_path)?;
        create_dir_all(&config.blob_path)?;

        Ok(Self { redb, config })
    }

    pub fn get(&self, key: &Key) -> Result<Option<VersionedValue>> {
        let txn = self.redb.begin_read()?;
        txn.open_table()
    }

    pub fn set(&mut self, key: &Key, value: VersionedValue) -> Result<StorageEvent> {
        todo!()
    }

    pub fn rm(&mut self, key: &Key) -> Result<StorageEvent> {
        todo!()
    }

    pub fn mv(&mut self, from: &Key, to: &Key, force: bool) -> Result<StorageEvent> {
        todo!()
    }

    pub fn list(&self, prefix: &Key) -> Result<Vec<Key>> {
        todo!()
    }

    pub fn all_keys(&self) -> Result<Vec<Key>> {
        todo!()
    }

    pub fn gc(&mut self) -> Result<GcStats> {
        todo!()
    }
}

pub struct GcStats {
    pub storage_events: Vec<StorageEvent>,
    pub keys_trashed: usize,
    pub keys_purged: usize,
    pub blobs_deleted: usize,
    pub bytes_reclaimed: u64,
}
