//! Clipboard operations for reading/writing system clipboard.
//!
//! Files take priority over text when the clipboard contains both.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessFailed(String),

    #[error("No content in clipboard")]
    NoContent,

    #[error("Failed to read clipboard: {0}")]
    ReadFailed(String),

    #[error("Failed to write to clipboard: {0}")]
    WriteFailed(String),
}

pub(crate) enum ClipboardContent {
    Text(String),
    Files(Vec<PathBuf>),
}

use clipboard_rs::{Clipboard, ClipboardContext, ContentFormat};

/// Read current clipboard content.
/// Files take priority over text when both are present.
pub(crate) fn read_clipboard() -> Result<ClipboardContent, ClipboardError> {
    let ctx = ClipboardContext::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    // Check files first (higher priority)
    if ctx.has(ContentFormat::Files) {
        let files = ctx
            .get_files()
            .map_err(|e| ClipboardError::ReadFailed(e.to_string()))?;

        if !files.is_empty() {
            let paths = files.into_iter().map(PathBuf::from).collect();
            return Ok(ClipboardContent::Files(paths));
        }
    }

    // Fall back to text
    if ctx.has(ContentFormat::Text) {
        let text = ctx
            .get_text()
            .map_err(|e| ClipboardError::ReadFailed(e.to_string()))?;

        if !text.is_empty() {
            return Ok(ClipboardContent::Text(text));
        }
    }

    Err(ClipboardError::NoContent)
}

/// Write text to clipboard.
pub(crate) fn write_text(text: &str) -> Result<(), ClipboardError> {
    let ctx = ClipboardContext::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    ctx.set_text(text.to_string())
        .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
}

/// Write file paths to clipboard.
pub(crate) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    let ctx = ClipboardContext::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    let file_strings: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    ctx.set_files(file_strings)
        .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
}
