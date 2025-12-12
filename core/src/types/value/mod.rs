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
pub struct ClipData {
    pub plain_text: Option<String>,
    /// Currently len is 0..=1 but might be extended in the future
    pub rich_data: Vec<RichData>,
}

#[derive(Debug, Clone)]
pub enum RichData {
    Files(Vec<FileData>),
    // Image(Vec<u8>),
    // Html(String),
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
