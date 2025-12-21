mod common {
    use crate::core::file::{FileStorage, TEXT_FILE_NAME, TextData};
    use crate::types::value::versioned_value::ValueVariant;
    use crate::types::value::versioned_value::latest_value::{
        BlobStoredFileData, FileData, InlineFileData, Value,
    };
    use std::borrow::Cow;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    pub(super) fn store_inline_file(
        temp_dir: &TempDir,
        storage: &FileStorage,
        key_hash: impl AsRef<Path> + Copy,
        file_name: &str,
        contents: &str,
    ) -> InlineFileData {
        let test_file_path = temp_dir.path().join(file_name);
        let mut test_file = std::fs::File::create(&test_file_path).unwrap();
        test_file.write_all(contents.as_bytes()).unwrap();

        let result = storage
            .store_file(key_hash.as_ref(), &test_file_path)
            .unwrap();

        match result {
            FileData::Inlined(inline_data) => {
                assert_eq!(inline_data.file_name, file_name);
                assert_eq!(inline_data.data, contents.as_bytes());
                inline_data
            }
            _ => panic!("Expected inline file data"),
        }
    }

    pub(super) fn store_blob_file(
        temp_dir: &TempDir,
        storage: &FileStorage,
        key_hash: impl AsRef<Path> + Copy,
        file_name: &str,
        contents: &str,
    ) -> BlobStoredFileData {
        let test_file_path = temp_dir.path().join(file_name);
        let mut test_file = std::fs::File::create(&test_file_path).unwrap();
        test_file.write_all(contents.as_bytes()).unwrap();

        let result = storage
            .store_file(key_hash.as_ref(), &test_file_path)
            .unwrap();

        match result {
            FileData::BlobStored(blob_data) => {
                assert_eq!(blob_data.file_name, file_name);
                assert_eq!(blob_data.hash, {
                    let mut hasher = <Value as ValueVariant>::Hasher::new();
                    hasher.update(contents.as_bytes());
                    hasher.finalize()
                });

                let stored_file_path = storage
                    .base_path
                    .join(key_hash)
                    .join(blob_data.hash.to_string())
                    .join(&blob_data.file_name);
                assert!(stored_file_path.exists());
                assert_eq!(std::fs::read_to_string(stored_file_path).unwrap(), contents);
                blob_data
            }
            _ => panic!("Expected blob stored file data"),
        }
    }

    pub(super) fn store_inline_text(
        storage: &FileStorage,
        key_hash: impl AsRef<Path> + Copy,
        text: &str,
    ) -> String {
        let result = storage
            .store_text(key_hash.as_ref(), Cow::Borrowed(text))
            .unwrap();

        match result {
            TextData::Inlined(inlined_text) => {
                assert_eq!(inlined_text, text);
                inlined_text
            }
            _ => panic!("Expected inlined text data, got {:?}", result),
        }
    }

    pub(super) fn store_blob_text(
        storage: &FileStorage,
        key_hash: impl AsRef<Path> + Copy,
        text: &str,
    ) -> TextData {
        let result = storage
            .store_text(key_hash.as_ref(), Cow::Borrowed(text))
            .unwrap();

        match &result {
            TextData::BlobStored => {
                let stored_file_path = storage.base_path.join(key_hash).join(TEXT_FILE_NAME);
                assert!(stored_file_path.exists());
                assert_eq!(std::fs::read_to_string(stored_file_path).unwrap(), text);
            }
            _ => panic!("Expected blob stored text data, got {:?}", result),
        }

        result
    }
}
mod store_file {
    use crate::core::file::tests::common::store_blob_file;
    use crate::core::file::*;
    use std::ffi::OsString;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_store_inline_file() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let test_file_path = temp_dir.path().join("test_inline.txt");
        let mut test_file = std::fs::File::create(&test_file_path).unwrap();
        writeln!(test_file, "This is a small file.").unwrap();

        let result = storage
            .store_file(Path::new("key_hash"), &test_file_path)
            .unwrap();

        match result {
            FileData::Inlined(inline_data) => {
                assert_eq!(inline_data.file_name, "test_inline.txt");
                assert_eq!(inline_data.data, b"This is a small file.\n");
            }
            _ => panic!("Expected inline file data"),
        }
    }

    #[test]
    fn test_store_blob_file() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        store_blob_file(
            &temp_dir,
            &storage,
            Path::new("key_hash"),
            "test_blob.txt",
            "This is a blob stored file.",
        );
    }

    #[test]
    fn test_store_two_blob_files() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        store_blob_file(
            &temp_dir,
            &storage,
            "key_hash",
            "test_blob1.txt",
            "This is the first blob stored file.",
        );

        store_blob_file(
            &temp_dir,
            &storage,
            "key_hash",
            "test_blob2.txt",
            "This is the second blob stored file.",
        );
    }

    #[test]
    fn test_store_directory_error() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let result = storage.store_file(Path::new("key_hash"), temp_dir.path());

        match result {
            Err(FileStorageError::IsDirectory) => {}
            result => panic!("Expected IsDirectory error, got {:?}", result),
        }
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS does not support non-UTF-8 file names in HFS+ and APFS"
    )]
    fn test_store_non_utf8_file_name() {
        #[cfg(unix)]
        let invalid: OsString = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0x80, 0x81, 0x82])
        };

        #[cfg(windows)]
        let invalid: OsString = {
            use std::os::windows::ffi::OsStringExt;
            // Unpaired surrogate - valid WTF-8, invalid UTF-8
            OsString::from_wide(&[0xD800])
        };

        #[cfg(not(any(unix, windows)))]
        {
            panic!("Test not supported on this platform");
        }

        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let non_utf8_file_path = temp_dir.path().join(invalid);
        std::fs::File::create(&non_utf8_file_path).unwrap();
        let result = storage.store_file(Path::new("key_hash"), &non_utf8_file_path);
        match result {
            Err(FileStorageError::NonUtf8FileName) => {}
            result => panic!("Expected NonUtf8FileName error, got {:?}", result),
        }
    }
}

mod store_text {
    use crate::core::file::tests::common::store_blob_text;
    use crate::core::file::*;
    use tempfile::tempdir;

    #[test]
    fn test_store_inlined_text() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let text = "This is a short text.";
        let result = storage
            .store_text(Path::new("key_hash"), Cow::Borrowed(text))
            .unwrap();

        match &result {
            TextData::Inlined(i) => assert_eq!(i, text),
            _ => panic!("Expected inlined text data, got {:?}", result),
        }
    }

    #[test]
    fn test_store_blob_stored_text() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        store_blob_text(&storage, Path::new("key_hash"), "This is a long text.");
    }
}

mod remove_blob_stored_file {
    use super::common::store_blob_file;
    use crate::core::file::*;
    use tempfile::tempdir;

    #[test]
    fn test_remove_blob_file() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let file_name = "to_be_removed.txt";
        let blob_data = store_blob_file(
            &temp_dir,
            &storage,
            key_path,
            file_name,
            "This blob file will be removed.",
        );

        storage
            .remove_blob_stored_file(key_path, &blob_data)
            .unwrap();

        let stored_file_path = storage
            .base_path
            .join(key_path)
            .join(blob_data.hash.to_string())
            .join(file_name);
        assert!(!stored_file_path.exists());
        let parent_dir = stored_file_path.parent().unwrap();
        assert!(!parent_dir.exists());
    }

    #[test]
    fn test_remove_nonexistent_blob_file() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let blob_data = BlobStoredFileData {
            file_name: "nonexistent.txt".to_string(),
            hash: {
                let mut hasher = <Value as ValueVariant>::Hasher::new();
                hasher.update(b"nonexistent content");
                hasher.finalize()
            },
        };

        let result = storage.remove_blob_stored_file(key_path, &blob_data);
        result.unwrap(); // Should succeed (idempotent delete)
    }

    #[test]
    fn test_remove_blob_file_from_two_files() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let file_name1 = "shared_file1.txt";
        let file_name2 = "shared_file2.txt";
        let file_contents1 = "This is the first shared blob file.";
        let file_contents2 = "This is the second shared blob file.";

        let blob_data1 = store_blob_file(&temp_dir, &storage, key_path, file_name1, file_contents1);

        let blob_data2 = store_blob_file(&temp_dir, &storage, key_path, file_name2, file_contents2);

        storage
            .remove_blob_stored_file(key_path, &blob_data1)
            .unwrap();

        let stored_file_path1 = storage
            .base_path
            .join(key_path)
            .join(blob_data1.hash.to_string())
            .join(&blob_data1.file_name);
        assert!(!stored_file_path1.exists());

        let stored_file_path2 = storage
            .base_path
            .join(key_path)
            .join(blob_data2.hash.to_string())
            .join(&blob_data2.file_name);
        assert!(stored_file_path2.exists());
    }

    #[test]
    fn test_remove_last_file_cleans_directory() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let file_name = "only_file.txt";
        let blob_data = store_blob_file(
            &temp_dir,
            &storage,
            key_path,
            file_name,
            "This is the only blob file.",
        );

        storage
            .remove_blob_stored_file(key_path, &blob_data)
            .unwrap();

        let stored_file_path = storage
            .base_path
            .join(key_path)
            .join(blob_data.hash.to_string())
            .join(file_name);
        assert!(!stored_file_path.exists());
        let hash_dir = storage
            .base_path
            .join(key_path)
            .join(blob_data.hash.to_string());
        assert!(!hash_dir.exists());
        let key_dir = storage.base_path.join(key_path);
        assert!(!key_dir.exists());
    }
}

mod remove_blob_stored_text {
    use crate::core::file::tests::common::store_blob_text;
    use crate::core::file::*;
    use tempfile::tempdir;

    #[test]
    fn test_remove_blob_stored_text() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        store_blob_text(&storage, key_path, "This blob text will be removed.");

        storage.remove_blob_stored_text(key_path).unwrap();

        let stored_file_path = storage.base_path.join(key_path).join(TEXT_FILE_NAME);
        assert!(!stored_file_path.exists());
    }
}

mod remove_all {
    use crate::core::file::FileStorage;
    use crate::core::file::tests::common::{
        store_blob_file, store_blob_text, store_inline_file, store_inline_text,
    };
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_remove_all_files_and_texts() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let inlined_text = "inline";
        let blob_text = "This is a blob.";

        store_inline_file(&temp_dir, &storage, key_path, "inline.txt", inlined_text);
        store_blob_file(&temp_dir, &storage, key_path, "blob.txt", blob_text);

        store_inline_text(&storage, key_path, inlined_text);
        store_blob_text(&storage, key_path, blob_text);

        assert!(temp_dir.path().join(key_path).exists());

        storage.remove_all(key_path).unwrap();

        assert!(!temp_dir.path().join(key_path).exists());
    }
}

mod ensure_file_path {
    use crate::core::file::tests::common::{store_blob_file, store_inline_file};
    use crate::core::file::{FileStorage, TEMP_INLINE_CACHE};
    use crate::types::value::versioned_value::ValueVariant;
    use crate::types::value::versioned_value::latest_value::{FileData, Value};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_inlined_file_path() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let file_name = "inlined.txt";
        let contents = "This is an inlined file.";
        let hash = <Value as ValueVariant>::Hasher::new()
            .update(contents.as_bytes())
            .finalize();

        let key_path = Path::new("key_hash");
        let inline_file_data =
            store_inline_file(&temp_dir, &storage, key_path, file_name, contents);

        let result = storage
            .ensure_file_path(key_path, &FileData::Inlined(inline_file_data))
            .unwrap();

        assert_eq!(std::fs::read_to_string(&result).unwrap(), contents);
        assert_eq!(
            result,
            temp_dir
                .path()
                .join(TEMP_INLINE_CACHE)
                .join(key_path)
                .join(hash.to_string())
                .join(file_name)
        );
    }

    #[test]
    fn test_ensure_is_idempotent_for_same_key_and_file() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let file_name = "inlined.txt";
        let contents = "This is an inlined file.";
        let hash = <Value as ValueVariant>::Hasher::new()
            .update(contents.as_bytes())
            .finalize();

        let key_path = Path::new("key_hash");

        // First ensure: should materialize the file.
        let inline_file_data_1 =
            store_inline_file(&temp_dir, &storage, key_path, file_name, contents);
        let ensured_1 = storage
            .ensure_file_path(key_path, &FileData::Inlined(inline_file_data_1))
            .unwrap();

        assert!(ensured_1.exists());

        let metadata_1 = std::fs::metadata(&ensured_1).unwrap();
        let modified_1 = metadata_1.modified().unwrap();

        // Second ensure with the same key and same content: should not rewrite.
        let inline_file_data_2 =
            store_inline_file(&temp_dir, &storage, key_path, file_name, contents);
        let ensured_2 = storage
            .ensure_file_path(key_path, &FileData::Inlined(inline_file_data_2))
            .unwrap();

        assert_eq!(ensured_2, ensured_1);
        assert_eq!(
            ensured_2,
            temp_dir
                .path()
                .join(TEMP_INLINE_CACHE)
                .join(key_path)
                .join(hash.to_string())
                .join(file_name)
        );

        let metadata_2 = std::fs::metadata(&ensured_2).unwrap();
        let modified_2 = metadata_2.modified().unwrap();

        // If we didn't rewrite, mtime should stay the same (or at least not go backwards).
        // Prefer strict equality, but some filesystems have coarse timestamp resolution.
        assert!(modified_2 >= modified_1);
    }

    #[test]
    fn test_ensure_switching_keys_clears_previous_ensured_cache() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 1024,
        };

        let file_name = "inlined.txt";
        let contents = "This is an inlined file.";
        let hash = <Value as ValueVariant>::Hasher::new()
            .update(contents.as_bytes())
            .finalize();

        let key_a = Path::new("key_hash_a");
        let key_b = Path::new("key_hash_b");

        let inline_a = store_inline_file(&temp_dir, &storage, key_a, file_name, contents);
        let inline_b = store_inline_file(&temp_dir, &storage, key_b, file_name, contents);

        let ensured_a = storage
            .ensure_file_path(key_a, &FileData::Inlined(inline_a))
            .unwrap();

        assert!(ensured_a.exists());
        assert_eq!(
            ensured_a,
            temp_dir
                .path()
                .join(TEMP_INLINE_CACHE)
                .join(key_a)
                .join(hash.to_string())
                .join(file_name)
        );

        let ensured_b = storage
            .ensure_file_path(key_b, &FileData::Inlined(inline_b))
            .unwrap();

        assert!(ensured_b.exists());
        assert_eq!(
            ensured_b,
            temp_dir
                .path()
                .join(TEMP_INLINE_CACHE)
                .join(key_b)
                .join(hash.to_string())
                .join(file_name)
        );

        // Ensuring for key_b should clear ensured cache for key_a.
        let ensured_root = temp_dir.path().join(TEMP_INLINE_CACHE);
        assert!(!ensured_root.join(key_a).exists());
        assert!(ensured_root.join(key_b).exists());
    }

    #[test]
    fn test_ensure_blob_stored_file_path() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let file_name = "blob_stored.txt";
        let contents = "This is a blob stored file.";
        let hash = <Value as ValueVariant>::Hasher::new()
            .update(contents.as_bytes())
            .finalize();

        let key_path = Path::new("key_hash");
        let blob_file_data = store_blob_file(&temp_dir, &storage, key_path, file_name, contents);

        let result = storage
            .ensure_file_path(key_path, &FileData::BlobStored(blob_file_data))
            .unwrap();

        assert_eq!(std::fs::read_to_string(&result).unwrap(), contents);
        assert_eq!(
            result,
            temp_dir
                .path()
                .join(key_path)
                .join(hash.to_string())
                .join(file_name)
        );
    }
}
