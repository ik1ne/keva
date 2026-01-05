//! IDropTarget implementation for drag-drop interception.
//!
//! In CompositionController mode, we register our own IDropTarget to intercept
//! file drops from Explorer. This allows us to extract file paths via CF_HDROP
//! before forwarding the drop event to WebView2.

use crate::platform::handlers::get_resize_border;
use crate::webview::WEBVIEW;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::RwLock;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CompositionController3;
use windows::Win32::Foundation::{HWND, POINT, POINTL};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
    ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::core::{Interface, Ref};

/// Cached file paths from the most recent drag operation.
/// Indexed by order (0, 1, 2, ...) to match File objects in JS.
static DROPPED_PATHS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Retrieves and clears the cached dropped file paths.
pub fn take_dropped_paths() -> Vec<PathBuf> {
    std::mem::take(&mut *DROPPED_PATHS.write().unwrap())
}

/// Our IDropTarget implementation that intercepts drag-drop and forwards to WebView2.
#[windows_core::implement(IDropTarget)]
pub struct DropTarget {
    hwnd: HWND,
}

impl DropTarget {
    /// Converts screen coordinates to WebView client coordinates.
    fn to_webview_point(&self, pt: &POINTL) -> POINT {
        let mut point = POINT { x: pt.x, y: pt.y };
        let _ = unsafe { ScreenToClient(self.hwnd, &mut point) };

        // Adjust for resize border offset
        let (border_x, border_y) = get_resize_border();
        point.x -= border_x;
        point.y -= border_y;
        point
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<'_, IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // Forward to WebView2 CompositionController3
        if let Some(wv) = WEBVIEW.get()
            && let Ok(cc3) =
                wv.composition_controller.cast::<ICoreWebView2CompositionController3>()
        {
            let point = self.to_webview_point(pt);
            let data_obj_opt = pdataobj.ok().ok();
            let _ = unsafe {
                cc3.DragEnter(data_obj_opt, grfkeystate.0, point, pdweffect as *mut u32)
            };
        }

        // If pdweffect is still unset, default to copy
        unsafe {
            if (*pdweffect).0 == 0 {
                *pdweffect = DROPEFFECT_COPY;
            }
        }

        Ok(())
    }

    fn DragOver(
        &self,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // Forward to WebView2 CompositionController3
        if let Some(wv) = WEBVIEW.get()
            && let Ok(cc3) =
                wv.composition_controller.cast::<ICoreWebView2CompositionController3>()
        {
            let point = self.to_webview_point(pt);
            let _ = unsafe { cc3.DragOver(grfkeystate.0, point, pdweffect as *mut u32) };
        }

        // If pdweffect is still unset, default to copy
        unsafe {
            if (*pdweffect).0 == 0 {
                *pdweffect = DROPEFFECT_COPY;
            }
        }

        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        // Forward to WebView2 CompositionController3
        if let Some(wv) = WEBVIEW.get()
            && let Ok(cc3) =
                wv.composition_controller.cast::<ICoreWebView2CompositionController3>()
        {
            let _ = unsafe { cc3.DragLeave() };
        }

        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Ref<'_, IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // Extract and cache file paths from the data object
        if let Ok(data_obj) = pdataobj.ok() {
            let paths = extract_paths_from_hdrop(data_obj);
            *DROPPED_PATHS.write().unwrap() = paths;
        }

        // Forward to WebView2 CompositionController3
        if let Some(wv) = WEBVIEW.get()
            && let Ok(cc3) =
                wv.composition_controller.cast::<ICoreWebView2CompositionController3>()
        {
            let point = self.to_webview_point(pt);
            let data_obj_opt = pdataobj.ok().ok();
            let _ =
                unsafe { cc3.Drop(data_obj_opt, grfkeystate.0, point, pdweffect as *mut u32) };
        }

        // If pdweffect is still unset, default to copy
        unsafe {
            if (*pdweffect).0 == 0 {
                *pdweffect = DROPEFFECT_COPY;
            }
        }

        Ok(())
    }
}

/// Extracts file paths from an IDataObject via CF_HDROP format.
fn extract_paths_from_hdrop(data_obj: &IDataObject) -> Vec<PathBuf> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let mut medium = match unsafe { data_obj.GetData(&format) } {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    let hdrop = HDROP(unsafe { medium.u.hGlobal.0 } as *mut _);
    let count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };

    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        // First call to get required buffer size
        let len = unsafe { DragQueryFileW(hdrop, i, None) };
        if len == 0 {
            continue;
        }

        // Second call to get the actual path
        let mut buf = vec![0u16; (len + 1) as usize];
        let actual_len = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) };
        if actual_len > 0 {
            // Trim to actual length (excluding null terminator)
            buf.truncate(actual_len as usize);
            let path = PathBuf::from(OsString::from_wide(&buf));
            paths.push(path);
        }
    }

    unsafe { ReleaseStgMedium(&mut medium) };
    paths
}

/// Registers the window as a drop target.
pub fn register_drop_target(hwnd: HWND) -> windows::core::Result<()> {
    let target = DropTarget { hwnd };
    let target_interface: IDropTarget = target.into();
    unsafe { RegisterDragDrop(hwnd, &target_interface) }
}

/// Unregisters the window as a drop target.
pub fn revoke_drop_target(hwnd: HWND) {
    let _ = unsafe { RevokeDragDrop(hwnd) };
}
