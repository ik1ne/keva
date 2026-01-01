//! Window message handlers.

use crate::platform::tray::{
    IDM_LAUNCH_AT_LOGIN, IDM_QUIT, IDM_SETTINGS, IDM_SHOW, remove_tray_icon, show_tray_menu,
};
use crate::render::theme::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Theme};
use crate::webview::WEBVIEW;
use crate::webview::bridge::post_message;
use crate::webview::messages::OutgoingMessage;
use std::sync::atomic::{AtomicIsize, AtomicU8, Ordering};
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
        RDW_INVALIDATE, RedrawWindow,
    },
    UI::{
        HiDpi::GetDpiForSystem,
        Input::KeyboardAndMouse::VK_ESCAPE,
        WindowsAndMessaging::{
            GetClientRect, GetSystemMetrics, GetWindowRect, IsWindowVisible, MINMAXINFO,
            NCCALCSIZE_PARAMS, PostMessageW, PostQuitMessage, SM_CXPADDEDBORDER, SM_CXSIZEFRAME,
            SM_CYSIZEFRAME, SW_HIDE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER,
            SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow,
            USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_LBUTTONUP, WM_RBUTTONUP, WVR_VALIDRECTS,
        },
    },
};
use windows::core::PCWSTR;
use windows_strings::w;

/// Stores the previously focused window handle to restore focus on Esc.
pub static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

static CURRENT_THEME: AtomicU8 = AtomicU8::new(0);

pub fn set_current_theme(theme: Theme) {
    let value = match theme {
        Theme::Dark => 0,
        Theme::Light => 1,
    };
    CURRENT_THEME.store(value, Ordering::Relaxed);
}

fn get_current_theme() -> Theme {
    match CURRENT_THEME.load(Ordering::Relaxed) {
        0 => Theme::Dark,
        _ => Theme::Light,
    }
}

pub fn scale_for_dpi(logical: i32, dpi: u32) -> i32 {
    (logical as i64 * dpi as i64 / USER_DEFAULT_SCREEN_DPI as i64) as i32
}

/// Returns system resize border size (includes padding for touch targets).
pub fn get_resize_border() -> (i32, i32) {
    unsafe {
        let padded = GetSystemMetrics(SM_CXPADDEDBORDER);
        let border_x = GetSystemMetrics(SM_CXSIZEFRAME) + padded;
        let border_y = GetSystemMetrics(SM_CYSIZEFRAME) + padded;
        (border_x, border_y)
    }
}

/// WM_CREATE: Enable rounded corners and trigger frame recalculation.
pub fn on_create(hwnd: HWND) -> LRESULT {
    unsafe {
        // Windows 11 rounded corners
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            size_of_val(&preference) as u32,
        );

        // SWP_FRAMECHANGED triggers WM_NCCALCSIZE to set up borderless frame
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        );
    }
    LRESULT(0)
}

/// WM_GETMINMAXINFO: Enforce minimum window size during resize.
pub fn on_getminmaxinfo(lparam: LPARAM) -> LRESULT {
    // lparam points to MINMAXINFO struct
    let info = lparam.0 as *mut MINMAXINFO;
    if !info.is_null() {
        unsafe {
            let dpi = GetDpiForSystem();
            (*info).ptMinTrackSize.x = scale_for_dpi(MIN_WINDOW_WIDTH, dpi);
            (*info).ptMinTrackSize.y = scale_for_dpi(MIN_WINDOW_HEIGHT, dpi);
        }
    }
    LRESULT(0)
}

/// WM_NCCALCSIZE: Remove non-client area to create borderless window.
pub fn on_nccalcsize(wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // wparam == 0: simple request, just return 0 to remove non-client area
    if wparam.0 == 0 {
        return LRESULT(0);
    }

    // wparam != 0: detailed request during resize
    // Nullify source/dest rectangles to prevent BitBlt artifacts when resizing
    let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
    if !params.is_null() {
        unsafe {
            // rgrc[0] = new client area (keep as-is = full window)
            // rgrc[1], rgrc[2] = old client/window areas, set to 1px to disable BitBlt
            (*params).rgrc[1] = RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            };
            (*params).rgrc[2] = (*params).rgrc[1];
        }
    }
    // WVR_VALIDRECTS: we've set valid rectangles, don't need system to calculate
    LRESULT(WVR_VALIDRECTS as isize)
}

/// WM_ACTIVATE: Track previously focused window to restore on Esc.
pub fn on_activate(wparam: WPARAM, lparam: LPARAM) {
    let activating = (wparam.0 & 0xFFFF) != 0;
    let previous_window = lparam.0;
    if activating && previous_window != 0 {
        PREV_FOREGROUND.store(previous_window, Ordering::Relaxed);
    }
}

/// WM_KEYDOWN: Hide window on Esc, restoring focus to previous window.
pub fn on_keydown(hwnd: HWND, wparam: WPARAM) -> Option<LRESULT> {
    let virtual_key = wparam.0 as u16;
    if virtual_key == VK_ESCAPE.0 {
        let prev = PREV_FOREGROUND.load(Ordering::Relaxed);
        unsafe {
            if prev != 0 {
                let _ = SetForegroundWindow(HWND(prev as *mut _));
            }
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        return Some(LRESULT(0));
    }
    None
}

/// WM_TRAYICON: Handle system tray icon clicks.
pub fn on_trayicon(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    // Low word of lparam contains the mouse message
    let mouse_msg = (lparam.0 & 0xFFFF) as u32;
    unsafe {
        if mouse_msg == WM_LBUTTONUP {
            if IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
        } else if mouse_msg == WM_RBUTTONUP {
            show_tray_menu(hwnd);
        }
    }
    LRESULT(0)
}

/// WM_COMMAND: Handle menu commands from tray context menu.
pub fn on_command(hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let cmd_id = (wparam.0 & 0xFFFF) as u32;
    unsafe {
        match cmd_id {
            IDM_SHOW => {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            IDM_SETTINGS => {}
            IDM_LAUNCH_AT_LOGIN => {}
            IDM_QUIT => {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            _ => {}
        }
    }
    LRESULT(0)
}

/// WM_SIZE: Resize WebView to match window, accounting for borders.
pub fn on_size(wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let size_type = wparam.0 as u32;
    let width = (lparam.0 & 0xFFFF) as i32;
    let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

    // SIZE_MAXIMIZED = 2: window is maximized or snapped (no visible borders)
    let is_maximized = size_type == 2;

    if let Some(wv) = WEBVIEW.get() {
        let (x, y, w, h) = if is_maximized {
            (0, 0, width, height)
        } else {
            let (border_x, border_y) = get_resize_border();
            (
                border_x,
                border_y,
                width - 2 * border_x,
                height - 2 * border_y,
            )
        };
        wv.set_bounds(x, y, w, h);
    }

    LRESULT(0)
}

/// WM_PAINT: Paint border regions around WebView.
pub fn on_paint(hwnd: HWND) -> LRESULT {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let bg_color = match get_current_theme() {
            Theme::Dark => COLORREF(0x001a1a1a),
            Theme::Light => COLORREF(0x00ffffff),
        };
        let brush = CreateSolidBrush(bg_color);

        let mut client_rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut client_rect);

        // Paint entire client area; WebView renders on top
        let left = RECT {
            left: 0,
            top: 0,
            right: client_rect.right,
            bottom: client_rect.bottom,
        };
        FillRect(hdc, &left, brush);

        let _ = DeleteObject(brush.into());
        let _ = EndPaint(hwnd, &ps);
    }
    LRESULT(0)
}

/// WM_DESTROY: Clean up and exit application.
pub fn on_destroy(hwnd: HWND) -> LRESULT {
    unsafe {
        remove_tray_icon(hwnd);
        PostQuitMessage(0);
    }
    LRESULT(0)
}

/// WM_SETTINGCHANGE: Detect system theme changes.
pub fn on_settingchange(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    // lparam points to the setting name as a wide string
    if lparam.0 != 0 {
        let setting_ptr = lparam.0 as *const u16;
        let setting = PCWSTR::from_raw(setting_ptr);

        // "ImmersiveColorSet" is broadcast when system theme changes
        if unsafe { setting.as_wide() == w!("ImmersiveColorSet").as_wide() } {
            let theme = Theme::detect_system();
            eprintln!("[Native] System theme changed: {:?}", theme);

            set_current_theme(theme);
            unsafe {
                let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE);
            }

            if let Some(wv) = WEBVIEW.get() {
                let msg = OutgoingMessage::Theme {
                    theme: theme.as_str().to_string(),
                };
                post_message(&wv.webview, &msg);
            }
        }
    }
    LRESULT(0)
}

/// WM_WEBVIEW_MESSAGE: Forward JSON message to WebView (marshaled from forwarder thread).
pub fn on_webview_message(lparam: LPARAM) -> LRESULT {
    // LPARAM contains a Box<String> pointer from the forwarder thread
    let ptr = lparam.0 as *mut String;
    if ptr.is_null() {
        return LRESULT(0);
    }

    // Reconstruct the Box and take ownership (will be dropped at end of scope)
    let json = unsafe { Box::from_raw(ptr) };

    if let Some(wv) = WEBVIEW.get() {
        // Keep Vec alive until PostWebMessageAsJson returns
        let wide: Vec<u16> = json.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_pwstr = windows::core::PWSTR(wide.as_ptr() as *mut u16);
        let _ = unsafe { wv.webview.PostWebMessageAsJson(msg_pwstr) };
    }

    LRESULT(0)
}
