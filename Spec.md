# Project Specification: Keva

## 1. Overview

Keva is a local key-value store designed for clipboard-like data management. It provides fast storage and retrieval with
fuzzy search capabilities.

- **Name:** Keva
- **Platforms:** macOS (.app bundle), Windows (installer with uninstaller)

## 2. Core Concepts

### Keys

Keys are flat strings. The `/` character has no special meaning to the storage layer. The GUI may visually group keys
sharing common prefixes, but this is cosmetic only with no behavioral implications.

Examples of valid keys:

- `project/config/theme`
- `my-notes`
- `2024/01/15`

### Value Types

Values are stored as one of two types:

1. **Text:** Plain text content.
2. **Files:** One or more files copied from file manager (hard copy of file contents).

| Copy Source               | Stored As                |
|---------------------------|--------------------------|
| Text from any application | Text                     |
| File from Finder/Explorer | Files (hard copy)        |
| Multiple files            | Files (multiple entries) |

When clipboard contains both files and text, **files take priority** (text is discarded).

## 3. Architecture

Single-process application containing GUI window and keva-core storage layer.

**Process Behavior:**

- Starts as background process (no window on launch)
- No Dock icon (macOS) / Taskbar icon visible (Windows)
    - Windows limitation: hiding from taskbar also hides from Alt+Tab, so taskbar icon is kept
- Menu bar icon (macOS) / System tray icon (Windows) visible by default
- Window hidden keeps process alive in background

**Launch and Activation:**

- Global shortcut `Cmd+Shift+K` (macOS) / `Ctrl+Shift+K` (Windows) shows window
- Launch at login: user opts in via first-run dialog (see Section 4)
    - macOS: Login Items via `SMAppService`
    - Windows: Registry `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- Must appear correctly in OS startup settings (Task Manager on Windows, Login Items on macOS)

**Single Instance:**

- Only one instance runs at a time
- Relaunch app (Spotlight, double-click) → activates existing instance's window
- Implementation:
    - Windows: Named mutex + `WM_COPYDATA` message
    - macOS: Unix domain socket in data directory
- If existing instance doesn't respond within 2 seconds: offer to force-quit and relaunch

**In-Memory State:**

- Search index built on launch, persists while process runs

**Windows Uninstaller:**

- Remove startup registry entry (`HKCU\...\Run`)
- Remove application files
- Prompt: "Delete all Keva data?" (config, database, blobs)
    - Yes: Remove data directory
    - No: Leave data directory intact

## 4. GUI

### Cross-Platform Consistency

The UI is identical on macOS and Windows:

- Custom-drawn window (no native decorations)
- No title bar, close/minimize/maximize buttons
- Same fonts, colors, and layout on all platforms
- Only keyboard shortcuts differ (`Cmd` on macOS, `Ctrl` on Windows)

### Window Behavior

**Window Controls:**

- `Cmd+Shift+K` / `Ctrl+Shift+K` → Show window (global shortcut, works when hidden)
- `Esc` → Hide window (only when window is focused)
- `Cmd+Q` / `Alt+F4` → Quit app entirely (only when window is focused)
- Window does NOT close on focus loss (supports drag/drop and copy/paste workflows)
- Tray icon left-click also toggles visibility

**Resize and Move:**

The window has a thin border area (approximately 8px total):

- **Outer 5px:** Resize handle (triggers OS-level resize)
- **Inner 3px:** Drag to move window

Additionally, the search icon (🔍) in the search bar acts as a drag handle. Clicking it does nothing; dragging it moves
the window.

**Window Positioning:**

- First launch: Center of primary monitor
- Subsequent launches: Remember last position and size per monitor
    - Position stored keyed by monitor identifier
    - If monitor configuration changes, center on current monitor
- Multi-monitor: If remembered position is off-screen, center on monitor containing cursor

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

Split pane with three components:

- **Top:** Search bar (key filter, fuzzy matching)
- **Left:** Key list (filtered by search bar)
- **Right:** Inspector/Preview pane (view or edit value)

### Search Bar and Left Pane Relationship

Search bar and left pane selection are independent:

- Search bar filters the left pane results AND serves as target key for right pane when nothing is selected.
- Clicking a key in left pane does NOT update search bar.
- Right pane shows: selected key's value (if selection exists) OR empty editor for search bar's key (if no selection).

### Right Pane Behavior

**Empty State (no value for target key):**

- Shows text input with placeholder: `Write or paste value for "<key>"`
- Accepts:
    - Text input → stored as plain text
    - Paste (`Cmd+V` / `Ctrl+V`) with files → stored as files, shows preview
    - Paste (`Cmd+V` / `Ctrl+V`) with plain text → inserted at cursor
    - Drag & drop file → stored as file contents, shows preview

**Text Editing State (plain text value exists):**

- Standard text editor behavior
- Paste (`Cmd+V` / `Ctrl+V`):
    - If clipboard contains plain text → insert at cursor
    - If clipboard contains only files → blocked, show hint: "Clear text to paste files"
- Auto-save after 3 seconds of inactivity or on window hide

**Preview State (files value exists):**

- Shows file list/preview
- Delete button to clear value and return to empty state

### Left Pane Controls

Each key displays on hover/selection:

- **Rename button (pen icon):** Opens inline editor to modify key. Confirmation prompt if rename would overwrite
  existing key.
- **Delete button (trash icon):** Deletes the key (follows configured delete style).

### Search Behavior

- **Mode:** Fuzzy matching only
- **Ranking:** Exact match > Prefix > Substring > Subsequence
- **Case Sensitivity:** Smart case (case-insensitive unless query contains uppercase)
- **Trash Handling:** Trash items included but ranked at bottom with 🗑️ icon
- **Stale Items:** Items past TTL remain visible until GC runs (GC is the single source of truth for state transitions)

### Keyboard Shortcuts

| State                             | Key            | Action                                            |
|-----------------------------------|----------------|---------------------------------------------------|
| Global (even when hidden)         | `Cmd+Shift+K`  | Show window (macOS)                               |
| Global (even when hidden)         | `Ctrl+Shift+K` | Show window (Windows)                             |
| Window focused                    | `Esc`          | Hide window (process stays alive)                 |
| Window focused                    | `Cmd+Q`        | Quit app entirely (macOS)                         |
| Window focused                    | `Alt+F4`       | Quit app entirely (Windows)                       |
| Key selected in left pane         | `Shift+Enter`  | Copy value to clipboard, hide window              |
| Key selected in left pane         | `Enter`        | Focus right pane for editing                      |
| No selection, search bar has text | `Enter`        | Focus right pane for editing (creates key if new) |
| Window focused                    | `Cmd+,`        | Open settings dialog                              |

### Keyboard Navigation

| Key       | Action                     |
|-----------|----------------------------|
| `↑` / `↓` | Navigate key list          |
| `Cmd+F`   | Focus search bar (macOS)   |
| `Ctrl+F`  | Focus search bar (Windows) |

### First-Run Experience

On very first launch (no config.toml exists):

1. Show welcome dialog with:
    - Brief explanation: "Keva runs in the background. Use Cmd+Shift+K (macOS) / Ctrl+Shift+K (Windows) to show the
      window."
    - Checkbox: "Launch Keva at login" (checked by default)
    - Button: "Get Started"
2. If checkbox is checked, register login item
3. Create config.toml with user preferences
4. Show main window

### Settings Dialog

- Opened via `Cmd+,` or tray icon menu
- Changes saved to config file on dialog close
- Applied immediately to running application

**Settings Categories:**

| Category  | Setting              | Description                           |
|-----------|----------------------|---------------------------------------|
| General   | Theme                | Dark / Light / Follow System          |
| General   | Launch at Login      | Toggle auto-start                     |
| General   | Show Tray Icon       | Toggle tray icon visibility           |
| Shortcuts | Global Shortcut      | Key combination to show window        |
| Data      | Delete Style         | Soft (to trash) / Immediate           |
| Data      | Large File Threshold | Size limit before confirmation prompt |
| Lifecycle | Trash TTL            | Days before items auto-trash          |
| Lifecycle | Purge TTL            | Days before trashed items are deleted |

**Note:** If tray icon is hidden and window is also hidden, user can still access settings by relaunching the app
(which shows the existing instance's window) and pressing `Cmd+,` / `Ctrl+,`.

### Drag & Drop

- Drop on **Right Pane:** Stores file contents to currently targeted key.
- Drop on **Left Pane (Specific Key):** Stores file contents to that key.
- **Large File Handling:** Files exceeding threshold trigger confirmation prompt.

## 5. Configuration

### Data Directory

Default location: `~/.keva/`

Override via environment variable: `KEVA_DATA_DIR`

Directory structure:

```
{data_dir}/
├── config.toml    # Adjustable settings
├── keva.redb      # Database
└── blobs/         # Large file storage
```

### Config File Format

Location: `{data_dir}/config.toml`

```toml
# Config version for migration support
config_version = 1

# Appearance: "dark", "light", or "system"
theme = "system"

# Global shortcut to show window
# Format: "Modifier+Key" (e.g., "Cmd+Shift+K", "Ctrl+Shift+K")
global_shortcut = "Cmd+Shift+K"  # or "Ctrl+Shift+K" on Windows

# Start automatically at login
launch_at_login = true

# Show menu bar / system tray icon
show_tray_icon = true

# Delete behavior: "soft" (to trash) or "immediate" (permanent)
delete_style = "soft"

# Files larger than this trigger confirmation prompt (bytes)
large_file_threshold = 268435456  # 256 MB

# Duration before active items move to trash (seconds)
trash_ttl = 2592000  # 30 days

# Duration before trashed items are purged (seconds)
purge_ttl = 604800  # 7 days

# Window position and size per monitor (managed automatically)
# Key format: "monitor_<identifier>" where identifier is OS-provided
[window.default]
width = 800
height = 600

[window.monitors."monitor_12345"]  # Example: specific monitor
x = 100
y = 100
width = 800
height = 600
```

### Config Validation

On launch, if config.toml contains invalid values:

1. Popup displays specific validation errors
2. User chooses: **[Launch with defaults]** or **[Quit]**
3. "Launch with defaults" overwrites invalid fields and proceeds
4. "Quit" exits without modifying config file

If config.toml is missing: created with defaults, no popup.

### Settings Reference

| Setting                | Default             | Description                                                |
|------------------------|---------------------|------------------------------------------------------------|
| `config_version`       | `1`                 | Config format version for migrations                       |
| `theme`                | `"system"`          | `"dark"`, `"light"`, or `"system"` (follow OS)             |
| `global_shortcut`      | `"Cmd+Shift+K"`     | Key combination to show window (platform-specific default) |
| `launch_at_login`      | `true`              | Start automatically at system login                        |
| `show_tray_icon`       | `true`              | Show menu bar / system tray icon                           |
| `delete_style`         | `"soft"`            | `"soft"` = move to trash, `"immediate"` = permanent delete |
| `large_file_threshold` | `268435456` (256MB) | Confirmation prompt for files exceeding this size (bytes)  |
| `trash_ttl`            | `2592000` (30 days) | Seconds before inactive items move to trash                |
| `purge_ttl`            | `604800` (7 days)   | Seconds before trashed items are permanently deleted       |

## 6. Lifecycle Management

### Timestamps

Each key stores:

- **created_at:** When the key was first created.
- **updated_at:** When the value was last modified.
- **last_accessed:** When the key was last viewed or copied to clipboard.
- **trashed_at:** When the key was moved to Trash (if applicable).

### TTL Calculation

TTL expiration is based on `last_accessed`. Operations that update `last_accessed`:

- Selecting key in left pane (viewing in right pane)
- Copying value to clipboard

### Lifecycle Stages

1. **Active:** Normal visibility. Transitions to Trash when `last_accessed + trash_ttl` expires.

2. **Trash:** Soft-deleted, hidden from default view.
    - Skipped if delete style is Immediate.
    - Searchable (bottom of results, 🗑️ icon).
    - Transitions to Purge when `trashed_at + purge_ttl` expires.

3. **Purge:** Considered permanently deleted.
    - Hidden from all interfaces immediately upon TTL expiration.
    - Physical data removed at next GC cycle.

### Maintenance (Garbage Collection)

- Moves items from Active to Trash based on TTL
- Permanently removes items past purge TTL
- Reclaims storage space from deleted blobs
- May perform in-memory maintenance tasks (e.g., search index compaction) to avoid heavy work during active UI
  interaction

Triggers:

- Window hide
- App quit
- Periodically while running (configurable; default: 1 day)

Note: Search results may be slightly outdated until maintenance runs.

## 7. Error Handling

### Global Shortcut Conflicts

If the configured shortcut is already registered by another application:

1. Show notification: "Shortcut Cmd+Shift+K is in use by another application"
2. Open settings dialog with shortcut field focused
3. User must choose a different shortcut or resolve the conflict externally

### Database Errors

| Error              | User Message                                       | Recovery Action                    |
|--------------------|----------------------------------------------------|------------------------------------|
| Database corrupted | "Database is corrupted. Create new database?"      | Backup old, create fresh           |
| Disk full          | "Disk is full. Cannot save changes."               | Retry after user frees space       |
| File locked        | "Database is locked by another process."           | Offer to force-quit other instance |
| Permission denied  | "Cannot access data directory. Check permissions." | Show path, suggest fix             |

### Auto-Save Failure

If auto-save fails (disk full, permissions, etc.):

1. Show non-blocking notification: "Failed to save changes"
2. Keep unsaved changes in memory
3. Retry on next edit or explicit save
4. On window hide: warn user before hiding if unsaved changes exist

### Large File Handling

- Threshold applies **per file**, not total
- Files exceeding threshold show confirmation: "File X is Y MB. Store anyway?"
- **Hard maximum:** 1 GB per file (reject larger files with error message)
- Multiple files: each checked individually against threshold