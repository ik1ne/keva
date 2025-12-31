//! File storage for content, attachments, and thumbnails.
//!
//! Storage structure:
//! - content/{key_hash}.md - Markdown content
//! - blobs/{key_hash}/{filename} - Attachments
//! - thumbnails/{key_hash}/{filename}.thumb - Generated thumbnails

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

        #[error("Image error: {0}")]
        Image(#[from] image::ImageError),

        #[error("Resize error: {0}")]
        Resize(#[from] fast_image_resize::ResizeError),

        #[error("Unsupported image format")]
        UnsupportedFormat,
    }
}

use error::FileStorageError;

/// Manages file storage for content, attachments, and thumbnails.
pub struct FileStorage {
    pub content_path: PathBuf,
    pub blobs_path: PathBuf,
    pub thumbnails_path: PathBuf,
}

/// Removes a directory if it exists and is empty.
fn remove_dir_if_empty(path: &Path) -> Result<(), FileStorageError> {
    if path.exists() && path.read_dir()?.next().is_none() {
        std::fs::remove_dir(path)?;
    }
    Ok(())
}

/// Content file operations.
impl FileStorage {
    /// Creates an empty content file for a key.
    pub fn create_content(&self, key_hash: &Path) -> Result<(), FileStorageError> {
        let content_file = self.content_path.join(key_hash).with_extension("md");
        if let Some(parent) = content_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(&content_file)?;
        Ok(())
    }

    /// Returns the path to a key's content file.
    pub fn content_file_path(&self, key_hash: &Path) -> PathBuf {
        self.content_path.join(key_hash).with_extension("md")
    }

    /// Removes a key's content file.
    pub fn remove_content(&self, key_hash: &Path) -> Result<(), FileStorageError> {
        let content_file = self.content_path.join(key_hash).with_extension("md");
        if content_file.exists() {
            std::fs::remove_file(&content_file)?;
        }
        Ok(())
    }
}

/// Attachment file operations.
impl FileStorage {
    /// Copies a file to the attachments directory.
    ///
    /// Returns the size of the file in bytes.
    pub fn add_attachment(
        &self,
        key_hash: &Path,
        source: &Path,
        filename: &str,
    ) -> Result<u64, FileStorageError> {
        let metadata = std::fs::metadata(source)?;
        if metadata.is_dir() {
            return Err(FileStorageError::IsDirectory);
        }

        let dest_dir = self.blobs_path.join(key_hash);
        std::fs::create_dir_all(&dest_dir)?;

        let dest_path = dest_dir.join(filename);
        std::fs::copy(source, &dest_path)?;

        Ok(metadata.len())
    }

    /// Returns the path to a specific attachment.
    pub fn attachment_path(&self, key_hash: &Path, filename: &str) -> PathBuf {
        self.blobs_path.join(key_hash).join(filename)
    }

    /// Removes an attachment file.
    pub fn remove_attachment(
        &self,
        key_hash: &Path,
        filename: &str,
    ) -> Result<(), FileStorageError> {
        let file_path = self.blobs_path.join(key_hash).join(filename);
        if file_path.exists() {
            std::fs::remove_file(&file_path)?;
        }

        // Clean up empty key directory
        let key_dir = self.blobs_path.join(key_hash);
        remove_dir_if_empty(&key_dir)?;

        Ok(())
    }

    /// Renames an attachment file.
    pub fn rename_attachment(
        &self,
        key_hash: &Path,
        old_filename: &str,
        new_filename: &str,
    ) -> Result<(), FileStorageError> {
        let old_path = self.blobs_path.join(key_hash).join(old_filename);
        let new_path = self.blobs_path.join(key_hash).join(new_filename);

        if old_path.exists() {
            std::fs::rename(old_path, new_path)?;
        }
        Ok(())
    }

    /// Removes all attachments for a key.
    pub fn remove_all_attachments(&self, key_hash: &Path) -> Result<(), FileStorageError> {
        let dir_path = self.blobs_path.join(key_hash);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)?;
        }
        Ok(())
    }
}

/// Thumbnail operations.
impl FileStorage {
    /// Maximum thumbnail dimension in pixels.
    const THUMB_SIZE: u32 = 200;

    /// Supported image extensions for thumbnail generation.
    const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

    /// Checks if a filename has a supported image extension.
    pub fn is_supported_image(filename: &str) -> bool {
        let ext = filename
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        Self::SUPPORTED_EXTENSIONS.contains(&ext.as_str())
    }

    /// Returns the path to a thumbnail file.
    pub fn thumbnail_path(&self, key_hash: &Path, filename: &str) -> PathBuf {
        self.thumbnails_path
            .join(key_hash)
            .join(format!("{}.thumb", filename))
    }

    /// Generates a thumbnail for an attachment.
    ///
    /// Returns `Err(UnsupportedFormat)` if the file is not a supported image.
    pub fn generate_thumbnail(
        &self,
        key_hash: &Path,
        filename: &str,
    ) -> Result<(), FileStorageError> {
        if !Self::is_supported_image(filename) {
            return Err(FileStorageError::UnsupportedFormat);
        }

        let source_path = self.attachment_path(key_hash, filename);
        let thumb_path = self.thumbnail_path(key_hash, filename);

        // Load source image
        let src_image = image::open(&source_path)?;
        let (src_width, src_height) = (src_image.width(), src_image.height());

        // Calculate target dimensions preserving aspect ratio
        let scale = (Self::THUMB_SIZE as f32 / src_width.max(src_height) as f32).min(1.0);
        let dst_width = ((src_width as f32 * scale) as u32).max(1);
        let dst_height = ((src_height as f32 * scale) as u32).max(1);

        // Create destination image
        let mut dst_image = image::DynamicImage::new(dst_width, dst_height, src_image.color());

        // Resize using fast_image_resize
        let mut resizer = fast_image_resize::Resizer::new();
        resizer.resize(
            &src_image,
            &mut dst_image,
            Some(&fast_image_resize::ResizeOptions::new().resize_alg(
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
            )),
        )?;

        // Ensure thumbnail directory exists
        if let Some(parent) = thumb_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Save as PNG
        dst_image.save_with_format(&thumb_path, image::ImageFormat::Png)?;

        Ok(())
    }

    /// Removes a thumbnail file.
    pub fn remove_thumbnail(
        &self,
        key_hash: &Path,
        filename: &str,
    ) -> Result<(), FileStorageError> {
        let thumb_path = self.thumbnail_path(key_hash, filename);
        if thumb_path.exists() {
            std::fs::remove_file(&thumb_path)?;
        }

        // Clean up empty key directory
        let key_dir = self.thumbnails_path.join(key_hash);
        remove_dir_if_empty(&key_dir)?;

        Ok(())
    }

    /// Removes all thumbnails for a key.
    pub fn remove_all_thumbnails(&self, key_hash: &Path) -> Result<(), FileStorageError> {
        let dir_path = self.thumbnails_path.join(key_hash);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)?;
        }
        Ok(())
    }
}

/// Cleanup operations.
impl FileStorage {
    /// Removes all files for a key (content, attachments, thumbnails).
    pub fn remove_all(&self, key_hash: &Path) -> Result<(), FileStorageError> {
        self.remove_content(key_hash)?;
        self.remove_all_attachments(key_hash)?;
        self.remove_all_thumbnails(key_hash)?;
        Ok(())
    }

    /// Renames all files from one key to another (content, attachments, thumbnails).
    pub fn rename_all(
        &self,
        old_key_hash: &Path,
        new_key_hash: &Path,
    ) -> Result<(), FileStorageError> {
        // Rename content file
        let old_content = self.content_path.join(old_key_hash).with_extension("md");
        let new_content = self.content_path.join(new_key_hash).with_extension("md");
        if old_content.exists() {
            if new_content.exists() {
                std::fs::remove_file(&new_content)?;
            }
            std::fs::rename(old_content, new_content)?;
        }

        // Rename attachments directory
        let old_blobs = self.blobs_path.join(old_key_hash);
        let new_blobs = self.blobs_path.join(new_key_hash);
        if old_blobs.exists() {
            if new_blobs.exists() {
                std::fs::remove_dir_all(&new_blobs)?;
            }
            std::fs::rename(old_blobs, new_blobs)?;
        }

        // Rename thumbnails directory
        let old_thumbs = self.thumbnails_path.join(old_key_hash);
        let new_thumbs = self.thumbnails_path.join(new_key_hash);
        if old_thumbs.exists() {
            if new_thumbs.exists() {
                std::fs::remove_dir_all(&new_thumbs)?;
            }
            std::fs::rename(old_thumbs, new_thumbs)?;
        }

        Ok(())
    }

    /// Lists all key hashes that have blob directories.
    ///
    /// Used for orphan blob detection during garbage collection.
    pub fn list_blob_key_hashes(&self) -> Result<Vec<PathBuf>, FileStorageError> {
        if !self.blobs_path.exists() {
            return Ok(Vec::new());
        }

        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&self.blobs_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name()
            {
                dirs.push(PathBuf::from(name));
            }
        }

        Ok(dirs)
    }

    /// Lists all key hashes that have content files.
    pub fn list_content_key_hashes(&self) -> Result<Vec<PathBuf>, FileStorageError> {
        if !self.content_path.exists() {
            return Ok(Vec::new());
        }

        let mut hashes = Vec::new();
        for entry in std::fs::read_dir(&self.content_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path.extension().is_some_and(|e| e == "md")
                && let Some(stem) = path.file_stem()
            {
                hashes.push(PathBuf::from(stem));
            }
        }

        Ok(hashes)
    }
}

#[cfg(test)]
mod tests;
