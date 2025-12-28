//! Background worker thread for KevaCore operations.
//!
//! Keeps keva operations off the UI thread to prevent blocking.

use crate::webview::messages::{KeyInfo, OutgoingMessage, ValueInfo};
use keva_core::core::KevaCore;
use keva_core::types::{ClipData, Config, Key, SavedConfig, TextContent};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

/// Custom message ID for keva responses.
pub const WM_KEVA_RESPONSE: u32 = WM_APP + 1;

/// Request types sent from UI thread to worker.
pub enum Request {
    GetKeys,
    GetValue { key: String },
}

/// Starts the worker thread and returns channels for communication.
///
/// Returns `(request_sender, response_receiver)`:
/// - `request_sender`: Send requests to the worker (store in WebView)
/// - `response_receiver`: Receive responses from the worker (store in AppState)
pub fn start(hwnd: HWND) -> (Sender<Request>, Receiver<OutgoingMessage>) {
    let (request_tx, request_rx) = mpsc::channel::<Request>();
    let (response_tx, response_rx) = mpsc::channel::<OutgoingMessage>();

    let hwnd = hwnd.0 as isize;
    thread::spawn(move || {
        let hwnd = HWND(hwnd as *mut _);
        worker_loop(request_rx, response_tx, hwnd);
    });

    eprintln!("[KevaWorker] Started");
    (request_tx, response_rx)
}

fn worker_loop(requests: Receiver<Request>, responses: Sender<OutgoingMessage>, hwnd: HWND) {
    let mut keva = open_keva();
    eprintln!("[KevaWorker] Database at {}", get_data_path().display());

    for request in requests {
        let response = handle_request(&mut keva, request);
        if responses.send(response).is_ok() {
            // Notify UI thread that a response is ready
            let _ = unsafe { PostMessageW(Some(hwnd), WM_KEVA_RESPONSE, WPARAM(0), LPARAM(0)) };
        }
    }
}

fn handle_request(keva: &mut KevaCore, request: Request) -> OutgoingMessage {
    match request {
        Request::GetKeys => {
            let active = keva.active_keys().unwrap_or_default();
            let trashed = keva.trashed_keys().unwrap_or_default();

            let mut keys: Vec<KeyInfo> = active
                .iter()
                .map(|k| KeyInfo {
                    name: k.as_str().to_string(),
                    trashed: false,
                })
                .collect();

            keys.extend(trashed.iter().map(|k| KeyInfo {
                name: k.as_str().to_string(),
                trashed: true,
            }));

            eprintln!("[KevaWorker] Fetched {} keys", keys.len());
            OutgoingMessage::Keys { keys }
        }
        Request::GetValue { key: key_str } => {
            let now = SystemTime::now();
            let result = (|| {
                let key = Key::try_from(key_str.as_str()).ok()?;
                let _ = keva.touch(&key, now);
                keva.get(&key).ok().flatten()
            })();

            let value = result.map(|v| match v.clip_data {
                ClipData::Text(TextContent::Inlined(s)) => ValueInfo::Text { content: s },
                ClipData::Text(TextContent::BlobStored { path }) => {
                    let content = std::fs::read_to_string(path).unwrap_or_default();
                    ValueInfo::Text { content }
                }
                ClipData::Files(files) => ValueInfo::Files { count: files.len() },
            });

            OutgoingMessage::Value { value }
        }
    }
}

fn open_keva() -> KevaCore {
    let config = Config {
        base_path: get_data_path(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
            inline_threshold_bytes: 1024 * 1024,               // 1MB
        },
    };

    KevaCore::open(config).expect("Failed to open keva database")
}

fn get_data_path() -> PathBuf {
    std::env::var("KEVA_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA not set")
                .join("keva")
        })
}
