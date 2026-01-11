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

/// Messages from native to WebView.
///
/// All messages are posted via PostMessageW from worker thread to UI thread.
/// The Value variant requires `PostWebMessageAsJsonWithAdditionalObjects` for FileSystemHandle.
#[derive(Serialize)]
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
    /// Key value with FileSystemHandle for content file.
    Value {
        key: String,
        key_hash: String,
        /// Base path to blobs directory for constructing file:// URLs.
        blobs_path: String,
        #[serde(skip)]
        content_path: std::path::PathBuf,
        read_only: bool,
        attachments: Vec<AttachmentInfo>,
    },
    /// Files pasted from clipboard (paths cached in native).
    FilesPasted {
        files: Vec<String>,
    },
    /// Signal JS to perform a copy action.
    DoCopy {
        action: CopyAction,
    },
    /// Result of copy operation.
    CopyResult {
        success: bool,
    },
    /// Open settings panel with current config.
    OpenSettings {
        config: keva_core::types::AppConfig,
        /// Read from registry, not config file.
        launch_at_login: bool,
    },
}

/// Copy action type for DoCopy message.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyAction {
    Markdown,
    Html,
    Files,
}

/// Exact match status for current search query.
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

/// SAFETY: WebView COM interfaces are apartment-threaded and must only be accessed
/// from the UI thread. This is safe because:
/// - The worker thread never calls methods on WebView directly; it only posts
///   messages to the UI thread via PostMessageW with raw pointers
/// - All actual WebView method calls occur in UI thread message handlers
///   (wndproc, WebMessageReceived, AcceleratorKeyPressed, etc.)
/// - The OnceLock ensures initialization happens exactly once on the UI thread
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
