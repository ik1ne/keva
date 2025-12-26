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
    EDIT_BG_COLORREF, EDIT_TEXT_COLORREF, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, WINDOW_HEIGHT,
    WINDOW_WIDTH,
};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Graphics::Gdi::{
            CreateSolidBrush, FillRect, HBRUSH, HDC, SetBkColor, SetTextColor, ValidateRect,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::EM_SETCUEBANNER,
            Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE},
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetClientRect,
                GetMessageW, GetSystemMetrics, GetWindowLongPtrW, IDC_ARROW, IsWindowVisible,
                LoadCursorW, MINMAXINFO, MSG, PostQuitMessage, RegisterClassW, SM_CXSCREEN,
                SM_CYSCREEN, SW_HIDE, SW_SHOW, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
                SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
                WM_ACTIVATE, WM_COMMAND, WM_CREATE, WM_CTLCOLOREDIT, WM_DESTROY,
                WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONUP, WM_NCACTIVATE, WM_NCCALCSIZE,
                WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WM_SIZE, WNDCLASSW, WS_CHILD,
                WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU, WS_TABSTOP,
                WS_VISIBLE,
            },
        },
    },
    core::{Result, w},
};

/// Stores the previously focused window to restore on Esc.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Cached brush for EDIT control background (stored as isize for thread safety).
static EDIT_BG_BRUSH: AtomicIsize = AtomicIsize::new(0);

/// Subclass procedure for EDIT control to handle WM_ERASEBKGND directly.
unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    _dw_ref_data: usize,
) -> LRESULT {
    unsafe {
        match msg {
            windows::Win32::UI::WindowsAndMessaging::WM_ERASEBKGND => {
                // Paint background with dark color
                let hdc = HDC(wparam.0 as *mut _);
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);

                // Get or create the background brush
                let mut brush_handle = EDIT_BG_BRUSH.load(Ordering::Relaxed);
                if brush_handle == 0 {
                    let brush = CreateSolidBrush(EDIT_BG_COLORREF);
                    brush_handle = brush.0 as isize;
                    EDIT_BG_BRUSH.store(brush_handle, Ordering::Relaxed);
                }
                let brush = HBRUSH(brush_handle as *mut _);
                FillRect(hdc, &rect, brush);
                LRESULT(1) // Return non-zero to indicate we handled it
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

/// Runs the application.
pub fn run() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("KevaWindowClass");

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
        // WS_CLIPCHILDREN prevents painting over child windows (EDIT control)
        let style =
            WS_POPUP | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN;

        // WS_EX_NOREDIRECTIONBITMAP required for DirectComposition (flicker-free resize)
        let ex_style = WS_EX_APPWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP;

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

        // Create App after window (DirectComposition needs hwnd)
        let app = Box::new(App::new(hwnd, WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32)?);
        let app_ptr = Box::into_raw(app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize);

        // Create search bar EDIT control
        if let Some(app) = get_app(hwnd) {
            let layout = &app.state().layout;
            let search_edit = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.search_input.x as i32,
                layout.search_input.y as i32,
                layout.search_input.width as i32,
                layout.search_input.height as i32,
                Some(hwnd),
                None,
                Some(instance.into()),
                None,
            )?;

            // Subclass the EDIT control to handle WM_ERASEBKGND directly
            let _ = SetWindowSubclass(search_edit, Some(edit_subclass_proc), 1, 0);

            // Set placeholder text
            let placeholder = w!("Search keys...");
            let _ = SendMessageW(
                search_edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(placeholder.as_ptr() as isize)),
            );

            // Store EDIT handle in app state
            app.state_mut().search_edit = Some(search_edit);
        }

        // Create system tray icon
        add_tray_icon(hwnd)?;

        // Show window, bring to foreground, then set focus to search bar
        // SetFocus must be called AFTER ShowWindow for the EDIT control to receive input
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        if let Some(app) = get_app(hwnd)
            && let Some(search_edit) = app.state().search_edit
        {
            let _ = SetFocus(Some(search_edit));
        }

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
                // App is set up after CreateWindowExW returns
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
                // Return 0 to remove the non-client area entirely (borderless)
                LRESULT(0)
            }
            WM_NCACTIVATE => {
                // Prevent default non-client area painting (gray border)
                // Return TRUE to indicate we handled it
                LRESULT(TRUE.0 as isize)
            }
            WM_NCHITTEST => {
                let cursor_x = (lparam.0 & 0xFFFF) as i16 as i32;
                let cursor_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                if let Some(app) = get_app(hwnd) {
                    hit_test(hwnd, cursor_x, cursor_y, app.layout())
                } else {
                    // During window creation, fall back to default behavior
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_ACTIVATE => {
                let activating = (wparam.0 & 0xFFFF) != 0;
                let previous_window = lparam.0;
                if activating && previous_window != 0 {
                    PREV_FOREGROUND.store(previous_window, Ordering::Relaxed);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT => {
                // Customize EDIT control text colors for dark theme
                let hdc = HDC(wparam.0 as *mut _);
                SetTextColor(hdc, EDIT_TEXT_COLORREF);
                SetBkColor(hdc, EDIT_BG_COLORREF);

                // Return cached background brush (create on first use)
                let mut brush_handle = EDIT_BG_BRUSH.load(Ordering::Relaxed);
                if brush_handle == 0 {
                    let brush = CreateSolidBrush(EDIT_BG_COLORREF);
                    brush_handle = brush.0 as isize;
                    EDIT_BG_BRUSH.store(brush_handle, Ordering::Relaxed);
                }
                LRESULT(brush_handle)
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
                let width = (lparam.0 & 0xFFFF) as u32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
                if let Some(app) = get_app(hwnd) {
                    app.resize(width, height);

                    // Reposition EDIT control
                    if let Some(search_edit) = app.state().search_edit {
                        let layout = &app.state().layout;
                        let _ = SetWindowPos(
                            search_edit,
                            None,
                            layout.search_input.x as i32,
                            layout.search_input.y as i32,
                            layout.search_input.width as i32,
                            layout.search_input.height as i32,
                            SWP_NOZORDER,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                if let Some(app) = get_app(hwnd) {
                    app.paint();
                }
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
