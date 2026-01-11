//! Window creation and message handling.

use crate::keva_worker;
use crate::platform::wm;
use crate::platform::{
    drop_target::register_drop_target,
    handlers::{
        on_activate, on_command, on_create, on_destroy, on_getminmaxinfo, on_nccalcsize,
        on_open_file_picker, on_paint, on_setfocus, on_settingchange, on_size, on_trayicon,
        on_webview_message, scale_for_dpi, set_app_config, set_current_theme,
        show_and_focus_window,
    },
    hit_test::hit_test,
    hotkey::register_global_hotkey,
    input::{forward_mouse_message, forward_pointer_message},
    single_instance::check_single_instance,
    tray::{WM_TRAYICON, add_tray_icon},
};
use crate::render::theme::{Theme, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::webview::{OutgoingMessage, WEBVIEW, bridge::post_message, init_webview};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, TRUE, WPARAM},
        Graphics::Dwm::DwmExtendFrameIntoClientArea,
        System::LibraryLoader::GetModuleHandleW,
        System::Ole::{OleInitialize, OleUninitialize},
        UI::Controls::MARGINS,
        UI::{
            HiDpi::GetDpiForSystem,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
                HCURSOR, HTCLIENT, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassW,
                SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SWP_NOCOPYBITS, SetCursor, SetForegroundWindow,
                ShowWindow, TranslateMessage, WINDOWPOS, WM_ACTIVATE, WM_CLOSE, WM_COMMAND,
                WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_HOTKEY,
                WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN,
                WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE,
                WM_NCHITTEST, WM_PAINT, WM_POINTERDOWN, WM_POINTERENTER, WM_POINTERLEAVE,
                WM_POINTERUP, WM_POINTERUPDATE, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
                WM_SETCURSOR, WM_SETFOCUS, WM_SETTINGCHANGE, WM_SIZE, WM_WINDOWPOSCHANGING,
                WNDCLASSW, WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
                WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
            },
        },
    },
    core::{Result, w},
};

pub fn run() -> Result<()> {
    unsafe {
        // Initialize OLE for UI thread (required for WebView2, file picker, drag-drop, etc.)
        // OleInitialize internally calls CoInitializeEx with COINIT_APARTMENTTHREADED
        let _ = OleInitialize(None);

        let instance = GetModuleHandleW(None)?;
        let class_name = w!("KevaWindowClass");

        // Check for existing instance - if found, activate it and exit
        let _instance_guard = match check_single_instance(class_name) {
            Ok(guard) => guard,
            Err(()) => {
                OleUninitialize();
                return Ok(());
            }
        };

        let config_path = keva_worker::get_data_path().join("config.toml");
        let config = keva_core::types::AppConfig::load(&config_path).unwrap_or_default();
        set_app_config(config.clone());
        let initial_theme = match config.general.theme {
            keva_core::types::Theme::Dark => Theme::Dark,
            keva_core::types::Theme::Light => Theme::Light,
            keva_core::types::Theme::System => Theme::detect_system(),
        };
        set_current_theme(initial_theme);

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0);

        // WS_POPUP: borderless window (no title bar)
        // WS_SIZEBOX: resizable edges
        // WS_CLIPCHILDREN: prevents painting over child windows (WebView)
        let style =
            WS_POPUP | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN;

        // WS_EX_TOPMOST: always on top
        let ex_style = WS_EX_APPWINDOW | WS_EX_TOPMOST;

        let dpi = GetDpiForSystem();
        let window_width = scale_for_dpi(WINDOW_WIDTH, dpi);
        let window_height = scale_for_dpi(WINDOW_HEIGHT, dpi);

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

        // Extend frame into entire client area to suppress DWM border rendering.
        // The WM_PAINT handler paints over this with an opaque background.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        // Register global hotkey from config
        if !register_global_hotkey(hwnd, &config.shortcuts.global_shortcut) {
            eprintln!(
                "[Hotkey] Failed to register '{}' - may be in use by another application",
                config.shortcuts.global_shortcut
            );
        }

        // Start worker thread (owns KevaCore + SearchEngine, posts directly to UI thread)
        let request_tx = keva_worker::start(hwnd);

        // Create WebView filling entire client area
        init_webview(
            hwnd,
            0,
            0,
            window_width,
            window_height,
            initial_theme,
            request_tx,
        );

        add_tray_icon(hwnd)?;

        // Register drop target for drag-drop interception
        register_drop_target(hwnd)?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        OleUninitialize();

        Ok(())
    }
}

/// Window procedure: dispatches Windows messages to handlers.
extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => on_create(hwnd),
        WM_GETMINMAXINFO => on_getminmaxinfo(lparam),
        WM_NCCALCSIZE => on_nccalcsize(hwnd, wparam, lparam),
        // WM_NCACTIVATE: return TRUE to prevent default non-client painting
        WM_NCACTIVATE => LRESULT(TRUE.0 as isize),
        // WM_WINDOWPOSCHANGING: disable BitBlt to prevent visual artifacts during resize
        WM_WINDOWPOSCHANGING => unsafe {
            let wp = lparam.0 as *mut WINDOWPOS;
            if !wp.is_null() {
                (*wp).flags |= SWP_NOCOPYBITS;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        // WM_NCHITTEST: determine resize/drag areas for borderless window
        WM_NCHITTEST => {
            let cursor_x = (lparam.0 & 0xFFFF) as i16 as i32;
            let cursor_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            hit_test(hwnd, cursor_x, cursor_y)
        }
        WM_ACTIVATE => {
            on_activate(wparam, lparam);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_SETFOCUS => {
            on_setfocus();
            LRESULT(0)
        }
        // WM_SETCURSOR: Query cursor from WebView2 CompositionController
        WM_SETCURSOR => {
            let hit_test_result = (lparam.0 & 0xFFFF) as u32;
            if hit_test_result == HTCLIENT
                && let Some(wv) = WEBVIEW.get()
            {
                let mut cursor = HCURSOR::default();
                if unsafe { wv.composition_controller.Cursor(&mut cursor) }.is_ok() {
                    unsafe { SetCursor(Some(cursor)) };
                    return LRESULT(1);
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_ERASEBKGND => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        // Mouse messages: forward to WebView2 CompositionController
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_RBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MBUTTONDBLCLK
        | WM_MOUSEWHEEL => {
            forward_mouse_message(hwnd, msg, wparam, lparam);
            LRESULT(0)
        }
        // Pointer messages (touch/pen): forward to WebView2 CompositionController
        WM_POINTERDOWN | WM_POINTERUP | WM_POINTERUPDATE | WM_POINTERENTER | WM_POINTERLEAVE => {
            forward_pointer_message(hwnd, msg, wparam);
            LRESULT(0)
        }
        WM_TRAYICON => on_trayicon(hwnd, lparam),
        WM_COMMAND => on_command(hwnd, wparam),
        wm::SHUTDOWN_COMPLETE => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        wm::WEBVIEW_MESSAGE => on_webview_message(lparam),
        wm::OPEN_FILE_PICKER => on_open_file_picker(hwnd, lparam),
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
        WM_HOTKEY => {
            show_and_focus_window(hwnd);
            LRESULT(0)
        }
        wm::ACTIVATE_INSTANCE => {
            show_and_focus_window(hwnd);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
