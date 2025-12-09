mod model;

pub use model::Value;
use std::path::Path;

#[derive(Debug)]
pub enum KevaError {
    Io(std::io::Error),
    StoreError(String),
    NotFound(String),
}

impl From<std::io::Error> for KevaError {
    fn from(e: std::io::Error) -> Self {
        KevaError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, KevaError>;

pub struct KevaStore {
    // fields to be added later
}

impl KevaStore {
    pub fn open(_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {})
    }

    pub fn get(&self, _key: &str) -> Result<Option<Value>> {
        Ok(None)
    }

    pub fn set(&mut self, _key: &str, _value: Value) -> Result<()> {
        Ok(())
    }

    pub fn pdel(&mut self, _key: &str) -> Result<()> {
        Ok(())
    }

    pub fn ls(&self, _key: &str) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
