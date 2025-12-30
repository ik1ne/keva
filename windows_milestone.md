# Windows Implementation Milestones

This document defines the implementation milestones for Keva on Windows. Each milestone builds upon the previous ones
and includes test cases for verification.

## Milestone Overview

| #   | Milestone              | Description                                  | Status |
|-----|------------------------|----------------------------------------------|--------|
| M0  | keva_core Verification | Verify keva_core matches keva_core.md spec   | ❌      |
| M1  | Window Skeleton        | Borderless window, resize, tray icon         | ✅      |
| M2  | WebView + Bridge       | WebView2 hosting, postMessage, dark theme    | ✅      |
| M3  | Worker Thread          | Main↔Worker mpsc, keva_core integration      | ❌      |
| M4  | Search Engine          | Nucleo on main thread, progressive results   | ❌      |
| M5  | Key List               | Left pane, create/rename/delete, selection   | ❌      |
| M6  | Monaco Editor          | FileSystemHandle, markdown mode, auto-save   | ❌      |
| M7  | Four-State Focus       | Focus model, keyboard navigation, dimming    | ❌      |
| M8  | Attachments Panel      | File list, thumbnails, drag to Monaco        | ❌      |
| M9  | Clipboard              | Native read, paste intercept, copy shortcuts | ❌      |
| M10 | Edit/Preview Toggle    | Markdown renderer, att: link transform       | ❌      |
| M11 | Trash                  | Trash section, restore, GC triggers          | ❌      |
| M12 | Settings               | Dialog, config persistence, theme            | ❌      |
| M13 | Global Hotkey          | Ctrl+Alt+K registration, conflict detection  | ❌      |
| M14 | Single Instance        | Named mutex, activate existing window        | ❌      |
| M15 | Window Position Memory | Per-monitor position, off-screen check       | ❌      |
| M16 | First-Run Dialog       | Welcome message, launch at login checkbox    | ❌      |
| M17 | Monaco Bundling        | Embed resources, single exe                  | ❌      |
| M18 | Installer              | WiX/MSIX, uninstaller, data deletion prompt  | ❌      |

---

## M0: Core Crates Verification

**Goal:** Verify keva_core and keva_search implementations match their specifications.

**Description:** Review existing crate implementations against their specification documents. For keva_core: verify the
unified data model (markdown + attachments), storage structure with separate content/, blobs/, thumbnails/ trees,
attachment operations with conflict resolution, and thumbnail versioning. For keva_search: verify the dual-index
architecture, tombstone-based deletion, stop-at-threshold behavior, and maintenance compaction. This milestone is
complete when both crates compile with the specified API surface and pass their test suites.

**Dependencies:** None

**keva_core Key APIs:**

| Category    | APIs                                                                                       |
|-------------|--------------------------------------------------------------------------------------------|
| Lifecycle   | `open(config)`                                                                             |
| Key Ops     | `get()`, `active_keys()`, `trashed_keys()`, `touch()`, `rename()`                          |
| Content     | `get_content_path()`, `create()`, `mark_content_modified()`                                |
| Attachments | `get_attachment_path()`, `add_attachments()`, `remove_attachment()`, `rename_attachment()` |
| Thumbnails  | `get_thumbnail_path()` with `THUMB_VER` check                                              |
| Trash       | `trash()`, `restore()`, `purge()`                                                          |
| Clipboard   | `read_clipboard()`, `copy_text_to_clipboard()`, `copy_attachments_to_clipboard()`          |
| Maintenance | `maintenance()`                                                                            |

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
| TC-M0-01 | keva_core compiles with specified API surface                            | ❌      |
| TC-M0-02 | keva_core storage structure matches spec (content/, blobs/, thumbnails/) | ❌      |
| TC-M0-03 | keva_core Key validation enforces constraints (1-256 chars, trimmed)     | ❌      |
| TC-M0-04 | keva_core attachment conflict resolution works (Overwrite/Rename/Skip)   | ❌      |
| TC-M0-05 | keva_core thumbnail versioning triggers regeneration                     | ❌      |
| TC-M0-06 | keva_core 1GB file limit enforced                                        | ❌      |
| TC-M0-07 | keva_core lifecycle transitions correct (Active→Trash→Purge)             | ❌      |
| TC-M0-08 | keva_core timestamp updates match spec                                   | ❌      |
| TC-M0-09 | keva_core test suite passes                                              | ❌      |
| TC-M0-10 | keva_search compiles with specified API surface                          | ❌      |
| TC-M0-11 | keva_search dual-index architecture (active/trashed)                     | ❌      |
| TC-M0-12 | keva_search tombstone-based deletion works                               | ❌      |
| TC-M0-13 | keva_search stop-at-threshold behavior (100 active, 20 trashed)          | ❌      |
| TC-M0-14 | keva_search index compaction triggers at rebuild_threshold               | ❌      |
| TC-M0-15 | keva_search smart case matching works                                    | ❌      |
| TC-M0-16 | keva_search test suite passes                                            | ❌      |

---

## M1: Window Skeleton

**Goal:** Borderless window with system tray and basic window management.

**Description:** Native Rust window using `windows` crate. No title bar, system-metrics-based outer resize zone, always
on top. System tray icon with left-click toggle and right-click context menu. DPI-aware rendering. Esc hides window
without destroying it. Window stays on top to enable drag/drop from other apps.

**Dependencies:** None

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
| TC-M1-09 | Alt+F4 quits application entirely          | ❌      |
| TC-M1-10 | Corner drag resizes diagonally             | ❌      |
| TC-M1-11 | Window respects minimum size (400x300)     | ❌      |
| TC-M1-12 | Aero Snap to left edge (half-screen)       | ❌      |
| TC-M1-13 | Aero Snap to right edge (half-screen)      | ❌      |
| TC-M1-14 | Aero Snap to top edge (maximize)           | ❌      |
| TC-M1-15 | Aero Snap to corner (quarter-screen)       | ❌      |
| TC-M1-16 | Drag from maximized restores window        | ❌      |
| TC-M1-17 | Resize border scales correctly at 150% DPI | ❌      |

---

## M2: WebView + Bridge

**Goal:** WebView2 hosting with bidirectional message passing.

**Description:** Embed WebView2 control in the native window. Establish postMessage bridge for Native↔WebView
communication. Apply dark theme to WebView. Load initial HTML shell with placeholder content.

**Dependencies:** M1

**Implementation Notes:**

- WebView2 SDK 1.0.2470+ required for FileSystemHandle
    - Check `ICoreWebView2Environment14` availability at runtime for graceful fallback
- `CreateCoreWebView2EnvironmentWithOptions`
- `add_WebMessageReceived` for JS→Native
- `PostWebMessageAsJson` for Native→JS
- Dark background via `ICoreWebView2Controller2::SetDefaultBackgroundColor()`
- Runtime detection: `GetAvailableCoreWebView2BrowserVersionString`
    - If missing: show download prompt with link to WebView2 installer

**Message Protocol:**

```typescript
// Native → WebView
{
    type: "searchResults", keys
:
    [...]
}
{
    type: "clipboard", content
:
    {
        text: "..."
    }
|
    {
        files: [...]
    }
}
{
    type: "fileHandle", handle
:
    FileSystemFileHandle
}

// WebView → Native
{
    type: "search", query
:
    "..."
}
{
    type: "readClipboard"
}
{
    type: "createKey", key
:
    "..."
}
{
    type: "hide"
}
{
    type: "quit"
}
```

**Test Cases:**

| TC       | Description                                       | Status |
|----------|---------------------------------------------------|--------|
| TC-M2-01 | WebView2 loads without error                      | ✅      |
| TC-M2-02 | HTML content renders in WebView                   | ✅      |
| TC-M2-03 | JS postMessage reaches Native handler             | ✅      |
| TC-M2-04 | Native PostWebMessageAsJson reaches JS            | ✅      |
| TC-M2-05 | Dark theme applied (no white flash)               | ✅      |
| TC-M2-06 | DevTools accessible via F12 (debug builds)        | ✅      |
| TC-M2-07 | App shows error if WebView2 runtime not installed | ❌      |
| TC-M2-08 | Large message (>1MB) transfers correctly          | ❌      |

---

## M3: Worker Thread

**Goal:** Background thread for keva_core operations.

**Description:** Spawn worker thread on startup. Main thread sends requests via mpsc channel. Worker executes keva_core
operations and posts results back. Main thread receives results via custom window message.

**Dependencies:** M0, M2

**Implementation Notes:**

- `std::sync::mpsc::channel` for Main→Worker
- `PostMessageW(WM_WORKER_RESULT)` for Worker→Main
- Worker owns `KevaCore` instance exclusively
- Request/Response enums for type-safe messaging
- Error handling: Response enum includes Result variants for error propagation
- Panic handling: `catch_unwind` in worker loop to prevent main thread crash

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

| TC       | Description                              | Status |
|----------|------------------------------------------|--------|
| TC-M3-01 | Worker thread starts on app launch       | ❌      |
| TC-M3-02 | Request sent from main reaches worker    | ❌      |
| TC-M3-03 | Response posted back to main thread      | ❌      |
| TC-M3-04 | Multiple rapid requests handled in order | ❌      |
| TC-M3-05 | Worker thread exits cleanly on app quit  | ❌      |
| TC-M3-06 | Worker error propagates to main thread   | ❌      |
| TC-M3-07 | Worker panic doesn't crash main thread   | ❌      |

---

## M4: Search Engine

**Goal:** Fuzzy search with progressive results.

**Description:** Integrate Nucleo for fuzzy matching on main thread. SearchEngine wrapper provides
set_query/tick/results API. Nucleo's notify callback triggers WM_SEARCH_READY. Results capped at 100 active + 20 trashed
keys.

**Dependencies:** M3

**Implementation Notes:**

- `nucleo` crate for fuzzy matching
- SearchEngine runs on main thread (Nucleo spawns internal threadpool)
- `notify` callback → `PostMessageW(WM_SEARCH_READY)`
- Smart case: case-insensitive unless query has uppercase
- Active keys always ranked before trashed keys

**API:**

```rust
impl SearchEngine {
    fn new(notify: impl Fn() + Send + Sync) -> Self;
    fn set_items(&mut self, active: Vec<Key>, trashed: Vec<Key>);
    fn set_query(&mut self, query: &str);
    fn tick(&mut self) -> bool;  // true if results changed
    fn results(&self) -> (&[Key], &[Key]);  // (active, trashed)
}
```

**Test Cases:**

| TC       | Description                                       | Status |
|----------|---------------------------------------------------|--------|
| TC-M4-01 | Empty query returns all keys                      | ❌      |
| TC-M4-02 | Query filters keys by fuzzy match                 | ❌      |
| TC-M4-03 | Results update progressively (not blocked)        | ❌      |
| TC-M4-04 | Active keys appear before trashed keys            | ❌      |
| TC-M4-05 | Maximum 100 active results returned               | ❌      |
| TC-M4-06 | Maximum 20 trashed results returned               | ❌      |
| TC-M4-07 | Smart case: "abc" matches "ABC", "Abc" doesn't    | ❌      |
| TC-M4-08 | Special characters in query don't crash (*, [, \) | ❌      |
| TC-M4-09 | Unicode query matches unicode keys                | ❌      |

---

## M5: Key List

**Goal:** Left pane with key display and management.

**Description:** Render filtered key list in left pane. Support single selection with keyboard and mouse. Create new
keys via search bar Enter. Rename and delete keys with inline controls. Visual distinction between active and trashed
keys.

**Dependencies:** M4

**Implementation Notes:**

- Virtual scrolling for large key lists
- Inline rename editor on pen icon click
- Delete follows configured delete_style (soft/immediate)
- Trash section at bottom with fixed height (~2.5 rows)
- Long key names: truncate with ellipsis (max display width = pane width - padding)
- Inline error message: red text below input field

**Key Interactions:**

- Click key → select and show in right pane
- Down arrow from search → focus first key
- Up arrow from first key → focus search bar
- Enter on selected key → focus right top pane
- Delete key → trash or immediate delete

**Test Cases:**

| TC       | Description                                         | Status |
|----------|-----------------------------------------------------|--------|
| TC-M5-01 | Keys display in left pane                           | ❌      |
| TC-M5-02 | Click key selects it                                | ❌      |
| TC-M5-03 | Arrow keys navigate key list                        | ❌      |
| TC-M5-04 | Enter in search bar creates new key                 | ❌      |
| TC-M5-05 | Enter in search bar selects existing key            | ❌      |
| TC-M5-06 | Rename key via inline editor                        | ❌      |
| TC-M5-07 | Delete key moves to trash (soft mode)               | ❌      |
| TC-M5-08 | Delete key permanently removes (immediate mode)     | ❌      |
| TC-M5-09 | Trash section shows at bottom                       | ❌      |
| TC-M5-10 | Trashed keys show 🗑️ icon                          | ❌      |
| TC-M5-11 | Long key name shows ellipsis                        | ❌      |
| TC-M5-12 | Rename to empty string rejected                     | ❌      |
| TC-M5-13 | Rename to >256 chars rejected with inline error     | ❌      |
| TC-M5-14 | Rename to existing key shows overwrite confirmation | ❌      |
| TC-M5-15 | Keys with / display correctly (flat, no hierarchy)  | ❌      |

---

## M6: Monaco Editor

**Goal:** Markdown editor with direct file access.

**Description:** Embed Monaco editor in right top pane. Use FileSystemHandle API for direct blob file read/write.
Markdown language mode with syntax highlighting. Placeholder text when empty. Auto-save via FileSystemHandle (no
postMessage serialization).

**Dependencies:** M3, M5

**Implementation Notes:**

- Monaco loaded from bundled resources (see M17)
- `PostWebMessageAsJsonWithAdditionalObjects` for FileSystemHandle
- Monaco config: `pasteAs: { enabled: false }`, `dragAndDrop: true`
- Placeholder: "Type something, or drag files here..."
- `mark_content_modified()` called on key switch and window hide
- Error UI: Toast notification for FileSystemHandle failures (permission denied, locked)
- Debounce: 100ms delay before loading content on rapid key switching

**FileSystemHandle Flow:**

```
1. User selects key
2. Native: get_content_path() → path
3. Native: create FileSystemHandle for path
4. Native: PostWebMessageAsJsonWithAdditionalObjects(handle)
5. WebView: Monaco reads/writes via handle
6. User switches key or hides window
7. Native: mark_content_modified()
```

**Test Cases:**

| TC       | Description                                    | Status |
|----------|------------------------------------------------|--------|
| TC-M6-01 | Monaco editor loads                            | ❌      |
| TC-M6-02 | Content loads from file on key select          | ❌      |
| TC-M6-03 | Edits save directly to file                    | ❌      |
| TC-M6-04 | Markdown syntax highlighting works             | ❌      |
| TC-M6-05 | Placeholder shows when content empty           | ❌      |
| TC-M6-06 | Large file (10MB) loads without hang           | ❌      |
| TC-M6-07 | mark_content_modified called on key switch     | ❌      |
| TC-M6-08 | Permission denied shows error message          | ❌      |
| TC-M6-09 | File locked by another process shows error     | ❌      |
| TC-M6-10 | Switching keys rapidly doesn't corrupt content | ❌      |

---

## M7: Four-State Focus

**Goal:** Mutually exclusive focus between four panes.

**Description:** Implement four-state focus model: search bar, left pane, right top, right bottom. Only one pane active
at a time. Visual indicators for active/inactive state. Keyboard navigation between panes.

**Dependencies:** M5, M6

**Implementation Notes:**

- Active pane: cursor visible, full highlight
- Inactive pane: no cursor, dimmed
- Tab order: search → left → right top → right bottom
- Esc from any pane: hide window (unless modal open)

**Focus States:**

| Active Pane  | Search Bar | Left Pane   | Right Top | Right Bottom |
|--------------|------------|-------------|-----------|--------------|
| Search bar   | Cursor     | Dimmed      | No cursor | Dimmed       |
| Left pane    | No cursor  | Highlighted | No cursor | Dimmed       |
| Right top    | No cursor  | Dimmed      | Cursor    | Dimmed       |
| Right bottom | No cursor  | Dimmed      | No cursor | Highlighted  |

**Test Cases:**

| TC       | Description                                | Status |
|----------|--------------------------------------------|--------|
| TC-M7-01 | Only one pane shows active state           | ❌      |
| TC-M7-02 | Click pane activates it                    | ❌      |
| TC-M7-03 | Down arrow from search focuses left pane   | ❌      |
| TC-M7-04 | Enter from left pane focuses right top     | ❌      |
| TC-M7-05 | Inactive panes show dimmed styling         | ❌      |
| TC-M7-06 | Left pane selection persists when inactive | ❌      |
| TC-M7-07 | Tab key cycles through panes               | ❌      |
| TC-M7-08 | Shift+Tab reverse cycles                   | ❌      |
| TC-M7-09 | Ctrl+L focuses search bar from any pane    | ❌      |

---

## M8: Attachments Panel

**Goal:** Right bottom pane for file attachments.

**Description:** Display attachment list with filename, size, type icon. Generate and display thumbnails for images.
Support multi-select with Shift/Ctrl click. Add files via button or drop. Drag attachment to Monaco inserts link.

**Dependencies:** M6, M7

**Implementation Notes:**

- Thumbnail generation on import (worker thread)
- Supported thumbnail formats: png, jpg, jpeg, gif, webp, svg
- Thumbnail stored as {filename}.thumb
- [X] button per attachment for removal
- Warning dialog if removing referenced attachment
- Empty state: show only [+ Add files] button centered
- Duplicate dialog: "'{filename}' already exists." with [Overwrite] [Rename] [Skip] [Cancel]
- Multi-file duplicate: adds "☐ Apply to all (N remaining)" checkbox

**Drag to Monaco:**

```
1. User drags attachment from panel
2. Drop on Monaco at cursor position
3. Insert: [filename](att:filename)
```

**Test Cases:**

| TC       | Description                                           | Status |
|----------|-------------------------------------------------------|--------|
| TC-M8-01 | Attachments list displays files                       | ❌      |
| TC-M8-02 | File size shown in human-readable format              | ❌      |
| TC-M8-03 | Image attachments show thumbnail                      | ❌      |
| TC-M8-04 | Non-image attachments show type icon                  | ❌      |
| TC-M8-05 | Click [+ Add files] opens file picker                 | ❌      |
| TC-M8-06 | Multi-select with Ctrl+click                          | ❌      |
| TC-M8-07 | Drag attachment to Monaco inserts link                | ❌      |
| TC-M8-08 | [X] button removes attachment                         | ❌      |
| TC-M8-09 | Warning shown when removing referenced attachment     | ❌      |
| TC-M8-10 | Duplicate filename shows Overwrite/Rename/Skip dialog | ❌      |
| TC-M8-11 | File >1GB rejected with error message                 | ❌      |
| TC-M8-12 | Directory drop rejected                               | ❌      |
| TC-M8-13 | Empty panel shows [+ Add files] centered              | ❌      |
| TC-M8-14 | Multi-file paste with "Apply to all" checkbox works   | ❌      |

---

## M9: Clipboard

**Goal:** Native clipboard integration with paste interception.

**Description:** Native reads clipboard via Win32 API (CF_HDROP for files). WebView intercepts paste and requests
clipboard from native. Context-aware paste behavior. Copy shortcuts for markdown, HTML, and files.

**Dependencies:** M6, M8

**Implementation Notes:**

- `OpenClipboard`, `GetClipboardData(CF_HDROP)` for files
- `GetClipboardData(CF_UNICODETEXT)` for text
- WebView: `addEventListener('paste', preventDefault)`
- WebView sends `{ type: "readClipboard" }` to native
- Text paste into attachments panel: confirmation dialog "Paste text as new file?"

**Copy Shortcuts:**

| Shortcut   | Action                             | On Success  |
|------------|------------------------------------|-------------|
| Ctrl+C     | Copy selection (context-dependent) | Stay open   |
| Ctrl+Alt+T | Copy whole markdown as plain text  | Hide window |
| Ctrl+Alt+R | Copy rendered preview as HTML      | Hide window |
| Ctrl+Alt+F | Copy all attachments to clipboard  | Hide window |

**Test Cases:**

| TC       | Description                                            | Status |
|----------|--------------------------------------------------------|--------|
| TC-M9-01 | Paste text into search bar                             | ❌      |
| TC-M9-02 | Paste text into Monaco                                 | ❌      |
| TC-M9-03 | Paste files adds attachments + inserts links           | ❌      |
| TC-M9-04 | Ctrl+C in Monaco copies selected text                  | ❌      |
| TC-M9-05 | Ctrl+C in attachments copies selected files            | ❌      |
| TC-M9-06 | Ctrl+Alt+T copies markdown, hides window               | ❌      |
| TC-M9-07 | Ctrl+Alt+R copies rendered HTML, hides window          | ❌      |
| TC-M9-08 | Ctrl+Alt+F copies attachments, hides window            | ❌      |
| TC-M9-09 | "Nothing to copy" shown when no target key             | ❌      |
| TC-M9-10 | Paste files into search bar does nothing               | ❌      |
| TC-M9-11 | Empty clipboard paste does nothing                     | ❌      |
| TC-M9-12 | Paste text into right bottom shows confirmation dialog | ❌      |

---

## M10: Edit/Preview Toggle

**Goal:** Toggle between markdown editing and rendered preview.

**Description:** Two-tab interface in right top pane: Edit and Preview. Edit mode shows Monaco editor. Preview mode
shows rendered markdown with inline images. Attachment links (att:filename) transformed to blob paths for display.

**Dependencies:** M6, M8

**Implementation Notes:**

- Markdown renderer: marked.js or markdown-it
- `att:filename` → blob path transformation for images
- Non-image att: links remain clickable
- Preview is read-only
- Sanitization: DOMPurify to prevent XSS from malicious markdown
- Broken att: link: show placeholder icon with "File not found" tooltip
- External links (http://, https://): open in default browser

**Link Transformation:**

```markdown
<!-- Source -->
[image.png](att:image.png)

<!-- Preview renders as -->
<img src="file:///path/to/blobs/{key_hash}/image.png">
```

**Test Cases:**

| TC        | Description                                      | Status |
|-----------|--------------------------------------------------|--------|
| TC-M10-01 | Edit tab shows Monaco editor                     | ❌      |
| TC-M10-02 | Preview tab shows rendered markdown              | ❌      |
| TC-M10-03 | att: image links display inline                  | ❌      |
| TC-M10-04 | att: non-image links are clickable               | ❌      |
| TC-M10-05 | Preview updates when switching from Edit         | ❌      |
| TC-M10-06 | Preview is read-only                             | ❌      |
| TC-M10-07 | Broken att: link shows placeholder/error         | ❌      |
| TC-M10-08 | Malicious markdown doesn't execute scripts (XSS) | ❌      |
| TC-M10-09 | External links (http://) open in browser         | ❌      |

---

## M11: Trash

**Goal:** Trash section with restore and permanent delete.

**Description:** Trash section in left pane shows trashed keys. Restore button moves key back to active. Permanent
delete button removes key and files. GC runs on window hide and periodically (1 day interval).

**Dependencies:** M5

**Implementation Notes:**

- Trash section: fixed height ~2.5 rows at bottom
- Click required to enter trash section from active keys
- Arrow navigation within trash section (bounded)
- Trashed keys are read-only (must restore to edit)
- Timer: `SetTimer` with 24h interval for periodic GC
- Alternative: check elapsed time on window show (simpler, saves timer resource)

**GC Triggers:**

- Window hide → `maintenance()`
- Timer (every 24 hours) → `maintenance()`
- NOT on app quit (fast exit)

**Test Cases:**

| TC        | Description                                                       | Status |
|-----------|-------------------------------------------------------------------|--------|
| TC-M11-01 | Trash section shows trashed keys                                  | ❌      |
| TC-M11-02 | Restore button moves key to active                                | ❌      |
| TC-M11-03 | Permanent delete removes key and files                            | ❌      |
| TC-M11-04 | Trashed key is read-only                                          | ❌      |
| TC-M11-05 | GC runs on window hide                                            | ❌      |
| TC-M11-06 | GC moves expired active keys to trash                             | ❌      |
| TC-M11-07 | GC purges expired trashed keys                                    | ❌      |
| TC-M11-08 | Restore when active key with same name exists → conflict handling | ❌      |
| TC-M11-09 | Drop onto trashed key rejected                                    | ❌      |

---

## M12: Settings

**Goal:** Settings dialog with persistent configuration.

**Description:** Modal settings dialog opened via Ctrl+, or tray menu. Categories: General, Shortcuts, Data, Lifecycle.
Changes saved to config.toml on dialog close. Applied immediately to running app.

**Dependencies:** M1

**Settings:**

| Category  | Setting              | Type                  | Default    |
|-----------|----------------------|-----------------------|------------|
| General   | Theme                | Dark / Light / System | System     |
| General   | Launch at Login      | Toggle                | false      |
| General   | Show Tray Icon       | Toggle                | true       |
| Shortcuts | Global Shortcut      | Key capture           | Ctrl+Alt+K |
| Data      | Delete Style         | Soft / Immediate      | Soft       |
| Data      | Large File Threshold | Size (1MB-1GB)        | 50 MB      |
| Lifecycle | Trash TTL            | Days (1-365)          | 30 days    |
| Lifecycle | Purge TTL            | Days (1-365)          | 7 days     |

**Test Cases:**

| TC        | Description                                     | Status |
|-----------|-------------------------------------------------|--------|
| TC-M12-01 | Ctrl+, opens settings dialog                    | ❌      |
| TC-M12-02 | Tray menu opens settings                        | ❌      |
| TC-M12-03 | Theme change applies immediately                | ❌      |
| TC-M12-04 | Settings saved to config.toml                   | ❌      |
| TC-M12-05 | Invalid config shows validation popup           | ❌      |
| TC-M12-06 | Esc closes settings dialog                      | ❌      |
| TC-M12-07 | TTL values have min/max validation (1-365 days) | ❌      |
| TC-M12-08 | Large File Threshold has min/max (1MB-1GB)      | ❌      |

---

## M13: Global Hotkey

**Goal:** System-wide Ctrl+Alt+K to show window.

**Description:** Register global hotkey on startup. Show window when pressed (even from background). Detect conflicts
with other applications. Fallback: double-click exe to show window.

**Dependencies:** M1, M12

**Implementation Notes:**

- `RegisterHotKey(hwnd, id, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x4B)` (0x4B = 'K')
- MOD_NOREPEAT prevents repeated WM_HOTKEY when key is held
- Configurable via settings (key capture dialog)
- Conflict detection: RegisterHotKey failure
- `UnregisterHotKey` on WM_DESTROY and when changing hotkey

**Conflict Handling:**

1. Show notification: "Shortcut in use by another application"
2. Open settings with shortcut field focused
3. User chooses different shortcut

**Test Cases:**

| TC        | Description                                          | Status |
|-----------|------------------------------------------------------|--------|
| TC-M13-01 | Ctrl+Alt+K shows window from any app                 | ❌      |
| TC-M13-02 | Hotkey works when window already visible             | ❌      |
| TC-M13-03 | Custom hotkey can be configured                      | ❌      |
| TC-M13-04 | Conflict shows notification                          | ❌      |
| TC-M13-05 | Double-click exe shows window as fallback            | ❌      |
| TC-M13-06 | Hotkey unregistered on app exit                      | ❌      |
| TC-M13-07 | Changing hotkey in settings re-registers immediately | ❌      |

---

## M14: Single Instance

**Goal:** Ensure only one instance runs at a time.

**Description:** Use named mutex to detect existing instance. If already running, activate existing window instead of
launching new. Use WM_COPYDATA to signal existing instance.

**Dependencies:** M1

**Implementation Notes:**

- `CreateMutexW` with name `"Local\\Keva_SingleInstance"` (Local\\ = per-session)
- If mutex exists: `FindWindowW(class_name, None)` to locate existing window
- Send WM_COPYDATA to signal existing instance
- Existing instance handles WM_COPYDATA by showing window
- If window minimized: `ShowWindow(SW_RESTORE)` before `SetForegroundWindow`

**Test Cases:**

| TC        | Description                                       | Status |
|-----------|---------------------------------------------------|--------|
| TC-M14-01 | First launch creates mutex                        | ❌      |
| TC-M14-02 | Second launch detects existing instance           | ❌      |
| TC-M14-03 | Second launch activates existing window           | ❌      |
| TC-M14-04 | Second launch exits after activation              | ❌      |
| TC-M14-05 | Simultaneous double-launch race condition handled | ❌      |
| TC-M14-06 | Activate works when existing window is minimized  | ❌      |

---

## M15: Window Position Memory

**Goal:** Remember window position per monitor.

**Description:** Store window position and size in config.toml keyed by monitor identifier. Restore position on
subsequent launches. Handle monitor configuration changes gracefully.

**Dependencies:** M1, M12

**Implementation Notes:**

- Monitor ID via `MONITORINFOEXW::szDevice` (device name like `\\.\DISPLAY1`)
- HMONITOR is a runtime handle, not stable across reboots
- Position stored in `[window.monitors."DISPLAY1"]` section
- Off-screen check: if position outside current monitors, center on cursor's monitor
- First launch: center on primary monitor

**Test Cases:**

| TC        | Description                                         | Status |
|-----------|-----------------------------------------------------|--------|
| TC-M15-01 | Position saved on window move                       | ❌      |
| TC-M15-02 | Position restored on next launch                    | ❌      |
| TC-M15-03 | Different position per monitor                      | ❌      |
| TC-M15-04 | Off-screen position → center on cursor monitor      | ❌      |
| TC-M15-05 | New monitor config → center on primary              | ❌      |
| TC-M15-06 | Window size respects minimum constraints on restore | ❌      |

---

## M16: First-Run Dialog

**Goal:** Welcome experience on first launch.

**Description:** Detect first launch (no config.toml). Show welcome dialog with launch-at-login checkbox. Create
config.toml with user preferences. Register login item if checkbox checked.

**Dependencies:** M12

**Dialog Content:**

```
┌─────────────────────────────────────────────────┐
│ Welcome to Keva                                 │
│                                                 │
│ Keva stores your notes and files locally.       │
│ Press Ctrl+Alt+K anytime to open this window.   │
│                                                 │
│ ☑ Launch Keva at login                         │
│                                                 │
│                              [Get Started]      │
└─────────────────────────────────────────────────┘
```

**Test Cases:**

| TC        | Description                                     | Status |
|-----------|-------------------------------------------------|--------|
| TC-M16-01 | First launch shows welcome dialog               | ❌      |
| TC-M16-02 | Checkbox checked registers login item           | ❌      |
| TC-M16-03 | Checkbox unchecked skips login item             | ❌      |
| TC-M16-04 | Config.toml created after dialog                | ❌      |
| TC-M16-05 | Subsequent launches skip dialog                 | ❌      |
| TC-M16-06 | Enter key activates "Get Started" button        | ❌      |
| TC-M16-07 | Dialog has no X button (must click Get Started) | ❌      |

---

## M17: Monaco Bundling

**Goal:** Embed Monaco and resources in single executable.

**Description:** Bundle Monaco editor files, HTML, CSS, JS into the executable. Extract to temp or serve from memory.
Ensure offline operation without external dependencies.

**Dependencies:** M6

**Implementation Notes:**

- `include_bytes!` or `rust-embed` crate
- Monaco files: editor.main.js, editor.main.css, etc.
- Options: extract to %TEMP% on launch, or serve via custom protocol
- Custom WebView2 scheme: `keva://resources/`
- Alternative: `SetVirtualHostNameToFolderMapping` (simpler, maps hostname to folder)

**Test Cases:**

| TC        | Description                                | Status |
|-----------|--------------------------------------------|--------|
| TC-M17-01 | App runs without network                   | ❌      |
| TC-M17-02 | Monaco loads from bundled resources        | ❌      |
| TC-M17-03 | No external file dependencies              | ❌      |
| TC-M17-04 | Resources load quickly (< 500ms)           | ❌      |
| TC-M17-05 | Custom protocol keva:// resolves resources | ❌      |

---

## M18: Installer

**Goal:** Professional installer with clean uninstall.

**Description:** Create Windows installer (WiX or MSIX). Install to Program Files. Register in Add/Remove Programs.
Uninstaller removes files and optionally data.

**Dependencies:** All previous milestones

**Installation:**

- Install to `%ProgramFiles%\Keva`
- Add to Start Menu
- Register uninstaller in registry
- Optionally register login item

**Uninstallation:**

1. Remove startup registry entry
2. Remove application files
3. Prompt: "Delete all Keva data?"
    - Yes: Remove `%LOCALAPPDATA%\keva`
    - No: Leave data intact

**Test Cases:**

| TC        | Description                             | Status |
|-----------|-----------------------------------------|--------|
| TC-M18-01 | Installer completes without error       | ❌      |
| TC-M18-02 | App appears in Start Menu               | ❌      |
| TC-M18-03 | App appears in Add/Remove Programs      | ❌      |
| TC-M18-04 | Uninstaller removes application files   | ❌      |
| TC-M18-05 | Uninstaller prompts for data deletion   | ❌      |
| TC-M18-06 | "Yes" deletes data directory            | ❌      |
| TC-M18-07 | "No" preserves data directory           | ❌      |
| TC-M18-08 | App visible in Task Manager Startup tab | ❌      |
| TC-M18-09 | Upgrade install preserves user data     | ❌      |
| TC-M18-10 | Silent install (/quiet) works           | ❌      |
| TC-M18-11 | Installer requests UAC elevation        | ❌      |
| TC-M18-12 | Running app is closed before upgrade    | ❌      |

---

## Cross-Cutting Concerns

| Area             | Notes                                                |
|------------------|------------------------------------------------------|
| WebView2 Runtime | M2: Download prompt if runtime missing               |
| Logging          | Consider: Debug logging to %LOCALAPPDATA%\keva\logs\ |
| Accessibility    | Consider: High contrast mode support                 |

## Dependency Graph

```
M0 (keva_core) ─────────────────────────────────────────┐
                                                        │
M1 (Window) ──► M2 (WebView) ──► M3 (Worker) ──────────┤
     │              │                 │                 │
     │              │                 ▼                 │
     │              │            M4 (Search) ──────────►│
     │              │                 │                 │
     │              │                 ▼                 │
     │              │            M5 (Key List) ────────►│
     │              │                 │                 │
     │              │                 ▼                 │
     │              └───────────► M6 (Monaco) ─────────►│
     │                               │                  │
     │                               ▼                  │
     │                          M7 (Focus) ────────────►│
     │                               │                  │
     │                               ▼                  │
     │                          M8 (Attachments) ──────►│
     │                               │                  │
     │                               ▼                  │
     │                          M9 (Clipboard) ────────►│
     │                               │                  │
     │                               ▼                  │
     │                          M10 (Preview) ─────────►│
     │                                                  │
     ├──► M11 (Trash) ─────────────────────────────────►│
     │                                                  │
     ├──► M12 (Settings) ──► M13 (Hotkey) ─────────────►│
     │         │                                        │
     │         └──────────► M15 (Position) ────────────►│
     │                                                  │
     │                      M16 (First-Run) ───────────►│
     │                                                  │
     ├──► M14 (Single Instance) ───────────────────────►│
     │                                                  │
     └──► M6 ──► M17 (Bundling) ───────────────────────►│
                                                        │
                                                        ▼
                                                   M18 (Installer)
```

---

## Implementation Order (Recommended)

**Phase 1: Foundation**

1. M0 - keva_core (storage layer must be solid first)
2. M1 - Window Skeleton (already complete)
3. M2 - WebView + Bridge (already complete)
4. M3 - Worker Thread

**Phase 2: Core Features**

5. M4 - Search Engine
6. M5 - Key List
7. M6 - Monaco Editor
8. M7 - Four-State Focus

**Phase 3: Content Management**

9. M8 - Attachments Panel
10. M9 - Clipboard
11. M10 - Edit/Preview Toggle
12. M11 - Trash

**Phase 4: Polish**

13. M12 - Settings
14. M13 - Global Hotkey
15. M14 - Single Instance
16. M15 - Window Position Memory
17. M16 - First-Run Dialog

**Phase 5: Distribution**

18. M17 - Monaco Bundling
19. M18 - Installer