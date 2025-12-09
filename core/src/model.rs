use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Plain text content
    Text(String),
    /// Small binary data stored directly in the database (< 1MB)
    BinaryEmbedded(Vec<u8>),
    /// Large binary data stored in valid external storage (> 1MB)
    BinaryBlob(PathBuf),
    /// Reference to an external file
    Link(PathBuf),
}
