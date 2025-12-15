# Technical Notes: Keva Implementation Hints

This document captures implementation decisions and research findings. Not part of the spec—intended as developer
reference.

## 1. Target Platforms

- macOS
- Windows

## 2. Language

Rust (pure Rust preferred to minimize dependencies)

## 3. Recommended Libraries

| Component        | Library       | Notes                                                                                        |
|------------------|---------------|----------------------------------------------------------------------------------------------|
| Storage          | redb          | Pure Rust, ACID, single-writer/multi-reader, 3.8x faster individual writes than alternatives |
| Full-text search | tantivy       | Pure Rust, Lucene-inspired, production-ready                                                 |
| Fuzzy search     | nucleo        | Path-aware scoring, incremental search, parallel matching                                    |
| GUI framework    | egui or Slint | egui: faster cold start (~250ms); Slint: smaller binary, more mature API                     |
| Clipboard        | clipboard-rs  | Multi-format support (text, HTML, RTF, images, file lists)                                   |
| Global hotkey    | global-hotkey | Cross-platform, maintained by Tauri team                                                     |

## 4. Storage Architecture

```
~/.keva/
  keva.redb              # Metadata + inlined values (text/small files)
  blobs/
    {key_path}/          # Organized by key
      {content_hash}/    # Content-addressable subdirectory
        {filename}       # Original filename preserved
      text.txt           # Blob-stored text (when too large to inline)
    temp_inline/         # Temporary cache for ensure_*_path methods
      {key_path}/...
```

- Inline threshold configurable (`inline_threshold_bytes`) - small data stored in redb, large data as blob files
- Use BLAKE3 for content hashing (fast, secure)
- redb handles concurrent access via file locking
- Original filenames preserved for blob-stored files

## 5. Performance Targets

| Operation                 | Target |
|---------------------------|--------|
| GUI visible (with daemon) | <100ms |
| GUI visible (cold start)  | <500ms |
| First keystroke response  | <50ms  |
| Subsequent keystrokes     | <16ms  |
| Value preview (text)      | <50ms  |
| Value preview (image)     | <200ms |

## 6. Fuzzy Search Optimization

### Incremental Filtering

Cache previous results; filter from cache when query extends:

```
"a"   → full scan, cache results
"ab"  → filter cached "a" results
"abc" → filter cached "ab" results
"ab"  → use cached "ab" results (backspace)
"abd" → full scan (different branch)
```

### Scoring Tiers (nucleo handles this)

1. Exact match
2. Exact match (case-insensitive)
3. Prefix match
4. Child path match (query `a/b`, key `a/b/c`)
5. Substring match
6. Subsequence match (with bonuses for consecutive chars, word boundaries)

## 7. Daemon Architecture

### Single-Process Model

GUI runs inside daemon process (no IPC needed):

```
┌─────────────────────────────────┐
│         Daemon Process          │
│  ┌───────────┐  ┌────────────┐  │
│  │  Hotkey   │  │   Window   │  │
│  │ Listener  │──│  (hidden)  │  │
│  └───────────┘  └────────────┘  │
│         │                       │
│  ┌──────┴──────────────────┐   │
│  │     Storage Access      │   │
│  └─────────────────────────┘   │
└─────────────────────────────────┘
```

### Lifecycle

1. `kv daemon start` or `kv gui` → spawn daemon process
2. Create hidden window, register hotkey
3. Hotkey pressed → `window.set_visible(true)`
4. Escape/blur → `window.set_visible(false)`, run GC
5. `kv daemon stop` → graceful shutdown

### Launch at Login

- macOS: Write plist to `~/Library/LaunchAgents/com.keva.daemon.plist`
- Windows: Task Scheduler via `schtasks.exe` or COM API

### Single Instance Enforcement

- PID file at `~/.keva/daemon.pid`
- Or named mutex (Windows) / Unix socket (macOS)

## 8. Clipboard Handling

### Detection Priority

```rust
if clipboard.has_rich_format() {
store_rich_format()
if clipboard.has_meaningful_plain_text() {
store_plain_text()
}
} else {
store_plain_text()
}
```

### File List Handling

When clipboard contains file paths (copy from Finder/Explorer):

1. Calculate total size
2. If > threshold, prompt user
3. Read file contents and store as blob
4. Store file metadata (original name, MIME type)

## 9. Binary Optimization

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

Expected binary size: ~2-4MB

## 10. Concurrency Notes

### redb Behavior

- Multiple readers: allowed concurrently
- Single writer: blocks other writers until commit
- Read during write: sees pre-commit state

### CLI + Daemon Coexistence

Both can run simultaneously. Potential brief blocking on concurrent writes (typically <10ms for small operations).

## 11. GC Implementation

```rust
fn gc() {
    // Phase 1: Update timestamps (fast, in transaction)
    let purge_candidates = db.query("WHERE purge_at < now()");

    // Phase 2: Delete blobs (slow, outside transaction)
    for key in purge_candidates {
        if !any_other_key_references(key.blob_hash) {
            delete_blob_file(key.blob_hash);
        }
    }

    // Phase 3: Compact if needed
    if space_reclaimed > threshold {
        db.compact();
    }
}
```

## 12. Open Questions

- [ ] Default global shortcut key (platform-specific?)
- [x] TTL values - configurable via `SavedConfig` (`trash_ttl`, `purge_ttl`)
- [x] Maximum key length - 256 characters (`MAX_KEY_LENGTH`)
- [ ] Maximum value size (for plain text stored in redb) - currently limited by `inline_threshold_bytes`
- [x] Inline threshold - configurable via `SavedConfig` (`inline_threshold_bytes`)