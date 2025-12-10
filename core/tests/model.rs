//! Tests for the data model

mod common;

use keva_core::{Key, RichFormat};
use keva_core::model::RichStorage;

/// Verify a single key path (e.g., `project`) can hold a value.
#[test]
fn test_parallel_storage_node_and_children() {
    let (store, _temp) = common::test_store();

    // Create parent with a value
    let parent = Key::new("project").unwrap();
    store.set(&parent, "Parent value").unwrap();

    // Create child with a value
    let child = Key::new("project/config").unwrap();
    store.set(&child, "Child value").unwrap();

    // Both should exist independently
    assert_eq!(store.get_text(&parent).unwrap(), Some("Parent value".to_string()));
    assert_eq!(store.get_text(&child).unwrap(), Some("Child value".to_string()));

    // Listing children should work
    let children = store.ls(&parent, false).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].as_str(), "project/config");
}

/// Verify `get` returns only the value for the specific key, effectively ignoring children.
#[test]
fn test_node_retrieval_returns_value_only() {
    let (store, _temp) = common::test_store();

    let parent = Key::new("node").unwrap();
    let child = Key::new("node/child").unwrap();

    store.set(&parent, "Node value").unwrap();
    store.set(&child, "Child value").unwrap();

    // Getting parent should not include child
    let parent_value = store.get_text(&parent).unwrap();
    assert_eq!(parent_value, Some("Node value".to_string()));
}

/// Verify storing and retrieving UTF-8 string values.
#[test]
fn test_store_text_value() {
    let (store, _temp) = common::test_store();

    let key = Key::new("text/unicode").unwrap();
    let unicode_text = "Hello, 世界! 🌍 Привет мир! مرحبا بالعالم";

    store.set(&key, unicode_text).unwrap();

    let retrieved = store.get_text(&key).unwrap();
    assert_eq!(retrieved, Some(unicode_text.to_string()));
}

/// Verify storing and retrieving Markdown content (metadata indicating type).
#[test]
fn test_store_rich_text() {
    let (store, _temp) = common::test_store();

    let key = Key::new("docs/readme").unwrap();
    let html_content = b"<html><body><h1>Hello</h1></body></html>";

    store.set_rich(&key, html_content, RichFormat::Html, Some("Hello".to_string())).unwrap();

    // Should have plain text
    let text = store.get_text(&key).unwrap();
    assert_eq!(text, Some("Hello".to_string()));

    // Should have rich data
    let rich = store.get_rich_data(&key).unwrap();
    assert!(rich.is_some());
    let (rich_data, data) = rich.unwrap();
    assert!(matches!(rich_data.format, RichFormat::Html));
    assert_eq!(data, html_content);
}

/// Verify importing a file <1MB stores it inline (Redb) and retrieves it identically.
#[test]
fn test_store_small_embedded_file() {
    let (store, _temp) = common::test_store();

    let key = Key::new("files/small").unwrap();
    let small_data = vec![0u8; 1024]; // 1KB

    store.set_rich(&key, &small_data, RichFormat::Binary {
        mime_type: "application/octet-stream".to_string(),
    }, None).unwrap();

    let (rich_data, data) = store.get_rich_data(&key).unwrap().unwrap();

    // Should be stored inline (under threshold)
    assert!(matches!(rich_data.storage, RichStorage::Inline(_)));
    assert_eq!(data, small_data);
}

/// Verify importing a file >=1MB stores it in blob storage and retrieves it identically.
#[test]
fn test_store_large_embedded_file() {
    use tempfile::TempDir;

    // Create a fresh temp dir (not using common::test_store to avoid DB conflict)
    let temp_dir = TempDir::new().unwrap();

    // Create a config with lower threshold for testing
    let config = keva_core::Config::new(temp_dir.path().to_path_buf())
        .with_blob_threshold(1024); // 1KB threshold

    let store = keva_core::Store::open(config).unwrap();

    let key = Key::new("files/large").unwrap();
    let large_data = vec![0xABu8; 2048]; // 2KB (above 1KB threshold)

    store.set_rich(&key, &large_data, RichFormat::Binary {
        mime_type: "application/octet-stream".to_string(),
    }, None).unwrap();

    let (rich_data, data) = store.get_rich_data(&key).unwrap().unwrap();

    // Should be stored as blob
    assert!(matches!(rich_data.storage, RichStorage::Blob { .. }));
    assert_eq!(data, large_data);
}

/// Verify linking a file stores only the OS path and retrieves that path.
#[test]
fn test_store_linked_file() {
    let (store, temp_dir) = common::test_store();

    // Create a test file
    let file_path = temp_dir.path().join("linked_file.txt");
    std::fs::write(&file_path, b"Linked content").unwrap();

    let key = Key::new("links/file").unwrap();
    store.set_link(&key, &file_path, RichFormat::Binary {
        mime_type: "text/plain".to_string(),
    }, None).unwrap();

    let (rich_data, data) = store.get_rich_data(&key).unwrap().unwrap();

    // Should be stored as link
    assert!(matches!(rich_data.storage, RichStorage::Link { .. }));
    assert_eq!(data, b"Linked content");
}

/// Verify retrieved items correctly report their type.
#[test]
fn test_value_type_metadata() {
    let (store, _temp) = common::test_store();

    // Text value
    let text_key = Key::new("types/text").unwrap();
    store.set(&text_key, "Plain text").unwrap();

    let entry = store.get(&text_key, false).unwrap().unwrap();
    assert!(entry.value.has_plain_text());
    assert!(!entry.value.has_rich());

    // Rich value
    let rich_key = Key::new("types/rich").unwrap();
    store.set_rich(&rich_key, b"PNG data", RichFormat::Png, None).unwrap();

    let entry = store.get(&rich_key, false).unwrap().unwrap();
    assert!(!entry.value.has_plain_text());
    assert!(entry.value.has_rich());

    // Both
    let both_key = Key::new("types/both").unwrap();
    store.set_rich(&both_key, b"HTML", RichFormat::Html, Some("Text".to_string())).unwrap();

    let entry = store.get(&both_key, false).unwrap().unwrap();
    assert!(entry.value.has_plain_text());
    assert!(entry.value.has_rich());
}
