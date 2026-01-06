//! DragStarting event handler for dragging attachments to external apps.
//!
//! When the user drags an attachment from the attachments pane, WebView2 fires
//! a DragStarting event. We intercept this to create a shell data object
//! containing the actual blob file paths, enabling drag-drop to Explorer/email/etc.

use crate::keva_worker::get_data_path;
use keva_core::core::KevaCore;
use keva_core::types::Key;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::LazyLock;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DragStartingEventArgs;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, S_OK,
};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILClone, ILCreateFromPathW, ILFindLastID, ILFree, ILRemoveLastID, SHCreateDataObject,
};
use windows_core::{BOOL, HRESULT, implement, w};

/// Clipboard format for Chromium's custom MIME data wrapper.
static CHROMIUM_CUSTOM_CF: LazyLock<u16> = LazyLock::new(|| {
    let cf = unsafe { RegisterClipboardFormatW(w!("Chromium Web Custom MIME Data Format")) };
    debug_assert!(cf != 0, "Failed to register clipboard format");
    cf as u16
});

#[derive(Debug, serde::Deserialize)]
struct DragData {
    key: String,
    filenames: Vec<String>,
}

/// Creates a shell data object for file drag-drop using SHCreateDataObject.
/// This provides CF_HDROP plus all the shell formats that Explorer expects.
fn create_shell_data_object(paths: &[PathBuf]) -> windows::core::Result<IDataObject> {
    if paths.is_empty() {
        return Err(windows::core::Error::empty());
    }

    let mut full_pidls: Vec<*mut ITEMIDLIST> = Vec::new();
    for path in paths {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let pidl = unsafe { ILCreateFromPathW(windows::core::PCWSTR(wide.as_ptr())) };
        if !pidl.is_null() {
            full_pidls.push(pidl);
        }
    }

    if full_pidls.is_empty() {
        return Err(windows::core::Error::empty());
    }

    // Parent folder PIDL: clone first item and remove the filename component
    let parent_pidl = unsafe { ILClone(full_pidls[0]) };
    if parent_pidl.is_null() {
        for pidl in &full_pidls {
            unsafe { ILFree(Some(*pidl as *const _)) };
        }
        return Err(windows::core::Error::empty());
    }
    let _ = unsafe { ILRemoveLastID(Some(parent_pidl)) };

    // Child PIDLs point to the last item ID in each full PIDL (relative to parent)
    let child_pidls: Vec<*const ITEMIDLIST> = full_pidls
        .iter()
        .map(|&pidl| unsafe { ILFindLastID(pidl) as *const ITEMIDLIST })
        .collect();

    let result = unsafe {
        SHCreateDataObject(
            Some(parent_pidl as *const ITEMIDLIST),
            Some(&child_pidls),
            None,
        )
    };

    unsafe { ILFree(Some(parent_pidl as *const _)) };
    for pidl in &full_pidls {
        unsafe { ILFree(Some(*pidl as *const _)) };
    }

    result
}

/// Handles the DragStarting event from WebView2.
///
/// Returns true if we handled the drag (caller should set Handled=true),
/// false if we should let WebView2 handle it normally.
pub fn handle_drag_starting(
    args: &ICoreWebView2DragStartingEventArgs,
) -> windows::core::Result<bool> {
    let data_obj = unsafe { args.Data()? };

    let Some(drag_data) = extract_attachment_drag_data(&data_obj) else {
        return Ok(false);
    };

    let Ok(key) = Key::try_from(drag_data.key.as_str()) else {
        return Ok(false);
    };

    let data_path = get_data_path();
    let paths: Vec<PathBuf> = drag_data
        .filenames
        .iter()
        .map(|filename| KevaCore::attachment_blob_path(&data_path, &key, filename))
        .filter(|p| p.exists())
        .collect();

    if paths.is_empty() {
        return Ok(false);
    }

    let data_obj = create_shell_data_object(&paths)?;
    let drop_source: IDropSource = SimpleDropSource.into();

    let mut effect = DROPEFFECT_NONE;
    let _ = unsafe { DoDragDrop(&data_obj, &drop_source, DROPEFFECT_COPY, &mut effect) };

    Ok(true)
}

/// Extracts attachment drag data from IDataObject if present.
fn extract_attachment_drag_data(data_obj: &IDataObject) -> Option<DragData> {
    let cf = *CHROMIUM_CUSTOM_CF;
    if cf == 0 {
        return None;
    }

    let formatetc = FORMATETC {
        cfFormat: cf,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let medium = unsafe { data_obj.GetData(&formatetc) }.ok()?;

    let hglobal = unsafe { medium.u.hGlobal };
    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return None;
    }

    let result = parse_chromium_custom_data(ptr);

    let _ = unsafe { GlobalUnlock(hglobal) };

    result
}

/// Parses Chromium's custom MIME data format to extract our attachment data.
///
/// Format (pickle): map_size:u64, then for each entry:
///   key_len:u32, key:utf16[key_len], padding, value_len:u32, value:utf16[value_len], padding
fn parse_chromium_custom_data(ptr: *mut std::ffi::c_void) -> Option<DragData> {
    const TARGET_MIME: &str = "application/x-keva-attachments";

    let data = ptr as *const u8;

    unsafe {
        let map_size = *(data as *const u64);
        let mut offset = 8usize;

        let mut read_string = || {
            let len = *(data.add(offset) as *const u32) as usize;
            offset += 4;
            let slice = std::slice::from_raw_parts(data.add(offset) as *const u16, len);
            let s = String::from_utf16_lossy(slice);
            offset += len * 2;
            offset = (offset + 3) & !3;
            s
        };

        for _ in 0..map_size {
            let key = read_string();
            let value = read_string();

            if key == TARGET_MIME {
                return serde_json::from_str(&value).ok();
            }
        }
    }

    None
}

/// Simple IDropSource implementation.
#[implement(IDropSource)]
struct SimpleDropSource;

impl IDropSource_Impl for SimpleDropSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}
