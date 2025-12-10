//! Storage layer for Keva
//!
//! This module provides the main Store interface that handles:
//! - Key-value storage using redb
//! - Blob storage for large binary data
//! - CRUD operations
//! - Lifecycle management and garbage collection

mod blob;
mod db;

use chrono::Utc;
use std::path::Path;

use crate::config::{Config, DeleteStyle};
use crate::error::Result;
use crate::model::{Entry, Key, Lifecycle, RichData, RichStorage, Value};
use crate::search::{SearchEngine, SearchResult, SearchScope};

pub use blob::BlobStore;
pub use db::Database;

/// Options for delete operations
#[derive(Debug, Clone, Default)]
pub struct DeleteOptions {
    /// Force permanent deletion (skip trash)
    pub permanent: bool,
    /// Force soft delete (move to trash)
    pub trash: bool,
    /// Delete recursively (include children)
    pub recursive: bool,
}

/// Options for move operations
#[derive(Debug, Clone, Default)]
pub struct MoveOptions {
    /// Overwrite existing key at destination
    pub force: bool,
}

/// The main storage interface for Keva
pub struct Store {
    config: Config,
    db: Database,
    blobs: BlobStore,
    search: SearchEngine,
}

impl Store {
    /// Open or create a store with the given configuration
    pub fn open(config: Config) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)?;

        let db = Database::open(&config)?;
        let blobs = BlobStore::new(config.blobs_dir());
        let search = SearchEngine::open(&config)?;

        Ok(Self {
            config,
            db,
            blobs,
            search,
        })
    }

    /// Open a store with default configuration
    pub fn open_default() -> Result<Self> {
        Self::open(Config::default_location()?)
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    // ==================== CRUD Operations ====================

    /// Get an entry by key
    ///
    /// Returns `None` if the key doesn't exist.
    /// Returns an error if the entry is in Trash or Purged state (unless include_trash is true).
    pub fn get(&self, key: &Key, include_trash: bool) -> Result<Option<Entry>> {
        let entry = self.db.get(key)?;

        match entry {
            Some(entry) => {
                let state = entry.lifecycle();
                match state {
                    Lifecycle::Active => Ok(Some(entry)),
                    Lifecycle::Trash => {
                        if include_trash {
                            Ok(Some(entry))
                        } else {
                            Err(crate::Error::InTrash(key.to_string()))
                        }
                    }
                    Lifecycle::Purged => Err(crate::Error::Purged(key.to_string())),
                }
            }
            None => Ok(None),
        }
    }

    /// Get the plain text value for a key
    pub fn get_text(&self, key: &Key) -> Result<Option<String>> {
        Ok(self.get(key, false)?.and_then(|e| e.value.plain_text))
    }

    /// Get the rich data for a key, reading from blob storage if needed
    pub fn get_rich_data(&self, key: &Key) -> Result<Option<(RichData, Vec<u8>)>> {
        let entry = self.get(key, false)?;

        match entry.and_then(|e| e.value.rich) {
            Some(rich) => {
                let data = match &rich.storage {
                    RichStorage::Inline(data) => data.clone(),
                    RichStorage::Blob { hash, .. } => self.blobs.read(hash)?,
                    RichStorage::Link { path } => std::fs::read(path)?,
                };
                Ok(Some((rich, data)))
            }
            None => Ok(None),
        }
    }

    /// Set a plain text value for a key
    pub fn set(&self, key: &Key, text: impl Into<String>) -> Result<()> {
        let text = text.into();
        let value = Value::plain_text(text.clone());

        self.set_value(key, value)?;

        // Update search index
        self.search.index_entry(key, Some(&text))?;

        Ok(())
    }

    /// Set a value with optional rich data
    pub fn set_value(&self, key: &Key, value: Value) -> Result<()> {
        let existing = self.db.get(key)?;

        let entry = match existing {
            Some(mut entry) => {
                entry.value = value;
                entry.timestamps.touch();
                // Clear trash/purge timestamps when updating
                entry.timestamps.trash_at = None;
                entry.timestamps.purge_at = None;
                entry
            }
            None => Entry::new(key.clone(), value),
        };

        self.db.put(&entry)?;

        // Update search index
        self.search.index_entry(key, entry.value.plain_text.as_deref())?;

        Ok(())
    }

    /// Store rich data for a key
    pub fn set_rich(
        &self,
        key: &Key,
        data: &[u8],
        format: crate::model::RichFormat,
        plain_text: Option<String>,
    ) -> Result<()> {
        let storage = if data.len() as u64 > self.config.blob_threshold {
            let hash = self.blobs.write(data)?;
            RichStorage::Blob {
                hash,
                size: data.len() as u64,
            }
        } else {
            RichStorage::Inline(data.to_vec())
        };

        let rich = RichData { format, storage };
        let value = Value {
            plain_text: plain_text.filter(|t| !t.trim().is_empty()),
            rich: Some(rich),
        };

        self.set_value(key, value)
    }

    /// Store a link to an external file
    pub fn set_link(
        &self,
        key: &Key,
        path: impl AsRef<Path>,
        format: crate::model::RichFormat,
        plain_text: Option<String>,
    ) -> Result<()> {
        let rich = RichData {
            format,
            storage: RichStorage::Link {
                path: path.as_ref().to_path_buf(),
            },
        };
        let value = Value {
            plain_text: plain_text.filter(|t| !t.trim().is_empty()),
            rich: Some(rich),
        };

        self.set_value(key, value)
    }

    /// Remove a key
    pub fn rm(&self, key: &Key, options: DeleteOptions) -> Result<()> {
        // Determine actual delete behavior
        let permanent = if options.permanent {
            true
        } else if options.trash {
            false
        } else {
            self.config.delete_style == DeleteStyle::Immediate
        };

        if options.recursive {
            // Delete children first
            let children = self.list_all_descendants(key)?;
            for child_key in children {
                self.delete_single(&child_key, permanent)?;
            }
        }

        self.delete_single(key, permanent)
    }

    /// Delete a single key
    fn delete_single(&self, key: &Key, permanent: bool) -> Result<()> {
        if permanent {
            // Permanent delete: remove from db and cleanup blobs
            if let Some(entry) = self.db.get(key)? {
                // Clean up blob if it exists
                if let Some(RichData {
                    storage: RichStorage::Blob { hash, .. },
                    ..
                }) = &entry.value.rich
                {
                    // Check if any other entry references this blob
                    if !self.blob_has_other_references(hash, key)? {
                        self.blobs.delete(hash)?;
                    }
                }
                self.db.delete(key)?;
                self.search.remove_entry(key)?;
            }
        } else {
            // Soft delete: mark as trash
            if let Some(mut entry) = self.db.get(key)? {
                let now = Utc::now();
                entry.timestamps.trash_at = Some(now);
                entry.timestamps.purge_at =
                    Some(now + self.config.ttl.trash_to_purge_duration());
                self.db.put(&entry)?;
                // Don't remove from search index - it will be filtered by lifecycle state
            }
        }
        Ok(())
    }

    /// Check if a blob is referenced by any entry other than the given key
    fn blob_has_other_references(&self, hash: &str, exclude_key: &Key) -> Result<bool> {
        let all_entries = self.db.list_all()?;
        for entry in all_entries {
            if &entry.key == exclude_key {
                continue;
            }
            if let Some(RichData {
                storage: RichStorage::Blob { hash: h, .. },
                ..
            }) = &entry.value.rich
            {
                if h == hash {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Move/rename a key
    pub fn mv(&self, from: &Key, to: &Key, options: MoveOptions) -> Result<()> {
        // Check if destination exists
        if self.db.get(to)?.is_some() {
            if !options.force {
                return Err(crate::Error::KeyExists(to.to_string()));
            }
            // Delete destination if force is specified
            self.rm(to, DeleteOptions { permanent: true, ..Default::default() })?;
        }

        // Get the source entry
        let entry = self.db.get(from)?.ok_or_else(|| crate::Error::KeyNotFound(from.to_string()))?;

        // Create new entry with the new key
        let new_entry = Entry {
            key: to.clone(),
            value: entry.value,
            timestamps: entry.timestamps,
        };

        // Write new entry and delete old one
        self.db.put(&new_entry)?;
        self.db.delete(from)?;

        // Update search index
        self.search.remove_entry(from)?;
        self.search.index_entry(to, new_entry.value.plain_text.as_deref())?;

        Ok(())
    }

    /// List direct children of a key
    pub fn ls(&self, key: &Key, include_trash: bool) -> Result<Vec<Key>> {
        let all = self.db.list_all()?;
        let children: Vec<Key> = all
            .into_iter()
            .filter(|entry| {
                entry.key.is_child_of(key)
                    && (include_trash || entry.lifecycle() == Lifecycle::Active)
            })
            .map(|entry| entry.key)
            .collect();
        Ok(children)
    }

    /// List all keys at the root level
    pub fn ls_root(&self, include_trash: bool) -> Result<Vec<Key>> {
        let all = self.db.list_all()?;
        let roots: Vec<Key> = all
            .into_iter()
            .filter(|entry| {
                entry.key.parent().is_none()
                    && (include_trash || entry.lifecycle() == Lifecycle::Active)
            })
            .map(|entry| entry.key)
            .collect();
        Ok(roots)
    }

    /// List all descendants of a key
    fn list_all_descendants(&self, key: &Key) -> Result<Vec<Key>> {
        let all = self.db.list_all()?;
        let descendants: Vec<Key> = all
            .into_iter()
            .filter(|entry| entry.key.is_descendant_of(key))
            .map(|entry| entry.key)
            .collect();
        Ok(descendants)
    }

    // ==================== Search Operations ====================

    /// Search for keys matching a query
    pub fn search(&mut self, query: &str, scope: SearchScope, include_trash: bool) -> Result<Vec<SearchResult>> {
        self.search.search(query, scope, include_trash, &self.db)
    }

    // ==================== Lifecycle & GC Operations ====================

    /// Run garbage collection
    ///
    /// This will:
    /// 1. Permanently remove entries that have exceeded purge TTL
    /// 2. Clean up orphaned blobs
    pub fn gc(&self) -> Result<GcStats> {
        let mut stats = GcStats::default();
        let now = Utc::now();

        // Phase 1: Find and remove purged entries
        let all_entries = self.db.list_all()?;
        let mut active_blob_hashes: Vec<String> = Vec::new();

        for entry in all_entries {
            if let Some(purge_at) = entry.timestamps.purge_at {
                if now >= purge_at {
                    // Entry should be purged
                    self.db.delete(&entry.key)?;
                    self.search.remove_entry(&entry.key)?;
                    stats.entries_purged += 1;
                } else {
                    // Entry is still active, track its blob
                    if let Some(RichData {
                        storage: RichStorage::Blob { hash, .. },
                        ..
                    }) = &entry.value.rich
                    {
                        active_blob_hashes.push(hash.clone());
                    }
                }
            } else {
                // Entry has no purge timestamp, track its blob
                if let Some(RichData {
                    storage: RichStorage::Blob { hash, .. },
                    ..
                }) = &entry.value.rich
                {
                    active_blob_hashes.push(hash.clone());
                }
            }
        }

        // Phase 2: Clean up orphaned blobs
        let orphaned = self.blobs.find_orphaned(&active_blob_hashes)?;
        for hash in orphaned {
            if let Ok(size) = self.blobs.size(&hash) {
                stats.bytes_reclaimed += size;
            }
            self.blobs.delete(&hash)?;
            stats.blobs_removed += 1;
        }

        Ok(stats)
    }

    /// Restore an item from trash
    pub fn restore(&self, key: &Key) -> Result<()> {
        let mut entry = self.db.get(key)?.ok_or_else(|| crate::Error::KeyNotFound(key.to_string()))?;

        if entry.lifecycle() != Lifecycle::Trash {
            return Err(crate::Error::Config(format!(
                "Key {} is not in trash",
                key
            )));
        }

        entry.timestamps.trash_at = None;
        entry.timestamps.purge_at = None;
        entry.timestamps.touch();

        self.db.put(&entry)?;
        Ok(())
    }

    // ==================== File Import ====================

    /// Check file size before import (for large file warning)
    pub fn check_file_size(&self, path: impl AsRef<Path>) -> Result<(u64, bool)> {
        let metadata = std::fs::metadata(path.as_ref())?;
        let size = metadata.len();
        let exceeds_threshold = size > self.config.large_file_threshold;
        Ok((size, exceeds_threshold))
    }

    /// Import a file as embedded data
    pub fn import_file(
        &self,
        key: &Key,
        path: impl AsRef<Path>,
        format: crate::model::RichFormat,
    ) -> Result<()> {
        let data = std::fs::read(path.as_ref())?;
        self.set_rich(key, &data, format, None)
    }

    // ==================== Utility ====================

    /// Check if a key exists
    pub fn exists(&self, key: &Key) -> Result<bool> {
        Ok(self.db.get(key)?.is_some())
    }

    /// Get all keys (for debugging/testing)
    pub fn all_keys(&self, include_trash: bool) -> Result<Vec<Key>> {
        let all = self.db.list_all()?;
        let keys: Vec<Key> = all
            .into_iter()
            .filter(|entry| include_trash || entry.lifecycle() == Lifecycle::Active)
            .map(|entry| entry.key)
            .collect();
        Ok(keys)
    }
}

/// Statistics from garbage collection
#[derive(Debug, Default)]
pub struct GcStats {
    /// Number of entries permanently removed
    pub entries_purged: usize,
    /// Number of blob files removed
    pub blobs_removed: usize,
    /// Total bytes reclaimed from blob storage
    pub bytes_reclaimed: u64,
}
