//! WebView2 initialization and management.

pub mod bridge;
mod init;
pub mod messages;

pub use init::init_webview;

use serde::Serialize;
use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
};
use webview2_com::pwstr_from_str;
use windows::Win32::Foundation::RECT;

pub mod wm;

/// Attachment metadata for WebView display.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub filename: String,
    pub size: u64,
    pub thumbnail_url: Option<String>,
}

/// Request data for sending a FileSystemHandle to WebView.
pub struct FileHandleRequest {
    pub key: String,
    pub content_path: std::path::PathBuf,
    pub read_only: bool,
    pub attachments: Vec<AttachmentInfo>,
}

/// Request data for opening a file picker.
pub struct FilePickerRequest {
    pub key: String,
    pub request_tx: std::sync::mpsc::Sender<crate::keva_worker::Request>,
}

pub static WEBVIEW: OnceLock<WebView> = OnceLock::new();

pub struct WebView {
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
    pub env: ICoreWebView2Environment,
}

unsafe impl Send for WebView {}
unsafe impl Sync for WebView {}

impl WebView {
    /// Sets the bounds of the WebView within its parent window.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let rect = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = self.controller.SetBounds(rect);
        }
    }

    #[expect(dead_code)]
    pub fn post_message(&self, json: &str) {
        unsafe {
            let msg = pwstr_from_str(json);
            let _ = self.webview.PostWebMessageAsJson(msg);
        }
    }
}
