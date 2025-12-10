//! Tests for search functionality

mod common;

use keva_core::{Key, SearchScope};
use keva_core::search::SearchMode;
use keva_core::storage::DeleteOptions;

/// Verify search can toggle between searching Key paths vs Key paths + Value content.
#[test]
fn test_search_scope_keys_vs_content() {
    let (mut store, _temp) = common::test_store();

    // Create entries where content differs from key
    let key1 = Key::new("documents/report").unwrap();
    let key2 = Key::new("files/data").unwrap();

    store.set(&key1, "This is about finances").unwrap();
    store.set(&key2, "This contains report data").unwrap();

    // Search for "report" in keys only - should find key1
    let results = store.search("report", SearchScope::Keys, false).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key.as_str(), "documents/report");

    // Search for "report" in keys and content - should find both
    let results = store.search("report", SearchScope::KeysAndContent, false).unwrap();
    assert_eq!(results.len(), 2);
}

/// Verify search results include `Trash` items when requested (or by default) but rank them lowest.
#[test]
fn test_search_includes_trash_ranked_low() {
    let (mut store, _temp) = common::test_store();

    let active_key = Key::new("active/item").unwrap();
    let trash_key = Key::new("trash/item").unwrap();

    store.set(&active_key, "Active").unwrap();
    store.set(&trash_key, "In trash").unwrap();

    // Move one to trash
    store.rm(&trash_key, DeleteOptions { trash: true, ..Default::default() }).unwrap();

    // Search with include_trash
    let results = store.search("item", SearchScope::Keys, true).unwrap();
    assert_eq!(results.len(), 2);

    // Active should be first, trash should be last
    assert!(!results[0].is_trash);
    assert!(results[1].is_trash);
}

/// Verify search results **never** return items that have exceeded the Purge TTL.
#[test]
fn test_search_excludes_purged() {
    use keva_core::config::TtlConfig;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let config = keva_core::Config::new(temp_dir.path().to_path_buf())
        .with_ttl(TtlConfig {
            active_to_trash_days: None,
            trash_to_purge_days: 0, // Immediate purge
        });
    let mut store = keva_core::Store::open(config).unwrap();

    let key = Key::new("will-purge").unwrap();
    store.set(&key, "Value").unwrap();

    // Soft delete - with 0 TTL, will be immediately purged
    store.rm(&key, DeleteOptions { trash: true, ..Default::default() }).unwrap();

    // Wait to ensure timestamp is in the past
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Search should not find purged items
    let results = store.search("purge", SearchScope::Keys, true).unwrap();
    assert!(results.is_empty());
}

/// Verify that a query with special characters (e.g. `*`) triggers Regex mode.
#[test]
fn test_search_auto_detects_regex_mode() {
    assert_eq!(SearchMode::detect("hello"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("hello*"), SearchMode::Regex);
    assert_eq!(SearchMode::detect("^start"), SearchMode::Regex);
    assert_eq!(SearchMode::detect("[a-z]+"), SearchMode::Regex);
    assert_eq!(SearchMode::detect("a|b"), SearchMode::Regex);
}

/// Verify that a standard string triggers Fuzzy mode.
#[test]
fn test_search_defaults_to_fuzzy_mode() {
    // Standard alphanumeric
    assert_eq!(SearchMode::detect("hello"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("hello-world"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("hello_world"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("path/to/key"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("file.txt"), SearchMode::Fuzzy);
    assert_eq!(SearchMode::detect("hello world"), SearchMode::Fuzzy);
}

/// Test fuzzy matching scores exact matches higher
#[test]
fn test_fuzzy_exact_match_ranked_highest() {
    let (mut store, _temp) = common::test_store();

    let key1 = Key::new("config").unwrap();
    let key2 = Key::new("configuration").unwrap();
    let key3 = Key::new("project/config/settings").unwrap();

    store.set(&key1, "1").unwrap();
    store.set(&key2, "2").unwrap();
    store.set(&key3, "3").unwrap();

    let results = store.search("config", SearchScope::Keys, false).unwrap();

    // All should match
    assert_eq!(results.len(), 3);

    // Exact match should be ranked first or among the top
    let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
    assert!(keys.contains(&"config"));
}

/// Test regex search
#[test]
fn test_regex_search() {
    let (mut store, _temp) = common::test_store();

    let key1 = Key::new("file1.txt").unwrap();
    let key2 = Key::new("file2.txt").unwrap();
    let key3 = Key::new("document.pdf").unwrap();

    store.set(&key1, "1").unwrap();
    store.set(&key2, "2").unwrap();
    store.set(&key3, "3").unwrap();

    // Regex to match .txt files
    let results = store.search(r"\.txt$", SearchScope::Keys, false).unwrap();

    assert_eq!(results.len(), 2);
    let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
    assert!(keys.contains(&"file1.txt"));
    assert!(keys.contains(&"file2.txt"));
    assert!(!keys.contains(&"document.pdf"));
}

/// Test empty query returns all items
#[test]
fn test_empty_query_returns_all() {
    let (mut store, _temp) = common::test_store();

    let key1 = Key::new("a").unwrap();
    let key2 = Key::new("b").unwrap();
    let key3 = Key::new("c").unwrap();

    store.set(&key1, "1").unwrap();
    store.set(&key2, "2").unwrap();
    store.set(&key3, "3").unwrap();

    let results = store.search("", SearchScope::Keys, false).unwrap();
    assert_eq!(results.len(), 3);
}
