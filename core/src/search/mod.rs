//! Search functionality for Keva
//!
//! Supports both fuzzy matching (using nucleo) and regex matching.
//! Automatically detects which mode to use based on query content.

mod fuzzy;
mod engine;

pub use engine::SearchEngine;
pub use fuzzy::FuzzyMatcher;

use crate::model::{Key, Lifecycle};

/// Search scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScope {
    /// Search only key paths
    #[default]
    Keys,
    /// Search key paths and value content
    KeysAndContent,
}

/// Search mode (auto-detected from query)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Fuzzy matching (default for alphanumeric queries)
    Fuzzy,
    /// Regex matching (when query contains regex symbols)
    Regex,
}

impl SearchMode {
    /// Detect the appropriate search mode from a query
    pub fn detect(query: &str) -> Self {
        // Regex symbols that trigger regex mode
        const REGEX_CHARS: &[char] = &['*', '?', '^', '$', '[', ']', '(', ')', '{', '}', '|', '+', '\\'];

        if query.chars().any(|c| REGEX_CHARS.contains(&c)) {
            SearchMode::Regex
        } else {
            SearchMode::Fuzzy
        }
    }

    /// Get the icon for this search mode
    pub fn icon(&self) -> &'static str {
        match self {
            SearchMode::Fuzzy => "🧲",
            SearchMode::Regex => ".*",
        }
    }
}

/// A search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching key
    pub key: Key,
    /// Search score (higher is better)
    pub score: u32,
    /// Whether this item is in trash
    pub is_trash: bool,
    /// The lifecycle state
    pub lifecycle: Lifecycle,
    /// Matched indices in the key (for highlighting)
    pub matched_indices: Vec<usize>,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(key: Key, score: u32, lifecycle: Lifecycle) -> Self {
        Self {
            key,
            score,
            is_trash: lifecycle == Lifecycle::Trash,
            lifecycle,
            matched_indices: Vec::new(),
        }
    }

    /// Set the matched indices
    pub fn with_indices(mut self, indices: Vec<usize>) -> Self {
        self.matched_indices = indices;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_mode_detection() {
        assert_eq!(SearchMode::detect("hello"), SearchMode::Fuzzy);
        assert_eq!(SearchMode::detect("hello-world"), SearchMode::Fuzzy);
        assert_eq!(SearchMode::detect("path/to/key"), SearchMode::Fuzzy);
        assert_eq!(SearchMode::detect("hello_world"), SearchMode::Fuzzy);
        assert_eq!(SearchMode::detect("test.txt"), SearchMode::Fuzzy);

        assert_eq!(SearchMode::detect("hello*"), SearchMode::Regex);
        assert_eq!(SearchMode::detect("^start"), SearchMode::Regex);
        assert_eq!(SearchMode::detect("end$"), SearchMode::Regex);
        assert_eq!(SearchMode::detect("[a-z]"), SearchMode::Regex);
        assert_eq!(SearchMode::detect("a|b"), SearchMode::Regex);
        assert_eq!(SearchMode::detect("a+"), SearchMode::Regex);
    }
}
