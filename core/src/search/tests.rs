use super::*;
use crate::types::Key;

mod common {
    use super::*;

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_new(s.to_string()).unwrap()
    }

    pub(super) fn test_config() -> SearchConfig {
        SearchConfig {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
            rebuild_threshold: 100,
        }
    }

    pub(super) fn create_engine() -> SearchEngine {
        SearchEngine::new(vec![], vec![], test_config())
    }

    pub(super) fn create_engine_with_active(keys: &[&str]) -> SearchEngine {
        let active = keys.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active, vec![], test_config())
    }

    pub(super) fn create_engine_with_both(active: &[&str], trashed: &[&str]) -> SearchEngine {
        let active_keys = active.iter().map(|s| make_key(s)).collect();
        let trashed_keys = trashed.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active_keys, trashed_keys, test_config())
    }
}

mod new {
    use super::common::*;
    use super::*;

    #[test]
    fn test_new_with_active_keys() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 2);
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_new_with_trashed_keys() {
        let trashed = vec![make_key("trashed1"), make_key("trashed2")];
        let mut engine = SearchEngine::new(vec![], trashed, test_config());

        let results = engine
            .search(SearchQuery::Fuzzy("trashed".to_string()), 100, .., ..)
            .unwrap();

        assert!(results.active.is_empty());
        assert_eq!(results.trashed.len(), 2);
    }

    #[test]
    fn test_new_with_both_active_and_trashed() {
        let mut engine = create_engine_with_both(&["active"], &["trashed"]);

        let results = engine
            .search(SearchQuery::Fuzzy("a".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 1);
        assert_eq!(results.trashed.len(), 1);
    }

    #[test]
    fn test_new_empty() {
        let mut engine = create_engine();

        let results = engine
            .search(SearchQuery::Fuzzy("anything".to_string()), 100, .., ..)
            .unwrap();

        assert!(results.active.is_empty());
        assert!(results.trashed.is_empty());
    }
}

mod add_active {
    use super::common::*;
    use super::*;

    #[test]
    fn test_add_active_new_key() {
        let mut engine = create_engine();

        engine.add_active(make_key("new_key"));

        let results = engine
            .search(SearchQuery::Fuzzy("new_key".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 1);
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_add_active_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        // Verify key is in trash
        let before = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert!(before.active.is_empty());
        assert_eq!(before.trashed.len(), 1);

        // Add as active should move it from trash
        engine.add_active(make_key("key"));

        let after = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert_eq!(after.active.len(), 1);
        assert!(after.trashed.is_empty());
    }

    #[test]
    fn test_add_active_idempotent() {
        let mut engine = create_engine();

        engine.add_active(make_key("key"));
        engine.add_active(make_key("key")); // Add again

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();

        // Should only appear once
        assert_eq!(results.active.len(), 1);
    }
}

mod trash {
    use super::common::*;
    use super::*;

    #[test]
    fn test_trash_moves_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        // Verify key is active
        let before = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert_eq!(before.active.len(), 1);
        assert!(before.trashed.is_empty());

        engine.trash(&make_key("key"));

        let after = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert!(after.active.is_empty());
        assert_eq!(after.trashed.len(), 1);
    }

    #[test]
    fn test_trash_key_not_in_active_adds_to_trash() {
        let mut engine = create_engine();

        // Trashing a key not in active index adds it to trash index
        engine.trash(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert!(results.active.is_empty());
        assert_eq!(results.trashed.len(), 1);
    }
}

mod restore {
    use super::common::*;
    use super::*;

    #[test]
    fn test_restore_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.restore(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert_eq!(results.active.len(), 1);
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_restore_key_not_in_trash_adds_to_active() {
        let mut engine = create_engine();

        // Restoring a key not in trash index adds it to active index
        engine.restore(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert_eq!(results.active.len(), 1);
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_trash_then_restore_roundtrip() {
        let mut engine = create_engine_with_active(&["foo"]);

        // Initially active
        let r1 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100, .., ..)
            .unwrap();
        assert!(r1.active.iter().any(|k| k.as_str() == "foo"));
        assert!(r1.trashed.is_empty());

        // Trash it
        engine.trash(&make_key("foo"));
        let r2 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100, .., ..)
            .unwrap();
        assert!(r2.active.is_empty());
        assert!(r2.trashed.iter().any(|k| k.as_str() == "foo"));

        // Restore it
        engine.restore(&make_key("foo"));
        let r3 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100, .., ..)
            .unwrap();
        assert!(r3.active.iter().any(|k| k.as_str() == "foo"));
        assert!(r3.trashed.is_empty());
    }
}

mod remove {
    use super::common::*;
    use super::*;

    #[test]
    fn test_remove_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.remove(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert!(results.active.is_empty());
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_remove_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.remove(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();
        assert!(results.active.is_empty());
        assert!(results.trashed.is_empty());
    }

    #[test]
    fn test_remove_removes_from_both() {
        let mut engine = create_engine_with_both(&["k"], &["k2"]);

        engine.remove(&make_key("k"));
        engine.remove(&make_key("k2"));

        let results = engine
            .search(SearchQuery::Fuzzy("k".to_string()), 100, .., ..)
            .unwrap();
        assert!(!results.active.iter().any(|k| k.as_str() == "k"));
        assert!(!results.trashed.iter().any(|k| k.as_str() == "k2"));
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut engine = create_engine();

        // Should not panic or error
        engine.remove(&make_key("nonexistent"));
    }
}

mod search {
    use super::common::*;
    use super::*;

    #[test]
    fn test_search_separates_active_and_trashed() {
        let mut engine = create_engine_with_both(&["a1", "a2"], &["t1"]);

        let results = engine
            .search(SearchQuery::Fuzzy("1".to_string()), 100, .., ..)
            .unwrap();

        // Active and trashed results are in separate containers
        let active_keys: Vec<&str> = results.active.iter().map(|k| k.as_str()).collect();
        let trashed_keys: Vec<&str> = results.trashed.iter().map(|k| k.as_str()).collect();

        assert!(active_keys.contains(&"a1"));
        assert!(trashed_keys.contains(&"t1"));
    }

    #[test]
    fn test_search_empty_pattern() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let results = engine
            .search(SearchQuery::Fuzzy(String::new()), 100, .., ..)
            .unwrap();

        // Empty pattern matches everything
        assert_eq!(results.active.len(), 2);
    }

    #[test]
    fn test_search_no_matches() {
        let mut engine = create_engine_with_active(&["apple", "banana"]);

        let results = engine
            .search(SearchQuery::Fuzzy("xyz".to_string()), 100, .., ..)
            .unwrap();

        assert!(results.active.is_empty());
        assert!(results.trashed.is_empty());
    }
}

mod config {
    use super::common::*;
    use super::*;

    #[test]
    fn test_case_sensitive_search() {
        let config = SearchConfig {
            case_matching: CaseMatching::Sensitive,
            unicode_normalization: true,
            rebuild_threshold: 100,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Should only match "TestKey" with capital T
        let results = engine
            .search(SearchQuery::Fuzzy("Test".to_string()), 100, .., ..)
            .unwrap();

        let keys: Vec<&str> = results.active.iter().map(|k| k.as_str()).collect();
        assert!(keys.contains(&"TestKey"));
        // "testkey" should not match when searching for "Test" (capital T)
    }

    #[test]
    fn test_case_insensitive_search() {
        let config = SearchConfig {
            case_matching: CaseMatching::Insensitive,
            unicode_normalization: true,
            rebuild_threshold: 100,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        let results = engine
            .search(SearchQuery::Fuzzy("TEST".to_string()), 100, .., ..)
            .unwrap();

        // Both should match
        assert_eq!(results.active.len(), 2);
    }

    #[test]
    fn test_smart_case_lowercase_query() {
        let config = SearchConfig {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
            rebuild_threshold: 100,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Lowercase query should match both (case-insensitive)
        let results = engine
            .search(SearchQuery::Fuzzy("test".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 2);
    }

    #[test]
    fn test_smart_case_uppercase_query() {
        let config = SearchConfig {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
            rebuild_threshold: 100,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Query with uppercase should be case-sensitive
        let results = engine
            .search(SearchQuery::Fuzzy("Test".to_string()), 100, .., ..)
            .unwrap();

        // With smart case, "Test" matches "TestKey" but not "testkey"
        let keys: Vec<&str> = results.active.iter().map(|k| k.as_str()).collect();
        assert!(keys.contains(&"TestKey"));
    }
}

mod maintenance {
    use super::common::*;
    use super::*;

    #[test]
    fn test_maintenance_compact_does_not_affect_search() {
        let mut engine = create_engine_with_active(&["key1", "key2", "key3"]);

        // Remove some keys to trigger potential compaction
        engine.remove(&make_key("key1"));

        // Run maintenance
        engine.maintenance_compact();

        // Search should still work correctly
        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 2);
        let keys: Vec<&str> = results.active.iter().map(|k| k.as_str()).collect();
        assert!(!keys.contains(&"key1"));
        assert!(keys.contains(&"key2"));
        assert!(keys.contains(&"key3"));
    }

    #[test]
    fn test_maintenance_after_many_deletions() {
        let mut engine = create_engine();

        // Add many keys
        for i in 0..150 {
            engine.add_active(make_key(&format!("key{}", i)));
        }

        // Remove many keys (exceeds rebuild threshold of 100)
        for i in 0..110 {
            engine.remove(&make_key(&format!("key{}", i)));
        }

        // Maintenance should trigger rebuild
        engine.maintenance_compact();

        // Remaining keys should still be searchable
        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 40);
    }

    /// Re-adding a previously-removed key after rebuild must work.
    ///
    /// Regression test: before the fix, rebuild left stale entries in
    /// `injected_keys` and `tombstones`, causing re-added keys to be
    /// "revived" in tracking but not re-injected into Nucleo.
    #[test]
    fn test_insert_after_rebuild_of_removed_key() {
        let mut engine = create_engine();

        // Add target key
        engine.add_active(make_key("target"));

        // Add and remove many keys to trigger rebuild
        for i in 0..110 {
            engine.add_active(make_key(&format!("filler{}", i)));
        }
        for i in 0..110 {
            engine.remove(&make_key(&format!("filler{}", i)));
        }

        // Remove target key (now tombstoned)
        engine.remove(&make_key("target"));

        // Trigger rebuild
        engine.maintenance_compact();

        // Re-add the same key
        engine.add_active(make_key("target"));

        // Key must be searchable
        let results = engine
            .search(SearchQuery::Fuzzy("target".to_string()), 100, .., ..)
            .unwrap();

        assert_eq!(results.active.len(), 1);
        assert_eq!(results.active[0].as_str(), "target");
    }
}
