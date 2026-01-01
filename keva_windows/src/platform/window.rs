//! Window creation and message handling.

use crate::app::App;
use crate::keva_worker::{self, WM_KEVA_RESPONSE, WM_SHUTDOWN_COMPLETE};
use crate::platform::{
    handlers::{
        get_resize_border, on_activate, on_command, on_create, on_destroy, on_getminmaxinfo,
        on_keva_response, on_keydown, on_nccalcsize, on_paint, on_settingchange, on_size,
        on_trayicon, scale_for_dpi, set_current_theme,
    },
    hit_test::hit_test,
    tray::{WM_TRAYICON, add_tray_icon},
};
use crate::render::theme::{Theme, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::webview::init_webview;
use crate::webview::{WEBVIEW, bridge::post_message, messages::OutgoingMessage};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, TRUE, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::GetDpiForSystem,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW,
                GetSystemMetrics, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassW,
                SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SWP_NOCOPYBITS, SetForegroundWindow,
                SetWindowLongPtrW, ShowWindow, TranslateMessage, WINDOWPOS, WM_ACTIVATE, WM_CLOSE,
                WM_COMMAND, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN,
                WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_PAINT, WM_SETTINGCHANGE, WM_SIZE,
                WM_WINDOWPOSCHANGING, WNDCLASSW, WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_EX_TOPMOST,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
            },
        },
    },
    core::{Result, w},
};

/// Runs the application.
pub fn run() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("KevaWindowClass");

        // Detect system theme for initial background color
        let initial_theme = Theme::detect_system();
        set_current_theme(initial_theme);
        eprintln!("[Native] Initial theme: {:?}", initial_theme);

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
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

        // Scale window dimensions for DPI
        let dpi = GetDpiForSystem();
        let window_width = scale_for_dpi(WINDOW_WIDTH, dpi);
        let window_height = scale_for_dpi(WINDOW_HEIGHT, dpi);

        // Center window on screen
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - window_width) / 2;
        let y = (screen_height - window_height) / 2;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("Keva"),
            style,
            x,
            y,
            window_width,
            window_height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        // Start keva worker thread
        let (request_tx, response_rx) = keva_worker::start(hwnd);

        // Create App with response receiver
        let app_ptr = Box::into_raw(Box::new(App { response_rx }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize);

        // Create WebView with resize border insets
        let (border_x, border_y) = get_resize_border();

        init_webview(
            hwnd,
            border_x,
            border_y,
            window_width - 2 * border_x,
            window_height - 2 * border_y,
            initial_theme,
            request_tx,
        );

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

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => on_create(hwnd),
        WM_GETMINMAXINFO => on_getminmaxinfo(lparam),
        WM_NCCALCSIZE => on_nccalcsize(wparam, lparam),
        WM_NCACTIVATE => LRESULT(TRUE.0 as isize),
        WM_WINDOWPOSCHANGING => unsafe {
            let wp = lparam.0 as *mut WINDOWPOS;
            if !wp.is_null() {
                (*wp).flags |= SWP_NOCOPYBITS;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_NCHITTEST => {
            let cursor_x = (lparam.0 & 0xFFFF) as i16 as i32;
            let cursor_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            hit_test(hwnd, cursor_x, cursor_y)
        }
        WM_ACTIVATE => {
            on_activate(wparam, lparam);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_ERASEBKGND => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        WM_KEYDOWN => on_keydown(hwnd, wparam)
            .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }),
        WM_TRAYICON => on_trayicon(hwnd, lparam),
        WM_COMMAND => on_command(hwnd, wparam),
        WM_KEVA_RESPONSE =>
        // SAFETY: wndproc is single-threaded
        unsafe { on_keva_response(hwnd) },
        WM_SHUTDOWN_COMPLETE => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(wv) = WEBVIEW.get() {
                post_message(&wv.webview, &OutgoingMessage::Shutdown);
            } else {
                unsafe { PostQuitMessage(0) };
            }
            LRESULT(0)
        }
        WM_SIZE => on_size(wparam, lparam),
        WM_SETTINGCHANGE => on_settingchange(hwnd, lparam),
        WM_PAINT => on_paint(hwnd),
        WM_DESTROY => on_destroy(hwnd),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
