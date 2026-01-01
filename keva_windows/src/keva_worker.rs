//! Background worker thread for KevaCore and SearchEngine operations.

use crate::webview::messages::{ExactMatch, OutgoingMessage, ValueInfo};
use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, SavedConfig};
use keva_search::{SearchConfig, SearchEngine, SearchQuery};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_SHUTDOWN_COMPLETE: u32 = WM_APP + 1;

pub enum Request {
    /// WebView is ready - respond with CoreReady after init is done.
    WebviewReady,
    GetValue {
        key: String,
    },
    Save {
        key: String,
        content: String,
    },
    Create {
        key: String,
    },
    Search {
        query: String,
    },
    SearchTick,
    Shutdown,
}

/// Starts the worker thread.
///
/// The worker owns KevaCore and SearchEngine. It handles all requests and sends
/// responses via `response_tx`. The caller should pass `response_rx` to the
/// forwarder thread for WebView delivery.
pub fn start(hwnd: HWND, response_tx: Sender<OutgoingMessage>) -> Sender<Request> {
    let (request_tx, request_rx) = mpsc::channel::<Request>();

    let notify_tx = request_tx.clone();
    let hwnd_raw = hwnd.0 as isize;

    thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut _);

        let keva = open_keva();
        let active_keys = keva.active_keys().unwrap_or_default();
        let trashed_keys = keva.trashed_keys().unwrap_or_default();

        let notify = Arc::new(move || {
            let _ = notify_tx.send(Request::SearchTick);
        });
        let search = SearchEngine::new(active_keys, trashed_keys, SearchConfig::default(), notify);

        worker_loop(keva, search, request_rx, response_tx, hwnd);
    });

    request_tx
}

fn worker_loop(
    mut keva: KevaCore,
    mut search: SearchEngine,
    requests: mpsc::Receiver<Request>,
    responses: Sender<OutgoingMessage>,
    hwnd: HWND,
) {
    let mut current_query = String::new();

    // Set empty query to trigger initial SearchResults
    search.set_query(SearchQuery::Fuzzy(String::new()));

    for request in requests {
        match request {
            Request::WebviewReady => {
                // Worker is initialized - signal WebView to hide splash
                let _ = responses.send(OutgoingMessage::CoreReady);
            }

            Request::GetValue { key: key_str } => {
                let now = SystemTime::now();
                let value = (|| {
                    let key = Key::try_from(key_str.as_str()).ok()?;
                    let _ = keva.touch(&key, now);
                    let _value = keva.get(&key).ok().flatten()?;
                    let content_path = keva.content_path(&key);
                    let content = std::fs::read_to_string(content_path).unwrap_or_default();
                    Some(ValueInfo::Text { content })
                })();
                let _ = responses.send(OutgoingMessage::Value {
                    key: key_str,
                    value,
                });
            }

            Request::Save {
                key: key_str,
                content,
            } => {
                let now = SystemTime::now();
                if let Ok(key) = Key::try_from(key_str.as_str()) {
                    let content_path = keva.content_path(&key);
                    if std::fs::write(&content_path, &content).is_ok() {
                        let _ = keva.touch(&key, now);
                    }
                }
            }

            Request::Create { key: key_str } => {
                let now = SystemTime::now();
                let success = Key::try_from(key_str.as_str())
                    .ok()
                    .and_then(|key| {
                        if keva.create(&key, now).is_ok() {
                            search.add_active(key);
                            Some(())
                        } else {
                            None
                        }
                    })
                    .is_some();

                let _ = responses.send(OutgoingMessage::KeyCreated {
                    key: key_str,
                    success,
                });

                if success {
                    // Refresh search to include newly created key
                    search.set_query(SearchQuery::Fuzzy(current_query.clone()));
                    search.tick();
                    send_search_results(&search, &current_query, &responses);
                }
            }

            Request::Search { query } => {
                current_query = query.clone();
                search.set_query(SearchQuery::Fuzzy(query));
                // Immediately tick and send results for responsiveness
                search.tick();
                send_search_results(&search, &current_query, &responses);
            }

            Request::SearchTick => {
                if search.tick() {
                    send_search_results(&search, &current_query, &responses);
                }
            }

            Request::Shutdown => {
                let _ =
                    unsafe { PostMessageW(Some(hwnd), WM_SHUTDOWN_COMPLETE, WPARAM(0), LPARAM(0)) };
                break;
            }
        }
    }
}

fn send_search_results(
    search: &SearchEngine,
    current_query: &str,
    responses: &Sender<OutgoingMessage>,
) {
    let active_keys: Vec<String> = search
        .active_results()
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();

    let trashed_keys: Vec<String> = search
        .trashed_results()
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();

    let exact_match = Key::try_from(current_query)
        .ok()
        .map(|key| {
            if search.has_active(&key) {
                ExactMatch::Active
            } else if search.has_trashed(&key) {
                ExactMatch::Trashed
            } else {
                ExactMatch::None
            }
        })
        .unwrap_or(ExactMatch::None);

    let _ = responses.send(OutgoingMessage::SearchResults {
        active_keys,
        trashed_keys,
        exact_match,
    });
}

fn open_keva() -> KevaCore {
    let config = Config {
        base_path: get_data_path(),
        saved: SavedConfig {
            trash_ttl: Duration::from_secs(30 * 24 * 60 * 60),
            purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),
        },
    };
    KevaCore::open(config).expect("Failed to open database")
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
