# Technical Notes: Keva

## 1. High-Level Architecture

Keva operates as a local-first application with a strict separation between the "Logical" layer (API/Rules) and the "
Physical" layer (Storage).

### Hybrid Storage Strategy

To support both high performance for metadata and storage for large binary files (256MB+), Keva uses a split approach:

- **Metadata & Small Values:** Stored in `redb` (Embedded B-Tree Database).
- **Large Blobs (>1MB):** Stored as raw files in a managed `blobs/` directory. The DB stores only the pointer.
- **Consistency:** Write operations prioritize "File First, DB Second." Read operations resolve the path dynamically.

### The "Dot" Namespace Strategy

To resolve the filesystem conflict where a path cannot be both a file and a folder:

- **Concept:** Every key is treated as a container.
- **Implementation:** If a user stores a value at `project`, the system internally stores it at `project/.`.

## 2. Search Engine Design

### Hybrid Search Logic

Keva avoids complex database indexing in favor of a high-performance **Parallel Linear Scan**.

- **Fuzzy Mode (Default):**
    - **Trigger:** Query contains standard characters (alphanumerics, `/`, `.`, `-`).
    - **Engine:** `nucleo-matcher`.
    - **Behavior:** Subsequence matching (e.g., `a/b/c` matches `a/big/cat`).
- **Regex Mode:**
    - **Trigger:** Query contains specific regex symbols (`*`, `?`, `^`, etc.).
    - **Engine:** Rust `regex` crate.
    - **Sorting:** *Deferred (TBD).*

## 3. Garbage Collection (GC) Strategy

### Detached "Scavenger" Process

To avoid the overhead of a permanent background daemon, Keva uses a "Fire-and-Forget" model.

- **Mechanism:** The main application spawns a separate child process (`keva --gc`) to handle cleanup tasks.
- **Behavior:** The main app **detaches** immediately. The child process opens the DB, checks the `lifecycle`
  timestamps, performs the cleanup, and exits.