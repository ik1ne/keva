//! Blob storage mapping and inlined file management

use crate::storage::file::error::Error;
use crate::types::value::versioned_value::latest_value;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("File not found")]
        FileNotFound,
    }
}
pub mod blob {}
pub mod inline {}

pub struct FileStorage {
    pub inline_threshold_bytes: u64,
}

impl FileStorage {
    pub fn store_file(
        &self,
        key_path: &Path,
        file: &Path,
    ) -> Result<latest_value::FileData, Error> {
    }
    pub fn store_text(
        &self,
        key_path: &Path,
        text: Cow<'_, str>,
    ) -> Result<latest_value::TextData, Error> {
    }
    pub fn remove_file(&self, key_path: &Path, file: &latest_value::FileData) -> Result<(), Error> {
    }
    pub fn remove_text(&self, key_path: &Path, text: &latest_value::TextData) -> Result<(), Error> {
    }
    pub fn remove_all(&self, key_path: &Path) -> Result<(), Error> {}
    pub fn ensure_file_path(
        &self,
        key_path: &Path,
        file: &latest_value::FileData,
    ) -> Result<PathBuf, Error> {
    }
    pub fn ensure_text_path(
        &self,
        key_path: &Path,
        text: &latest_value::TextData,
    ) -> Result<PathBuf, Error> {
    }
    fn cleanup_cache(&self, keep: Option<&Path>) -> Result<(), Error> {}
}
