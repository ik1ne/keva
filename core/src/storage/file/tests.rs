mod common {
    use crate::storage::file::FileStorage;
    use crate::types::value::versioned_value::ValueVariant;
    use crate::types::value::versioned_value::latest_value::{BlobStoredFileData, FileData, Value};
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

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
}
mod store_file {
    use crate::storage::file::tests::common::store_blob_file;
    use crate::storage::file::*;
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
    use crate::storage::file::*;
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

        let text = "This is a long text.";
        let result = storage
            .store_text(Path::new("key_hash"), Cow::Borrowed(text))
            .unwrap();

        match &result {
            TextData::BlobStored => {
                let stored_file_path = storage.base_path.join("key_hash").join(TEXT_FILE_NAME);
                assert!(stored_file_path.exists());
                assert_eq!(std::fs::read_to_string(stored_file_path).unwrap(), text);
            }
            _ => panic!("Expected blob stored text data, got {:?}", result),
        }
    }
}

mod remove_file {
    use super::common::store_blob_file;
    use crate::storage::file::*;
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

        storage.remove_file(key_path, &blob_data).unwrap();

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

        let result = storage.remove_file(key_path, &blob_data);
        result.unwrap_err();
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

        storage.remove_file(key_path, &blob_data1).unwrap();

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

        storage.remove_file(key_path, &blob_data).unwrap();

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

mod remove_text {
    use crate::storage::file::*;
    use tempfile::tempdir;

    #[test]
    fn test_remove_blob_stored_text() {
        let temp_dir = tempdir().unwrap();
        let storage = FileStorage {
            base_path: temp_dir.path().to_path_buf(),
            inline_threshold_bytes: 10,
        };

        let key_path = Path::new("key_hash");
        let text = "This is a blob stored text.";
        let result = storage.store_text(key_path, Cow::Borrowed(text)).unwrap();

        match &result {
            TextData::BlobStored => {}
            _ => panic!("Expected blob stored text data, got {:?}", result),
        };

        storage
            .remove_text(key_path, &TextData::BlobStored)
            .unwrap();

        let stored_file_path = storage.base_path.join(key_path).join(TEXT_FILE_NAME);
        assert!(!stored_file_path.exists());
    }
}
