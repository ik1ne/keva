/// Verify search can toggle between searching Key paths vs Key paths + Value content.
#[test]
fn test_search_scope_keys_vs_content() {}

/// Verify search results include `Trash` items when requested (or by default) but rank them lowest.
#[test]
fn test_search_includes_trash_ranked_low() {}

/// Verify search results **never** return items that have exceeded the Purge TTL.
#[test]
fn test_search_excludes_purged() {}

/// Verify that a query with special characters (e.g. `*`) triggers Regex mode.
#[test]
fn test_search_auto_detects_regex_mode() {}

/// Verify that a standard string triggers Fuzzy mode.
#[test]
fn test_search_defaults_to_fuzzy_mode() {}
