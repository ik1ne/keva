# Implementation Details

This document describes the current implementation status of Keva based on codebase analysis.

## Project Structure

```
keva/
├── core/           # keva_core - Core storage library (IMPLEMENTED)
├── cli/            # keva_cli - Future scope, not v1 (placeholder in workspace)
├── gui/            # keva_gui - GUI application (PLACEHOLDER)
├── Spec.md         # Functional specification (v1 = GUI-only)
└── Planned.md      # Future features (CLI, rich formats, etc.)
```

> **Note**: The workspace `Cargo.toml` includes `cli` member for future development, but CLI is not part of v1 scope.

---

## Core Library (`keva_core`)

### Module Overview

| Module      | Path                    | Description                                               |
|-------------|-------------------------|-----------------------------------------------------------|
| `core/`     | `core/src/core/`        | Main `KevaCore` struct, database operations, file storage |
| `types/`    | `core/src/types/`       | Key, Value, Config, TTL types                             |
| `clipboard` | `core/src/clipboard.rs` | Clipboard read/write operations                           |
| `search`    | `gui/src/search/`       | Fuzzy search engine (lives in GUI crate)                  |

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

### 6. Search Engine

Lives in `keva_gui` crate for non-blocking UI integration.

**Fuzzy Search**:

- Uses `nucleo` library
- Non-blocking API: `set_query()` + `tick()` + zero-copy iteration
- Smart case matching (case-insensitive unless query contains uppercase)
- Callback-based notification for UI updates

**Index Management**:

- Incremental updates via mutation methods (`add_active`, `trash`, `restore`, etc.)
- Separate indexes for Active and Trash keys
- Tombstone tracking for pending deletions

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

### keva_gui

| Crate    | Version | Purpose      |
|----------|---------|--------------|
| `nucleo` | 0.5     | Fuzzy search |

---

## Test Coverage

| Test Module    | Location                    | Coverage                      |
|----------------|-----------------------------|-------------------------------|
| Database tests | `core/src/core/db/tests.rs` | CRUD, TTL, GC, transactions   |
| KevaCore tests | `core/src/core/tests.rs`    | Integration tests (132 tests) |
| Type tests     | `core/src/types/*/tests.rs` | Key, Value, Config validation |
| Search tests   | `gui/src/search/tests.rs`   | Fuzzy search, index mutations |

---

## Not Implemented

### From Spec.md (v1 Scope - GUI Only)

| Feature           | Status                                      |
|-------------------|---------------------------------------------|
| GUI application   | Placeholder only (`"Hello from keva_gui!"`) |
| macOS .app bundle | Not started                                 |
| Windows installer | Not started                                 |

### From Planned.md (Future Scope)

| Feature                                 | Status                           |
|-----------------------------------------|----------------------------------|
| CLI interface                           | Placeholder exists, not v1 scope |
| Regex search mode                       | Not implemented                  |
| Rich format support (HTML, RTF, images) | Not implemented                  |
| Value content search                    | Not implemented                  |

---

## Architecture Summary

```
keva_core (Storage Layer)
├── Database (redb)
│   ├── Main Table: Key → VersionedValue
│   ├── TRASHED_TTL Table
│   └── PURGED_TTL Table
├── FileStorage
│   ├── Inline data (< threshold) in DB
│   └── Blob data with BLAKE3 addressing
└── Clipboard
    └── Cross-platform I/O

keva_gui (UI Layer)
└── SearchEngine
    ├── Nucleo fuzzy search
    ├── Non-blocking tick-based API
    └── Zero-copy result iteration
```

**Key Design Decisions**:

1. **Content-Addressable Storage**: BLAKE3 enables deduplication
2. **GC as Source of Truth**: State transitions only happen during maintenance
3. **Non-Blocking Search**: GUI-owned search with callback notifications
4. **Single Writer Model**: redb supports multi-reader/single-writer
5. **Versioned Values**: Supports future schema migrations
