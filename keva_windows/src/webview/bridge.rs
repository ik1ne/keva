//! WebView message bridge.

use super::WEBVIEW;
use super::messages::{IncomingMessage, OutgoingMessage};
use crate::keva_worker::Request;
use crate::render::theme::Theme;
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
use windows::core::PWSTR;

pub fn handle_webview_message(msg: &str, parent_hwnd: HWND, request_tx: &Sender<Request>) {
    let Ok(message) = serde_json::from_str::<IncomingMessage>(msg) else {
        eprintln!("[Bridge] Failed to parse: {}", msg);
        return;
    };

    match message {
        IncomingMessage::Ready => {
            eprintln!("[Bridge] WebView ready");
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
        IncomingMessage::Hide => {
            let _ = unsafe { ShowWindow(parent_hwnd, SW_HIDE) };
        }
        IncomingMessage::ShutdownAck => {
            eprintln!("[Bridge] ShutdownAck, sending to worker");
            let _ = request_tx.send(Request::Shutdown);
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
