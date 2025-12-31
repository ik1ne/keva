use super::*;
use crate::core::KevaCore;
use crate::types::TtlKey;
use crate::types::config::SavedConfig;
use common::{create_test_db, make_key};
use std::time::Duration;
use tempfile::TempDir;

mod common {
    use super::*;

    pub(super) fn create_test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
                purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
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
            },
        };
        let db = Database::new(config).unwrap();
        (db, temp_dir)
    }

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_from(s).unwrap()
    }
}

mod create {
    use super::*;

    #[test]
    fn test_create_new_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert!(value.attachments.is_empty());
        assert_eq!(value.thumb_version, KevaCore::THUMB_VER);
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                lifecycle_state: LifecycleState::Active { last_accessed: now },
            }
        )
    }

    #[test]
    fn test_create_registers_in_ttl_table() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/ttl");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let write_txn = db.db.begin_write().unwrap();
        let ttl_key = TtlKey {
            timestamp: now,
            key: key.clone(),
        };
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &ttl_key).unwrap());
    }

    #[test]
    fn test_create_existing_key_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let result = db.create(&key, now);
        assert!(matches!(result, Err(DatabaseError::AlreadyExists)));
    }

    #[test]
    fn test_create_trashed_key_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.trash(&key, now).unwrap();

        let result = db.create(&key, now);
        assert!(matches!(result, Err(DatabaseError::AlreadyExists)));
    }
}

mod get {
    use super::*;

    #[test]
    fn test_get_existing_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert!(value.attachments.is_empty());
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                lifecycle_state: LifecycleState::Active { last_accessed: now },
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

        db.create(&key, now).unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                lifecycle_state: LifecycleState::Trash {
                    trashed_at: trash_time
                },
            }
        );
    }
}

mod touch {
    use super::*;

    #[test]
    fn test_touch_updates_last_accessed() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/touch");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let touch_time = create_time + Duration::from_secs(50);
        db.touch(&key, touch_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: touch_time
                },
            }
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

        db.create(&key, now).unwrap();
        db.trash(&key, now).unwrap();

        let result = db.touch(&key, now);
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod mark_content_modified {
    use super::*;

    #[test]
    fn test_mark_content_modified_updates_timestamps() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let modify_time = create_time + Duration::from_secs(50);
        db.mark_content_modified(&key, modify_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: modify_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: modify_time
                },
            }
        );
    }

    #[test]
    fn test_mark_content_modified_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.mark_content_modified(&key, SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_mark_content_modified_trashed_key_fails() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.trash(&key, now).unwrap();

        let result = db.mark_content_modified(&key, now);
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod add_attachment {
    use super::*;

    #[test]
    fn test_add_attachment() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let add_time = create_time + Duration::from_secs(10);
        db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            add_time,
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "test.txt");
        assert_eq!(value.attachments[0].size, 100);
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: add_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: add_time
                },
            }
        );
    }

    #[test]
    fn test_add_multiple_attachments() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "file1.txt".to_string(),
                size: 100,
            },
            now,
        )
        .unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "file2.txt".to_string(),
                size: 200,
            },
            now,
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 2);
    }

    #[test]
    fn test_add_attachment_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            SystemTime::now(),
        );
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_add_attachment_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.trash(&key, now).unwrap();

        let result = db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            now,
        );
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }

    #[test]
    fn test_add_attachment_updates_ttl() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/ttl");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let add_time = create_time + Duration::from_secs(10);
        db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            add_time,
        )
        .unwrap();

        let write_txn = db.db.begin_write().unwrap();
        let old_ttl = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl = TtlKey {
            timestamp: add_time,
            key: key.clone(),
        };
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &old_ttl).unwrap());
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &new_ttl).unwrap());
    }
}

mod remove_attachment {
    use super::*;

    #[test]
    fn test_remove_attachment() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            create_time,
        )
        .unwrap();

        let remove_time = create_time + Duration::from_secs(10);
        db.remove_attachment(&key, "test.txt", remove_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert!(value.attachments.is_empty());
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: remove_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: remove_time
                },
            }
        );
    }

    #[test]
    fn test_remove_nonexistent_attachment() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let result = db.remove_attachment(&key, "nonexistent.txt", now);
        assert!(matches!(result, Err(DatabaseError::AttachmentNotFound(_))));
    }

    #[test]
    fn test_remove_attachment_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.remove_attachment(&key, "test.txt", SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_remove_attachment_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trashed");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            now,
        )
        .unwrap();
        db.trash(&key, now).unwrap();

        let result = db.remove_attachment(&key, "test.txt", now);
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }

    #[test]
    fn test_remove_attachment_updates_ttl() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/ttl");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "test.txt".to_string(),
                size: 100,
            },
            create_time,
        )
        .unwrap();

        let remove_time = create_time + Duration::from_secs(10);
        db.remove_attachment(&key, "test.txt", remove_time).unwrap();

        let write_txn = db.db.begin_write().unwrap();
        let old_ttl = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl = TtlKey {
            timestamp: remove_time,
            key: key.clone(),
        };
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &old_ttl).unwrap());
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &new_ttl).unwrap());
    }
}

mod rename_attachment {
    use super::*;

    #[test]
    fn test_rename_attachment() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "old.txt".to_string(),
                size: 100,
            },
            create_time,
        )
        .unwrap();

        let rename_time = create_time + Duration::from_secs(10);
        db.rename_attachment(&key, "old.txt", "new.txt", rename_time)
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "new.txt");
        assert_eq!(value.attachments[0].size, 100);
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: rename_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: rename_time
                },
            }
        );
    }

    #[test]
    fn test_rename_nonexistent_attachment() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let result = db.rename_attachment(&key, "nonexistent.txt", "new.txt", now);
        assert!(matches!(result, Err(DatabaseError::AttachmentNotFound(_))));
    }

    #[test]
    fn test_rename_overwrites_existing() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/key");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "a.txt".to_string(),
                size: 100,
            },
            now,
        )
        .unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "b.txt".to_string(),
                size: 200,
            },
            now,
        )
        .unwrap();

        db.rename_attachment(&key, "a.txt", "b.txt", now).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "b.txt");
        assert_eq!(value.attachments[0].size, 100); // Source's size preserved
    }

    #[test]
    fn test_rename_attachment_updates_ttl() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/ttl");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "old.txt".to_string(),
                size: 100,
            },
            create_time,
        )
        .unwrap();

        let rename_time = create_time + Duration::from_secs(10);
        db.rename_attachment(&key, "old.txt", "new.txt", rename_time)
            .unwrap();

        let write_txn = db.db.begin_write().unwrap();
        let old_ttl = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl = TtlKey {
            timestamp: rename_time,
            key: key.clone(),
        };
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &old_ttl).unwrap());
        assert!(ACTIVE_EXPIRY.remove(&write_txn, &new_ttl).unwrap());
    }
}

mod rename {
    use super::*;

    #[test]
    fn test_rename_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/key");
        let dst = make_key("dst/key");
        let now = SystemTime::now();

        db.create(&src, now).unwrap();
        db.rename(&src, &dst, now).unwrap();

        assert!(db.get(&src).unwrap().is_none());

        let value = db.get(&dst).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                lifecycle_state: LifecycleState::Active { last_accessed: now },
            }
        );
    }

    #[test]
    fn test_rename_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/trashed");
        let dst = make_key("dst/trashed");
        let now = SystemTime::now();

        db.create(&src, now).unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&src, trash_time).unwrap();

        db.rename(&src, &dst, now).unwrap();

        let value = db.get(&dst).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: now,
                updated_at: now,
                lifecycle_state: LifecycleState::Trash {
                    trashed_at: trash_time
                },
            }
        );
    }

    #[test]
    fn test_rename_nonexistent_key() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("nonexistent");
        let dst = make_key("dst/key");

        let result = db.rename(&src, &dst, SystemTime::now());
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }

    #[test]
    fn test_rename_overwrites_existing_key() {
        let (mut db, _temp) = create_test_db();
        let key1 = make_key("key1");
        let key2 = make_key("key2");
        let now = SystemTime::now();

        db.create(&key1, now).unwrap();
        db.create(&key2, now).unwrap();

        db.rename(&key1, &key2, now).unwrap();

        assert!(db.get(&key1).unwrap().is_none());
        assert!(db.get(&key2).unwrap().is_some());
    }

    #[test]
    fn test_rename_transfers_active_ttl_entry() {
        let (mut db, _temp) = create_test_db();
        let src = make_key("src/key");
        let dst = make_key("dst/key");
        let now = SystemTime::now();

        db.create(&src, now).unwrap();
        db.rename(&src, &dst, now).unwrap();

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

        db.create(&src, now).unwrap();

        let trash_time = now + Duration::from_secs(10);
        db.trash(&src, trash_time).unwrap();

        db.rename(&src, &dst, now).unwrap();

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

mod update_thumb_version {
    use super::*;

    #[test]
    fn test_update_thumb_version() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/thumb");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.thumb_version, KevaCore::THUMB_VER);

        db.update_thumb_version(&key, KevaCore::THUMB_VER + 1)
            .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.thumb_version, KevaCore::THUMB_VER + 1);
    }

    #[test]
    fn test_update_thumb_version_not_found() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("nonexistent");

        let result = db.update_thumb_version(&key, 1);
        assert!(matches!(result, Err(DatabaseError::NotFound)));
    }
}

mod trash {
    use super::*;

    #[test]
    fn test_trash_sets_lifecycle_and_timestamp() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/trash");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let trash_time = create_time + Duration::from_secs(100);
        db.trash(&key, trash_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Trash {
                    trashed_at: trash_time
                },
            }
        );

        let write_txn = db.db.begin_write().unwrap();
        let old_ttl_key = TtlKey {
            timestamp: create_time,
            key: key.clone(),
        };
        let new_ttl_key = TtlKey {
            timestamp: trash_time,
            key: key.clone(),
        };
        assert!(!ACTIVE_EXPIRY.remove(&write_txn, &old_ttl_key).unwrap());
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

        db.create(&key, now).unwrap();
        db.trash(&key, now).unwrap();

        let result = db.trash(&key, now);
        assert!(matches!(result, Err(DatabaseError::Trashed)));
    }
}

mod restore {
    use super::*;

    #[test]
    fn test_restore_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/restore");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let restore_time = create_time + Duration::from_secs(20);
        db.restore(&key, restore_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        // Note: restore only updates last_accessed, not updated_at
        // (restore is not a content modification)
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: restore_time
                },
            }
        );

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

        db.create(&key, now).unwrap();

        let result = db.restore(&key, now);
        assert!(matches!(result, Err(DatabaseError::NotTrashed)));
    }
}

mod purge {
    use super::*;

    #[test]
    fn test_purge_removes_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/purge");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.purge(&key).unwrap();

        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_trashed_key() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("test/purge-trashed");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
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
    use super::*;
    use common::{create_test_db, create_test_db_with_ttl, make_key};

    #[test]
    fn test_gc_no_expired() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        db.create(&make_key("test"), now).unwrap();

        let result = db.gc(now).unwrap();
        assert!(result.trashed.is_empty());
        assert!(result.purged.is_empty());
    }

    #[test]
    fn test_gc_trashes_expired() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("test");

        db.create(&key, create_time).unwrap();

        let gc_time = create_time + Duration::from_secs(150);
        let result = db.gc(gc_time).unwrap();

        assert_eq!(result.trashed, std::slice::from_ref(&key));
        assert!(result.purged.is_empty());

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Trash {
                    trashed_at: gc_time
                },
            }
        );
    }

    #[test]
    fn test_gc_purges_expired() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("test");

        db.create(&key, create_time).unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let gc_time = trash_time + Duration::from_secs(60);
        let result = db.gc(gc_time).unwrap();

        assert!(result.trashed.is_empty());
        assert_eq!(result.purged, std::slice::from_ref(&key));

        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_gc_full_lifecycle() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("lifecycle-test");

        db.create(&key, create_time).unwrap();

        // Phase 1: Before trash TTL
        let t1 = create_time + Duration::from_secs(50);
        let result1 = db.gc(t1).unwrap();
        assert!(result1.trashed.is_empty());
        assert!(result1.purged.is_empty());

        // Phase 2: After trash TTL
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
                lifecycle_state: LifecycleState::Trash { trashed_at: t2 },
            }
        );

        // Phase 3: After purge TTL
        let t3 = t2 + Duration::from_secs(60);
        let result3 = db.gc(t3).unwrap();
        assert!(result3.trashed.is_empty());
        assert_eq!(result3.purged, std::slice::from_ref(&key));

        assert!(db.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_touch_resets_trash_timer() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);

        let create_time = SystemTime::now();
        let key = make_key("touch-test");

        db.create(&key, create_time).unwrap();

        let touch_time = create_time + Duration::from_secs(80);
        db.touch(&key, touch_time).unwrap();

        // Would be expired based on create_time, but not based on touch_time
        let check_time = create_time + Duration::from_secs(150);
        let result = db.gc(check_time).unwrap();
        assert!(result.trashed.is_empty());

        // Now past touch_time + trash_ttl
        let check_time2 = touch_time + Duration::from_secs(110);
        let result2 = db.gc(check_time2).unwrap();
        assert_eq!(result2.trashed.len(), 1);
    }
}

mod edge_cases {
    use super::*;
    use common::{create_test_db, create_test_db_with_ttl, make_key};

    #[test]
    fn test_multiple_attachments() {
        let (mut db, _temp) = create_test_db();
        let key = make_key("multi-attachment");
        let now = SystemTime::now();

        db.create(&key, now).unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "file1.txt".to_string(),
                size: 100,
            },
            now,
        )
        .unwrap();
        db.add_attachment(
            &key,
            Attachment {
                filename: "file2.txt".to_string(),
                size: 200,
            },
            now,
        )
        .unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 2);
        assert_eq!(value.attachments[0].filename, "file1.txt");
        assert_eq!(value.attachments[1].filename, "file2.txt");
    }

    #[test]
    fn test_hierarchical_keys() {
        let (mut db, _temp) = create_test_db();
        let now = SystemTime::now();

        let keys = [
            "project/config/theme",
            "project/config/language",
            "project/data",
            "other/key",
        ];

        for key_str in &keys {
            let key = make_key(key_str);
            db.create(&key, now).unwrap();
        }

        for key_str in &keys {
            let key = make_key(key_str);
            let value = db.get(&key).unwrap().unwrap();
            assert!(matches!(
                value.metadata.lifecycle_state,
                LifecycleState::Active { .. }
            ));
        }
    }

    /// Stale active keys (TTL expired but GC hasn't run) can still be touched.
    #[test]
    fn test_stale_active_key_can_be_rescued_by_touch() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);
        let key = make_key("rescue-me");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let stale_time = create_time + Duration::from_secs(150);
        db.touch(&key, stale_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: stale_time
                },
            }
        );

        let result = db.gc(stale_time).unwrap();
        assert!(result.trashed.is_empty());
    }

    /// Stale trashed keys (purge TTL expired but GC hasn't run) can still be restored.
    #[test]
    fn test_stale_trashed_key_can_be_rescued_by_restore() {
        let (mut db, _temp) = create_test_db_with_ttl(100, 50);
        let key = make_key("rescue-me");
        let create_time = SystemTime::now();

        db.create(&key, create_time).unwrap();

        let trash_time = create_time + Duration::from_secs(10);
        db.trash(&key, trash_time).unwrap();

        let stale_time = trash_time + Duration::from_secs(60);
        db.restore(&key, stale_time).unwrap();

        let value = db.get(&key).unwrap().unwrap();
        // Note: restore only updates last_accessed, not updated_at
        assert_eq!(
            value.metadata,
            Metadata {
                created_at: create_time,
                updated_at: create_time,
                lifecycle_state: LifecycleState::Active {
                    last_accessed: stale_time
                },
            }
        );

        let result = db.gc(stale_time).unwrap();
        assert!(result.trashed.is_empty());
        assert!(result.purged.is_empty());
    }
}
