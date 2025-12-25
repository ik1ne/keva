//! Internal per-lifecycle fuzzy index.

use keva_core::types::Key;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config as NucleoConfig, Nucleo, Snapshot, Utf32String};
use std::collections::HashSet;
use std::sync::Arc;

/// Internal per-lifecycle fuzzy index.
///
/// Wraps nucleo with tombstone-based deletion tracking and query caching.
/// Nucleo is append-only, so we track:
/// - `injected_keys`: keys injected into Nucleo at least once
/// - `tombstones`: keys to filter out from search results
/// - `current_pattern`: cached for append optimization
pub(crate) struct Index {
    nucleo: Nucleo<Key>,
    /// Keys that have been injected into Nucleo at least once.
    injected_keys: HashSet<Key>,
    /// Keys marked as removed (tombstones) - filtered out from search results.
    tombstones: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
    /// Cached pattern for append optimization.
    current_pattern: String,
}

impl Index {
    pub(crate) fn new(
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
            current_pattern: String::new(),
        };

        for key in initial {
            index.insert(key);
        }

        index
    }

    pub(crate) fn is_present(&self, key: &Key) -> bool {
        self.injected_keys.contains(key) && !self.tombstones.contains(key)
    }

    pub(crate) fn insert(&mut self, key: Key) {
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

    pub(crate) fn remove(&mut self, key: &Key) {
        if !self.injected_keys.contains(key) {
            return;
        }
        if self.tombstones.insert(key.clone()) {
            self.pending_deletions += 1;
        }
    }

    /// Sets the search pattern with automatic append detection.
    ///
    /// If the new pattern extends the current one (e.g., "fo" -> "foo"),
    /// nucleo can reuse previous matching work for better performance.
    pub(crate) fn set_pattern(
        &mut self,
        pattern: &str,
        case_matching: CaseMatching,
        normalization: Normalization,
    ) {
        let append = !self.current_pattern.is_empty() && pattern.starts_with(&self.current_pattern);

        self.nucleo
            .pattern
            .reparse(0, pattern, case_matching, normalization, append);
        self.current_pattern = pattern.to_string();
    }

    /// Drives the search forward without blocking.
    ///
    /// Returns true if the search has finished.
    pub(crate) fn tick(&mut self) -> bool {
        let status = self.nucleo.tick(0);
        !status.running
    }

    pub(crate) fn rebuild_if_needed(&mut self) {
        if self.pending_deletions > self.rebuild_threshold {
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        self.nucleo.restart(true);
        self.pending_deletions = 0;
        self.current_pattern.clear();

        let injector = self.nucleo.injector();
        for key in self.injected_keys.difference(&self.tombstones) {
            let key_clone = key.clone();
            injector.push(key_clone, |item, cols| {
                cols[0] = Utf32String::from(item.as_str());
            });
        }

        self.injected_keys.retain(|k| !self.tombstones.contains(k));
        self.tombstones.clear();
        self.current_pattern.clear();
    }

    pub(crate) fn results(&self) -> SearchResults<'_> {
        SearchResults {
            snapshot: self.nucleo.snapshot(),
            tombstones: &self.tombstones,
        }
    }
}

/// Search results snapshot that provides zero-copy iteration.
///
/// Borrows from the SearchEngine. Use `iter()` to iterate over
/// matched keys without collecting.
pub struct SearchResults<'a> {
    pub(crate) snapshot: &'a Snapshot<Key>,
    pub(crate) tombstones: &'a HashSet<Key>,
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
