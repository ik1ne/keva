use crate::storage::error::StorageError;
use crate::types::{Config, Key, Value};
use std::path::Path;

pub(crate) mod db;
pub(crate) mod file;

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum StorageError {}
}

pub struct Storage {
    db: db::Database,
    file: file::FileStorage,
    config: Config,
}

impl Storage {
    pub fn open(config: Config) -> Result<Self, StorageError> {}

    // CRUD
    pub fn get(&self, key: &Key) -> Result<Option<Value>, StorageError> {}
    pub fn insert_text(&mut self, key: &Key, text: &str) -> Result<Value, StorageError> {}
    pub fn insert_from_clipboard(&mut self, key: &Key) -> Result<Value, StorageError> {}
    pub fn add_files(
        &mut self,
        key: &Key,
        file_path: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<Value, StorageError> {
    }
    pub fn add_from_clipboard(&mut self, key: &Key) -> Result<Value, StorageError> {}
    pub fn delete(&mut self, key: &Key) -> Result<Option<Value>, StorageError> {}
    pub fn rename(&mut self, old_key: &Key, new_key: &Key) -> Result<(), StorageError> {}

    // Query
    pub fn keys(&self) -> Result<Vec<Key>, StorageError> {}
    pub fn list(&self, prefix: &str) -> Result<Vec<Key>, StorageError> {}

    // Maintenance
    pub fn gc(&mut self) -> Result<(), StorageError> {}
}
