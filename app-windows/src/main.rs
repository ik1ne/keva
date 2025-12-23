//! Keva Windows application.
//!
//! A borderless window with system tray integration for the Keva clipboard manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM},
        Graphics::{
            Dwm::DwmExtendFrameIntoClientArea,
            Gdi::{GetStockObject, ValidateRect, BLACK_BRUSH, HBRUSH},
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::MARGINS,
            Input::KeyboardAndMouse::VK_ESCAPE,
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
                NOTIFYICONDATAW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
                GetWindowRect, IsWindowVisible, LoadIconW, PostQuitMessage, RegisterClassW,
                ShowWindow, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTLEFT, HTRIGHT,
                HTTOP, HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IDI_APPLICATION, LoadCursorW, MSG,
                SetForegroundWindow, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, WM_ACTIVATE,
                WM_DESTROY, WM_KEYDOWN, WM_LBUTTONUP, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST,
                WM_PAINT, WM_USER, WNDCLASSW, WS_EX_APPWINDOW, WS_MAXIMIZEBOX,
                WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
            },
        },
    },
};

/// Border width in pixels for resize hit detection.
const RESIZE_BORDER: i32 = 6;
const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 600;

/// Custom message for tray icon events.
const WM_TRAYICON: u32 = WM_USER + 1;
/// Tray icon ID.
const TRAY_ICON_ID: u32 = 1;

/// Stores the previously focused window to restore on Esc.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

fn main() -> Result<()> {
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

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
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
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let _ = ValidateRect(Some(hwnd), None);
                LRESULT(0)
            }
            WM_DESTROY => {
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
