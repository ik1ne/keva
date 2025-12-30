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
    created_at: SystemTime,
    updated_at: SystemTime,
    last_accessed: SystemTime,
    trashed_at: Option<SystemTime>,
    lifecycle_state: LifecycleState,
}
```

### LifecycleState

```rust
enum LifecycleState {
    Active,
    Trash,
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
- **Hard maximum: 1 GB per file** (enforced by core)

### Thumbnails

Generated previews stored at `thumbnails/{key_hash}/{filename}.thumb`.

- Supported formats: png, jpg, jpeg, gif, webp, svg
- Version-controlled regeneration (see Thumbnail Versioning)
- Missing thumbnail → fallback to icon in UI

## Thumbnail Versioning

```rust
/// Increment when adding new format support or changing thumbnail generation
const THUMB_VER: u32 = 1;
```

Each Value stores `thumb_version`. On thumbnail access:

```rust
fn get_thumbnail_path(key, filename) -> Option<PathBuf> {
    let value = get(key);

    if value.thumb_version < THUMB_VER {
        // Regenerate all thumbnails for this key
        for attachment in &value.attachments {
            if is_supported_image(&attachment.filename) {
                // Ignore failure - format might not be supported yet
                let _ = try_generate_thumbnail(key, &attachment.filename);
            }
        }
        // Update version regardless of individual success/failure
        update_thumb_version(key, THUMB_VER);
    }

    let thumb_path = thumbnails_dir / key_hash / format!("{}.thumb", filename);
    if thumb_path.exists() {
        Some(thumb_path)
    } else {
        None
    }
}
```

Benefits:

- Automatic upgrade on app update
- No per-attachment thumbnail state tracking
- Failed generations don't retry until next THUMB_VER bump

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
    fn open(config: Config) -> Result<Self, StorageError>;
}
```

### Key Operations

```rust
impl KevaCore {
    /// Retrieve value by key (does NOT update last_accessed)
    fn get(&self, key: &Key) -> Result<Option<Value>, StorageError>;

    /// List all Active keys
    fn active_keys(&self) -> Result<Vec<Key>, StorageError>;

    /// List all Trash keys
    fn trashed_keys(&self) -> Result<Vec<Key>, StorageError>;

    /// Update last_accessed timestamp
    fn touch(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;

    /// Rename key (optionally overwrite existing)
    fn rename(
        &mut self,
        old: &Key,
        new: &Key,
        overwrite: bool,
        now: SystemTime,
    ) -> Result<(), StorageError>;
}
```

### Content Operations

```rust
impl KevaCore {
    /// Get content file path for FileSystemHandle
    /// Path is derived: content/{key_hash}.md
    /// Requires Value to ensure key exists (file guaranteed to exist)
    fn get_content_path(&self, key: &Key, _value: &Value) -> PathBuf;

    /// Create key with empty content.md
    /// Returns error if key already exists
    fn create(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;

    /// Mark content as modified (updates updated_at and last_accessed)
    /// Called by UI on debounced save, key switch, or app exit
    fn mark_content_modified(
        &mut self,
        key: &Key,
        now: SystemTime
    ) -> Result<(), StorageError>;
}
```

### Attachment Operations

Note: To list attachments, use `get(key)?.attachments`.

```rust
impl KevaCore {
    /// Get path to specific attachment
    fn get_attachment_path(
        &self,
        key: &Key,
        filename: &str
    ) -> Result<PathBuf, StorageError>;

    /// Add attachments with pre-resolved conflict decisions
    /// Rejects files > 1 GB
    fn add_attachments(
        &mut self,
        key: &Key,
        files: Vec<(PathBuf, ConflictResolution)>,
        now: SystemTime,
    ) -> Result<Vec<AddResult>, StorageError>;

    /// Remove attachment by filename
    fn remove_attachment(
        &mut self,
        key: &Key,
        filename: &str,
        now: SystemTime
    ) -> Result<(), StorageError>;

    /// Rename attachment
    fn rename_attachment(
        &mut self,
        key: &Key,
        old_filename: &str,
        new_filename: &str,
        now: SystemTime,
    ) -> Result<(), StorageError>;
}
```

### Thumbnail Operations

```rust
impl KevaCore {
    /// Get thumbnail path, regenerating if version outdated
    /// Returns None for unsupported formats or if generation failed
    fn get_thumbnail_path(
        &mut self,
        key: &Key,
        filename: &str
    ) -> Result<Option<PathBuf>, StorageError>;
}
```

### Trash Operations

```rust
impl KevaCore {
    /// Move key to Trash
    fn trash(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;

    /// Restore key from Trash to Active
    fn restore(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;

    /// Permanently delete key and all associated files
    fn purge(&mut self, key: &Key) -> Result<(), StorageError>;
}
```

### Maintenance

```rust
impl KevaCore {
    /// Run garbage collection
    /// - Moves Active → Trash based on trash_ttl
    /// - Purges Trash items based on purge_ttl
    /// - Cleans orphaned blob/thumbnail files
    /// Returns list of keys that were trashed (for UI to handle)
    fn maintenance(&mut self, now: SystemTime) -> Result<MaintenanceState, StorageError>;
}
```

## Types

### ConflictResolution

```rust
enum ConflictResolution {
    Overwrite,  // Replace existing file
    Rename,     // Auto-generate "file (1).ext"
    Skip,       // Skip this file
}
```

### AddResult

```rust
enum AddResult {
    Added { filename: String },
    Renamed { original: String, actual: String },
    Skipped { filename: String },
    Overwritten { filename: String },
    Rejected { filename: String, reason: String },  // e.g., exceeds 1 GB
}
```

### MaintenanceState

```rust
struct MaintenanceState {
    keys_trashed: Vec<Key>,
    keys_purged: usize,
    orphaned_files_removed: usize,
}
```

## Errors

### StorageError

```rust
enum StorageError {
    Database(DatabaseError),
    FileStorage(FileStorageError),
    KeyIsTrashed,           // Operation requires Active key
    KeyExists,              // create() called on existing key
    AlreadyTrashed,         // Key already in Trash
    DestinationExists,      // Rename target exists (and overwrite=false)
    AttachmentNotFound,     // Referenced attachment doesn't exist
    FileTooLarge,           // File exceeds 1 GB limit
}
```

### DatabaseError

```rust
enum DatabaseError {
    NotFound,
    Trashed,
    NotTrashed,
    EmptyInput,
    Internal(redb::Error),
}
```

### FileStorageError

```rust
enum FileStorageError {
    Io(std::io::Error),
    IsDirectory,
    NonUtf8FileName,
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

| Operation                    | created_at | updated_at | last_accessed | trashed_at |
|------------------------------|------------|------------|---------------|------------|
| create()                     | Set        | Set        | Set           | -          |
| mark_content_modified()      | -          | Set        | Set           | -          |
| Add/remove/rename attachment | -          | Set        | Set           | -          |
| rename()                     | -          | Set        | Set           | -          |
| touch()                      | -          | -          | Set           | -          |
| trash()                      | -          | -          | -             | Set        |
| restore()                    | -          | -          | Set           | Clear      |

### Touch Semantics

`touch()` is called explicitly by UI on:

- Selecting key in left pane
- Copying to clipboard

`get()` does NOT auto-touch. This allows reading without affecting TTL.

### mark_content_modified Timing

Called by UI on:

- Debounced save (500ms after edit)
- Key switch (if dirty)
- App exit (if dirty)

## Thread Safety

keva_core is designed for single-threaded access from a worker thread. The Windows implementation uses:

```
Main Thread ──► mpsc::channel ──► Worker Thread ──► KevaCore
```

All keva_core operations happen on the worker thread. Results are posted back to main thread.