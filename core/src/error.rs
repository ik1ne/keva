use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("clipboard error: {0}")]
    Clipboard(#[from] ClipboardError),

    #[error("validation error: {0}")]
    Validation(#[from] ValidationError),
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(String),

    #[error("blob not found: {0}")]
    BlobNotFound(String),

    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("key already exists: {0}")]
    KeyAlreadyExists(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("clipboard access failed: {0}")]
    Access(String),

    #[error("unsupported format")]
    UnsupportedFormat,
}

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("value too large: {size} bytes exceeds {max} bytes")]
    ValueTooLarge { size: u64, max: u64 },
}
