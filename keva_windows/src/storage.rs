//! Storage layer initialization and access.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use keva_core::core::KevaCore;
use keva_core::types::{Config, SavedConfig};
use once_cell::sync::OnceCell;

static KEVA: OnceCell<Mutex<KevaCore>> = OnceCell::new();

/// Initializes the storage layer. Must be called once at startup.
pub fn init() {
    let config = Config {
        base_path: get_data_path(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
            inline_threshold_bytes: 1024 * 1024,               // 1MB
        },
    };

    let keva = KevaCore::open(config).expect("Failed to open keva database");
    KEVA.set(Mutex::new(keva))
        .unwrap_or_else(|_| panic!("Storage already initialized"));

    eprintln!("[Storage] Initialized at {}", get_data_path().display());
}

/// Provides access to KevaCore. Panics if not initialized.
pub fn with_keva<T>(f: impl FnOnce(&mut KevaCore) -> T) -> T {
    let mutex = KEVA.get().expect("Storage not initialized");
    let mut guard = mutex.lock().unwrap();
    f(&mut guard)
}

fn get_data_path() -> PathBuf {
    std::env::var("KEVA_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA not set")
                .join("keva")
        })
}
