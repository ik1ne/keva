# keva_core Specification

## Overview

keva_core is the platform-agnostic storage layer for Keva. It manages key-value storage with markdown content and file
attachments.

## Data Model

### Key

Validated string with constraints:

- Valid UTF-8
- Length: 1–256 characters
- Trimmed (no leading/trailing whitespace)
- Non-empty

Enforced by `Key` struct using Nutype.

### Value

```rust
struct Value {
    metadata: Metadata,
    attachments: Vec<Attachment>,
    thumb_version: u32,
}
```

Every key has exactly one markdown content file (path derived from key) plus zero or more attachments.

### Metadata

```rust
struct Metadata {
    lifecycle_state: LifecycleState,
}
```

### LifecycleState

Timestamps are embedded in the state variants for a cleaner model where each state owns its relevant timestamp.

```rust
enum LifecycleState {
    Active { last_accessed: SystemTime },
    Trash { trashed_at: SystemTime },
}
```

### Attachment

```rust
struct Attachment {
    filename: String,      // unique within key
    size: u64,
}
```

Filenames must be unique within a key. Duplicate filenames are handled at the API level with explicit conflict
resolution.

## Storage Structure

All storage is file-based. No inline storage in database.

```
{base_path}/
├── keva.redb                              # Database (metadata only)
├── content/{key_hash}.md                  # Markdown content (flat)
├── blobs/{key_hash}/{filename}            # Attachments
└── thumbnails/{key_hash}/{filename}.thumb # Generated thumbnails
```

- `{key_hash}`: Deterministic hash of key string
- `{filename}`: Original filename (unique within key)
- Three separate trees prevent filename collisions

### Database

`keva.redb` stores only metadata:

- Key → Value mapping (Metadata + attachments list + thumb_version)
- No content stored in database

### Content

Each key has exactly one markdown file at `content/{key_hash}.md`.

- Path derived from key (no storage in Value struct)
- Created when key is first created
- May be empty (zero bytes)
- Edited directly via FileSystemHandle (not through keva_core API)

### Blobs

Attachments stored at `blobs/{key_hash}/{filename}`.

- Original filename preserved
- Unique within key (enforced by API)
- Copied via `std::fs::copy` (enables CoW on supporting filesystems)

### Thumbnails

Generated previews stored at `thumbnails/{key_hash}/{filename}.thumb`.

- Supported formats: png, jpg, jpeg, gif, webp
- Version-controlled regeneration (see Thumbnail Versioning)
- Missing thumbnail → fallback to icon in UI

## Thumbnail Versioning

```rust
/// Increment when adding new format support or changing thumbnail generation
const THUMB_VER: u32 = 1;
```

Each Value stores `thumb_version`. On thumbnail access via `thumbnail_paths()`:

```rust
fn thumbnail_paths(key) -> HashMap<String, PathBuf> {
    let value = get(key);
    let mut result = HashMap::new();

    for attachment in &value.attachments {
        if is_supported_image(&attachment.filename) {
            // Regenerate if version outdated
            if value.thumb_version < THUMB_VER {
                let _ = generate_thumbnail(key, &attachment.filename);
            }
            result.insert(attachment.filename, thumbnail_path(key, &attachment.filename));
        }
    }

    if value.thumb_version < THUMB_VER {
        update_thumb_version(key, THUMB_VER);
    }

    result
}
```

Benefits:

- Automatic upgrade on app update
- No per-attachment thumbnail state tracking
- Failed generations don't retry until next THUMB_VER bump
- Bulk access returns all thumbnail paths at once

## Configuration

```rust
struct Config {
    base_path: PathBuf,
    trash_ttl: Duration,   // default: 30 days
    purge_ttl: Duration,   // default: 7 days
}
```

## API

### Core Lifecycle

```rust
impl KevaCore {
    /// Opens or creates storage at configured path
    fn open(config: Config) -> Result<Self, KevaError>;
}
```

### Key Operations

```rust
impl KevaCore {
    /// Retrieve value by key (does NOT update last_accessed)
    fn get(&self, key: &Key) -> Result<Option<Value>, KevaError>;

    /// List all Active keys
    fn active_keys(&self) -> Result<Vec<Key>, KevaError>;

    /// List all Trash keys
    fn trashed_keys(&self) -> Result<Vec<Key>, KevaError>;

    /// Update last_accessed timestamp, returns updated Value
    fn touch(&mut self, key: &Key, now: SystemTime) -> Result<Value, KevaError>;

    /// Rename key. Returns DestinationExists error if target exists.
    fn rename(
        &mut self,
        old_key: &Key,
        new_key: &Key,
        now: SystemTime,
    ) -> Result<(), KevaError>;
}
```

### Content Operations

```rust
impl KevaCore {
    /// Get content file path for FileSystemHandle
    /// Path is derived: content/{key_hash}.md
    fn content_path(&self, key: &Key) -> PathBuf;

    /// Create key with empty content.md, returns the new Value
    /// Returns error if key already exists
    fn create(&mut self, key: &Key, now: SystemTime) -> Result<Value, KevaError>;
}
```

Note: Content modification tracking is handled by calling `touch()` after saving content via FileSystemHandle.

### Attachment Operations

Note: To list attachments, use `get(key)?.attachments`.

```rust
impl KevaCore {
    /// Get path to specific attachment
    fn attachment_path(&self, key: &Key, filename: &str) -> PathBuf;

    /// Add attachments with optional conflict resolution per file.
    /// If resolution is None, defaults to Rename on conflict.
    /// Returns the updated Value.
    fn add_attachments(
        &mut self,
        key: &Key,
        files: &[(PathBuf, Option<AttachmentConflictResolution>)],
        now: SystemTime,
    ) -> Result<Value, KevaError>;

    /// Remove attachment by filename
    fn remove_attachment(
        &mut self,
        key: &Key,
        filename: &str,
        now: SystemTime,
    ) -> Result<(), KevaError>;

    /// Rename attachment. Returns DestinationExists if new_filename already exists.
    fn rename_attachment(
        &mut self,
        key: &Key,
        old_filename: &str,
        new_filename: &str,
        now: SystemTime,
    ) -> Result<(), KevaError>;
}
```

### Thumbnail Operations

```rust
impl KevaCore {
    /// Get thumbnail paths for all supported image attachments.
    /// Regenerates thumbnails if version is outdated.
    /// Returns filename → thumbnail path map.
    fn thumbnail_paths(
        &mut self,
        key: &Key,
    ) -> Result<HashMap<String, PathBuf>, KevaError>;
}
```

### Trash Operations

```rust
impl KevaCore {
    /// Move key to Trash
    fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), KevaError>;

    /// Restore key from Trash to Active
    fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), KevaError>;

    /// Permanently delete key and all associated files
    fn purge(&mut self, key: &Key) -> Result<(), KevaError>;
}
```

### Maintenance

```rust
impl KevaCore {
    /// Run garbage collection
    /// - Moves Active → Trash based on trash_ttl
    /// - Purges Trash items based on purge_ttl
    /// - Cleans orphaned blob/thumbnail/content files
    fn maintenance(&mut self, now: SystemTime) -> Result<MaintenanceOutcome, KevaError>;
}
```

## Types

### AttachmentConflictResolution

```rust
enum AttachmentConflictResolution {
    Overwrite,  // Replace existing file
    Rename,     // Auto-generate "file (1).ext"
    Skip,       // Skip this file
}
```

### MaintenanceOutcome

```rust
struct MaintenanceOutcome {
    keys_trashed: Vec<Key>,
    keys_purged: Vec<Key>,
    orphaned_files_removed: usize,
}
```

## Errors

### KevaError

Top-level error type. Most specific errors are in DatabaseError.

```rust
enum KevaError {
    Database(DatabaseError),
    FileStorage(FileStorageError),
    DestinationExists,      // Rename target exists (key or attachment)
}
```

### DatabaseError

```rust
enum DatabaseError {
    Redb(redb::DatabaseError),
    TableError(redb::TableError),
    StorageError(redb::StorageError),
    TransactionError(redb::TransactionError),
    CommitError(redb::CommitError),
    Io(std::io::Error),
    NotFound,
    Trashed,                        // Operation requires Active key
    NotTrashed,                     // restore() called on non-trashed key
    AlreadyExists,                  // create() called on existing key
    AttachmentNotFound(String),
    AttachmentExists(String),
}
```

### FileStorageError

```rust
enum FileStorageError {
    Io(std::io::Error),
    IsDirectory,
    NonUtf8FileName,
    Image(image::ImageError),
    Resize(fast_image_resize::ResizeError),
    UnsupportedFormat,
}
```

## Lifecycle

### State Transitions

```
                 trash()
    Active ─────────────────────► Trash
       │                        │
       │                        │ restore()
       │                        ▼
       │                      Active
       │
       │  trash_ttl expires     │  purge_ttl expires
       │  (via maintenance)     │  (via maintenance)
       ▼                        ▼
     Trash ──────────────────────► Purged (deleted)
                purge()
```

### Timestamp Updates

The simplified timestamp model stores `last_accessed` in the Active state and `trashed_at` in the Trash state.

| Operation                    | last_accessed | trashed_at |
|------------------------------|---------------|------------|
| create()                     | Set           | -          |
| Add/remove/rename attachment | Set           | -          |
| rename()                     | Set           | -          |
| touch()                      | Set           | -          |
| trash()                      | -             | Set        |
| restore()                    | Set           | Clear      |

### Touch Semantics

`touch()` is called explicitly by UI on:

- Selecting key in left pane
- Copying to clipboard
- Content modification (debounced save, key switch, app exit)

`get()` does NOT auto-touch. This allows reading without affecting TTL.

## Thread Safety

keva_core is designed for single-threaded access from a worker thread. The Windows implementation uses:

```
Main Thread ──► mpsc::channel ──► Worker Thread ──► KevaCore
```

All keva_core operations happen on the worker thread. Results are posted back to main thread.