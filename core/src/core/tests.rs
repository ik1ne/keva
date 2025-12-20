use super::*;
use crate::search::SearchConfig;
use crate::types::config::SavedConfig;
use std::time::Duration;
use tempfile::TempDir;

fn create_test_storage() -> (KevaCore, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = Config {
        base_path: temp_dir.path().to_path_buf(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
            inline_threshold_bytes: 1024 * 1024,               // 1MB
        },
    };
    let storage = KevaCore::open(config, SearchConfig::default()).unwrap();
    (storage, temp_dir)
}

fn make_key(s: &str) -> Key {
    Key::try_from(s).unwrap()
}

mod upsert_text {
    use super::*;
    use crate::types::value::versioned_value::latest_value::TextData;

    #[test]
    fn test_upsert_creates_new_key() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "hello world", now).unwrap();

        let value = storage.get(&key, now).unwrap().unwrap();
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
        let first_value = storage.get(&key, now).unwrap().unwrap();
        let created_at = first_value.metadata.created_at;

        // Small delay to ensure different timestamps
        std::thread::sleep(Duration::from_millis(10));
        let later = SystemTime::now();

        storage.upsert_text(&key, "second", later).unwrap();
        let second_value = storage.get(&key, later).unwrap().unwrap();

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
        let value = storage.get(&key, now).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);

        // Upsert should fail - must restore first
        let result = storage.upsert_text(&key, "second", now);
        assert!(matches!(result, Err(StorageError::KeyIsTrashed)));
    }
}

mod add_files {
    use super::*;
    use crate::types::value::versioned_value::latest_value::{FileData, InlineFileData};
    use std::io::Write;

    fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[test]
    fn test_add_files_creates_new_key() {
        let (mut storage, temp) = create_test_storage();
        let key = make_key("test/files");
        let file_path = create_test_file(&temp, "test.txt", b"file content");
        let now = SystemTime::now();

        storage.add_files(&key, [&file_path], now).unwrap();

        let value = storage.get(&key, now).unwrap().unwrap();
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

        let value = storage.get(&key, now).unwrap().unwrap();
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

mod trash {
    use super::*;

    #[test]
    fn test_trash_makes_key_trashed() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");
        let now = SystemTime::now();

        storage.upsert_text(&key, "content", now).unwrap();
        let active_value = storage.get(&key, now).unwrap().unwrap();
        assert_eq!(
            active_value.metadata.lifecycle_state,
            LifecycleState::Active
        );

        storage.trash(&key, now).unwrap();

        // Key should still be visible, but with Trash state
        let trashed_value = storage.get(&key, now).unwrap().unwrap();
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
    use super::*;
    use crate::types::value::versioned_value::latest_value::TextData;

    #[test]
    fn test_rename_key() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.upsert_text(&old_key, "content", now).unwrap();
        storage.rename(&old_key, &new_key, false, now).unwrap();

        assert!(storage.get(&old_key, now).unwrap().is_none());
        let value = storage.get(&new_key, now).unwrap().unwrap();
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

        let result = storage.rename(&old_key, &new_key, false, now);
        assert!(matches!(result, Err(StorageError::DestinationExists)));

        // Both keys should still exist with original content
        assert!(storage.get(&old_key, now).unwrap().is_some());
        assert!(storage.get(&new_key, now).unwrap().is_some());
    }

    #[test]
    fn test_rename_with_overwrite() {
        let (mut storage, _temp) = create_test_storage();
        let old_key = make_key("old/key");
        let new_key = make_key("new/key");
        let now = SystemTime::now();

        storage.upsert_text(&old_key, "old content", now).unwrap();
        storage.upsert_text(&new_key, "new content", now).unwrap();

        storage.rename(&old_key, &new_key, true, now).unwrap();

        assert!(storage.get(&old_key, now).unwrap().is_none());
        let value = storage.get(&new_key, now).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("old content".to_string()))
        );
    }
}

mod keys_and_list {
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
