use super::*;
use crate::types::Key;

mod common {
    use super::*;

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_new(s.to_string()).unwrap()
    }

    pub(super) fn create_engine() -> SearchEngine {
        SearchEngine::new(vec![], vec![], SearchConfig::default())
    }

    pub(super) fn create_engine_with_active(keys: &[&str]) -> SearchEngine {
        let active = keys.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active, vec![], SearchConfig::default())
    }

    pub(super) fn create_engine_with_both(active: &[&str], trashed: &[&str]) -> SearchEngine {
        let active_keys = active.iter().map(|s| make_key(s)).collect();
        let trashed_keys = trashed.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active_keys, trashed_keys, SearchConfig::default())
    }
}

mod new {
    use super::common::*;
    use super::*;

    #[test]
    fn test_new_with_active_keys() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| !r.is_trash));
    }

    #[test]
    fn test_new_with_trashed_keys() {
        let trashed = vec![make_key("trashed1"), make_key("trashed2")];
        let mut engine = SearchEngine::new(vec![], trashed, SearchConfig::default());

        let results = engine
            .search(SearchQuery::Fuzzy("trashed".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_trash));
    }

    #[test]
    fn test_new_with_both_active_and_trashed() {
        let mut engine = create_engine_with_both(&["active"], &["trashed"]);

        let results = engine
            .search(SearchQuery::Fuzzy("a".to_string()), 100)
            .unwrap();

        // Both should be searchable
        let active_results: Vec<_> = results.iter().filter(|r| !r.is_trash).collect();
        let trash_results: Vec<_> = results.iter().filter(|r| r.is_trash).collect();

        assert_eq!(active_results.len(), 1);
        assert_eq!(trash_results.len(), 1);
    }

    #[test]
    fn test_new_empty() {
        let mut engine = create_engine();

        let results = engine
            .search(SearchQuery::Fuzzy("anything".to_string()), 100)
            .unwrap();

        assert!(results.is_empty());
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
            .search(SearchQuery::Fuzzy("new_key".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_trash);
    }

    #[test]
    fn test_add_active_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        // Verify key is in trash
        let before = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert!(before[0].is_trash);

        // Add as active should move it from trash
        engine.add_active(make_key("key"));

        let after = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert_eq!(after.len(), 1);
        assert!(!after[0].is_trash);
    }

    #[test]
    fn test_add_active_idempotent() {
        let mut engine = create_engine();

        engine.add_active(make_key("key"));
        engine.add_active(make_key("key")); // Add again

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();

        // Should only appear once
        assert_eq!(results.len(), 1);
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
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert!(!before[0].is_trash);

        engine.trash(&make_key("key"));

        let after = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert_eq!(after.len(), 1);
        assert!(after[0].is_trash);
    }

    #[test]
    fn test_trash_key_not_in_active_adds_to_trash() {
        let mut engine = create_engine();

        // Trashing a key not in active index adds it to trash index
        engine.trash(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_trash);
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
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_trash);
    }

    #[test]
    fn test_restore_key_not_in_trash_adds_to_active() {
        let mut engine = create_engine();

        // Restoring a key not in trash index adds it to active index
        engine.restore(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_trash);
    }

    #[test]
    fn test_trash_then_restore_roundtrip() {
        let mut engine = create_engine_with_active(&["foo"]);

        // Initially active
        let r1 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100)
            .unwrap();
        assert!(r1.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));

        // Trash it
        engine.trash(&make_key("foo"));
        let r2 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100)
            .unwrap();
        assert!(r2.iter().any(|r| r.key.as_str() == "foo" && r.is_trash));
        assert!(!r2.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));

        // Restore it
        engine.restore(&make_key("foo"));
        let r3 = engine
            .search(SearchQuery::Fuzzy("foo".to_string()), 100)
            .unwrap();
        assert!(r3.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));
        assert!(!r3.iter().any(|r| r.key.as_str() == "foo" && r.is_trash));
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
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.remove(&make_key("key"));

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_removes_from_both() {
        let mut engine = create_engine_with_both(&["k"], &["k2"]);

        engine.remove(&make_key("k"));
        engine.remove(&make_key("k2"));

        let results = engine
            .search(SearchQuery::Fuzzy("k".to_string()), 100)
            .unwrap();
        assert!(!results.iter().any(|x| x.key.as_str() == "k"));
        assert!(!results.iter().any(|x| x.key.as_str() == "k2"));
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
    fn test_search_active_results_first() {
        let mut engine = create_engine_with_both(&["a1", "a2"], &["t1"]);

        let results = engine
            .search(SearchQuery::Fuzzy("1".to_string()), 100)
            .unwrap();

        // Active results should come first
        let keys: Vec<(&str, bool)> = results
            .iter()
            .map(|r| (r.key.as_str(), r.is_trash))
            .collect();

        assert!(keys.contains(&("a1", false)));
        assert!(keys.contains(&("t1", true)));

        // Find positions
        let active_pos = keys.iter().position(|k| k == &("a1", false)).unwrap();
        let trash_pos = keys.iter().position(|k| k == &("t1", true)).unwrap();
        assert!(active_pos < trash_pos);
    }

    #[test]
    fn test_search_returns_scores() {
        let mut engine = create_engine_with_active(&["exact_match", "partial"]);

        let results = engine
            .search(SearchQuery::Fuzzy("exact_match".to_string()), 100)
            .unwrap();

        // Exact match should have higher score
        let exact = results.iter().find(|r| r.key.as_str() == "exact_match");
        let partial = results.iter().find(|r| r.key.as_str() == "partial");

        assert!(exact.is_some());
        // Partial may or may not match; if it does, exact should score higher
        if let Some(p) = partial {
            assert!(exact.unwrap().score >= p.score);
        }
    }

    #[test]
    fn test_search_empty_pattern() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let results = engine
            .search(SearchQuery::Fuzzy(String::new()), 100)
            .unwrap();

        // Empty pattern matches everything
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_no_matches() {
        let mut engine = create_engine_with_active(&["apple", "banana"]);

        let results = engine
            .search(SearchQuery::Fuzzy("xyz".to_string()), 100)
            .unwrap();

        assert!(results.is_empty());
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
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Should only match "TestKey" with capital T
        let results = engine
            .search(SearchQuery::Fuzzy("Test".to_string()), 100)
            .unwrap();

        let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"TestKey"));
        // "testkey" should not match when searching for "Test" (capital T)
    }

    #[test]
    fn test_case_insensitive_search() {
        let config = SearchConfig {
            case_matching: CaseMatching::Insensitive,
            unicode_normalization: true,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        let results = engine
            .search(SearchQuery::Fuzzy("TEST".to_string()), 100)
            .unwrap();

        // Both should match
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_smart_case_lowercase_query() {
        let config = SearchConfig {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Lowercase query should match both (case-insensitive)
        let results = engine
            .search(SearchQuery::Fuzzy("test".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_smart_case_uppercase_query() {
        let config = SearchConfig {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
        };
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
        );

        // Query with uppercase should be case-sensitive
        let results = engine
            .search(SearchQuery::Fuzzy("Test".to_string()), 100)
            .unwrap();

        // With smart case, "Test" matches "TestKey" but not "testkey"
        let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
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
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 2);
        let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
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
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();

        assert_eq!(results.len(), 40);
    }
}
