//! Keva Core Library
//!
//! This is the core library for Keva, a local key-value store for structured data.
//! It provides storage, CRUD operations, lifecycle management, and search functionality.

pub mod config;
pub mod error;
pub mod model;
pub mod storage;
pub mod search;
pub mod clipboard;

pub use config::Config;
pub use error::{Error, Result};
pub use model::{Entry, Key, Lifecycle, RichFormat, Value};
pub use storage::Store;
pub use search::{SearchMode, SearchResult, SearchScope};
