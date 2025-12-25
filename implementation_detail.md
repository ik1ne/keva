# Implementation Details

This document describes the current implementation status of Keva based on codebase analysis.

## Project Structure

```
keva/
├── core/           # keva_core - Core storage library (IMPLEMENTED)
├── search/         # keva_search - Fuzzy search library (IMPLEMENTED)
│   └── src/
│       ├── lib.rs      # Public exports
│       ├── engine.rs   # SearchEngine struct
│       ├── index.rs    # Internal Index (nucleo wrapper)
│       ├── config.rs   # SearchConfig, CaseMatching
│       ├── query.rs    # SearchQuery enum
│       ├── results.rs  # SearchResults struct
│       └── tests.rs    # All tests (32 tests)
├── ffi/            # keva_ffi - C FFI bindings for macOS (PLACEHOLDER)
├── keva_windows/   # Windows GUI app (IN PROGRESS)
│   └── src/
│       ├── main.rs     # Window creation, message loop, tray icon
│       ├── app.rs      # App state, keva_core integration
│       └── renderer.rs # Direct2D rendering
├── app-macos/      # macOS GUI app (NOT STARTED)
├── Spec.md         # Functional specification
├── Planned.md      # Future features (CLI, rich formats, etc.)
└── todo.md         # Implementation plan and progress
```

---

## Core Library (`keva_core`)

### Module Overview

| Module      | Path                    | Description                                               |
|-------------|-------------------------|-----------------------------------------------------------|
| `core/`     | `core/src/core/`        | Main `KevaCore` struct, database operations, file storage |
| `types/`    | `core/src/types/`       | Key, Value, Config, TTL types                             |
| `clipboard` | `core/src/clipboard.rs` | Clipboard read/write operations                           |

---

## Implemented Features

### 1. Storage Layer (redb)

- **Main Table**: `Key` → `VersionedValue` mapping
- **TTL Tables**:
    - `TRASHED_TTL`: Tracks Active keys for auto-trash (based on `last_accessed`)
    - `PURGED_TTL`: Tracks Trash keys for permanent deletion (based on `trashed_at`)
- **Transactions**: ACID-compliant read/write transactions
- **Range Queries**: BTree ordering for prefix-based listing

### 2. Value Types

| Type  | Storage        | Description                                         |
|-------|----------------|-----------------------------------------------------|
| Text  | Inline or Blob | Plain text content                                  |
| Files | Inline or Blob | One or more files with original filenames preserved |

- **Inline Threshold**: Configurable (default 1MB)
- **Blob Storage**: Content-addressable with BLAKE3 hashing
- **Blob Path Format**: `blobs/{key_hash}/{content_hash}/{filename}`

### 3. Key System

- **Format**: Flat UTF-8 strings
- **Max Length**: 256 characters
- **Special Characters**: `/` has no special meaning (cosmetic grouping only)

### 4. Lifecycle Management

Three lifecycle states implemented:

```
Active  ──[trash_ttl expires]──►  Trash  ──[purge_ttl expires]──►  Purge (deleted)
   │                                 │
   └──[user trash]──►                └──[user restore]──► Active
```

**Timestamps tracked**:

- `created_at` - When key was first created
- `updated_at` - When value was last modified
- `last_accessed` - When key was last viewed/copied (drives trash TTL)
- `trashed_at` - When key was moved to Trash (drives purge TTL)

**TTL Enforcement**: GC is the single source of truth for state transitions. Stale entries remain visible until GC runs.

### 5. Garbage Collection

- Moves expired Active keys → Trash
- Permanently deletes expired Trash keys
- Removes orphan blob files
- Triggered manually via `gc(now)` method

### 6. Search Engine (`keva_search`)

**Status:** ✅ Implemented

Uses `nucleo` library for fuzzy matching. Shared between Windows (direct) and macOS (via FFI).

**Architecture:**

- Two independent fuzzy indexes: **Active** and **Trash**
- Append-only design with tombstone-based deletion
- Periodic rebuild during maintenance when tombstones exceed threshold

**Features:**

- Fuzzy matching with configurable case matching (Sensitive, Insensitive, Smart)
- Non-blocking API for responsive UI (`set_query()`, `tick()`, `is_finished()`)
- Separate indexes for Active and Trash keys
- Zero-copy iteration over search results

**Public API:**

```rust
// Create
SearchEngine::new(active, trashed, config, notify)

// Mutation
engine.add_active(key)
engine.trash( & key)
engine.restore( & key)
engine.remove( & key)
engine.rename( & old, new)

// Search
engine.set_query(SearchQuery::Fuzzy(pattern))
engine.tick()  // Non-blocking
engine.is_finished()
engine.active_results().iter()
engine.trashed_results().iter()

// Maintenance
engine.maintenance_compact()
```

### 7. Clipboard Integration

- Platform-agnostic via `clipboard-rs`
- Multi-format detection
- **Priority Rule**: Files take priority over text when both present
- Operations: `import_clipboard()`, `copy_to_clipboard()`

---

## KevaCore API

### Read Operations

| Method           | Description         |
|------------------|---------------------|
| `get(key)`       | Retrieve value      |
| `active_keys()`  | Get all Active keys |
| `trashed_keys()` | Get all Trash keys  |

### Write Operations

| Method                          | Description                 |
|---------------------------------|-----------------------------|
| `upsert_text(key, text, now)`   | Create or update text value |
| `add_files(key, paths, now)`    | Add files to a key          |
| `remove_file_at(key, idx, now)` | Remove file by index        |
| `touch(key, now)`               | Update last_accessed        |

### Lifecycle Operations

| Method              | Description                  |
|---------------------|------------------------------|
| `trash(key, now)`   | Soft-delete to Trash state   |
| `restore(key, now)` | Restore from Trash to Active |
| `purge(key)`        | Permanently delete           |

### Key Management Operations

| Method                                | Description |
|---------------------------------------|-------------|
| `rename(old_key, new_key, overwrite)` | Rename key  |

### Clipboard Operations

| Method                        | Description                                       |
|-------------------------------|---------------------------------------------------|
| `copy_to_clipboard(key, now)` | Copy value to clipboard (updates `last_accessed`) |
| `import_clipboard(key, now)`  | Import clipboard content to key                   |

### Maintenance

| Method             | Description                    |
|--------------------|--------------------------------|
| `maintenance(now)` | Run GC and orphan blob cleanup |

---

## Configuration (`KevaConfig`)

| Setting            | Default      | Description                                     |
|--------------------|--------------|-------------------------------------------------|
| `inline_threshold` | 1 MB         | Files smaller than this are stored inline in DB |
| `trash_ttl`        | Configurable | Duration before Active → Trash                  |
| `purge_ttl`        | Configurable | Duration before Trash → Purge                   |

---

## Versioning System

- Values stored as `VersionedValue` enum
- Current version: **V1**
- V1 uses BLAKE3 hashing for content addressing
- Designed for forward-compatible schema evolution

---

## Dependencies

### keva_core

| Crate          | Version | Purpose                 |
|----------------|---------|-------------------------|
| `redb`         | 3.x     | Embedded ACID database  |
| `blake3`       | 1.x     | Content hashing         |
| `clipboard-rs` | 0.3     | Clipboard I/O           |
| `postcard`     | -       | Binary serialization    |
| `serde`        | -       | Serialization framework |
| `nutype`       | 0.6     | Validated string types  |
| `thiserror`    | -       | Error handling          |

### keva_search

| Crate       | Version | Purpose        |
|-------------|---------|----------------|
| `keva_core` | path    | Key type       |
| `nucleo`    | 0.5     | Fuzzy matching |

### keva_windows

| Crate     | Version | Purpose                    |
|-----------|---------|----------------------------|
| `windows` | 0.62    | Win32 API, Direct2D, Shell |

---

## Test Coverage

| Test Module    | Location                    | Coverage                         |
|----------------|-----------------------------|----------------------------------|
| Database tests | `core/src/core/db/tests.rs` | CRUD, TTL, GC, transactions      |
| KevaCore tests | `core/src/core/tests.rs`    | Integration tests (132 tests)    |
| Type tests     | `core/src/types/*/tests.rs` | Key, Value, Config validation    |
| Search tests   | `search/src/tests.rs`       | All search operations (32 tests) |

---

## Implementation Status

### Windows App (`keva_windows`)

**M1-win Complete** - Window skeleton with tray integration.

| Feature                        | Status         | Milestone |
|--------------------------------|----------------|-----------|
| Borderless window              | ✅ Complete    | M1        |
| System tray icon               | ✅ Complete    | M1        |
| Tray left-click toggles window | ✅ Complete    | M1        |
| Tray right-click menu          | ✅ Complete    | M1        |
| Resize from edges (5px)        | ✅ Complete    | M1        |
| Esc hides window               | ✅ Complete    | M1        |
| Alt+Tab visibility             | ✅ Complete    | M1        |
| Direct2D rendering             | ✅ Complete    | M1        |
| Layout (search/left/right)     | ❌ Pending     | M2        |
| keva_core integration          | ❌ Pending     | M3        |
| Key list display               | ❌ Pending     | M3        |
| Text preview (Rich Edit)       | ❌ Pending     | M5        |
| File preview (IPreviewHandler) | ❌ Pending     | M13       |
| Clipboard paste to create      | ❌ Pending     | M6        |
| Fuzzy search                   | ❌ Pending     | M7        |
| Global hotkey                  | ❌ Pending     | M16       |
| Settings dialog                | ❌ Pending     | M15       |

### macOS App (`app-macos`)

| Feature          | Status    |
|------------------|-----------|
| FFI layer        | ⏳ Pending |
| App skeleton     | ⏳ Pending |
| Core integration | ⏳ Pending |

### From Planned.md (Future Scope)

| Feature                                 | Status          |
|-----------------------------------------|-----------------|
| CLI interface                           | Not v1 scope    |
| Regex search mode                       | Not implemented |
| Rich format support (HTML, RTF, images) | Not implemented |
| Value content search                    | Not implemented |

---

## Architecture Summary

```
keva_core (Storage Layer - Rust)
├── Database (redb)
│   ├── Main Table: Key → VersionedValue
│   ├── TRASHED_TTL Table
│   └── PURGED_TTL Table
├── FileStorage
│   ├── Inline data (< threshold) in DB
│   └── Blob data with BLAKE3 addressing
└── Clipboard
    └── Cross-platform I/O

keva_search (Fuzzy Search - Rust)
├── SearchEngine (dual Active/Trash indexes)
├── Index (nucleo wrapper with tombstones)
└── Non-blocking API for GUI integration

keva_windows (Windows GUI - Rust)
├── Win32 API via `windows` crate
├── Direct2D for custom rendering
├── DirectWrite for text
├── keva_search for fuzzy search
└── Native controls (Rich Edit, IPreviewHandler)

app-macos (macOS GUI - Swift, planned)
├── AppKit/Cocoa
├── FFI to keva_core and keva_search via keva_ffi
└── Native controls (NSTextView, QLPreviewView)
```

**Key Design Decisions**:

1. **Content-Addressable Storage**: BLAKE3 enables deduplication
2. **GC as Source of Truth**: State transitions only happen during maintenance
3. **Hybrid Native UI**: Platform-specific apps for best UX, shared core
4. **Single Writer Model**: redb supports multi-reader/single-writer
5. **Versioned Values**: Supports future schema migrations

**Windows-Specific Notes**:

- Taskbar icon remains visible (hiding breaks Alt+Tab - Windows limitation)
- Uses Direct2D + DirectWrite for custom key list rendering
- Native Rich Edit control for text preview
- IPreviewHandler for file preview (same as Explorer)
