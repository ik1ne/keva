mod common {
    use crate::core::db::Database;
    use crate::types::config::{Config, SavedConfig};
    use crate::types::key::Key;
    use std::time::Duration;
    use tempfile::TempDir;

    pub(super) fn create_test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
                purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
                inline_threshold_bytes: 1024 * 1024, // 1MB
            },
        };
        let db = Database::new(config).unwrap();
        (db, temp_dir)
    }

    pub(super) fn create_test_db_with_ttl(
        trash_ttl_secs: u64,
        purge_ttl_secs: u64,
    ) -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(trash_ttl_secs),
                purge_ttl: Duration::from_secs(purge_ttl_secs),
                inline_threshold_bytes: 1024 * 1024,
            },
        };
        let db = Database::new(config).unwrap();
        (db, temp_dir)
    }

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_from(s).unwrap()
    }
}

mod crud {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, TRASHED_TTL};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{
        FileData, InlineFileData, LifecycleState, TextData,
    };
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_insert_and_get_text() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("hello world".to_string())),
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("hello world".to_string()))
        );
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
        assert_eq!(value.metadata.created_at, now);
        assert_eq!(value.metadata.updated_at, now);
        assert!(value.metadata.trashed_at.is_none());

        // Verify TTL table: key should be in TRASHED_TTL for auto-trash scheduling
        let write_txn = db.db.begin_write().unwrap();
        let ttl_key = TtlKey {
            timestamp: now,
            key: key.clone(),
        };
        // Key should be in TRASHED_TTL (remove returns true)
        assert!(TRASHED_TTL.remove(&write_txn, &ttl_key).unwrap());
    }

    #[test]
    fn test_insert_and_get_files() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/with-file");
        let now = SystemTime::now();

        let file_data = FileData::Inlined(InlineFileData {
            file_name: "test.txt".to_string(),
            data: b"file content".to_vec(),
        });

        db.insert(&key, now, ClipData::Files(vec![file_data.clone()]))
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 1);
                assert_eq!(files[0], file_data);
            }
            ClipData::Text(_) => panic!("Expected Files variant"),
        }
    }

    #[test]
    fn test_get_nonexistent_key() {
        let (db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.get(&key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_overwrites_existing_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/overwrite");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("first".to_string())),
        )
        .unwrap();

        // Insert again should overwrite (Database always overwrites, Storage handles permission)
        let insert_time = now + Duration::from_secs(100);
        db.insert(
            &key,
            insert_time,
            ClipData::Text(TextData::Inlined("second".to_string())),
        )
        .unwrap();

        // Value should be the new one
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.created_at, insert_time);
        assert_eq!(value.metadata.updated_at, insert_time);
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
    }

    #[test]
    fn test_insert_overwrites_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed-overwrite");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("first".to_string())),
        )
        .unwrap();

        // Trash the key
        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        // Insert should overwrite the trashed key
        let insert_time = now + Duration::from_secs(100);
        db.insert(
            &key,
            insert_time,
            ClipData::Text(TextData::Inlined("second".to_string())),
        )
        .unwrap();

        // Value should be the new one, Active state
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.created_at, insert_time);
        assert_eq!(value.metadata.updated_at, insert_time);
        assert!(value.metadata.trashed_at.is_none());
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
    }

    #[test]
    fn test_overwrite_with_purge_then_insert() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/overwrite");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("first".to_string())),
        )
        .unwrap();

        // To overwrite, must purge first
        db.purge(&key).unwrap();

        let update_time = create_time + Duration::from_secs(100);
        db.insert(
            &key,
            update_time,
            ClipData::Text(TextData::Inlined("second".to_string())),
        )
        .unwrap();

        // Value should be the new one with new timestamps
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.created_at, update_time);
        assert_eq!(value.metadata.updated_at, update_time);
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
    }
}

mod append_files {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{FileData, InlineFileData, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_append_files_to_existing_files() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/append");
        let create_time = SystemTime::now();

        // Insert initial value with one file
        db.insert(
            &key,
            create_time,
            ClipData::Files(vec![FileData::Inlined(InlineFileData {
                file_name: "file1.txt".to_string(),
                data: b"content1".to_vec(),
            })]),
        )
        .unwrap();

        // Append another file
        let append_time = create_time + Duration::from_secs(50);
        db.append_files(
            &key,
            append_time,
            vec![FileData::Inlined(InlineFileData {
                file_name: "file2.txt".to_string(),
                data: b"content2".to_vec(),
            })],
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();

        // Should have 2 files now
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 2);
            }
            ClipData::Text(_) => panic!("Expected Files variant"),
        }

        // Timestamps should be updated
        assert_eq!(value.metadata.created_at, create_time);
        assert_eq!(value.metadata.updated_at, append_time);
    }

    #[test]
    fn test_append_files_to_text_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/append-to-text");
        let create_time = SystemTime::now();

        // Insert with text only
        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("text only".to_string())),
        )
        .unwrap();

        // Append a file should fail with TypeMismatch
        let result = db.append_files(
            &key,
            create_time + Duration::from_secs(50),
            vec![FileData::Inlined(InlineFileData {
                file_name: "new_file.txt".to_string(),
                data: b"new content".to_vec(),
            })],
        );
        assert!(matches!(result, Err(DatabaseError::TypeMismatch)));
    }

    #[test]
    fn test_append_files_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.append_files(
            &key,
            SystemTime::now(),
            vec![FileData::Inlined(InlineFileData {
                file_name: "file.txt".to_string(),
                data: b"content".to_vec(),
            })],
        );
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_append_files_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Files(vec![FileData::Inlined(InlineFileData {
                file_name: "file.txt".to_string(),
                data: b"content".to_vec(),
            })]),
        )
        .unwrap();

        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        let result = db.append_files(
            &key,
            now + Duration::from_secs(20),
            vec![FileData::Inlined(InlineFileData {
                file_name: "file2.txt".to_string(),
                data: b"content2".to_vec(),
            })],
        );
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_append_empty_files_is_noop() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/empty-append");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Files(vec![FileData::Inlined(InlineFileData {
                file_name: "file.txt".to_string(),
                data: b"content".to_vec(),
            })]),
        )
        .unwrap();

        // Append empty vec should be a no-op
        db.append_files(&key, now + Duration::from_secs(50), vec![])
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        // updated_at should NOT change because empty append is a no-op
        assert_eq!(value.metadata.updated_at, now);
    }
}

mod touch {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::TextData;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_touch_updates_timestamp() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/touch");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let touch_time = create_time + Duration::from_secs(50);
        db.touch(&key, touch_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.created_at, create_time);
        assert_eq!(value.metadata.updated_at, touch_time);
        // Content should be unchanged
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
    }

    #[test]
    fn test_touch_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.touch(&key, SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_touch_trashed_key_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        let result = db.touch(&key, now + Duration::from_secs(20));
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }
}

mod trash {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError, PURGED_TTL, TRASHED_TTL};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_trash_sets_lifecycle_and_timestamp() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trash");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = create_time + Duration::from_secs(100);
        db.trash(&key, trash_time).unwrap();

        // Verify metadata
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);
        assert_eq!(value.metadata.trashed_at, Some(trash_time));

        // Verify TTL tables: key should be moved from TRASHED_TTL to PURGED_TTL
        let write_txn = db.db.begin_write().unwrap();
        let old_ttl_key = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl_key = TtlKey {
            timestamp: trash_time,
            key: key.clone(),
        };
        // Key should no longer be in TRASHED_TTL (remove returns false)
        assert!(!TRASHED_TTL.remove(&write_txn, &old_ttl_key).unwrap());
        // Key should be in PURGED_TTL (remove returns true)
        assert!(PURGED_TTL.remove(&write_txn, &new_ttl_key).unwrap());
    }

    #[test]
    fn test_trash_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.trash(&key, SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_trash_already_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/double-trash");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        // Trying to trash again should fail
        let result = db.trash(&key, now + Duration::from_secs(20));
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }
}

mod purge {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::TextData;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_purge_removes_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/purge");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.purge(&key).unwrap();

        // Key should no longer exist
        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/purge-trashed");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.trash(&key, now + Duration::from_secs(10)).unwrap();
        db.purge(&key).unwrap();

        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.purge(&key);
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }
}

mod rename {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{LifecycleState, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_rename_key() {
        let (mut db, _temp) = create_test_db();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        db.insert(
            &old_key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.rename(&old_key, &new_key).unwrap();

        // Old key should not exist
        assert!(db.get(&old_key).unwrap().is_none());

        // New key should have the value
        let value = db.get(&new_key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
    }

    #[test]
    fn test_rename_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let old_key = make_key("old/trashed");
        let new_key = make_key("new/trashed");
        let now = SystemTime::now();

        db.insert(
            &old_key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&old_key, trash_time).unwrap();

        db.rename(&old_key, &new_key).unwrap();

        let value = db.get(&new_key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);
        assert_eq!(value.metadata.trashed_at, Some(trash_time));
    }

    #[test]
    fn test_rename_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let old_key = make_key("nonexistent");
        let new_key = make_key("new/key");

        let result = db.rename(&old_key, &new_key);
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_rename_overwrites_existing_key() {
        let (mut db, _temp) = create_test_db();
        let key1 = make_key("key1");
        let key2 = make_key("key2");
        let now = SystemTime::now();

        db.insert(
            &key1,
            now,
            ClipData::Text(TextData::Inlined("content1".to_string())),
        )
        .unwrap();

        db.insert(
            &key2,
            now,
            ClipData::Text(TextData::Inlined("content2".to_string())),
        )
        .unwrap();

        // Rename key1 to key2 should overwrite key2 (Database always overwrites, Storage handles permission)
        db.rename(&key1, &key2).unwrap();

        // key1 should not exist
        assert!(db.get(&key1).unwrap().is_none());

        // key2 should have key1's content
        let value = db.get(&key2).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content1".to_string()))
        );
    }

    #[test]
    fn test_rename_overwrites_trashed_new_key() {
        let (mut db, _temp) = create_test_db();
        let key1 = make_key("source");
        let key2 = make_key("target");
        let now = SystemTime::now();

        db.insert(
            &key1,
            now,
            ClipData::Text(TextData::Inlined("source content".to_string())),
        )
        .unwrap();

        db.insert(
            &key2,
            now,
            ClipData::Text(TextData::Inlined("target content".to_string())),
        )
        .unwrap();

        // Trash key2
        db.trash(&key2, now + Duration::from_secs(10)).unwrap();

        // Rename key1 to key2 should succeed (overwriting trashed key2)
        db.rename(&key1, &key2).unwrap();

        // key1 should not exist
        assert!(db.get(&key1).unwrap().is_none());

        // key2 should have key1's content
        let value = db.get(&key2).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("source content".to_string()))
        );
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
    }
}

mod keys {
    use super::common::{create_test_db, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::TextData;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_keys_empty_database() {
        let (db, _temp) = create_test_db();
        let keys = db.keys().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_keys_multiple_entries() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        let key_names = ["alpha", "beta", "gamma"];
        for name in &key_names {
            let key = make_key(name);
            db.insert(
                &key,
                now,
                ClipData::Text(TextData::Inlined(format!("content-{}", name))),
            )
            .unwrap();
        }

        let keys = db.keys().unwrap();
        assert_eq!(keys.len(), 3);

        // Convert to strings for easier comparison
        let key_strings: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
        for name in &key_names {
            assert!(key_strings.contains(&name.to_string()));
        }
    }

    #[test]
    fn test_keys_includes_trashed() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        let active_key = make_key("active");
        let trashed_key = make_key("trashed");

        db.insert(
            &active_key,
            now,
            ClipData::Text(TextData::Inlined("active".to_string())),
        )
        .unwrap();

        db.insert(
            &trashed_key,
            now,
            ClipData::Text(TextData::Inlined("trashed".to_string())),
        )
        .unwrap();

        db.trash(&trashed_key, now + Duration::from_secs(10))
            .unwrap();

        let keys = db.keys().unwrap();
        assert_eq!(keys.len(), 2);
    }
}

mod gc {
    use super::common::{create_test_db, create_test_db_with_ttl, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_gc_no_expired() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        db.insert(
            &make_key("test"),
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Immediately check - nothing should be expired
        let result = db.gc(now).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_gc_trashes_expired() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("test");

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Run GC after TTL has passed
        let gc_time = create_time + Duration::from_secs(150);
        let result = db.gc(gc_time).unwrap();

        assert_eq!(result.trashed.len(), 1);
        assert_eq!(result.trashed[0].to_string(), "test");
        assert!(result.purged.is_empty());

        // Verify the key is now trashed
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);
        assert_eq!(value.metadata.trashed_at, Some(gc_time));
    }

    #[test]
    fn test_gc_purges_expired() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("test");

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        // Run GC after purge TTL has passed
        let gc_time = trash_time + Duration::from_secs(60);
        let result = db.gc(gc_time).unwrap();

        assert!(result.trashed.is_empty());
        assert_eq!(result.purged.len(), 1);
        assert_eq!(result.purged[0].to_string(), "test");

        // Verify the key no longer exists
        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_gc_full_lifecycle() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("lifecycle-test");

        // Create
        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Phase 1: Before trash TTL - nothing to do
        let t1 = create_time + Duration::from_secs(50);
        let result1 = db.gc(t1).unwrap();
        assert!(result1.is_empty());

        // Phase 2: After trash TTL - should be trashed
        let t2 = create_time + Duration::from_secs(150);
        let result2 = db.gc(t2).unwrap();
        assert_eq!(result2.trashed.len(), 1);
        assert!(result2.purged.is_empty());

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);

        // Phase 3: After purge TTL - should be purged
        let t3 = t2 + Duration::from_secs(60);
        let result3 = db.gc(t3).unwrap();
        assert!(result3.trashed.is_empty());
        assert_eq!(result3.purged.len(), 1);

        // Key should be gone
        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_touch_resets_trash_timer() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("touch-test");

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Touch at t=80 (before trash TTL of 100)
        let touch_time = create_time + Duration::from_secs(80);
        db.touch(&key, touch_time).unwrap();

        // GC at t=150 (would be expired based on create_time, but not based on touch_time)
        let check_time = create_time + Duration::from_secs(150);
        let result = db.gc(check_time).unwrap();
        assert!(result.trashed.is_empty()); // Touch reset the timer

        // GC at t=200 (now past touch_time + trash_ttl)
        let check_time2 = touch_time + Duration::from_secs(110);
        let result2 = db.gc(check_time2).unwrap();
        assert_eq!(result2.trashed.len(), 1);
    }
}

mod edge_cases {
    use super::common::{create_test_db, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::{FileData, InlineFileData, TextData};
    use std::time::SystemTime;

    #[test]
    fn test_insert_blob_stored_text() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("blob-text");
        let now = SystemTime::now();

        db.insert(&key, now, ClipData::Text(TextData::BlobStored))
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.clip_data, ClipData::Text(TextData::BlobStored));
    }

    #[test]
    fn test_multiple_files() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("multi-file");
        let now = SystemTime::now();

        let files = vec![
            FileData::Inlined(InlineFileData {
                file_name: "file1.txt".to_string(),
                data: b"content1".to_vec(),
            }),
            FileData::Inlined(InlineFileData {
                file_name: "file2.txt".to_string(),
                data: b"content2".to_vec(),
            }),
        ];

        db.insert(&key, now, ClipData::Files(files)).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 2);
            }
            ClipData::Text(_) => panic!("Expected Files variant"),
        }
    }

    #[test]
    fn test_hierarchical_keys() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        // Keva supports hierarchical keys (though not implicit parents)
        let keys = [
            "project/config/theme",
            "project/config/language",
            "project/data",
            "other/key",
        ];

        for key_str in &keys {
            let key = make_key(key_str);
            db.insert(
                &key,
                now,
                ClipData::Text(TextData::Inlined(key_str.to_string())),
            )
            .unwrap();
        }

        let all_keys = db.keys().unwrap();
        assert_eq!(all_keys.len(), 4);

        // Each key should be independently accessible
        for key_str in &keys {
            let key = make_key(key_str);
            let value = db.get(&key).unwrap().unwrap();
            assert_eq!(
                value.clip_data,
                ClipData::Text(TextData::Inlined(key_str.to_string()))
            );
        }
    }
}
