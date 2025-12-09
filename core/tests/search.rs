/// Verify search can toggle between searching Key paths vs Key paths + Value content.
#[test]
fn test_search_scope_keys_vs_content() {}

/// Verify search results include `Trash` items when requested (or by default) but rank them lowest.
#[test]
fn test_search_includes_trash_ranked_low() {}

/// Verify search results **never** return items that have exceeded the Purge TTL.
#[test]
fn test_search_excludes_purged() {}
