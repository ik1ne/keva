pub(crate) mod clipboard;
pub mod core;
#[cfg(feature = "search")]
pub(crate) mod search;
pub mod types;

#[cfg(feature = "search")]
pub use search::{CaseMatching, SearchConfig, SearchError, SearchQuery, SearchResults};
