//! Search module for fuzzy search.
//!
//! Uses nucleo for fuzzy search.
//! SearchEngine maintains incremental updates to avoid rebuilding on every mutation.

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

/// Search result with key, score, and trash status
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub key: Key,
    pub score: u32,
    pub is_trash: bool,
}

/// Case matching behavior for search
#[derive(Debug, Clone, Copy, Default)]
pub enum CaseMatching {
    /// Always case sensitive
    Sensitive,
    /// Always case insensitive
    Insensitive,
    /// Smart case: case-insensitive unless query contains uppercase
    #[default]
    Smart,
}

/// Configuration for search behavior
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

/// Item stored in Nucleo index
struct SearchItem {
    key: Key,
    is_trash: bool,
}

/// Search engine with incremental updates
pub(crate) struct SearchEngine {
    nucleo: Nucleo<SearchItem>,
    active_keys: HashSet<Key>,
    trashed_keys: HashSet<Key>,
    pending_deletions: usize,
    rebuild_threshold: usize,
    config: SearchConfig,
}

impl SearchEngine {
    /// Creates a new search engine with initial keys
    pub(crate) fn new(active: Vec<Key>, trashed: Vec<Key>, config: SearchConfig) -> Self {
        // No-op notify callback (we use sync API)
        let notify = Arc::new(|| {});

        let nucleo_config = NucleoConfig::DEFAULT;
        let nucleo = Nucleo::new(nucleo_config, notify, None, 1);

        let active_keys: HashSet<Key> = active.into_iter().collect();
        let trashed_keys: HashSet<Key> = trashed.into_iter().collect();

        // Inject all keys
        let injector = nucleo.injector();
        for key in &active_keys {
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: false,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }
        for key in &trashed_keys {
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: true,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }

        Self {
            nucleo,
            active_keys,
            trashed_keys,
            pending_deletions: 0,
            rebuild_threshold: 100,
            config,
        }
    }

    /// Adds a key as active
    pub(crate) fn add_active(&mut self, key: Key) {
        if self.active_keys.insert(key.clone()) {
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key,
                    is_trash: false,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }
    }

    /// Adds a key as trashed
    pub(crate) fn add_trashed(&mut self, key: Key) {
        if self.trashed_keys.insert(key.clone()) {
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key,
                    is_trash: true,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }
    }

    /// Removes a key from the index
    pub(crate) fn remove(&mut self, key: &Key) {
        let removed = self.active_keys.remove(key) || self.trashed_keys.remove(key);
        if removed {
            self.pending_deletions += 1;
            self.rebuild_if_needed();
        }
    }

    /// Moves a key from active to trashed
    pub(crate) fn trash(&mut self, key: &Key) {
        if self.active_keys.remove(key) {
            self.trashed_keys.insert(key.clone());
            self.pending_deletions += 1;
            // Re-inject with is_trash: true
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: true,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
            self.rebuild_if_needed();
        }
    }

    /// Moves a key from trashed to active
    pub(crate) fn restore(&mut self, key: &Key) {
        if self.trashed_keys.remove(key) {
            self.active_keys.insert(key.clone());
            self.pending_deletions += 1;
            // Re-inject with is_trash: false
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: false,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
            self.rebuild_if_needed();
        }
    }

    /// Renames a key
    pub(crate) fn rename(&mut self, old: &Key, new: Key) {
        let was_active = self.active_keys.remove(old);
        let was_trashed = self.trashed_keys.remove(old);

        if was_active {
            self.active_keys.insert(new.clone());
            self.pending_deletions += 1;
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key: new,
                    is_trash: false,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
            self.rebuild_if_needed();
        } else if was_trashed {
            self.trashed_keys.insert(new.clone());
            self.pending_deletions += 1;
            let injector = self.nucleo.injector();
            injector.push(
                SearchItem {
                    key: new,
                    is_trash: true,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
            self.rebuild_if_needed();
        }
    }

    /// Rebuilds the index if pending deletions exceed threshold
    fn rebuild_if_needed(&mut self) {
        if self.pending_deletions > self.rebuild_threshold {
            self.rebuild();
        }
    }

    /// Rebuilds the entire index from scratch
    fn rebuild(&mut self) {
        self.nucleo.restart(true);
        self.pending_deletions = 0;

        let injector = self.nucleo.injector();
        for key in &self.active_keys {
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: false,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }
        for key in &self.trashed_keys {
            injector.push(
                SearchItem {
                    key: key.clone(),
                    is_trash: true,
                },
                |item, cols| {
                    cols[0] = Utf32String::from(item.key.as_str());
                },
            );
        }
    }

    /// Searches for keys matching the query.
    pub(crate) fn search(
        &mut self,
        query: SearchQuery,
        timeout_ms: u64,
    ) -> Result<Vec<SearchResult>, SearchError> {
        match query {
            SearchQuery::Fuzzy(pattern) => self.search_fuzzy(&pattern, timeout_ms),
        }
    }

    fn search_fuzzy(
        &mut self,
        pattern: &str,
        timeout_ms: u64,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Convert our CaseMatching to nucleo's
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

        // Update the pattern
        self.nucleo
            .pattern
            .reparse(0, pattern, case_matching, normalization, false);

        // Process until done
        loop {
            let status = self.nucleo.tick(timeout_ms);
            if !status.running {
                break;
            }
        }

        // Collect results, filtering out deleted items
        let snapshot = self.nucleo.snapshot();
        let mut results = Vec::new();
        let mut matcher = Matcher::new(NucleoConfig::DEFAULT);

        for item in snapshot.matched_items(..) {
            let key = &item.data.key;
            let is_trash = item.data.is_trash;

            // Filter: only include if key is still in the appropriate set
            let is_valid = if is_trash {
                self.trashed_keys.contains(key)
            } else {
                self.active_keys.contains(key)
            };

            if is_valid {
                // Get score from the match
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
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(s: &str) -> Key {
        Key::try_new(s.to_string()).unwrap()
    }

    #[test]
    fn test_search_engine_new() {
        let active = vec![make_key("foo"), make_key("bar")];
        let trashed = vec![make_key("baz")];
        let config = SearchConfig::default();

        let engine = SearchEngine::new(active, trashed, config);

        assert_eq!(engine.active_keys.len(), 2);
        assert_eq!(engine.trashed_keys.len(), 1);
    }

    #[test]
    fn test_fuzzy_search_basic() {
        let active = vec![make_key("hello"), make_key("world"), make_key("help")];
        let trashed = vec![];
        let config = SearchConfig::default();

        let mut engine = SearchEngine::new(active, trashed, config);

        let results = engine
            .search(SearchQuery::Fuzzy("hel".to_string()), 100)
            .unwrap();

        // Should match "hello" and "help"
        let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"hello"));
        assert!(keys.contains(&"help"));
        assert!(!keys.contains(&"world"));
    }

    #[test]
    fn test_add_and_remove() {
        let active = vec![make_key("foo")];
        let trashed = vec![];
        let config = SearchConfig::default();

        let mut engine = SearchEngine::new(active, trashed, config);

        // Add a new key
        engine.add_active(make_key("bar"));
        assert!(engine.active_keys.contains(&make_key("bar")));

        // Remove a key
        engine.remove(&make_key("foo"));
        assert!(!engine.active_keys.contains(&make_key("foo")));
    }

    #[test]
    fn test_trash_and_restore() {
        let active = vec![make_key("foo")];
        let trashed = vec![];
        let config = SearchConfig::default();

        let mut engine = SearchEngine::new(active, trashed, config);

        // Trash the key
        engine.trash(&make_key("foo"));
        assert!(!engine.active_keys.contains(&make_key("foo")));
        assert!(engine.trashed_keys.contains(&make_key("foo")));

        // Restore the key
        engine.restore(&make_key("foo"));
        assert!(engine.active_keys.contains(&make_key("foo")));
        assert!(!engine.trashed_keys.contains(&make_key("foo")));
    }

    #[test]
    fn test_rename() {
        let active = vec![make_key("old")];
        let trashed = vec![];
        let config = SearchConfig::default();

        let mut engine = SearchEngine::new(active, trashed, config);

        engine.rename(&make_key("old"), make_key("new"));
        assert!(!engine.active_keys.contains(&make_key("old")));
        assert!(engine.active_keys.contains(&make_key("new")));
    }

    #[test]
    fn test_search_includes_trash_status() {
        let active = vec![make_key("active_key")];
        let trashed = vec![make_key("trashed_key")];
        let config = SearchConfig::default();

        let mut engine = SearchEngine::new(active, trashed, config);

        let results = engine
            .search(SearchQuery::Fuzzy("key".to_string()), 100)
            .unwrap();

        let active_result = results.iter().find(|r| r.key.as_str() == "active_key");
        let trashed_result = results.iter().find(|r| r.key.as_str() == "trashed_key");

        assert!(active_result.is_some());
        assert!(!active_result.unwrap().is_trash);

        assert!(trashed_result.is_some());
        assert!(trashed_result.unwrap().is_trash);
    }
}
