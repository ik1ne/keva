# Project Specification: Keva

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

Values are stored using clipboard-native formats:

1. **Richest Format:** The highest-fidelity format from clipboard (e.g., image, HTML, application-specific data).
2. **Plain Text (Optional):** Stored alongside rich format if meaningful (non-empty, non-whitespace).

When pasting from clipboard:

- If rich data exists → store rich data (+ plain text if meaningful)
- If only plain text exists → store plain text only

Examples:
| Copy Source | Stored |
|-------------|--------|
| Text from browser | plain text + HTML |
| Cell from Excel | plain text + Excel format |
| Image from Photoshop | image only |
| Screenshot | image only |
| File from Finder/Explorer | file contents (hard copy) |
| Formatted text from Word | plain text + RTF/HTML |

## 3. Interfaces & Features

### CLI Commands

- `get <key>`: Output the plain text value to stdout. Outputs empty string if no plain text exists.
    - `--raw`: Output the rich format as binary to stdout.
- `set <key> <value>`: Set the plain text value for the key.
- `rm <key>`: Remove the key. Behavior depends on configuration.
    - `-r` / `--recursive`: Delete the key and all its children.
    - `--trash`: Force soft delete (move to Trash).
    - `--permanent`: Force immediate, permanent deletion.
- `mv <key> <new_key>`: Rename/move a key without modifying its value. Fails if `<new_key>` already exists unless
  `--force` is specified.
    - `--force`: Overwrite existing key at destination.
- `ls <key>`: List children of the key.
- `search <query>`: Search the database using the configured Hybrid Logic.
    - Output: List of matching keys (ranked).
- `paste <key>`: Write current clipboard content to the key (follows clipboard-native storage rules).
- `copy <key>`: Copy the key's value to the clipboard.
- **Flags:** `--include-trash` to search/list deleted items.

### GUI Design

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
- Values are auto-saved after inactivity period or on window close.

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

#### Drag & Drop

- Drop on **Right Pane:** Stores file contents to currently targeted key.
- Drop on **Left Pane (Specific Key):** Stores file contents to that key.
- **Large File Handling:** Files exceeding threshold trigger confirmation prompt before copy.

## 4. Operational Behaviors

### Configuration & Preferences

The following behaviors are user-configurable via a persistent config file.

- **Delete Style:**
    - **Soft (Default):** Deletions move items to **Trash**.
    - **Immediate:** Deletions permanently remove items, skipping the Trash lifecycle.
    - *Note: CLI flags (`--permanent`, `--trash`) and GUI modifiers (`Shift+Delete`) override this setting.*
- **Large File Threshold:** Size limit triggering the import confirmation prompt (Default: **256MB**).
- **TTL Durations:** Timers for Lifecycle stages (Active → Trash → Purge).

### Safety Thresholds

- **Large Files:** If a paste/drop operation exceeds the **Large File Threshold** (Configurable, default 256MB), the
  system must prompt the user for explicit confirmation before proceeding.

### Lifecycle Management (Waterfall TTL)

Items progress through three stages based on configurable timestamps.

1. **Active:** Normal visibility.
2. **Trash:** Item is marked as soft-deleted and hidden from standard view.
    - **Condition:** Skipped if **Delete Style** is set to `Immediate` or if `rm --permanent` is used.
    - **CLI:** Accessible only via `--include-trash`.
    - **GUI:** Always searchable (bottom of list, 🗑️ icon).
3. **Purge:** Item is considered permanently deleted.
    - **Visibility:** Hidden from all interfaces (Search/List/Get) immediately upon TTL expiration.
    - **Storage:** Physical data persists until the next Garbage Collection cycle.

### Garbage Collection (GC)

To maintain performance and reclaim disk space, Keva performs automated maintenance.

- **Trigger:** Automated background process runs upon application exit (interval configurable).
- **Scope:**
    - Moves items from **Active** to **Trash** based on TTL.
    - Permanently removes items in the **Trash** stage based on TTL.
        - Reclaims storage space from deleted file blobs.
- **Manual Override:**
    - Users can force a cleanup immediately via the CLI command `kv gc`.
    - Users can force no cleanup upon exit via the CLI flag `--no-gc`.