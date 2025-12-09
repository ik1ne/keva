/// Verify `get` returns `None` for non-existent keys.
#[test]
fn test_get_non_existent() {}

/// Verify `set` overwrites an existing value for a key without affecting its children or parent.
#[test]
fn test_set_isolation() {}

/// Verify `ls` returns direct children of a given key.
#[test]
fn test_list_children() {}

/// Verify removing an item (default config) marks it as `Trash` but keeps the data.
#[test]
fn test_soft_delete() {}

/// Verify removing an item with `permanent: true` removes it entirely.
#[test]
fn test_permanent_delete() {}

/// Verify that deleting a parent key does NOT remove its children.
#[test]
fn test_delete_non_recursive() {}
