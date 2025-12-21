//! Search module for fuzzy key search.
//!
//! Design:
//! - Two independent fuzzy indexes: Active and Trash.
//! - Each index is append-only (Nucleo has no deletions), so we track:
//!   - `injected_keys`: keys injected into Nucleo at least once
//!   - `tombstones`: keys to filter out from search results
//! - Search filters out stale Nucleo entries using `is_present(key)`.
//! - Heavy compaction/rebuild runs during periodic maintenance, not on every search.
//!
//! Non-blocking API:
//! - `set_query()`: Sets the search pattern
//! - `tick()`: Drives search forward without blocking (calls nucleo.tick(0))
//! - Query methods directly on SearchEngine (no intermediate state object)

use keva_core::types::Key;
use nucleo::{Config as NucleoConfig, Nucleo, Snapshot, Utf32String};
use std::collections::HashSet;
use std::sync::Arc;

/// Query type for search.
///
/// Currently supports fuzzy search only.
#[derive(Debug, Clone)]
pub enum SearchQuery {
    Fuzzy(String),
}

/// Case matching behavior for search.
#[derive(Debug, Clone, Copy, Default)]
pub enum CaseMatching {
    /// Always case sensitive.
    Sensitive,
    /// Always case insensitive.
    Insensitive,
    /// Smart case: case-insensitive unless query contains uppercase.
    #[default]
    Smart,
}

/// Configuration for search behavior.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub case_matching: CaseMatching,
    pub unicode_normalization: bool,
    /// Number of pending deletions before triggering index rebuild.
    pub rebuild_threshold: usize,
}

/// Search results snapshot that provides zero-copy iteration.
///
/// Borrows from the SearchEngine. Use `iter()` to iterate over
/// matched keys without collecting.
pub struct SearchResults<'a> {
    snapshot: &'a Snapshot<Key>,
    tombstones: &'a HashSet<Key>,
}

impl<'a> SearchResults<'a> {
    /// Iterates over matched keys, filtering out tombstoned entries.
    pub fn iter(&self) -> impl Iterator<Item = &Key> + '_ {
        self.snapshot
            .matched_items(..)
            .filter(|item| !self.tombstones.contains(item.data))
            .map(|item| item.data)
    }
}

/// Internal per-lifecycle fuzzy index.
struct Index {
    nucleo: Nucleo<Key>,
    /// Keys that have been injected into Nucleo at least once.
    injected_keys: HashSet<Key>,
    /// Keys marked as removed (tombstones) - filtered out from search results.
    tombstones: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
}

impl Index {
    fn new(
        initial: Vec<Key>,
        rebuild_threshold: usize,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let nucleo = Nucleo::new(NucleoConfig::DEFAULT, notify, None, 1);

        let mut index = Self {
            nucleo,
            injected_keys: HashSet::new(),
            tombstones: HashSet::new(),
            pending_deletions: 0,
            rebuild_threshold,
        };

        for key in initial {
            index.insert(key);
        }

        index
    }

    fn is_present(&self, key: &Key) -> bool {
        self.injected_keys.contains(key) && !self.tombstones.contains(key)
    }

    fn insert(&mut self, key: Key) {
        if self.injected_keys.insert(key.clone()) {
            let injector = self.nucleo.injector();
            injector.push(key, |item, cols| {
                cols[0] = Utf32String::from(item.as_str());
            });
        } else {
            // Already injected; revive if previously tombstoned.
            self.tombstones.remove(&key);
        }
    }

    fn remove(&mut self, key: &Key) {
        if !self.injected_keys.contains(key) {
            return;
        }
        if self.tombstones.insert(key.clone()) {
            self.pending_deletions += 1;
        }
    }

    fn rebuild_if_needed(&mut self) {
        if self.pending_deletions > self.rebuild_threshold {
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        // Compute surviving keys before clearing state.
        let surviving: HashSet<Key> = self
            .injected_keys
            .difference(&self.tombstones)
            .cloned()
            .collect();

        self.nucleo.restart(true);
        self.pending_deletions = 0;

        // Update tracking sets to reflect the new Nucleo state.
        // This ensures insert() works correctly for previously-tombstoned keys.
        self.injected_keys = surviving;
        self.tombstones.clear();

        let injector = self.nucleo.injector();
        for key in &self.injected_keys {
            let key_clone = key.clone();
            injector.push(key_clone, |item, cols| {
                cols[0] = Utf32String::from(item.as_str());
            });
        }
    }

    fn results(&self) -> SearchResults<'_> {
        SearchResults {
            snapshot: self.nucleo.snapshot(),
            tombstones: &self.tombstones,
        }
    }
}

/// Search engine using two independent fuzzy indexes:
/// - Active index
/// - Trash index
///
/// This is designed for non-blocking GUI integration. The notify callback
/// is invoked when Nucleo's background worker has new results ready.
pub struct SearchEngine {
    active: Index,
    trash: Index,
    config: SearchConfig,
    active_finished: bool,
    trashed_finished: bool,
}

/// Create operations.
impl SearchEngine {
    /// Creates a new search engine with initial keys and a notification callback.
    ///
    /// The `notify` callback is invoked by Nucleo's background worker when new
    /// results are ready. This is typically used to trigger a UI repaint.
    pub fn new(
        active: Vec<Key>,
        trashed: Vec<Key>,
        config: SearchConfig,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let rebuild_threshold = config.rebuild_threshold;

        Self {
            active: Index::new(active, rebuild_threshold, notify.clone()),
            trash: Index::new(trashed, rebuild_threshold, notify),
            config,
            active_finished: true,
            trashed_finished: true,
        }
    }
}

/// Mutation operations.
impl SearchEngine {
    /// Adds a key as active.
    pub fn add_active(&mut self, key: Key) {
        // Ensure it isn't considered present in trash.
        self.trash.remove(&key);
        self.active.insert(key);
    }

    /// Moves a key from active to trashed.
    pub fn trash(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.insert(key.clone());
    }

    /// Moves a key from trashed to active.
    pub fn restore(&mut self, key: &Key) {
        self.trash.remove(key);
        self.active.insert(key.clone());
    }

    /// Removes a key from both indexes (purge).
    pub fn remove(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.remove(key);
    }

    /// Renames a key within whichever index it is currently present in.
    pub fn rename(&mut self, old: &Key, new: Key) {
        if self.active.is_present(old) {
            self.active.remove(old);
            self.active.insert(new);
            return;
        }
        if self.trash.is_present(old) {
            self.trash.remove(old);
            self.trash.insert(new);
        }
    }
}

/// Search operations.
impl SearchEngine {
    /// Sets the search pattern.
    ///
    /// This reconfigures the Nucleo pattern for both indexes. The search runs
    /// asynchronously on Nucleo's background threadpool. Call `tick()` to
    /// drive the search forward.
    pub fn set_query(&mut self, query: SearchQuery) {
        let SearchQuery::Fuzzy(ref pattern) = query;

        let case_matching = match self.config.case_matching {
            CaseMatching::Sensitive => nucleo::pattern::CaseMatching::Respect,
            CaseMatching::Insensitive => nucleo::pattern::CaseMatching::Ignore,
            CaseMatching::Smart => nucleo::pattern::CaseMatching::Smart,
        };

        let normalization = if self.config.unicode_normalization {
            nucleo::pattern::Normalization::Smart
        } else {
            nucleo::pattern::Normalization::Never
        };

        self.active
            .nucleo
            .pattern
            .reparse(0, pattern, case_matching, normalization, false);
        self.trash
            .nucleo
            .pattern
            .reparse(0, pattern, case_matching, normalization, false);

        self.active_finished = false;
        self.trashed_finished = false;
    }

    /// Drives the search forward without blocking.
    ///
    /// This calls `nucleo.tick(0)` on both indexes, which returns immediately.
    /// Call this from the GUI event loop (e.g., after receiving a notify callback
    /// or on each frame while `!is_finished()`).
    pub fn tick(&mut self) {
        let active_status = self.active.nucleo.tick(0);
        let trash_status = self.trash.nucleo.tick(0);

        self.active_finished = !active_status.running;
        self.trashed_finished = !trash_status.running;
    }

    /// Returns true if both indexes have finished searching.
    pub fn is_finished(&self) -> bool {
        self.active_finished && self.trashed_finished
    }

    /// Returns active search results for zero-copy iteration.
    pub fn active_results(&self) -> SearchResults<'_> {
        self.active.results()
    }

    /// Returns trashed search results for zero-copy iteration.
    pub fn trashed_results(&self) -> SearchResults<'_> {
        self.trash.results()
    }
}

/// Maintenance operations.
impl SearchEngine {
    /// Performs search index maintenance.
    ///
    /// Triggers index rebuild if pending deletions exceed the threshold.
    /// Call this during `KevaCore::maintenance(...)` to avoid heavy work
    /// during active UI interactions.
    pub fn maintenance_compact(&mut self) {
        self.active.rebuild_if_needed();
        self.trash.rebuild_if_needed();
    }
}

#[cfg(test)]
mod tests;
