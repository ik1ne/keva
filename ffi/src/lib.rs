mod messages;
mod worker;

use messages::IncomingMessage;
use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Opaque handle to the worker thread.
struct WorkerHandle {
    tx: Sender<IncomingMessage>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Start the worker thread.
///
/// `data_dir` is a UTF-8 null-terminated path to the data directory.
/// `callback` is called from the worker thread with each JSON response.
/// `context` is passed through to the callback unchanged.
///
/// Returns an opaque handle. The caller must call `keva_worker_stop` to free it.
///
/// # Safety
///
/// - `data_dir` must be a valid null-terminated C string.
/// - `context` must remain valid until `keva_worker_stop` returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn keva_worker_start(
    data_dir: *const c_char,
    callback: unsafe extern "C" fn(*mut c_void, *const c_char),
    context: *mut c_void,
) -> *mut c_void {
    let data_dir = unsafe { CStr::from_ptr(data_dir) };
    let data_dir = PathBuf::from(data_dir.to_string_lossy().as_ref());

    let (tx, thread) = worker::start(data_dir, callback, context);

    let handle = Box::new(WorkerHandle {
        tx,
        thread: Some(thread),
    });
    Box::into_raw(handle) as *mut c_void
}

/// Send a JSON request to the worker thread.
///
/// `json` is a null-terminated UTF-8 JSON string. The worker will parse it and
/// process the request.
///
/// # Safety
///
/// - `handle` must be a valid pointer returned by `keva_worker_start`.
/// - `json` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn keva_worker_send(handle: *mut c_void, json: *const c_char) {
    let handle = unsafe { &*(handle as *const WorkerHandle) };
    let json = unsafe { CStr::from_ptr(json) };
    let json = json.to_string_lossy();

    let Ok(msg) = serde_json::from_str::<IncomingMessage>(&json) else {
        return;
    };

    let _ = handle.tx.send(msg);
}

/// Stop the worker thread and free the handle.
///
/// Sends a shutdown request and blocks until the worker thread exits.
///
/// # Safety
///
/// - `handle` must be a valid pointer returned by `keva_worker_start`.
/// - Must be called exactly once per handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn keva_worker_stop(handle: *mut c_void) {
    let mut handle = unsafe { Box::from_raw(handle as *mut WorkerHandle) };
    let _ = handle.tx.send(IncomingMessage::ShutdownAck);
    if let Some(thread) = handle.thread.take() {
        let _ = thread.join();
    }
}
