//! WebView2 initialization and management.

pub mod bridge;
mod init;
pub mod messages;

pub use init::init_webview;

use crate::platform::composition::CompositionHost;
use serde::Serialize;
use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2CompositionController, ICoreWebView2Controller,
    ICoreWebView2Environment,
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

/// Messages sent directly from UI thread to WebView (not via forwarder).
///
/// These messages require UI thread APIs like `PostWebMessageAsJsonWithAdditionalObjects`
/// and cannot be routed through the forwarder thread.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DirectOutgoingMessage {
    Value {
        key: String,
        key_hash: String,
        #[serde(skip)]
        content_path: std::path::PathBuf,
        read_only: bool,
        attachments: Vec<AttachmentInfo>,
    },
}

/// Request data for opening a file picker.
pub struct FilePickerRequest {
    pub key: String,
    pub request_tx: std::sync::mpsc::Sender<crate::keva_worker::Request>,
}

pub static WEBVIEW: OnceLock<WebView> = OnceLock::new();

pub struct WebView {
    pub composition_controller: ICoreWebView2CompositionController,
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
    pub env: ICoreWebView2Environment,
    composition_host: CompositionHost,
}

unsafe impl Send for WebView {}
unsafe impl Sync for WebView {}

impl WebView {
    /// Sets the bounds of the WebView within its parent window.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        // Position via DirectComposition visual offset
        let _ = self.composition_host.set_offset(x, y);

        // Size via controller bounds (position is 0,0 since offset handles it)
        let rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let _ = unsafe { self.controller.SetBounds(rect) };
    }

    /// Commits pending DirectComposition changes.
    pub fn commit_composition(&self) {
        let _ = self.composition_host.commit();
    }

    #[expect(dead_code)]
    pub fn post_message(&self, json: &str) {
        unsafe {
            let msg = pwstr_from_str(json);
            let _ = self.webview.PostWebMessageAsJson(msg);
        }
    }
}
