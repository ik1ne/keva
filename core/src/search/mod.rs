//! Search module for fuzzy key search.
//!
//! Design:
//! - Two independent fuzzy indexes: Active and Trash.
//! - Each index is append-only (Nucleo has no deletions), so we track:
//!   - `injected_keys`: keys injected into Nucleo at least once
//!   - `tombstones`: keys to filter out from search results
//! - Search filters out stale Nucleo entries using `is_present(key)`.
//! - Heavy compaction/rebuild runs during periodic maintenance, not on every search.

use crate::types::Key;
use nucleo::{Config as NucleoConfig, Nucleo, Utf32String};
use std::collections::HashSet;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Query type for search.
///
/// Currently supports fuzzy search only.
#[derive(Debug, Clone)]
pub enum SearchQuery {
    Fuzzy(String),
}

/// Search results separated by lifecycle state.
#[derive(Debug, Clone)]
pub struct SearchResults {
    pub active: Vec<Key>,
    pub trashed: Vec<Key>,
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

/// Search error type.
///
/// Search is currently infallible, but we keep an explicit error type for API stability.
/// This is intentionally uninhabited (no variants).
#[derive(Debug, Error)]
pub enum SearchError {}

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
    fn new(initial: Vec<Key>, rebuild_threshold: usize) -> Self {
        let notify = Arc::new(|| {});
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
}

/// Search engine using two independent fuzzy indexes:
/// - Active index
/// - Trash index
pub(crate) struct SearchEngine {
    active: Index,
    trash: Index,
    config: SearchConfig,
}

impl SearchEngine {
    /// Creates a new search engine with initial keys.
    pub(crate) fn new(active: Vec<Key>, trashed: Vec<Key>, config: SearchConfig) -> Self {
        let rebuild_threshold = config.rebuild_threshold;

        Self {
            active: Index::new(active, rebuild_threshold),
            trash: Index::new(trashed, rebuild_threshold),
            config,
        }
    }

    /// Adds a key as active.
    pub(crate) fn add_active(&mut self, key: Key) {
        // Ensure it isn't considered present in trash.
        self.trash.remove(&key);
        self.active.insert(key);
    }

    /// Adds a key as trashed.
    #[allow(dead_code)] // Reserved for future GUI integration
    pub(crate) fn add_trashed(&mut self, key: Key) {
        self.active.remove(&key);
        self.trash.insert(key);
    }

    /// Moves a key from active to trashed.
    pub(crate) fn trash(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.insert(key.clone());
    }

    /// Moves a key from trashed to active.
    pub(crate) fn restore(&mut self, key: &Key) {
        self.trash.remove(key);
        self.active.insert(key.clone());
    }

    /// Removes a key from both indexes (purge).
    pub(crate) fn remove(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.remove(key);
    }

    /// Renames a key within whichever index it is currently present in.
    #[allow(dead_code)] // Reserved for future GUI integration
    pub(crate) fn rename(&mut self, old: &Key, new: Key) {
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

    /// Searches for keys matching the query.
    ///
    /// Returns results separated into active and trashed containers.
    /// Use `active_range` and `trashed_range` to limit/paginate results (e.g., `0..10` or `..` for all).
    pub(crate) fn search(
        &mut self,
        query: SearchQuery,
        timeout_ms: u64,
        active_range: impl RangeBounds<usize>,
        trashed_range: impl RangeBounds<usize>,
    ) -> Result<SearchResults, SearchError> {
        let active_bounds = Self::to_bounds(&active_range);
        let trashed_bounds = Self::to_bounds(&trashed_range);

        match query {
            SearchQuery::Fuzzy(pattern) => {
                Ok(self.search_fuzzy(&pattern, timeout_ms, active_bounds, trashed_bounds))
            }
        }
    }

    fn to_bounds(range: &impl RangeBounds<usize>) -> (usize, usize) {
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.saturating_add(1),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n.saturating_add(1),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => usize::MAX,
        };
        (start, end)
    }

    fn search_fuzzy(
        &mut self,
        pattern: &str,
        timeout_ms: u64,
        active_bounds: (usize, usize),
        trashed_bounds: (usize, usize),
    ) -> SearchResults {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);

        // Clone config so we don't hold an immutable borrow of `self` while mutably borrowing indexes.
        let config = self.config.clone();

        let active =
            Self::search_fuzzy_in_index(&config, &mut self.active, pattern, deadline, active_bounds);
        let trashed =
            Self::search_fuzzy_in_index(&config, &mut self.trash, pattern, deadline, trashed_bounds);

        SearchResults { active, trashed }
    }

    fn search_fuzzy_in_index(
        config: &SearchConfig,
        index: &mut Index,
        pattern: &str,
        deadline: Instant,
        bounds: (usize, usize),
    ) -> Vec<Key> {
        // Convert our CaseMatching to nucleo's.
        let case_matching = match config.case_matching {
            CaseMatching::Sensitive => nucleo::pattern::CaseMatching::Respect,
            CaseMatching::Insensitive => nucleo::pattern::CaseMatching::Ignore,
            CaseMatching::Smart => nucleo::pattern::CaseMatching::Smart,
        };

        let normalization = if config.unicode_normalization {
            nucleo::pattern::Normalization::Smart
        } else {
            nucleo::pattern::Normalization::Never
        };

        index
            .nucleo
            .pattern
            .reparse(0, pattern, case_matching, normalization, false);

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let status = index.nucleo.tick(remaining.as_millis() as u64);
            if !status.running {
                break;
            }
        }

        let snapshot = index.nucleo.snapshot();

        let mut results = Vec::new();
        let mut count = 0usize;
        let (skip, take) = (bounds.0, bounds.1.saturating_sub(bounds.0));

        for item in snapshot.matched_items(..) {
            let key: &Key = item.data;

            // Stale entry filter: only include if key is currently present in this index.
            if !index.is_present(key) {
                continue;
            }

            if count >= skip {
                results.push(key.clone());
                if results.len() >= take {
                    break;
                }
            }
            count += 1;
        }

        results
    }

    /// Performs search index maintenance.
    ///
    /// Intended to be called during `KevaCore::maintenance(...)` to avoid heavy work
    /// during active UI interactions.
    pub(crate) fn maintenance_compact(&mut self) {
        self.active.rebuild_if_needed();
        self.trash.rebuild_if_needed();
    }
}

#[cfg(test)]
mod tests;
