use keva_core::{KevaStore, Value};
use tempfile::tempdir;

mod common;

#[test]
fn test_value_types() {
    let tmp = tempdir().unwrap();
    let mut store = KevaStore::open(tmp.path()).unwrap();

    let bin_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
    store
        .set("binary", Value::BinaryEmbedded(bin_data.clone()))
        .unwrap();

    let val = store.get("binary").unwrap();
    assert_eq!(val, Some(Value::BinaryEmbedded(bin_data)));
}
