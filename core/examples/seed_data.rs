//! Populates keva database with test data for debugging keva_windows.
//!
//! Run with: `cargo run -q --example seed_data -p keva_core`

use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, SavedConfig};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn main() {
    let base_path = get_default_data_path();
    println!("Using data path: {}", base_path.display());

    let config = Config {
        base_path: base_path.clone(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
            inline_threshold_bytes: 1024 * 1024,               // 1MB
        },
    };

    let mut keva = KevaCore::open(config).expect("Failed to open keva database");
    let now = SystemTime::now();

    println!("\n[Inlined Text Keys]");
    seed_inlined_text(&mut keva, now);

    println!("\n[Blob-Stored Text Keys]");
    seed_blob_text(&mut keva, now);

    println!("\n[File Keys]");
    seed_files(&mut keva, now, &base_path);

    println!("\n[Trashed Keys]");
    seed_trashed(&mut keva, now);

    // Summary
    let active = keva.active_keys().unwrap_or_default();
    let trashed = keva.trashed_keys().unwrap_or_default();
    println!(
        "\nDatabase now has {} active keys and {} trashed keys",
        active.len(),
        trashed.len()
    );
}

fn seed_inlined_text(keva: &mut KevaCore, now: SystemTime) {
    let keys = [
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

    for (key_str, content) in keys {
        let key = Key::try_from(key_str).expect("Invalid key");
        match keva.upsert_text(&key, content, now) {
            Ok(()) => println!("  Created: {}", key_str),
            Err(e) => println!("  Skipped {} ({})", key_str, e),
        }
    }
}

fn seed_blob_text(keva: &mut KevaCore, now: SystemTime) {
    // Create text larger than 1MB to trigger blob storage
    let large_content = "x".repeat(1024 * 1024 + 100);
    let key = Key::try_from("large-text").expect("Invalid key");
    match keva.upsert_text(&key, &large_content, now) {
        Ok(()) => println!("  Created: large-text ({}KB blob-stored)", large_content.len() / 1024),
        Err(e) => println!("  Skipped large-text ({})", e),
    }
}

fn seed_files(keva: &mut KevaCore, now: SystemTime, base_path: &PathBuf) {
    let temp_dir = base_path.join("_seed_temp");
    std::fs::create_dir_all(&temp_dir).ok();

    // Create test files
    let files = [
        ("document.txt", b"This is a text document." as &[u8]),
        ("notes.md", b"# Notes\n\n- Item 1\n- Item 2"),
        ("data.json", b"{\"key\": \"value\", \"count\": 42}"),
    ];

    let mut file_paths = Vec::new();
    for (name, content) in files {
        let path = temp_dir.join(name);
        if let Ok(mut f) = std::fs::File::create(&path) {
            f.write_all(content).ok();
            file_paths.push(path);
        }
    }

    // Add files to keva
    let key = Key::try_from("my-files").expect("Invalid key");
    match keva.add_files(&key, &file_paths, now) {
        Ok(()) => println!("  Created: my-files ({} files)", file_paths.len()),
        Err(e) => println!("  Skipped my-files ({})", e),
    }

    // Clean up temp files
    std::fs::remove_dir_all(&temp_dir).ok();
}

fn seed_trashed(keva: &mut KevaCore, now: SystemTime) {
    let keys = [
        ("old-draft", "This is an old draft that was deleted"),
        ("deprecated/config", "old_setting=true"),
    ];

    for (key_str, content) in keys {
        let key = Key::try_from(key_str).expect("Invalid key");
        if keva.upsert_text(&key, content, now).is_ok() {
            match keva.trash(&key, now) {
                Ok(()) => println!("  Trashed: {}", key_str),
                Err(e) => println!("  Failed to trash {} ({})", key_str, e),
            }
        }
    }
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
