# Project Specification: Keva

## 1. Overview

Keva is a local key-value store designed for clipboard-like data management. It provides fast storage and retrieval with
fuzzy search capabilities.

- **Name:** Keva
- **Platforms:** macOS (.app bundle), Windows (installer)

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

Single-process application containing GUI window and keva-core storage layer. System tray integration in Phase 2+.

**Phases:**

| Phase | Features                                | GC Trigger   |
|-------|-----------------------------------------|--------------|
| 1     | Window only, no tray, no hotkey         | Window close |
| 2+    | System tray, global hotkey, window hide | Window hide  |

## 4. GUI

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
    - `Ctrl+V` with files → stored as files, shows preview
    - `Ctrl+V` with plain text → inserted at cursor
    - Drag & drop file → stored as file contents, shows preview

**Text Editing State (plain text value exists):**

- Standard text editor behavior
- `Ctrl+V`:
    - If clipboard contains plain text → insert at cursor
    - If clipboard contains only files → blocked, show hint: "Clear text to paste files"
- Auto-save after 3 seconds of inactivity or on window close/hide

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
- **TTL Filtering:**
    - Items past trash TTL shown as trash (even if GC hasn't run)
    - Items past purge TTL excluded from results

### Keyboard Shortcuts

| State                             | Key           | Action                                            |
|-----------------------------------|---------------|---------------------------------------------------|
| Key selected in left pane         | `Enter`       | Copy value to clipboard, close/hide window        |
| Key selected in left pane         | `Shift+Enter` | Focus right pane for editing                      |
| No selection, search bar has text | `Enter`       | Focus right pane for editing (creates key if new) |

### Drag & Drop

- Drop on **Right Pane:** Stores file contents to currently targeted key.
- Drop on **Left Pane (Specific Key):** Stores file contents to that key.
- **Large File Handling:** Files exceeding threshold trigger confirmation prompt.

## 5. Configuration

### Data Settings

- **Delete Style:**
    - **Soft (Default):** Deletions move items to Trash.
    - **Immediate:** Deletions permanently remove items.
- **Large File Threshold:** Size limit triggering import confirmation (Default: 256MB).
- **TTL Durations:** Configurable timers for lifecycle stages.

### Application Settings (Phase 2+)

- **Global Shortcut:** Key combination to show/hide window.
- **Launch at Login:** Toggle for auto-start.

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

### Garbage Collection

- Moves items from Active to Trash based on TTL
- Permanently removes items past purge TTL
- Reclaims storage space from deleted blobs

Trigger: Window close (Phase 1) or window hide (Phase 2+).