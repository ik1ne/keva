#[derive(Debug, Clone, Copy, Default)]
pub enum CaseMatching {
    Sensitive,
    Insensitive,
    /// Case-insensitive unless query contains uppercase.
    #[default]
    Smart,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub case_matching: CaseMatching,
    pub unicode_normalization: bool,
    pub rebuild_threshold: usize,
    pub active_result_limit: usize,
    pub trashed_result_limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_matching: CaseMatching::default(),
            unicode_normalization: true,
            rebuild_threshold: 100,
            active_result_limit: 100,
            trashed_result_limit: 20,
        }
    }
}
