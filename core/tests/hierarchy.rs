use keva_core::{KevaStore, Value};
use tempfile::tempdir;

mod common;

#[test]
fn test_hierarchy_listing() {
    let tmp = tempdir().unwrap();
    let mut store = KevaStore::open(tmp.path()).unwrap();

    store.set("a/b", Value::Text("child1".into())).unwrap();
    store.set("a/c", Value::Text("child2".into())).unwrap();

    let children = store.ls("a").unwrap();
    // Order is not guaranteed, check content
    assert!(children.contains(&"b".to_string()));
    assert!(children.contains(&"c".to_string()));
}

#[test]
fn test_overlapping_key_and_child() {
    let tmp = tempdir().unwrap();
    let mut store = KevaStore::open(tmp.path()).unwrap();

    // Strategy: Abandon "." replacement. Keys should coexist directly.
    store
        .set("project", Value::Text("Project Description".into()))
        .unwrap();

    store
        .set("project/config", Value::Text("Config Data".into()))
        .unwrap();

    // Verify getting "project" returns the text
    let val = store.get("project").unwrap();
    assert_eq!(val, Some(Value::Text("Project Description".into())));

    // Verify "project" is also a parent
    let kids = store.ls("project").unwrap();
    assert!(kids.contains(&"config".to_string()));
}
