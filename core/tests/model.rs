/// Verify a single key path (e.g., `project`) can hold a value.
#[test]
fn test_parallel_storage_node_and_children() {}

/// Verify `get` returns only the value for the specific key, effectively ignoring children.
#[test]
fn test_node_retrieval_returns_value_only() {}

/// Verify storing and retrieving UTF-8 string values.
#[test]
fn test_store_text_value() {}

/// Verify storing and retrieving Markdown content (metadata indicating type).
#[test]
fn test_store_rich_text() {}

/// Verify importing a file <1MB stores it inline (Redb) and retrieves it identically.
#[test]
fn test_store_small_embedded_file() {}

/// Verify importing a file >=1MB stores it in blob storage and retrieves it identically.
#[test]
fn test_store_large_embedded_file() {}

/// Verify linking a file stores only the OS path and retrieves that path.
#[test]
fn test_store_linked_file() {}

/// Verify retrieved items correctly report their type.
#[test]
fn test_value_type_metadata() {}
