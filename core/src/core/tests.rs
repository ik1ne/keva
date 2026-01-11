use super::*;
use crate::core::file_storage::FileStorage;
use crate::types::value::LifecycleState;
use common::*;
use std::io::Write;
use std::time::Duration;
use tempfile::TempDir;

mod common {
    use super::*;

    pub(super) fn create_test_storage() -> (KevaCore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
        };

        let storage = KevaCore::open(config).unwrap();

        (storage, temp_dir)
    }

    pub(super) fn make_gc_config(trash_ttl_secs: u64, purge_ttl_secs: u64) -> GcConfig {
        GcConfig {
            trash_ttl: Duration::from_secs(trash_ttl_secs),
            purge_ttl: Duration::from_secs(purge_ttl_secs),
        }
    }

    pub(super) fn make_key(s: &str) -> Key {
        Key::try_from(s).unwrap()
    }

    pub(super) fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }
}

mod create {
    use super::*;

    #[test]
    fn test_create_new_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata.lifecycle_state,
            LifecycleState::Active { last_accessed: now }
        );
        assert!(value.attachments.is_empty());
    }

    #[test]
    fn test_create_creates_content_file() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let content_path = storage.content_path(&key);
        assert!(content_path.exists());

        // Content should be empty
        let content = std::fs::read_to_string(&content_path).unwrap();
        assert_eq!(content, "");

        // Path should be in content directory
        assert!(content_path.starts_with(temp.path().join("content")));
    }

    #[test]
    fn test_create_existing_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let result = storage.create(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::AlreadyExists))
        ));
    }

    #[test]
    fn test_create_trashed_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let result = storage.create(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::AlreadyExists))
        ));
    }
}

mod get {
    use super::*;

    #[test]
    fn test_get_active_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            value.metadata.lifecycle_state,
            LifecycleState::Active { .. }
        ));
    }

    #[test]
    fn test_get_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            value.metadata.lifecycle_state,
            LifecycleState::Trash { .. }
        ));
    }

    #[test]
    fn test_get_nonexistent_key() {
        let (storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");

        let result = storage.get(&key).unwrap();
        assert!(result.is_none());
    }
}

mod touch {
    use super::*;

    #[test]
    fn test_touch_updates_last_accessed() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let later = now + Duration::from_secs(10);
        storage.touch(&key, later).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            value.metadata.lifecycle_state,
            LifecycleState::Active {
                last_accessed: later
            }
        );
    }

    #[test]
    fn test_touch_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();

        let result = storage.touch(&make_key("missing"), now);
        assert!(matches!(
            result,
            Err(KevaError::Database(db::error::DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_touch_trashed_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let result = storage.touch(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(db::error::DatabaseError::Trashed))
        ));
    }
}

mod add_attachments {
    use super::*;

    #[test]
    fn test_add_single_attachment() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "test.txt", b"file content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "test.txt");
        assert_eq!(value.attachments[0].size, 12); // "file content".len()
    }

    #[test]
    fn test_add_multiple_attachments() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file1 = create_test_file(&temp, "file1.txt", b"content1");
        let file2 = create_test_file(&temp, "file2.txt", b"content2");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(
                &key,
                vec![(file1, "file1.txt".into()), (file2, "file2.txt".into())],
                now,
            )
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 2);
    }

    #[test]
    fn test_add_attachment_to_nonexistent_key_fails() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("nonexistent");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        let result = storage.add_attachments(&key, vec![(file_path, "test.txt".into())], now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_add_attachment_with_custom_target_name() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "original.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "renamed.txt".into())], now)
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "renamed.txt");
    }

    #[test]
    fn test_add_attachment_overwrites_existing() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let file1 = create_test_file(&temp, "first.txt", b"first");
        storage
            .add_attachments(&key, vec![(file1, "same.txt".into())], now)
            .unwrap();

        let file2 = create_test_file(&temp, "second.txt", b"second content");
        storage
            .add_attachments(&key, vec![(file2, "same.txt".into())], now)
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "same.txt");
        assert_eq!(value.attachments[0].size, 14); // "second content".len()
    }

    #[test]
    fn test_attachment_file_exists_on_disk() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "test.txt", b"file content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let attachment_path = storage.attachment_path(&key, "test.txt");
        assert!(attachment_path.exists());
        assert_eq!(
            std::fs::read_to_string(&attachment_path).unwrap(),
            "file content"
        );
    }
}

mod remove_attachment {
    use super::*;

    #[test]
    fn test_remove_attachment() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let later = now + Duration::from_secs(1);
        storage.remove_attachment(&key, "test.txt", later).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert!(value.attachments.is_empty());
    }

    #[test]
    fn test_remove_attachment_removes_file_from_disk() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let attachment_path = storage.attachment_path(&key, "test.txt");
        assert!(attachment_path.exists());

        storage.remove_attachment(&key, "test.txt", now).unwrap();
        assert!(!attachment_path.exists());
    }

    #[test]
    fn test_remove_nonexistent_attachment_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let result = storage.remove_attachment(&key, "nonexistent.txt", now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::AttachmentNotFound(_)))
        ));
    }
}

mod rename_attachment {
    use super::*;

    #[test]
    fn test_rename_attachment() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "old.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "old.txt".into())], now)
            .unwrap();

        storage
            .rename_attachment(&key, "old.txt", "new.txt", now)
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "new.txt");

        // Old path should not exist, new path should
        let old_path = storage.attachment_path(&key, "old.txt");
        let new_path = storage.attachment_path(&key, "new.txt");
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    #[test]
    fn test_rename_nonexistent_attachment_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let result = storage.rename_attachment(&key, "nonexistent.txt", "new.txt", now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::AttachmentNotFound(_)))
        ));
    }

    #[test]
    fn test_rename_to_existing_fails() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_a = create_test_file(&temp, "a.txt", b"content A");
        let file_b = create_test_file(&temp, "b.txt", b"content B");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(
                &key,
                vec![(file_a, "a.txt".into()), (file_b, "b.txt".into())],
                now,
            )
            .unwrap();

        // Rename a.txt -> b.txt should fail (destination exists)
        let result = storage.rename_attachment(&key, "a.txt", "b.txt", now);
        assert!(matches!(result, Err(KevaError::DestinationExists)));

        // Both files should still exist unchanged
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 2);
    }

    #[test]
    fn test_rename_to_same_name_is_noop() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "a.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "a.txt".into())], now)
            .unwrap();

        storage
            .rename_attachment(&key, "a.txt", "a.txt", now)
            .unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.attachments.len(), 1);
        assert_eq!(value.attachments[0].filename, "a.txt");
    }
}

mod trash {
    use super::*;

    #[test]
    fn test_trash_makes_key_trashed() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            value.metadata.lifecycle_state,
            LifecycleState::Trash { .. }
        ));
    }

    #[test]
    fn test_trash_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");
        let now = SystemTime::now();

        let result = storage.trash(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(db::error::DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_trash_already_trashed_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let result = storage.trash(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::Trashed))
        ));
    }
}

mod rename {
    use super::*;

    #[test]
    fn test_rename_key() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.create(&old_key, now).unwrap();
        storage.rename(&old_key, &new_key, now).unwrap();

        assert!(storage.get(&old_key).unwrap().is_none());
        assert!(storage.get(&new_key).unwrap().is_some());
    }

    #[test]
    fn test_rename_moves_content_file() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.create(&old_key, now).unwrap();

        let old_content_path = storage.content_path(&old_key);

        // Write some content
        std::fs::write(&old_content_path, "test content").unwrap();

        storage.rename(&old_key, &new_key, now).unwrap();

        assert!(!old_content_path.exists());

        let new_content_path = storage.content_path(&new_key);
        assert!(new_content_path.exists());
        assert_eq!(
            std::fs::read_to_string(&new_content_path).unwrap(),
            "test content"
        );
    }

    #[test]
    fn test_rename_moves_attachments() {
        let (mut storage, temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let file_path = create_test_file(&temp, "test.txt", b"attachment");
        let now = SystemTime::now();

        storage.create(&old_key, now).unwrap();
        storage
            .add_attachments(&old_key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let old_attachment_path = storage.attachment_path(&old_key, "test.txt");
        assert!(old_attachment_path.exists());

        storage.rename(&old_key, &new_key, now).unwrap();

        assert!(!old_attachment_path.exists());
        let new_attachment_path = storage.attachment_path(&new_key, "test.txt");
        assert!(new_attachment_path.exists());
    }

    #[test]
    fn test_rename_to_same_key_is_noop() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("same/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.rename(&key, &key, now).unwrap();

        assert!(storage.get(&key).unwrap().is_some());
    }

    #[test]
    fn test_rename_fails_if_destination_exists() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.create(&old_key, now).unwrap();
        storage.create(&new_key, now).unwrap();

        let result = storage.rename(&old_key, &new_key, now);
        assert!(matches!(result, Err(KevaError::DestinationExists)));
    }
}

mod keys {
    use super::*;

    #[test]
    fn test_active_keys_returns_only_active() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();

        storage.create(&make_key("active1"), now).unwrap();
        storage.create(&make_key("active2"), now).unwrap();
        storage.create(&make_key("trashed"), now).unwrap();
        storage.trash(&make_key("trashed"), now).unwrap();

        let keys = storage.active_keys().unwrap();
        assert_eq!(keys.len(), 2);

        let key_strings: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
        assert!(key_strings.contains(&"active1".to_string()));
        assert!(key_strings.contains(&"active2".to_string()));
        assert!(!key_strings.contains(&"trashed".to_string()));
    }

    #[test]
    fn test_trashed_keys_returns_only_trashed() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();

        storage.create(&make_key("active"), now).unwrap();
        storage.create(&make_key("trashed1"), now).unwrap();
        storage.create(&make_key("trashed2"), now).unwrap();
        storage.trash(&make_key("trashed1"), now).unwrap();
        storage.trash(&make_key("trashed2"), now).unwrap();

        let keys = storage.trashed_keys().unwrap();
        assert_eq!(keys.len(), 2);

        let key_strings: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
        assert!(key_strings.contains(&"trashed1".to_string()));
        assert!(key_strings.contains(&"trashed2".to_string()));
        assert!(!key_strings.contains(&"active".to_string()));
    }
}

mod restore {
    use super::*;

    #[test]
    fn test_restore_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            value.metadata.lifecycle_state,
            LifecycleState::Trash { .. }
        ));

        storage.restore(&key, now).unwrap();

        let restored = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            restored.metadata.lifecycle_state,
            LifecycleState::Active { .. }
        ));
    }

    #[test]
    fn test_restore_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");
        let now = SystemTime::now();

        let result = storage.restore(&key, now);
        assert!(matches!(
            result,
            Err(KevaError::Database(db::error::DatabaseError::NotFound))
        ));
    }
}

mod purge {
    use super::*;

    #[test]
    fn test_purge_active_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.purge(&key).unwrap();

        assert!(storage.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();
        storage.purge(&key).unwrap();

        assert!(storage.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_removes_content_file() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let content_path = storage.content_path(&key);
        assert!(content_path.exists());

        storage.purge(&key).unwrap();
        assert!(!content_path.exists());
    }

    #[test]
    fn test_purge_removes_attachments() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let attachment_path = storage.attachment_path(&key, "test.txt");
        assert!(attachment_path.exists());

        storage.purge(&key).unwrap();
        assert!(!attachment_path.exists());
    }

    #[test]
    fn test_purge_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");

        let result = storage.purge(&key);
        assert!(matches!(
            result,
            Err(KevaError::Database(DatabaseError::NotFound))
        ));
    }
}

mod maintenance {
    use super::*;

    #[test]
    fn test_maintenance_purges_expired_trash_keys() {
        let (mut storage, _temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage.trash(&key, now).unwrap();

        let after_ttl = now + Duration::from_secs(6);
        let result = storage.maintenance(after_ttl, gc_config).unwrap();

        assert!(result.keys_purged.contains(&key));
        assert!(result.keys_trashed.is_empty());
    }

    #[test]
    fn test_maintenance_trashes_expired_active_keys() {
        let (mut storage, _temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let after_ttl = now + Duration::from_secs(11);
        let result = storage.maintenance(after_ttl, gc_config).unwrap();

        assert!(result.keys_trashed.contains(&key));
        assert!(result.keys_purged.is_empty());

        let keys = storage.trashed_keys().unwrap();
        assert!(keys.contains(&key));
    }

    #[test]
    fn test_maintenance_cleans_up_files() {
        let (mut storage, temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let content_path = storage.content_path(&key);
        let attachment_path = storage.attachment_path(&key, "test.txt");
        assert!(content_path.exists());
        assert!(attachment_path.exists());

        storage.trash(&key, now).unwrap();
        let after_ttl = now + Duration::from_secs(6);
        storage.maintenance(after_ttl, gc_config).unwrap();

        assert!(!content_path.exists());
        assert!(!attachment_path.exists());
    }

    #[test]
    fn test_maintenance_with_no_expired_keys() {
        let (mut storage, _temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let result = storage.maintenance(now, gc_config).unwrap();

        assert!(result.keys_trashed.is_empty());
        assert!(result.keys_purged.is_empty());

        let value = storage.get(&key).unwrap().unwrap();
        assert!(matches!(
            value.metadata.lifecycle_state,
            LifecycleState::Active { .. }
        ));
    }

    #[test]
    fn test_maintenance_cleans_orphan_blobs() {
        let (mut storage, temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(file_path, "test.txt".into())], now)
            .unwrap();

        let key_hash = KevaCore::key_to_path(&key);
        let blob_dir = temp.path().join("blobs").join(&key_hash);
        assert!(blob_dir.exists());

        // Simulate orphan by directly purging from DB without file cleanup
        storage.db.purge(&key).unwrap();

        // Blob still exists (orphaned)
        assert!(blob_dir.exists());

        // Maintenance should clean it up
        let result = storage.maintenance(now, gc_config).unwrap();
        assert!(!blob_dir.exists());
        assert!(result.orphaned_files_removed > 0);
    }

    #[test]
    fn test_maintenance_cleans_orphan_content() {
        let (mut storage, temp) = create_test_storage();
        let gc_config = make_gc_config(10, 5);
        let key = make_key("key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let key_hash = KevaCore::key_to_path(&key);
        let content_file = temp
            .path()
            .join("content")
            .join(format!("{}.md", key_hash.display()));
        assert!(content_file.exists());

        // Simulate orphan by directly purging from DB without file cleanup
        storage.db.purge(&key).unwrap();

        // Content still exists (orphaned)
        assert!(content_file.exists());

        // Maintenance should clean it up
        let result = storage.maintenance(now, gc_config).unwrap();
        assert!(!content_file.exists());
        assert!(result.orphaned_files_removed > 0);
    }
}

mod thumbnail {
    use super::*;

    #[test]
    fn test_thumbnail_paths_excludes_unsupported_formats() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/key");
        let pdf_path = create_test_file(&temp, "document.pdf", b"pdf content");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();
        storage
            .add_attachments(&key, vec![(pdf_path, "document.pdf".into())], now)
            .unwrap();

        let paths = storage.thumbnail_paths(&key).unwrap();
        // PDF is not a supported image format, so no thumbnail
        assert!(paths.is_empty());
    }

    #[test]
    fn test_thumbnail_paths_empty_for_no_attachments() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        let paths = storage.thumbnail_paths(&key).unwrap();
        assert!(paths.is_empty());
    }

    #[test]
    fn test_new_key_starts_with_current_thumb_version() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.create(&key, now).unwrap();

        // New keys start with current THUMB_VER (no regeneration needed)
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.thumb_version, FileStorage::THUMB_VER);
    }
}
