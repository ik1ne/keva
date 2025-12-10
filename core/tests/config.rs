//! Tests for configuration

mod common;

use keva_core::{Config, Key};
use keva_core::config::{DeleteStyle, TtlConfig};
use keva_core::storage::DeleteOptions;
use tempfile::TempDir;

/// Verify `rm` follows the configured default (Soft vs Immediate) when no specific flag is passed.
#[test]
fn test_rm_honors_default_soft_delete() {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::new(temp_dir.path().to_path_buf())
        .with_delete_style(DeleteStyle::Soft);
    let store = keva_core::Store::open(config).unwrap();

    let key = Key::new("test-key").unwrap();
    store.set(&key, "Value").unwrap();

    // Delete without explicit flag - should use config default (Soft)
    store.rm(&key, DeleteOptions::default()).unwrap();

    // Should be in trash, not deleted
    let entry = store.get(&key, true).unwrap();
    assert!(entry.is_some());
    assert!(entry.unwrap().is_trash());
}

/// Verify `rm` follows the configured default (Immediate) when set.
#[test]
fn test_rm_honors_default_immediate_delete() {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::new(temp_dir.path().to_path_buf())
        .with_delete_style(DeleteStyle::Immediate);
    let store = keva_core::Store::open(config).unwrap();

    let key = Key::new("test-key").unwrap();
    store.set(&key, "Value").unwrap();

    // Delete without explicit flag - should use config default (Immediate)
    store.rm(&key, DeleteOptions::default()).unwrap();

    // Should be completely gone
    let entry = store.get(&key, true).unwrap();
    assert!(entry.is_none());
}

/// Verify importing a file follows the configured default (Embed).
#[test]
fn test_import_honors_default_embed() {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::new(temp_dir.path().to_path_buf());
    let store = keva_core::Store::open(config).unwrap();

    // Create a test file
    let file_path = temp_dir.path().join("test_file.txt");
    std::fs::write(&file_path, b"File content").unwrap();

    let key = Key::new("imported").unwrap();
    store.import_file(&key, &file_path, keva_core::RichFormat::Binary {
        mime_type: "text/plain".to_string(),
    }).unwrap();

    // Should be stored as embedded data, not a link
    let (rich_data, data) = store.get_rich_data(&key).unwrap().unwrap();
    assert!(!matches!(rich_data.storage, keva_core::model::RichStorage::Link { .. }));
    assert_eq!(data, b"File content");
}

/// Verify the core logic exposes the size of a file before import.
#[test]
fn test_large_file_size_check() {
    let temp_dir = TempDir::new().unwrap();

    // Create store with low threshold
    let config = Config::new(temp_dir.path().to_path_buf())
        .with_large_file_threshold(100); // 100 bytes
    let store = keva_core::Store::open(config).unwrap();

    // Create a small file
    let small_file = temp_dir.path().join("small.txt");
    std::fs::write(&small_file, b"small").unwrap();

    let (size, exceeds) = store.check_file_size(&small_file).unwrap();
    assert_eq!(size, 5);
    assert!(!exceeds);

    // Create a large file
    let large_file = temp_dir.path().join("large.txt");
    std::fs::write(&large_file, vec![0u8; 200]).unwrap();

    let (size, exceeds) = store.check_file_size(&large_file).unwrap();
    assert_eq!(size, 200);
    assert!(exceeds);
}

/// Verify TTL configuration
#[test]
fn test_ttl_configuration() {
    let temp_dir = TempDir::new().unwrap();

    let ttl = TtlConfig {
        active_to_trash_days: Some(7),
        trash_to_purge_days: 30,
    };

    let config = Config::new(temp_dir.path().to_path_buf())
        .with_ttl(ttl.clone());

    assert_eq!(config.ttl.active_to_trash_days, Some(7));
    assert_eq!(config.ttl.trash_to_purge_days, 30);
}

/// Verify blob threshold configuration
#[test]
fn test_blob_threshold_configuration() {
    let temp_dir = TempDir::new().unwrap();

    let config = Config::new(temp_dir.path().to_path_buf())
        .with_blob_threshold(50 * 1024); // 50KB

    assert_eq!(config.blob_threshold, 50 * 1024);
}
