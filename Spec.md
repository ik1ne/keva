# Project Specification: Keva (v3)

## 1. Overview

Keva is a local Key-Value store designed for structured data management. It features a hierarchical namespace and
supports dual interfaces: a CLI for automation/scripting and a GUI for visual exploration.

- **Name:** Keva
- **CLI Alias:** `kv`

## 2. Core Concepts

### Namespace Structure

- **Hierarchy:** Keys are path-based (e.g., `project/config/theme`). Note: This is a logical hierarchy.
    - **No Implicit Parents:** Creating a key `a/b/c` does NOT automatically create `a` or `a/b`.
    - **Non-Recursive Delete:** Deleting `a` does NOT delete `a/b`.
- **Parallel Storage:** A key (e.g., `project`) can contain children (like a folder) *and* store its own value (like a
  file) simultaneously. This allows `project` to have text content while `project/config` exists as a child key.

### Supported Value Types

Values are stored as one of two types:

1. **Text:** Plain text content (inlined in database or blob-stored for large text).
2. **Files:** One or more files copied from file manager (hard copy of file contents).

*Note: Rich format support (HTML, images, RTF, application-specific data) is planned for a future version.*

Examples:
| Copy Source | Stored |
|-------------|--------|
| Text from any application | Text |
| File from Finder/Explorer | Files (hard copy) |
| Multiple files | Files (multiple entries) |

## 3. Architecture

### Component Overview

```
┌─────────────────────────────────────────────┐
│                 Storage                     │
└──────────────┬──────────────┬───────────────┘
               │              │
        ┌──────┴──────┐ ┌─────┴─────┐
        │     CLI     │ │  Daemon   │
        └─────────────┘ └─────┬─────┘
                              │
                        ┌─────┴─────┐
                        │    GUI    │
                        └───────────┘
```

### Storage Access

- **CLI:** Accesses storage directly.
- **Daemon:** Accesses storage directly. GUI window runs inside daemon process.
- **Concurrency:** Storage layer handles locking internally. Multiple readers allowed; writers block until prior write
  commits.
- **Simultaneous Access:** CLI and daemon can run simultaneously. Write operations may briefly block if both attempt
  concurrent writes.

### Operating Modes

| Mode               | Hotkey | GC               | Use Case                 |
|--------------------|--------|------------------|--------------------------|
| CLI only           | None   | Manual (`kv gc`) | Scripting, automation    |
| GUI without daemon | None   | On window close  | Occasional manual launch |
| GUI with daemon    | Yes    | On window hide   | Quick access via hotkey  |

## 4. Interfaces & Features

### CLI Commands

#### Data Operations

- `get <key>`: Output the plain text value to stdout. Outputs empty string if no plain text exists.
    - `--raw`: Output the rich format as binary to stdout.
- `set <key> <value>`: Set the plain text value for the key.
- `rm <key>`: Remove the key. Behavior depends on configuration.
    - `-r` / `--recursive`: Delete the key and all its children (keys matching `<key>` and `<key>/*`).
    - `--trash`: Force soft delete (move to Trash).
    - `--permanent`: Force immediate, permanent deletion.
- `mv <key> <new_key>`: Rename/move a key without modifying its value. Fails if `<new_key>` already exists unless
  `--force` is specified.
    - `--force`: Overwrite existing key at destination.
- `ls [prefix]`: List all keys matching the prefix (or all keys if no prefix given).
    - `--include-trash`: Include trashed items in results (hidden by default).
- `paste <key>`: Write current clipboard content to the key (follows clipboard-native storage rules).
- `copy <key>`: Copy the key's value to the clipboard.
- `gc`: Manually trigger garbage collection.

#### Daemon Operations

- `daemon start`: Start the daemon process (if not already running).
- `daemon stop`: Stop the running daemon process.
- `daemon status`: Check if daemon is running.
- `daemon install`: Register daemon to launch at system login.
- `daemon uninstall`: Remove launch-at-login registration.

### GUI Design

#### Launch Methods

1. **Without daemon:** Run `kv gui` or launch application directly.
2. **With daemon:** Press configured hotkey.

#### Layout

Split Pane with three components:

- **Top:** Search/Spotlight bar (key input and filter)
- **Left:** Tree Explorer (visualizing hierarchy, filtered by search bar)
- **Right:** Inspector/Preview pane (renders content or provides editor)

#### Search Bar and Left Pane Relationship

Search bar and left pane selection are **independent**:

- Search bar filters the left pane results AND serves as target key for right pane when nothing is selected.
- Clicking a key in left pane does NOT update search bar (prevents list reshuffling).
- Right pane shows: selected key's value (if selection exists) OR empty editor for search bar's key (if no selection).

#### Right Pane Behavior

**Empty State (no value for target key):**

- Shows text input with placeholder: `Write or paste value for "<key>"`
- Accepts:
    - Text input → stored as plain text
    - `Ctrl+V` with rich data → stored as rich format, shows preview
    - `Ctrl+V` with plain text only → inserted at cursor
    - Drag & drop file → stored as file contents, shows preview

**Text Editing State (plain text value exists):**

- Standard text editor behavior
- `Ctrl+V`:
    - If clipboard contains plain text → insert at cursor (even if rich data also exists)
    - If clipboard contains only rich data → blocked, show hint bar: "Clear text to paste rich content"
- Right-click menu:
    - If clipboard has plain text: Cut, Copy, Paste (enabled)
    - If clipboard has only rich data: Cut, Copy, Paste (grayed out)

**Preview State (rich format value exists):**

- Shows rendered preview (image, PDF, formatted text, etc.)
- Delete button visible to clear value and return to empty state

#### Left Pane Controls

Each tree node displays on hover/selection:

- **Rename button (pen icon):** Opens inline editor to modify key path. Confirmation prompt if rename would overwrite
  existing key.
- **Delete button (trash icon):** Deletes the key (follows configured delete style).

#### Key Creation and Saving

- Keys are created implicitly when a value is first added.
- Values are auto-saved after inactivity period or on window close/hide.

#### Search Behavior (Spotlight-style)

- **Scope:** Defaults to **Key Search**. Optional toggle for **Value Content**.
- **Filtering:**
    - **TTL Check:**
        - Items exceeding their **Trash** timestamp are treated as **Trash** (ranked bottom + icon), even if GC has not
          yet run.
        - Items exceeding their **Purge** timestamp are automatically excluded from results.
    - **Trash Handling:** Trash items are included in search results but ranked at the bottom with a 🗑️ icon.
- **Hybrid Logic:**
    - **Fuzzy Mode (Default):** Active when query contains alphanumerics, `-`, `_`, `/`, `.`, or space.
        - **Ranking:** Exact Match > Prefix/Children > Substring > Subsequence.
    - **Regex Mode:** Active when query contains regex symbols (e.g., `*`, `?`, `^`, `[`). Sorts by shortest match
      first.
- **Visuals:** Shows "Magnet" icon 🧲 for Fuzzy, "Code" icon `.*` for Regex.

#### Keyboard Shortcuts

| State                             | Key           | Action                                            |
|-----------------------------------|---------------|---------------------------------------------------|
| Key selected in left pane         | `Enter`       | Copy value to clipboard, close window             |
| Key selected in left pane         | `Shift+Enter` | Focus right pane for editing                      |
| No selection, search bar has text | `Enter`       | Focus right pane for editing (creates key if new) |

#### Drag & Drop

- Drop on **Right Pane:** Stores file contents to currently targeted key.
- Drop on **Left Pane (Specific Key):** Stores file contents to that key.
- **Large File Handling:** Files exceeding threshold trigger confirmation prompt before copy.

## 5. Operational Behaviors

### Configuration & Preferences

The following behaviors are user-configurable via a config file and/or GUI preferences pane.

#### Data Settings

- **Delete Style:**
    - **Soft (Default):** Deletions move items to **Trash**.
    - **Immediate:** Deletions permanently remove items, skipping the Trash lifecycle.
    - *Note: CLI flags (`--permanent`, `--trash`) and GUI modifiers (`Shift+Delete`) override this setting.*
- **Large File Threshold:** Size limit triggering the import confirmation prompt (Default: **256MB**).
- **TTL Durations:** Timers for Lifecycle stages (Active → Trash → Purge).

#### Daemon Settings (GUI Preferences)

- **Global Shortcut:** Key combination to show/hide GUI window.
- **Launch at Login:** Toggle to enable/disable daemon auto-start.
- **Daemon Status:** Indicator showing whether daemon is currently running.

### Safety Thresholds

- **Large Files:** If a paste/drop operation exceeds the **Large File Threshold** (Configurable, default 256MB), the
  system must prompt the user for explicit confirmation before proceeding.

### Lifecycle Management (Waterfall TTL)

Items progress through three stages based on configurable timestamps.

1. **Active:** Normal visibility.
2. **Trash:** Item is marked as soft-deleted and hidden from standard view.
    - **Condition:** Skipped if **Delete Style** is set to `Immediate` or if `rm --permanent` is used.
    - **CLI:** Hidden from `ls` by default; use `--include-trash` to show.
    - **GUI:** Always searchable (bottom of list, 🗑️ icon).
3. **Purge:** Item is considered permanently deleted.
    - **Visibility:** Hidden from all interfaces (Search/List/Get) immediately upon TTL expiration.
    - **Storage:** Physical data persists until the next Garbage Collection cycle.

### Garbage Collection (GC)

To maintain performance and reclaim disk space, Keva performs automated maintenance.

- **Scope:**
    - Moves items from **Active** to **Trash** based on TTL.
    - Permanently removes items in the **Trash** stage based on TTL.
    - Reclaims storage space from deleted file blobs.

- **Trigger by Mode:**

| Mode               | GC Trigger              |
|--------------------|-------------------------|
| CLI                | Manual only via `kv gc` |
| GUI without daemon | On window close         |
| GUI with daemon    | On window hide          |

- **Note:** CLI-only users who never run `kv gc` will accumulate trash until manually triggered.

---

## 6. Future Plans

The following features are planned but not yet implemented:

### CLI Search Command

- `search <query>`: Search the database using Hybrid Logic (fuzzy + regex).
- Requires daemon for nucleo persistence to achieve acceptable performance.
- Until daemon is implemented, search is GUI-only.

### Direct Children Listing

- `ls --children <key>`: List only direct children of a key (one level deep).
- Example: If keys `a`, `a/b`, `a/b/c` exist, `ls --children a` returns only `a/b`.
- Useful for tree navigation in GUI.

### Rich Format Support

- HTML, images, RTF, application-specific clipboard data.
- `get --raw`: Output rich format as binary to stdout.

### Value Content Search

- Search within value contents, not just keys.
- Toggle in GUI search bar.