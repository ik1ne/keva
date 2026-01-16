use super::*;

#[test]
fn key_normal_usage() {
    let key_str = "valid_key";
    let key = Key::try_from(key_str).unwrap();
    assert_eq!(key.as_str(), key_str);

    let bytes = <Key as redb::Value>::as_bytes(&key);
    let key_from_bytes = <Key as redb::Value>::from_bytes(bytes);
    assert_eq!(key, key_from_bytes);
}

#[test]
fn key_rejects_empty_string() {
    let result = Key::try_from("");
    result.unwrap_err();
}

#[test]
fn key_rejects_whitespace_string() {
    let result = Key::try_from("   ");
    result.unwrap_err();
}

#[test]
fn key_rejects_too_long_string() {
    let long_string = "a".repeat(MAX_KEY_LENGTH + 1);
    let result = Key::try_from(long_string.as_str());
    result.unwrap_err();
}

#[test]
fn key_ordering() {
    const KEYS: [&str; 4] = ["a", "b", "a/", "apple"];

    for l in KEYS.iter() {
        for r in KEYS.iter() {
            let key_l = Key::try_from(*l).unwrap();
            let key_r = Key::try_from(*r).unwrap();
            let expected_ordering = l.cmp(r);
            assert_eq!(
                key_l.cmp(&key_r),
                expected_ordering,
                "Comparing '{}' and '{}'",
                l,
                r
            );
        }
    }
}
