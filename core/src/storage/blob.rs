//! Blob storage for large binary data
//!
//! Uses content-addressable storage with BLAKE3 hashing.

use std::fs;
use std::path::PathBuf;

use crate::error::Result;

/// Content-addressable blob storage
pub struct BlobStore {
    base_dir: PathBuf,
}

impl BlobStore {
    /// Create a new blob store at the given directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the path for a blob with the given hash
    fn blob_path(&self, hash: &str) -> PathBuf {
        // Use first 2 characters as subdirectory for better filesystem performance
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        self.base_dir.join(prefix).join(rest)
    }

    /// Write data to blob storage, returning the content hash
    pub fn write(&self, data: &[u8]) -> Result<String> {
        let hash = blake3::hash(data).to_hex().to_string();
        let path = self.blob_path(&hash);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically by writing to temp file and renaming
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, data)?;
        fs::rename(&temp_path, &path)?;

        Ok(hash)
    }

    /// Read data from blob storage
    pub fn read(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.blob_path(hash);
        if !path.exists() {
            return Err(crate::Error::BlobNotFound(hash.to_string()));
        }
        Ok(fs::read(path)?)
    }

    /// Check if a blob exists
    pub fn exists(&self, hash: &str) -> bool {
        self.blob_path(hash).exists()
    }

    /// Get the size of a blob
    pub fn size(&self, hash: &str) -> Result<u64> {
        let path = self.blob_path(hash);
        if !path.exists() {
            return Err(crate::Error::BlobNotFound(hash.to_string()));
        }
        Ok(fs::metadata(path)?.len())
    }

    /// Delete a blob
    pub fn delete(&self, hash: &str) -> Result<()> {
        let path = self.blob_path(hash);
        if path.exists() {
            fs::remove_file(&path)?;

            // Try to remove parent directory if empty
            if let Some(parent) = path.parent() {
                let _ = fs::remove_dir(parent); // Ignore error if not empty
            }
        }
        Ok(())
    }

    /// Find orphaned blobs (blobs not in the active set)
    pub fn find_orphaned(&self, active_hashes: &[String]) -> Result<Vec<String>> {
        let mut orphaned = Vec::new();

        if !self.base_dir.exists() {
            return Ok(orphaned);
        }

        // Walk the blob directory
        for prefix_entry in fs::read_dir(&self.base_dir)? {
            let prefix_entry = prefix_entry?;
            if !prefix_entry.file_type()?.is_dir() {
                continue;
            }

            let prefix = prefix_entry.file_name().to_string_lossy().to_string();

            for blob_entry in fs::read_dir(prefix_entry.path())? {
                let blob_entry = blob_entry?;
                if !blob_entry.file_type()?.is_file() {
                    continue;
                }

                let rest = blob_entry.file_name().to_string_lossy().to_string();
                let hash = format!("{}{}", prefix, rest);

                if !active_hashes.contains(&hash) {
                    orphaned.push(hash);
                }
            }
        }

        Ok(orphaned)
    }

    /// Get total size of all blobs
    pub fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;

        if !self.base_dir.exists() {
            return Ok(0);
        }

        for prefix_entry in fs::read_dir(&self.base_dir)? {
            let prefix_entry = prefix_entry?;
            if !prefix_entry.file_type()?.is_dir() {
                continue;
            }

            for blob_entry in fs::read_dir(prefix_entry.path())? {
                let blob_entry = blob_entry?;
                if blob_entry.file_type()?.is_file() {
                    total += blob_entry.metadata()?.len();
                }
            }
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf());

        let data = b"Hello, World!";
        let hash = store.write(data).unwrap();

        assert!(store.exists(&hash));
        assert_eq!(store.read(&hash).unwrap(), data);
    }

    #[test]
    fn test_content_addressable() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf());

        let data = b"Same content";
        let hash1 = store.write(data).unwrap();
        let hash2 = store.write(data).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf());

        let data = b"To be deleted";
        let hash = store.write(data).unwrap();

        assert!(store.exists(&hash));
        store.delete(&hash).unwrap();
        assert!(!store.exists(&hash));
    }

    #[test]
    fn test_find_orphaned() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf());

        let hash1 = store.write(b"data1").unwrap();
        let hash2 = store.write(b"data2").unwrap();
        let hash3 = store.write(b"data3").unwrap();

        // Only hash1 and hash2 are active
        let active = vec![hash1.clone(), hash2.clone()];
        let orphaned = store.find_orphaned(&active).unwrap();

        assert_eq!(orphaned.len(), 1);
        assert!(orphaned.contains(&hash3));
    }
}
