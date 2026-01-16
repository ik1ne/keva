use std::time::SystemTime;

pub(crate) mod versioned_value;

use versioned_value::latest_value;

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, Clone)]
pub struct PublicValue {
    pub metadata: Metadata,
    pub attachments: Vec<Attachment>,
    pub thumb_version: u32,
}

impl PublicValue {
    pub(crate) fn from_latest_value(value: latest_value::Value) -> Self {
        let metadata = Metadata {
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
