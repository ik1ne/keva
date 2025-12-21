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
                inline_threshold_bytes: 1024 * 1024,               // 1MB
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

mod insert {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ACTIVE_EXPIRY, ClipData};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{
        FileData, InlineFileData, LifecycleState, Metadata, TextData,
    };
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_insert_text() {
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

        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        )
    }

    #[test]
    fn test_insert_files() {
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
                assert_eq!(files, &[file_data]);
            }
            ClipData::Text(t) => panic!("Expected Files variant, got: {t:?}"),
        }
    }

    #[test]
    fn test_insert_registers_in_ttl_table() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/ttl");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Verify TTL table: key should be in ACTIVE_EXPIRY for auto-trash scheduling
        let write_txn = db.db.begin_write().unwrap();
        let ttl_key = TtlKey {
            timestamp: now,
            key: key.clone(),
        };
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &ttl_key).unwrap());
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

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: insert_time,
                updated_at: insert_time,
                last_accessed: insert_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
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

        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        let insert_time = now + Duration::from_secs(100);
        db.insert(
            &key,
            insert_time,
            ClipData::Text(TextData::Inlined("second".to_string())),
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: insert_time,
                updated_at: insert_time,
                last_accessed: insert_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
    }

    #[test]
    fn test_insert_after_purge() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/overwrite");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("first".to_string())),
        )
        .unwrap();

        db.purge(&key).unwrap();

        let update_time = create_time + Duration::from_secs(100);
        db.insert(
            &key,
            update_time,
            ClipData::Text(TextData::Inlined("second".to_string())),
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: update_time,
                updated_at: update_time,
                last_accessed: update_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
    }
}

mod get {
    use super::common::{create_test_db, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_get_existing_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_get_nonexistent_key() {
        let (db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.get(&key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: Some(trash_time),
                lifecycle_state: LifecycleState::Trash,
            }
        );
    }
}

mod update {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_update_text() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/update");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("original".to_string())),
        )
        .unwrap();

        let update_time = create_time + Duration::from_secs(50);
        db.update(
            &key,
            update_time,
            ClipData::Text(TextData::Inlined("updated".to_string())),
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("updated".to_string()))
        );
        // created_at should be preserved, updated_at and last_accessed should reflect update time
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: update_time,
                last_accessed: update_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_update_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.update(
            &key,
            SystemTime::now(),
            ClipData::Text(TextData::Inlined("text".to_string())),
        );
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_update_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("original".to_string())),
        )
        .unwrap();

        db.trash(&key, now + Duration::from_secs(10)).unwrap();

        let result = db.update(
            &key,
            now + Duration::from_secs(20),
            ClipData::Text(TextData::Inlined("updated".to_string())),
        );
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod touch {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_touch_updates_last_accessed() {
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
        // touch() only updates last_accessed, not updated_at
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                last_accessed: touch_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
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
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod append_files {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{
        FileData, InlineFileData, LifecycleState, Metadata, TextData,
    };
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_append_files_to_existing_files() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/append");
        let create_time = SystemTime::now();

        let file1 = FileData::Inlined(InlineFileData {
            file_name: "file1.txt".to_string(),
            data: b"content1".to_vec(),
        });
        let file2 = FileData::Inlined(InlineFileData {
            file_name: "file2.txt".to_string(),
            data: b"content2".to_vec(),
        });

        // Insert initial value with one file
        db.insert(&key, create_time, ClipData::Files(vec![file1.clone()]))
            .unwrap();

        // Append another file
        let append_time = create_time + Duration::from_secs(50);
        db.append_files(&key, append_time, vec![file2.clone()])
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();

        // Should have 2 files now
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files, &[file1, file2]);
            }
            ClipData::Text(t) => panic!("Expected Files variant, got: {t:?}"),
        }

        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: append_time,
                last_accessed: append_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
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
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }

    #[test]
    fn test_append_empty_files_returns_error() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/empty-append");
        let now = SystemTime::now();

        let file = FileData::Inlined(InlineFileData {
            file_name: "file.txt".to_string(),
            data: b"content".to_vec(),
        });

        db.insert(&key, now, ClipData::Files(vec![file.clone()]))
            .unwrap();

        let result = db.append_files(&key, now + Duration::from_secs(50), vec![]);
        assert!(matches!(result, Err(DatabaseError::EmptyInput)));
    }
}

mod remove_file_at {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ClipData, DatabaseError};
    use crate::types::value::versioned_value::latest_value::{
        FileData, InlineFileData, LifecycleState, Metadata, TextData,
    };
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_remove_file_at_removes_entry() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/remove_file_at");
        let now = SystemTime::now();

        let file1 = FileData::Inlined(InlineFileData {
            file_name: "file1.txt".to_string(),
            data: b"content1".to_vec(),
        });
        let file2 = FileData::Inlined(InlineFileData {
            file_name: "file2.txt".to_string(),
            data: b"content2".to_vec(),
        });

        db.insert(
            &key,
            now,
            ClipData::Files(vec![file1.clone(), file2.clone()]),
        )
        .unwrap();

        let remove_time = now + Duration::from_secs(10);
        let removed = db.remove_file_at(&key, remove_time, 0).unwrap();

        assert_eq!(removed, file1);

        let value = db.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files, &[file2]);
            }
            ClipData::Text(t) => panic!("Expected Files variant, got: {t:?}"),
        }
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: remove_time,
                last_accessed: remove_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_remove_file_at_last_file_becomes_empty_text() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/remove_last");
        let now = SystemTime::now();

        let file = FileData::Inlined(InlineFileData {
            file_name: "only_file.txt".to_string(),
            data: b"content".to_vec(),
        });

        db.insert(&key, now, ClipData::Files(vec![file])).unwrap();

        let remove_time = now + Duration::from_secs(10);
        db.remove_file_at(&key, remove_time, 0).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined(String::new()))
        );
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: remove_time,
                last_accessed: remove_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_remove_file_at_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.remove_file_at(&key, SystemTime::now(), 0);
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_remove_file_at_trashed_key() {
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

        let result = db.remove_file_at(&key, now + Duration::from_secs(20), 0);
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }

    #[test]
    fn test_remove_file_at_on_text_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/text");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("text content".to_string())),
        )
        .unwrap();

        let result = db.remove_file_at(&key, now, 0);
        assert!(matches!(result, Err(DatabaseError::TypeMismatch)));
    }

    #[test]
    fn test_remove_file_at_out_of_bounds() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/oob");
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

        let result = db.remove_file_at(&key, now, 5);
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }
}

mod rename {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ACTIVE_EXPIRY, ClipData, DatabaseError, TRASH_EXPIRY};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_rename_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/key");
        let dst = make_key("dst/key");
        let now = SystemTime::now();

        db.insert(
            &src,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.rename(&src, &dst).unwrap();

        // Old key should not exist
        assert!(db.get(&src).unwrap().is_none());

        // New key should have the value with preserved metadata
        let value = db.get(&dst).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_rename_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/trashed");
        let dst = make_key("dst/trashed");
        let now = SystemTime::now();

        db.insert(
            &src,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&src, trash_time).unwrap();

        db.rename(&src, &dst).unwrap();

        let value = db.get(&dst).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: Some(trash_time),
                lifecycle_state: LifecycleState::Trash,
            }
        );
    }

    #[test]
    fn test_rename_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("nonexistent");
        let dst = make_key("dst/key");

        let result = db.rename(&src, &dst);
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
    fn test_rename_overwrites_trashed_dst() {
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

        // key2 should have key1's content and metadata
        let value = db.get(&key2).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("source content".to_string()))
        );
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
    }

    #[test]
    fn test_rename_transfers_active_ttl_entry() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/key");
        let dst = make_key("dst/key");
        let now = SystemTime::now();

        db.insert(
            &src,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        db.rename(&src, &dst).unwrap();

        // Verify TTL entry transferred: src removed, dst added with same timestamp
        let write_txn = db.db.begin_write().unwrap();
        let src_ttl = TtlKey {
            timestamp: now,
            key: src.clone(),
        };
        let dst_ttl = TtlKey {
            timestamp: now,
            key: dst.clone(),
        };
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &src_ttl).unwrap());
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &dst_ttl).unwrap());
    }

    #[test]
    fn test_rename_transfers_trash_ttl_entry() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/trashed");
        let dst = make_key("dst/trashed");
        let now = SystemTime::now();

        db.insert(
            &src,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&src, trash_time).unwrap();

        db.rename(&src, &dst).unwrap();

        // Verify TTL entry transferred: src removed, dst added with same timestamp
        let write_txn = db.db.begin_write().unwrap();
        let src_ttl = TtlKey {
            timestamp: trash_time,
            key: src.clone(),
        };
        let dst_ttl = TtlKey {
            timestamp: trash_time,
            key: dst.clone(),
        };
        assert!(!TRASH_EXPIRY.remove(&write_txn, &src_ttl).unwrap());
        assert!(TRASH_EXPIRY.remove(&write_txn, &dst_ttl).unwrap());
    }
}

mod trash {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ACTIVE_EXPIRY, ClipData, DatabaseError, TRASH_EXPIRY};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
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

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                last_accessed: create_time,
                trashed_at: Some(trash_time),
                lifecycle_state: LifecycleState::Trash,
            }
        );

        // Verify TTL tables: key should be moved from ACTIVE_EXPIRY to TRASH_EXPIRY
        let write_txn = db.db.begin_write().unwrap();
        let old_ttl_key = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl_key = TtlKey {
            timestamp: trash_time,
            key: key.clone(),
        };
        // Key should no longer be in ACTIVE_EXPIRY (remove returns false)
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &old_ttl_key).unwrap());
        // Key should be in TRASH_EXPIRY (remove returns true)
        assert!(TRASH_EXPIRY.remove(&write_txn, &new_ttl_key).unwrap());
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
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod restore {
    use super::common::{create_test_db, make_key};
    use crate::core::db::{ACTIVE_EXPIRY, ClipData, DatabaseError};
    use crate::types::TtlKey;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_restore_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/restore");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let restore_time = create_time + Duration::from_secs(20);
        db.restore(&key, restore_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: restore_time,
                last_accessed: restore_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );

        // Verify TTL table: key should be back in ACTIVE_EXPIRY
        let write_txn = db.db.begin_write().unwrap();
        let ttl_key = TtlKey {
            timestamp: restore_time,
            key: key.clone(),
        };
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &ttl_key).unwrap());
    }

    #[test]
    fn test_restore_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.restore(&key, SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_restore_active_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/active");
        let now = SystemTime::now();

        db.insert(
            &key,
            now,
            ClipData::Text(TextData::Inlined("content".to_string())),
        )
        .unwrap();

        // Trying to restore an active key should fail
        let result = db.restore(&key, now + Duration::from_secs(10));
        assert!(matches!(result, Err(DatabaseError::NotTrashed)));
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

mod gc {
    use super::common::{create_test_db, create_test_db_with_ttl, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::{LifecycleState, Metadata, TextData};
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

        assert_eq!(result.trashed, std::slice::from_ref(&key));
        assert!(result.purged.is_empty());

        // Verify the key is now trashed
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                last_accessed: create_time,
                trashed_at: Some(gc_time),
                lifecycle_state: LifecycleState::Trash,
            }
        );
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
        assert_eq!(result.purged, std::slice::from_ref(&key));

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
        assert_eq!(result2.trashed, std::slice::from_ref(&key));
        assert!(result2.purged.is_empty());

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                last_accessed: create_time,
                trashed_at: Some(t2),
                lifecycle_state: LifecycleState::Trash,
            }
        );

        // Phase 3: After purge TTL - should be purged
        let t3 = t2 + Duration::from_secs(60);
        let result3 = db.gc(t3).unwrap();
        assert!(result3.trashed.is_empty());
        assert_eq!(result3.purged, std::slice::from_ref(&key));

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
    use super::common::{create_test_db, create_test_db_with_ttl, make_key};
    use crate::core::db::ClipData;
    use crate::types::value::versioned_value::latest_value::{
        FileData, InlineFileData, LifecycleState, Metadata, TextData,
    };
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_insert_blob_stored_text() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("blob-text");
        let now = SystemTime::now();

        db.insert(&key, now, ClipData::Text(TextData::BlobStored))
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.clip_data, ClipData::Text(TextData::BlobStored));
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                last_accessed: now,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );
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

        db.insert(&key, now, ClipData::Files(files.clone()))
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(result_files) => {
                assert_eq!(result_files, &files);
            }
            ClipData::Text(t) => panic!("Expected Files variant, got: {t:?}"),
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

    /// Stale active keys (TTL expired but GC hasn't run) can still be touched.
    ///
    /// This is intentional: GC is the point of no return, not TTL expiration.
    /// The window between TTL expiration and GC serves as a grace period
    /// where users can rescue their data.
    #[test]
    fn test_stale_active_key_can_be_rescued_by_touch() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);
        let key = make_key("rescue-me");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("important data".to_string())),
        )
        .unwrap();

        // Time passes beyond trash TTL (100s), but GC hasn't run
        let stale_time = create_time + Duration::from_secs(150);

        // Touch should succeed, rescuing the key from future GC
        db.touch(&key, stale_time).unwrap();

        // Verify the key is still active with updated last_accessed
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                last_accessed: stale_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );

        // GC at stale_time should not trash it (TTL reset by touch)
        let result = db.gc(stale_time).unwrap();
        assert!(result.trashed.is_empty());
    }

    /// Stale trashed keys (purge TTL expired but GC hasn't run) can still be restored.
    ///
    /// This is intentional: GC is the point of no return, not TTL expiration.
    /// The window between TTL expiration and GC serves as a grace period
    /// where users can rescue their data.
    #[test]
    fn test_stale_trashed_key_can_be_rescued_by_restore() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);
        let key = make_key("rescue-me");
        let create_time = SystemTime::now();

        db.insert(
            &key,
            create_time,
            ClipData::Text(TextData::Inlined("important data".to_string())),
        )
        .unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        // Time passes beyond purge TTL (50s from trash_time), but GC hasn't run
        let stale_time = trash_time + Duration::from_secs(60);

        // Restore should succeed, rescuing the key from future GC
        db.restore(&key, stale_time).unwrap();

        // Verify the key is active again
        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: stale_time,
                last_accessed: stale_time,
                trashed_at: None,
                lifecycle_state: LifecycleState::Active,
            }
        );

        // GC at stale_time should not trash or purge it
        let result = db.gc(stale_time).unwrap();
        assert!(result.is_empty());
    }
}
