use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub base_path: PathBuf,
    pub saved: SavedConfig,
}

#[derive(Clone)]
pub struct SavedConfig {
    pub trash_ttl: Duration,
    pub purge_ttl: Duration,
}

impl Config {
    pub fn db_path(&self) -> PathBuf {
        self.base_path.join("keva.redb")
    }

    pub fn content_path(&self) -> PathBuf {
        self.base_path.join("content")
    }

    pub fn blobs_path(&self) -> PathBuf {
        self.base_path.join("blobs")
    }

    pub fn thumbnails_path(&self) -> PathBuf {
        self.base_path.join("thumbnails")
    }
}
