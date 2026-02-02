//! Background worker thread for KevaCore and SearchEngine operations.

use crate::messages::{
    AttachmentInfo, ExactMatch, IncomingMessage, OutgoingMessage, RenameResultType,
};
use keva_core::core::KevaCore;
use keva_core::types::{Config, GcConfig, Key, LifecycleConfig, LifecycleState};
use keva_search::{SearchConfig, SearchEngine, SearchQuery};
use std::ffi::{CString, c_char, c_void};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime};

/// Callback + context bundle for sending responses from the worker thread.
///
/// SAFETY: The caller of `start` guarantees the context pointer is valid for
/// the lifetime of the worker thread (until `keva_worker_stop` returns).
struct Responder {
    callback: unsafe extern "C" fn(context: *mut c_void, json: *const c_char),
    context: *mut c_void,
}

unsafe impl Send for Responder {}

impl Responder {
    fn send(&self, msg: &OutgoingMessage) {
        let Ok(json) = serde_json::to_string(msg) else {
            return;
        };
        let Ok(cstr) = CString::new(json) else {
            return;
        };
        unsafe {
            (self.callback)(self.context, cstr.as_ptr());
        }
    }
}

/// Starts the worker thread.
///
/// Returns the sender for dispatching messages and the thread join handle.
/// The callback is invoked from the worker thread with each JSON response.
pub fn start(
    data_dir: PathBuf,
    callback: unsafe extern "C" fn(*mut c_void, *const c_char),
    context: *mut c_void,
) -> (Sender<IncomingMessage>, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<IncomingMessage>();
    let notify_tx = tx.clone();

    let responder = Responder { callback, context };

    let handle = std::thread::spawn(move || {
        let gc_config = GcConfig::from(&LifecycleConfig::default());

        let keva = match open_keva(&data_dir) {
            Ok(keva) => keva,
            Err(e) => {
                responder.send(&OutgoingMessage::CoreInitFailed {
                    message: e,
                    data_dir: data_dir.to_string_lossy().into_owned(),
                });
                return;
            }
        };

        let active_keys = keva.active_keys().unwrap_or_default();
        let trashed_keys = keva.trashed_keys().unwrap_or_default();

        let notify = Arc::new(move || {
            let _ = notify_tx.send(IncomingMessage::SearchTick);
        });
        let search = SearchEngine::new(active_keys, trashed_keys, SearchConfig::default(), notify);

        worker_loop(keva, search, rx, gc_config, &responder);
    });

    (tx, handle)
}

fn worker_loop(
    mut keva: KevaCore,
    mut search: SearchEngine,
    rx: mpsc::Receiver<IncomingMessage>,
    gc_config: GcConfig,
    resp: &Responder,
) {
    let mut current_query = String::new();

    // Set empty query to trigger initial SearchResults
    search.set_query(SearchQuery::Fuzzy(String::new()));

    // Run maintenance on launch if needed
    handle_maintenance(&mut keva, &mut search, &current_query, false, gc_config, resp);
    let mut next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;

    loop {
        let timeout = next_maintenance.saturating_duration_since(Instant::now());
        let msg = match rx.recv_timeout(timeout) {
            Ok(msg) => Some(msg),
            Err(RecvTimeoutError::Timeout) => {
                handle_maintenance(
                    &mut keva,
                    &mut search,
                    &current_query,
                    false,
                    gc_config,
                    resp,
                );
                next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let Some(msg) = msg else { continue };

        match msg {
            IncomingMessage::Ready => {
                resp.send(&OutgoingMessage::CoreReady);
            }
            IncomingMessage::Select { key } => {
                handle_get_value(&mut keva, &key, resp);
            }
            IncomingMessage::Save { key, content } => {
                handle_save(&mut keva, &key, &content, resp);
            }
            IncomingMessage::Create { key } => {
                handle_create(&mut keva, &mut search, &key, &current_query, resp);
            }
            IncomingMessage::Rename {
                old_key,
                new_key,
                force,
            } => {
                handle_rename(&mut keva, &mut search, &old_key, &new_key, force, resp);
            }
            IncomingMessage::Trash { key } => {
                handle_trash(&mut keva, &mut search, &key, &current_query, resp);
            }
            IncomingMessage::Restore { key } => {
                handle_restore(&mut keva, &mut search, &key, &current_query, resp);
            }
            IncomingMessage::Purge { key } => {
                handle_purge(&mut keva, &mut search, &key, &current_query, resp);
            }
            IncomingMessage::Search { query } => {
                current_query = query.clone();
                search.set_query(SearchQuery::Fuzzy(query));
                search.tick();
                send_search_results(&search, &current_query, resp);
            }
            IncomingMessage::SearchTick => {
                if search.tick() {
                    send_search_results(&search, &current_query, resp);
                }
            }
            IncomingMessage::Touch { key } => {
                if let Ok(key) = Key::try_from(key.as_str()) {
                    let _ = keva.touch(&key, SystemTime::now());
                }
            }
            IncomingMessage::Maintenance { force } => {
                handle_maintenance(
                    &mut keva,
                    &mut search,
                    &current_query,
                    force,
                    gc_config,
                    resp,
                );
                next_maintenance = Instant::now() + MAINTENANCE_INTERVAL;
            }
            IncomingMessage::ShutdownAck => {
                break;
            }
        }
    }
}

/// Get value and read content from file.
fn handle_get_value(keva: &mut KevaCore, key_str: &str, resp: &Responder) {
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

    // Read content from file
    let content_path = keva.content_path(&key);
    let content = std::fs::read_to_string(&content_path).unwrap_or_default();

    // Build attachment info with thumbnail URLs
    let thumbnail_paths = keva.thumbnail_paths(&key).unwrap_or_default();
    let attachments: Vec<AttachmentInfo> = value
        .attachments
        .into_iter()
        .map(|att| {
            let thumbnail_url = thumbnail_paths.get(&att.filename).map(|rel_path| {
                format!(
                    "keva-app://thumbnails/{}",
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

    resp.send(&OutgoingMessage::Value {
        key: key_str.to_string(),
        key_hash,
        content,
        read_only,
        attachments,
    });
}

fn handle_save(keva: &mut KevaCore, key_str: &str, content: &str, resp: &Responder) {
    let Ok(key) = Key::try_from(key_str) else {
        resp.send(&OutgoingMessage::SaveFailed {
            key: key_str.to_string(),
            message: format!("Invalid key: '{key_str}'"),
        });
        return;
    };

    let content_path = keva.content_path(&key);
    if let Err(e) = std::fs::write(&content_path, content) {
        resp.send(&OutgoingMessage::SaveFailed {
            key: key_str.to_string(),
            message: format!("Write failed: {e}"),
        });
        return;
    }

    if let Err(e) = keva.touch(&key, SystemTime::now()) {
        eprintln!("Warning: failed to update timestamp for '{key_str}': {e}");
    }
}

fn handle_create(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    resp: &Responder,
) {
    let success = try_create(keva, search, key_str).is_some();

    resp.send(&OutgoingMessage::KeyCreated {
        key: key_str.to_string(),
        success,
    });

    if success {
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, resp);
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
    resp: &Responder,
) {
    let result = try_rename(keva, search, old_key_str, new_key_str, force);
    resp.send(&OutgoingMessage::RenameResult {
        old_key: old_key_str.to_string(),
        new_key: new_key_str.to_string(),
        result: result.unwrap_or_else(|e| e),
    });
}

fn try_rename(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    old_key_str: &str,
    new_key_str: &str,
    force: bool,
) -> Result<RenameResultType, RenameResultType> {
    let old_key = Key::try_from(old_key_str).map_err(|_| RenameResultType::InvalidKey)?;
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
    resp: &Responder,
) {
    let now = SystemTime::now();
    if let Ok(key) = Key::try_from(key_str)
        && keva.trash(&key, now).is_ok()
    {
        search.trash(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, resp);
    }
}

fn handle_restore(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    resp: &Responder,
) {
    let now = SystemTime::now();
    if let Ok(key) = Key::try_from(key_str)
        && keva.restore(&key, now).is_ok()
    {
        search.restore(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, resp);
    }
}

fn handle_purge(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    key_str: &str,
    current_query: &str,
    resp: &Responder,
) {
    if let Ok(key) = Key::try_from(key_str)
        && keva.purge(&key).is_ok()
    {
        search.remove(&key);
        search.set_query(SearchQuery::Fuzzy(current_query.to_string()));
        search.tick();
        send_search_results(search, current_query, resp);
    }
}

/// 24 hours interval for periodic maintenance check.
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

fn handle_maintenance(
    keva: &mut KevaCore,
    search: &mut SearchEngine,
    current_query: &str,
    force: bool,
    gc_config: GcConfig,
    resp: &Responder,
) {
    let now = SystemTime::now();

    if !force && !keva.should_run_maintenance(now, MAINTENANCE_INTERVAL) {
        return;
    }

    let Ok(outcome) = keva.maintenance(now, gc_config) else {
        return;
    };

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
        send_search_results(search, current_query, resp);
    }
}

fn send_search_results(search: &SearchEngine, current_query: &str, resp: &Responder) {
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

    resp.send(&OutgoingMessage::SearchResults {
        active_keys,
        trashed_keys,
        exact_match,
    });
}

fn open_keva(data_dir: &PathBuf) -> Result<KevaCore, String> {
    std::fs::create_dir_all(data_dir)
        .map_err(|e| format!("Failed to create data directory: {e}"))?;

    let config = Config {
        base_path: data_dir.clone(),
    };
    KevaCore::open(config).map_err(|e| e.to_string())
}
