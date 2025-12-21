use super::*;
use keva_core::types::Key;
use std::sync::Arc;

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

    pub(super) fn no_op_notify() -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }

    pub(super) fn create_engine() -> SearchEngine {
        SearchEngine::new(vec![], vec![], test_config(), no_op_notify())
    }

    pub(super) fn create_engine_with_active(keys: &[&str]) -> SearchEngine {
        let active = keys.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active, vec![], test_config(), no_op_notify())
    }

    pub(super) fn create_engine_with_both(active: &[&str], trashed: &[&str]) -> SearchEngine {
        let active_keys = active.iter().map(|s| make_key(s)).collect();
        let trashed_keys = trashed.iter().map(|s| make_key(s)).collect();
        SearchEngine::new(active_keys, trashed_keys, test_config(), no_op_notify())
    }

    /// Runs search to completion.
    pub(super) fn search(engine: &mut SearchEngine, query: &str) {
        engine.set_query(SearchQuery::Fuzzy(query.to_string()));
        while !engine.is_finished() {
            engine.tick();
        }
    }
}

mod new {
    use super::common::*;
    use super::*;

    #[test]
    fn test_new_with_active_keys() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 2);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_new_with_trashed_keys() {
        let trashed = vec![make_key("trashed1"), make_key("trashed2")];
        let mut engine = SearchEngine::new(vec![], trashed, test_config(), no_op_notify());

        search(&mut engine, "trashed");

        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 2);
    }

    #[test]
    fn test_new_with_both_active_and_trashed() {
        let mut engine = create_engine_with_both(&["active"], &["trashed"]);

        search(&mut engine, "a");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 1);
    }

    #[test]
    fn test_new_empty() {
        let mut engine = create_engine();

        search(&mut engine, "anything");

        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }
}

mod add_active {
    use super::common::*;

    #[test]
    fn test_add_active_new_key() {
        let mut engine = create_engine();

        engine.add_active(make_key("new_key"));

        search(&mut engine, "new_key");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_add_active_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        // Verify key is in trash
        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 1);

        // Add as active should move it from trash
        engine.add_active(make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_add_active_idempotent() {
        let mut engine = create_engine();

        engine.add_active(make_key("key"));
        engine.add_active(make_key("key")); // Add again

        search(&mut engine, "key");

        // Should only appear once
        assert_eq!(engine.active_results().iter().count(), 1);
    }
}

mod trash {
    use super::common::*;

    #[test]
    fn test_trash_moves_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        // Verify key is active
        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);

        engine.trash(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 1);
    }

    #[test]
    fn test_trash_key_not_in_active_adds_to_trash() {
        let mut engine = create_engine();

        // Trashing a key not in active index adds it to trash index
        engine.trash(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 1);
    }
}

mod restore {
    use super::common::*;

    #[test]
    fn test_restore_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.restore(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_restore_key_not_in_trash_adds_to_active() {
        let mut engine = create_engine();

        // Restoring a key not in trash index adds it to active index
        engine.restore(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_trash_then_restore_roundtrip() {
        let mut engine = create_engine_with_active(&["foo"]);

        // Initially active
        search(&mut engine, "foo");
        assert!(engine.active_results().iter().any(|k| k.as_str() == "foo"));
        assert_eq!(engine.trashed_results().iter().count(), 0);

        // Trash it
        engine.trash(&make_key("foo"));
        search(&mut engine, "foo");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert!(engine.trashed_results().iter().any(|k| k.as_str() == "foo"));

        // Restore it
        engine.restore(&make_key("foo"));
        search(&mut engine, "foo");
        assert!(engine.active_results().iter().any(|k| k.as_str() == "foo"));
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }
}

mod remove {
    use super::common::*;

    #[test]
    fn test_remove_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.remove(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_remove_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.remove(&make_key("key"));

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_remove_removes_from_both() {
        let mut engine = create_engine_with_both(&["k"], &["k2"]);

        engine.remove(&make_key("k"));
        engine.remove(&make_key("k2"));

        search(&mut engine, "k");
        assert!(!engine.active_results().iter().any(|k| k.as_str() == "k"));
        assert!(!engine.trashed_results().iter().any(|k| k.as_str() == "k2"));
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut engine = create_engine();

        // Should not panic or error
        engine.remove(&make_key("nonexistent"));
    }
}

mod rename {
    use super::common::*;

    #[test]
    fn test_rename_in_active() {
        let mut engine = create_engine_with_active(&["old_key"]);

        engine.rename(&make_key("old_key"), make_key("new_key"));

        search(&mut engine, "new_key");
        assert!(
            engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "new_key")
        );
        assert!(
            !engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "old_key")
        );
    }

    #[test]
    fn test_rename_in_trash() {
        let mut engine = create_engine_with_both(&[], &["old_key"]);

        engine.rename(&make_key("old_key"), make_key("new_key"));

        search(&mut engine, "new_key");
        assert!(
            engine
                .trashed_results()
                .iter()
                .any(|k| k.as_str() == "new_key")
        );
        assert!(
            !engine
                .trashed_results()
                .iter()
                .any(|k| k.as_str() == "old_key")
        );
    }

    #[test]
    fn test_rename_nonexistent_is_noop() {
        let mut engine = create_engine();

        // Should not panic or error
        engine.rename(&make_key("nonexistent"), make_key("new_key"));
    }
}

mod search_tests {
    use super::common::*;

    #[test]
    fn test_search_separates_active_and_trashed() {
        let mut engine = create_engine_with_both(&["a1", "a2"], &["t1"]);

        search(&mut engine, "1");

        // Active and trashed results are in separate containers
        assert!(engine.active_results().iter().any(|k| k.as_str() == "a1"));
        assert!(engine.trashed_results().iter().any(|k| k.as_str() == "t1"));
    }

    #[test]
    fn test_search_empty_pattern() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        search(&mut engine, "");

        // Empty pattern matches everything
        assert_eq!(engine.active_results().iter().count(), 2);
    }

    #[test]
    fn test_search_no_matches() {
        let mut engine = create_engine_with_active(&["apple", "banana"]);

        search(&mut engine, "xyz");

        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 0);
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
            no_op_notify(),
        );

        // Should only match "TestKey" with capital T
        search(&mut engine, "Test");

        assert!(
            engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "TestKey")
        );
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
            no_op_notify(),
        );

        search(&mut engine, "TEST");

        // Both should match
        assert_eq!(engine.active_results().iter().count(), 2);
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
            no_op_notify(),
        );

        // Lowercase query should match both (case-insensitive)
        search(&mut engine, "test");

        assert_eq!(engine.active_results().iter().count(), 2);
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
            no_op_notify(),
        );

        // Query with uppercase should be case-sensitive
        search(&mut engine, "Test");

        // With smart case, "Test" matches "TestKey" but not "testkey"
        assert!(
            engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "TestKey")
        );
    }
}

mod maintenance {
    use super::common::*;

    #[test]
    fn test_maintenance_compact_does_not_affect_search() {
        let mut engine = create_engine_with_active(&["key1", "key2", "key3"]);

        // Remove some keys to trigger potential compaction
        engine.remove(&make_key("key1"));

        // Run maintenance
        engine.maintenance_compact();

        // Search should still work correctly
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 2);
        assert!(!engine.active_results().iter().any(|k| k.as_str() == "key1"));
        assert!(engine.active_results().iter().any(|k| k.as_str() == "key2"));
        assert!(engine.active_results().iter().any(|k| k.as_str() == "key3"));
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
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 40);
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
        search(&mut engine, "target");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(
            engine.active_results().iter().next().unwrap().as_str(),
            "target"
        );
    }
}

mod tick_behavior {
    use super::common::*;
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_tick_zero_is_non_blocking() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));

        // tick(0) should return immediately
        engine.tick();

        // Should be able to get results (may or may not be finished)
        let _results = engine.active_results();
    }

    #[test]
    fn test_callback_is_invoked() {
        let notified = Arc::new(AtomicBool::new(false));
        let notified_clone = notified.clone();
        let notify = Arc::new(move || {
            notified_clone.store(true, Ordering::SeqCst);
        });

        let mut engine = SearchEngine::new(vec![make_key("key")], vec![], test_config(), notify);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));

        // Tick until finished
        for _ in 0..100 {
            engine.tick();
            if engine.is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Callback should have been invoked
        assert!(notified.load(Ordering::SeqCst));
    }

    #[test]
    fn test_is_finished_after_search_completes() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));

        // Tick until finished
        for _ in 0..100 {
            engine.tick();
            if engine.is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(engine.is_finished());
    }
}
