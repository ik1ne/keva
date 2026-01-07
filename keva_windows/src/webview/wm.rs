//! Custom window messages (WM_APP + N).
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// Posted by worker when shutdown is complete.
pub const SHUTDOWN_COMPLETE: u32 = WM_APP + 1;

/// Posted by worker to send OutgoingMessage to WebView.
/// LPARAM contains a Box<OutgoingMessage> pointer.
/// Value variant uses PostWebMessageAsJsonWithAdditionalObjects for FileSystemHandle.
pub const WEBVIEW_MESSAGE: u32 = WM_APP + 2;

/// Posted by bridge to open file picker on UI thread.
/// LPARAM contains a Box<FilePickerRequest> pointer.
pub const OPEN_FILE_PICKER: u32 = WM_APP + 3;
