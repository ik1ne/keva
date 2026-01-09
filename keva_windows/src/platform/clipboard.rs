//! Win32 clipboard operations.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::RwLock;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

const CF_UNICODETEXT: u32 = 13;

/// Cached file paths for pending add operation (from drop or clipboard).
static PENDING_FILE_PATHS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Clipboard content that was read.
pub struct ClipboardContent {
    pub text: Option<String>,
    pub files: Vec<PathBuf>,
}

/// Reads clipboard content (text and/or files).
pub fn read_clipboard(hwnd: HWND) -> ClipboardContent {
    let mut content = ClipboardContent {
        text: None,
        files: Vec::new(),
    };

    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return content;
        }

        // Try CF_UNICODETEXT
        if let Ok(handle) = GetClipboardData(CF_UNICODETEXT)
            && !handle.is_invalid()
        {
            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal) as *const u16;
            if !ptr.is_null() {
                let size = GlobalSize(hglobal);
                let max_len = size / 2;
                let slice = std::slice::from_raw_parts(ptr, max_len);
                let actual_len = slice.iter().position(|&c| c == 0).unwrap_or(max_len);
                content.text = Some(String::from_utf16_lossy(&slice[..actual_len]));
                let _ = GlobalUnlock(hglobal);
            }
        }

        // Try CF_HDROP
        if let Ok(handle) = GetClipboardData(CF_HDROP.0 as u32)
            && !handle.is_invalid()
        {
            let hdrop = HDROP(handle.0 as *mut _);
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; (len + 1) as usize];
                let actual_len = DragQueryFileW(hdrop, i, Some(&mut buf));
                if actual_len > 0 {
                    buf.truncate(actual_len as usize);
                    content.files.push(PathBuf::from(OsString::from_wide(&buf)));
                }
            }
        }

        let _ = CloseClipboard();
    }

    content
}

/// Stores file paths for later retrieval (used by both drop and clipboard paste).
pub fn set_pending_file_paths(paths: Vec<PathBuf>) {
    if let Ok(mut guard) = PENDING_FILE_PATHS.write() {
        *guard = paths;
    }
}

/// Takes cached file paths (clears the cache).
pub fn take_pending_file_paths() -> Vec<PathBuf> {
    PENDING_FILE_PATHS
        .write()
        .map(|mut guard| std::mem::take(&mut *guard))
        .unwrap_or_default()
}

/// Writes files to clipboard as CF_HDROP.
pub fn write_files(hwnd: HWND, paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return false;
    }

    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return false;
        }
        let _ = EmptyClipboard();

        // Build DROPFILES structure
        // Header (20 bytes) + null-terminated file paths + final double null
        let mut data: Vec<u8> = Vec::new();

        // DROPFILES header
        let header_size: u32 = 20;
        data.extend_from_slice(&header_size.to_le_bytes()); // pFiles offset
        data.extend_from_slice(&0i32.to_le_bytes()); // pt.x
        data.extend_from_slice(&0i32.to_le_bytes()); // pt.y
        data.extend_from_slice(&0u32.to_le_bytes()); // fNC
        data.extend_from_slice(&1u32.to_le_bytes()); // fWide = TRUE (Unicode)

        // File paths as null-terminated wide strings
        for path in paths {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            for c in wide {
                data.extend_from_slice(&c.to_le_bytes());
            }
        }
        // Final double null terminator
        data.extend_from_slice(&[0u8, 0u8]);

        let Ok(hglobal) = GlobalAlloc(GMEM_MOVEABLE, data.len()) else {
            let _ = CloseClipboard();
            return false;
        };

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return false;
        }

        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        let _ = GlobalUnlock(hglobal);

        let result = SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(hglobal.0))).is_ok();
        let _ = CloseClipboard();
        result
    }
}
