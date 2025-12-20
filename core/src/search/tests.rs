use super::*;

fn make_key(s: &str) -> Key {
    Key::try_new(s.to_string()).unwrap()
}

#[test]
fn test_active_and_trash_are_separate() {
    let active = vec![make_key("a1"), make_key("a2")];
    let trashed = vec![make_key("t1")];

    let mut engine = SearchEngine::new(active, trashed, SearchConfig::default());

    let results = engine
        .search(SearchQuery::Fuzzy("1".to_string()), 100)
        .unwrap();

    // Active results should come first and have is_trash=false; trash results last with is_trash=true.
    let keys: Vec<(&str, bool)> = results
        .iter()
        .map(|r| (r.key.as_str(), r.is_trash))
        .collect();

    assert!(keys.contains(&("a1", false)));
    assert!(keys.contains(&("t1", true)));
}

#[test]
fn test_trash_and_restore_reflected_in_search() {
    let active = vec![make_key("foo")];
    let trashed = vec![];

    let mut engine = SearchEngine::new(active, trashed, SearchConfig::default());

    // Initially active.
    let r1 = engine
        .search(SearchQuery::Fuzzy("foo".to_string()), 100)
        .unwrap();
    assert!(r1.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));

    // Trash it.
    engine.trash(&make_key("foo"));
    let r2 = engine
        .search(SearchQuery::Fuzzy("foo".to_string()), 100)
        .unwrap();
    assert!(r2.iter().any(|r| r.key.as_str() == "foo" && r.is_trash));
    assert!(!r2.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));

    // Restore it.
    engine.restore(&make_key("foo"));
    let r3 = engine
        .search(SearchQuery::Fuzzy("foo".to_string()), 100)
        .unwrap();
    assert!(r3.iter().any(|r| r.key.as_str() == "foo" && !r.is_trash));
    assert!(!r3.iter().any(|r| r.key.as_str() == "foo" && r.is_trash));
}

#[test]
fn test_remove_removes_from_both() {
    let active = vec![make_key("k")];
    let trashed = vec![make_key("k2")];

    let mut engine = SearchEngine::new(active, trashed, SearchConfig::default());
    engine.remove(&make_key("k"));
    engine.remove(&make_key("k2"));

    let r = engine
        .search(SearchQuery::Fuzzy("k".to_string()), 100)
        .unwrap();

    assert!(!r.iter().any(|x| x.key.as_str() == "k"));
    assert!(!r.iter().any(|x| x.key.as_str() == "k2"));
}
