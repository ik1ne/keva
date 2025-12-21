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

    /// Runs search to completion and returns the final state.
    pub(super) fn search(engine: &mut SearchEngine, query: &str) -> SearchState {
        engine.set_query(SearchQuery::Fuzzy(query.to_string()));
        while !engine.state().is_finished() {
            engine.tick();
        }
        engine.state()
    }
}

mod new {
    use super::common::*;
    use super::*;

    #[test]
    fn test_new_with_active_keys() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let state = search(&mut engine, "key");

        assert_eq!(state.active_key_count(), 2);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_new_with_trashed_keys() {
        let trashed = vec![make_key("trashed1"), make_key("trashed2")];
        let mut engine = SearchEngine::new(vec![], trashed, test_config(), no_op_notify());

        let state = search(&mut engine, "trashed");

        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 2);
    }

    #[test]
    fn test_new_with_both_active_and_trashed() {
        let mut engine = create_engine_with_both(&["active"], &["trashed"]);

        let state = search(&mut engine, "a");

        assert_eq!(state.active_key_count(), 1);
        assert_eq!(state.trashed_key_count(), 1);
    }

    #[test]
    fn test_new_empty() {
        let mut engine = create_engine();

        let state = search(&mut engine, "anything");

        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 0);
    }
}

mod add_active {
    use super::common::*;

    #[test]
    fn test_add_active_new_key() {
        let mut engine = create_engine();

        engine.add_active(make_key("new_key"));

        let state = search(&mut engine, "new_key");

        assert_eq!(state.active_key_count(), 1);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_add_active_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        // Verify key is in trash
        let before = search(&mut engine, "key");
        assert_eq!(before.active_key_count(), 0);
        assert_eq!(before.trashed_key_count(), 1);

        // Add as active should move it from trash
        engine.add_active(make_key("key"));

        let after = search(&mut engine, "key");
        assert_eq!(after.active_key_count(), 1);
        assert_eq!(after.trashed_key_count(), 0);
    }

    #[test]
    fn test_add_active_idempotent() {
        let mut engine = create_engine();

        engine.add_active(make_key("key"));
        engine.add_active(make_key("key")); // Add again

        let state = search(&mut engine, "key");

        // Should only appear once
        assert_eq!(state.active_key_count(), 1);
    }
}

mod trash {
    use super::common::*;

    #[test]
    fn test_trash_moves_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        // Verify key is active
        let before = search(&mut engine, "key");
        assert_eq!(before.active_key_count(), 1);
        assert_eq!(before.trashed_key_count(), 0);

        engine.trash(&make_key("key"));

        let after = search(&mut engine, "key");
        assert_eq!(after.active_key_count(), 0);
        assert_eq!(after.trashed_key_count(), 1);
    }

    #[test]
    fn test_trash_key_not_in_active_adds_to_trash() {
        let mut engine = create_engine();

        // Trashing a key not in active index adds it to trash index
        engine.trash(&make_key("key"));

        let state = search(&mut engine, "key");
        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 1);
    }
}

mod restore {
    use super::common::*;

    #[test]
    fn test_restore_moves_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.restore(&make_key("key"));

        let state = search(&mut engine, "key");
        assert_eq!(state.active_key_count(), 1);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_restore_key_not_in_trash_adds_to_active() {
        let mut engine = create_engine();

        // Restoring a key not in trash index adds it to active index
        engine.restore(&make_key("key"));

        let state = search(&mut engine, "key");
        assert_eq!(state.active_key_count(), 1);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_trash_then_restore_roundtrip() {
        let mut engine = create_engine_with_active(&["foo"]);

        // Initially active
        let r1 = search(&mut engine, "foo");
        assert!(r1.active_keys(..).iter().any(|k| k.as_str() == "foo"));
        assert_eq!(r1.trashed_key_count(), 0);

        // Trash it
        engine.trash(&make_key("foo"));
        let r2 = search(&mut engine, "foo");
        assert_eq!(r2.active_key_count(), 0);
        assert!(r2.trashed_keys(..).iter().any(|k| k.as_str() == "foo"));

        // Restore it
        engine.restore(&make_key("foo"));
        let r3 = search(&mut engine, "foo");
        assert!(r3.active_keys(..).iter().any(|k| k.as_str() == "foo"));
        assert_eq!(r3.trashed_key_count(), 0);
    }
}

mod remove {
    use super::common::*;

    #[test]
    fn test_remove_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.remove(&make_key("key"));

        let state = search(&mut engine, "key");
        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_remove_from_trash() {
        let mut engine = create_engine_with_both(&[], &["key"]);

        engine.remove(&make_key("key"));

        let state = search(&mut engine, "key");
        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 0);
    }

    #[test]
    fn test_remove_removes_from_both() {
        let mut engine = create_engine_with_both(&["k"], &["k2"]);

        engine.remove(&make_key("k"));
        engine.remove(&make_key("k2"));

        let state = search(&mut engine, "k");
        assert!(!state.active_keys(..).iter().any(|k| k.as_str() == "k"));
        assert!(!state.trashed_keys(..).iter().any(|k| k.as_str() == "k2"));
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut engine = create_engine();

        // Should not panic or error
        engine.remove(&make_key("nonexistent"));
    }
}

mod search_tests {
    use super::common::*;

    #[test]
    fn test_search_separates_active_and_trashed() {
        let mut engine = create_engine_with_both(&["a1", "a2"], &["t1"]);

        let state = search(&mut engine, "1");

        // Active and trashed results are in separate containers
        let active_keys: Vec<String> = state.active_keys(..).into_iter().map(|k| k.as_str().to_string()).collect();
        let trashed_keys: Vec<String> = state.trashed_keys(..).into_iter().map(|k| k.as_str().to_string()).collect();

        assert!(active_keys.iter().any(|k| k == "a1"));
        assert!(trashed_keys.iter().any(|k| k == "t1"));
    }

    #[test]
    fn test_search_empty_pattern() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        let state = search(&mut engine, "");

        // Empty pattern matches everything
        assert_eq!(state.active_key_count(), 2);
    }

    #[test]
    fn test_search_no_matches() {
        let mut engine = create_engine_with_active(&["apple", "banana"]);

        let state = search(&mut engine, "xyz");

        assert_eq!(state.active_key_count(), 0);
        assert_eq!(state.trashed_key_count(), 0);
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
        let state = search(&mut engine, "Test");

        let keys: Vec<String> = state.active_keys(..).into_iter().map(|k| k.as_str().to_string()).collect();
        assert!(keys.iter().any(|k| k == "TestKey"));
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

        let state = search(&mut engine, "TEST");

        // Both should match
        assert_eq!(state.active_key_count(), 2);
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
        let state = search(&mut engine, "test");

        assert_eq!(state.active_key_count(), 2);
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
        let state = search(&mut engine, "Test");

        // With smart case, "Test" matches "TestKey" but not "testkey"
        let keys: Vec<String> = state.active_keys(..).into_iter().map(|k| k.as_str().to_string()).collect();
        assert!(keys.iter().any(|k| k == "TestKey"));
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
        let state = search(&mut engine, "key");

        assert_eq!(state.active_key_count(), 2);
        let keys: Vec<String> = state.active_keys(..).into_iter().map(|k| k.as_str().to_string()).collect();
        assert!(!keys.iter().any(|k| k == "key1"));
        assert!(keys.iter().any(|k| k == "key2"));
        assert!(keys.iter().any(|k| k == "key3"));
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
        let state = search(&mut engine, "key");

        assert_eq!(state.active_key_count(), 40);
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
        let state = search(&mut engine, "target");

        assert_eq!(state.active_key_count(), 1);
        assert_eq!(state.active_keys(..).first().unwrap().as_str(), "target");
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

        // Should be able to get state (may or may not be finished)
        let _state = engine.state();
    }

    #[test]
    fn test_callback_is_invoked() {
        let notified = Arc::new(AtomicBool::new(false));
        let notified_clone = notified.clone();
        let notify = Arc::new(move || {
            notified_clone.store(true, Ordering::SeqCst);
        });

        let mut engine = SearchEngine::new(
            vec![make_key("key")],
            vec![],
            test_config(),
            notify,
        );

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));

        // Tick until finished
        for _ in 0..100 {
            engine.tick();
            if engine.state().is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Callback should have been invoked
        assert!(notified.load(Ordering::SeqCst));
    }

    #[test]
    fn test_state_is_finished_after_search_completes() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));

        // Tick until finished
        for _ in 0..100 {
            engine.tick();
            if engine.state().is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(engine.state().is_finished());
    }
}

mod range_bounds {
    use super::common::*;

    #[test]
    fn test_active_keys_with_range() {
        let mut engine = create_engine_with_active(&["a", "b", "c", "d", "e"]);

        let state = search(&mut engine, "");

        // Get first 2
        let first_two = state.active_keys(0..2);
        assert_eq!(first_two.len(), 2);

        // Get all
        let all = state.active_keys(..);
        assert_eq!(all.len(), 5);

        // Get from index 2
        let from_two = state.active_keys(2..);
        assert_eq!(from_two.len(), 3);
    }

    #[test]
    fn test_range_out_of_bounds_is_clamped() {
        let mut engine = create_engine_with_active(&["a", "b"]);

        let state = search(&mut engine, "");

        // Request more than available
        let result = state.active_keys(0..100);
        assert_eq!(result.len(), 2);

        // Start beyond end
        let empty = state.active_keys(10..20);
        assert!(empty.is_empty());
    }
}
