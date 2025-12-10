//! Common test utilities

use keva_core::{Config, Store};
use tempfile::TempDir;

/// Create a test store with a temporary directory
pub fn test_store() -> (Store, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::new(temp_dir.path().to_path_buf());
    let store = Store::open(config).unwrap();
    (store, temp_dir)
}

/// Create a test config with a temporary directory
pub fn test_config(temp_dir: &TempDir) -> Config {
    Config::new(temp_dir.path().to_path_buf())
}
