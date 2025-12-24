//! Keva fuzzy search library.
//!
//! Provides non-blocking fuzzy search for Active and Trash key indexes.
//!
//! # Design
//!
//! - Two independent fuzzy indexes: Active and Trash.
//! - Each index is append-only (Nucleo has no deletions), so we track:
//!   - `injected_keys`: keys injected into Nucleo at least once
//!   - `tombstones`: keys to filter out from search results
//! - Search filters out stale Nucleo entries using tombstones.
//! - Heavy compaction/rebuild runs during periodic maintenance, not on every search.
//!
//! # Non-blocking API
//!
//! - `set_query()`: Sets the search pattern
//! - `tick()`: Drives search forward without blocking (calls nucleo.tick(0))
//! - `active_results()`, `trashed_results()`: Get search results

mod config;
mod engine;
mod index;
mod query;
mod results;

pub use config::{CaseMatching, SearchConfig};
pub use engine::SearchEngine;
pub use query::SearchQuery;
pub use results::SearchResults;

#[cfg(test)]
mod tests;
