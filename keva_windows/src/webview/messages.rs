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
    Search { query: String },
    Select { key: String },
    Save { key: String, content: String },
    Create { key: String },
    Hide,
    ShutdownAck,
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
        keys: Vec<KeyInfo>,
        exact_match: ExactMatch,
    },
    Value {
        key: String,
        value: Option<ValueInfo>,
    },
    Shutdown,
}

/// Exact match status for current search query.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExactMatch {
    None,
    Active,
    Trashed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub name: String,
    pub trashed: bool,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ValueInfo {
    Text {
        content: String,
    },
    #[expect(dead_code)]
    Files {
        count: usize,
    },
}
