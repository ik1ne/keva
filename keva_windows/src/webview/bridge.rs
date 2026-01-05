//! WebView message bridge.

use super::WEBVIEW;
use super::messages::{IncomingMessage, OutgoingMessage};
use super::wm;
use super::FilePickerRequest;
use crate::keva_worker::Request;
use crate::platform::drop_target::take_dropped_paths;
use crate::render::theme::Theme;
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW, PostMessageW, PostQuitMessage, SW_HIDE, ShowWindow,
};
use windows_strings::w;
use windows::core::PWSTR;

pub fn handle_webview_message(msg: &str, parent_hwnd: HWND, request_tx: &Sender<Request>) {
    let Ok(message) = serde_json::from_str::<IncomingMessage>(msg) else {
        eprintln!("[Bridge] Failed to parse: {}", msg);
        return;
    };

    match message {
        IncomingMessage::Ready => {
            // Send theme directly (synchronous, UI thread)
            if let Some(wv) = WEBVIEW.get() {
                let theme = Theme::detect_system();
                post_message(
                    &wv.webview,
                    &OutgoingMessage::Theme {
                        theme: theme.as_str().to_string(),
                    },
                );
            }
            // Worker responds with CoreReady after init is done
            let _ = request_tx.send(Request::WebviewReady);
        }
        IncomingMessage::Search { query } => {
            let _ = request_tx.send(Request::Search { query });
        }
        IncomingMessage::Select { key } => {
            let _ = request_tx.send(Request::GetValue { key });
        }
        IncomingMessage::Save { key, content } => {
            let _ = request_tx.send(Request::Save { key, content });
        }
        IncomingMessage::Create { key } => {
            let _ = request_tx.send(Request::Create { key });
        }
        IncomingMessage::Rename {
            old_key,
            new_key,
            force,
        } => {
            let _ = request_tx.send(Request::Rename {
                old_key,
                new_key,
                force,
            });
        }
        IncomingMessage::Trash { key } => {
            let _ = request_tx.send(Request::Trash { key });
        }
        IncomingMessage::Touch { key } => {
            let _ = request_tx.send(Request::Touch { key });
        }
        IncomingMessage::Hide => {
            let _ = unsafe { ShowWindow(parent_hwnd, SW_HIDE) };
        }
        IncomingMessage::ShutdownAck => {
            let _ = request_tx.send(Request::Shutdown);
        }
        IncomingMessage::ShutdownBlocked => {
            let result = unsafe {
                MessageBoxW(
                    Some(parent_hwnd),
                    w!("File copy in progress. Exit anyway?"),
                    w!("Keva"),
                    MB_YESNO | MB_ICONWARNING,
                )
            };
            if result == IDYES {
                unsafe { PostQuitMessage(0) };
            }
        }
        IncomingMessage::OpenFilePicker { key } => {
            // Post to UI thread - file picker must run on UI thread
            let request = Box::new(FilePickerRequest {
                key,
                request_tx: request_tx.clone(),
            });
            unsafe {
                let _ = PostMessageW(
                    Some(parent_hwnd),
                    wm::OPEN_FILE_PICKER,
                    WPARAM(0),
                    LPARAM(Box::into_raw(request) as isize),
                );
            }
        }
        IncomingMessage::AddAttachments { key, files } => {
            let _ = request_tx.send(Request::AddAttachments { key, files });
        }
        IncomingMessage::RemoveAttachment { key, filename } => {
            let _ = request_tx.send(Request::RemoveAttachment { key, filename });
        }
        IncomingMessage::RenameAttachment {
            key,
            old_filename,
            new_filename,
            force,
        } => {
            let _ = request_tx.send(Request::RenameAttachment {
                key,
                old_filename,
                new_filename,
                force,
            });
        }
        IncomingMessage::AddDroppedFiles { key, files } => {
            // Get cached paths from IDropTarget and match by index
            let cached_paths = take_dropped_paths();
            let resolved_files: Vec<(std::path::PathBuf, String)> = files
                .into_iter()
                .filter_map(|(index, filename)| {
                    cached_paths.get(index).map(|path| (path.clone(), filename))
                })
                .collect();

            if !resolved_files.is_empty() {
                let _ = request_tx.send(Request::AddDroppedFiles {
                    key,
                    files: resolved_files,
                });
            }
        }
    }
}

pub fn post_message(wv: &ICoreWebView2, msg: &OutgoingMessage) {
    let json = serde_json::to_string(msg).expect("Failed to serialize message");
    // Keep Vec alive until PostWebMessageAsJson returns
    let wide: Vec<u16> = json.encode_utf16().chain(std::iter::once(0)).collect();
    let msg_pwstr = PWSTR(wide.as_ptr() as *mut u16);
    let _ = unsafe { wv.PostWebMessageAsJson(msg_pwstr) };
}

pub fn pwstr_to_string(pwstr: PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe { pwstr.to_string().unwrap_or_default() }
}
