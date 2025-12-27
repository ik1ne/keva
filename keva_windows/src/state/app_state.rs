//! Central application state.

use crate::webview::WebView;

/// Application state container.
pub struct AppState {
    /// The single WebView covering the entire window.
    pub webview: Option<WebView>,
}

impl AppState {
    pub fn new() -> Self {
        Self { webview: None }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
