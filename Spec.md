# Project Specification: Keva

## 1. Overview

Keva is a local Key-Value store designed for structured data management. It features a hierarchical namespace and
supports dual interfaces: a CLI for automation/scripting and a GUI for visual exploration.

- **Name:** Keva
- **CLI Alias:** `kv`

## 2. Core Concepts

### Namespace Structure

- **Hierarchy:** Keys are path-based (e.g., `project/config/theme`).
- **Parallel Storage:** A key (e.g., `project`) can contain children (like a folder) *and* store its own value (like a
  file) simultaneously. This allows `project` to have text content while `project/config` exists as a child key.

### Supported Value Types

1. **Text:** Plain strings.
2. **Rich Text:** Formatted content (Markdown).
3. **File (Embedded):** Binary file copy stored fully within Keva.
4. **File (Linked):** A reference path to an external file on the OS.

## 3. Interfaces & Features

### CLI Commands

- `get <key>`: Output the value to stdout.
- `set <key> <value>`: Set the value for the key.
- `rm <key>`: Remove the key. Behavior depends on configuration.
    - `--trash`: Force soft delete (move to Trash).
    - `--permanent`: Force immediate, permanent deletion.
- `ls <key>`: List children of the key.
- `paste <key>`: Write current clipboard content to the key.
- `copy <key>`: Copy the key's value to the clipboard.
- `import <key> <file>`: Embed a file (copy) into the store.
- `link <key> <file>`: Store a reference link to the file.
- **Flags:** `--include-trash` to search deleted items.

### GUI Design

- **Layout:** Split Pane.
    - **Left:** Tree Explorer (visualizing hierarchy).
    - **Right:** Inspector/Preview (renders text, images, PDF, etc.).
- **Search Behavior (Spotlight-style):**
    - **Scope:** Defaults to **Key Search**. Optional toggle for **Value Content**.
    - **Filtering:**
        - **TTL Check:**
            - Items exceeding their **Trash** timestamp are treated as **Trash** (ranked bottom + icon), even if the gc
              has not yet run.
            - Items exceeding their **Purge** timestamp are automatically excluded from results.
        - **Trash Handling:** Trash items are included in search results but ranked at the bottom with a 🗑️ icon.
    - **Hybrid Logic:**
        - **Fuzzy Mode (Default):** Active when query contains alphanumerics, `-`, `_`, `/`, `.`, or space.
            - **Ranking:** Exact Match > Prefix/Children > Substring > Subsequence.
            - **Selection:** Automatically selects the top result for immediate preview.
        - **Regex Mode:** Active when query contains regex symbols (e.g., `*`, `?`, `^`, `[`). Sorts by shortest match
          first.
    - **Visuals:** Shows "Magnet" icon 🧲 for Fuzzy, "Code" icon `.*` for Regex.
- **Drag & Drop:**
    - Drop on **Right Pane:** Overwrites the currently selected key.
    - Drop on **Left Pane (Empty Space):** Prompts to create a new key using the filename.
    - Drop on **Left Pane (Specific Key):** Prompts to import/link into that key.
    - **Modifier Key:** Holding Alt/Option inverts the default import behavior (Embed vs. Link).

## 4. Operational Behaviors

### Configuration & Preferences

The following behaviors are user-configurable via a persistent config file.

- **Delete Style:**
    - **Soft (Default):** Deletions move items to **Trash**.
    - **Immediate:** Deletions permanently remove items, skipping the Trash lifecycle.
    - *Note: CLI flags (`--permanent`, `--trash`) and GUI modifiers (`Shift+Delete`) override this setting.*
- **Import Style:**
    - **Embed (Default):** Files dropped or imported are copied into Keva's storage.
    - **Link:** Files dropped or imported store a reference path to the original file.
    - *Note: Holding Alt/Option during drag-and-drop inverts this behavior.*
- **Large File Threshold:** Size limit triggering the import confirmation prompt (Default: **256MB**).
- **TTL Durations:** Timers for Lifecycle stages (Active → Trash → Purge).

### Safety Thresholds

- **Large Files:** If an import operation exceeds the **Large File Threshold** (Configurable, default 256MB), the system
  must prompt the user for explicit confirmation before proceeding.

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
