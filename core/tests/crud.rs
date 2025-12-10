//! Tests for CRUD operations

mod common;

use keva_core::{Key, Error};
use keva_core::storage::DeleteOptions;

/// Verify `get` returns `None` for non-existent keys.
#[test]
fn test_get_non_existent() {
    let (store, _temp) = common::test_store();

    let key = Key::new("nonexistent").unwrap();
    let result = store.get(&key, false).unwrap();
    assert!(result.is_none());
}

/// Verify `set` overwrites an existing value for a key without affecting its children or parent.
#[test]
fn test_set_isolation() {
    let (store, _temp) = common::test_store();

    let parent = Key::new("parent").unwrap();
    let key = Key::new("parent/key").unwrap();
    let child = Key::new("parent/key/child").unwrap();

    // Set up hierarchy
    store.set(&parent, "Parent").unwrap();
    store.set(&key, "Original").unwrap();
    store.set(&child, "Child").unwrap();

    // Update the middle key
    store.set(&key, "Updated").unwrap();

    // Parent and child should be unchanged
    assert_eq!(store.get_text(&parent).unwrap(), Some("Parent".to_string()));
    assert_eq!(store.get_text(&key).unwrap(), Some("Updated".to_string()));
    assert_eq!(store.get_text(&child).unwrap(), Some("Child".to_string()));
}

/// Verify `ls` returns direct children of a given key.
#[test]
fn test_list_children() {
    let (store, _temp) = common::test_store();

    let parent = Key::new("parent").unwrap();
    let child1 = Key::new("parent/child1").unwrap();
    let child2 = Key::new("parent/child2").unwrap();
    let grandchild = Key::new("parent/child1/grandchild").unwrap();

    store.set(&parent, "Parent").unwrap();
    store.set(&child1, "Child 1").unwrap();
    store.set(&child2, "Child 2").unwrap();
    store.set(&grandchild, "Grandchild").unwrap();

    // List children of parent - should only get direct children
    let children = store.ls(&parent, false).unwrap();
    assert_eq!(children.len(), 2);

    let child_keys: Vec<&str> = children.iter().map(|k| k.as_str()).collect();
    assert!(child_keys.contains(&"parent/child1"));
    assert!(child_keys.contains(&"parent/child2"));
    assert!(!child_keys.contains(&"parent/child1/grandchild"));
}

/// Verify removing an item (default config) marks it as `Trash` but keeps the data.
#[test]
fn test_soft_delete() {
    let (store, _temp) = common::test_store();

    let key = Key::new("to-trash").unwrap();
    store.set(&key, "Some value").unwrap();

    // Soft delete (default)
    store.rm(&key, DeleteOptions { trash: true, ..Default::default() }).unwrap();

    // Should not be accessible without include_trash
    let result = store.get(&key, false);
    assert!(matches!(result, Err(Error::InTrash(_))));

    // Should be accessible with include_trash
    let entry = store.get(&key, true).unwrap().unwrap();
    assert!(entry.is_trash());
    assert_eq!(entry.value.plain_text, Some("Some value".to_string()));
}

/// Verify removing an item with `permanent: true` removes it entirely.
#[test]
fn test_permanent_delete() {
    let (store, _temp) = common::test_store();

    let key = Key::new("to-delete").unwrap();
    store.set(&key, "Some value").unwrap();

    // Permanent delete
    store.rm(&key, DeleteOptions { permanent: true, ..Default::default() }).unwrap();

    // Should not exist at all
    let result = store.get(&key, true).unwrap();
    assert!(result.is_none());
}

/// Verify that deleting a parent key does NOT remove its children.
#[test]
fn test_delete_non_recursive() {
    let (store, _temp) = common::test_store();

    let parent = Key::new("parent").unwrap();
    let child = Key::new("parent/child").unwrap();

    store.set(&parent, "Parent").unwrap();
    store.set(&child, "Child").unwrap();

    // Delete parent only (non-recursive)
    store.rm(&parent, DeleteOptions { permanent: true, ..Default::default() }).unwrap();

    // Parent should be gone
    assert!(store.get(&parent, true).unwrap().is_none());

    // Child should still exist
    assert!(store.get(&child, false).unwrap().is_some());
}

/// Verify recursive delete removes children
#[test]
fn test_delete_recursive() {
    let (store, _temp) = common::test_store();

    let parent = Key::new("parent").unwrap();
    let child = Key::new("parent/child").unwrap();
    let grandchild = Key::new("parent/child/grandchild").unwrap();

    store.set(&parent, "Parent").unwrap();
    store.set(&child, "Child").unwrap();
    store.set(&grandchild, "Grandchild").unwrap();

    // Delete recursively
    store.rm(&parent, DeleteOptions {
        permanent: true,
        recursive: true,
        ..Default::default()
    }).unwrap();

    // All should be gone
    assert!(store.get(&parent, true).unwrap().is_none());
    assert!(store.get(&child, true).unwrap().is_none());
    assert!(store.get(&grandchild, true).unwrap().is_none());
}

/// Verify move operation
#[test]
fn test_move() {
    let (store, _temp) = common::test_store();

    let from = Key::new("old/path").unwrap();
    let to = Key::new("new/path").unwrap();

    store.set(&from, "Moving value").unwrap();

    // Move
    store.mv(&from, &to, Default::default()).unwrap();

    // Old key should not exist
    assert!(store.get(&from, true).unwrap().is_none());

    // New key should have the value
    assert_eq!(store.get_text(&to).unwrap(), Some("Moving value".to_string()));
}

/// Verify move fails if destination exists (without force)
#[test]
fn test_move_fails_if_exists() {
    let (store, _temp) = common::test_store();

    let from = Key::new("source").unwrap();
    let to = Key::new("destination").unwrap();

    store.set(&from, "Source value").unwrap();
    store.set(&to, "Existing value").unwrap();

    // Move should fail
    let result = store.mv(&from, &to, Default::default());
    assert!(matches!(result, Err(Error::KeyExists(_))));
}

/// Verify move with force overwrites destination
#[test]
fn test_move_force() {
    let (store, _temp) = common::test_store();

    let from = Key::new("source").unwrap();
    let to = Key::new("destination").unwrap();

    store.set(&from, "Source value").unwrap();
    store.set(&to, "Existing value").unwrap();

    // Move with force
    store.mv(&from, &to, keva_core::storage::MoveOptions { force: true }).unwrap();

    // Destination should have source's value
    assert_eq!(store.get_text(&to).unwrap(), Some("Source value".to_string()));

    // Source should be gone
    assert!(store.get(&from, true).unwrap().is_none());
}
