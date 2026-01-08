//! WebView message types.

use serde::Deserialize;

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
    ShutdownBlocked,
    OpenFilePicker {
        key: String,
    },
    /// Add attachments with target filenames.
    AddAttachments {
        key: String,
        /// Each file: [source_path, target_filename]
        files: Vec<(String, String)>,
    },
    /// Remove an attachment from a key.
    RemoveAttachment {
        key: String,
        filename: String,
    },
    /// Rename an attachment.
    RenameAttachment {
        key: String,
        old_filename: String,
        new_filename: String,
        /// If true, overwrite existing file with same name.
        force: bool,
    },
    /// Add dropped files using cached paths from IDropTarget.
    /// Files are referenced by index (matching order in JS drop event).
    AddDroppedFiles {
        key: String,
        /// Each file: [index, resolved_filename]
        files: Vec<(usize, String)>,
    },
    /// Add files from clipboard using cached paths.
    AddClipboardFiles {
        key: String,
        /// Each file: [index, resolved_filename]
        files: Vec<(usize, String)>,
    },
    /// Copy files to clipboard.
    CopyFiles {
        key: String,
        filenames: Vec<String>,
    },
}
