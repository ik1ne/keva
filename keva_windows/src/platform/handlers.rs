//! Window message handlers.

use crate::app::App;
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
            GWLP_USERDATA, GetClientRect, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
            IsWindowVisible, MINMAXINFO, NCCALCSIZE_PARAMS, PostMessageW, PostQuitMessage,
            SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SW_HIDE, SW_SHOW, SWP_FRAMECHANGED,
            SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow,
            SetWindowPos, ShowWindow, USER_DEFAULT_SCREEN_DPI, WM_CLOSE, WM_LBUTTONUP,
            WM_RBUTTONUP, WVR_VALIDRECTS,
        },
    },
};
use windows::core::PCWSTR;
use windows_strings::w;

/// Stores the previously focused window to restore on Esc.
pub static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Stores the current theme (0 = Dark, 1 = Light).
static CURRENT_THEME: AtomicU8 = AtomicU8::new(0);

/// Sets the current theme for border painting.
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

/// Scales a logical pixel value to physical pixels based on system DPI.
pub fn scale_for_dpi(logical: i32, dpi: u32) -> i32 {
    (logical as i64 * dpi as i64 / USER_DEFAULT_SCREEN_DPI as i64) as i32
}

/// Returns the resize border size using system metrics.
pub fn get_resize_border() -> (i32, i32) {
    unsafe {
        let padded = GetSystemMetrics(SM_CXPADDEDBORDER);
        let border_x = GetSystemMetrics(SM_CXSIZEFRAME) + padded;
        let border_y = GetSystemMetrics(SM_CYSIZEFRAME) + padded;
        (border_x, border_y)
    }
}

/// Gets the App instance from the window's user data.
///
/// # Safety
///
/// Caller must ensure only one mutable reference exists at a time.
pub unsafe fn get_app(hwnd: HWND) -> Option<&'static mut App> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

/// WM_CREATE: Enable rounded corners and trigger frame update.
pub fn on_create(hwnd: HWND) -> LRESULT {
    unsafe {
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            size_of_val(&preference) as u32,
        );

        // Trigger WM_NCCALCSIZE to properly set up the borderless frame
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

/// WM_GETMINMAXINFO: Enforce minimum window size.
pub fn on_getminmaxinfo(lparam: LPARAM) -> LRESULT {
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

/// WM_NCCALCSIZE: Implement borderless window.
pub fn on_nccalcsize(wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if wparam.0 == 0 {
        // wparam == FALSE: Just return 0 to remove non-client area
        return LRESULT(0);
    }

    // wparam == TRUE: Window is being resized.

    // Nullify the source/dest rectangles to prevent BitBlt jitter
    // when resizing from top/left edges.
    let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
    if !params.is_null() {
        unsafe {
            // rgrc[0] stays as-is (new client area = full window)
            // rgrc[1] and rgrc[2] are set to same 1px rect to nullify BitBlt
            (*params).rgrc[1] = RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            };
            (*params).rgrc[2] = (*params).rgrc[1];
        }
    }
    LRESULT(WVR_VALIDRECTS as isize)
}

/// WM_ACTIVATE: Store previous foreground window.
pub fn on_activate(wparam: WPARAM, lparam: LPARAM) {
    let activating = (wparam.0 & 0xFFFF) != 0;
    let previous_window = lparam.0;
    if activating && previous_window != 0 {
        PREV_FOREGROUND.store(previous_window, Ordering::Relaxed);
    }
}

/// WM_KEYDOWN: Handle Escape to hide window.
/// Returns Some(LRESULT) if handled, None to delegate to DefWindowProcW.
pub fn on_keydown(hwnd: HWND, wparam: WPARAM) -> Option<LRESULT> {
    let virtual_key = wparam.0 as u16;
    if virtual_key == VK_ESCAPE.0 {
        // Restore focus to previous window before hiding
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

/// WM_TRAYICON: Handle tray icon clicks.
pub fn on_trayicon(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let mouse_msg = (lparam.0 & 0xFFFF) as u32;
    unsafe {
        if mouse_msg == WM_LBUTTONUP {
            // Toggle window visibility on left click
            if IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
        } else if mouse_msg == WM_RBUTTONUP {
            // Show context menu on right click
            show_tray_menu(hwnd);
        }
    }
    LRESULT(0)
}

/// WM_COMMAND: Handle menu commands.
pub fn on_command(hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let cmd_id = (wparam.0 & 0xFFFF) as u32;
    unsafe {
        match cmd_id {
            IDM_SHOW => {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            IDM_SETTINGS => {
                // Non-functional until M15-win
            }
            IDM_LAUNCH_AT_LOGIN => {
                // Non-functional until M20-win
            }
            IDM_QUIT => {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            _ => {}
        }
    }
    LRESULT(0)
}

/// WM_SIZE: Resize WebView to match window.
pub fn on_size(wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let size_type = wparam.0 as u32;
    let width = (lparam.0 & 0xFFFF) as i32;
    let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

    // SIZE_MAXIMIZED = 2: window is maximized or snapped
    let is_maximized = size_type == 2;

    // Resize WebView with border insets (none when maximized)
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

        // Get border color based on current theme
        let bg_color = match get_current_theme() {
            Theme::Dark => COLORREF(0x001a1a1a),  // #1a1a1a
            Theme::Light => COLORREF(0x00ffffff), // #ffffff
        };
        let brush = CreateSolidBrush(bg_color);

        // Get client rect and border sizes
        let mut client_rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut client_rect);

        // Paint the whole client area, since WebView renders over the window background
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

/// WM_DESTROY: Clean up app state.
pub fn on_destroy(hwnd: HWND) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if !ptr.is_null() {
            drop(Box::from_raw(ptr));
        }
        remove_tray_icon(hwnd);
        PostQuitMessage(0);
    }
    LRESULT(0)
}

/// WM_KEVA_RESPONSE: Forward worker responses to WebView.
///
/// # Safety
///
/// Should not be called concurrently.
pub unsafe fn on_keva_response(hwnd: HWND) -> LRESULT {
    // SAFETY: Called from wndproc which is single-threaded
    let Some(app) = (unsafe { get_app(hwnd) }) else {
        return LRESULT(0);
    };
    let Some(wv) = WEBVIEW.get() else {
        return LRESULT(0);
    };

    while let Ok(response) = app.response_rx.try_recv() {
        post_message(&wv.webview, &response);
    }

    LRESULT(0)
}

/// WM_SETTINGCHANGE: Detect system theme changes.
pub fn on_settingchange(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    // lparam points to a wide string (PCWSTR) with the setting name
    if lparam.0 != 0 {
        let setting_ptr = lparam.0 as *const u16;
        let setting = PCWSTR::from_raw(setting_ptr);

        if unsafe { setting.as_wide() == w!("ImmersiveColorSet").as_wide() } {
            let theme = Theme::detect_system();
            eprintln!("[Native] System theme changed: {:?}", theme);

            // Update stored theme and trigger repaint
            set_current_theme(theme);
            unsafe {
                let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE);
            }

            // Send theme to WebView
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
