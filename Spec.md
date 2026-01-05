# Project Specification: Keva (v2)

## 1. Overview

Keva is a local key-value store designed for knowledge management with Markdown support and file attachments. It
provides fast storage and retrieval with fuzzy search capabilities.

Inspired by Unclutter's notes+files workflow, but with multiple named keys and search.

- **Name:** Keva
- **Platform:** Windows (installer with uninstaller)

## 2. Core Concepts

### Keys

Keys are flat strings with the following constraints:

- Valid UTF-8
- Length: 1–256 characters
- Enforced by `keva_core::Key` struct (Nutype)

The `/` character has no special meaning to the storage layer. The GUI may visually group keys sharing common prefixes,
but this is cosmetic only with no behavioral implications.

Examples of valid keys:

- `project/config/theme`
- `my-notes`
- `2024/01/15`

### Value Types

Each key stores a unified value containing:

1. **Content:** Markdown text (always exists, may be empty)
2. **Attachments:** Zero or more files (hard copy of file contents)

### Attachment Link Format

Attachments are referenced in Markdown using filename-based links:

```markdown
[spec.pdf](att:spec.pdf)
[image.png](att:image.png)
```

- Format: `[display text](att:filename)`
- Filename must be unique per key (duplicates not allowed)
- Links are human-readable and manually typeable

### Empty Values

- Empty Markdown content keeps the key with an empty file (key is not deleted).
- Deleting all attachments keeps the key with Markdown only.
- Attachments without Markdown references are allowed (file-only storage is valid use case).

### Process Behavior

- Starts as background process (no window on launch)
- Taskbar icon visible (hiding from taskbar also hides from Alt+Tab)
- System tray icon visible by default
- Window hidden keeps process alive in background

### Launch and Activation

- Global shortcut `Ctrl+Alt+K` shows window
- Launch at login: user opts in via first-run dialog (see Section 4)
    - Registry `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- Must appear correctly in Task Manager Startup tab

### Single Instance

- Only one instance runs at a time
- Relaunch app (double-click) → activates existing instance's window

### Windows Uninstaller

- Remove startup registry entry (`HKCU\...\Run`)
- Remove application files
- Prompt: "Delete all Keva data?" (config, database, blobs)
    - Yes: Remove data directory
    - No: Leave data directory intact

## 4. GUI

### Custom Window

- Custom-drawn window (no native decorations)
- No title bar, close/minimize/maximize buttons

### Window Behavior

**Window Controls:**

- `Ctrl+Alt+K` → Show window (global shortcut, works when hidden)
- `Esc` → Dismiss modal if open, else hide window
- `Alt+F4` → Quit app entirely (only when window is focused)
- Window does NOT close on focus loss (supports drag/drop and copy/paste workflows)
- Window stays on top of other windows (enables drag/drop from other apps)
- Tray icon left-click also toggles visibility

**Keyboard Handling:**

- Global hotkey (Ctrl+Alt+K): Native intercepts
- All other keys: WebView handles
- Esc: WebView decides (dismiss modal or send "hide")
- Alt+F4: WebView sends "quit"

**Resize and Move:**

- **Outer border:** Resize handle (triggers OS-level resize), should behave like native window border
- **Search icon (🔍):** Drag handle for moving window (click does nothing, drag moves window)

**Window Positioning:**

- First launch: Center of primary monitor
- Subsequent launches: Remember last position and size per monitor
    - Position stored keyed by monitor identifier
    - If monitor configuration changes, center on current monitor
- Multi-monitor: If remembered position is off-screen, center on monitor containing cursor

**Window Show State:**

- Search text preserved from previous session
- Text is selected (as if Ctrl+A pressed)
- User can type to replace or use arrow keys to preserve existing text

### Tray Icon Behavior

**Tooltip:** "Keva"

**Left-click:** Toggle window visibility (show if hidden, hide if shown)

**Right-click menu:**

| Item            | Action                                    |
|-----------------|-------------------------------------------|
| Show Keva       | Show window (disabled if already visible) |
| Settings...     | Open settings dialog                      |
| ---             | Separator                                 |
| Launch at Login | Checkbox toggle (synced with settings)    |
| ---             | Separator                                 |
| Quit Keva       | Terminate application                     |

### Layout

Four-pane layout:

```
┌─────────────────────────────────────────────────────────────┐
│ [🔍] Search bar                                    [✏️/➕]  │
├──────────────┬──────────────────────────────────────────────┤
│              │ Right Top: [Edit] [Preview]                  │
│  Key List    │ ┌──────────────────────────────────────────┐ │
│              │ │ # Notes                                  │ │
│  key-1       │ │ See [spec.pdf](att:spec.pdf)             │ │
│  key-2       │ │                                          │ │
│  key-3       │ └──────────────────────────────────────────┘ │
│              ├──────────────────────────────────────────────┤
│  ─────────── │ Right Bottom: Attachments                    │
│  Trash (N)   │ ┌──────────────────────────────────────────┐ │
│              │ │ 📄 spec.pdf (1.2MB)                  [X] │ │
│              │ │ [img] photo.png (340KB)              [X] │ │
│              │ │            [+ Add files]                 │ │
│              │ └──────────────────────────────────────────┘ │
└──────────────┴──────────────────────────────────────────────┘
```

Components:

- **Search bar:** Key filter, fuzzy matching, action button
- **Left pane:** Key list (filtered), trash section at bottom
- **Right top pane:** Markdown editor (Monaco) with Edit/Preview toggle
- **Right bottom pane:** Attachments list with delete buttons

### Four-State Focus Model

All four panes have mutually exclusive active state. Only one can be active at a time.

**Active States:**

| Active Pane  | Search Bar | Left Pane   | Right Top | Right Bottom |
|--------------|------------|-------------|-----------|--------------|
| Search bar   | Cursor     | Dimmed      | No cursor | Dimmed       |
| Left pane    | No cursor  | Highlighted | No cursor | Dimmed       |
| Right top    | No cursor  | Dimmed      | Cursor    | Dimmed       |
| Right bottom | No cursor  | Dimmed      | No cursor | Highlighted  |

**Visual Indicators:**

| Pane         | Active                | Inactive               |
|--------------|-----------------------|------------------------|
| Search bar   | Text cursor, normal   | No cursor, dimmed text |
| Left pane    | Key fully highlighted | Key dimmed highlight   |
| Right top    | Text cursor visible   | No cursor, dimmed      |
| Right bottom | Selection highlighted | Selection dimmed       |

**Selection Persistence:**

- Left pane: Selection persists when inactive (determines target key)
- Right bottom: File selection persists when inactive (shown dimmed)

### Target Key

Target key determines which key's content is shown in right panes.

```
Target Key =
    if (left pane has selection) → selected key
    else if (search bar has exact match) → matched key
    else → none
```

Left pane selection takes priority.

### Search Bar Behavior

**Components:**

```
[🔍] [__search text__] [✏️/➕]
```

**Behavior:**

- Each keystroke triggers search, results update progressively
- **Typing clears left pane selection** (no stale state)
- Empty search bar shows all keys
- Plus button hidden when exact match exists

**Enter Key:**

| State              | Action                                           |
|--------------------|--------------------------------------------------|
| Exact match exists | Select key (dimmed), cursor to right top pane    |
| No exact match     | Create key, select (dimmed), cursor to right top |

### Focus Transitions

| From         | Action                    | To                                     |
|--------------|---------------------------|----------------------------------------|
| Search bar   | Down arrow                | Left pane (first key selected)         |
| Search bar   | Click key in list         | Left pane                              |
| Search bar   | Enter                     | Right top (select/create key)          |
| Search bar   | Click right top           | Right top                              |
| Search bar   | Click right bottom        | Right bottom                           |
| Left pane    | Up arrow (from first key) | Search bar                             |
| Left pane    | Down/Up arrow             | Navigate within list                   |
| Left pane    | Enter                     | Right top pane                         |
| Left pane    | Delete key                | Delete selected key                    |
| Left pane    | Click search bar          | Search bar                             |
| Left pane    | Click right top           | Right top                              |
| Left pane    | Click right bottom        | Right bottom                           |
| Right top    | Esc                       | Hide window (or dismiss modal if open) |
| Right top    | Click search bar          | Search bar                             |
| Right top    | Click key in list         | Left pane                              |
| Right top    | Click right bottom        | Right bottom                           |
| Right bottom | Esc                       | Hide window (or dismiss modal if open) |
| Right bottom | Click search bar          | Search bar                             |
| Right bottom | Click key in list         | Left pane                              |
| Right bottom | Click right top           | Right top                              |

### Keyboard Shortcuts

**Global:**

| Key          | Action                          |
|--------------|---------------------------------|
| `Ctrl+Alt+K` | Show window (works when hidden) |

**Window Focused:**

| Key      | Action                                  |
|----------|-----------------------------------------|
| `Esc`    | Dismiss modal if open, else hide window |
| `Alt+F4` | Quit app entirely                       |
| `Ctrl+S` | Focus search bar                        |
| `Ctrl+,` | Open settings dialog                    |

**Copy Shortcuts:**

| Key          | Action                             | On Success  |
|--------------|------------------------------------|-------------|
| `Ctrl+C`     | Copy selection (context-dependent) | Stay open   |
| `Ctrl+Alt+T` | Copy whole markdown as plain text  | Hide window |
| `Ctrl+Alt+R` | Copy rendered preview as HTML      | Hide window |
| `Ctrl+Alt+F` | Copy all attachments to clipboard  | Hide window |

**Copy Shortcut Logic (Ctrl+Alt+T/R/F):**

```
if (left pane has selection):
    copy from right pane content
    hide window
else if (search bar has exact match):
    load key content
    copy
    hide window
else:
    show popup "Nothing to copy"
    stay open
```

**Context-Dependent Ctrl+C:**

| Active Pane  | Selection        | Action                  |
|--------------|------------------|-------------------------|
| Search bar   | Text selected    | Copy as plain text      |
| Right top    | Text selected    | Copy as plain text      |
| Right bottom | File(s) selected | Copy files to clipboard |

**Navigation:**

| State                        | Key      | Action                         |
|------------------------------|----------|--------------------------------|
| Left pane focused            | `Enter`  | Focus right top pane           |
| Left pane focused            | `Delete` | Delete selected key            |
| Search bar focused, has text | `Enter`  | Select/create key, focus right |

### Right Top Pane (Markdown Editor)

**Edit/Preview Toggle:**

```
┌─────────────────────────────────────────────┐
│ [Edit] [Preview]                            │
├─────────────────────────────────────────────┤
│  (Monaco or rendered HTML based on tab)     │
└─────────────────────────────────────────────┘
```

| Mode    | Content                      | Editable |
|---------|------------------------------|----------|
| Edit    | Monaco editor (raw markdown) | Yes      |
| Preview | Rendered HTML, images inline | No       |

**Preview Rendering:**

- `att:filename` links transformed to blob paths for image display
- Clickable links for non-image attachments

**Monaco Configuration:**

- Language mode: Markdown
- `pasteAs: { enabled: false }` (enables paste interception)
- `dragAndDrop: true` (internal text drag)
- `dropIntoEditor: { enabled: true }` (external drops)
- `placeholder: "Type something, or drag files here..."` (shown when empty)

**Auto-Save:**

- Monaco writes directly to blob file via FileSystemHandle
- Timestamps updated on key switch or window hide

### Right Bottom Pane (Attachments)

**Visibility:**

- **No key selected:** Hidden (pane not visible)
- **Key selected:** Visible with slide-up animation
- **Trashed key selected:** Visible but read-only (no add/delete/rename, user can view attached files)

**Layout:**

```
┌─────────────────────────────────────────────┐
│ 📄 document.pdf    1.2 MB         [✏️] [X]  │
│ [img] image.png    340 KB         [✏️] [X]  │  ← thumbnail for images
│                                             │
│              [+ Add files]                  │
└─────────────────────────────────────────────┘
```

**Empty State:**

When no attachments exist, show centered placeholder text:
```
┌─────────────────────────────────────────────┐
│                                             │
│      Drop files here or onto the editor     │
│                                             │
└─────────────────────────────────────────────┘
```

**Drop Zone Overlay:**

When dragging files over the attachments pane:
- Overlay appears on top of existing file list
- Shows drop hint text with highlighted border
- Indicates valid drop target

```
┌─────────────────────────────────────────────┐
│ ┌─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐ │
│ │                                       │ │
│ │   Drop files here or onto the editor  │ │  ← highlighted overlay
│ │                                       │ │
│ └─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘ │
└─────────────────────────────────────────────┘
```

**Features:**

- File list with name, size, type icon
- Thumbnail preview for images (png, jpg, jpeg, gif, webp, svg)
- Multi-select with Shift/Ctrl click
- [X] button: Remove attachment (shows confirmation dialog)
- [+ Add files]: File picker or drop zone
- Drag file to Monaco: Insert link at drop position
- [✏️] button: Inline rename attachment
    - Opens inline text input with current filename
    - If new name exists: show duplicate dialog (Overwrite/Rename/Cancel)
    - If referenced in markdown: show update dialog (Update/Don't Update/Cancel)
        - Update: rename file and replace all `att:oldname` with `att:newname` in editor
        - Don't Update: rename file only (references become broken)
        - Cancel: abort rename
    - Invalid names (empty, too long) rejected with inline error

**Thumbnail Generation:**

- Generated on import
- Lazy generation for app upgrades (new format support)
- Fallback to icon while generating

**Delete Attachment Confirmation:**

```
┌─────────────────────────────────────────────┐
│ Delete "file.pdf"?                          │
│                                             │
│ [Delete]  [Cancel]                          │
└─────────────────────────────────────────────┘
```

### Left Pane Controls

Each active key displays on hover/selection:

- **Rename button (pen icon):** Opens inline editor to modify key.
    - If rename target exists: confirmation prompt, target key is permanently overwritten (no restoration)
    - Invalid key names (empty, >256 chars) rejected with inline error message
    - Renamed key maintains its position in the list (no re-search); removed only when search query changes
- **Delete button (trash icon):** Deletes the key

**Trashed Key Controls:**

- **Restore button:** Restores key to active state
- **Permanent delete button:** Permanently removes key and value

### Trash Section

The left pane has a separate trash section at the bottom:

- **Fixed height:** Approximately 2-2.5x the height of a single key row
- **Visibility:** Hidden when no trashed keys match the current search
- **Header:** "Trash (N)" showing count of matching trashed keys
- **Separate navigation:** Click required to enter trash section from active keys
- **Arrow navigation:** Up/Down arrows navigate within trash section
- **Boundaries:** Up arrow from first trash key stays in trash; down arrow from last trash key stays in trash
- **Exit:** Click on active key or search bar to exit trash section

### Search Behavior

- **Mode:** Fuzzy matching only (via `keva_search` crate using nucleo)
- **Ranking:** Determined by nucleo algorithm; active keys always before trashed
- **Case Sensitivity:** Smart case (case-insensitive unless query contains uppercase)
- **Trash Handling:** Trash items shown in separate section at bottom with 🗑️ icon
- **Stale Items:** Items past TTL remain visible until GC runs (GC is the single source of truth for state transitions)

**Search Result Limits:**

- Active keys: Maximum 100 results displayed
- Trashed keys: Maximum 20 results displayed

### Clipboard Handling

**Paste Behavior:**

| Active Pane  | Clipboard | Action                                      |
|--------------|-----------|---------------------------------------------|
| Search bar   | Text      | Insert into search bar                      |
| Search bar   | Files     | Do nothing; User must press enter first     |     
| Right top    | Text      | Insert at cursor (Monaco)                   |
| Right top    | Files     | Add to attachments + insert links at cursor |
| Right bottom | Text      | Show confirmation dialog                    |
| Right bottom | Files     | Add to attachments                          |

**Link Insertion Format (multiple files):**

```markdown
[report.pdf](att:report.pdf), [data.xlsx](att:data.xlsx), [image.png](att:image.png)
```

Comma-separated inline. User can reformat as needed.

**Overwrite Confirmation:**

- Modal dialog with message and two buttons
- [Yes] button focused by default (Enter confirms)
- Keyboard shortcuts: Alt+Y (Yes), Alt+N (No)
- Escape key dismisses (same as No)

### Duplicate File Handling

Duplicate filenames are not allowed within a key.

**Single File Drop/Paste:**

```
┌─────────────────────────────────────────────┐
│ "report.pdf" already exists.                │
│                                             │
│ [Overwrite]  [Rename]  [Cancel]             │
└─────────────────────────────────────────────┘
```

- **Overwrite:** Replace existing file
- **Rename:** Auto-generate "report (1).pdf"
- **Cancel:** Skip this file

**Multi-File Paste (with duplicates):**

```
┌─────────────────────────────────────────────┐
│ "report.pdf" already exists.                │
│                                             │
│ ☐ Apply to all (3 remaining)                │
│                                             │
│ [Overwrite]  [Rename]  [Skip]  [Cancel All] │
└─────────────────────────────────────────────┘
```

- **Overwrite:** Replace this one
- **Rename:** Auto-rename this one
- **Skip:** Skip this one, continue with others
- **Cancel All:** Abort entire paste
- **Apply to all:** Selected action applies to remaining conflicts

**Apply to All Behavior:**

| Checkbox | Button     | Effect                   |
|----------|------------|--------------------------|
| ☐        | Overwrite  | Overwrite this, ask next |
| ☐        | Rename     | Rename this, ask next    |
| ☐        | Skip       | Skip this, ask next      |
| ☐        | Cancel All | Abort, nothing pasted    |
| ☑        | Overwrite  | Overwrite all conflicts  |
| ☑        | Rename     | Rename all conflicts     |
| ☑        | Skip       | Skip all conflicts       |

### Drag & Drop

**Drop Targets:**

| Target             | Text                    | Files                             |
|--------------------|-------------------------|-----------------------------------|
| Right top (Monaco) | Insert at drop position | Add to attachments + insert links |
| Right bottom       | No action               | Add to attachments                |
| Key in left pane   | N/A                     | Add to that key's attachments     |
| Trashed key        | Rejected                | Rejected                          |
| Search bar         | Not a drop target       | Not a drop target                 |

**Drag from Attachments Panel:**

- Drag file to Monaco → Insert `[filename](att:filename)` at drop position

**Monaco Text Drop:**

- Internal drag: Monaco built-in (`dragAndDrop: true`)
- External text drop: DOM drop event → Monaco executeEdits

**Large File Handling:**

- Threshold applies **per file**, not total
- Multiple files: each checked individually against threshold

### First-Run Experience

On first launch (no config.toml exists):

1. Show welcome dialog:
    - Title: "Welcome to Keva"
    - Message: "Keva stores your notes and files locally. Press Ctrl+Alt+K anytime to open this window."
    - Checkbox: "Launch Keva at login" (checked by default)
    - Button: "Get Started"
2. If checkbox is checked, register login item
3. Create config.toml with user preferences
4. Show main window

### Settings Dialog

- Opened via `Ctrl+,` or tray icon menu
- Changes saved to config file on dialog close
- Applied immediately to running application
- Global shortcut configuration uses key capture dialog
- Close via X button or Esc key

**Settings Categories:**

| Category  | Setting         | Description                           |
|-----------|-----------------|---------------------------------------|
| General   | Theme           | Dark / Light / Follow System          |
| General   | Launch at Login | Toggle auto-start                     |
| General   | Show Tray Icon  | Toggle tray icon visibility           |
| Shortcuts | Global Shortcut | Key combination to show window        |
| Lifecycle | Trash TTL       | Days before items auto-trash          |
| Lifecycle | Purge TTL       | Days before trashed items are deleted |

## 5. Configuration

### Data Directory

Default location: `%LOCALAPPDATA%\keva`

Override via environment variable: `KEVA_DATA_DIR`

### Config Validation

On launch, if config.toml contains invalid values:

1. Popup displays specific validation errors
2. User chooses: **[Launch with defaults]** or **[Quit]**
3. "Launch with defaults" overwrites invalid fields and proceeds
4. "Quit" exits without modifying config file

If config.toml is missing: created with defaults, no popup.

## 6. Lifecycle Management

### Timestamps

Each key stores:

- **last_accessed:** When the key was last viewed, copied to clipboard, or value was modified (Active state).
- **trashed_at:** When the key was moved to Trash (Trash state).

### TTL Calculation

TTL expiration is based on `last_accessed`. Operations that update `last_accessed`:

- Selecting key in left pane (viewing in right pane)
- Copying value to clipboard
- Modifying the value (keva_core handles this internally)

### Lifecycle Stages

1. **Active:** Normal visibility. Transitions to Trash when `last_accessed + trash_ttl` expires.

2. **Trash:** Soft-deleted, shown in trash section.
    - Searchable (shown in trash section with 🗑️ icon).
    - Read-only (must restore to edit).
    - Transitions to Purge when `trashed_at + purge_ttl` expires.

3. **Purge:** Considered permanently deleted.
    - Hidden from all interfaces immediately upon TTL expiration.
    - Physical data removed at next GC cycle.

**Note:** Trash and purge exist for unaccessed key cleanup, not for accidental deletion prevention. Rename overwrites
are permanent with no restoration.

### Maintenance (Garbage Collection)

- Moves items from Active to Trash based on TTL
- Permanently removes items past purge TTL
- Reclaims storage space from deleted blobs
- May perform in-memory maintenance tasks (e.g., search index compaction)

**Triggers:**

- Window hide
- Periodically while running (fixed: 1 day)

**NOT triggered on:**

- App quit (for fast exit)

## 7. Error Handling

### Global Shortcut Conflicts

If the configured shortcut is already registered by another application:

1. Show notification: "Shortcut Ctrl+Alt+K is in use by another application"
2. Open settings dialog with shortcut field focused
3. User must choose a different shortcut or resolve the conflict externally
4. Alternative: User can double-click .exe to show window if hotkey unavailable

### Database Errors

| Error              | User Message                                       | Recovery Action                    |
|--------------------|----------------------------------------------------|------------------------------------|
| Database corrupted | "Database is corrupted. Create new database?"      | Backup old, create fresh           |
| Disk full          | "Disk is full. Cannot save changes."               | Retry after user frees space       |
| File locked        | "Database is locked by another process."           | Offer to force-quit other instance |
| Permission denied  | "Cannot access data directory. Check permissions." | Show path, suggest fix             |

### Copy Failure

If copy shortcut fails (no target key, no content):

- Show popup: "Nothing to copy"
- Window stays open