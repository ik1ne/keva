//! Keva Windows application.
//!
//! A borderless window with system tray integration for the Keva clipboard manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod renderer;

use app::App;
use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM},
        Graphics::{
            Dwm::DwmExtendFrameIntoClientArea,
            Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH, ValidateRect},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::MARGINS,
            Input::KeyboardAndMouse::VK_ESCAPE,
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
                DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
                GetWindowRect, GWLP_USERDATA, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
                HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IDI_APPLICATION,
                IsWindowVisible, LoadCursorW, LoadIconW, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG,
                PostQuitMessage, RegisterClassW, SetForegroundWindow, SetWindowLongPtrW,
                ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, TPM_BOTTOMALIGN,
                TPM_LEFTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, WM_ACTIVATE, WM_COMMAND,
                WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONUP, WM_NCACTIVATE, WM_NCCALCSIZE,
                WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WM_SIZE, WM_USER, WNDCLASSW, WS_EX_APPWINDOW,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
            },
        },
    },
    core::{Result, w},
};

/// Border width in pixels for resize hit detection.
const RESIZE_BORDER: i32 = 5;
const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 600;

/// Custom message for tray icon events.
const WM_TRAYICON: u32 = WM_USER + 1;
/// Tray icon ID.
const TRAY_ICON_ID: u32 = 1;

/// Tray menu item IDs.
const IDM_SHOW: u32 = 1001;
const IDM_SETTINGS: u32 = 1002;
const IDM_LAUNCH_AT_LOGIN: u32 = 1003;
const IDM_QUIT: u32 = 1004;

/// Stores the previously focused window to restore on Esc.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

fn main() -> Result<()> {
    // Initialize keva_core
    let app = match App::new() {
        Ok(app) => Box::new(app),
        Err(e) => {
            eprintln!("Failed to initialize Keva: {e}");
            return Ok(());
        }
    };

    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("KevaWindowClass");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: class_name,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0);

        // Borderless window with resize capability
        let style = WS_POPUP | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU;

        // Center window on screen
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_width - WINDOW_WIDTH) / 2;
        let y = (screen_height - WINDOW_HEIGHT) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW, // Force Alt+Tab visibility
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

        // Store app state in window
        let app_ptr = Box::into_raw(app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize);

        // Extend DWM frame into entire client area for smooth compositing
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        // Create system tray icon
        add_tray_icon(hwnd)?;

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

/// Adds a system tray icon for the window.
fn add_tray_icon(hwnd: HWND) -> Result<()> {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: LoadIconW(None, IDI_APPLICATION)?,
            ..Default::default()
        };

        // Set tooltip
        let tooltip = "Keva";
        for (i, c) in tooltip.encode_utf16().enumerate() {
            if i < nid.szTip.len() - 1 {
                nid.szTip[i] = c;
            }
        }

        if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_thread())
        }
    }
}

/// Removes the system tray icon.
fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// Shows the tray icon context menu.
fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let Ok(hmenu) = CreatePopupMenu() else {
            return;
        };

        let is_visible = IsWindowVisible(hwnd).as_bool();

        // "Show Keva" - disabled if already visible
        let show_flags = if is_visible {
            MF_STRING | MF_GRAYED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(hmenu, show_flags, IDM_SHOW as usize, w!("Show Keva"));

        // "Settings..." - non-functional until M15-win
        let _ = AppendMenuW(hmenu, MF_STRING | MF_GRAYED, IDM_SETTINGS as usize, w!("Settings..."));

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

        // "Launch at Login" - non-functional until M20-win
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_GRAYED,
            IDM_LAUNCH_AT_LOGIN as usize,
            w!("Launch at Login"),
        );

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

        // "Quit Keva"
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_QUIT as usize, w!("Quit Keva"));

        // Get cursor position for menu placement
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        // Required to make the menu dismiss when clicking outside
        let _ = SetForegroundWindow(hwnd);

        // Show the menu
        let _ = TrackPopupMenu(
            hmenu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );

        let _ = DestroyMenu(hmenu);
    }
}

/// Gets the App instance from the window's user data.
fn get_app(hwnd: HWND) -> Option<&'static mut App> {
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
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                hit_test(hwnd, x, y)
            }
            WM_ACTIVATE => {
                // When activated, lParam contains the handle of the window being deactivated
                let activating = (wparam.0 & 0xFFFF) != 0;
                if activating && lparam.0 != 0 {
                    PREV_FOREGROUND.store(lparam.0, Ordering::Relaxed);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    // Restore focus to previous window before hiding
                    let prev = PREV_FOREGROUND.load(Ordering::Relaxed);
                    if prev != 0 {
                        let _ = SetForegroundWindow(HWND(prev as *mut _));
                    }
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                LRESULT(0)
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
                }
                LRESULT(0)
            }
            WM_PAINT => {
                if let Some(app) = get_app(hwnd) {
                    app.paint(hwnd);
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

/// Determines which part of the window the cursor is over for resize/drag.
fn hit_test(hwnd: HWND, x: i32, y: i32) -> LRESULT {
    let mut rect = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut rect) };

    let left = rect.left;
    let top = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;

    let on_left = x >= left && x < left + RESIZE_BORDER;
    let on_right = x >= right - RESIZE_BORDER && x < right;
    let on_top = y >= top && y < top + RESIZE_BORDER;
    let on_bottom = y >= bottom - RESIZE_BORDER && y < bottom;

    let result = if on_top && on_left {
        HTTOPLEFT
    } else if on_top && on_right {
        HTTOPRIGHT
    } else if on_bottom && on_left {
        HTBOTTOMLEFT
    } else if on_bottom && on_right {
        HTBOTTOMRIGHT
    } else if on_left {
        HTLEFT
    } else if on_right {
        HTRIGHT
    } else if on_top {
        HTTOP
    } else if on_bottom {
        HTBOTTOM
    } else {
        // Entire window is draggable (will refine later for content areas)
        HTCAPTION
    };

    LRESULT(result as isize)
}
