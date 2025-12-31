use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::ValueVariant;

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    pub metadata: Metadata,
    pub attachments: Vec<Attachment>,
    pub thumb_version: u32,
}

impl ValueVariant for Value {
    const VERSION: u8 = 1;
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub lifecycle_state: LifecycleState,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LifecycleState {
    Active { last_accessed: SystemTime },
    Trash { trashed_at: SystemTime },
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub size: u64,
}
