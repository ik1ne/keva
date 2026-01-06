# Windows Implementation Milestones

This document defines the implementation milestones for Keva on Windows. Each milestone builds upon the previous ones
and includes test cases for verification.

## Milestone Overview

| #    | Milestone              | Description                                   | Status |
|------|------------------------|-----------------------------------------------|--------|
| M0   | keva_core Verification | Verify keva_core matches keva_core.md spec    | ✅      |
| M1   | Window Skeleton        | Borderless window, resize, tray icon          | ✅      |
| M2   | WebView + Bridge       | WebView2 hosting, postMessage, dark theme     | ✅      |
| M3   | Worker Thread          | Main↔Worker mpsc, keva_core integration       | ✅      |
| M4   | Search Engine          | Nucleo on main thread, progressive results    | ✅      |
| M5   | Key List               | Left pane, create/rename/delete, selection    | ✅      |
| M6   | Monaco Editor          | FileSystemHandle, markdown mode, auto-save    | ✅      |
| M7   | Four-State Focus       | Focus model, keyboard navigation, dimming     | ✅      |
| M8   | Attachments Display    | File list, sizes, icons, thumbnails, picker   | ✅      |
| M9   | Attachment Operations  | Remove with confirmation, inline rename       | ✅      |
| M10a | CompositionController  | WebView2 migration for native drag-drop       | ✅      |
| M10  | Attachment Drag & Drop | Drag to Monaco, file drop, multi-file batch   | ✅      |
| M11  | Attachment Drag Out    | Drag attachments to external apps (copy)      | ✅      |
| M12  | Clipboard              | Native read, paste intercept, copy shortcuts  | ❌      |
| M13  | Edit/Preview Toggle    | Markdown renderer, att: link transform        | ❌      |
| M14  | Trash                  | Trash section, restore, GC triggers           | ❌      |
| M15  | Settings               | Dialog, config persistence, theme             | ❌      |
| M16  | Global Hotkey          | Ctrl+Alt+K registration, conflict detection   | ❌      |
| M17  | Single Instance        | Named mutex, activate existing window         | ❌      |
| M18  | Window Position Memory | Per-monitor position, off-screen check        | ❌      |
| M19  | First-Run Dialog       | Welcome message, launch at login checkbox     | ❌      |
| M20  | Monaco Bundling        | Embed resources, single exe                   | ❌      |
| M21  | Installer              | WiX/MSIX, uninstaller, WebView2 version check | ❌      |
| M22  | Layout Polish          | Resizable panes, layout persistence           | ❌      |

---

## M0: Core Crates Verification

**Goal:** Verify keva_core and keva_search implementations match their specifications.

**Description:** Review existing crate implementations against their specification documents. For keva_core: verify the
unified data model (markdown + attachments), storage structure with separate content/, blobs/, thumbnails/ trees,
attachment operations with conflict resolution, and thumbnail versioning. For keva_search: verify the dual-index
architecture, tombstone-based deletion, stop-at-threshold behavior, and maintenance compaction. This milestone is
complete when both crates compile with the specified API surface and pass their test suites.

**keva_core Key APIs:**

| Category    | APIs                                                                                   |
|-------------|----------------------------------------------------------------------------------------|
| Lifecycle   | `open(config)`                                                                         |
| Key Ops     | `get()`, `active_keys()`, `trashed_keys()`, `touch()`, `rename()`                      |
| Content     | `content_path()`, `create()`                                                           |
| Attachments | `attachment_path()`, `add_attachments()`, `remove_attachment()`, `rename_attachment()` |
| Thumbnails  | `thumbnail_paths()`                                                                    |
| Trash       | `trash()`, `restore()`, `purge()`                                                      |
| Maintenance | `maintenance()`                                                                        |

Note: Clipboard operations are platform-specific and implemented in keva_windows (M11).

**keva_search Key APIs:**

| Category    | APIs                                                                          |
|-------------|-------------------------------------------------------------------------------|
| Constructor | `SearchEngine::new(active, trashed, config, notify)`                          |
| Mutation    | `add_active()`, `trash()`, `restore()`, `remove()`, `rename()`                |
| Search      | `set_query()`, `tick()`, `is_done()`, `active_results()`, `trashed_results()` |
| Maintenance | `maintenance_compact()`                                                       |
| Results     | `SearchResults::iter()`                                                       |

**Test Cases:**

| TC       | Description                                                              | Status |
|----------|--------------------------------------------------------------------------|--------|
| TC-M0-01 | keva_core compiles with specified API surface                            | ✅      |
| TC-M0-02 | keva_core storage structure matches spec (content/, blobs/, thumbnails/) | ✅      |
| TC-M0-03 | keva_core Key validation enforces constraints (1-256 chars, trimmed)     | ✅      |
| TC-M0-04 | keva_core attachment conflict resolution works (Overwrite/Rename/Skip)   | ✅      |
| TC-M0-05 | keva_core thumbnail versioning triggers regeneration                     | ✅      |
| TC-M0-06 | keva_core lifecycle transitions correct (Active→Trash→Purge)             | ✅      |
| TC-M0-07 | keva_core timestamp updates (last_accessed, trashed_at)                  | ✅      |
| TC-M0-08 | keva_core test suite passes (163 tests)                                  | ✅      |
| TC-M0-09 | keva_search compiles with specified API surface                          | ✅      |
| TC-M0-10 | keva_search dual-index architecture (active/trashed)                     | ✅      |
| TC-M0-11 | keva_search tombstone-based deletion works                               | ✅      |
| TC-M0-12 | keva_search stop-at-threshold behavior (100 active, 20 trashed)          | ✅      |
| TC-M0-13 | keva_search index compaction triggers at rebuild_threshold               | ✅      |
| TC-M0-14 | keva_search smart case matching works                                    | ✅      |
| TC-M0-15 | keva_search test suite passes (35 tests)                                 | ✅      |

---

## M1: Window Skeleton

**Goal:** Borderless window with system tray and basic window management.

**Description:** Native Rust window using `windows` crate. No title bar, system-metrics-based outer resize zone, always
on top. System tray icon with left-click toggle and right-click context menu. DPI-aware rendering. Esc hides window
without destroying it. Window stays on top to enable drag/drop from other apps.

**Implementation Notes:**

- `WS_POPUP` style for borderless window
- `WS_EX_TOPMOST` for always on top
- `WM_NCHITTEST` handling for resize border:
    - Border width: `GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER)`
    - Border height: `GetSystemMetrics(SM_CYSIZEFRAME) + GetSystemMetrics(SM_CYPADDEDBORDER)`
- `Shell_NotifyIconW` for tray icon
- `SetProcessDpiAwarenessContext` for DPI awareness
- Minimum window size: 400×300 logical pixels (enforced via `WM_GETMINMAXINFO`)
- Aero Snap support requires `WM_NCHITTEST` returning appropriate `HT*` values

**Test Cases:**

| TC       | Description                                | Status |
|----------|--------------------------------------------|--------|
| TC-M1-01 | Window appears centered on primary monitor | ✅      |
| TC-M1-02 | Drag from outer edge resizes window        | ✅      |
| TC-M1-03 | Tray icon visible with "Keva" tooltip      | ✅      |
| TC-M1-04 | Tray left-click toggles window visibility  | ✅      |
| TC-M1-05 | Tray right-click shows context menu        | ✅      |
| TC-M1-06 | Esc hides window (process stays alive)     | ✅      |
| TC-M1-07 | Window stays on top of other windows       | ✅      |
| TC-M1-08 | Text is crisp at 150% DPI scaling          | ✅      |
| TC-M1-09 | Alt+F4 quits application entirely          | ✅      |
| TC-M1-10 | Corner drag resizes diagonally             | ✅      |
| TC-M1-11 | Window respects minimum size (400x300)     | ✅      |
| TC-M1-12 | Aero Snap to left edge (half-screen)       | ✅      |
| TC-M1-13 | Aero Snap to right edge (half-screen)      | ✅      |
| TC-M1-14 | Aero Snap to top edge (maximize)           | ✅      |
| TC-M1-15 | Aero Snap to corner (quarter-screen)       | ✅      |
| TC-M1-16 | Drag from maximized restores window        | ✅      |
| TC-M1-17 | Resize border scales correctly at 150% DPI | ✅      |

---

## M2: WebView + Message Protocol

**Goal:** Define Native↔WebView message protocol structure.

**Description:** WebView2 hosting is verified. This milestone defines the message categories and establishes the
protocol conventions. Each subsequent milestone will specify its required messages in detail.

**Implementation Notes:**

- WebView2 SDK 1.0.2470+ required for FileSystemHandle
    - Check `ICoreWebView2Environment14` availability at runtime
- `PostWebMessageAsJson` for standard messages
- `PostWebMessageAsJsonWithAdditionalObjects` for FileSystemHandle transfer
- Runtime detection: `GetAvailableCoreWebView2BrowserVersionString`
    - If missing: show download prompt with link to WebView2 installer

**Message Categories:**

| Category    | Direction      | Purpose                                      |
|-------------|----------------|----------------------------------------------|
| Search      | Bidirectional  | Query and results                            |
| Key Ops     | WebView→Native | Create, rename, delete, trash, restore       |
| Content     | Native→WebView | FileSystemHandle transfer, modification flag |
| Attachments | Bidirectional  | Add, remove, rename, thumbnails              |
| Clipboard   | Bidirectional  | Read request, content response, copy ops     |
| Window      | WebView→Native | Hide, quit, drag, settings                   |
| State       | Native→WebView | Theme, visibility, operation results         |

**Protocol Conventions:**

- All messages are JSON with `type` discriminator
- Native→WebView: state pushes, responses to requests
- WebView→Native: operation requests, UI events
- Errors returned via `operationResult` message with `success: false`

**Test Cases:**

| TC       | Description                                          | Status |
|----------|------------------------------------------------------|--------|
| TC-M2-01 | WebView loads and displays UI                        | ✅      |
| TC-M2-02 | Initial theme matches system dark/light mode         | ✅      |
| TC-M2-03 | Native↔WebView messages work (key list appears)      | ✅      |
| TC-M2-04 | Changing system theme updates WebView immediately    | ✅      |
| TC-M2-05 | Changing system theme updates window border color    | ✅      |
| TC-M2-06 | Splash screen shows during initial load              | ✅      |
| TC-M2-07 | Splash screen disappears after keys message received | ✅      |

---

## M3: Worker Thread

**Goal:** Background thread for keva_core operations.

**Description:** Spawn worker thread on startup. Main thread sends requests via mpsc channel. Worker executes keva_core
operations and posts results back via custom window message. Worker owns `KevaCore` instance exclusively.

**Implementation Notes:**

- `std::sync::mpsc::channel` for Main→Worker
- `PostMessageW(WM_WORKER_RESULT)` for Worker→Main
- Request/Response enums for type-safe messaging

**Threading Model:**

```
Main Thread                     Worker Thread
    │                               │
    ├─── Request::CreateKey ───────►│
    │                               ├─── keva_core.create()
    │◄── WM_WORKER_RESULT ──────────┤
    │    (Response::KeyCreated)     │
```

**Test Cases:**

| TC       | Description                               | Status |
|----------|-------------------------------------------|--------|
| TC-M3-01 | App quits cleanly via Alt+F4 (no hang)    | ✅      |
| TC-M3-02 | App quits cleanly via tray menu (no hang) | ✅      |
| TC-M3-03 | Creating key updates UI without freezing  | ✅      |

---

## M4: Search Engine

**Goal:** Fuzzy search with progressive, stable results.

**Description:** Integrate keva_search on main thread. SearchEngine wraps Nucleo with dual indexes (active/trashed),
tombstone-based deletion, and stop-at-threshold behavior. Nucleo's notify callback posts `WM_SEARCH_READY` to trigger UI
updates.

**Implementation Notes:**

- SearchEngine lives on main thread; Nucleo spawns internal worker pool
- `notify` callback: `PostMessageW(hwnd, WM_SEARCH_READY, ...)`
- On `WM_SEARCH_READY`: call `tick()`, update UI if results changed
- Threshold stops result changes (100 active, 20 trashed)
- Key mutations (create/rename/delete) update indexes via `add_active()`, `trash()`, `remove()`, `rename()`
- `maintenance_compact()` called during `keva_core::maintenance()`

**Test Cases:**

| TC       | Description                                     | Status |
|----------|-------------------------------------------------|--------|
| TC-M4-01 | Type in search bar, matching keys appear        | ✅      |
| TC-M4-02 | Empty search shows all keys                     | ✅      |
| TC-M4-03 | Results stop changing after threshold reached   | ✅      |
| TC-M4-04 | "abc" matches "ABC"; "Abc" does not match "abc" | ✅      |
| TC-M4-05 | Trashed keys appear in separate section         | ✅      |

---

## M5: Key List

**Goal:** Left pane with key display and management.

**Description:** Render filtered key list in left pane. Support single selection with keyboard and mouse. Create new
keys via search bar Enter. Rename and delete keys with inline controls. Delete always moves to trash. Trash section at
bottom shows trashed keys separately.

**Implementation Notes:**

- Virtual scrolling for large key lists
- Inline rename editor on pen icon click
- Trash section at bottom with fixed height (~2.5 rows)
- Long key names: truncate with ellipsis
- Inline error message for invalid rename (empty, >256 chars)
- Rename updates key in place (maintains position); list only refreshes on search query change

**Key Interactions:**

- Click key → select and show in right pane
- Down arrow from search → focus first key
- Up arrow from first key → focus search bar
- Enter on selected key → focus right top pane
- Delete key → move to trash

**Test Cases:**

| TC       | Description                                                         | Status |
|----------|---------------------------------------------------------------------|--------|
| TC-M5-01 | Keys display in left pane                                           | ✅      |
| TC-M5-02 | Click key selects it, content shown in right pane                   | ✅      |
| TC-M5-03 | Arrow keys navigate key list                                        | ✅      |
| TC-M5-04 | Enter in search bar with no match creates key, focuses editor       | ✅      |
| TC-M5-05 | Enter in search bar with exact match selects key, focuses editor    | ✅      |
| TC-M5-06 | Rename key via inline editor                                        | ✅      |
| TC-M5-07 | Rename to existing key shows overwrite confirmation                 | ✅      |
| TC-M5-08 | Rename validation rejects empty or >256 chars, keeps focus          | ✅      |
| TC-M5-09 | Delete key moves to trash                                           | ✅      |
| TC-M5-10 | Trash section shows at bottom with trashed keys                     | ✅      |
| TC-M5-11 | Long key name truncates with ellipsis                               | ✅      |
| TC-M5-12 | Up arrow from first key returns focus to search bar                 | ✅      |
| TC-M5-13 | Renamed key maintains position in list (no re-sort)                 | ✅      |
| TC-M5-14 | Escape during rename cancels without hiding window                  | ✅      |
| TC-M5-15 | Search action button shows ➕ (no match) or ✏️ (match)               | ✅      |
| TC-M5-16 | Clicking action button creates/selects key, focuses editor          | ✅      |
| TC-M5-17 | Rename overwrite updates selection and reloads content              | ✅      |
| TC-M5-18 | Unsaved changes saved when selection changes (typing or key switch) | ✅      |
| TC-M5-19 | Up arrow in search bar does nothing                                 | ✅      |
| TC-M5-20 | Enter on selected key in left pane focuses editor                   | ✅      |
| TC-M5-21 | Clicking trashed key shows content as read-only                     | ✅      |
| TC-M5-22 | Down arrow from last key stays on last key                          | ✅      |
| TC-M5-23 | Renaming key to same name cancels rename (no change)                | ✅      |
| TC-M5-24 | Key action buttons (rename/delete) appear on hover                  | ✅      |

---

## M6: Monaco Editor

**Goal:** Markdown editor with direct file access and autosave.

**Description:** Embed Monaco editor in right top pane. Use FileSystemHandle API for direct file read/write. Markdown
language mode with syntax highlighting. Placeholder text when empty. Autosave via debounced writes; forced save on key
switch and app exit.

**Implementation Notes:**

- Monaco loaded from bundled resources (see M17)
- `PostWebMessageAsJsonWithAdditionalObjects` for FileSystemHandle
- Monaco config: `pasteAs: { enabled: false }`, `dragAndDrop: true`
- Placeholder: "Type something, or drag files here..."

**Save Behavior:**

| Trigger                | File Write | touch() |
|------------------------|------------|---------|
| Debounced (500ms idle) | ✓          | ✓       |
| Key switch (if dirty)  | ✓          | ✓       |
| App exit (if dirty)    | ✓          | ✓       |

**Exit Flow:**

```
1. Exit triggered (Alt+F4, tray quit, WM_ENDSESSION)
2. Native → WebView: { type: "prepareExit" }
3. WebView: flush dirty content via FileSystemHandle
4. WebView → Native: { type: "readyToExit" }
5. Native: destroy window, exit process
```

**FileSystemHandle Flow:**

```
1. User selects key
2. Native: get_content_path() → path
3. Native: create FileSystemHandle for path
4. Native: PostWebMessageAsJsonWithAdditionalObjects(handle)
5. WebView: Monaco reads via handle
6. User edits → debounced save → Monaco writes via handle
7. User switches key → forced save if dirty
```

**Test Cases:**

| TC       | Description                                                     | Status |
|----------|-----------------------------------------------------------------|--------|
| TC-M6-01 | Monaco editor loads with markdown highlighting                  | ✅      |
| TC-M6-02 | Selecting key loads content into editor                         | ✅      |
| TC-M6-03 | Edits persist after switching away and back                     | ✅      |
| TC-M6-04 | Placeholder shows when content empty                            | ✅      |
| TC-M6-05 | Rapid key switching does not lose unsaved edits                 | ✅      |
| TC-M6-06 | Quitting app does not lose unsaved edits                        | ✅      |
| TC-M6-07 | Trashed key shows "Restore from trash to edit" on type attempt  | ✅      |
| TC-M6-08 | Trashed key content is read-only (cannot edit)                  | ✅      |
| TC-M6-09 | Active key content is editable                                  | ✅      |
| TC-M6-10 | Theme switch updates Monaco theme (dark/light)                  | ✅      |
| TC-M6-11 | New key starts with empty content and placeholder               | ✅      |
| TC-M6-12 | Content persists after app restart                              | ✅      |
| TC-M6-13 | Large file (>1MB) loads in plaintext mode with reduced features | ✅      |
| TC-M6-14 | Write error shows banner with retry button (see note)           | ✅      |
| TC-M6-15 | Re-selecting same key does not reload content                   | ✅      |

**TC-M6-14 Test Procedure:**

1. Create a key with some content
2. Find the content file in `%LOCALAPPDATA%\keva\content\` and mark it as read-only
3. Edit and save - error banner should appear
4. Uncheck read-only on the file
5. Press retry - save should succeed and banner should disappear

---

## M7: Four-State Focus

**Goal:** Mutually exclusive focus between four panes.

**Description:** Implement four-state focus model: search bar, left pane, right top, right bottom. Only one pane active
at a time. Visual indicators for active/inactive state. Keyboard navigation between panes.

**Implementation Notes:**

- Active pane: cursor visible, full highlight
- Inactive pane: no cursor, dimmed
- Esc from any pane: hide window (unless modal open)

**Focus States:**

| Active Pane  | Search Bar | Left Pane   | Right Top | Right Bottom |
|--------------|------------|-------------|-----------|--------------|
| Search bar   | Cursor     | Dimmed      | No cursor | Dimmed       |
| Left pane    | No cursor  | Highlighted | No cursor | Dimmed       |
| Right top    | No cursor  | Dimmed      | Cursor    | Dimmed       |
| Right bottom | No cursor  | Dimmed      | No cursor | Highlighted  |

**Test Cases:**

| TC       | Description                                                  | Status |
|----------|--------------------------------------------------------------|--------|
| TC-M7-01 | Only one pane shows active state                             | ✅      |
| TC-M7-02 | Click pane activates it                                      | ✅      |
| TC-M7-03 | Down arrow from search focuses left pane                     | ✅      |
| TC-M7-04 | Enter from left pane focuses right top                       | ✅      |
| TC-M7-05 | Inactive panes show dimmed styling                           | ✅      |
| TC-M7-06 | Left pane selection persists when inactive                   | ✅      |
| TC-M7-07 | Ctrl+S focuses search bar from any pane                      | ✅      |
| TC-M7-08 | Clicking key in list while right pane is focused selects key | ✅      |
| TC-M7-09 | Ctrl+S from editor focuses search bar with text selected     | ✅      |
| TC-M7-10 | Clicking Monaco sets editor as active pane                   | ✅      |
| TC-M7-11 | Shift+click on first attachment (no prior selection) selects | ✅      |
| TC-M7-12 | Window hide → show restores focus to previously active pane  | ✅      |
| TC-M7-13 | Arrow navigation in empty key list does nothing (no crash)   | ✅      |
| TC-M7-14 | Tab key does nothing (blocked globally)                      | ✅      |

---

## M8: Attachments Display

**Goal:** Display attachments with file picker for adding files.

**Description:** Right bottom pane displays attachment list with filename, size, and type icon. Generate and display
thumbnails for images. Add files via [+ Add files] button that opens native file picker. Handle duplicate filenames
with conflict dialog. Empty state shows centered add button.

**Implementation Notes:**

- Thumbnail generation on import (worker thread)
- Supported thumbnail formats: png, jpg, jpeg, gif, webp, svg
- Thumbnail stored as {filename}.thumb
- File size formatting: bytes → KB/MB/GB
- Type icons for non-image files (📄 document, 🎵 audio, 🎬 video, etc.)
- [+] button in header opens file picker (disabled when no key selected or key is trashed)
- Duplicate dialog: "'{filename}' already exists." with [Overwrite] [Rename] [Skip]
- Multi-file: "Apply to all" checkbox to use same action for remaining conflicts
- Native file picker via Win32 `GetOpenFileNameW` or IFileOpenDialog

**Test Cases:**

| TC       | Description                                      | Status |
|----------|--------------------------------------------------|--------|
| TC-M8-01 | Attachments list displays files with names       | ✅      |
| TC-M8-02 | File size shown in human-readable format (KB/MB) | ✅      |
| TC-M8-03 | Image attachments show thumbnail                 | ✅      |
| TC-M8-04 | Non-image attachments show type icon             | ✅      |
| TC-M8-05 | Click [+ Add files] opens file picker            | ✅      |
| TC-M8-06 | Duplicate filename shows conflict dialog         | ✅      |
| TC-M8-07 | Multi-select with Ctrl+click                     | ✅      |
| TC-M8-08 | Shift+click range selection works                | ✅      |

---

## M9: Attachment Operations

**Goal:** Remove and rename attachments with appropriate dialogs.

**Description:** Each attachment has [X] remove button and [✏️] rename button. Remove shows confirmation dialog.
Rename uses inline editor with conflict handling for duplicate names.

**Implementation Notes:**

- [X] button per attachment for removal
- [✏️] button for inline rename (similar to key rename in M5)
- Delete confirmation dialog: "Delete '{filename}'?" with [Delete] [Cancel]
- Rename conflict dialog: "'{filename}' already exists." with [Overwrite] [Cancel]
- Invalid rename (empty name) rejected with inline error
- Buttons appear on hover (like key action buttons)

**Rename Reference Update:**

When renaming an attachment that is referenced in markdown, show dialog:

```
┌─────────────────────────────────────────────────┐
│ "old.pdf" is referenced in your notes.          │
│ Update references to "new.pdf"?                 │
│                                                 │
│ [Update]  [Don't Update]  [Cancel]              │
└─────────────────────────────────────────────────┘
```

- **Update:** Rename file and replace all `att:old.pdf` with `att:new.pdf` in editor
- **Don't Update:** Rename file only (references become broken)
- **Cancel:** Abort rename operation

Reference update is handled by frontend (modifies editor content directly).

**Test Cases:**

| TC       | Description                                       | Status |
|----------|---------------------------------------------------|--------|
| TC-M9-01 | [X] button shows delete confirmation dialog       | ✅      |
| TC-M9-02 | Confirming delete removes attachment              | ✅      |
| TC-M9-03 | Rename attachment via inline editor               | ✅      |
| TC-M9-04 | Rename to existing filename shows conflict dialog | ✅      |
| TC-M9-05 | Empty rename rejected with inline error           | ✅      |
| TC-M9-06 | Action buttons appear on hover                    | ✅      |
| TC-M9-07 | Escape cancels rename without hiding window       | ✅      |
| TC-M9-08 | Rename referenced file with Update updates editor | ✅      |
| TC-M9-09 | Rename with Don't Update leaves editor unchanged  | ✅      |

---

## M10a: WebView2 CompositionController Migration

**Goal:** Convert WebView2 from standard Controller to CompositionController mode, enabling native drag-drop
interception.

**Description:** Standard Controller mode prevents access to file paths during drag-drop (web security restriction).
CompositionController mode gives native code full control over input, allowing us to intercept drops via `IDropTarget`,
extract paths, and forward events to WebView2.

**Reference:** See `research/composition_controller_migration.md` for detailed architecture and implementation notes.

**Implementation Notes:**

- Replace `CreateCoreWebView2Controller` with `CreateCoreWebView2CompositionController`
- Set up DirectComposition device and visual tree
- Forward client-area input via `SendMouseInput()`, `SendKeyboardInput()`
- Handle cursor changes via `CursorChanged` event
- Implement `IDropTarget` to cache file paths and forward to controller
- Add `addDroppedFiles` message for index-based file resolution

**Test Cases:**

| TC         | Description                                              | Status |
|------------|----------------------------------------------------------|--------|
| TC-M10a-01 | Window displays correctly (no visual regression)         | ✅      |
| TC-M10a-02 | Mouse clicks work throughout UI                          | ✅      |
| TC-M10a-03 | Keyboard input works (typing, shortcuts)                 | ✅      |
| TC-M10a-04 | Window resize from edges works                           | ✅      |
| TC-M10a-05 | CSS `app-region: drag` enables window dragging           | ✅      |
| TC-M10a-06 | Text is crisp at 150% DPI scaling                        | ✅      |
| TC-M10a-07 | Existing file picker (`openFilePicker`) still works      | ✅      |
| TC-M10a-08 | Scroll wheel works in scrollable content                 | ✅      |
| TC-M10a-09 | Cursor changes appropriately (pointer, text cursor)      | ✅      |
| TC-M10a-10 | Drop single file → paths cached, DOM event fires         | ✅      |
| TC-M10a-11 | Drop files with same name (mod.rs, mod.rs) → index works | ✅      |
| TC-M10a-12 | Drag enter/leave without drop → cache cleared            | ✅      |

---

## M10: Attachment Drag & Drop

**Goal:** Drag attachments to Monaco and drop files onto attachments pane.

**Description:** Drag attachment from panel to Monaco inserts markdown link at drop position. Drop files from Explorer
onto attachments pane adds them. Multi-file operations show "Apply to all" checkbox for batch conflict resolution.

**Implementation Notes:**

- Drag from attachments panel: set dataTransfer with filename
- Drop on Monaco: insert `[filename](att:filename)` at cursor position
- Drop files onto attachments pane: add to current key's attachments
- Multi-file with duplicates: "☐ Apply to all (N remaining)" checkbox
- Multi-file dialog buttons: [Overwrite] [Rename] [Skip] [Cancel All]
- Drop onto trashed key: rejected (show error)

**Drag to Monaco:**

```
1. User drags attachment from panel
2. Drop on Monaco at cursor position
3. Insert: [filename](att:filename)
```

**Test Cases:**

| TC        | Description                                        | Status |
|-----------|----------------------------------------------------|--------|
| TC-M10-01 | Drag attachment to Monaco inserts link             | ✅      |
| TC-M10-02 | Drop files onto attachments pane adds them         | ✅      |
| TC-M10-03 | Multi-file drop with duplicates shows batch dialog | ✅      |
| TC-M10-04 | "Apply to all" checkbox applies to remaining files | ✅      |
| TC-M10-05 | Drop onto trashed key rejected                     | ✅      |
| TC-M10-06 | Drag multiple selected attachments to Monaco       | ✅      |
| TC-M10-07 | Drop text from external app onto Monaco            | ✅      |
| TC-M10-08 | Escape during drag cancels drag, doesn't hide      | ✅      |

---

## M11: Attachment Drag Out

**Goal:** Drag attachments from Keva to external applications.

**Description:** Drag attachments from the attachments panel to external drop targets (File Explorer, email clients,
etc.). Uses `DragStarting` event on `ICoreWebView2CompositionController5` to intercept WebView drag and create
`CF_HDROP` with blob paths. Supports single and multi-file selection. Copy only (attachment remains in Keva).

**Prerequisites:**

- WebView2 Runtime with `ICoreWebView2CompositionController5` support (DragStarting API)
- Minimum WebView2 version enforced at app launch

**Implementation Notes:**

- Subscribe to `DragStarting` event on CompositionController5
- Detect internal attachment drag via `application/x-keva-attachments` in IDataObject
- Create new IDataObject with `CF_HDROP` containing absolute blob paths
- Set `args.Handled = true` to suppress WebView default drag
- Call `DoDragDrop()` with `DROPEFFECT_COPY`
- Internal drop detection: check if paths are inside Keva blob storage directory

**Drop Resolution:**

When drop occurs back onto Keva window (detected via path prefix in `IDropTarget::Drop`):

| Drop Location    | Action                           |
|------------------|----------------------------------|
| Monaco           | Insert `[file](att:file)` links  |
| Attachments pane | No-op (already attached)         |
| External app     | DoDragDrop handles (file copied) |

**Test Cases:**

| TC        | Description                                     | Status |
|-----------|-------------------------------------------------|--------|
| TC-M11-01 | Drag single attachment to Explorer creates copy | ✅      |
| TC-M11-02 | Drag multiple selected attachments to Explorer  | ✅      |
| TC-M11-03 | Drag to email client attaches file              | ✅      |
| TC-M11-04 | Drag from trashed key rejected (no drag start)  | ✅      |
| TC-M11-05 | Escape cancels drag operation                   | ✅      |
| TC-M11-06 | Internal drag to Monaco still inserts links     | ✅      |
| TC-M11-07 | Internal drag to attachments pane is no-op      | ✅      |
| TC-M11-08 | Monaco internal text drag-drop still works      | ✅      |

---

## M12: Clipboard

**Goal:** Native clipboard integration with paste interception.

**Description:** Native reads clipboard via Win32 API (CF_HDROP for files, CF_UNICODETEXT for text). WebView intercepts
paste and requests clipboard from native. Context-aware paste behavior. Copy shortcuts for markdown, HTML, and files.

**Implementation Notes:**

- `OpenClipboard`, `GetClipboardData(CF_HDROP)` for files
- `GetClipboardData(CF_UNICODETEXT)` for text
- WebView: `addEventListener('paste', preventDefault)`
- WebView sends `{ type: "readClipboard" }` to native
- Paste text into attachments panel: show confirmation dialog

**Copy Shortcuts:**

| Shortcut   | Action                             | On Success  |
|------------|------------------------------------|-------------|
| Ctrl+C     | Copy selection (context-dependent) | Stay open   |
| Ctrl+Alt+T | Copy whole markdown as plain text  | Hide window |
| Ctrl+Alt+R | Copy rendered preview as HTML      | Hide window |
| Ctrl+Alt+F | Copy all attachments to clipboard  | Hide window |

**Test Cases:**

| TC        | Description                                          | Status |
|-----------|------------------------------------------------------|--------|
| TC-M12-01 | Paste text into search bar                           | ❌      |
| TC-M12-02 | Paste text into Monaco                               | ❌      |
| TC-M12-03 | Paste files adds attachments + inserts links         | ❌      |
| TC-M12-04 | Ctrl+C in Monaco copies selected text                | ❌      |
| TC-M12-05 | Ctrl+C in attachments copies selected files          | ❌      |
| TC-M12-06 | Ctrl+Alt+T copies markdown, hides window             | ❌      |
| TC-M12-07 | Ctrl+Alt+R copies rendered HTML, hides window        | ❌      |
| TC-M12-08 | Ctrl+Alt+F copies attachments, hides window          | ❌      |
| TC-M12-09 | "Nothing to copy" shown when no target key           | ❌      |
| TC-M12-10 | Paste files into search bar does nothing             | ❌      |
| TC-M12-11 | Paste text into attachments panel shows confirmation | ❌      |

---

## M13: Edit/Preview Toggle

**Goal:** Toggle between markdown editing and rendered preview.

**Description:** Two-tab interface in right top pane: Edit and Preview. Edit mode shows Monaco editor. Preview mode
shows rendered markdown with inline images. Attachment links (att:filename) transformed to blob paths for display.

**Implementation Notes:**

- Markdown renderer: marked.js or markdown-it
- `att:filename` → blob path transformation for images
- Non-image att: links remain clickable (open file)
- Preview is read-only
- Sanitization: DOMPurify to prevent XSS
- Broken att: link: show placeholder icon with tooltip
- External links (http://, https://): open in default browser

**Link Transformation:**

```markdown
<!-- Source -->
[image.png](att:image.png)

<!-- Preview renders as -->
<img src="file:///path/to/blobs/{key_hash}/image.png">
```

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M13-01 | Edit tab shows Monaco editor                 | ❌      |
| TC-M13-02 | Preview tab shows rendered markdown          | ❌      |
| TC-M13-03 | att: image links display inline              | ❌      |
| TC-M13-04 | att: non-image links are clickable           | ❌      |
| TC-M13-05 | Preview updates when switching from Edit     | ❌      |
| TC-M13-06 | Preview is read-only (no cursor, no editing) | ❌      |
| TC-M13-07 | Broken att: link shows placeholder           | ❌      |
| TC-M13-08 | External links open in default browser       | ❌      |

---

## M14: Trash

**Goal:** Trash section with restore and permanent delete.

**Description:** Trash section in left pane shows trashed keys. Restore button moves key back to active. Permanent
delete button removes key and files. GC runs on window hide and periodically.

**Implementation Notes:**

- Trash section: fixed height ~2.5 rows at bottom
- Click required to enter trash section from active keys
- Arrow navigation within trash section (bounded)
- Trashed keys are read-only (must restore to edit)
- Periodic GC: check elapsed time on window show (simpler than timer)
- GC must handle currently selected key being trashed (clear selection, refresh UI)

**GC Triggers:**

- Window hide → `maintenance()`
- Window show if >24h since last GC → `maintenance()`
- NOT on app quit (fast exit)

**Test Cases:**

| TC        | Description                                       | Status |
|-----------|---------------------------------------------------|--------|
| TC-M14-01 | Trash section shows trashed keys                  | ❌      |
| TC-M14-02 | Restore button moves key to active                | ❌      |
| TC-M14-03 | Permanent delete removes key and files            | ❌      |
| TC-M14-04 | Trashed key content is read-only                  | ❌      |
| TC-M14-05 | Drop onto trashed key rejected                    | ❌      |
| TC-M14-06 | Arrow keys navigate within trash section          | ❌      |
| TC-M14-07 | Click required to enter trash section from active | ❌      |

---

## M15: Settings

**Goal:** Settings dialog with persistent configuration.

**Description:** Modal settings dialog opened via Ctrl+, or tray menu. Changes saved to config.toml on dialog close.
Applied immediately to running app.

**Settings:**

| Category  | Setting         | Type                  | Default    |
|-----------|-----------------|-----------------------|------------|
| General   | Theme           | Dark / Light / System | System     |
| General   | Launch at Login | Toggle                | false      |
| General   | Show Tray Icon  | Toggle                | true       |
| Shortcuts | Global Shortcut | Key capture           | Ctrl+Alt+K |
| Lifecycle | Trash TTL       | Days (1-365)          | 30 days    |
| Lifecycle | Purge TTL       | Days (1-365)          | 7 days     |

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M15-01 | Ctrl+, opens settings dialog                 | ❌      |
| TC-M15-02 | Tray menu opens settings                     | ❌      |
| TC-M15-03 | Theme change applies immediately             | ❌      |
| TC-M15-04 | Settings saved to config.toml                | ❌      |
| TC-M15-05 | Esc closes settings dialog                   | ❌      |
| TC-M15-06 | Launch at login toggle creates/removes entry | ❌      |
| TC-M15-07 | TTL settings are editable                    | ❌      |

---

## M16: Global Hotkey

**Goal:** System-wide Ctrl+Alt+K to show window.

**Description:** Register global hotkey on startup. Show window when pressed (even from background). Detect conflicts
with other applications. Fallback: double-click exe to show window.

**Implementation Notes:**

- `RegisterHotKey(hwnd, id, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x4B)`
- MOD_NOREPEAT prevents repeated WM_HOTKEY when held
- Configurable via settings (key capture dialog)
- Conflict detection: RegisterHotKey failure
- `UnregisterHotKey` on WM_DESTROY and when changing hotkey

**Conflict Handling:**

1. Show notification: "Shortcut in use by another application"
2. Open settings with shortcut field focused
3. User chooses different shortcut

**Test Cases:**

| TC        | Description                                 | Status |
|-----------|---------------------------------------------|--------|
| TC-M16-01 | Ctrl+Alt+K shows window from any app        | ❌      |
| TC-M16-02 | Hotkey works when window already visible    | ❌      |
| TC-M16-03 | Custom hotkey can be configured in settings | ❌      |
| TC-M16-04 | Double-click exe shows window as fallback   | ❌      |

---

## M17: Single Instance

**Goal:** Ensure only one instance runs at a time.

**Description:** Use named mutex to detect existing instance. If already running, activate existing window instead of
launching new.

**Implementation Notes:**

- `CreateMutexW` with name `"Local\\Keva_SingleInstance"`
- If mutex exists: `FindWindowW(class_name, None)` to locate existing window
- Send WM_COPYDATA to signal existing instance
- Existing instance handles WM_COPYDATA by showing window
- If window minimized: `ShowWindow(SW_RESTORE)` before `SetForegroundWindow`

**Test Cases:**

| TC        | Description                             | Status |
|-----------|-----------------------------------------|--------|
| TC-M17-01 | Second launch activates existing window | ❌      |
| TC-M17-02 | Second launch exits after activation    | ❌      |
| TC-M17-03 | Works when existing window is hidden    | ❌      |

---

## M18: Window Position Memory

**Goal:** Remember window position per monitor.

**Description:** Store window position and size in config.toml keyed by monitor identifier. Restore position on
subsequent launches. Handle monitor configuration changes gracefully.

**Implementation Notes:**

- Monitor ID via `MONITORINFOEXW::szDevice` (e.g., `\\.\DISPLAY1`)
- Position stored in `[window.monitors."DISPLAY1"]` section
- Off-screen check: if position outside current monitors, center on cursor's monitor
- First launch: center on primary monitor

**Test Cases:**

| TC        | Description                             | Status |
|-----------|-----------------------------------------|--------|
| TC-M18-01 | Position restored on next launch        | ❌      |
| TC-M18-02 | Size restored on next launch            | ❌      |
| TC-M18-03 | First launch centers on primary monitor | ❌      |

---

## M19: First-Run Dialog

**Goal:** Welcome experience on first launch.

**Description:** Detect first launch (no config.toml). Show welcome dialog with launch-at-login checkbox. Create
config.toml with user preferences.

**Dialog Content:**

```
┌─────────────────────────────────────────────────┐
│ Welcome to Keva                                 │
│                                                 │
│ Keva stores your notes and files locally.       │
│ Press Ctrl+Alt+K anytime to open this window.   │
│                                                 │
│ ☑ Launch Keva at login                          │
│                                                 │
│                              [Get Started]      │
└─────────────────────────────────────────────────┘
```

**Test Cases:**

Setup: Delete config.toml before testing.

| TC        | Description                                 | Status |
|-----------|---------------------------------------------|--------|
| TC-M19-01 | First launch (no config) shows welcome      | ❌      |
| TC-M19-02 | Get Started button closes dialog            | ❌      |
| TC-M19-03 | Subsequent launches skip welcome dialog     | ❌      |
| TC-M19-04 | Launch at login checkbox persists to config | ❌      |

---

## M20: Monaco Bundling

**Goal:** Embed Monaco and resources in single executable.

**Description:** Bundle Monaco editor files, HTML, CSS, JS into the executable. Serve via custom protocol or virtual
host mapping. Ensure offline operation.

**Implementation Notes:**

- `include_bytes!` or `rust-embed` crate
- Monaco files: editor.main.js, editor.main.css, etc.
- Option A: `SetVirtualHostNameToFolderMapping` (simpler)
- Option B: Custom WebView2 scheme `keva://resources/`

**Test Cases:**

Setup: Disconnect network or use airplane mode.

| TC        | Description                             | Status |
|-----------|-----------------------------------------|--------|
| TC-M20-01 | App launches without network connection | ❌      |
| TC-M20-02 | Monaco editor functions without network | ❌      |
| TC-M20-03 | All UI assets load (no broken images)   | ❌      |

---

## M21: Installer

**Goal:** Professional installer with clean uninstall and WebView2 version check.

**Description:** Create Windows installer (WiX or MSIX). Install to Program Files. Register in Add/Remove Programs.
Uninstaller removes files and optionally data. Verify WebView2 Runtime meets minimum version requirement.

**Installation:**

- Install to `%ProgramFiles%\Keva`
- Add to Start Menu
- Register uninstaller in registry

**WebView2 Version Check:**

Keva requires WebView2 Runtime with `ICoreWebView2CompositionController5` support (DragStarting API).

| Scenario                    | Installer Behavior                          |
|-----------------------------|---------------------------------------------|
| WebView2 not installed      | Prompt to download/install WebView2 Runtime |
| WebView2 version too old    | Prompt to update via Windows Update         |
| WebView2 version sufficient | Continue installation                       |

**Uninstallation:**

1. Remove startup registry entry
2. Remove application files
3. Prompt: "Delete all Keva data?"

- Yes: Remove `%LOCALAPPDATA%\keva`
- No: Leave data intact

**Test Cases:**

| TC        | Description                            | Status |
|-----------|----------------------------------------|--------|
| TC-M21-01 | Installer completes without error      | ❌      |
| TC-M21-02 | App appears in Start Menu              | ❌      |
| TC-M21-03 | App appears in Add/Remove Programs     | ❌      |
| TC-M21-04 | Uninstaller removes application files  | ❌      |
| TC-M21-05 | Uninstaller prompts for data deletion  | ❌      |
| TC-M21-06 | "Yes" deletes data directory           | ❌      |
| TC-M21-07 | "No" preserves data directory          | ❌      |
| TC-M21-08 | Upgrade install preserves user data    | ❌      |
| TC-M21-09 | Installer detects missing WebView2     | ❌      |
| TC-M21-10 | Installer detects outdated WebView2    | ❌      |
| TC-M21-11 | Installer proceeds with valid WebView2 | ❌      |

---

## M22: Layout Polish

**Goal:** Resizable panes with persistent layout preferences.

**Description:** Add draggable dividers between panes. Left/right divider resizes key list width. Editor/attachments
divider resizes attachments panel height. Sizes persist across sessions. Handle window resize gracefully by clamping
pane sizes to valid ranges.

**Implementation Notes:**

- Drag handle between left and right panes (4-6px wide)
- Drag handle between editor and attachments pane (4-6px tall)
- Cursor changes to `col-resize` / `row-resize` on hover
- Left pane: min 150px, max 50% of window width
- Attachments pane: min 60px, max 50% of right pane height
- On window resize: clamp pane sizes if exceeds max
- Persist sizes to config (not ratios)

**Test Cases:**

| TC        | Description                                     | Status |
|-----------|-------------------------------------------------|--------|
| TC-M22-01 | Drag divider resizes left pane                  | ❌      |
| TC-M22-02 | Left pane respects minimum width (150px)        | ❌      |
| TC-M22-03 | Left pane respects maximum width (50% window)   | ❌      |
| TC-M22-04 | Pane sizes persist after restart                | ❌      |
| TC-M22-05 | Window resize clamps pane sizes if needed       | ❌      |
| TC-M22-06 | Cursor shows col-resize on left/right divider   | ❌      |
| TC-M22-07 | Drag divider resizes attachments pane height    | ❌      |
| TC-M22-08 | Attachments pane respects minimum height (60px) | ❌      |
| TC-M22-09 | Attachments pane respects maximum height (50%)  | ❌      |
| TC-M22-10 | Cursor shows row-resize on editor/att divider   | ❌      |

---