//! Custom window messages (WM_APP + N).
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// Posted by worker when shutdown is complete.
pub const SHUTDOWN_COMPLETE: u32 = WM_APP + 1;

/// Posted by forwarder to marshal PostWebMessageAsJson to UI thread.
/// LPARAM contains a Box<String> pointer to the JSON message.
pub const WEBVIEW_MESSAGE: u32 = WM_APP + 2;

/// Posted by worker to send FileSystemHandle to WebView.
/// LPARAM contains a Box<FileHandleRequest> pointer.
pub const SEND_FILE_HANDLE: u32 = WM_APP + 3;
