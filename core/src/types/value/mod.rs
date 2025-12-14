pub(crate) mod versioned_value;

#[derive(Debug, Clone)]
pub struct Value {
    pub metadata: Metadata,
    pub clip_data: ClipData,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LifecycleState {
    Active,
    Trash,
    Purge,
}

#[derive(Debug, Clone)]
pub enum ClipData {
    /// Pure plaintext copy
    Text(String),
    /// File copy from file manager
    Files(Vec<FileData>),
}

#[derive(Debug, Clone)]
pub enum FileData {
    Inlined(InlineFileData),
    BlobStored(BlobStoredFileData),
}

#[derive(Debug, Clone)]
pub struct InlineFileData {
    pub file_name: String,
}

#[derive(Debug, Clone)]
pub struct BlobStoredFileData {
    pub file_name: String,
}
