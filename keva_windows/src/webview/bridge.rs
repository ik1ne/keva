//! WebView message bridge.

use super::messages::{IncomingMessage, OutgoingMessage};
use crate::keva_worker::Request;
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use webview2_com::pwstr_from_str;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
use windows::core::PWSTR;

/// Handles messages from WebView and sends responses.
pub fn handle_webview_message(msg: &str, parent_hwnd: HWND, request_tx: &Sender<Request>) {
    let Ok(message) = serde_json::from_str::<IncomingMessage>(msg) else {
        eprintln!("[Native] Failed to parse message: {}", msg);
        return;
    };

    match message {
        IncomingMessage::Ready => {
            eprintln!("[Native] Received 'ready' from WebView");
            let _ = request_tx.send(Request::GetKeys);
        }
        IncomingMessage::Select { key } => {
            eprintln!("[Native] Selected key: {}", key);
            let _ = request_tx.send(Request::GetValue { key });
        }
        IncomingMessage::Save { key, content } => {
            eprintln!("[Native] Saving key: {}", key);
            let _ = request_tx.send(Request::Save { key, content });
        }
        IncomingMessage::Create { key } => {
            eprintln!("[Native] Creating key: {}", key);
            let _ = request_tx.send(Request::Create { key });
        }
        IncomingMessage::Hide => {
            let _ = unsafe { ShowWindow(parent_hwnd, SW_HIDE) };
        }
        IncomingMessage::ShutdownAck => {
            eprintln!("[Native] ShutdownAck received, sending shutdown to worker");
            let _ = request_tx.send(Request::Shutdown);
        }
    }
}

/// Posts an outgoing message to the WebView.
pub fn post_message(wv: &ICoreWebView2, msg: &OutgoingMessage) {
    let json = serde_json::to_string(msg).expect("Failed to serialize message");
    let msg_pwstr = pwstr_from_str(&json);
    let _ = unsafe { wv.PostWebMessageAsJson(msg_pwstr) };
}

/// Converts a PWSTR to a String.
pub fn pwstr_to_string(pwstr: PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe { pwstr.to_string().unwrap_or_default() }
}
