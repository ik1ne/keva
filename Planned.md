# Future Plans

Features planned but not in v1 scope:

1. **macOS Application**: Native macOS app via Swift + FFI to keva_core/keva_search.

2. **Rich format support**: HTML, images, RTF, application-specific clipboard data. Includes binary output for
   programmatic access.

3. **Value content search**: Search within value contents, not just keys.

4. **Regex search mode**: Regular expression matching as alternative to fuzzy search.

5. **CLI interface**: Command-line interface for scripting and automation.

6. **Native file preview**: IPreviewHandler on Windows, Quick Look on macOS.

---

## macOS Application (Reference)

Preserved from v1 planning for future implementation.

### Architecture

- `keva_ffi` crate: C FFI bindings exposing keva_core and keva_search
- Swift app using FFI via bridging header
- Build: Swift Package Manager or xcodebuild (no Xcode IDE required)

### FFI Crate

```toml
[package]
name = "keva_ffi"

[lib]
crate-type = ["cdylib"]

[dependencies]
keva_core = { path = "../core" }
keva_search = { path = "../search" }

[build-dependencies]
cbindgen = "0.27"
```

### C API

```c
// Lifecycle
KevaHandle* keva_open(const char* path);
void keva_close(KevaHandle* handle);

// CRUD
int32_t keva_set_text(KevaHandle* h, const char* key, const char* text);
int32_t keva_set_files(KevaHandle* h, const char* key, const char** paths, size_t count);
KevaValue* keva_get(KevaHandle* h, const char* key);
int32_t keva_delete(KevaHandle* h, const char* key);
int32_t keva_rename(KevaHandle* h, const char* old_key, const char* new_key);

// Listing
KevaKeyList* keva_list_keys(KevaHandle* h);

// Memory
void keva_free_value(KevaValue* value);
void keva_free_key_list(KevaKeyList* list);
```

### macOS-Specific UI

- Borderless window (NSWindow, styleMask)
- Menu bar icon (NSStatusItem)
- LSUIElement=true in Info.plist (hide from Dock/Cmd+Tab)
- Cmd+Shift+K global shortcut
- Text preview: NSTextView
- File preview: QLPreviewView
- Clipboard: NSPasteboard
- Launch at Login: SMAppService

---

## CLI Specification (Reference)

Preserved from v1 planning for future implementation.

### CLI Alias

`kv`

### Data Operations

- `get <key>`: Output the plain text value to stdout. Outputs empty string if no plain text exists.
    - `--raw`: Output the rich format as binary to stdout.
- `set <key> <value>`: Set the plain text value for the key.
- `rm <key>`: Remove the key.
    - `-r` / `--recursive`: Delete the key and all its children (keys matching `<key>` and `<key>/*`).
    - `--trash`: Force soft delete (move to Trash).
    - `--permanent`: Force immediate, permanent deletion.
- `mv <key> <new_key>`: Rename a key without modifying its value. Fails if `<new_key>` already exists unless `--force`
  is specified.
    - `--force`: Overwrite existing key at destination.
- `ls [prefix]`: List all keys matching the prefix (or all keys if no prefix given).
    - `--include-trash`: Include trashed items in results (hidden by default).
- `import <key>`: Import current clipboard content to the key.
    - When clipboard contains both files and text, **files take priority** (text is discarded).
- `copy <key>`: Copy the key's value to the clipboard. Also updates `last_accessed` timestamp.
- `gc`: Manually trigger garbage collection.

### Search Command

- `search <query>`: Search the database for keys matching the query.
    - `--fuzzy` (default): Use fuzzy matching.
    - `--regex`: Use regular expression matching.