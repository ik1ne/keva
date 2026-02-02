use serde::{Deserialize, Serialize};

/// Messages from WebView to native (via Swift FFI).
#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IncomingMessage {
    Ready,
    Search {
        query: String,
    },
    Select {
        key: String,
    },
    Save {
        key: String,
        content: String,
    },
    Create {
        key: String,
    },
    Rename {
        old_key: String,
        new_key: String,
        force: bool,
    },
    Trash {
        key: String,
    },
    Restore {
        key: String,
    },
    Purge {
        key: String,
    },
    Touch {
        key: String,
    },
    ShutdownAck,
    Maintenance {
        force: bool,
    },
    /// Internal: triggered by search engine when results are ready.
    #[serde(skip)]
    SearchTick,
}

/// Messages from native to WebView (via Swift FFI).
///
/// Some variants (Theme, Shutdown, Toast) are sent directly from Swift or
/// reserved for future features. They are included for message type completeness.
#[derive(Serialize)]
#[allow(dead_code)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OutgoingMessage {
    CoreReady,
    CoreInitFailed {
        message: String,
        data_dir: String,
    },
    Theme {
        theme: String,
    },
    KeyCreated {
        key: String,
        success: bool,
    },
    SearchResults {
        active_keys: Vec<String>,
        trashed_keys: Vec<String>,
        exact_match: ExactMatch,
    },
    RenameResult {
        old_key: String,
        new_key: String,
        result: RenameResultType,
    },
    Shutdown,
    /// Key value with content read from file.
    Value {
        key: String,
        key_hash: String,
        content: String,
        read_only: bool,
        attachments: Vec<AttachmentInfo>,
    },
    Toast {
        message: String,
    },
    SaveFailed {
        key: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExactMatch {
    None,
    Active,
    Trashed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameResultType {
    Success,
    DestinationExists,
    InvalidKey,
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub filename: String,
    pub size: u64,
    pub thumbnail_url: Option<String>,
}
