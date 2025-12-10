use crate::error::Result;
use crate::types::config::Config;
use crate::types::key::Key;
use crate::types::value::Value;

pub struct Storage {
    // redb database handle
    // blob storage path
}

impl Storage {
    pub fn open(config: &Config) -> Result<Self> {
        todo!()
    }

    pub fn get(&self, key: &Key) -> Result<Option<Value>> {
        todo!()
    }

    pub fn set(&mut self, key: &Key, value: Value) -> Result<()> {
        todo!()
    }

    pub fn rm(&mut self, key: &Key) -> Result<()> {
        todo!()
    }

    pub fn mv(&mut self, from: &Key, to: &Key, force: bool) -> Result<()> {
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
    pub keys_trashed: usize,
    pub keys_purged: usize,
    pub blobs_deleted: usize,
    pub bytes_reclaimed: u64,
}
