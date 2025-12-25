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

**Status:** Complete

**Requirements:**

| Requirement      | Description                                                | Status |
|------------------|------------------------------------------------------------|--------|
| Window style     | Borderless (WS_POPUP), no title bar or native controls     | ✅      |
| Resize           | 5px outer zone triggers OS resize                          | ✅      |
| Initial position | Centered on primary monitor                                | ✅      |
| Smooth resize    | DwmExtendFrameIntoClientArea enabled                       | ✅      |
| Tray icon        | Visible with tooltip "Keva"                                | ✅      |
| Tray left-click  | Toggle window visibility                                   | ✅      |
| Tray right-click | Context menu (Show, Settings, Launch at Login, Quit)       | ✅      |
| Esc key          | Hides window                                               | ✅      |
| Alt+Tab          | Window visible (taskbar icon remains - Windows limitation) | ✅      |

**Tray Menu Items:**

| Item            | Action                | Notes                        |
|-----------------|-----------------------|------------------------------|
| Show Keva       | Show window           | Disabled if already visible  |
| Settings...     | Open settings dialog  | Non-functional until M15-win |
| Launch at Login | Toggle checkbox       | Non-functional until M20-win |
| Quit Keva       | Terminate application |                              |

**Test Cases:**

| TC       | Description                            | Status |
|----------|----------------------------------------|--------|
| TC-M1-01 | Window appears centered on launch      | ✅      |
| TC-M1-02 | Drag from outer edge resizes window    | ✅      |
| TC-M1-03 | Tray icon visible with correct tooltip | ✅      |
| TC-M1-04 | Tray left-click toggles visibility     | ✅      |
| TC-M1-05 | Tray right-click shows menu            | ✅      |
| TC-M1-06 | Esc hides window                       | ✅      |
| TC-M1-07 | Window visible in Alt+Tab              | ✅      |
| TC-M1-08 | Quit menu item terminates app          | ✅      |

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
- Search icon is the primary drag handle for moving the window

### M3-win: Core Integration & Key List

**Goal:** Initialize keva_core, display active keys in left pane.

**Status:** Partial (keva_core init exists, key list basic)

**Requirements:**

| Requirement    | Description                                                        | Status |
|----------------|--------------------------------------------------------------------|--------|
| keva_core init | Initialize KevaCore on app startup                                 | ✅      |
| Data directory | Use default `%APPDATA%\Keva` or KEVA_DATA_DIR environment variable | ⚠️     |
| Config         | Load config.toml if exists, use defaults otherwise                 | ❌      |
| Key list       | Display all active keys from active_keys()                         | ✅      |
| Scrolling      | Key list scrolls when content exceeds viewport                     | ❌      |
| Empty state    | Empty database shows empty list (or "No keys" placeholder)         | ❌      |
| Refresh        | Key list reflects current database state on window show            | ❌      |

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
- Data directory: Code uses `%USERPROFILE%\.keva`, should be `%APPDATA%\Keva`
- Refresh on window show needed for consistency after external changes

### M4-win: Key Selection & Value Display

**Goal:** Click key to select, display value in right pane, implement three-state focus model.

**Status:** Not Started

**Requirements:**

| Requirement          | Description                                                   | Status |
|----------------------|---------------------------------------------------------------|--------|
| Click to select      | Clicking key in list selects it                               | ❌      |
| Selection highlight  | Selected key visually highlighted                             | ❌      |
| Right pane display   | Shows selected key's value                                    | ❌      |
| Text value           | Display text content (read-only for now)                      | ❌      |
| Files value          | Display placeholder "N file(s)" (detailed in M13)             | ❌      |
| Empty value          | Display placeholder                                           | ❌      |
| Touch on select      | Call touch() when key selected                                | ❌      |
| Three-state focus    | Search bar, left pane, right pane mutually exclusive          | ❌      |
| Search bar focus     | Clicking search bar clears key selection                      | ❌      |
| Search bar highlight | Visual focus indicator when search bar active                 | ❌      |
| Left pane dimmed     | When right pane focused, left pane key shows dimmed highlight | ❌      |

**Test Cases:**

| TC       | Description                                                                           | Status |
|----------|---------------------------------------------------------------------------------------|--------|
| TC-M4-01 | Clicking key highlights it, dims search bar                                           | ❌      |
| TC-M4-02 | Selected key's text value displays in right pane                                      | ❌      |
| TC-M4-03 | Clicking different key updates selection and right pane                               | ❌      |
| TC-M4-04 | Clicking search bar clears selection, restores normal text                            | ❌      |
| TC-M4-05 | Search bar shows pen icon when exact key exists (visual only, click deferred to M7)   | ❌      |
| TC-M4-06 | Search bar shows plus icon when key doesn't exist (visual only, click deferred to M7) | ❌      |
| TC-M4-07 | Button hidden when search bar empty                                                   | ❌      |
| TC-M4-08 | Button hidden when key selected in list                                               | ❌      |
| TC-M4-09 | Hovering button shows tooltip                                                         | ❌      |
| TC-M4-10 | Selecting key updates last_accessed                                                   | ❌      |
| TC-M4-11 | Typing clears selection and updates right pane live                                   | ❌      |
| TC-M4-12 | Files value shows placeholder text                                                    | ❌      |
| TC-M4-13 | Clicking right pane transfers focus to right pane                                     | ❌      |
| TC-M4-14 | Right pane focused shows dimmed highlight on left pane key                            | ❌      |

**Three-State Focus Model:**

The search bar, left pane, and right pane are mutually exclusive focus states.

| State              | Search Bar                 | Left Pane        | Right Pane                            |
|--------------------|----------------------------|------------------|---------------------------------------|
| Search bar focused | Focus highlight            | No selection     | Shows target based on search text     |
| Left pane focused  | Dimmed text, button hidden | Key highlighted  | Shows selected key's value            |
| Right pane focused | Dimmed text, button hidden | Dimmed highlight | Text cursor active / Files list shown |

**Focus Transitions:**

| From       | Action                    | To                                    |
|------------|---------------------------|---------------------------------------|
| Search bar | Down arrow                | Left pane (first key selected)        |
| Search bar | Click key in list         | Left pane                             |
| Search bar | Enter                     | Right pane (editing)                  |
| Search bar | Click right pane          | Right pane                            |
| Left pane  | Up arrow (from first key) | Search bar (cursor position restored) |
| Left pane  | Enter                     | Right pane (editing)                  |
| Left pane  | Click search bar          | Search bar                            |
| Left pane  | Click right pane          | Right pane                            |
| Right pane | Esc                       | Hide window                           |
| Right pane | Click search bar          | Search bar                            |
| Right pane | Click key in list         | Left pane                             |

**Search Bar States:**

| State                         | Text Style       | Button        | Right Pane                    |
|-------------------------------|------------------|---------------|-------------------------------|
| Empty                         | Gray placeholder | Hidden        | Empty                         |
| Text, key EXISTS              | Normal           | ✏️ Pen (edit) | Existing key's value          |
| Text, key DOESN'T EXIST       | Normal           | ➕ Plus (add)  | "Press Enter to add {key}..." |
| Inactive (left pane focused)  | Dimmed gray      | Hidden        | Selected key's value          |
| Inactive (right pane focused) | Dimmed gray      | Hidden        | Editing selected key          |

**Button Display (M4 scope - visual only):**

| State                | Icon   | Tooltip                |
|----------------------|--------|------------------------|
| Key EXISTS           | ✏️ Pen | "Edit {key} (Enter)"   |
| Key DOESN'T EXIST    | ➕ Plus | "Create {key} (Enter)" |
| Empty / Key selected | Hidden | -                      |

**Note:** Button click/Enter action deferred to M7-win.

### M5-win: Text Editor & Auto-Save

**Goal:** Editable text area in right pane with automatic saving.

**Status:** Not Started

**Requirements:**

| Requirement     | Description                                                 | Status |
|-----------------|-------------------------------------------------------------|--------|
| Text editing    | Right pane text content is editable                         | ❌      |
| Edit trigger    | Clicking in right pane text area enables editing            | ❌      |
| Auto-save       | Save changes after 3 seconds of inactivity (last keystroke) | ❌      |
| Save method     | Call upsert_text() on keva_core                             | ❌      |
| Key list update | New key appears in left pane after first save               | ❌      |
| Save on hide    | Save pending changes when window hides (Esc)                | ❌      |
| Save on quit    | Save pending changes when app exits (Alt+F4, tray Quit)     | ❌      |
| Save on switch  | Save pending changes when selecting different key           | ❌      |
| Empty text      | Saving empty string stores empty Text value (key preserved) | ❌      |

**Test Cases:**

| TC       | Description                                                   | Status |
|----------|---------------------------------------------------------------|--------|
| TC-M5-01 | Clicking text area allows typing                              | ❌      |
| TC-M5-02 | Changes auto-save after 3 seconds idle                        | ❌      |
| TC-M5-03 | Saved changes persist after app restart                       | ❌      |
| TC-M5-04 | Pressing Esc saves pending changes before hiding              | ❌      |
| TC-M5-05 | Quitting app saves pending changes                            | ❌      |
| TC-M5-06 | Switching selection saves pending changes to previous key     | ❌      |
| TC-M5-07 | Deleting all text saves empty string (key not deleted)        | ❌      |
| TC-M5-08 | Rapid typing delays save until 3 seconds after last keystroke | ❌      |

### M6-win: Clipboard Paste Handling

**Goal:** App-wide Ctrl+V with context-aware behavior.

**Status:** Not Started

**Requirements:**

| Requirement    | Description                                          | Status |
|----------------|------------------------------------------------------|--------|
| Paste scope    | App-wide Ctrl+V interception                         | ❌      |
| Clipboard read | Detect clipboard content type (text, files, both)    | ❌      |
| Files priority | If clipboard has both text and files, treat as files | ❌      |

**Paste Behavior by Context:**

| Focus                      | Clipboard | Action                                              |
|----------------------------|-----------|-----------------------------------------------------|
| Search bar                 | Text      | Insert text into search bar (as query)              |
| Search bar                 | Files     | Create/update key value for search bar text         |
| Right pane (text editor)   | Text      | Insert at cursor                                    |
| Right pane (text editor)   | Files     | Show warning, Ctrl+V again to overwrite             |
| Right pane (Files display) | Text      | Show warning, Ctrl+V again to overwrite             |
| Right pane (Files display) | Files     | Silent append                                       |
| Left pane (key selected)   | Files     | Silent append if Files value; replace if Text/empty |
| Left pane (key selected)   | Text      | Show warning if Files value; replace if Text value  |

**Overwrite Confirmation:**

| Element              | Description                                        |
|----------------------|----------------------------------------------------|
| Warning text         | Red text at bottom of right pane                   |
| Message (text→files) | "Press Ctrl+V again to replace text with files"    |
| Message (files→text) | "Press Ctrl+V again to replace files with text"    |
| Timeout              | Warning clears after 2 seconds or any other action |
| Second Ctrl+V        | Execute overwrite within timeout window            |

**File Size Handling:**

| Condition                       | Behavior                                     |
|---------------------------------|----------------------------------------------|
| Any file > 1GB                  | Reject entire paste with error message       |
| Any file > large_file_threshold | Reject entire paste with confirmation dialog |

**Test Cases:**

| TC       | Description                                                                            | Status |
|----------|----------------------------------------------------------------------------------------|--------|
| TC-M6-01 | Paste text with search bar focused inserts into search bar                             | ❌      |
| TC-M6-02 | Paste files with search bar focused creates/updates key value                          | ❌      |
| TC-M6-03 | Paste text into text editor inserts at cursor                                          | ❌      |
| TC-M6-04 | Paste files into text editor shows warning                                             | ❌      |
| TC-M6-05 | Paste text into Files display shows warning                                            | ❌      |
| TC-M6-06 | Paste files into Files display appends silently                                        | ❌      |
| TC-M6-07 | Second Ctrl+V within 2 seconds overwrites                                              | ❌      |
| TC-M6-08 | Warning clears after timeout                                                           | ❌      |
| TC-M6-09 | Clipboard with both text and files treated as files                                    | ❌      |
| TC-M6-10 | File over 1GB rejected with error                                                      | ❌      |
| TC-M6-11 | File over threshold shows confirmation dialog                                          | ❌      |
| TC-M6-12 | Duplicate file (same hash) silently ignored on append                                  | ❌      |
| TC-M6-13 | Paste files with search bar focused and key doesn't exist creates key with Files value | ❌      |

### M7-win: Search Integration & Filtering

**Goal:** Connect search bar to keva_search, filter key list, enable pen/plus button functionality.

**Status:** Not Started

**Requirements:**

| Requirement          | Description                                                                | Status |
|----------------------|----------------------------------------------------------------------------|--------|
| Search engine init   | Initialize SearchEngine on app startup with keys from keva_core            | ❌      |
| Query input          | Every keystroke calls set_query() and starts tick timer                    | ❌      |
| Tick timer           | Configurable interval (default 16ms) while search active                   | ❌      |
| Timer stop           | Stop timer when is_finished() returns true                                 | ❌      |
| Key list filtering   | Left pane shows only matching keys from search results                     | ❌      |
| Match highlighting   | Matched characters highlighted in key names (using match_indices)          | ❌      |
| Empty query          | Shows all keys (active first, then trashed)                                | ❌      |
| Search bar preserved | Window hide preserves search text, window show restores with text selected | ❌      |

**Button Functionality:**

| Button                     | Click Action                        |
|----------------------------|-------------------------------------|
| ✏️ Pen (key exists)        | Focus right pane editor             |
| ➕ Plus (key doesn't exist) | Create key, focus right pane editor |

**Enter Key (search bar focused):**

| Condition         | Action                               |
|-------------------|--------------------------------------|
| Key exists        | Focus right pane editor for that key |
| Key doesn't exist | Create key, focus right pane editor  |

**Index Maintenance:**

| Event                   | SearchEngine Call                              |
|-------------------------|------------------------------------------------|
| App startup             | new(active_keys, trashed_keys, config, notify) |
| Key created             | add_active(key)                                |
| Key deleted (soft)      | trash(key)                                     |
| Key deleted (permanent) | remove(key)                                    |
| Key restored            | restore(key)                                   |
| Key renamed             | rename(old, new)                               |

**Test Cases:**

| TC       | Description                                             | Status |
|----------|---------------------------------------------------------|--------|
| TC-M7-01 | Typing filters key list to matching keys                | ❌      |
| TC-M7-02 | Matched characters highlighted in key names             | ❌      |
| TC-M7-03 | Empty search bar shows all keys                         | ❌      |
| TC-M7-04 | Clicking pen button focuses right pane editor           | ❌      |
| TC-M7-05 | Clicking plus button creates key and focuses editor     | ❌      |
| TC-M7-06 | Enter with existing key focuses editor                  | ❌      |
| TC-M7-07 | Enter with new key creates and focuses editor           | ❌      |
| TC-M7-08 | Timer starts on keystroke, stops when search finishes   | ❌      |
| TC-M7-09 | Window hide preserves search text                       | ❌      |
| TC-M7-10 | Window show restores search text, all selected          | ❌      |
| TC-M7-11 | Created key appears in search results                   | ❌      |
| TC-M7-12 | Smart case: lowercase query matches any case            | ❌      |
| TC-M7-13 | Smart case: uppercase in query matches case-sensitively | ❌      |

### M8-win: Keyboard Navigation

**Goal:** Arrow keys, Enter, Delete, Escape, and Ctrl+Alt+C for efficient keyboard-driven workflow.

**Status:** Not Started

**Requirements:**

| Requirement                   | Description                                           | Status |
|-------------------------------|-------------------------------------------------------|--------|
| Down arrow (search bar)       | Move focus to first key in list                       | ❌      |
| Up arrow (search bar)         | No action (stay in search bar)                        | ❌      |
| Down arrow (key selected)     | Move selection to next key                            | ❌      |
| Up arrow (key selected)       | Move selection to previous key                        | ❌      |
| Up arrow (first key)          | Return to search bar, restore cursor to last position | ❌      |
| Down arrow (last key)         | No action (stay on last key)                          | ❌      |
| Enter (key selected)          | Focus right pane editor                               | ❌      |
| Delete (left pane focused)    | Delete selected key (follows delete_style)            | ❌      |
| Ctrl+Alt+C (right pane shown) | Copy value to clipboard, hide window                  | ❌      |
| Escape                        | Hide window (regardless of focus)                     | ❌      |

**Cursor Position Memory:**

| Event                                          | Behavior                              |
|------------------------------------------------|---------------------------------------|
| Leave search bar (down arrow)                  | Remember cursor position              |
| Return to search bar (up arrow from first key) | Restore cursor to remembered position |

**Ctrl+Alt+C Behavior:**

| Value Type | Clipboard Content                             |
|------------|-----------------------------------------------|
| Text       | Plain text copied                             |
| Files      | Files copied (platform file clipboard format) |
| Empty      | Empty string copied                           |

**Test Cases:**

| TC       | Description                                                        | Status |
|----------|--------------------------------------------------------------------|--------|
| TC-M8-01 | Down arrow from search bar selects first key                       | ❌      |
| TC-M8-02 | Up arrow from search bar does nothing                              | ❌      |
| TC-M8-03 | Down arrow moves to next key                                       | ❌      |
| TC-M8-04 | Up arrow moves to previous key                                     | ❌      |
| TC-M8-05 | Up arrow from first key returns to search bar with cursor restored | ❌      |
| TC-M8-06 | Down arrow from last key stays on last key                         | ❌      |
| TC-M8-07 | Enter on selected key focuses right pane                           | ❌      |
| TC-M8-08 | Ctrl+Alt+C copies text value and hides window                      | ❌      |
| TC-M8-09 | Ctrl+Alt+C copies files value and hides window                     | ❌      |
| TC-M8-10 | Ctrl+Alt+C with empty value copies empty string and hides window   | ❌      |
| TC-M8-11 | Escape hides window from any focus state                           | ❌      |
| TC-M8-12 | Copy updates last_accessed                                         | ❌      |
| TC-M8-13 | Delete key deletes selected key (follows delete_style)             | ❌      |

### M10-win: Inline Rename

**Goal:** Rename keys via inline editor in left pane.

**Status:** Not Started

**Note:** M9-win (Key Creation) skipped - functionality already covered in M7-win.

**Requirements:**

| Requirement       | Description                                        | Status |
|-------------------|----------------------------------------------------|--------|
| Rename button     | Pen icon on hover/selection in left pane key list  | ❌      |
| Click to rename   | Clicking pen icon opens inline text editor         | ❌      |
| Inline editor     | Replaces key name display with editable text field | ❌      |
| Initial selection | All text selected when editor opens                | ❌      |
| Confirm           | Enter confirms rename                              | ❌      |
| Cancel            | Escape cancels rename, restores original name      | ❌      |
| Click outside     | Confirms rename                                    | ❌      |
| Validation        | 1-256 chars, valid UTF-8 (enforced by Key type)    | ❌      |

**Note on Escape:** During inline rename, Escape cancels the rename (local behavior) rather than hiding the window (
global behavior). This is an exception to the normal Escape behavior.

**Rename Outcomes:**

| Condition              | Behavior                        |
|------------------------|---------------------------------|
| New name same as old   | No action, close editor         |
| New name doesn't exist | Rename key, update search index |
| New name exists        | Show confirmation dialog        |

**Overwrite Confirmation:**

| Element        | Description                                |
|----------------|--------------------------------------------|
| Dialog message | "Key '{new}' already exists. Overwrite?"   |
| Yes            | Overwrite (old target permanently deleted) |
| No             | Cancel rename, close editor                |

**Search Index Update:**

| Event            | Action                                |
|------------------|---------------------------------------|
| Rename confirmed | Call rename(old, new) on SearchEngine |

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M10-01 | Pen icon visible on key hover                | ❌      |
| TC-M10-02 | Clicking pen opens inline editor             | ❌      |
| TC-M10-03 | Text fully selected when editor opens        | ❌      |
| TC-M10-04 | Enter confirms rename                        | ❌      |
| TC-M10-05 | Escape cancels rename (does not hide window) | ❌      |
| TC-M10-06 | Click outside confirms rename                | ❌      |
| TC-M10-07 | Rename to same name closes editor, no action | ❌      |
| TC-M10-08 | Rename to new name updates key list          | ❌      |
| TC-M10-09 | Rename to existing name shows confirmation   | ❌      |
| TC-M10-10 | Overwrite confirmation Yes overwrites        | ❌      |
| TC-M10-11 | Overwrite confirmation No cancels            | ❌      |
| TC-M10-12 | Invalid name (empty, >256 chars) shows error | ❌      |
| TC-M10-13 | Renamed key searchable under new name        | ❌      |

### M11-win: Delete Key

**Goal:** Delete keys via trash icon in left pane or Delete key.

**Status:** Not Started

**Requirements:**

| Requirement      | Description                                           | Status |
|------------------|-------------------------------------------------------|--------|
| Delete button    | Trash icon on hover/selection in left pane key list   | ❌      |
| Click to delete  | Clicking trash icon deletes key                       | ❌      |
| Delete key       | Pressing Delete key deletes selected key              | ❌      |
| Delete style     | Follows delete_style setting (soft or immediate)      | ❌      |
| Soft delete      | Calls trash() on keva_core, key moves to trash        | ❌      |
| Immediate delete | Calls purge() on keva_core, key permanently deleted   | ❌      |
| No confirmation  | Delete executes immediately (trash provides recovery) | ❌      |

**Search Index Update:**

| Delete Style | Action                           |
|--------------|----------------------------------|
| Soft         | Call trash(key) on SearchEngine  |
| Immediate    | Call remove(key) on SearchEngine |

**Post-Delete State:**

| Condition                | Behavior                                                 |
|--------------------------|----------------------------------------------------------|
| Deleted key was selected | Selection clears, right pane updates based on search bar |
| Deleted key not selected | No change to selection                                   |

**Test Cases:**

| TC        | Description                                            | Status |
|-----------|--------------------------------------------------------|--------|
| TC-M11-01 | Trash icon visible on key hover                        | ❌      |
| TC-M11-02 | Clicking trash with soft delete moves key to trash     | ❌      |
| TC-M11-03 | Clicking trash with immediate delete removes key       | ❌      |
| TC-M11-04 | Deleted key disappears from active list                | ❌      |
| TC-M11-05 | Soft-deleted key appears in trash results              | ❌      |
| TC-M11-06 | Immediate-deleted key not in any results               | ❌      |
| TC-M11-07 | Selection clears when selected key deleted             | ❌      |
| TC-M11-08 | Right pane updates after selected key deleted          | ❌      |
| TC-M11-09 | Delete key deletes selected key (follows delete_style) | ❌      |

### M12-win: Trash UI

**Goal:** Display trashed keys in dedicated bottom section, enable restore and permanent delete.

**Status:** Not Started

**Left Pane Layout:**

```
┌─────────────────┐
│ Key list        │
│ (active keys)   │
│ (scrollable)    │
│                 │
├─────────────────┤
│ Trash (N)       │
│ (trashed keys)  │
└─────────────────┘
```

**Requirements:**

| Requirement             | Description                                                       | Status |
|-------------------------|-------------------------------------------------------------------|--------|
| Trash section           | Fixed height section at bottom (2-2.5x key row height)            | ❌      |
| Trash header            | "Trash (N)" label showing count                                   | ❌      |
| Visibility              | Trash section hidden when no trash matches                        | ❌      |
| Separate navigation     | Click required to enter trash from active keys                    | ❌      |
| Trash indicator         | 🗑️ icon prefix on trashed keys                                   | ❌      |
| Selection               | Clicking trashed key selects it                                   | ❌      |
| Right pane              | Shows trashed key's value (read-only)                             | ❌      |
| Restore button          | Visible when trashed key selected                                 | ❌      |
| Permanent delete button | Visible when trashed key selected                                 | ❌      |
| Arrow nav within trash  | Up/Down arrows navigate within trash section                      | ❌      |
| Arrow nav boundaries    | Up from first trash stays in trash; down from last stays in trash | ❌      |

**Trashed Key Actions:**

| Button           | Action                                              |
|------------------|-----------------------------------------------------|
| Restore          | Calls restore() on keva_core, key becomes active    |
| Permanent delete | Calls purge() on keva_core, key permanently removed |

**Search Index Update:**

| Action           | SearchEngine Call |
|------------------|-------------------|
| Restore          | restore(key)      |
| Permanent delete | remove(key)       |

**Button Placement:**

| Location      | Description                                                     |
|---------------|-----------------------------------------------------------------|
| Trash key row | Restore and permanent delete buttons (replaces pen/trash icons) |

**Test Cases:**

| TC        | Description                                              | Status |
|-----------|----------------------------------------------------------|--------|
| TC-M12-01 | Trash section appears at bottom when trash matches exist | ❌      |
| TC-M12-02 | Trash section hidden when no trash matches               | ❌      |
| TC-M12-03 | Trash header shows correct count                         | ❌      |
| TC-M12-04 | Trash section has fixed height                           | ❌      |
| TC-M12-05 | Trashed keys show 🗑️ icon prefix                        | ❌      |
| TC-M12-06 | Clicking trashed key selects it                          | ❌      |
| TC-M12-07 | Right pane shows trashed key's value                     | ❌      |
| TC-M12-08 | Right pane is read-only for trashed keys                 | ❌      |
| TC-M12-09 | Restore button visible for trashed key                   | ❌      |
| TC-M12-10 | Permanent delete button visible for trashed key          | ❌      |
| TC-M12-11 | Restore moves key to active list                         | ❌      |
| TC-M12-12 | Permanent delete removes key entirely                    | ❌      |
| TC-M12-13 | Restored key appears in active key list                  | ❌      |
| TC-M12-14 | Click required to enter trash section from active keys   | ❌      |
| TC-M12-15 | Up arrow from first trash key stays in trash             | ❌      |
| TC-M12-16 | Down arrow from last trash key stays in trash            | ❌      |
| TC-M12-17 | Arrow keys navigate within trash section                 | ❌      |

### M13-win: File Value Display

**Goal:** Display Files value as list with names, sizes, individual delete, and copy support.

**Status:** Not Started

**Requirements:**

| Requirement        | Description                                        | Status |
|--------------------|----------------------------------------------------|--------|
| File list          | Display each file as row with name and size        | ❌      |
| Size format        | Human-readable (e.g., "1.2 MB", "340 KB")          | ❌      |
| Scrollable         | File list scrolls if many files                    | ❌      |
| Delete individual  | X button on each file row                          | ❌      |
| Delete all         | Clear All button to remove all files               | ❌      |
| Empty after delete | Deleting last file results in empty Text value     | ❌      |
| Copy shortcut      | Ctrl+Alt+C copies files to clipboard, hides window | ❌      |

**File Row Layout:**

```
┌─────────────────────────────────┐
│ 📄 document.pdf    1.2 MB   [X] │
│ 📄 image.png       340 KB   [X] │
│ 📄 data.csv        12 KB    [X] │
│                                 │
│            [Clear All]          │
└─────────────────────────────────┘
```

**Delete Behavior:**

| Action                 | Result                                  |
|------------------------|-----------------------------------------|
| Delete individual file | Remove file from list, update keva_core |
| Delete last file       | Value becomes empty Text                |
| Clear all              | Value becomes empty Text                |

**Test Cases:**

| TC        | Description                                                   | Status |
|-----------|---------------------------------------------------------------|--------|
| TC-M13-01 | Files value displays file list                                | ❌      |
| TC-M13-02 | Each file shows name and size                                 | ❌      |
| TC-M13-03 | Size formatted human-readable                                 | ❌      |
| TC-M13-04 | File list scrolls when many files                             | ❌      |
| TC-M13-05 | X button removes individual file                              | ❌      |
| TC-M13-06 | Deleting last file results in empty value                     | ❌      |
| TC-M13-07 | Clear All removes all files                                   | ❌      |
| TC-M13-08 | Duplicate names display correctly (same name, different size) | ❌      |
| TC-M13-09 | File deletion updates keva_core                               | ❌      |
| TC-M13-10 | Ctrl+Alt+C copies files to clipboard and hides window         | ❌      |

### M14-win: Drag & Drop Files

**Goal:** Drop files or text onto left or right pane to store.

**Status:** Not Started

**Requirements:**

| Requirement              | Description                                            | Status |
|--------------------------|--------------------------------------------------------|--------|
| Drop target: right pane  | Drop onto right pane stores to current target key      | ❌      |
| Drop target: key in list | Drop onto specific key in left pane stores to that key | ❌      |
| Drop target: search bar  | Not a drop target                                      | ❌      |
| Drop on trashed key      | Rejected                                               | ❌      |
| Visual feedback          | Highlight drop target while dragging over              | ❌      |

**Drop Behavior Matrix:**

| Existing Value | Drop Content | Behavior                                |
|----------------|--------------|-----------------------------------------|
| Empty          | Files        | Accept, store as Files value            |
| Empty          | Text         | Accept, store as Text value             |
| Text           | Files        | Confirm: "Replace text with N file(s)?" |
| Text           | Text         | Confirm: "Replace existing text?"       |
| Files          | Files        | Silent append                           |
| Files          | Text         | Confirm: "Replace N file(s) with text?" |

**File Size Handling:**

| Condition                       | Behavior                                    |
|---------------------------------|---------------------------------------------|
| Any file > 1GB                  | Reject entire drop with error message       |
| Any file > large_file_threshold | Reject entire drop with confirmation dialog |

**Duplicate Handling:**

| Condition                  | Behavior                  |
|----------------------------|---------------------------|
| Same hash as existing file | Silently ignore duplicate |

**Test Cases:**

| TC        | Description                                       | Status |
|-----------|---------------------------------------------------|--------|
| TC-M14-01 | Drop files on right pane stores to target key     | ❌      |
| TC-M14-02 | Drop files on key in left pane stores to that key | ❌      |
| TC-M14-03 | Drop on empty value creates Files value           | ❌      |
| TC-M14-04 | Drop files on Text value shows confirmation       | ❌      |
| TC-M14-05 | Confirmation Yes replaces text with files         | ❌      |
| TC-M14-06 | Confirmation No cancels drop                      | ❌      |
| TC-M14-07 | Drop files on Files value appends silently        | ❌      |
| TC-M14-08 | Drop target highlights during drag                | ❌      |
| TC-M14-09 | File over 1GB rejected with error                 | ❌      |
| TC-M14-10 | File over threshold shows confirmation            | ❌      |
| TC-M14-11 | Duplicate file silently ignored                   | ❌      |
| TC-M14-12 | Drop on trashed key rejected                      | ❌      |
| TC-M14-13 | Drop text on empty value stores as Text           | ❌      |
| TC-M14-14 | Drop text on Text value shows confirmation        | ❌      |
| TC-M14-15 | Drop text on Files value shows confirmation       | ❌      |
| TC-M14-16 | Search bar is not a drop target                   | ❌      |

### M15-win: Settings Dialog

**Goal:** Settings UI with config persistence.

**Status:** Not Started

**Requirements:**

| Requirement       | Description                                     | Status |
|-------------------|-------------------------------------------------|--------|
| Open settings     | Ctrl+, or tray menu "Settings..."               | ❌      |
| Modal dialog      | Separate window, blocks main window interaction | ❌      |
| Save on close     | Changes saved to config.toml when dialog closes | ❌      |
| Apply immediately | Changes take effect without app restart         | ❌      |
| Close methods     | X button or Esc key closes dialog               | ❌      |

**Settings Fields:**

| Category  | Setting              | Control      | Values                       |
|-----------|----------------------|--------------|------------------------------|
| General   | Theme                | Dropdown     | Dark / Light / Follow System |
| General   | Launch at Login      | Checkbox     | On / Off                     |
| General   | Show Tray Icon       | Checkbox     | On / Off                     |
| Shortcuts | Global Shortcut      | Key capture  | Modifier+Key combination     |
| Data      | Delete Style         | Dropdown     | Soft / Immediate             |
| Data      | Large File Threshold | Number input | Bytes (default 256MB)        |
| Lifecycle | Trash TTL            | Number input | Days (default 30)            |
| Lifecycle | Purge TTL            | Number input | Days (default 7)             |

**Config File:**

| Requirement    | Description                                 |
|----------------|---------------------------------------------|
| Location       | `{data_dir}/config.toml`                    |
| Format         | TOML as specified in Spec.md                |
| Missing file   | Create with defaults                        |
| Invalid values | Show error popup on launch (handled in M19) |

**Test Cases:**

| TC        | Description                                | Status |
|-----------|--------------------------------------------|--------|
| TC-M15-01 | Ctrl+, opens settings dialog               | ❌      |
| TC-M15-02 | Tray menu "Settings..." opens dialog       | ❌      |
| TC-M15-03 | Theme change applies immediately           | ❌      |
| TC-M15-04 | Delete style change affects next delete    | ❌      |
| TC-M15-05 | Settings persist after app restart         | ❌      |
| TC-M15-06 | Closing dialog saves to config.toml        | ❌      |
| TC-M15-07 | All fields display current values on open  | ❌      |
| TC-M15-08 | Large file threshold accepts valid numbers | ❌      |
| TC-M15-09 | TTL fields accept valid numbers            | ❌      |
| TC-M15-10 | Launch at Login updates system setting     | ❌      |
| TC-M15-11 | Show Tray Icon toggles tray visibility     | ❌      |
| TC-M15-12 | Esc key closes settings dialog             | ❌      |

### M16-win: Global Hotkey

**Goal:** System-wide keyboard shortcut to show window, with conflict detection.

**Status:** Not Started

**Requirements:**

| Requirement      | Description                                  | Status |
|------------------|----------------------------------------------|--------|
| Default shortcut | Ctrl+Alt+K                                   | ❌      |
| Global scope     | Works when window hidden, other apps focused | ❌      |
| Show window      | Hotkey shows and focuses Keva window         | ❌      |
| Registration     | Register hotkey on app startup               | ❌      |
| Unregistration   | Unregister hotkey on app quit                | ❌      |
| Config sync      | Hotkey updates when changed in settings      | ❌      |

**Conflict Detection:**

| Condition          | Behavior                                                                  |
|--------------------|---------------------------------------------------------------------------|
| Registration fails | Another app has the shortcut                                              |
| On conflict        | Show notification: "Shortcut Ctrl+Alt+K is in use by another application" |
| After notification | Open settings dialog with shortcut field focused                          |

**Shortcut Change Flow:**

| Step | Action                                 |
|------|----------------------------------------|
| 1    | User opens settings, changes shortcut  |
| 2    | Unregister old shortcut                |
| 3    | Attempt register new shortcut          |
| 4    | If conflict, show error, revert to old |
| 5    | If success, save to config             |

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M16-01 | Hotkey shows window when hidden              | ❌      |
| TC-M16-02 | Hotkey shows window when other app focused   | ❌      |
| TC-M16-03 | Hotkey registered on app startup             | ❌      |
| TC-M16-04 | Hotkey unregistered on app quit              | ❌      |
| TC-M16-05 | Conflict shows notification                  | ❌      |
| TC-M16-06 | Conflict opens settings dialog               | ❌      |
| TC-M16-07 | Changing shortcut in settings updates hotkey | ❌      |
| TC-M16-08 | Invalid new shortcut reverts to old          | ❌      |
| TC-M16-09 | New shortcut persists after restart          | ❌      |

### M17-win: Single Instance Enforcement

**Goal:** Prevent multiple app instances, activate existing instance on relaunch.

**Status:** Not Started

**Requirements:**

| Requirement        | Description                                                       | Status |
|--------------------|-------------------------------------------------------------------|--------|
| Detection          | Check for existing instance on startup                            | ❌      |
| Mechanism          | Named mutex (e.g., "Keva_SingleInstance")                         | ❌      |
| Existing found     | Activate existing instance's window, exit new process             | ❌      |
| Activation message | Send message to existing instance to show window                  | ❌      |
| Timeout            | If existing instance unresponsive for 2 seconds, offer force-quit | ❌      |

**Force-Quit Dialog:**

| Element | Description                                        |
|---------|----------------------------------------------------|
| Message | "Keva is not responding. Force quit and relaunch?" |
| Yes     | Terminate existing process, continue startup       |
| No      | Exit new process                                   |

**Test Cases:**

| TC        | Description                                        | Status |
|-----------|----------------------------------------------------|--------|
| TC-M17-01 | First instance starts normally                     | ❌      |
| TC-M17-02 | Second instance activates first instance's window  | ❌      |
| TC-M17-03 | Second instance exits after activating first       | ❌      |
| TC-M17-04 | First instance window shows when activated         | ❌      |
| TC-M17-05 | Unresponsive instance triggers force-quit dialog   | ❌      |
| TC-M17-06 | Force-quit Yes terminates old, starts new          | ❌      |
| TC-M17-07 | Force-quit No exits new process                    | ❌      |
| TC-M17-08 | Mutex released on app quit                         | ❌      |
| TC-M17-09 | Crash leaves no stale mutex (Windows handles this) | ❌      |

### M18-win: Window Position Memory

**Goal:** Remember window position and size per monitor.

**Status:** Not Started

**Requirements:**

| Requirement      | Description                                                             | Status |
|------------------|-------------------------------------------------------------------------|--------|
| Save position    | Store window position and size on hide/quit                             | ❌      |
| Restore position | Restore on next window show                                             | ❌      |
| Per-monitor      | Position stored keyed by monitor identifier                             | ❌      |
| Monitor change   | If saved monitor not found, center on current monitor                   | ❌      |
| Off-screen check | If restored position is off-screen, center on monitor containing cursor | ❌      |

**Storage:**

| Location    | Description                        |
|-------------|------------------------------------|
| Config file | Window positions in config.toml    |
| Key format  | `[window.monitors."monitor_{id}"]` |
| Fields      | x, y, width, height                |

**Triggers:**

| Event              | Action                    |
|--------------------|---------------------------|
| Window hide (Esc)  | Save position             |
| App quit           | Save position             |
| Window move/resize | Save position (debounced) |

**Test Cases:**

| TC        | Description                                          | Status |
|-----------|------------------------------------------------------|--------|
| TC-M18-01 | Window position persists after hide and show         | ❌      |
| TC-M18-02 | Window position persists after app restart           | ❌      |
| TC-M18-03 | Window size persists after app restart               | ❌      |
| TC-M18-04 | Different monitors remember different positions      | ❌      |
| TC-M18-05 | Disconnected monitor falls back to center on current | ❌      |
| TC-M18-06 | Off-screen position corrected to visible area        | ❌      |
| TC-M18-07 | First launch centers on primary monitor              | ❌      |
| TC-M18-08 | Rapid move/resize doesn't spam config writes         | ❌      |

### M19-win: First-Run Dialog

**Goal:** Welcome dialog on first launch with launch-at-login option.

**Status:** Not Started

**Requirements:**

| Requirement | Description                              | Status |
|-------------|------------------------------------------|--------|
| Trigger     | Shown when no config.toml exists         | ❌      |
| Modal       | Blocks main window until dismissed       | ❌      |
| One-time    | Never shown again after first completion | ❌      |

**Dialog Content:**

| Element  | Description                                                                                            |
|----------|--------------------------------------------------------------------------------------------------------|
| Title    | "Welcome to Keva"                                                                                      |
| Message  | "Keva stores your clipboard snippets and files locally. Press Ctrl+Alt+K anytime to open this window." |
| Checkbox | "Launch Keva at login" (checked by default)                                                            |
| Button   | "Get Started"                                                                                          |

**Flow:**

| Step | Action                                   |
|------|------------------------------------------|
| 1    | App starts, no config.toml found         |
| 2    | Show first-run dialog                    |
| 3    | User clicks "Get Started"                |
| 4    | Create config.toml with defaults         |
| 5    | If checkbox checked, register login item |
| 6    | Show main window                         |

**Config Validation (on subsequent launches):**

| Condition                           | Behavior                              |
|-------------------------------------|---------------------------------------|
| config.toml missing                 | Show first-run dialog                 |
| config.toml invalid                 | Show error popup with specific errors |
| Error popup: "Launch with defaults" | Overwrite invalid fields, continue    |
| Error popup: "Quit"                 | Exit without modifying config         |

**Test Cases:**

| TC        | Description                                 | Status |
|-----------|---------------------------------------------|--------|
| TC-M19-01 | First-run dialog shown on fresh install     | ❌      |
| TC-M19-02 | Checkbox checked by default                 | ❌      |
| TC-M19-03 | "Get Started" creates config.toml           | ❌      |
| TC-M19-04 | Checked checkbox registers login item       | ❌      |
| TC-M19-05 | Unchecked checkbox skips login registration | ❌      |
| TC-M19-06 | Dialog not shown on subsequent launches     | ❌      |
| TC-M19-07 | Invalid config shows error popup            | ❌      |
| TC-M19-08 | "Launch with defaults" fixes invalid config | ❌      |
| TC-M19-09 | "Quit" exits without changes                | ❌      |

### M20-win: Installer & Distribution

**Goal:** Installable package with uninstaller and launch-at-login support.

**Status:** Not Started

**Requirements:**

| Requirement      | Description                                                          | Status |
|------------------|----------------------------------------------------------------------|--------|
| Installer format | Windows installer (WiX, MSIX, or similar)                            | ❌      |
| Install location | Program Files                                                        | ❌      |
| Start Menu       | Create shortcut                                                      | ❌      |
| Registry         | Register in Add/Remove Programs                                      | ❌      |
| Launch at login  | Registry entry in HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run | ❌      |

**Installer Flow:**

| Step | Action                              |
|------|-------------------------------------|
| 1    | User runs installer                 |
| 2    | Install to Program Files\Keva       |
| 3    | Create Start Menu shortcut          |
| 4    | Register in Add/Remove Programs     |
| 5    | Optionally launch app after install |

**Uninstaller Flow:**

| Step | Action                                          |
|------|-------------------------------------------------|
| 1    | User runs uninstaller (via Add/Remove Programs) |
| 2    | Remove application files                        |
| 3    | Remove Start Menu shortcut                      |
| 4    | Remove registry entries (including Run key)     |
| 5    | Prompt: "Delete all Keva data?"                 |
| 6    | Yes: Delete data directory (%APPDATA%\Keva)     |
| 7    | No: Leave data directory intact                 |

**Launch at Login:**

| Requirement  | Description                                        |
|--------------|----------------------------------------------------|
| Registry key | HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run |
| Value name   | "Keva"                                             |
| Value data   | Path to keva.exe                                   |
| Visibility   | Appears in Task Manager → Startup tab              |
| Toggle       | Settings checkbox adds/removes registry entry      |

**Test Cases:**

| TC        | Description                                                | Status |
|-----------|------------------------------------------------------------|--------|
| TC-M20-01 | Installer completes on clean system                        | ❌      |
| TC-M20-02 | App launches from Start Menu                               | ❌      |
| TC-M20-03 | App appears in Add/Remove Programs                         | ❌      |
| TC-M20-04 | Uninstaller removes application files                      | ❌      |
| TC-M20-05 | Uninstaller removes Start Menu shortcut                    | ❌      |
| TC-M20-06 | Uninstaller prompts for data deletion                      | ❌      |
| TC-M20-07 | Data deletion Yes removes %APPDATA%\Keva                   | ❌      |
| TC-M20-08 | Data deletion No preserves %APPDATA%\Keva                  | ❌      |
| TC-M20-09 | Launch at login enabled appears in Task Manager Startup    | ❌      |
| TC-M20-10 | Launch at login disabled removes from Task Manager Startup | ❌      |
| TC-M20-11 | App auto-starts after system reboot (when enabled)         | ❌      |
| TC-M20-12 | Reinstall over existing installation works                 | ❌      |

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