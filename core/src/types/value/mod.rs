//! Public value types for consumers.
//!
//! The `PublicValue` type is exported as `Value` from `keva_core::types`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) mod versioned_value;

use versioned_value::latest_value;

/// A value retrieved from storage, ready for consumption.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct PublicValue {
    pub metadata: Metadata,
    pub clip_data: ClipData,
}

/// Text file name for blob-stored text.
const TEXT_FILE_NAME: &str = "text.txt";

impl PublicValue {
    /// Converts a v1::Value to PublicValue with resolved paths.
    ///
    /// Returns `None` if the value is in Purge state (should not be visible).
    pub(crate) fn from_latest_value(
        value: latest_value::Value,
        blob_base: &Path,
        key_path: &Path,
    ) -> Option<Self> {
        if value.metadata.lifecycle_state == latest_value::LifecycleState::Purge {
            return None;
        }

        let metadata = Metadata {
            created_at: value.metadata.created_at,
            updated_at: value.metadata.updated_at,
            last_accessed: value.metadata.last_accessed,
            trashed_at: value.metadata.trashed_at,
            lifecycle_state: match value.metadata.lifecycle_state {
                latest_value::LifecycleState::Active => LifecycleState::Active,
                latest_value::LifecycleState::Trash => LifecycleState::Trash,
                latest_value::LifecycleState::Purge => unreachable!(),
            },
        };

        let clip_data = match value.clip_data {
            latest_value::ClipData::Text(latest_value::TextData::Inlined(s)) => {
                ClipData::Text(TextContent::Inlined(s))
            }
            latest_value::ClipData::Text(latest_value::TextData::BlobStored) => {
                ClipData::Text(TextContent::BlobStored {
                    path: blob_base.join(key_path).join(TEXT_FILE_NAME),
                })
            }
            latest_value::ClipData::Files(files) => ClipData::Files(
                files
                    .into_iter()
                    .map(|f| match f {
                        latest_value::FileData::Inlined(latest_value::InlineFileData {
                            file_name,
                            data,
                        }) => FileContent::Inlined(InlinedFile { file_name, data }),
                        latest_value::FileData::BlobStored(latest_value::BlobStoredFileData {
                            file_name,
                            hash,
                        }) => FileContent::BlobStored(BlobStoredFile {
                            path: blob_base
                                .join(key_path)
                                .join(hash.to_string())
                                .join(&file_name),
                            file_name,
                        }),
                    })
                    .collect(),
            ),
        };

        Some(Self {
            metadata,
            clip_data,
        })
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct Metadata {
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub last_accessed: SystemTime,
    pub trashed_at: Option<SystemTime>,
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LifecycleState {
    Active,
    Trash,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub enum ClipData {
    Text(TextContent),
    Files(Vec<FileContent>),
}

/// Text content with storage location indicator.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub enum TextContent {
    /// Text stored inline. String available directly.
    Inlined(String),
    /// Text stored on disk. Path provided for reading.
    BlobStored { path: PathBuf },
}

/// File content with storage location indicator.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub enum FileContent {
    /// File stored inline. Use `KevaCore::ensure_file_paths()` to get path.
    Inlined(InlinedFile),
    /// File stored on disk. Path provided for access.
    BlobStored(BlobStoredFile),
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct InlinedFile {
    pub file_name: String,
    pub data: Vec<u8>,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct BlobStoredFile {
    pub file_name: String,
    pub path: PathBuf,
}
