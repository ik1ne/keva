use crate::types::value::versioned_value::ValueVariant;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    pub metadata: Metadata,
    pub clip_data: ClipData,
}

impl ValueVariant for Value {
    const VERSION: u8 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub trashed_at: Option<SystemTime>,
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LifecycleState {
    Active,
    Trash,
    Purge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipData {
    pub plain_text: Option<TextData>,
    /// Currently len is 0..=1 but might be extended in the future
    pub rich_data: Vec<RichData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextData {
    Inlined(String),
    BlobStored(FileHash),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RichData {
    Files(Vec<FileData>),
    // Image(Vec<u8>),
    // Html(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileData {
    Inlined(InlineFileData),
    BlobStored(BlobStoredFileData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineFileData {
    pub file_name: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStoredFileData {
    pub file_name: String,
    pub hash: FileHash,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileHash {
    Blake3([u8; 32]),
}
