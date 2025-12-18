pub(crate) mod clipboard;
pub mod core;
pub(crate) mod search;
pub mod types;

pub use search::{CaseMatching, SearchConfig, SearchError, SearchQuery, SearchResult};
