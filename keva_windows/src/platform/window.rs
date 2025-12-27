//! Window creation and message handling.

use crate::app::App;
use crate::platform::{
    hit_test::hit_test,
    tray::{
        IDM_LAUNCH_AT_LOGIN, IDM_QUIT, IDM_SETTINGS, IDM_SHOW, WM_TRAYICON, add_tray_icon,
        remove_tray_icon, show_tray_menu,
    },
};
use crate::render::theme::{
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, RESIZE_BORDER, WINDOW_HEIGHT, WINDOW_WIDTH,
};
use crate::templates::APP_HTML_W;
use crate::webview::init_webview;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Graphics::{
            Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
            Gdi::{CreateSolidBrush, ValidateRect},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::VK_ESCAPE,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW,
                GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, IDC_ARROW, IsWindowVisible,
                LoadCursorW, MINMAXINFO, MSG, NCCALCSIZE_PARAMS, PostQuitMessage, RegisterClassW,
                SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOCOPYBITS,
                SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow,
                SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, WINDOWPOS,
                WM_ACTIVATE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO,
                WM_KEYDOWN, WM_LBUTTONUP, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_PAINT,
                WM_RBUTTONUP, WM_SIZE, WM_WINDOWPOSCHANGING, WNDCLASSW, WS_CLIPCHILDREN,
                WS_EX_APPWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
                WS_SIZEBOX, WS_SYSMENU, WVR_VALIDRECTS,
            },
        },
    },
    core::{Result, w},
};

/// Stores the previously focused window to restore on Esc.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Runs the application.
pub fn run() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("KevaWindowClass");

        // Create a dark background brush for resize border areas
        let bg_brush = CreateSolidBrush(COLORREF(0x001a1a1a)); // #1a1a1a in BGR

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: bg_brush,
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0);

        // Borderless window with resize capability
        // WS_CLIPCHILDREN prevents painting over child windows
        let style =
            WS_POPUP | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN;

        let ex_style = WS_EX_APPWINDOW | WS_EX_TOPMOST;

        // Center window on screen
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - WINDOW_WIDTH) / 2;
        let y = (screen_height - WINDOW_HEIGHT) / 2;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("Keva"),
            style,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        // Create App
        let app = Box::new(App::new(hwnd));
        let app_ptr = Box::into_raw(app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize);

        // Create single WebView covering the entire window (with resize border insets on all sides)
        let wv_x = RESIZE_BORDER;
        let wv_y = RESIZE_BORDER;
        let wv_width = WINDOW_WIDTH - 2 * RESIZE_BORDER;
        let wv_height = WINDOW_HEIGHT - 2 * RESIZE_BORDER;

        init_webview(hwnd, wv_x, wv_y, wv_width, wv_height, move |wv| {
            wv.navigate_html(APP_HTML_W);
            if let Some(app) = get_app(hwnd) {
                app.state_mut().webview = Some(wv);
            }
        });

        // Create system tray icon
        add_tray_icon(hwnd)?;

        // Show window and bring to foreground
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

/// Gets the App instance from the window's user data.
///
/// # Safety
///
/// Caller must ensure only one mutable reference exists at a time.
/// Calling this twice without dropping the first reference is UB.
unsafe fn get_app(hwnd: HWND) -> Option<&'static mut App> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                // Enable rounded corners on Windows 11
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
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                // Enforce minimum window size
                let info = lparam.0 as *mut MINMAXINFO;
                if !info.is_null() {
                    (*info).ptMinTrackSize.x = MIN_WINDOW_WIDTH;
                    (*info).ptMinTrackSize.y = MIN_WINDOW_HEIGHT;
                }
                LRESULT(0)
            }
            WM_NCCALCSIZE => {
                if wparam.0 != 0 {
                    // wparam == TRUE: Window is being resized.
                    // Nullify the source/dest rectangles to prevent BitBlt jitter
                    // when resizing from top/left edges.
                    let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
                    if !params.is_null() {
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
                    return LRESULT(WVR_VALIDRECTS as isize);
                }
                // wparam == FALSE: Just return 0 to remove non-client area
                LRESULT(0)
            }
            WM_NCACTIVATE => {
                // Prevent default non-client area painting (gray border)
                // Return TRUE to indicate we handled it
                LRESULT(TRUE.0 as isize)
            }
            WM_WINDOWPOSCHANGING => {
                // Disable BitBlt during window position changes to prevent jitter
                let wp = lparam.0 as *mut WINDOWPOS;
                if !wp.is_null() {
                    (*wp).flags |= SWP_NOCOPYBITS;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_NCHITTEST => {
                let cursor_x = (lparam.0 & 0xFFFF) as i16 as i32;
                let cursor_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                hit_test(hwnd, cursor_x, cursor_y)
            }
            WM_ACTIVATE => {
                let activating = (wparam.0 & 0xFFFF) != 0;
                let previous_window = lparam.0;
                if activating && previous_window != 0 {
                    PREV_FOREGROUND.store(previous_window, Ordering::Relaxed);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ERASEBKGND => {
                // Let DefWindowProcW paint the background using hbrBackground brush
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN => {
                let virtual_key = wparam.0 as u16;
                if virtual_key == VK_ESCAPE.0 {
                    // Restore focus to previous window before hiding
                    let prev = PREV_FOREGROUND.load(Ordering::Relaxed);
                    if prev != 0 {
                        let _ = SetForegroundWindow(HWND(prev as *mut _));
                    }
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_TRAYICON => {
                // lparam contains the mouse message
                let mouse_msg = (lparam.0 & 0xFFFF) as u32;
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
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = (wparam.0 & 0xFFFF) as u32;
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
                        let _ = ShowWindow(hwnd, SW_HIDE);
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_SIZE => {
                let size_type = wparam.0 as u32;
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

                // SIZE_MAXIMIZED = 2: window is maximized or snapped
                let is_maximized = size_type == 2;

                // Resize WebView to fill window
                // When maximized/snapped, use full window; otherwise inset for resize borders
                if let Some(app) = get_app(hwnd)
                    && let Some(wv) = &app.state().webview
                {
                    let (wv_x, wv_y, wv_width, wv_height) = if is_maximized {
                        (0, 0, width, height)
                    } else {
                        (
                            RESIZE_BORDER,
                            RESIZE_BORDER,
                            width - 2 * RESIZE_BORDER,
                            height - 2 * RESIZE_BORDER,
                        )
                    };
                    wv.set_bounds(wv_x, wv_y, wv_width, wv_height);
                }

                LRESULT(0)
            }
            WM_PAINT => {
                // No D2D rendering needed - WebView handles all content
                let _ = ValidateRect(Some(hwnd), None);
                LRESULT(0)
            }
            WM_DESTROY => {
                // Clean up app state
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                }
                remove_tray_icon(hwnd);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
