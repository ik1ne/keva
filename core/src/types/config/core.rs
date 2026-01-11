use std::path::PathBuf;

/// Core configuration for KevaCore initialization.
#[derive(Clone)]
pub struct Config {
    pub base_path: PathBuf,
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
