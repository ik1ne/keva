//! Background worker thread for KevaCore and SearchEngine operations.

use crate::webview::{AttachmentInfo, ExactMatch, OutgoingMessage, RenameResultType, wm};
use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, LifecycleState, SavedConfig};
use keva_search::{SearchConfig, SearchEngine, SearchQuery};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, PostMessageW};

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
    Rename {
        old_key: String,
        new_key: String,
        force: bool,
    },
    Trash {
        key: String,
    },
    Restore {
        key: String,
    },
    Purge {
        key: String,
    },
    Search {
        query: String,
    },
    SearchTick,
    /// Update timestamp after content save via FileSystemHandle.
    Touch {
        key: String,
    },
    /// Files selected from picker - send to frontend for conflict check.
    FilesSelected {
        key: String,
        files: Vec<std::path::PathBuf>,
    },
    /// Add attachments with target filenames from frontend.
    AddAttachments {
        key: String,
        /// (source_path, target_filename)
        files: Vec<(String, String)>,
    },
    /// Remove an attachment from a key.
    RemoveAttachment {
        key: String,
        filename: String,
    },
    /// Rename an attachment.
    RenameAttachment {
        key: String,
        old_filename: String,
        new_filename: String,
        /// If true, overwrite existing file with same name.
        force: bool,
    },
    /// Add files from drop or clipboard.
    AddFiles {
        key: String,
        /// (source_path, target_filename)
        files: Vec<(PathBuf, String)>,
    },
    /// Run maintenance (GC and orphan cleanup).
    /// If force is false, only runs if should_run_maintenance returns true.
    Maintenance {
        force: bool,
    },
    Shutdown,
}

/// Starts the worker thread.
///
/// The worker owns KevaCore and SearchEngine. It handles all requests and posts
/// responses directly to the UI thread via PostMessageW.
pub fn start(hwnd: HWND) -> mpsc::Sender<Request> {
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

        worker_loop(keva, search, request_rx, hwnd);
    });

    request_tx
}

/// Posts an OutgoingMessage to the UI thread for WebView delivery.
fn post_response(hwnd: HWND, msg: OutgoingMessage) {
    let ptr = Box::into_raw(Box::new(msg));
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            wm::WEBVIEW_MESSAGE,
            WPARAM(0),
            LPARAM(ptr as isize),
        );
    }
}

fn worker_loop(
    mut keva: KevaCore,
    mut search: SearchEngine,
    requests: mpsc::Receiver<Request>,
    hwnd: HWND,
) {
    let mut current_query = String::new();

    // Set empty query to trigger initial SearchResults
    search.set_query(SearchQuery::Fuzzy(String::new()));

    // Run maintenance on launch if needed (>24h since last run)
    handle_maintenance(&mut keva, &mut search, &current_query, false, hwnd);
    let mut next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;

    loop {
        let timeout = next_maintenance.saturating_duration_since(Instant::now());
        let request = match requests.recv_timeout(timeout) {
            Ok(req) => Some(req),
            Err(RecvTimeoutError::Timeout) => {
                // Timer fired - only run if window is hidden to avoid UI state issues
                let is_visible = unsafe { IsWindowVisible(hwnd).as_bool() };
                if !is_visible {
                    handle_maintenance(&mut keva, &mut search, &current_query, false, hwnd);
                }
                next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let Some(request) = request else { continue };

        match request {
            Request::WebviewReady => {
                post_response(hwnd, OutgoingMessage::CoreReady);
            }
            Request::GetValue { key } => {
                handle_get_value(&mut keva, &key, hwnd);
            }
            Request::Save { key, content } => {
                handle_save(&mut keva, &key, &content);
            }
            Request::Create { key } => {
                handle_create(&mut keva, &mut search, &key, &current_query, hwnd);
            }
            Request::Rename {
                old_key,
                new_key,
                force,
            } => {
                handle_rename(&mut keva, &mut search, &old_key, &new_key, force, hwnd);
            }
            Request::Trash { key } => {
                handle_trash(&mut keva, &mut search, &key, &current_query, hwnd);
            }
            Request::Restore { key } => {
                handle_restore(&mut keva, &mut search, &key, &current_query, hwnd);
            }
            Request::Purge { key } => {
                handle_purge(&mut keva, &mut search, &key, &current_query, hwnd);
            }
            Request::Search { query } => {
                current_query = query.clone();
                search.set_query(SearchQuery::Fuzzy(query));
                search.tick();
                send_search_results(&search, &current_query, hwnd);
            }
            Request::SearchTick => {
                if search.tick() {
                    send_search_results(&search, &current_query, hwnd);
                }
            }
            Request::Touch { key } => {
                if let Ok(key) = Key::try_from(key.as_str()) {
                    let _ = keva.touch(&key, SystemTime::now());
                }
            }
            Request::FilesSelected { key, files } => {
                handle_files_selected(&key, files, hwnd);
            }
            Request::AddAttachments { key, files } => {
                handle_add_attachments(&mut keva, &key, files, hwnd);
            }
            Request::RemoveAttachment { key, filename } => {
                handle_remove_attachment(&mut keva, &key, &filename, hwnd);
            }
            Request::RenameAttachment {
                key,
                old_filename,
                new_filename,
                force,
            } => {
                handle_rename_attachment(
                    &mut keva,
                    &key,
                    &old_filename,
                    &new_filename,
                    force,
                    hwnd,
                );
            }
            Request::AddFiles { key, files } => {
                handle_add_files(&mut keva, &key, files, hwnd);
            }
            Request::Maintenance { force } => {
                handle_maintenance(&mut keva, &mut search, &current_query, force, hwnd);
                // Reset timer after any maintenance (manual or scheduled)
                next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;
            }
            Request::Shutdown => {
                unsafe {
                    let _ = PostMessageW(Some(hwnd), wm::SHUTDOWN_COMPLETE, WPARAM(0), LPARAM(0));
                }
                break;
            }
        }
    }
}

fn handle_get_value(keva: &mut KevaCore, key_str: &str, hwnd: HWND) {
    let Some((value, read_only, key)) = (|| {
        let now = SystemTime::now();
        let key = Key::try_from(key_str).ok()?;
        let value = keva.get(&key).ok().flatten()?;
        let read_only = matches!(value.metadata.lifecycle_state, LifecycleState::Trash { .. });
        if !read_only {
            let _ = keva.touch(&key, now);
        }
        Some((value, read_only, key))
    })() else {
        return;
    };

    let key_hash = KevaCore::key_to_path(&key).to_string_lossy().into_owned();

    // Build attachment info with thumbnail URLs (paths are relative to thumbnails dir)
    let thumbnail_paths = keva.thumbnail_paths(&key).unwrap_or_default();
    let attachments: Vec<AttachmentInfo> = value
        .attachments
        .into_iter()
        .map(|att| {
            let thumbnail_url = thumbnail_paths.get(&att.filename).map(|rel_path| {
                format!(
                    "https://keva-data.local/thumbnails/{}",
                    rel_path.to_string_lossy().replace('\\', "/")
                )
            });
            AttachmentInfo {
                filename: att.filename,
                size: att.size,
                thumbnail_url,
            }
        })
        .collect();

    let blobs_path = get_data_path()
        .join("blobs")
        .to_string_lossy()
        .replace('\\', "/");

    post_response(
        hwnd,
        OutgoingMessage::Value {
            key: key_str.to_string(),
            key_hash,
            blobs_path,
            content_path: keva.content_path(&key),
            read_only,
            attachments,
        },
    );
}

fn handle_save(keva: &mut KevaCore, key_str: &str, content: &str) {
    if let Ok(key) = Key::try_from(key_str) {
        let content_path = keva.content_path(&key);
        if std::fs::write(&content_path, content).is_ok() {
            let _ = keva.touch(&key, SystemTime::now());
        }
    }
}

fn handle_files_selected(key_str: &str, files: Vec<PathBuf>, hwnd: HWND) {
    let files: Vec<String> = files
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    post_response(
        hwnd,
        OutgoingMessage::FilesSelected {
            key: key_str.to_string(),
            files,
        },
    );
}

fn handle_add_attachments(
    keva: &mut KevaCore,
    key_str: &str,
    files: Vec<(String, String)>,
    hwnd: HWND,
) {
    let Ok(key) = Key::try_from(key_str) else {
        return;
    };

    let files: Vec<(PathBuf, String)> = files
        .into_iter()
        .map(|(path, name)| (PathBuf::from(path), name))
        .collect();

    if keva.add_attachments(&key, files, SystemTime::now()).is_ok() {
        handle_get_value(keva, key_str, hwnd);
    }
}

fn handle_remove_attachment(keva: &mut KevaCore, key_str: &str, filename: &str, hwnd: HWND) {
    let Ok(key) = Key::try_from(key_str) else {
        return;
    };

    if keva
        .remove_attachment(&key, filename, SystemTime::now())
        .is_ok()
    {
        handle_get_value(keva, key_str, hwnd);
    }
}

fn handle_rename_attachment(
    keva: &mut KevaCore,
    key_str: &str,
    old_filename: &str,
    new_filename: &str,
    force: bool,
    hwnd: HWND,
) {
    let Ok(key) = Key::try_from(key_str) else {
        return;
    };

    // If force, remove destination first
    if force {
        let _ = keva.remove_attachment(&key, new_filename, SystemTime::now());
    }

    if keva
        .rename_attachment(&key, old_filename, new_filename, SystemTime::now())
        .is_ok()
    {
        handle_get_value(keva, key_str, hwnd);
    }
}

fn handle_add_files(keva: &mut KevaCore, key_str: &str, files: Vec<(PathBuf, String)>, hwnd: HWND) {
    let Ok(key) = Key::try_from(key_str) else {
        return;
    };

    if keva.add_attachments(&key, files, SystemTime::now()).is_ok() {
        handle_get_value(keva, key_str, hwnd);
    }
}

fn handle_create(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    hwnd: HWND,
) {
    let success = try_create(keva, search, key_str).is_some();

    post_response(
        hwnd,
        OutgoingMessage::KeyCreated {
            key: key_str.to_string(),
            success,
        },
    );

    if success {
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, hwnd);
    }
}

fn try_create(keva: &mut KevaCore, search: &mut SearchEngine, key_str: &str) -> Option<()> {
    let key = Key::try_from(key_str).ok()?;
    keva.create(&key, SystemTime::now()).ok()?;
    search.add_active(key);
    Some(())
}

fn handle_rename(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    old_key_str: &str,
    new_key_str: &str,
    force: bool,
    hwnd: HWND,
) {
    let result = try_rename(keva, search, old_key_str, new_key_str, force);
    post_response(
        hwnd,
        OutgoingMessage::RenameResult {
            old_key: old_key_str.to_string(),
            new_key: new_key_str.to_string(),
            result: result.unwrap_or_else(|e| e),
        },
    );
}

fn try_rename(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    old_key_str: &str,
    new_key_str: &str,
    force: bool,
) -> Result<RenameResultType, RenameResultType> {
    let old_key = Key::try_from(old_key_str).map_err(|_| RenameResultType::NotFound)?;
    let new_key = Key::try_from(new_key_str).map_err(|_| RenameResultType::InvalidKey)?;

    if keva.get(&new_key).ok().flatten().is_some() {
        if force {
            let _ = keva.purge(&new_key);
            search.remove(&new_key);
        } else {
            return Err(RenameResultType::DestinationExists);
        }
    }

    keva.rename(&old_key, &new_key, SystemTime::now())
        .map_err(|_| RenameResultType::NotFound)?;
    search.rename(&old_key, new_key);
    Ok(RenameResultType::Success)
}

fn handle_trash(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    hwnd: HWND,
) {
    let now = SystemTime::now();
    if let Ok(key) = Key::try_from(key_str)
        && keva.trash(&key, now).is_ok()
    {
        search.trash(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, hwnd);
    }
}

fn handle_restore(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    hwnd: HWND,
) {
    let now = SystemTime::now();
    if let Ok(key) = Key::try_from(key_str)
        && keva.restore(&key, now).is_ok()
    {
        search.restore(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, hwnd);
    }
}

fn handle_purge(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    hwnd: HWND,
) {
    if let Ok(key) = Key::try_from(key_str)
        && keva.purge(&key).is_ok()
    {
        search.remove(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, hwnd);
    }
}

/// 24 hours interval for periodic maintenance check.
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

fn handle_maintenance(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    current_query: &str,
    force: bool,
    hwnd: HWND,
) {
    let now = SystemTime::now();

    if !force && !keva.should_run_maintenance(now, MAINTENANCE_INTERVAL) {
        return;
    }

    let Ok(outcome) = keva.maintenance(now) else {
        return;
    };

    // Update search engine with auto-trashed and purged keys
    let mut changed = false;
    for key in &outcome.keys_trashed {
        search.trash(key);
        changed = true;
    }
    for key in &outcome.keys_purged {
        search.remove(key);
        changed = true;
    }

    if changed {
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, hwnd);
    }
}

fn send_search_results(search: &SearchEngine, current_query: &str, hwnd: HWND) {
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

    post_response(
        hwnd,
        OutgoingMessage::SearchResults {
            active_keys,
            trashed_keys,
            exact_match,
        },
    );
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

pub fn get_data_path() -> PathBuf {
    std::env::var("KEVA_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA not set")
                .join("keva")
        })
}
