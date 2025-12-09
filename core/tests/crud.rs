use keva_core::{KevaStore, Value};
use tempfile::tempdir;

mod common;

#[test]
fn test_basic_crud() {
    let tmp = tempdir().unwrap();
    let mut store = KevaStore::open(tmp.path()).expect("failed to open store");

    // Test Set & Get Text
    store
        .set("greetings", Value::Text("hello world".into()))
        .expect("failed to set text");

    let val = store.get("greetings").expect("failed to get value");
    assert_eq!(val, Some(Value::Text("hello world".into())));

    // Test Remove
    store.pdel("greetings").expect("failed to delete");
    let val = store.get("greetings").expect("failed to get value");
    assert_eq!(val, None);
}
