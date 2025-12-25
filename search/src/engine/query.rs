//! Search query types.

/// Query type for search.
///
/// Currently supports fuzzy search only. Designed to be extensible
/// for future regex support.
#[derive(Debug, Clone)]
pub enum SearchQuery {
    /// Fuzzy matching search.
    Fuzzy(String),
}
