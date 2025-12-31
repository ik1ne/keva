use super::*;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn create_test_storage() -> (FileStorage, tempfile::TempDir) {
    let temp_dir = tempdir().unwrap();
    let storage = FileStorage {
        content_path: temp_dir.path().join("content"),
        blobs_path: temp_dir.path().join("blobs"),
        thumbnails_path: temp_dir.path().join("thumbnails"),
    };
    (storage, temp_dir)
}

fn create_test_file(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(content).unwrap();
    path
}

mod create_content {
    use super::*;

    #[test]
    fn test_create_content_creates_empty_file() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        storage.create_content(key_hash).unwrap();

        let content_file = storage.content_file_path(key_hash);
        assert!(content_file.exists());
        assert_eq!(std::fs::read_to_string(&content_file).unwrap(), "");
    }

    #[test]
    fn test_create_content_creates_parent_directories() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        assert!(!storage.content_path.exists());
        storage.create_content(key_hash).unwrap();
        assert!(storage.content_path.exists());
    }

    #[test]
    fn test_content_file_path_has_md_extension() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let path = storage.content_file_path(key_hash);
        assert_eq!(path.extension().unwrap(), "md");
    }
}

mod remove_content {
    use super::*;

    #[test]
    fn test_remove_content_removes_file() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        storage.create_content(key_hash).unwrap();
        let content_file = storage.content_file_path(key_hash);
        assert!(content_file.exists());

        storage.remove_content(key_hash).unwrap();
        assert!(!content_file.exists());
    }

    #[test]
    fn test_remove_nonexistent_content_succeeds() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("nonexistent");

        storage.remove_content(key_hash).unwrap();
    }
}

mod add_attachment {
    use super::*;

    #[test]
    fn test_add_attachment_copies_file() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source = create_test_file(&temp, "source.txt", b"file content");

        let size = storage
            .add_attachment(key_hash, &source, "dest.txt")
            .unwrap();

        assert_eq!(size, 12);
        let dest_path = storage.attachment_path(key_hash, "dest.txt");
        assert!(dest_path.exists());
        assert_eq!(std::fs::read_to_string(&dest_path).unwrap(), "file content");
    }

    #[test]
    fn test_add_attachment_creates_directory() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source = create_test_file(&temp, "source.txt", b"content");

        assert!(!storage.blobs_path.join(key_hash).exists());
        storage
            .add_attachment(key_hash, &source, "file.txt")
            .unwrap();
        assert!(storage.blobs_path.join(key_hash).exists());
    }

    #[test]
    fn test_add_directory_fails() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let result = storage.add_attachment(key_hash, temp.path(), "dir");
        assert!(matches!(result, Err(FileStorageError::IsDirectory)));
    }
}

mod attachment_path {
    use super::*;

    #[test]
    fn test_attachment_path_format() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let path = storage.attachment_path(key_hash, "test.txt");
        assert!(
            path.ends_with("blobs/abc123/test.txt") || path.ends_with("blobs\\abc123\\test.txt")
        );
    }
}

mod remove_attachment {
    use super::*;

    #[test]
    fn test_remove_attachment_removes_file() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source = create_test_file(&temp, "source.txt", b"content");

        storage
            .add_attachment(key_hash, &source, "file.txt")
            .unwrap();
        let path = storage.attachment_path(key_hash, "file.txt");
        assert!(path.exists());

        storage.remove_attachment(key_hash, "file.txt").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_remove_last_attachment_cleans_directory() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source = create_test_file(&temp, "source.txt", b"content");

        storage
            .add_attachment(key_hash, &source, "file.txt")
            .unwrap();
        let key_dir = storage.blobs_path.join(key_hash);
        assert!(key_dir.exists());

        storage.remove_attachment(key_hash, "file.txt").unwrap();
        assert!(!key_dir.exists());
    }

    #[test]
    fn test_remove_nonexistent_attachment_succeeds() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        storage
            .remove_attachment(key_hash, "nonexistent.txt")
            .unwrap();
    }
}

mod rename_attachment {
    use super::*;

    #[test]
    fn test_rename_attachment_renames_file() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source = create_test_file(&temp, "source.txt", b"content");

        storage
            .add_attachment(key_hash, &source, "old.txt")
            .unwrap();

        storage
            .rename_attachment(key_hash, "old.txt", "new.txt")
            .unwrap();

        let old_path = storage.attachment_path(key_hash, "old.txt");
        let new_path = storage.attachment_path(key_hash, "new.txt");
        assert!(!old_path.exists());
        assert!(new_path.exists());
        assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "content");
    }

    #[test]
    fn test_rename_nonexistent_attachment_succeeds() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        storage
            .rename_attachment(key_hash, "nonexistent.txt", "new.txt")
            .unwrap();
    }
}

mod remove_all_attachments {
    use super::*;

    #[test]
    fn test_remove_all_attachments_removes_directory() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");
        let source1 = create_test_file(&temp, "file1.txt", b"content1");
        let source2 = create_test_file(&temp, "file2.txt", b"content2");

        storage
            .add_attachment(key_hash, &source1, "file1.txt")
            .unwrap();
        storage
            .add_attachment(key_hash, &source2, "file2.txt")
            .unwrap();

        let key_dir = storage.blobs_path.join(key_hash);
        assert!(key_dir.exists());

        storage.remove_all_attachments(key_hash).unwrap();
        assert!(!key_dir.exists());
    }

    #[test]
    fn test_remove_all_attachments_nonexistent_succeeds() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("nonexistent");

        storage.remove_all_attachments(key_hash).unwrap();
    }
}

mod thumbnail {
    use super::*;

    #[test]
    fn test_thumbnail_path_format() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let path = storage.thumbnail_path(key_hash, "image.png");
        assert!(
            path.ends_with("thumbnails/abc123/image.png.thumb")
                || path.ends_with("thumbnails\\abc123\\image.png.thumb")
        );
    }

    #[test]
    fn test_remove_thumbnail_removes_file() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let thumb_dir = storage.thumbnails_path.join(key_hash);
        std::fs::create_dir_all(&thumb_dir).unwrap();
        let thumb_path = thumb_dir.join("image.png.thumb");
        std::fs::write(&thumb_path, b"thumbnail data").unwrap();

        storage.remove_thumbnail(key_hash, "image.png").unwrap();
        assert!(!thumb_path.exists());
    }

    #[test]
    fn test_remove_last_thumbnail_cleans_directory() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let thumb_dir = storage.thumbnails_path.join(key_hash);
        std::fs::create_dir_all(&thumb_dir).unwrap();
        std::fs::write(thumb_dir.join("image.png.thumb"), b"data").unwrap();

        storage.remove_thumbnail(key_hash, "image.png").unwrap();
        assert!(!thumb_dir.exists());
    }

    #[test]
    fn test_remove_all_thumbnails_removes_directory() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        let thumb_dir = storage.thumbnails_path.join(key_hash);
        std::fs::create_dir_all(&thumb_dir).unwrap();
        std::fs::write(thumb_dir.join("img1.thumb"), b"data1").unwrap();
        std::fs::write(thumb_dir.join("img2.thumb"), b"data2").unwrap();

        storage.remove_all_thumbnails(key_hash).unwrap();
        assert!(!thumb_dir.exists());
    }
}

mod remove_all {
    use super::*;

    #[test]
    fn test_remove_all_removes_content_attachments_thumbnails() {
        let (storage, temp) = create_test_storage();
        let key_hash = Path::new("abc123");

        // Create content
        storage.create_content(key_hash).unwrap();

        // Create attachment
        let source = create_test_file(&temp, "file.txt", b"content");
        storage
            .add_attachment(key_hash, &source, "file.txt")
            .unwrap();

        // Create thumbnail
        let thumb_dir = storage.thumbnails_path.join(key_hash);
        std::fs::create_dir_all(&thumb_dir).unwrap();
        std::fs::write(thumb_dir.join("img.thumb"), b"data").unwrap();

        // Verify all exist
        assert!(storage.content_file_path(key_hash).exists());
        assert!(storage.blobs_path.join(key_hash).exists());
        assert!(storage.thumbnails_path.join(key_hash).exists());

        storage.remove_all(key_hash).unwrap();

        assert!(!storage.content_file_path(key_hash).exists());
        assert!(!storage.blobs_path.join(key_hash).exists());
        assert!(!storage.thumbnails_path.join(key_hash).exists());
    }
}

mod rename_all {
    use super::*;

    #[test]
    fn test_rename_all_renames_content_attachments_thumbnails() {
        let (storage, temp) = create_test_storage();
        let old_hash = Path::new("old_hash");
        let new_hash = Path::new("new_hash");

        // Create content
        storage.create_content(old_hash).unwrap();
        std::fs::write(storage.content_file_path(old_hash), "test").unwrap();

        // Create attachment
        let source = create_test_file(&temp, "file.txt", b"content");
        storage
            .add_attachment(old_hash, &source, "file.txt")
            .unwrap();

        // Create thumbnail
        let thumb_dir = storage.thumbnails_path.join(old_hash);
        std::fs::create_dir_all(&thumb_dir).unwrap();
        std::fs::write(thumb_dir.join("img.thumb"), b"data").unwrap();

        storage.rename_all(old_hash, new_hash).unwrap();

        // Old paths should not exist
        assert!(!storage.content_file_path(old_hash).exists());
        assert!(!storage.blobs_path.join(old_hash).exists());
        assert!(!storage.thumbnails_path.join(old_hash).exists());

        // New paths should exist
        assert!(storage.content_file_path(new_hash).exists());
        assert!(storage.blobs_path.join(new_hash).exists());
        assert!(storage.thumbnails_path.join(new_hash).exists());
    }

    #[test]
    fn test_rename_all_nonexistent_succeeds() {
        let (storage, _temp) = create_test_storage();
        let old_hash = Path::new("nonexistent");
        let new_hash = Path::new("new_hash");

        storage.rename_all(old_hash, new_hash).unwrap();
    }

    #[test]
    fn test_rename_all_overwrites_destination() {
        let (storage, temp) = create_test_storage();
        let old_hash = Path::new("old_hash");
        let new_hash = Path::new("new_hash");

        // Create old content
        storage.create_content(old_hash).unwrap();
        std::fs::write(storage.content_file_path(old_hash), "old content").unwrap();

        // Create old attachment
        let source1 = create_test_file(&temp, "old.txt", b"old attachment");
        storage
            .add_attachment(old_hash, &source1, "file.txt")
            .unwrap();

        // Create new content (to be overwritten)
        storage.create_content(new_hash).unwrap();
        std::fs::write(storage.content_file_path(new_hash), "new content").unwrap();

        // Create new attachment (to be overwritten)
        let source2 = create_test_file(&temp, "new.txt", b"new attachment");
        storage
            .add_attachment(new_hash, &source2, "other.txt")
            .unwrap();

        storage.rename_all(old_hash, new_hash).unwrap();

        // Content should be old content
        assert_eq!(
            std::fs::read_to_string(storage.content_file_path(new_hash)).unwrap(),
            "old content"
        );

        // Attachments should be from old key
        let new_blobs = storage.blobs_path.join(new_hash);
        assert!(new_blobs.join("file.txt").exists());
        assert!(!new_blobs.join("other.txt").exists());
    }
}

mod list_key_hashes {
    use super::*;

    #[test]
    fn test_list_blob_key_hashes() {
        let (storage, temp) = create_test_storage();
        let source = create_test_file(&temp, "file.txt", b"content");

        storage
            .add_attachment(Path::new("hash1"), &source, "file.txt")
            .unwrap();
        storage
            .add_attachment(Path::new("hash2"), &source, "file.txt")
            .unwrap();

        let hashes = storage.list_blob_key_hashes().unwrap();
        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains(&std::path::PathBuf::from("hash1")));
        assert!(hashes.contains(&std::path::PathBuf::from("hash2")));
    }

    #[test]
    fn test_list_blob_key_hashes_empty() {
        let (storage, _temp) = create_test_storage();

        let hashes = storage.list_blob_key_hashes().unwrap();
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_list_content_key_hashes() {
        let (storage, _temp) = create_test_storage();

        storage.create_content(Path::new("hash1")).unwrap();
        storage.create_content(Path::new("hash2")).unwrap();

        let hashes = storage.list_content_key_hashes().unwrap();
        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains(&std::path::PathBuf::from("hash1")));
        assert!(hashes.contains(&std::path::PathBuf::from("hash2")));
    }

    #[test]
    fn test_list_content_key_hashes_empty() {
        let (storage, _temp) = create_test_storage();

        let hashes = storage.list_content_key_hashes().unwrap();
        assert!(hashes.is_empty());
    }
}

mod thumbnail_generation {
    use super::*;

    #[test]
    fn test_is_supported_image_png() {
        assert!(FileStorage::is_supported_image("image.png"));
        assert!(FileStorage::is_supported_image("image.PNG"));
    }

    #[test]
    fn test_is_supported_image_jpeg() {
        assert!(FileStorage::is_supported_image("photo.jpg"));
        assert!(FileStorage::is_supported_image("photo.jpeg"));
        assert!(FileStorage::is_supported_image("photo.JPEG"));
    }

    #[test]
    fn test_is_supported_image_other_formats() {
        assert!(FileStorage::is_supported_image("image.gif"));
        assert!(FileStorage::is_supported_image("image.webp"));
    }

    #[test]
    fn test_is_supported_image_unsupported() {
        assert!(!FileStorage::is_supported_image("document.pdf"));
        assert!(!FileStorage::is_supported_image("video.mp4"));
        assert!(!FileStorage::is_supported_image("file.txt"));
        assert!(!FileStorage::is_supported_image("noext"));
    }

    #[test]
    fn test_generate_thumbnail_unsupported_format() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        let result = storage.generate_thumbnail(key_hash, "document.pdf");
        assert!(matches!(result, Err(FileStorageError::UnsupportedFormat)));
    }

    fn create_test_image(storage: &FileStorage, key_hash: &Path, filename: &str, width: u32, height: u32) {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let blob_dir = storage.blobs_path.join(key_hash);
        std::fs::create_dir_all(&blob_dir).unwrap();
        img.save(blob_dir.join(filename)).unwrap();
    }

    fn open_thumbnail(path: &Path) -> image::DynamicImage {
        let bytes = std::fs::read(path).unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    #[test]
    fn test_generate_thumbnail_landscape() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        create_test_image(&storage, key_hash, "landscape.png", 400, 300);
        storage.generate_thumbnail(key_hash, "landscape.png").unwrap();

        let thumb_path = storage.thumbnail_path(key_hash, "landscape.png");
        assert!(thumb_path.exists());

        let thumb = open_thumbnail(&thumb_path);
        // 400x300 scaled to fit 200px max -> 200x150
        assert_eq!(thumb.width(), 200);
        assert_eq!(thumb.height(), 150);
    }

    #[test]
    fn test_generate_thumbnail_portrait() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        create_test_image(&storage, key_hash, "portrait.png", 300, 600);
        storage.generate_thumbnail(key_hash, "portrait.png").unwrap();

        let thumb = open_thumbnail(&storage.thumbnail_path(key_hash, "portrait.png"));
        // 300x600 scaled to fit 200px max -> 100x200
        assert_eq!(thumb.width(), 100);
        assert_eq!(thumb.height(), 200);
    }

    #[test]
    fn test_generate_thumbnail_square() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        create_test_image(&storage, key_hash, "square.png", 400, 400);
        storage.generate_thumbnail(key_hash, "square.png").unwrap();

        let thumb = open_thumbnail(&storage.thumbnail_path(key_hash, "square.png"));
        assert_eq!(thumb.width(), 200);
        assert_eq!(thumb.height(), 200);
    }

    #[test]
    fn test_generate_thumbnail_no_upscale() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        create_test_image(&storage, key_hash, "small.png", 100, 80);
        storage.generate_thumbnail(key_hash, "small.png").unwrap();

        let thumb = open_thumbnail(&storage.thumbnail_path(key_hash, "small.png"));
        // Small images should not be upscaled
        assert_eq!(thumb.width(), 100);
        assert_eq!(thumb.height(), 80);
    }

    #[test]
    fn test_generate_thumbnail_exact_size() {
        let (storage, _temp) = create_test_storage();
        let key_hash = Path::new("test_hash");

        create_test_image(&storage, key_hash, "exact.png", 200, 150);
        storage.generate_thumbnail(key_hash, "exact.png").unwrap();

        let thumb = open_thumbnail(&storage.thumbnail_path(key_hash, "exact.png"));
        assert_eq!(thumb.width(), 200);
        assert_eq!(thumb.height(), 150);
    }
}
