mod index;
mod query;

use crate::config::{CaseMatching, SearchConfig};
use index::Index;
use keva_core::types::Key;
use nucleo::pattern::{CaseMatching as NucleoCaseMatching, Normalization};
use std::sync::Arc;

pub use index::SearchResults;
pub use query::SearchQuery;

pub struct SearchEngine {
    active: Index,
    trash: Index,
    config: SearchConfig,
}

impl SearchEngine {
    /// The `notify` callback is invoked by Nucleo's background worker when new results are ready.
    pub fn new(
        active: Vec<Key>,
        trashed: Vec<Key>,
        config: SearchConfig,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            active: Index::new(
                active,
                config.rebuild_threshold,
                config.active_result_limit,
                notify.clone(),
            ),
            trash: Index::new(
                trashed,
                config.rebuild_threshold,
                config.trashed_result_limit,
                notify,
            ),
            config,
        }
    }
}

/// Mutation operations.
impl SearchEngine {
    pub fn add_active(&mut self, key: Key) {
        self.trash.remove(&key);
        self.active.insert(key);
    }

    pub fn trash(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.insert(key.clone());
    }

    pub fn restore(&mut self, key: &Key) {
        self.trash.remove(key);
        self.active.insert(key.clone());
    }

    pub fn remove(&mut self, key: &Key) {
        self.active.remove(key);
        self.trash.remove(key);
    }

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
    pub fn set_query(&mut self, query: SearchQuery) {
        let SearchQuery::Fuzzy(ref pattern) = query;

        let case_matching = match self.config.case_matching {
            CaseMatching::Sensitive => NucleoCaseMatching::Respect,
            CaseMatching::Insensitive => NucleoCaseMatching::Ignore,
            CaseMatching::Smart => NucleoCaseMatching::Smart,
        };

        let normalization = if self.config.unicode_normalization {
            Normalization::Smart
        } else {
            Normalization::Never
        };

        self.active
            .set_pattern(pattern, case_matching, normalization);
        self.trash
            .set_pattern(pattern, case_matching, normalization);
    }

    /// Returns true if results may have changed.
    pub fn tick(&mut self) -> bool {
        let active_changed = self.active.tick();
        let trash_changed = self.trash.tick();
        active_changed || trash_changed
    }

    pub fn is_done(&self) -> bool {
        self.active.is_done() && self.trash.is_done()
    }

    pub fn active_results(&self) -> SearchResults<'_> {
        self.active.results()
    }

    pub fn trashed_results(&self) -> SearchResults<'_> {
        self.trash.results()
    }
}

/// Exact match operations (O(1) lookup).
impl SearchEngine {
    /// Returns true if the key exists in the active index.
    pub fn has_active(&self, key: &Key) -> bool {
        self.active.is_present(key)
    }

    /// Returns true if the key exists in the trash index.
    pub fn has_trashed(&self, key: &Key) -> bool {
        self.trash.is_present(key)
    }

    /// Returns true if the key exists in either index.
    pub fn has_key(&self, key: &Key) -> bool {
        self.has_active(key) || self.has_trashed(key)
    }
}

/// Maintenance operations.
impl SearchEngine {
    /// Triggers index rebuild if pending deletions exceed the threshold.
    pub fn maintenance_compact(&mut self) {
        self.active.rebuild_if_needed();
        self.trash.rebuild_if_needed();
    }
}
