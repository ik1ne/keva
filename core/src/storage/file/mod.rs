//! Blob storage mapping and inlined file management

use crate::storage::file::error::FileStorageError;
use crate::types::value::versioned_value::ValueVariant;
use crate::types::value::versioned_value::latest_value::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum FileStorageError {
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("Directory not supported")]
        IsDirectory,

        #[error("File name is not valid UTF-8")]
        NonUtf8FileName,
    }
}

pub struct FileStorage {
    pub base_path: PathBuf,
    pub inline_threshold_bytes: u64,
}

pub(crate) const TEXT_FILE_NAME: &str = "text.txt";

impl FileStorage {
    pub fn store_file(&self, key_path: &Path, file: &Path) -> Result<FileData, FileStorageError> {
        let metadata = std::fs::metadata(file)?;

        let is_inline = metadata.len() <= self.inline_threshold_bytes;
        if metadata.is_dir() {
            return Err(FileStorageError::IsDirectory);
        }
        let file_name: String = file
            .file_name()
            .ok_or(FileStorageError::IsDirectory)?
            .to_str()
            .ok_or(FileStorageError::NonUtf8FileName)?
            .to_string();

        if is_inline {
            Ok(FileData::Inlined(InlineFileData {
                file_name,
                data: std::fs::read(file)?,
            }))
        } else {
            let mut hasher = <Value as ValueVariant>::Hasher::new();
            let mut file_handle = std::fs::File::open(file)?;
            std::io::copy(&mut file_handle, &mut hasher)?;
            let hash = hasher.finalize();

            let file_path = self
                .base_path
                .join(key_path)
                .join(hash.to_string())
                .join(&file_name);

            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(file, file_path)?;

            Ok(FileData::BlobStored(BlobStoredFileData { file_name, hash }))
        }
    }

    pub fn store_text(
        &self,
        key_path: &Path,
        text: Cow<'_, str>,
    ) -> Result<TextData, FileStorageError> {
        let is_inline = text.len() as u64 <= self.inline_threshold_bytes;

        if is_inline {
            Ok(TextData::Inlined(text.into_owned()))
        } else {
            let text_path = self.base_path.join(key_path).join(TEXT_FILE_NAME);

            if let Some(parent) = text_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(text_path, text.as_bytes())?;

            Ok(TextData::BlobStored)
        }
    }

    pub fn remove_file(
        &self,
        key_path: &Path,
        BlobStoredFileData { file_name, hash }: &BlobStoredFileData,
    ) -> Result<(), FileStorageError> {
        let file_path = self
            .base_path
            .join(key_path)
            .join(hash.to_string())
            .join(file_name);
        if file_path.exists() {
            std::fs::remove_file(file_path.as_path())?;
        }

        let Some(hash_path) = file_path.parent() else {
            return Ok(());
        };

        if hash_path.read_dir()?.next().is_none() {
            std::fs::remove_dir(hash_path)?;
        }

        let Some(key_path_dir) = hash_path.parent() else {
            return Ok(());
        };

        if key_path_dir.read_dir()?.next().is_none() {
            std::fs::remove_dir(key_path_dir)?;
        }

        Ok(())
    }

    pub fn remove_text(&self, key_path: &Path, text: &TextData) -> Result<(), FileStorageError> {
        match text {
            TextData::Inlined(_) => Ok(()),
            TextData::BlobStored => {
                let text_path = self.base_path.join(key_path).join(TEXT_FILE_NAME);
                if text_path.exists() {
                    std::fs::remove_file(text_path.as_path())?;
                }

                let Some(key_path_dir) = text_path.parent() else {
                    return Ok(());
                };

                if key_path_dir.read_dir()?.next().is_none() {
                    std::fs::remove_dir(key_path_dir)?;
                }

                Ok(())
            }
        }
    }

    pub fn remove_all(&self, key_path: &Path) -> Result<(), FileStorageError> {
        todo!()
    }

    pub fn ensure_file_path(
        &self,
        key_path: &Path,
        file: &FileData,
    ) -> Result<PathBuf, FileStorageError> {
        todo!()
    }

    pub fn ensure_text_path(
        &self,
        key_path: &Path,
        text: &TextData,
    ) -> Result<PathBuf, FileStorageError> {
        todo!()
    }

    fn cleanup_cache(&self, keep: Option<&Path>) -> Result<(), FileStorageError> {
        todo!()
    }
}

#[cfg(test)]
mod tests;
