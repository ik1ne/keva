# Keva Windows Implementation Plan

## Architecture

**Full WebView approach** - Native Rust window with single WebView covering entire client area.

```
┌─────────────────────────────────────────────────────┐
│ Native Window (Rust + windows crate)                │
│ ┌─────────────────────────────────────────────────┐ │
│ │ WebView2 (single instance)                      │ │
│ │ ┌───────────────────────────────────────────┐   │ │
│ │ │ [🔍] Search bar        [-webkit-app-region]│   │ │
│ │ ├─────────────┬─────────────────────────────┤   │ │
│ │ │ Key List    │ Preview / Monaco Editor     │   │ │
│ │ │             │                             │   │ │
│ │ │ ─────────── │                             │   │ │
│ │ │ Trash (N)   │                             │   │ │
│ │ └─────────────┴─────────────────────────────┘   │ │
│ └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**Components:**

| Layer | Technology | Responsibility |
|-------|------------|----------------|
| Window | Rust + `windows` crate | Borderless window, tray, resize, DPI |
| WebView | WebView2 via `webview2-com` | All UI rendering |
| UI | HTML/CSS/JS + Monaco | Layout, interactions, text editing |
| Bridge | `postMessage` JSON | Native ↔ WebView communication |
| Core | `keva_core` (Rust) | Data storage, file handling |
| Search | `keva_search` (Rust) | Fuzzy search, indexing |

**Rationale:**

- WebView2 ships with Windows 10/11 (no bundling needed)
- Monaco provides VS Code-quality text editing for free
- HTML/CSS makes UI iteration 3-5x faster than Direct2D
- `keva_core`/`keva_search` stay in Rust (no rewrite)
- Single WebView = simpler than hybrid, faster than Electron

**Project structure:**

```
keva/
├── core/               # keva_core (Rust library)
├── search/             # keva_search (Rust library)
├── keva_windows/       # Windows app
│   ├── src/
│   │   ├── main.rs
│   │   ├── window.rs       # Win32 window, message loop
│   │   ├── webview.rs      # WebView2 setup, message bridge
│   │   ├── bridge.rs       # JSON message protocol
│   │   └── tray.rs         # System tray
│   └── ui/
│       ├── index.html      # Main UI
│       ├── styles.css      # Dark theme styles
│       ├── app.js          # UI logic
│       └── monaco/         # Pre-bundled Monaco editor
├── Spec.md
├── Planned.md
└── implementation_detail.md
```

**Reference documents:**

- `Spec.md` - Product specification (source of truth for behavior)
- `implementation_detail.md` - keva_core API reference
- `Planned.md` - Future features (not in scope)

---

## Message Bridge Protocol

Native and WebView communicate via JSON messages through `postMessage`.

**Native → WebView:**

```typescript
// Key list update
{ type: "keys", keys: [{ name: string, trashed: boolean }] }

// Selected key's value
{ type: "value", key: string, value: { type: "text", content: string } | { type: "files", files: [...] } | null }

// Search results
{ type: "searchResults", keys: [{ name: string, matches: number[], trashed: boolean }] }

// Config
{ type: "config", theme: "dark" | "light", ... }

// Force save (sent before window hide/quit)
{ type: "forceSave" }

// Clipboard content (native reads clipboard, sends to WebView)
{ type: "clipboard", content: { type: "text", text: string } | { type: "files", paths: string[] } | null }
```

**WebView → Native:**

```typescript
// Search query changed
{ type: "search", query: string }

// Key selected
{ type: "select", key: string }

// Value edited (debounced)
{ type: "save", key: string, content: string }

// Key operations
{ type: "create", key: string }
{ type: "rename", oldKey: string, newKey: string }
{ type: "delete", key: string }
{ type: "trash", key: string }
{ type: "restore", key: string }
{ type: "purge", key: string }

// Clipboard
{ type: "copy", key: string }
{ type: "paste", context: "search" | "editor" | "files" }

// Window
{ type: "hide" }
```

---

## Phase 0: Foundation

### M1-win: Window Skeleton

**Goal:** Borderless window with system tray, basic window management.

**Status:** Complete

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Window style | Borderless (WS_POPUP), no title bar | ✅ |
| Resize | 5px outer zone triggers OS resize | ✅ |
| Initial position | Centered on primary monitor | ✅ |
| DPI awareness | Per-monitor DPI aware | ✅ |
| Always on top | WS_EX_TOPMOST | ✅ |
| Tray icon | Visible with tooltip "Keva" | ✅ |
| Tray left-click | Toggle window visibility | ✅ |
| Tray right-click | Context menu | ✅ |
| Esc key | Hides window | ✅ |
| Minimum size | 400x300 enforced | ✅ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M1-01 | Window appears centered on launch | ✅ |
| TC-M1-02 | Drag from outer edge resizes window | ✅ |
| TC-M1-03 | Tray icon visible with correct tooltip | ✅ |
| TC-M1-04 | Tray left-click toggles visibility | ✅ |
| TC-M1-05 | Tray right-click shows menu | ✅ |
| TC-M1-06 | Esc hides window (not destroy) | ✅ |
| TC-M1-07 | Window stays on top | ✅ |
| TC-M1-08 | Text is crisp (DPI correct) | ✅ |

---

### M2-win: WebView + Bridge Foundation

**Goal:** WebView2 covering client area, bidirectional message bridge working.

**Status:** Complete

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| WebView2 init | Create WebView2 environment and controller | ✅ |
| Full coverage | WebView fills entire client area | ✅ |
| Resize sync | WebView resizes with window | ✅ |
| Bridge: N→W | Native sends JSON, WebView receives | ✅ |
| Bridge: W→N | WebView sends JSON, Native receives | ✅ |
| Drag region | Search icon area triggers window drag | ✅ |
| Theme | Dark theme applied to all elements | ✅ |

**Bridge Verification:**

To verify bidirectional communication works:
1. WebView sends `{ type: "ready" }` on load
2. Native logs receipt and responds with `{ type: "init", timestamp: ... }`
3. WebView displays timestamp in console or UI element

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M2-01 | WebView renders HTML content | ✅ |
| TC-M2-02 | Monaco editor loads and accepts input | ✅ |
| TC-M2-03 | Native→WebView message received | ✅ |
| TC-M2-04 | WebView→Native message received | ✅ |
| TC-M2-05 | Dragging search icon moves window | ✅ |
| TC-M2-06 | WebView resizes with window | ✅ |
| TC-M2-07 | Dark theme renders correctly | ✅ |
| TC-M2-08 | Window resize is smooth (no white flash) | ✅ |

---

## Phase 1: Core UI

### M3-win: Key List + Selection + Preview

**Goal:** Initialize keva_core, display keys, select to preview value.

**Status:** Complete

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| keva_core init | Initialize on startup | ✅ |
| Data directory | `%LOCALAPPDATA%\keva` or `KEVA_DATA_DIR` | ✅ |
| Key list | Display all active keys in left pane | ✅ |
| Scrolling | Key list scrolls when needed | ✅ |
| Click to select | Clicking key selects it | ✅ |
| Selection highlight | Selected key visually highlighted | ✅ |
| Preview text | Right pane shows text value (read-only) | ✅ |
| Preview files | Right pane shows "N file(s)" placeholder | ✅ |
| Empty state | Shows "No keys" when database empty | ✅ |
| Touch on select | Call `touch()` when key selected | ✅ |

**UI States:**

| Search Bar | Left Pane | Right Pane |
|------------|-----------|------------|
| Empty | All keys shown | Empty |
| Has text, key exists | Filtered keys | Existing key's value |
| Has text, key doesn't exist | Filtered keys | "Press Enter to create {key}" |
| Key selected | Key highlighted | Selected key's value |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M3-01 | App starts with empty database | ✅ |
| TC-M3-02 | App starts with existing database | ✅ |
| TC-M3-03 | Key list displays all active keys | ✅ |
| TC-M3-04 | Key list scrolls when many keys | ✅ |
| TC-M3-05 | Clicking key selects it | ✅ |
| TC-M3-06 | Selected key's value shown in preview | ✅ |
| TC-M3-07 | Selecting key calls touch() | ✅ |
| TC-M3-08 | Empty database shows empty state | ✅ |

---

### M4-win: Monaco Editor + Auto-Save

**Goal:** Edit text values with Monaco, auto-save after idle.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Monaco integration | Monaco editor in right pane for text values | ❌ |
| Edit trigger | Click in editor or press Enter enables editing | ❌ |
| Auto-save | Save after 3 seconds of no typing | ❌ |
| Save method | Bridge sends `{ type: "save", key, content }` | ❌ |
| Save on hide | Save pending changes when window hides | ❌ |
| Save on switch | Save when selecting different key | ❌ |
| Key creation | Enter in search bar creates key if doesn't exist | ❌ |
| New key in list | Created key appears in left pane | ❌ |

**Force Save Flow (window hide/quit):**

1. User presses Esc or clicks tray Quit
2. Native sends `{ type: "forceSave" }` to WebView
3. WebView checks if editor has unsaved changes (dirty flag)
4. If dirty, WebView sends `{ type: "save", key, content }` to Native
5. Native waits for save acknowledgment before hiding/quitting

**Button in Search Bar:**

| State | Button | Action |
|-------|--------|--------|
| Key exists | ✏️ Pen | Focus editor |
| Key doesn't exist | ➕ Plus | Create key, focus editor |
| Empty / Key selected | Hidden | - |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M4-01 | Monaco editor renders for text value | ❌ |
| TC-M4-02 | Typing in editor modifies content | ❌ |
| TC-M4-03 | Auto-save triggers after 3s idle | ❌ |
| TC-M4-04 | Saved content persists after restart | ❌ |
| TC-M4-05 | Esc saves pending changes before hide | ❌ |
| TC-M4-06 | Switching key saves previous changes | ❌ |
| TC-M4-07 | Enter creates new key when doesn't exist | ❌ |
| TC-M4-08 | Plus button creates key | ❌ |
| TC-M4-09 | Pen button focuses editor | ❌ |

---

### M5-win: Search Integration

**Goal:** Connect search bar to keva_search, filter and highlight results.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| SearchEngine init | Initialize with keys from keva_core | ❌ |
| Query input | Every keystroke sends search query to native | ❌ |
| Async results | Native returns results via bridge | ❌ |
| Key filtering | Left pane shows only matching keys | ❌ |
| Match highlighting | Matched characters highlighted in key names | ❌ |
| Empty query | Shows all keys (active first, then trashed) | ❌ |
| Preserve on hide | Search text preserved, restored with selection | ❌ |

**Index Maintenance:**

| Event | SearchEngine Call |
|-------|-------------------|
| App startup | `new(active_keys, trashed_keys, ...)` |
| Key created | `add_active(key)` |
| Key deleted (soft) | `trash(key)` |
| Key deleted (permanent) | `remove(key)` |
| Key restored | `restore(key)` |
| Key renamed | `rename(old, new)` |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M5-01 | Typing filters key list | ❌ |
| TC-M5-02 | Matched characters highlighted | ❌ |
| TC-M5-03 | Empty search shows all keys | ❌ |
| TC-M5-04 | Window hide preserves search text | ❌ |
| TC-M5-05 | Window show restores text selected | ❌ |
| TC-M5-06 | Created key appears in results | ❌ |
| TC-M5-07 | Smart case: lowercase matches any case | ❌ |
| TC-M5-08 | Smart case: uppercase matches exact case | ❌ |

---

## Phase 2: Operations

### M6-win: Keyboard Navigation

**Goal:** Arrow keys, Enter, Delete, Escape, Ctrl+Alt+C for keyboard workflow.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Down arrow (search) | Move focus to first key | ❌ |
| Up arrow (search) | No action | ❌ |
| Down/Up (key list) | Navigate keys | ❌ |
| Up from first key | Return to search bar | ❌ |
| Enter (key selected) | Focus editor | ❌ |
| Delete (key selected) | Delete key (follows delete_style) | ❌ |
| Ctrl+Alt+C | Copy value to clipboard, hide window | ❌ |
| Escape | Hide window (always) | ❌ |

**Ctrl+Alt+C Behavior:**

| Value Type | Clipboard Content |
|------------|-------------------|
| Text | Plain text |
| Files | File paths (platform format) |
| Empty | Empty string |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M6-01 | Down arrow from search selects first key | ❌ |
| TC-M6-02 | Arrow keys navigate key list | ❌ |
| TC-M6-03 | Up from first key returns to search | ❌ |
| TC-M6-04 | Enter on key focuses editor | ❌ |
| TC-M6-05 | Delete key deletes selected key | ❌ |
| TC-M6-06 | Ctrl+Alt+C copies and hides | ❌ |
| TC-M6-07 | Escape hides window | ❌ |

---

### M7-win: Clipboard Paste

**Goal:** Ctrl+V with context-aware behavior.

**Status:** Not Started

**Interception Architecture:**

Native intercepts Ctrl+V first (not WebView). Rationale:
- WebView has limited clipboard access for files (security sandbox)
- Native can read both text and file paths from Windows clipboard
- Native sends `{ type: "clipboard", content }` to WebView
- WebView decides action based on current focus context

Flow:
1. User presses Ctrl+V
2. Native intercepts via accelerator or message hook
3. Native reads clipboard (text or file paths)
4. Native sends clipboard content to WebView
5. WebView applies paste based on focus (search/editor/files)

**Paste Behavior:**

| Focus | Clipboard | Action |
|-------|-----------|--------|
| Search bar | Text | Insert into search |
| Search bar | Files | Create/update key value |
| Editor | Text | Insert at cursor |
| Editor | Files | Warning, Ctrl+V again to overwrite |
| Files display | Text | Warning, Ctrl+V again to overwrite |
| Files display | Files | Silent append |

**Overwrite Confirmation:**

| Element | Description |
|---------|-------------|
| Warning | Red text in right pane |
| Timeout | 2 seconds |
| Second Ctrl+V | Execute overwrite |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M7-01 | Paste text into search inserts text | ❌ |
| TC-M7-02 | Paste files with search focused creates key | ❌ |
| TC-M7-03 | Paste text into editor inserts at cursor | ❌ |
| TC-M7-04 | Paste files into editor shows warning | ❌ |
| TC-M7-05 | Second Ctrl+V within 2s overwrites | ❌ |
| TC-M7-06 | Paste files into files appends | ❌ |

---

### M8-win: Rename + Delete

**Goal:** Inline rename and delete with trash support.

**Status:** Not Started

**Rename:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Rename button | Pen icon on key hover | ❌ |
| Inline editor | Click pen → editable text field | ❌ |
| Initial selection | All text selected | ❌ |
| Confirm | Enter or click outside | ❌ |
| Cancel | Escape (does NOT hide window) | ❌ |
| Overwrite prompt | If target exists, show confirmation | ❌ |

**Delete:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Delete button | Trash icon on key hover | ❌ |
| Delete style | Follows config (soft or immediate) | ❌ |
| Soft delete | Moves to trash | ❌ |
| Immediate delete | Permanently removes | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M8-01 | Pen icon visible on hover | ❌ |
| TC-M8-02 | Click pen opens inline editor | ❌ |
| TC-M8-03 | Enter confirms rename | ❌ |
| TC-M8-04 | Escape cancels rename | ❌ |
| TC-M8-05 | Rename to existing shows confirmation | ❌ |
| TC-M8-06 | Trash icon visible on hover | ❌ |
| TC-M8-07 | Click trash with soft delete trashes key | ❌ |
| TC-M8-08 | Click trash with immediate delete purges key | ❌ |

---

### M9-win: Trash UI

**Goal:** Display trashed keys, enable restore and permanent delete.

**Status:** Not Started

**Layout:**

```
┌─────────────────┐
│ Active keys     │
│ (scrollable)    │
├─────────────────┤
│ Trash (N)       │
│ (trashed keys)  │
└─────────────────┘
```

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Trash section | Fixed height at bottom | ❌ |
| Trash header | "Trash (N)" with count | ❌ |
| Visibility | Hidden when no trash matches | ❌ |
| Trash indicator | 🗑️ icon prefix | ❌ |
| Selection | Click to select, shows value (read-only) | ❌ |
| Restore button | Visible for trashed key | ❌ |
| Permanent delete | Visible for trashed key | ❌ |
| Separate nav | Click required to enter trash from active | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M9-01 | Trash section appears when trash exists | ❌ |
| TC-M9-02 | Trash section hidden when empty | ❌ |
| TC-M9-03 | Trash header shows count | ❌ |
| TC-M9-04 | Click trashed key shows value | ❌ |
| TC-M9-05 | Restore moves key to active | ❌ |
| TC-M9-06 | Permanent delete removes key | ❌ |
| TC-M9-07 | Trashed key value is read-only | ❌ |

---

## Phase 3: Files

### M10-win: File Value Display

**Goal:** Display files list with names, sizes, delete buttons.

**Status:** Not Started

**Layout:**

```
┌─────────────────────────────────┐
│ 📄 document.pdf    1.2 MB   [X] │
│ 📄 image.png       340 KB   [X] │
│                                 │
│            [Clear All]          │
└─────────────────────────────────┘
```

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| File list | Each file as row with name and size | ❌ |
| Size format | Human-readable (e.g., "1.2 MB") | ❌ |
| Scrollable | Scrolls if many files | ❌ |
| Delete individual | X button on each row | ❌ |
| Clear all | Button to remove all files | ❌ |
| Empty after delete | Last file deleted → empty Text value | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M10-01 | Files value displays file list | ❌ |
| TC-M10-02 | Each file shows name and size | ❌ |
| TC-M10-03 | X button removes individual file | ❌ |
| TC-M10-04 | Clear All removes all files | ❌ |
| TC-M10-05 | Deleting last file → empty value | ❌ |

---

### M11-win: Drag & Drop

**Goal:** Drop files onto left or right pane to store.

**Status:** Not Started

**Drop Behavior:**

| Existing Value | Drop Content | Behavior |
|----------------|--------------|----------|
| Empty | Files | Accept |
| Empty | Text | Accept |
| Text | Files | Confirm: "Replace text with files?" |
| Files | Files | Silent append |
| Files | Text | Confirm: "Replace files with text?" |

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Drop: right pane | Stores to current target key | ❌ |
| Drop: key in list | Stores to that key | ❌ |
| Drop: search bar | Not a drop target | ❌ |
| Drop: trashed key | Rejected | ❌ |
| Visual feedback | Highlight drop target | ❌ |
| File size limit | >1GB rejected, >threshold confirms | ❌ |
| Duplicate handling | Same hash silently ignored | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M11-01 | Drop files on right pane stores | ❌ |
| TC-M11-02 | Drop files on key stores to that key | ❌ |
| TC-M11-03 | Drop files on Files value appends | ❌ |
| TC-M11-04 | Drop files on Text value confirms | ❌ |
| TC-M11-05 | Drop target highlights during drag | ❌ |
| TC-M11-06 | Drop on trashed key rejected | ❌ |

---

## Phase 4: Settings & Polish

### M12-win: Settings Dialog

**Goal:** Settings UI with config persistence.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Open | Ctrl+, or tray menu | ❌ |
| Modal | Blocks main window | ❌ |
| Save on close | Writes to config.toml | ❌ |
| Apply immediately | No restart needed | ❌ |

**Settings:**

| Category | Setting | Control | Values |
|----------|---------|---------|--------|
| General | Theme | Dropdown | Dark / Light / System |
| General | Launch at Login | Checkbox | On / Off |
| General | Show Tray Icon | Checkbox | On / Off |
| Shortcuts | Global Shortcut | Key capture | Modifier+Key |
| Data | Delete Style | Dropdown | Soft / Immediate |
| Data | Large File Threshold | Number | Bytes (default 256MB) |
| Lifecycle | Trash TTL | Number | Days (default 30) |
| Lifecycle | Purge TTL | Number | Days (default 7) |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M12-01 | Ctrl+, opens settings | ❌ |
| TC-M12-02 | Theme change applies immediately | ❌ |
| TC-M12-03 | Settings persist after restart | ❌ |
| TC-M12-04 | Esc closes settings dialog | ❌ |
| TC-M12-05 | Show Tray Icon toggle hides/shows tray icon | ❌ |
| TC-M12-06 | Purge TTL change affects trash cleanup timing | ❌ |

---

### M13-win: Global Hotkey

**Goal:** System-wide shortcut to show window.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Default | Ctrl+Alt+K | ❌ |
| Global scope | Works when window hidden | ❌ |
| Registration | On app startup | ❌ |
| Conflict detection | Show notification if in use | ❌ |
| Config sync | Updates when changed in settings | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M13-01 | Hotkey shows window when hidden | ❌ |
| TC-M13-02 | Hotkey works from other apps | ❌ |
| TC-M13-03 | Conflict shows notification | ❌ |
| TC-M13-04 | Changed hotkey works after restart | ❌ |

---

### M14-win: Single Instance

**Goal:** Prevent multiple instances, activate existing on relaunch.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Detection | Named mutex on startup | ❌ |
| Existing found | Activate existing window, exit new | ❌ |
| Timeout | 2s unresponsive → force-quit dialog | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M14-01 | First instance starts normally | ❌ |
| TC-M14-02 | Second instance activates first | ❌ |
| TC-M14-03 | Second instance exits | ❌ |
| TC-M14-04 | Unresponsive triggers force-quit dialog | ❌ |

---

### M15-win: Window Position Memory

**Goal:** Remember position and size per monitor.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Save position | On hide/quit | ❌ |
| Restore position | On next show | ❌ |
| Per-monitor | Keyed by monitor ID | ❌ |
| Off-screen check | Center if restored position invalid | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M15-01 | Position persists after hide/show | ❌ |
| TC-M15-02 | Position persists after restart | ❌ |
| TC-M15-03 | Different monitors remember different positions | ❌ |
| TC-M15-04 | Off-screen position corrected | ❌ |

---

### M16-win: First-Run Dialog

**Goal:** Welcome dialog on first launch.

**Status:** Not Started

**Content:**

| Element | Description |
|---------|-------------|
| Title | "Welcome to Keva" |
| Message | "Press Ctrl+Alt+K anytime to open." |
| Checkbox | "Launch at login" (checked by default) |
| Button | "Get Started" |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M16-01 | Dialog shown on first launch | ❌ |
| TC-M16-02 | Checkbox checked by default | ❌ |
| TC-M16-03 | "Get Started" creates config | ❌ |
| TC-M16-04 | Dialog not shown on subsequent launches | ❌ |

---

## Phase 5: Distribution

### M17-win: Monaco Bundling + Build

**Goal:** Bundle Monaco locally, optimize build.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Monaco bundle | Copy from npm to ui/monaco/ | ❌ |
| No CDN | All resources load locally | ❌ |
| Build script | Automate bundling in cargo build | ❌ |
| Embed HTML | Embed ui/ files in binary | ❌ |
| Single exe | No external files needed | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M17-01 | App works offline | ❌ |
| TC-M17-02 | No network requests during load | ❌ |
| TC-M17-03 | Single exe runs without ui/ folder | ❌ |

---

### M18-win: Installer & Distribution

**Goal:** Installable package with uninstaller.

**Status:** Not Started

**Requirements:**

| Requirement | Description | Status |
|-------------|-------------|--------|
| Installer format | WiX or MSIX | ❌ |
| Install location | Program Files | ❌ |
| Start Menu | Create shortcut | ❌ |
| Add/Remove Programs | Registry entry | ❌ |
| Launch at login | Registry Run key | ❌ |
| Uninstaller | Prompt for data deletion | ❌ |

**Test Cases:**

| TC | Description | Status |
|----|-------------|--------|
| TC-M18-01 | Installer completes on clean system | ❌ |
| TC-M18-02 | App launches from Start Menu | ❌ |
| TC-M18-03 | Uninstaller removes app | ❌ |
| TC-M18-04 | Data deletion prompt works | ❌ |
| TC-M18-05 | Launch at login works after reboot | ❌ |

---

## Implementation Notes

### Window Drag via CSS

```css
.search-icon {
    -webkit-app-region: drag;
    cursor: grab;
}

.search-input {
    -webkit-app-region: no-drag;
}
```

### Monaco Local Loading

```javascript
require.config({
    paths: { vs: './monaco/vs' }
});

require(['vs/editor/editor.main'], function() {
    editor = monaco.editor.create(container, {
        theme: 'vs-dark',
        automaticLayout: true,
        minimap: { enabled: false },
        wordWrap: 'on'
    });
});
```

### Message Bridge Pattern

```rust
// Native side
fn handle_webview_message(json: &str) {
    let msg: Message = serde_json::from_str(json)?;
    match msg {
        Message::Search { query } => { /* ... */ }
        Message::Save { key, content } => { /* ... */ }
        // ...
    }
}

fn send_to_webview(webview: &WebView, msg: &Message) {
    let json = serde_json::to_string(msg)?;
    webview.post_message(&json);
}
```

```javascript
// WebView side
window.chrome.webview.addEventListener('message', event => {
    const msg = event.data;
    switch (msg.type) {
        case 'keys': updateKeyList(msg.keys); break;
        case 'value': updatePreview(msg.value); break;
        // ...
    }
});

function sendToNative(msg) {
    window.chrome.webview.postMessage(msg);
}
```
