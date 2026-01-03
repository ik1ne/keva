//! WebView message types.

use serde::{Deserialize, Serialize};

/// Messages from WebView to native.
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
    Touch {
        key: String,
    },
    Hide,
    ShutdownAck,
    OpenFilePicker {
        key: String,
    },
    /// Add attachments with target filenames.
    AddAttachments {
        key: String,
        /// Each file: [source_path, target_filename]
        files: Vec<(String, String)>,
    },
}

/// Messages from native to WebView.
#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OutgoingMessage {
    /// Signals WebView that core is ready. Hide splash screen.
    CoreReady,
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
    /// Signals WebView to restore focus after window is shown.
    Focus,
    /// Files selected from file picker. Frontend should check conflicts and send AddAttachments.
    FilesSelected {
        key: String,
        /// Selected file paths
        files: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameResultType {
    Success,
    DestinationExists,
    InvalidKey,
    NotFound,
}

/// Exact match status for current search query.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExactMatch {
    None,
    Active,
    Trashed,
}
