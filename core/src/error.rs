//! Error types for Keva

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for Keva operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during Keva operations
#[derive(Debug, Error)]
pub enum Error {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Key already exists: {0}")]
    KeyExists(String),

    #[error("Invalid key path: {0}")]
    InvalidKey(String),

    #[error("Storage error: {0}")]
    Storage(#[from] redb::Error),

    #[error("Database error: {0}")]
    Database(#[from] redb::DatabaseError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),

    #[error("Table error: {0}")]
    Table(#[from] redb::TableError),

    #[error("Commit error: {0}")]
    Commit(#[from] redb::CommitError),

    #[error("Storage corruption error: {0}")]
    StorageCorruption(#[from] redb::StorageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Search index error: {0}")]
    SearchIndex(#[from] tantivy::TantivyError),

    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),

    #[error("Blob not found: {0}")]
    BlobNotFound(String),

    #[error("File too large: {path} is {size} bytes, threshold is {threshold} bytes")]
    FileTooLarge {
        path: PathBuf,
        size: u64,
        threshold: u64,
    },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Item is in trash: {0}")]
    InTrash(String),

    #[error("Item is purged: {0}")]
    Purged(String),
}
