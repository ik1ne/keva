//! Internal per-lifecycle fuzzy index.

use crate::results::SearchResults;
use keva_core::types::Key;
use nucleo::{Config as NucleoConfig, Nucleo, Utf32String};
use std::collections::HashSet;
use std::sync::Arc;

/// Internal per-lifecycle fuzzy index.
///
/// Wraps nucleo with tombstone-based deletion tracking.
/// Nucleo is append-only, so we track:
/// - `injected_keys`: keys injected into Nucleo at least once
/// - `tombstones`: keys to filter out from search results
pub(crate) struct Index {
    nucleo: Nucleo<Key>,
    /// Keys that have been injected into Nucleo at least once.
    injected_keys: HashSet<Key>,
    /// Keys marked as removed (tombstones) - filtered out from search results.
    tombstones: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
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

    pub(crate) fn rebuild_if_needed(&mut self) {
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

    pub(crate) fn results(&self) -> SearchResults<'_> {
        SearchResults {
            snapshot: self.nucleo.snapshot(),
            tombstones: &self.tombstones,
        }
    }

    pub(crate) fn nucleo_mut(&mut self) -> &mut Nucleo<Key> {
        &mut self.nucleo
    }
}
