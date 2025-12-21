use super::*;
use tempfile::tempdir;

mod common {
    use super::*;
    pub(super) use redb::ReadableDatabase;

    pub(super) const TEST_TABLE: TtlTable = TtlTable::new("test_ttl");

    pub(super) fn create_test_db() -> (redb::Database, tempfile::TempDir) {
        let temp = tempdir().unwrap();
        let db = redb::Database::create(temp.path().join("test.redb")).unwrap();
        (db, temp)
    }

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_new(s.to_string()).unwrap()
    }

    pub(super) fn make_ttl_key(key: &str, timestamp: SystemTime) -> TtlKey {
        TtlKey {
            timestamp,
            key: make_key(key),
        }
    }
}

mod init {
    use super::common::*;

    #[test]
    fn test_init_creates_table() {
        let (db, _temp) = create_test_db();
        let write_txn = db.begin_write().unwrap();

        TEST_TABLE.init(&write_txn).unwrap();

        write_txn.commit().unwrap();

        // Verify table exists by reading from it
        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_init_idempotent() {
        let (db, _temp) = create_test_db();

        // Init twice should not error
        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        write_txn.commit().unwrap();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        write_txn.commit().unwrap();
    }
}

mod insert {
    use super::common::*;
    use super::*;

    #[test]
    fn test_insert_single_key() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("key1", now))
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_str(), "key1");
    }

    #[test]
    fn test_insert_multiple_keys() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("key1", now))
            .unwrap();
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("key2", now + Duration::from_secs(10)),
            )
            .unwrap();
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("key3", now + Duration::from_secs(20)),
            )
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_insert_duplicate_is_idempotent() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();
        let ttl_key = make_ttl_key("key1", now);

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE.insert(&write_txn, &ttl_key).unwrap();
        TEST_TABLE.insert(&write_txn, &ttl_key).unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert_eq!(keys.len(), 1);
    }
}

mod remove {
    use super::common::*;
    use super::*;

    #[test]
    fn test_remove_existing_key() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();
        let ttl_key = make_ttl_key("key1", now);

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE.insert(&write_txn, &ttl_key).unwrap();
        write_txn.commit().unwrap();

        let write_txn = db.begin_write().unwrap();
        let removed = TEST_TABLE.remove(&write_txn, &ttl_key).unwrap();
        write_txn.commit().unwrap();

        assert!(removed);

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_key() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        write_txn.commit().unwrap();

        let write_txn = db.begin_write().unwrap();
        let removed = TEST_TABLE
            .remove(&write_txn, &make_ttl_key("nonexistent", now))
            .unwrap();
        write_txn.commit().unwrap();

        assert!(!removed);
    }

    #[test]
    fn test_remove_wrong_timestamp() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("key1", now))
            .unwrap();
        write_txn.commit().unwrap();

        // Try to remove with different timestamp
        let write_txn = db.begin_write().unwrap();
        let removed = TEST_TABLE
            .remove(
                &write_txn,
                &make_ttl_key("key1", now + Duration::from_secs(1)),
            )
            .unwrap();
        write_txn.commit().unwrap();

        // Should not remove because timestamp differs
        assert!(!removed);

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();
        assert_eq!(keys.len(), 1);
    }
}

mod expired_keys {
    use super::common::*;
    use super::*;

    #[test]
    fn test_no_expired_keys() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("key1", now))
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let expired = TEST_TABLE
            .expired_keys(&read_txn, now, Duration::from_secs(100))
            .unwrap();

        assert!(expired.is_empty());
    }

    #[test]
    fn test_all_keys_expired() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();
        let past = now - Duration::from_secs(200);

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("key1", past))
            .unwrap();
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("key2", past + Duration::from_secs(10)),
            )
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let expired = TEST_TABLE
            .expired_keys(&read_txn, now, Duration::from_secs(100))
            .unwrap();

        assert_eq!(expired.len(), 2);
    }

    #[test]
    fn test_some_keys_expired() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        // This key expired (timestamp + 100s <= now)
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("expired", now - Duration::from_secs(150)),
            )
            .unwrap();
        // This key not expired yet
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("active", now))
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let expired = TEST_TABLE
            .expired_keys(&read_txn, now, Duration::from_secs(100))
            .unwrap();

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].as_str(), "expired");
    }

    #[test]
    fn test_expired_keys_ordered_by_timestamp() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();
        let base = now - Duration::from_secs(500);

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        // Insert in non-chronological order
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("second", base + Duration::from_secs(100)),
            )
            .unwrap();
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("first", base))
            .unwrap();
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("third", base + Duration::from_secs(200)),
            )
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let expired = TEST_TABLE
            .expired_keys(&read_txn, now, Duration::from_secs(100))
            .unwrap();

        // Should be returned in timestamp order (oldest first)
        assert_eq!(expired.len(), 3);
        assert_eq!(expired[0].as_str(), "first");
        assert_eq!(expired[1].as_str(), "second");
        assert_eq!(expired[2].as_str(), "third");
    }

    #[test]
    fn test_expired_keys_boundary() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(100);

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        // Exactly at expiration boundary (timestamp + ttl == now)
        TEST_TABLE
            .insert(&write_txn, &make_ttl_key("boundary", now - ttl))
            .unwrap();
        // Just before expiration
        TEST_TABLE
            .insert(
                &write_txn,
                &make_ttl_key("not_expired", now - ttl + Duration::from_secs(1)),
            )
            .unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let expired = TEST_TABLE.expired_keys(&read_txn, now, ttl).unwrap();

        // Boundary case: expires_at <= now means exactly at boundary is expired
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].as_str(), "boundary");
    }
}

mod all_keys {
    use super::common::*;
    use super::*;

    #[test]
    fn test_all_keys_empty() {
        let (db, _temp) = create_test_db();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();

        assert!(keys.is_empty());
    }

    #[test]
    fn test_all_keys_returns_all() {
        let (db, _temp) = create_test_db();
        let now = SystemTime::now();

        let write_txn = db.begin_write().unwrap();
        TEST_TABLE.init(&write_txn).unwrap();
        for i in 0..5 {
            TEST_TABLE
                .insert(
                    &write_txn,
                    &make_ttl_key(&format!("key{}", i), now + Duration::from_secs(i)),
                )
                .unwrap();
        }
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let keys = TEST_TABLE.all_keys(&read_txn).unwrap();

        assert_eq!(keys.len(), 5);
    }
}
