use super::*;
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
            large_file_threshold_bytes: 1024 * 1024,           // 1MB
        },
    };
    let storage = KevaCore::open(config).unwrap();
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

        storage.upsert_text(&key, "hello world").unwrap();

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

        storage.upsert_text(&key, "first").unwrap();
        let first_value = storage.get(&key).unwrap().unwrap();
        let created_at = first_value.metadata.created_at;

        // Small delay to ensure different timestamps
        std::thread::sleep(Duration::from_millis(10));

        storage.upsert_text(&key, "second").unwrap();
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

        storage.upsert_text(&key, "first").unwrap();
        storage.trash(&key).unwrap();

        // Key should be visible as Trash
        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value.metadata.lifecycle_state, LifecycleState::Trash);

        // Upsert should fail - must restore first
        let result = storage.upsert_text(&key, "second");
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

        storage.add_files(&key, [&file_path]).unwrap();

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

        storage.add_files(&key, [&file1]).unwrap();
        storage.add_files(&key, [&file2]).unwrap();

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

        storage.upsert_text(&key, "text content").unwrap();

        let result = storage.add_files(&key, [&file_path]);
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

        storage.upsert_text(&key, "content").unwrap();
        let active_value = storage.get(&key).unwrap().unwrap();
        assert_eq!(
            active_value.metadata.lifecycle_state,
            LifecycleState::Active
        );

        storage.trash(&key).unwrap();

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

        let result = storage.trash(&key);
        assert!(matches!(
            result,
            Err(StorageError::Database(DatabaseError::NotFound))
        ));
    }

    #[test]
    fn test_trash_already_trashed_fails() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test/key");

        storage.upsert_text(&key, "content").unwrap();
        storage.trash(&key).unwrap();

        let result = storage.trash(&key);
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

        storage.upsert_text(&old_key, "content").unwrap();
        storage.rename(&old_key, &new_key).unwrap();

        assert!(storage.get(&old_key).unwrap().is_none());
        let value = storage.get(&new_key).unwrap().unwrap();
        assert_eq!(
            value.clip_data,
            ClipData::Text(TextData::Inlined("content".to_string()))
        );
    }
}

mod keys_and_list {
    use super::*;

    #[test]
    fn test_active_keys_returns_only_active() {
        let (mut storage, _temp) = create_test_storage();

        storage
            .upsert_text(&make_key("active1"), "content")
            .unwrap();
        storage
            .upsert_text(&make_key("active2"), "content")
            .unwrap();
        storage
            .upsert_text(&make_key("trashed"), "content")
            .unwrap();
        storage.trash(&make_key("trashed")).unwrap();

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

        storage.upsert_text(&make_key("active"), "content").unwrap();
        storage
            .upsert_text(&make_key("trashed1"), "content")
            .unwrap();
        storage
            .upsert_text(&make_key("trashed2"), "content")
            .unwrap();
        storage.trash(&make_key("trashed1")).unwrap();
        storage.trash(&make_key("trashed2")).unwrap();

        let keys = storage.trashed_keys().unwrap();
        assert_eq!(keys.len(), 2);

        let key_strings: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
        assert!(key_strings.contains(&"trashed1".to_string()));
        assert!(key_strings.contains(&"trashed2".to_string()));
        assert!(!key_strings.contains(&"active".to_string()));
    }

    #[test]
    fn test_list_filters_by_prefix() {
        let (mut storage, _temp) = create_test_storage();

        storage
            .upsert_text(&make_key("project/config"), "content")
            .unwrap();
        storage
            .upsert_text(&make_key("project/data"), "content")
            .unwrap();
        storage
            .upsert_text(&make_key("other/key"), "content")
            .unwrap();

        let project_keys = storage.list("project/").unwrap();
        assert_eq!(project_keys.len(), 2);

        let other_keys = storage.list("other/").unwrap();
        assert_eq!(other_keys.len(), 1);
    }
}

mod clipboard {
    use super::*;

    #[test]
    fn test_clipboard_methods_return_not_implemented() {
        let (mut storage, _temp) = create_test_storage();
        let key = make_key("test");

        let result1 = storage.upsert_from_clipboard(&key);
        assert!(matches!(
            result1,
            Err(StorageError::ClipboardNotImplemented)
        ));

        let result2 = storage.add_from_clipboard(&key);
        assert!(matches!(
            result2,
            Err(StorageError::ClipboardNotImplemented)
        ));
    }
}
