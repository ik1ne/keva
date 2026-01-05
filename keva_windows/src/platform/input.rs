//! Input forwarding for CompositionController mode.
//!
//! In CompositionController mode, the parent window receives all input events
//! and must forward them to WebView2 via SendMouseInput/SendPointerInput.
//!
//! The COREWEBVIEW2_MOUSE_EVENT_KIND values match WM_* message IDs directly.
//! The COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS values match MK_* flags directly.
//! The COREWEBVIEW2_POINTER_EVENT_KIND values match WM_POINTER* message IDs directly.

use crate::platform::handlers::get_resize_border;
use crate::webview::WEBVIEW;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
    COREWEBVIEW2_POINTER_EVENT_KIND, ICoreWebView2Environment3,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Input::Pointer::{GetPointerInfo, POINTER_INFO};
use windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL;
use windows::core::Interface;

/// Forwards a mouse message to WebView2's CompositionController.
pub fn forward_mouse_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<()> {
    let wv = WEBVIEW.get()?;

    // Extract coordinates from lparam (signed 16-bit values)
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let mut point = POINT { x, y };

    // WM_MOUSEWHEEL coordinates are in screen space - convert to client
    if msg == WM_MOUSEWHEEL {
        let _ = unsafe { ScreenToClient(hwnd, &mut point) };
    }

    // Adjust for resize border offset (Keva uses borderless window with resize borders)
    let (border_x, border_y) = get_resize_border();
    point.x -= border_x;
    point.y -= border_y;

    let event_kind = COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32);
    let virtual_keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS((wparam.0 & 0xFFFF) as i32);
    let mouse_data = ((wparam.0 >> 16) & 0xFFFF) as u32;

    unsafe {
        wv.composition_controller
            .SendMouseInput(event_kind, virtual_keys, mouse_data, point)
            .ok()
    }
}

/// Forwards a pointer message (touch/pen) to WebView2's CompositionController.
///
/// Only populates fields needed for basic touch gestures (tap, drag, scroll).
/// Omitted: pen tilt/pressure, touch orientation/pressure, himetric coords.
pub fn forward_pointer_message(hwnd: HWND, msg: u32, wparam: WPARAM) -> Option<()> {
    let wv = WEBVIEW.get()?;
    let pointer_id = (wparam.0 & 0xFFFF) as u32;

    let mut pointer_info = POINTER_INFO::default();
    unsafe { GetPointerInfo(pointer_id, &mut pointer_info) }.ok()?;

    let env3 = wv.env.cast::<ICoreWebView2Environment3>().ok()?;
    let wv_pointer_info = unsafe { env3.CreateCoreWebView2PointerInfo() }.ok()?;

    // Convert screen coordinates to WebView client coordinates
    let mut point = pointer_info.ptPixelLocation;
    let _ = unsafe { ScreenToClient(hwnd, &mut point) };
    let (border_x, border_y) = get_resize_border();
    point.x -= border_x;
    point.y -= border_y;

    unsafe {
        let _ = wv_pointer_info.SetPointerKind(pointer_info.pointerType.0 as u32);
        let _ = wv_pointer_info.SetPointerId(pointer_info.pointerId);
        let _ = wv_pointer_info.SetFrameId(pointer_info.frameId);
        let _ = wv_pointer_info.SetPointerFlags(pointer_info.pointerFlags.0);
        let _ = wv_pointer_info.SetPixelLocation(point);
        let _ = wv_pointer_info.SetPixelLocationRaw(point);
        let _ = wv_pointer_info.SetTime(pointer_info.dwTime);
    }

    let event_kind = COREWEBVIEW2_POINTER_EVENT_KIND(msg as i32);

    unsafe {
        wv.composition_controller
            .SendPointerInput(event_kind, &wv_pointer_info)
            .ok()
    }
}
