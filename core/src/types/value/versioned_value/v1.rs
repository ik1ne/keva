use crate::types::value::versioned_value::ValueVariant;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    pub metadata: Metadata,
    pub clip_data: ClipData,
}

impl ValueVariant for Value {
    const VERSION: u8 = 1;
    type Hasher = blake3_v1::Hasher;
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub last_accessed: SystemTime,
    pub trashed_at: Option<SystemTime>,
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LifecycleState {
    Active,
    Trash,
    Purge,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipData {
    /// Pure plaintext copy
    Text(TextData),
    /// File copy from file manager
    Files(Vec<FileData>),
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextData {
    Inlined(String),
    BlobStored,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileData {
    Inlined(InlineFileData),
    BlobStored(BlobStoredFileData),
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineFileData {
    pub file_name: String,
    pub data: Vec<u8>,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStoredFileData {
    pub file_name: String,
    pub hash: blake3_v1::Hash,
}
