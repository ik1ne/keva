//! Search configuration types.

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
    /// How to match case in search queries.
    pub case_matching: CaseMatching,
    /// Whether to apply Unicode normalization.
    pub unicode_normalization: bool,
    /// Number of pending deletions before triggering index rebuild.
    pub rebuild_threshold: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_matching: CaseMatching::default(),
            unicode_normalization: true,
            rebuild_threshold: 100,
        }
    }
}
