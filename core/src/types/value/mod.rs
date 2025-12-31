//! Public value types for consumers.
//!
//! The `PublicValue` type is exported as `Value` from `keva_core::types`.

use std::time::SystemTime;

pub(crate) mod versioned_value;

use versioned_value::latest_value;

/// A value retrieved from storage, ready for consumption.
#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct PublicValue {
    pub metadata: Metadata,
    pub attachments: Vec<Attachment>,
    pub thumb_version: u32,
}

impl PublicValue {
    /// Converts internal Value to PublicValue.
    pub(crate) fn from_latest_value(value: latest_value::Value) -> Self {
        let metadata = Metadata {
            created_at: value.metadata.created_at,
            updated_at: value.metadata.updated_at,
            lifecycle_state: match value.metadata.lifecycle_state {
                latest_value::LifecycleState::Active { last_accessed } => {
                    LifecycleState::Active { last_accessed }
                }
                latest_value::LifecycleState::Trash { trashed_at } => {
                    LifecycleState::Trash { trashed_at }
                }
            },
        };

        let attachments = value
            .attachments
            .into_iter()
            .map(|a| Attachment {
                filename: a.filename,
                size: a.size,
            })
            .collect();

        Self {
            metadata,
            attachments,
            thumb_version: value.thumb_version,
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct Metadata {
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LifecycleState {
    Active { last_accessed: SystemTime },
    Trash { trashed_at: SystemTime },
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub size: u64,
}
