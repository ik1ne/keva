# Keva GUI Implementation Plan

## Context

Keva is a local key-value store for clipboard-like data. The core library (`keva_core`) is implemented in Rust.

**Architecture Decision:** Hybrid native approach.

- **Windows:** Pure Rust (`windows` crate for Win32 + Direct2D)
- **macOS:** Swift/AppKit with FFI to `keva_core`
- **Shared:** `keva_core` (Rust), `keva_search` (Rust)

**Rationale:**

- gpui/tao can't handle borderless+resize on macOS
- Windows: `windows` crate is Microsoft-maintained, well-documented
- macOS: Swift is first-class for Cocoa, QuickLook integration is trivial
- FFI overhead for keva_core is minimal (not high-frequency calls)

**Reference documents:**

- `Spec.md` - Product specification (source of truth for behavior)
- `implementation_detail.md` - keva_core API reference
- `Planned.md` - Future features (not in scope)
- `windows_crate_research.md` - Windows API research

---

## To Decide

| Question | Context | Decision |
|----------|---------|----------|
| Keep 3px inner border drag zone? | M1-win requires 3px inner border for dragging, M2-win adds search icon as drag handle. Should we keep both drag methods or remove the border drag zone? | Pending |
| Data directory location? | Currently uses ~/.keva (%USERPROFILE%\.keva on Windows). Should we use %APPDATA%\Keva instead for Windows convention? Or keep ~/.keva for cross-platform consistency? | Pending |

---

**Project structure:**

```
keva/
├── core/           # keva_core (Rust library) - IMPLEMENTED
├── search/         # keva_search (Rust library) - IMPLEMENTED
├── ffi/            # C FFI bindings for macOS (Rust, builds dylib)
├── app-windows/    # Windows app (Rust + windows crate + Direct2D)
├── app-macos/      # macOS app (Swift/AppKit, links keva_ffi)
├── Spec.md
├── Planned.md
└── implementation_detail.md
```

---

## Phase 1: Windows App (Pure Rust)

### M1-win: Window Skeleton

**Goal:** Borderless window with system tray, basic window management.

**Status:** Partial (missing tray menu, drag zone refinement)

**Requirements:**

| Requirement      | Description                                                | Status                |
|------------------|------------------------------------------------------------|-----------------------|
| Window style     | Borderless (WS_POPUP), no title bar or native controls     | ✅                     |
| Resize           | 5px outer zone triggers OS resize                          | ⚠️ 6px                |
| Drag             | 3px inner border zone moves window                         | ❌ Entire window drags |
| Initial position | Centered on primary monitor                                | ✅                     |
| Smooth resize    | DwmExtendFrameIntoClientArea enabled                       | ✅                     |
| Tray icon        | Visible with tooltip "Keva"                                | ✅                     |
| Tray left-click  | Toggle window visibility                                   | ✅                     |
| Tray right-click | Context menu (Show, Settings, Launch at Login, Quit)       | ❌                     | 
| Esc key          | Hides window                                               | ✅                     |
| Alt+Tab          | Window visible (taskbar icon remains - Windows limitation) | ✅                     |

**Tray Menu Items:**

| Item            | Action                | Notes                        |
|-----------------|-----------------------|------------------------------|
| Show Keva       | Show window           | Disabled if already visible  |
| Settings...     | Open settings dialog  | Non-functional until M13-win |
| Launch at Login | Toggle checkbox       | Non-functional until M18-win |
| Quit Keva       | Terminate application |                              |

**Test Cases:**

| TC       | Description                            | Status |
|----------|----------------------------------------|--------|
| TC-M1-01 | Window appears centered on launch      | ✅      |
| TC-M1-02 | Drag from inner border moves window    | ❌      |
| TC-M1-03 | Drag from outer edge resizes window    | ✅      |
| TC-M1-04 | Tray icon visible with correct tooltip | ✅      |
| TC-M1-05 | Tray left-click toggles visibility     | ✅      |
| TC-M1-06 | Tray right-click shows menu            | ❌      |
| TC-M1-07 | Esc hides window                       | ✅      |
| TC-M1-08 | Window visible in Alt+Tab              | ✅      |
| TC-M1-09 | Quit menu item terminates app          | ❌      |

**Remaining Tasks:**

1. Add tray right-click context menu (TrackPopupMenu)
2. Implement drag zone (3px inner border returns HTCAPTION, rest returns HTCLIENT)
3. Adjust resize border from 6px to 5px

### M2-win: Layout Skeleton

**Goal:** Three-pane visual structure with no business logic.

**Status:** Not Started

**Requirements:**

| Requirement         | Description                                                            | Status |
|---------------------|------------------------------------------------------------------------|--------|
| Layout              | Three panes: search bar (top), key list (left), preview area (right)   | ❌      |
| Search bar          | Text input with placeholder "Search keys...", search icon (🔍) on left | ❌      |
| Search icon         | Drag handle (drag moves window, click does nothing)                    | ❌      |
| Left pane           | Empty placeholder area for future key list                             | ❌      |
| Right pane          | Empty placeholder area for future preview/editor                       | ❌      |
| Minimum window size | Enforce minimum (e.g., 400x300)                                        | ❌      |

**Test Cases:**

| TC       | Description                                        | Status |
|----------|----------------------------------------------------|--------|
| TC-M2-01 | Three-pane layout renders correctly                | ❌      |
| TC-M2-02 | Search bar visible with placeholder text           | ❌      |
| TC-M2-03 | Search icon drag moves window                      | ❌      |
| TC-M2-04 | Window enforces minimum size on resize             | ❌      |
| TC-M2-05 | Typing in search bar shows text (no filtering yet) | ❌      |

**Notes:**

- Current Direct2D renderer and basic key list display will be refactored for this layout
- Search icon provides an additional drag handle for moving the window

### M3-win: Core Integration & Key List

**Goal:** Initialize keva_core, display active keys in left pane.

**Status:** Partial (keva_core init exists, key list basic)

**Requirements:**

| Requirement      | Description                                                 | Status |
|------------------|-------------------------------------------------------------|--------|
| keva_core init   | Initialize KevaCore on app startup                          | ✅      |
| Data directory   | Use default ~/.keva/ or KEVA_DATA_DIR environment variable  | ✅      |
| Config           | Load config.toml if exists, use defaults otherwise          | ❌      |
| Key list         | Display all active keys from active_keys()                  | ✅      |
| Scrolling        | Key list scrolls when content exceeds viewport              | ❌      |
| Empty state      | Empty database shows empty list (or "No keys" placeholder)  | ❌      |
| Refresh          | Key list reflects current database state on window show     | ❌      |

**Test Cases:**

| TC       | Description                                       | Status |
|----------|---------------------------------------------------|--------|
| TC-M3-01 | App starts successfully with no existing database | ✅      |
| TC-M3-02 | App starts successfully with existing database    | ✅      |
| TC-M3-03 | Key list displays all active keys                 | ✅      |
| TC-M3-04 | Key list scrolls when many keys exist             | ❌      |
| TC-M3-05 | Empty database shows appropriate empty state      | ❌      |
| TC-M3-06 | Keys sorted alphabetically (or by nucleo default) | ❌      |

**Notes:**

- Current implementation has basic key list rendering but lacks scrolling, empty state, and config loading
- Refresh on window show needed for consistency after external changes

### M4-win: Key Selection & Value Display

**Goal:** Click key to select, display value in right pane.

**Status:** Not Started

**Requirements:**

| Requirement           | Description                                                  | Status |
|-----------------------|--------------------------------------------------------------|--------|
| Click to select       | Clicking key in list selects it                              | ❌      |
| Selection highlight   | Selected key visually highlighted                            | ❌      |
| Right pane display    | Shows selected key's value                                   | ❌      |
| Text value            | Display text content (read-only for now)                     | ❌      |
| Files value           | Display placeholder "N file(s)" (detailed in M11)            | ❌      |
| Empty value           | Display placeholder "No value"                               | ❌      |
| Touch on select       | Call touch() when key selected                               | ❌      |
| Focus exclusivity     | Search bar focused OR key selected, never both               | ❌      |
| Search bar focus      | Clicking search bar clears key selection                     | ❌      |
| Search bar highlight  | Visual focus indicator when search bar active                | ❌      |
| Right pane on deselect| Shows placeholder for search bar text                        | ❌      |

**Test Cases:**

| TC       | Description                                              | Status |
|----------|----------------------------------------------------------|--------|
| TC-M4-01 | Clicking key highlights it, dims search bar              | ❌      |
| TC-M4-02 | Selected key's text value displays in right pane         | ❌      |
| TC-M4-03 | Clicking different key updates selection and right pane  | ❌      |
| TC-M4-04 | Clicking search bar clears selection, restores normal text | ❌    |
| TC-M4-05 | Search bar shows pen icon when exact key exists          | ❌      |
| TC-M4-06 | Search bar shows plus icon when key doesn't exist        | ❌      |
| TC-M4-07 | Button hidden when search bar empty                      | ❌      |
| TC-M4-08 | Button hidden when key selected in list                  | ❌      |
| TC-M4-09 | Hovering button shows tooltip                            | ❌      |
| TC-M4-10 | Selecting key updates last_accessed                      | ❌      |
| TC-M4-11 | Typing clears selection and updates right pane live      | ❌      |
| TC-M4-12 | Files value shows placeholder text                       | ❌      |

**UX Model (Search Bar & Selection):**

The search bar and left pane selection are **mutually exclusive**. Only one can be "active" at a time, indicated by visual focus.

**Search Bar States:**

| State | Text Style | Button | Right Pane |
|-------|------------|--------|------------|
| Empty | Gray placeholder | Hidden | Empty |
| Text, key EXISTS | Normal | ✏️ Pen (edit) | Existing key's value |
| Text, key DOESN'T EXIST | Normal | ➕ Plus (add) | "Press Enter to add {key}..." |
| Inactive (left pane selected) | Dimmed gray | Hidden | Selected key's value |

**Selection Transitions:**

| Action | Search Bar | Left Pane | Right Pane |
|--------|------------|-----------|------------|
| Click search bar | Focused, normal text | Selection clears | Updates based on search text |
| Click key in list | Dimmed gray | Key highlighted | Selected key's value |
| Type in search bar | Focused (was already) | Selection clears | Updates live |

**Visual Focus (mutual exclusivity):**

| State | Search Bar | Left Pane |
|-------|------------|-----------|
| Search bar focused | Focus border/highlight | No selection |
| Key selected | No focus, dimmed text | Selected row highlighted (Spotlight-style) |

**Button Display (M4 scope - visual only):**

| State | Icon | Tooltip |
|-------|------|---------|
| Key EXISTS | ✏️ Pen | "Edit {key} (Enter)" |
| Key DOESN'T EXIST | ➕ Plus | "Create {key} (Enter)" |
| Empty / Key selected | Hidden | - |

**Note:** Button click/Enter action deferred to M7-win.

### M5-win: Text Editor & Auto-Save

**Goal:** Editable text area in right pane with automatic saving.

**Status:** Not Started

**Requirements:**

| Requirement       | Description                                          | Status |
|-------------------|------------------------------------------------------|--------|
| Text editing      | Right pane text content is editable (not read-only)  | ❌      |
| Edit trigger      | Clicking in right pane text area enables editing     | ❌      |
| Auto-save         | Save changes after 3 seconds of inactivity           | ❌      |
| Save method       | Call upsert_text() on keva_core                      | ❌      |
| Key list update   | New key appears in left pane after first save        | ❌      |
| Unsaved indicator | Optional: visual indicator for unsaved changes       | ❌      |
| Save on hide      | Save pending changes when window hides (Esc)         | ❌      |
| Empty text        | Saving empty string stores empty Text value (key preserved) | ❌ |

**Scope:**

| In Scope                          | Out of Scope             |
|-----------------------------------|--------------------------|
| Edit existing Text value          | Creating new key (M9)    |
| Edit empty value for existing key | Clipboard paste (M6)     |
| Auto-save timing                  | Files value editing      |

**Test Cases:**

| TC       | Description                                              | Status |
|----------|----------------------------------------------------------|--------|
| TC-M5-01 | Clicking text area allows typing                         | ❌      |
| TC-M5-02 | Changes auto-save after 3 seconds idle                   | ❌      |
| TC-M5-03 | Saved changes persist after app restart                  | ❌      |
| TC-M5-04 | Pressing Esc saves pending changes before hiding         | ❌      |
| TC-M5-05 | Deleting all text saves empty string (key not deleted)   | ❌      |
| TC-M5-06 | Rapid typing delays save until 3 seconds after last keystroke | ❌ |
| TC-M5-07 | Switching selection saves pending changes to previous key | ❌     |

---

## Phase 2: macOS App (Swift)

### M0-mac: Core FFI Layer

**Goal:** Expose keva_core and keva_search to Swift via C FFI.

**Dependencies:**

```toml
[dependencies]
keva_core = { path = "../core" }
keva_search = { path = "../search" }

[build-dependencies]
cbindgen = "0.27"
```

**Tasks:**

1. Create `ffi` crate with `crate-type = ["cdylib"]`
2. Define C-compatible API with `#[no_mangle]` and `extern "C"`
3. Handle memory management (Box for heap, CString for strings)
4. Generate `keva.h` via cbindgen
5. Build as `libkeva.dylib`
6. Expose keva_search API for fuzzy search

**API:**

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

**Acceptance criteria:**

- `libkeva.dylib` builds
- `keva.h` generated
- Can call from Swift playground

### M1-mac: Window Skeleton

**Goal:** Basic borderless window with menu bar icon.

**Build:** Swift Package Manager or xcodebuild (no Xcode IDE required)

**Tasks:**

1. Create Swift package or minimal Xcode project
2. Link `libkeva.dylib`, import `keva.h` via bridging header (from M0-mac)
3. Borderless window (NSWindow, styleMask)
4. Custom resize handling if needed
5. Menu bar icon (NSStatusItem)
6. Cmd+Q quits, Esc hides window
7. Set LSUIElement=true in Info.plist (hide from Dock/Cmd+Tab)

**Acceptance criteria:**

- App launches to menu bar
- Window shows/hides on click
- Window resizes properly
- No Dock icon, hidden from Cmd+Tab

### M2-mac: Core Integration

**Goal:** Connect UI to keva_core via FFI.

**Tasks:**

1. Swift wrapper around C FFI
2. Load/display keys
3. Text preview (NSTextView)
4. File preview (QLPreviewView)
5. Clipboard paste to create key

### M3-mac: Full Features

**Goal:** All Spec.md features.

**Tasks:**

1. Fuzzy search (keva_search via FFI)
2. Edit/rename/delete keys
3. Copy to clipboard (NSPasteboard)
4. Trash support
5. Settings window

---

## Phase 3: Polish

### M4: Distribution

**Windows:**

- Installer (WiX or MSIX)
- Launch at Login (Registry)
- Code signing (optional)

**macOS:**

- App bundle structure
- Launch at Login (LaunchAgent or SMLoginItemSetEnabled)
- Code signing + notarization

---

## Notes

### Windows Crate Features

Current features for `windows` crate (v0.62):

```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.62"
features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Controls",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Dwm",
    # Future: "Win32_System_Com" for IPreviewHandler
]
```

### macOS Borderless + Resize

```swift
let styleMask: NSWindow.StyleMask = [
    .borderless,
    .resizable,  // This should work in native Swift
]
window = NSWindow(contentRect: rect, styleMask: styleMask, ...)
```

If `.borderless` + `.resizable` doesn't work, implement `mouseDown`/`mouseDragged` for edge resizing.

### FFI Memory Rules

- Caller allocates path strings, FFI copies internally
- FFI allocates return values (KevaValue, KevaKeyList)
- Caller must call `keva_free_*` to release
- Error codes: 0 = success, negative = error
