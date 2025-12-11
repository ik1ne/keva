use std::path::PathBuf;
use std::time::Duration;

pub struct Config {
    pub storage_path: PathBuf,
    pub blob_path: PathBuf,
    pub trash_ttl: Duration,
    pub purge_ttl: Duration,
    pub large_file_threshold_bytes: u64,
}
