//! Creates 9000 keys for testing search and UI performance.
//!
//! Run with: `cargo run -q --example bulk_keys -p keva_core`

use keva_core::core::KevaCore;
use keva_core::types::{Config, Key};
use std::path::PathBuf;
use std::time::SystemTime;

const KEY_COUNT: usize = 9000;

fn main() {
    let base_path = get_default_data_path();
    println!("Using data path: {}", base_path.display());

    let config = Config {
        base_path: base_path.clone(),
    };

    let mut keva = KevaCore::open(config).expect("Failed to open keva database");
    let now = SystemTime::now();

    println!("Creating {} keys...", KEY_COUNT);

    let mut created = 0;
    let mut skipped = 0;

    for i in 0..KEY_COUNT {
        let key_str = format!("bulk/key-{:05}", i);
        let key = Key::try_from(key_str.as_str()).expect("Invalid key");

        if keva.get(&key).unwrap().is_none() {
            if keva.create(&key, now).is_ok() {
                let content = format!("Content for key {}\n\nThis is test data.", i);
                let content_path = keva.content_path(&key);
                std::fs::write(&content_path, content).ok();
                keva.touch(&key, now).ok();
                created += 1;
            }
        } else {
            skipped += 1;
        }

        if (i + 1) % 1000 == 0 {
            println!("  Progress: {}/{}", i + 1, KEY_COUNT);
        }
    }

    println!("\nCreated {} keys, skipped {} (already exist)", created, skipped);

    let active = keva.active_keys().unwrap_or_default();
    let trashed = keva.trashed_keys().unwrap_or_default();
    println!(
        "Database now has {} active keys and {} trashed keys",
        active.len(),
        trashed.len()
    );
}

fn get_default_data_path() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .expect("LOCALAPPDATA not set")
            .join("keva")
    }
}
