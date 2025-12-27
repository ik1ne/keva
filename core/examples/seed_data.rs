//! Populates keva database with test data for debugging keva_windows.
//!
//! Run with: `cargo run -q --example seed_data -p keva_core`

use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, SavedConfig};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn main() {
    let base_path = get_default_data_path();
    println!("Using data path: {}", base_path.display());

    let config = Config {
        base_path,
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
            inline_threshold_bytes: 1024 * 1024,               // 1MB
        },
    };

    let mut keva = KevaCore::open(config).expect("Failed to open keva database");
    let now = SystemTime::now();

    // Seed active text keys
    let active_keys = [
        (
            "todo",
            "- Buy groceries\n- Fix bug in login\n- Review PR #42",
        ),
        (
            "notes/meeting-2024",
            "Discussed Q4 roadmap. Action items: ...",
        ),
        ("snippets/rust/result", "fn foo() -> Result<T, E> { ... }"),
        ("api-key", "sk-1234567890abcdef"),
        ("quick note", "Remember to call mom"),
        ("project/readme", "# My Project\n\nA brief description."),
        ("config/database", "host=localhost\nport=5432\nuser=admin"),
        (
            "ideas",
            "1. Build a CLI tool\n2. Learn WebGPU\n3. Write a blog post",
        ),
    ];

    for (key_str, content) in active_keys {
        let key = Key::try_from(key_str).expect("Invalid key");
        match keva.upsert_text(&key, content, now) {
            Ok(()) => println!("  Created: {}", key_str),
            Err(e) => println!("  Skipped {} ({})", key_str, e),
        }
    }

    // Seed trashed keys
    let trashed_keys = [
        ("old-draft", "This is an old draft that was deleted"),
        ("deprecated/config", "old_setting=true"),
    ];

    for (key_str, content) in trashed_keys {
        let key = Key::try_from(key_str).expect("Invalid key");
        // First create, then trash
        if keva.upsert_text(&key, content, now).is_ok() {
            match keva.trash(&key, now) {
                Ok(()) => println!("  Trashed: {}", key_str),
                Err(e) => println!("  Failed to trash {} ({})", key_str, e),
            }
        }
    }

    // Summary
    let active = keva.active_keys().unwrap_or_default();
    let trashed = keva.trashed_keys().unwrap_or_default();
    println!(
        "\nDatabase now has {} active keys and {} trashed keys",
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
