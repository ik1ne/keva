//! Configuration for Keva

use chrono::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Delete behavior style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DeleteStyle {
    /// Deletions move items to Trash (default)
    #[default]
    Soft,
    /// Deletions permanently remove items immediately
    Immediate,
}

/// TTL (Time-To-Live) duration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    /// Duration before active items move to trash (None = never auto-trash)
    pub active_to_trash_days: Option<u32>,
    /// Duration before trashed items are purged
    pub trash_to_purge_days: u32,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            active_to_trash_days: None, // Never auto-trash by default
            trash_to_purge_days: 30,    // 30 days in trash before purge
        }
    }
}

impl TtlConfig {
    /// Get the duration until an active item should be trashed
    pub fn active_to_trash_duration(&self) -> Option<Duration> {
        self.active_to_trash_days.map(|d| Duration::days(d as i64))
    }

    /// Get the duration until a trashed item should be purged
    pub fn trash_to_purge_duration(&self) -> Duration {
        Duration::days(self.trash_to_purge_days as i64)
    }
}

/// Threshold for inline vs blob storage (in bytes)
pub const DEFAULT_BLOB_THRESHOLD: u64 = 100 * 1024; // 100KB

/// Threshold for large file warning (in bytes)
pub const DEFAULT_LARGE_FILE_THRESHOLD: u64 = 256 * 1024 * 1024; // 256MB

/// Main configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Base directory for Keva data
    pub data_dir: PathBuf,
    /// Delete behavior style
    pub delete_style: DeleteStyle,
    /// TTL configuration
    pub ttl: TtlConfig,
    /// Threshold for storing blobs externally (bytes)
    pub blob_threshold: u64,
    /// Threshold for large file warning (bytes)
    pub large_file_threshold: u64,
}

impl Config {
    /// Create a new config with the given data directory
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            delete_style: DeleteStyle::default(),
            ttl: TtlConfig::default(),
            blob_threshold: DEFAULT_BLOB_THRESHOLD,
            large_file_threshold: DEFAULT_LARGE_FILE_THRESHOLD,
        }
    }

    /// Create a config using the default data directory (~/.keva)
    pub fn default_location() -> crate::Result<Self> {
        let data_dir = dirs::home_dir()
            .ok_or_else(|| crate::Error::Config("Could not determine home directory".to_string()))?
            .join(".keva");
        Ok(Self::new(data_dir))
    }

    /// Get the path to the redb database file
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("keva.redb")
    }

    /// Get the path to the blobs directory
    pub fn blobs_dir(&self) -> PathBuf {
        self.data_dir.join("blobs")
    }

    /// Get the path to the search index directory
    pub fn index_dir(&self) -> PathBuf {
        self.data_dir.join("index")
    }

    /// Get the path to the config file
    pub fn config_file_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    /// Load configuration from the config file, or create default
    pub fn load_or_default() -> crate::Result<Self> {
        let default = Self::default_location()?;
        let config_path = default.config_file_path();

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(default)
        }
    }

    /// Save configuration to the config file
    pub fn save(&self) -> crate::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(self.config_file_path(), contents)?;
        Ok(())
    }

    /// Set the delete style
    pub fn with_delete_style(mut self, style: DeleteStyle) -> Self {
        self.delete_style = style;
        self
    }

    /// Set the TTL configuration
    pub fn with_ttl(mut self, ttl: TtlConfig) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set the blob threshold
    pub fn with_blob_threshold(mut self, threshold: u64) -> Self {
        self.blob_threshold = threshold;
        self
    }

    /// Set the large file threshold
    pub fn with_large_file_threshold(mut self, threshold: u64) -> Self {
        self.large_file_threshold = threshold;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default_location().unwrap_or_else(|_| {
            Self::new(PathBuf::from(".keva"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_config_defaults() {
        let ttl = TtlConfig::default();
        assert!(ttl.active_to_trash_days.is_none());
        assert_eq!(ttl.trash_to_purge_days, 30);
    }

    #[test]
    fn test_config_paths() {
        let config = Config::new(PathBuf::from("/test/.keva"));
        assert_eq!(config.db_path(), PathBuf::from("/test/.keva/keva.redb"));
        assert_eq!(config.blobs_dir(), PathBuf::from("/test/.keva/blobs"));
        assert_eq!(config.index_dir(), PathBuf::from("/test/.keva/index"));
    }
}
