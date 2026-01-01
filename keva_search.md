# keva_search Specification

## Overview

keva_search is the fuzzy search engine for Keva. It provides fast, non-blocking fuzzy matching over active and trashed
keys using Nucleo.

## Design Goals

1. **Non-blocking:** Search never blocks the UI thread
2. **Progressive:** Results appear incrementally as matching proceeds
3. **Stable:** Once display threshold is reached, results stop changing
4. **Separated:** Active and trashed keys searched independently

## Data Model

### SearchEngine

```rust
pub struct SearchEngine {
    active: Index,
    trash: Index,
    config: SearchConfig,
}
```

Main entry point. Owns two independent indexes for active and trashed keys.

### Index (internal)

```rust
struct Index {
    nucleo: Nucleo<Key>,
    injected_keys: HashSet<Key>,
    tombstones: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
    result_limit: usize,
    at_threshold: bool,
    current_pattern: String,
}
```

Wraps Nucleo with tombstone-based deletion, threshold tracking, and pattern caching for append optimization.

### SearchResults

```rust
pub struct SearchResults<'a> {
    snapshot: &'a Snapshot<Key>,
    tombstones: &'a HashSet<Key>,
}
```

Borrowed view of search results. Filters out tombstoned keys.

## Configuration

```rust
pub struct SearchConfig {
    pub case_matching: CaseMatching,
    pub unicode_normalization: bool,
    pub rebuild_threshold: usize,
    pub active_result_limit: usize,
    pub trashed_result_limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
            rebuild_threshold: 100,
            active_result_limit: 100,
            trashed_result_limit: 20,
        }
    }
}
```

### CaseMatching

```rust
pub enum CaseMatching {
    Sensitive,   // Always case-sensitive
    Insensitive, // Always case-insensitive
    Smart,       // Case-insensitive unless query contains uppercase
}
```

### SearchQuery

```rust
pub enum SearchQuery {
    Fuzzy(String),
}
```

## API

### Construction

```rust
impl SearchEngine {
    /// Creates a new search engine with initial keys
    pub fn new(
        active: Vec<Key>,
        trashed: Vec<Key>,
        config: SearchConfig,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self;
}
```

The `notify` callback is invoked by Nucleo's background worker when new results are ready. Typical usage: post a window
message to trigger UI update.

### Key Mutation

```rust
impl SearchEngine {
    /// Adds a key to active index (removes from trash if present)
    pub fn add_active(&mut self, key: Key);

    /// Moves a key from active to trash
    pub fn trash(&mut self, key: &Key);

    /// Moves a key from trash to active
    pub fn restore(&mut self, key: &Key);

    /// Removes a key from both indexes (purge)
    pub fn remove(&mut self, key: &Key);

    /// Renames a key within its current index
    pub fn rename(&mut self, old: &Key, new: Key);
}
```

### Search Operations

```rust
impl SearchEngine {
    /// Sets the search query, resets threshold state
    pub fn set_query(&mut self, query: SearchQuery);

    /// Drives search forward (non-blocking)
    /// Returns true if results changed
    pub fn tick(&mut self) -> bool;

    /// Returns true when both indexes hit their thresholds
    pub fn is_done(&self) -> bool;

    /// Returns active search results (limited by active_result_limit)
    pub fn active_results(&self) -> SearchResults<'_>;

    /// Returns trashed search results (limited by trashed_result_limit)
    pub fn trashed_results(&self) -> SearchResults<'_>;
}
```

### Maintenance

```rust
impl SearchEngine {
    /// Triggers index rebuild if pending deletions exceed threshold
    pub fn maintenance_compact(&mut self);
}
```

### SearchResults

```rust
impl<'a> SearchResults<'a> {
    /// Iterates over matched keys in score order
    pub fn iter(&self) -> impl Iterator<Item = &Key> + '_;
}
```

## Stop-at-Threshold Behavior

### Problem

Nucleo processes items in batches. Early ticks may show a key at rank 5, but later ticks push it to rank 150 as better
matches are found. If UI displays top 100, the key disappears mid-search.

### Solution

Each index stops ticking when its result limit is reached:

```rust
fn tick(&mut self) -> bool {
    if self.at_threshold {
        return false;  // Already stable, no changes
    }

    let status = self.nucleo.tick(0);  // Non-blocking

    // Count results excluding tombstones
    let result_count = self.nucleo.snapshot()
        .matched_items(..)
        .filter(|item| !self.tombstones.contains(item.data))
        .count();

    if result_count >= self.result_limit || !status.running {
        self.at_threshold = true;
    }

    true  // Results may have changed
}
```

### Behavior

| Event | Effect |
|-------|--------|
| `set_query()` | Resets `at_threshold` to false for both indexes |
| `tick()` when below threshold | Calls `nucleo.tick(0)`, results may change |
| `tick()` when at threshold | No-op, returns false |
| Results exceed limit in batch | Kept (overshoot allowed) |

### Rationale

- **Simpler than pinning:** No need to track selected item specially
- **Progressive results:** User sees results immediately, not after full search
- **Stability guarantee:** Once threshold hit, display won't shuffle
- **Clean separation:** Active and trashed thresholds independent

## Tombstone-Based Deletion

Nucleo is append-only. Deletions are handled via tombstones:

```rust
fn insert(&mut self, key: Key) {
    if self.injected_keys.insert(key.clone()) {
        // First time: inject into Nucleo
        self.nucleo.injector().push(key, ...);
    } else {
        // Already injected: revive by removing tombstone
        self.tombstones.remove(&key);
    }
}

fn remove(&mut self, key: &Key) {
    if self.injected_keys.contains(key) {
        self.tombstones.insert(key.clone());
        self.pending_deletions += 1;
    }
}
```

Results filter out tombstoned keys:

```rust
fn iter(&self) -> impl Iterator<Item = &Key> {
    self.snapshot
        .matched_items(..)
        .filter(|item| !self.tombstones.contains(item.data))
        .map(|item| item.data)
}
```

## Pattern Append Optimization

When the new search pattern extends the previous one (e.g., "fo" → "foo"), Nucleo can reuse previous matching work:

```rust
fn set_pattern(&mut self, pattern: &str, ...) {
    let append = !self.current_pattern.is_empty()
        && pattern.starts_with(&self.current_pattern);

    self.nucleo.pattern.reparse(0, pattern, case_matching, normalization, append);
    self.current_pattern = pattern.to_string();
    self.at_threshold = false;  // Reset threshold for new search
}
```

## Index Compaction

When `pending_deletions > rebuild_threshold`, the index can be rebuilt:

```rust
fn rebuild(&mut self) {
    self.nucleo.restart(true);

    // Re-inject only live keys
    let injector = self.nucleo.injector();
    for key in self.injected_keys.difference(&self.tombstones) {
        injector.push(key.clone(), ...);
    }

    // Clean up
    self.injected_keys.retain(|k| !self.tombstones.contains(k));
    self.tombstones.clear();
    self.pending_deletions = 0;
}
```

Triggered by `maintenance_compact()`, typically called during `keva_core::maintenance()`.

## Threading Model

```
                    ┌─────────────────────┐
                    │   Nucleo Workers    │
                    │  (background pool)  │
                    └──────────┬──────────┘
                               │ notify()
                               ▼
┌─────────────┐         ┌─────────────┐
│ Main Thread │◄────────│ WM_SEARCH   │
│             │         │ _READY      │
│ tick()      │         └─────────────┘
│ results()   │
└─────────────┘
```

- `SearchEngine` lives on main thread
- Nucleo spawns internal worker pool for matching
- `notify` callback fires from worker thread
- Main thread calls `tick()` to retrieve results
- All mutations (`add_active`, `trash`, etc.) happen on main thread

## Usage Pattern

```rust
// Initialization
let notify = Arc::new(|| PostMessageW(hwnd, WM_SEARCH_READY, ...));
let engine = SearchEngine::new(active_keys, trashed_keys, config, notify);

// On user input
engine.set_query(SearchQuery::Fuzzy(text));

// On WM_SEARCH_READY or timer
if engine.tick() {
    // Results changed, update UI
    let active = engine.active_results().iter().collect::<Vec<_>>();
    let trashed = engine.trashed_results().iter().collect::<Vec<_>>();
    update_key_list(active, trashed);
}

// Check if stable
if engine.is_done() {
    // Safe to rely on result order
}
```

## Empty Query Behavior

When query is empty:

- All non-tombstoned keys match
- Order is insertion order (not alphabetical)
- Threshold still applies (max 100 active, 20 trashed)
