use super::*;
use common::{
    create_engine, create_engine_with_active, create_engine_with_both, make_key, no_op_notify,
    search, test_config,
};
use keva_core::types::Key;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod common {
    use super::*;

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_new(s.to_string()).unwrap()
    }

    pub(super) fn test_config() -> SearchConfig {
        SearchConfig::default()
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

    pub(super) fn search(engine: &mut SearchEngine, query: &str) {
        engine.set_query(SearchQuery::Fuzzy(query.to_string()));
        while !engine.is_done() {
            engine.tick();
        }
    }
}

mod new {
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
    use super::*;

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

        search(&mut engine, "key");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 1);

        engine.add_active(make_key("key"));
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_add_active_idempotent() {
        let mut engine = create_engine();

        engine.add_active(make_key("key"));
        engine.add_active(make_key("key"));
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 1);
    }
}

mod trash {
    use super::*;

    #[test]
    fn test_trash_moves_from_active() {
        let mut engine = create_engine_with_active(&["key"]);

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

        engine.trash(&make_key("key"));
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 0);
        assert_eq!(engine.trashed_results().iter().count(), 1);
    }
}

mod restore {
    use super::*;

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

        engine.restore(&make_key("key"));
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }

    #[test]
    fn test_restore_trash_restore_roundtrip() {
        let mut engine = create_engine_with_active(&["foo"]);

        search(&mut engine, "foo");
        assert!(engine.active_results().iter().any(|k| k.as_str() == "foo"));
        assert_eq!(engine.trashed_results().iter().count(), 0);

        engine.trash(&make_key("foo"));
        search(&mut engine, "foo");
        assert_eq!(engine.active_results().iter().count(), 0);
        assert!(engine.trashed_results().iter().any(|k| k.as_str() == "foo"));

        engine.restore(&make_key("foo"));
        search(&mut engine, "foo");
        assert!(engine.active_results().iter().any(|k| k.as_str() == "foo"));
        assert_eq!(engine.trashed_results().iter().count(), 0);
    }
}

mod remove {
    use super::*;

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
    fn test_remove_from_both() {
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
        engine.remove(&make_key("nonexistent"));
    }
}

mod rename {
    use super::*;

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
        engine.rename(&make_key("nonexistent"), make_key("new_key"));
    }
}

mod search {
    use super::*;
    #[test]
    fn test_search_separates_active_and_trashed() {
        let mut engine = create_engine_with_both(&["a1", "a2"], &["t1"]);

        search(&mut engine, "1");

        assert!(engine.active_results().iter().any(|k| k.as_str() == "a1"));
        assert!(engine.trashed_results().iter().any(|k| k.as_str() == "t1"));
    }

    #[test]
    fn test_search_empty_pattern() {
        let mut engine = create_engine_with_active(&["key1", "key2"]);

        search(&mut engine, "");

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
    use super::*;

    fn config_with_case(case_matching: CaseMatching) -> SearchConfig {
        SearchConfig {
            case_matching,
            ..SearchConfig::default()
        }
    }

    #[test]
    fn test_config_case_sensitive() {
        let config = config_with_case(CaseMatching::Sensitive);
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
            no_op_notify(),
        );

        search(&mut engine, "Test");

        assert!(
            engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "TestKey")
        );
    }

    #[test]
    fn test_config_case_insensitive() {
        let config = config_with_case(CaseMatching::Insensitive);
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
            no_op_notify(),
        );

        search(&mut engine, "TEST");

        assert_eq!(engine.active_results().iter().count(), 2);
    }

    #[test]
    fn test_config_smart_case_lowercase_query() {
        let config = config_with_case(CaseMatching::Smart);
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
            no_op_notify(),
        );

        search(&mut engine, "test");

        assert_eq!(engine.active_results().iter().count(), 2);
    }

    #[test]
    fn test_config_smart_case_uppercase_query() {
        let config = config_with_case(CaseMatching::Smart);
        let mut engine = SearchEngine::new(
            vec![make_key("TestKey"), make_key("testkey")],
            vec![],
            config,
            no_op_notify(),
        );

        search(&mut engine, "Test");

        assert!(
            engine
                .active_results()
                .iter()
                .any(|k| k.as_str() == "TestKey")
        );
    }
}

mod maintenance {
    use super::*;

    #[test]
    fn test_maintenance_compact_does_not_affect_search() {
        let mut engine = create_engine_with_active(&["key1", "key2", "key3"]);

        engine.remove(&make_key("key1"));
        engine.maintenance_compact();
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 2);
        assert!(!engine.active_results().iter().any(|k| k.as_str() == "key1"));
        assert!(engine.active_results().iter().any(|k| k.as_str() == "key2"));
        assert!(engine.active_results().iter().any(|k| k.as_str() == "key3"));
    }

    #[test]
    fn test_maintenance_after_many_deletions() {
        let mut engine = create_engine();

        for i in 0..150 {
            engine.add_active(make_key(&format!("key{}", i)));
        }
        for i in 0..110 {
            engine.remove(&make_key(&format!("key{}", i)));
        }
        engine.maintenance_compact();
        search(&mut engine, "key");

        assert_eq!(engine.active_results().iter().count(), 40);
    }

    /// Re-adding a previously-removed key after rebuild must work.
    #[test]
    fn test_maintenance_insert_after_rebuild() {
        let mut engine = create_engine();

        engine.add_active(make_key("target"));
        for i in 0..110 {
            engine.add_active(make_key(&format!("filler{}", i)));
        }
        for i in 0..110 {
            engine.remove(&make_key(&format!("filler{}", i)));
        }
        engine.remove(&make_key("target"));
        engine.maintenance_compact();
        engine.add_active(make_key("target"));
        search(&mut engine, "target");

        assert_eq!(engine.active_results().iter().count(), 1);
        assert_eq!(
            engine.active_results().iter().next().unwrap().as_str(),
            "target"
        );
    }
}

mod tick {
    use super::*;

    #[test]
    fn test_tick_non_blocking() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));
        engine.tick();

        let _results = engine.active_results();
    }

    #[test]
    fn test_tick_callback_invoked() {
        let notified = Arc::new(AtomicBool::new(false));
        let notified_clone = notified.clone();
        let notify = Arc::new(move || {
            notified_clone.store(true, Ordering::SeqCst);
        });

        let mut engine = SearchEngine::new(vec![make_key("key")], vec![], test_config(), notify);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));
        for _ in 0..100 {
            engine.tick();
            if engine.is_done() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(notified.load(Ordering::SeqCst));
    }

    #[test]
    fn test_tick_is_done_after_completion() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));
        for _ in 0..100 {
            engine.tick();
            if engine.is_done() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(engine.is_done());
    }

    #[test]
    fn test_tick_returns_false_at_threshold() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));
        while !engine.is_done() {
            engine.tick();
        }

        assert!(!engine.tick());
        assert!(!engine.tick());
    }

    #[test]
    fn test_tick_set_query_resets_threshold() {
        let mut engine = create_engine_with_active(&["key"]);

        engine.set_query(SearchQuery::Fuzzy("key".to_string()));
        while !engine.is_done() {
            engine.tick();
        }
        assert!(engine.is_done());

        engine.set_query(SearchQuery::Fuzzy("k".to_string()));

        assert!(engine.tick());
    }
}

mod threshold {
    use super::*;
    use common::{make_key, no_op_notify};

    #[test]
    fn test_threshold_stops_at_result_limit() {
        let config = SearchConfig {
            active_result_limit: 5,
            trashed_result_limit: 2,
            ..SearchConfig::default()
        };

        let active: Vec<Key> = (0..20).map(|i| make_key(&format!("key{}", i))).collect();
        let trashed: Vec<Key> = (0..10).map(|i| make_key(&format!("trash{}", i))).collect();

        let mut engine = SearchEngine::new(active, trashed, config, no_op_notify());

        engine.set_query(SearchQuery::Fuzzy("".to_string()));
        while !engine.is_done() {
            engine.tick();
        }

        assert!(!engine.tick());
    }
}
