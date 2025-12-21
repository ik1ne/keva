use super::*;
use crate::types::config::SavedConfig;
use std::time::Duration;
use tempfile::TempDir;

mod common {
    use super::*;
    use std::io::Write;

    pub(super) fn create_test_storage() -> (KevaCore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
                purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
                inline_threshold_bytes: 1024 * 1024,               // 1MB
            },
        };

        let storage = KevaCore::open(config).unwrap();

        (storage, temp_dir)
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

mod upsert_text {
    use super::common::{create_test_storage, make_key};
    use super::*;
    use crate::types::value::versioned_value::latest_value::TextData;

    #[test]
    fn test_upsert_creates_new_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "hello world", now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("hello world".to_string()))
        );
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
    }

    #[test]
    fn test_upsert_updates_existing_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "first", now).unwrap();
        let first_value = storage.get(&key).unwrap().unwrap();
        let created_at = first_value.metadata.created_at;

        // Small delay to ensure different timestamps
        std::thread::sleep(Duration::from_millis(10));
        let later = SystemTime::now();

        storage.upsert_text(&key, "second", later).unwrap();
        let second_value = storage.get(&key).unwrap().unwrap();

        // Content should be updated
        assert_eq!(
            second_value.clip_data,
            ClipData::Text(TextData::Inlined("second".to_string()))
        );
        // created_at should be preserved
        assert_eq!(second_value.metadata.created_at, created_at);
        // updated_at should be different
        assert!(second_value.metadata.updated_at > created_at);
    }

    #[test]
    fn test_upsert_fails_on_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "first", now).unwrap();
        storage.trash(&key, now).unwrap();

        // Key should be visible as Trash
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);

        // Upsert should fail - must restore first
        let result = storage.upsert_text(&key, "second", now);
        assert!(matches!(result, Err(StorageError::KeyIsTrashed)));
    }

    #[test]
    fn test_upsert_blob_text_then_inline_removes_old_blob_file() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/blob_to_inline");
        let now = SystemTime::now();

        // Force blob storage by exceeding the inline threshold.
        let large_text = "x".repeat(1024 * 1024 + 1);
        storage.upsert_text(&key, &large_text, now).unwrap();

        // Confirm it is blob-stored and the file exists on disk.
        let v1 = storage.get(&key).unwrap().unwrap();
        match v1.clip_data {
            ClipData::Text(TextData::BlobStored) => {}
            _ => panic!("expected blob-stored text after large upsert"),
        }

        // This matches the on-disk path used by keva_core for blob-stored text.
        let key_path = {
            let hash = blake3_v1::hash(key.as_str().as_bytes());
            PathBuf::from(hash.to_hex().as_str())
        };
        let blob_text_path = temp
            .path()
            .join("blobs")
            .join(key_path)
            .join(file::TEXT_FILE_NAME);

        assert!(blob_text_path.exists());

        // Now shrink it below the inline threshold.
        let small_text = "small";
        let later = now + Duration::from_secs(1);
        storage.upsert_text(&key, small_text, later).unwrap();

        // Ensure it is now inlined.
        let v2 = storage.get(&key).unwrap().unwrap();
        match v2.clip_data {
            ClipData::Text(TextData::Inlined(s)) => assert_eq!(s, small_text),
            _ => panic!("expected inlined text after shrinking"),
        }

        // Old blob file should be removed.
        assert!(!blob_text_path.exists());
    }
}

mod add_files {
    use super::common::{create_test_file, create_test_storage, make_key};
    use super::*;
    use crate::types::value::versioned_value::latest_value::{FileData, InlineFileData};

    #[test]
    fn test_add_files_creates_new_key() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/files");
        let file_path = create_test_file(&temp, "test.txt", b"file content");
        let now = SystemTime::now();

        storage.add_files(&key, [&file_path], now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 1);
                match &files[0] {
                    FileData::Inlined(InlineFileData { file_name, data }) => {
                        assert_eq!(file_name, "test.txt");
                        assert_eq!(data, b"file content");
                    }
                    _ => panic!("Expected inlined file"),
                }
            }
            _ => panic!("Expected Files variant"),
        }
    }

    #[test]
    fn test_add_files_appends_to_existing() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/files");
        let file1 = create_test_file(&temp, "file1.txt", b"content1");
        let file2 = create_test_file(&temp, "file2.txt", b"content2");
        let now = SystemTime::now();

        storage.add_files(&key, [&file1], now).unwrap();
        storage.add_files(&key, [&file2], now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 2);
            }
            _ => panic!("Expected Files variant"),
        }
    }

    #[test]
    fn test_add_files_to_text_fails() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/text");
        let file_path = create_test_file(&temp, "test.txt", b"content");
        let now = SystemTime::now();

        storage.upsert_text(&key, "text content", now).unwrap();

        let result = storage.add_files(&key, [&file_path], now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::TypeMismatch))
        ));
    }
}

mod remove_file_at {
    use super::common::{create_test_file, create_test_storage, make_key};
    use super::*;
    use crate::types::value::versioned_value::latest_value::{FileData, InlineFileData, TextData};

    #[test]
    fn test_remove_file_at_removes_selected_entry() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/remove_file_at");
        let now = SystemTime::now();

        let file1 = create_test_file(&temp, "file1.txt", b"content1");
        let file2 = create_test_file(&temp, "file2.txt", b"content2");

        storage.add_files(&key, [&file1, &file2], now).unwrap();

        // Remove the first entry (file1).
        let later = now + Duration::from_secs(1);
        storage.remove_file_at(&key, 0, later).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Files(files) => {
                assert_eq!(files.len(), 1);
                match &files[0] {
                    FileData::Inlined(InlineFileData { file_name, data }) => {
                        assert_eq!(file_name, "file2.txt");
                        assert_eq!(data, b"content2");
                    }
                    _ => panic!("Expected remaining file to be inlined"),
                }
            }
            _ => panic!("Expected Files variant after removing one file"),
        }
    }

    #[test]
    fn test_remove_file_at_removes_blob_stored_file_from_disk() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/remove_file_at_blob");
        let now = SystemTime::now();

        // Force blob storage by exceeding the inline threshold (1MB in create_test_storage()).
        let big = vec![b'x'; 1024 * 1024 + 1];
        let big_path = create_test_file(&temp, "big.bin", &big);

        storage.add_files(&key, [&big_path], now).unwrap();

        // Confirm it is blob-stored and the blob exists on disk at the expected path.
        let v1 = storage.get(&key).unwrap().unwrap();
        let (file_name, hash) = match &v1.clip_data {
            ClipData::Files(files) => match &files[0] {
                FileData::BlobStored(b) => (b.file_name.clone(), b.hash),
                _ => panic!("expected blob-stored file after adding > threshold"),
            },
            _ => panic!("expected Files variant"),
        };

        let key_dir = {
            let hash = blake3_v1::hash(key.as_str().as_bytes());
            PathBuf::from(hash.to_hex().as_str())
        };

        let blob_path = temp
            .path()
            .join("blobs")
            .join(&key_dir)
            .join(hash.to_string())
            .join(&file_name);

        assert!(blob_path.exists());

        // Remove the only file entry; core should remove the blob file from disk.
        let later = now + Duration::from_secs(1);
        storage.remove_file_at(&key, 0, later).unwrap();

        assert!(!blob_path.exists());

        // Value should become empty text.
        let v2 = storage.get(&key).unwrap().unwrap();
        match &v2.clip_data {
            ClipData::Text(TextData::Inlined(s)) => assert_eq!(s, ""),
            _ => panic!("expected empty inlined text after removing last file"),
        }
    }

    #[test]
    fn test_remove_file_at_last_file_becomes_empty_text() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/remove_file_at_last");
        let now = SystemTime::now();

        let file1 = create_test_file(&temp, "file1.txt", b"content1");
        storage.add_files(&key, [&file1], now).unwrap();

        let later = now + Duration::from_secs(1);
        storage.remove_file_at(&key, 0, later).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        match &value.clip_data {
            ClipData::Text(TextData::Inlined(s)) => assert_eq!(s, ""),
            ClipData::Text(TextData::BlobStored) => {
                panic!("Expected empty inlined text after removing last file")
            }
            _ => panic!("Expected Text variant after removing last file"),
        }
    }

    #[test]
    fn test_remove_file_at_on_text_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/remove_file_at_text");
        let now = SystemTime::now();

        storage.upsert_text(&key, "text content", now).unwrap();

        let result = storage.remove_file_at(&key, 0, now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::TypeMismatch))
        ));
    }

    #[test]
    fn test_remove_file_at_out_of_bounds_fails() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/remove_file_at_oob");
        let now = SystemTime::now();

        let file1 = create_test_file(&temp, "file1.txt", b"content1");
        storage.add_files(&key, [&file1], now).unwrap();

        let result = storage.remove_file_at(&key, 1, now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }
}

mod trash {
    use super::common::{create_test_storage, make_key};
    use super::*;

    #[test]
    fn test_trash_makes_key_trashed() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        let active_value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            active_value.metadata.lifecycle_state,
            LifecycleState::Active
        );

        storage.trash(&key, now).unwrap();

        // Key should still be visible, but with Trash state
        let trashed_value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            trashed_value.metadata.lifecycle_state,
            LifecycleState::Trash
        );
    }

    #[test]
    fn test_trash_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");
        let now = SystemTime::now();

        let result = storage.trash(&key, now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_trash_already_trashed_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();

        let result = storage.trash(&key, now);
        assert!(matches!(result, Err(StorageError::AlreadyTrashed)));
    }
}

mod rename {
    use super::common::{create_test_storage, make_key};
    use super::*;
    use crate::types::value::versioned_value::latest_value::TextData;

    #[test]
    fn test_rename_key() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.upsert_text(&old_key, "content", now).unwrap();
        storage.rename(&old_key, &new_key, false).unwrap();

        assert!(storage.get(&old_key).unwrap().is_none());
        let value = storage.get(&new_key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
    }

    #[test]
    fn test_rename_to_same_key_is_noop() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("same/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();

        // Should be a no-op and not error.
        storage.rename(&key, &key, false).unwrap();

        // Value should remain intact.
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
    }

    #[test]
    fn test_rename_fails_if_destination_exists() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.upsert_text(&old_key, "old content", now).unwrap();
        storage.upsert_text(&new_key, "new content", now).unwrap();

        let result = storage.rename(&old_key, &new_key, false);
        assert!(matches!(result, Err(StorageError::DestinationExists)));

        // Both keys should still exist with original content
        assert!(storage.get(&old_key).unwrap().is_some());
        assert!(storage.get(&new_key).unwrap().is_some());
    }

    #[test]
    fn test_rename_with_overwrite() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.upsert_text(&old_key, "old content", now).unwrap();
        storage.upsert_text(&new_key, "new content", now).unwrap();

        storage.rename(&old_key, &new_key, true).unwrap();

        assert!(storage.get(&old_key).unwrap().is_none());
        let value = storage.get(&new_key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("old content".to_string()))
        );
    }
}

mod keys {
    use super::common::{create_test_storage, make_key};
    use super::*;

    #[test]
    fn test_active_keys_returns_only_active() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();

        storage
            .upsert_text(&make_key("active1"), "content", now)
            .unwrap();
        storage
            .upsert_text(&make_key("active2"), "content", now)
            .unwrap();
        storage
            .upsert_text(&make_key("trashed"), "content", now)
            .unwrap();
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

        storage
            .upsert_text(&make_key("active"), "content", now)
            .unwrap();
        storage
            .upsert_text(&make_key("trashed1"), "content", now)
            .unwrap();
        storage
            .upsert_text(&make_key("trashed2"), "content", now)
            .unwrap();
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

mod touch {
    use super::common::{create_test_storage, make_key};
    use super::*;

    #[test]
    fn test_touch_updates_last_accessed() {
        let (mut storage, _temp) = create_test_storage();

        let key = make_key("k");
        let t1 = SystemTime::now();
        storage.upsert_text(&key, "content", t1).unwrap();

        let v1 = storage.get(&key).unwrap().unwrap();
        let last_accessed_1 = v1.metadata.last_accessed;

        // Ensure a strictly later time so the assertion is deterministic
        let t2 = t1 + Duration::from_secs(1);

        storage.touch(&key, t2).unwrap();

        let v2 = storage.get(&key).unwrap().unwrap();
        assert!(v2.metadata.last_accessed >= t2);
        assert!(v2.metadata.last_accessed > last_accessed_1);
    }

    #[test]
    fn test_touch_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();

        let result = storage.touch(&make_key("missing"), now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_touch_trashed_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let now = SystemTime::now();
        let key = make_key("k");

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();

        let result = storage.touch(&key, now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::Trashed))
        ));
    }

    #[test]
    fn test_get_does_not_update_last_accessed() {
        let (mut storage, _temp) = create_test_storage();

        let key = make_key("k");
        let t1 = SystemTime::now();
        storage.upsert_text(&key, "content", t1).unwrap();

        let v1 = storage.get(&key).unwrap().unwrap();
        let last_accessed_1 = v1.metadata.last_accessed;

        // Read again. Contract: get() is a pure read and does not touch.
        let v2 = storage.get(&key).unwrap().unwrap();

        assert_eq!(v2.metadata.last_accessed, last_accessed_1);
    }
}

mod get {
    use super::common::{create_test_storage, make_key};
    use super::*;
    use crate::types::value::versioned_value::latest_value::TextData;

    #[test]
    fn test_get_active_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
    }

    #[test]
    fn test_get_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();

        // Get should still work for trashed keys
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);
    }

    #[test]
    fn test_get_nonexistent_key() {
        let (storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");

        let result = storage.get(&key).unwrap();
        assert!(result.is_none());
    }
}

mod restore {
    use super::common::{create_test_storage, make_key};
    use super::*;

    #[test]
    fn test_restore_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);

        storage.restore(&key, now).unwrap();

        let restored = storage.get(&key).unwrap().unwrap();
        assert_eq!(restored.metadata.lifecycle_state, LifecycleState::Active);
    }

    #[test]
    fn test_restore_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");
        let now = SystemTime::now();

        let result = storage.restore(&key, now);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_restore_active_key_is_noop() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();

        // Restoring an active key is a no-op (succeeds silently)
        storage.restore(&key, now).unwrap();

        // Key should still be active
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
    }
}

mod purge {
    use super::common::{create_test_storage, make_key};
    use super::*;

    #[test]
    fn test_purge_active_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.purge(&key).unwrap();

        assert!(storage.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_trashed_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();
        storage.purge(&key).unwrap();

        assert!(storage.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_purge_nonexistent_key_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("nonexistent");

        let result = storage.purge(&key);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }
}

mod maintenance {
    use super::common::make_key;
    use super::*;

    fn create_storage_with_short_ttl() -> (KevaCore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            base_path: temp_dir.path().to_path_buf(),
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(10),
                purge_ttl: Duration::from_secs(5),
                inline_threshold_bytes: 1024 * 1024,
            },
        };
        let storage = KevaCore::open(config).unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_maintenance_purges_expired_trash_keys() {
        let (mut storage, _temp) = create_storage_with_short_ttl();
        let key = make_key("key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        storage.trash(&key, now).unwrap();

        // Run maintenance after purge_ttl expires
        let after_ttl = now + Duration::from_secs(6);
        let result = storage.maintenance(after_ttl).unwrap();

        // Key should be in purged list
        assert!(result.purged.contains(&key));
        assert!(result.trashed.is_empty());
    }

    #[test]
    fn test_maintenance_trashes_expired_active_keys() {
        let (mut storage, _temp) = create_storage_with_short_ttl();
        let key = make_key("key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();

        // Run maintenance after trash_ttl expires
        let after_ttl = now + Duration::from_secs(11);
        let result = storage.maintenance(after_ttl).unwrap();

        // Key should be in trashed list
        assert!(result.trashed.contains(&key));
        assert!(result.purged.is_empty());

        // Key should now be in trash state in DB
        let keys = storage.trashed_keys().unwrap();
        assert!(keys.contains(&key));
    }

    #[test]
    fn test_maintenance_cleans_up_blob_files() {
        let (mut storage, temp) = create_storage_with_short_ttl();
        let key = make_key("key");
        let now = SystemTime::now();

        // Create blob-stored text
        let large_text = "x".repeat(1024 * 1024 + 1);
        storage.upsert_text(&key, &large_text, now).unwrap();

        let key_path = {
            let hash = blake3_v1::hash(key.as_str().as_bytes());
            PathBuf::from(hash.to_hex().as_str())
        };
        let blob_dir = temp.path().join("blobs").join(&key_path);
        assert!(blob_dir.exists());

        // Trash and wait for purge TTL
        storage.trash(&key, now).unwrap();
        let after_ttl = now + Duration::from_secs(6);
        storage.maintenance(after_ttl).unwrap();

        // Blob directory should be cleaned up
        assert!(!blob_dir.exists());
    }

    #[test]
    fn test_maintenance_with_no_expired_keys() {
        let (mut storage, _temp) = create_storage_with_short_ttl();
        let key = make_key("key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();

        // Run maintenance immediately (no TTL expired)
        let result = storage.maintenance(now).unwrap();

        assert!(result.trashed.is_empty());
        assert!(result.purged.is_empty());

        // Key should still be active
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Active);
    }

    #[test]
    fn test_maintenance_cleans_orphan_blobs() {
        let (mut storage, temp) = create_storage_with_short_ttl();
        let key = make_key("key");
        let now = SystemTime::now();

        // Create a key with blob storage
        let large_text = "x".repeat(1024 * 1024 + 1);
        storage.upsert_text(&key, &large_text, now).unwrap();

        let key_path = {
            let hash = blake3_v1::hash(key.as_str().as_bytes());
            PathBuf::from(hash.to_hex().as_str())
        };
        let blob_dir = temp.path().join("blobs").join(&key_path);
        assert!(blob_dir.exists());

        // Simulate orphan by directly purging from DB without cleanup
        storage.db.purge(&key).unwrap();

        // Blob still exists (orphaned)
        assert!(blob_dir.exists());

        // Maintenance should clean it up
        storage.maintenance(now).unwrap();
        assert!(!blob_dir.exists());
    }
}
