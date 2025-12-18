# keva_core Implementation TODO

## Summary

Implement two internal modules in keva_core:

1. **Clipboard** - Read/write system clipboard
2. **Search** - Fuzzy (nucleo) + Regex hybrid search

All public API exposed through `KevaCore` methods.

---

## Clipboard Module (`clipboard.rs`)

### Design

- Internal module (`pub(crate)`)
- Uses `clipboard-rs` crate (already in Cargo.toml)

### Read Priority

When clipboard contains both files and text, **files take priority** - text is discarded.

### Internal API

```rust
pub(crate) enum ClipboardContent {
    Text(String),
    Files(Vec<PathBuf>),
}

pub(crate) fn read_clipboard() -> Result<ClipboardContent, ClipboardError>;
pub(crate) fn write_text(text: &str) -> Result<(), ClipboardError>;
pub(crate) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError>;
```

### KevaCore Public Methods

```rust
impl KevaCore {
    /// Import clipboard content to a key
    /// Files take priority over text when both are present
    pub fn import_clipboard(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;

    /// Copy key's value to clipboard and update access time
    pub fn copy_to_clipboard(&mut self, key: &Key, now: SystemTime) -> Result<(), StorageError>;
}
```

---

## Search Module (`search.rs`)

### Design

- Internal module (`pub(crate)`)
- Uses `nucleo` for fuzzy search (already in Cargo.toml)
- Uses `regex` for regex mode (**add to Cargo.toml**)
- SearchEngine owned by KevaCore with **incremental updates**
- **No auto-detection** - caller explicitly specifies search mode

### Public Types (re-exported from lib.rs)

```rust
/// Query with explicit search mode - no auto-detection
pub enum SearchQuery {
    Fuzzy(String),
    Regex(String),
}

pub struct SearchResult {
    pub key: Key,
    pub score: u32,
    pub is_trash: bool,
}

/// Case matching behavior for fuzzy search
#[derive(Debug, Clone, Copy, Default)]
pub enum CaseMatching {
    /// Always case sensitive
    Sensitive,
    /// Always case insensitive
    Insensitive,
    /// Smart case: case-insensitive unless query contains uppercase
    #[default]
    Smart,
}

/// Configuration for fuzzy search behavior
#[derive(Debug, Clone, Default)]
pub struct SearchConfig {
    pub case_matching: CaseMatching,        // Default: Smart
    pub unicode_normalization: bool,        // Default: true
}

impl SearchConfig {
    pub fn new() -> Self {
        Self {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
        }
    }
}
```

### Internal SearchEngine

```rust
pub(crate) struct SearchEngine {
    nucleo: Nucleo<SearchItem>,
    active_keys: HashSet<Key>,
    trashed_keys: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
    config: SearchConfig,
}

impl SearchEngine {
    pub(crate) fn new(
        active: Vec<Key>,
        trashed: Vec<Key>,
        config: SearchConfig,
    ) -> Self;

    // Incremental updates
    pub(crate) fn add_active(&mut self, key: Key);
    pub(crate) fn add_trashed(&mut self, key: Key);
    pub(crate) fn remove(&mut self, key: &Key);
    pub(crate) fn trash(&mut self, key: &Key);
    pub(crate) fn restore(&mut self, key: &Key);
    pub(crate) fn rename(&mut self, old: &Key, new: &Key);
    pub(crate) fn rebuild(&mut self);

    pub(crate) fn search(
        &mut self,
        query: SearchQuery,
        timeout_ms: u64,
    ) -> Result<Vec<SearchResult>, SearchError>;
}
```

### Rebuild Operation

**Why needed:** Nucleo is append-only (no remove API). Deleted items stay in Nucleo but are filtered from results.

**What it does:**

1. Clears Nucleo completely (`nucleo.restart(true)`)
2. Resets `pending_deletions` to 0
3. Re-injects only current valid keys from `active_keys` and `trashed_keys`

**When triggered:** Automatically when `pending_deletions > rebuild_threshold` (default: 100)

### KevaCore Integration

```rust
impl KevaCore {
    pub fn open(config: Config, search_config: SearchConfig) -> Result<Self, StorageError> {
        let search_engine = SearchEngine::new(
            db.active_keys()?,
            db.trashed_keys()?,
            search_config,
        );
        // ...
    }

    // Update search engine on mutations
    pub fn upsert_text(...) { /* ... */ self.search_engine.add_active(key); }
    pub fn trash(...) { /* ... */ self.search_engine.trash(key); }
    pub fn restore(...) { /* ... */ self.search_engine.restore(key); }
    pub fn purge(...) { /* ... */ self.search_engine.remove(key); }
    pub fn rename(...) { /* ... */ self.search_engine.rename(old, new); }
    pub fn gc(...) { /* ... update trashed/purged keys ... */ }

    /// Search with explicit mode (no auto-detection)
    pub fn search(
        &mut self,
        query: SearchQuery,
        timeout_ms: u64,
    ) -> Result<Vec<SearchResult>, StorageError>;
}
```

---

## Metadata Changes

### Add `last_accessed` Timestamp

Update value metadata to include `last_accessed`:

```rust
pub struct Metadata {
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub last_accessed: SystemTime,  // NEW
    pub trashed_at: Option<SystemTime>,
    pub lifecycle_state: LifecycleState,
}
```

### TTL Based on `last_accessed`

**Change:** TTL expiration is now based on `last_accessed`, not `updated_at`.

Operations that update `last_accessed`:

- `get()` - reading a value
- `copy_to_clipboard()` - copying value to clipboard

This extends the lifetime of frequently accessed entries.

### Effective Lifecycle State Calculation

```rust
fn effective_lifecycle_state(&self, value: &Value, now: SystemTime) -> LifecycleState {
    match value.metadata.lifecycle_state {
        LifecycleState::Active => {
            // Use last_accessed instead of updated_at
            let expires_at = value.metadata.last_accessed + self.config.saved.trash_ttl;
            if expires_at <= now {
                LifecycleState::Trash
            } else {
                LifecycleState::Active
            }
        }
        LifecycleState::Trash => {
            // ... same as before ...
        }
        LifecycleState::Purge => LifecycleState::Purge,
    }
}
```

---

## Dependencies

**Already present:**

- `clipboard-rs = "0.3"`
- `nucleo = "0.5"`

**To add:**

- `regex = "1"`

---

## Files to Modify

| File                                         | Changes                                                         |
|----------------------------------------------|-----------------------------------------------------------------|
| `core/Cargo.toml`                            | Add `regex = "1"`                                               |
| `core/src/clipboard.rs`                      | Implement (currently empty)                                     |
| `core/src/search.rs`                         | Implement (currently empty)                                     |
| `core/src/core/mod.rs`                       | Add `search_engine` field, new methods, update existing methods |
| `core/src/lib.rs`                            | Re-export `SearchQuery`, `SearchResult`, `SearchConfig`         |
| `core/src/types/value/versioned_value/v1.rs` | Add `last_accessed` to metadata                                 |
| `core/src/core/db/mod.rs`                    | Update `get()` to touch `last_accessed`                         |

---

## Implementation Order

1. Add `regex` dependency
2. Add `last_accessed` to metadata (versioned value migration)
3. Implement `clipboard.rs`
4. Implement `search.rs` with `SearchConfig`
5. Update `KevaCore` with clipboard methods (`import_clipboard`, `copy_to_clipboard`)
6. Update `KevaCore` with search engine integration
7. Update `get()` and `copy_to_clipboard()` to update `last_accessed`
8. Update existing `KevaCore` methods to call search engine deltas
9. Tests
