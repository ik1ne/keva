#![allow(dead_code)]
//! Search module for fuzzy key search.
//!
//! Design:
//! - Two independent fuzzy indexes:
//!   - Active index (A)
//!   - Trash index (T)
//! - Each index is append-only (Nucleo has no deletions), so we track:
//!   - `all`: keys that have been injected at least once
//!   - `removed`: tombstones for keys that should be considered removed from this index
//! - Search filters out stale Nucleo entries using `is_present(key)`.
//! - Heavy compaction/rebuild is intended to run during periodic maintenance, not on every search.
//!
//! Note: This intentionally does NOT dedupe results per key. If Nucleo returns duplicate
//! entries for the same key, those may surface in results. This is acceptable for now pre-release.

use crate::types::Key;
use nucleo::{Config as NucleoConfig, Matcher, Nucleo, Utf32String};
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

/// Query type for search.
///
/// Currently supports fuzzy search only.
#[derive(Debug, Clone)]
pub enum SearchQuery {
    Fuzzy(String),
}

/// Search result with key, score, and trash status.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub key: Key,
    pub score: u32,
    pub is_trash: bool,
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
#[derive(Debug, Clone, Default)]
pub struct SearchConfig {
    pub case_matching: CaseMatching,
    pub unicode_normalization: bool,
}

impl SearchConfig {
    pub fn new() -> Self {
        Self {
            case_matching: CaseMatching::Smart,
            unicode_normalization: true,
        }
    }
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
    all: HashSet<Key>,
    removed: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
}

impl Index {
    fn new(initial: Vec<Key>, rebuild_threshold: usize) -> Self {
        // No-op notify callback (sync API).
        let notify = Arc::new(|| {});
        let nucleo = Nucleo::new(NucleoConfig::DEFAULT, notify, None, 1);

        let mut index = Self {
            nucleo,
            all: HashSet::new(),
            removed: HashSet::new(),
            pending_deletions: 0,
            rebuild_threshold,
        };

        for key in initial {
            index.insert(key);
        }

        index
    }

    fn is_present(&self, key: &Key) -> bool {
        self.all.contains(key) && !self.removed.contains(key)
    }

    fn insert(&mut self, key: Key) {
        if self.all.insert(key.clone()) {
            let injector = self.nucleo.injector();
            injector.push(key, |item, cols| {
                cols[0] = Utf32String::from(item.as_str());
            });
        } else {
            // Already injected at least once; just "revive" if previously removed.
            self.removed.remove(&key);
        }
    }

    fn remove(&mut self, key: &Key) {
        if !self.all.contains(key) {
            // Never injected into this index; nothing to do.
            return;
        }
        // Tombstone only once for accounting.
        if self.removed.insert(key.clone()) {
            self.pending_deletions += 1;
        }
    }

    fn rebuild_if_needed(&mut self) {
        if self.pending_deletions > self.rebuild_threshold {
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        self.nucleo.restart(true);
        self.pending_deletions = 0;

        let injector = self.nucleo.injector();
        for key in self.all.difference(&self.removed) {
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
        // Threshold can be tuned; default is conservative to avoid frequent rebuilds.
        let rebuild_threshold = 100;

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
    pub(crate) fn add_trashed(&mut self, key: Key) {
        // Ensure it isn't considered present in active.
        self.active.remove(&key);
        self.trash.insert(key);
    }

    /// Removes a key from both indexes (purge).
    pub(crate) fn remove(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.remove(key);
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

    /// Renames a key within whichever index it is currently present in.
    ///
    /// Note: KevaCore typically orchestrates rename as:
    /// - if overwrite: remove(dst) (both indices)
    /// - remove(src) from active (rename is active-only in KevaCore)
    /// - add_active(dst)
    ///
    /// This method is provided for convenience and correctness in cases where callers
    /// want the search engine to decide source bucket.
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

    /// Performs search index maintenance.
    ///
    /// Intended to be called during `KevaCore::maintenance(...)` to avoid heavy work
    /// during active UI interactions.
    pub(crate) fn maintenance_compact(&mut self) {
        self.active.rebuild_if_needed();
        self.trash.rebuild_if_needed();
    }

    /// Searches for keys matching the query.
    ///
    /// Returns active results first, then trash results.
    pub(crate) fn search(
        &mut self,
        query: SearchQuery,
        timeout_ms: u64,
    ) -> Result<Vec<SearchResult>, SearchError> {
        match query {
            SearchQuery::Fuzzy(pattern) => Ok(self.search_fuzzy(&pattern, timeout_ms)),
        }
    }

    fn search_fuzzy(&mut self, pattern: &str, timeout_ms: u64) -> Vec<SearchResult> {
        let mut out = Vec::new();

        // Clone config so we don't hold an immutable borrow of `self` while mutably borrowing indexes.
        let config = self.config.clone();

        // Active first.
        out.extend(Self::search_fuzzy_in_index(
            &config,
            &mut self.active,
            pattern,
            timeout_ms,
            false,
        ));
        // Trash last.
        out.extend(Self::search_fuzzy_in_index(
            &config,
            &mut self.trash,
            pattern,
            timeout_ms,
            true,
        ));

        out
    }

    fn search_fuzzy_in_index(
        config: &SearchConfig,
        index: &mut Index,
        pattern: &str,
        timeout_ms: u64,
        is_trash: bool,
    ) -> Vec<SearchResult> {
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
            let status = index.nucleo.tick(timeout_ms);
            if !status.running {
                break;
            }
        }

        let snapshot = index.nucleo.snapshot();
        let mut matcher = Matcher::new(NucleoConfig::DEFAULT);

        let mut results = Vec::new();
        for item in snapshot.matched_items(..) {
            let key: &Key = &item.data;

            // Stale entry filter: only include if key is currently present in this index.
            if !index.is_present(key) {
                continue;
            }

            let score = snapshot
                .pattern()
                .column_pattern(0)
                .score(item.matcher_columns[0].slice(..), &mut matcher)
                .unwrap_or(0);

            results.push(SearchResult {
                key: key.clone(),
                score,
                is_trash,
            });
        }

        results
    }
}

#[cfg(test)]
mod tests;
