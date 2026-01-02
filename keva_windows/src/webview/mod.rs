//! WebView2 initialization and management.

pub mod bridge;
mod init;
pub mod messages;

pub use init::init_webview;

use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
};
use webview2_com::pwstr_from_str;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// Posted by forwarder thread to marshal PostWebMessageAsJson to UI thread.
/// LPARAM contains a Box<String> pointer to the JSON message.
pub const WM_WEBVIEW_MESSAGE: u32 = WM_APP + 2;

/// Posted by forwarder thread to send FileSystemHandle to WebView.
/// LPARAM contains a Box<FileHandleRequest> pointer.
pub const WM_SEND_FILE_HANDLE: u32 = WM_APP + 3;

/// Request data for sending a FileSystemHandle to WebView.
pub struct FileHandleRequest {
    pub key: String,
    pub content_path: std::path::PathBuf,
    pub read_only: bool,
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
