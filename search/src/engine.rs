//! Search engine with dual indexes for Active and Trash keys.

use crate::config::{CaseMatching, SearchConfig};
use crate::index::Index;
use crate::query::SearchQuery;
use crate::results::SearchResults;
use keva_core::types::Key;
use std::sync::Arc;

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
            .nucleo_mut()
            .pattern
            .reparse(0, pattern, case_matching, normalization, false);
        self.trash
            .nucleo_mut()
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
        let active_status = self.active.nucleo_mut().tick(0);
        let trash_status = self.trash.nucleo_mut().tick(0);

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
