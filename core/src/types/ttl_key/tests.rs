use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn ttl_key_normal_usage() {
    let now = SystemTime::now();
    let key_str = "valid_key";
    let key = Key::try_from(key_str).unwrap();
    let ttl_key = TtlKey {
        timestamp: now,
        key: key.clone(),
    };

    let bytes = <TtlKey as redb::Value>::as_bytes(&ttl_key);
    let ttl_key_from_bytes = <TtlKey as redb::Value>::from_bytes(&bytes);
    assert_eq!(ttl_key, ttl_key_from_bytes);
}

#[test]
fn ttl_key_ordering() {
    let now = SystemTime::now();
    let later = now + Duration::from_secs(10);

    let key1 = TtlKey {
        timestamp: now,
        key: Key::try_from("a").unwrap(),
    };
    let key2 = TtlKey {
        timestamp: now,
        key: Key::try_from("b").unwrap(),
    };
    let key3 = TtlKey {
        timestamp: later,
        key: Key::try_from("a").unwrap(),
    };

    let bytes1 = <TtlKey as redb::Value>::as_bytes(&key1);
    let bytes2 = <TtlKey as redb::Value>::as_bytes(&key2);
    let bytes3 = <TtlKey as redb::Value>::as_bytes(&key3);

    assert_eq!(
        <TtlKey as redb::Key>::compare(&bytes1, &bytes2),
        "a".cmp("b")
    );
    assert_eq!(
        <TtlKey as redb::Key>::compare(&bytes1, &bytes3),
        now.cmp(&later)
    );
    assert_eq!(
        <TtlKey as redb::Key>::compare(&bytes1, &bytes1),
        Ordering::Equal
    )
}
