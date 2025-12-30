//! WebView message types.

use serde::{Deserialize, Serialize};

/// Messages from WebView to native.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum IncomingMessage {
    Ready,
    Select { key: String },
    Save { key: String, content: String },
    Create { key: String },
    Hide,
    ShutdownAck,
}

/// Messages from native to WebView.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OutgoingMessage {
    Keys { keys: Vec<KeyInfo> },
    Value { value: Option<ValueInfo> },
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub name: String,
    pub trashed: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ValueInfo {
    Text { content: String },
    Files { count: usize },
}
