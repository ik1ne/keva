use keva_core::types::Key;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config as NucleoConfig, Nucleo, Snapshot, Utf32String};
use std::collections::HashSet;
use std::sync::Arc;

/// Wraps Nucleo with tombstone-based deletion and threshold tracking.
pub(crate) struct Index {
    nucleo: Nucleo<Key>,
    injected_keys: HashSet<Key>,
    tombstones: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
    result_limit: usize,
    at_threshold: bool,
    current_pattern: String,
    /// True when current query uses append optimization (count may be stale until done).
    is_appending: bool,
}

impl Index {
    pub(crate) fn new(
        initial: Vec<Key>,
        rebuild_threshold: usize,
        result_limit: usize,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let nucleo = Nucleo::new(NucleoConfig::DEFAULT, notify, None, 1);

        let mut index = Self {
            nucleo,
            injected_keys: HashSet::new(),
            tombstones: HashSet::new(),
            pending_deletions: 0,
            rebuild_threshold,
            result_limit,
            at_threshold: false,
            current_pattern: String::new(),
            is_appending: false,
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

    /// Uses append optimization if pattern extends the previous one.
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
        self.at_threshold = false;
        self.is_appending = append;
    }

    /// Returns true if results may have changed and we should send updates.
    pub(crate) fn tick(&mut self) -> bool {
        if self.at_threshold {
            return false;
        }

        let status = self.nucleo.tick(0);

        // With append optimization, the count includes stale matches until filtering completes.
        // Only use count threshold when not appending (fresh search) or when nucleo is done.
        let count_reliable = !self.is_appending || !status.running;
        if count_reliable {
            let result_count = self.nucleo.snapshot().matched_item_count();
            if result_count >= self.result_limit as u32 {
                self.at_threshold = true;
                return true;
            }
        }

        if !status.running {
            self.at_threshold = true;
        }

        true
    }

    pub(crate) fn is_done(&self) -> bool {
        self.at_threshold
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
            result_limit: self.result_limit,
        }
    }
}

pub struct SearchResults<'a> {
    pub(crate) snapshot: &'a Snapshot<Key>,
    pub(crate) tombstones: &'a HashSet<Key>,
    pub(crate) result_limit: usize,
}

impl<'a> SearchResults<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &Key> + '_ {
        self.snapshot
            .matched_items(..)
            .filter(|item| !self.tombstones.contains(item.data))
            .map(|item| item.data)
            .take(self.result_limit)
    }
}
