# Project Specification: Keva

## 1. Overview

Keva is a local Key-Value store designed for structured data management. It features a hierarchical namespace and
supports dual interfaces: a CLI for automation/scripting and a GUI for visual exploration.

- **Name:** Keva
- **CLI Alias:** `kv`

## 2. Core Logic

### Namespace Structure

- **Hierarchy:** Keys are path-based (e.g., `project/config/theme`).
- **"Dot" Strategy:** To support both folder-like and file-like behaviors simultaneously, a key (e.g., `project`) can
  contain children *and* store its own value. The value for `project` is effectively stored at `project/.`.

### Supported Value Types

1. **Text:** Plain strings.
2. **Rich Text:** Formatted content.
3. **File (Embedded):** Binary file copy stored fully within Keva.
4. **File (Linked):** A reference path to an external file on the OS.

## 3. Interfaces & Features

### CLI Commands

- `get <key>`: Output the value to stdout.
- `set <key> <value>`: Set the value for the key.
- `rm <key>`: Remove the key (soft delete to Trash).
- `ls <key>`: List children of the key.
- `paste <key>`: Write current clipboard content to the key.
- `copy <key>`: Copy the key's value to the clipboard.
- `import <key> <file>`: Embed a file (copy) into the store.
- `link <key> <file>`: Store a reference link to the file.
- **Flags:** Support for accessing "Trash" (deleted items).

### GUI Design

- **Layout:** Split Pane.
    - **Left:** Tree Explorer (visualizing hierarchy).
    - **Right:** Inspector/Preview (renders text, images, PDF, etc.).
- **Search (Spotlight-style):**
    - Defaults to **Regex** matching on **Keys**.
    - Optional toggle to include **Value** content in search (off by default).
- **Drag & Drop:**
    - Drop on **Right Pane:** Overwrites the currently selected key.
    - Drop on **Left Pane (Empty Space):** Prompts to create a new key using the filename.
    - Drop on **Left Pane (Specific Key):** Prompts to import/link into that key.
    - **Modifier Key:** Holding Alt/Option inverts the default import behavior (Embed vs. Link). (default behavior is
      configurable)

## 4. Operational Behaviors

### Safety Thresholds

- **Large Files:** If an import operation exceeds **256MB** (configurable), the system must prompt the user for explicit
  confirmation before proceeding.

### Lifecycle Management (Waterfall TTL)

Items can progress through three automated stages based on time(three configurable ttl values):

1. **Active:** Normal visibility.
2. **Stale:** Item remains visible/accessible but is visually marked as expired (warning).
3. **Trash:** Item is moved to a hidden `__trash__` namespace (soft delete).
4. **Purge:** Item is permanently deleted.
