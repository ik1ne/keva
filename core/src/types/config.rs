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
    pub inline_threshold_bytes: u64,
}

impl Config {
    pub fn db_path(&self) -> PathBuf {
        self.base_path.join("keva.redb")
    }

    pub fn blob_path(&self) -> PathBuf {
        self.base_path.join("blobs")
    }
}
