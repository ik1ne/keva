# Future Plans

Features planned but not in v1 scope:

1. **Rich format support**: HTML, images, RTF, application-specific clipboard data. Includes binary output for
   programmatic access.

2. **Value content search**: Search within value contents, not just keys.

3. **Regex search mode**: Regular expression matching as alternative to fuzzy search.

4. **CLI interface**: Command-line interface for scripting and automation.

---

## CLI Specification (Reference)

Preserved from v1 planning for future implementation.

### CLI Alias

`kv`

### Data Operations

- `get <key>`: Output the plain text value to stdout. Outputs empty string if no plain text exists.
    - `--raw`: Output the rich format as binary to stdout.
- `set <key> <value>`: Set the plain text value for the key.
- `rm <key>`: Remove the key.
    - `-r` / `--recursive`: Delete the key and all its children (keys matching `<key>` and `<key>/*`).
    - `--trash`: Force soft delete (move to Trash).
    - `--permanent`: Force immediate, permanent deletion.
- `mv <key> <new_key>`: Rename a key without modifying its value. Fails if `<new_key>` already exists unless `--force`
  is specified.
    - `--force`: Overwrite existing key at destination.
- `ls [prefix]`: List all keys matching the prefix (or all keys if no prefix given).
    - `--include-trash`: Include trashed items in results (hidden by default).
- `import <key>`: Import current clipboard content to the key.
    - When clipboard contains both files and text, **files take priority** (text is discarded).
- `copy <key>`: Copy the key's value to the clipboard. Also updates `last_accessed` timestamp.
- `gc`: Manually trigger garbage collection.

### Search Command

- `search <query>`: Search the database for keys matching the query.
    - `--fuzzy` (default): Use fuzzy matching.
    - `--regex`: Use regular expression matching.