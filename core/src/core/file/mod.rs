//! Blob storage mapping and inlined file management

use crate::core::file::error::FileStorageError;
use crate::types::value::versioned_value::ValueVariant;
use crate::types::value::versioned_value::file_hash::FileHasher;
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

/// Manages file storage and out-lining inlined files.
///
/// # Fields
/// - `base_path`: The base directory where blob-stored files are kept.
/// - `inline_threshold_bytes`: The size threshold (in bytes) for inlining files.
///
/// # File Storage Structure
/// Blob-stored files are organized in `{base_path}/{key_path}/{file_hash}/{file_name}`.
///
/// # Text Storage Structure
/// Blob-stored text files are stored in `{base_path}/{key_path}/text.txt`.
///
/// # Inlined Files/Text
/// Inlined files are stored inside [`db`](crate::core::db::Database).
///
/// # Ensuring File Paths
/// The `ensure_file_path` method provides a way to get a filesystem path for both
/// inlined and blob-stored files, writing inlined files to a temporary location if necessary.
pub struct FileStorage {
    pub base_path: PathBuf,
    pub inline_threshold_bytes: u64,
}

pub(crate) const TEXT_FILE_NAME: &str = "text.txt";
pub(crate) const ENSURE_INLINED_DIR: &str = "temp_inline";

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

    pub fn remove_blob_stored_file(
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

    pub fn remove_blob_stored_text(&self, key_path: &Path) -> Result<(), FileStorageError> {
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

    pub fn remove_all(&self, key_path: &Path) -> Result<(), FileStorageError> {
        let dir_path = self.base_path.join(key_path);
        if dir_path.exists() {
            std::fs::remove_dir_all(dir_path.as_path())?;
        }

        Ok(())
    }

    /// Renames a key's blob directory.
    ///
    /// If the old directory doesn't exist (no blobs stored), this is a no-op.
    pub fn rename(&self, old_key_path: &Path, new_key_path: &Path) -> Result<(), FileStorageError> {
        let old_dir = self.base_path.join(old_key_path);
        let new_dir = self.base_path.join(new_key_path);

        if !old_dir.exists() {
            return Ok(()); // No blobs to move
        }

        if new_dir.exists() {
            std::fs::remove_dir_all(&new_dir)?;
        }
        std::fs::rename(old_dir, new_dir)?;

        Ok(())
    }

    /// Ensures that the file represented by `file` exists on disk, returning its path.
    ///
    /// If the file is inlined, it will be written to a temporary location under
    /// [ENSURE_INLINED_DIR] within the `key_path` directory.
    pub fn ensure_file_path(
        &self,
        key_path: &Path,
        file: &FileData,
    ) -> Result<PathBuf, FileStorageError> {
        let inline_dir = self.base_path.join(ENSURE_INLINED_DIR).join(key_path);
        match file {
            FileData::Inlined(InlineFileData { file_name, data }) => {
                let hash = <<Value as ValueVariant>::Hasher as FileHasher>::new()
                    .update(data)
                    .finalize();
                let file_dir_path = inline_dir.join(hash.to_string());
                std::fs::create_dir_all(file_dir_path.as_path())?;
                let file_path = file_dir_path.join(file_name);
                std::fs::write(file_path.as_path(), data)?;

                Ok(file_path)
            }
            FileData::BlobStored(BlobStoredFileData { file_name, hash }) => {
                let file_path = self
                    .base_path
                    .join(key_path)
                    .join(hash.to_string())
                    .join(file_name);

                Ok(file_path)
            }
        }
    }

    pub fn ensure_text_path(
        &self,
        key_path: &Path,
        text: &TextData,
    ) -> Result<PathBuf, FileStorageError> {
        match text {
            TextData::Inlined(data) => {
                let file_dir_path = self.base_path.join(ENSURE_INLINED_DIR).join(key_path);
                std::fs::create_dir_all(file_dir_path.as_path())?;
                let file_path = file_dir_path.join(TEXT_FILE_NAME);
                std::fs::write(file_path.as_path(), data.as_bytes())?;

                Ok(file_path)
            }
            TextData::BlobStored => {
                let text_path = self.base_path.join(key_path).join(TEXT_FILE_NAME);

                Ok(text_path)
            }
        }
    }

    fn cleanup_ensure_cache(&self, keep_key_path: Option<&Path>) -> Result<(), FileStorageError> {
        let ensure_dir = self.base_path.join(ENSURE_INLINED_DIR);
        if !ensure_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(ensure_dir.as_path())? {
            let entry = entry?;
            let path = entry.path();
            if let Some(keep_path) = keep_key_path
                && path.ends_with(keep_path)
            {
                continue;
            }
            if path.is_dir() {
                std::fs::remove_dir_all(path.as_path())?;
            } else {
                std::fs::remove_file(path.as_path())?;
            }
        }

        Ok(())
    }

    /// Lists all blob directories in the storage.
    ///
    /// Returns the directory names (which are blake3 hashes of keys).
    /// This is used for orphan blob detection during garbage collection.
    pub fn list_blob_dirs(&self) -> Result<Vec<PathBuf>, FileStorageError> {
        if !self.base_path.exists() {
            return Ok(Vec::new());
        }

        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            // Skip the temp_inline directory
            if path.file_name() == Some(std::ffi::OsStr::new(ENSURE_INLINED_DIR)) {
                continue;
            }
            if path.is_dir() {
                // Return just the directory name (the hash)
                if let Some(name) = path.file_name() {
                    dirs.push(PathBuf::from(name));
                }
            }
        }

        Ok(dirs)
    }
}

#[cfg(test)]
mod tests;
