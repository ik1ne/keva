//! System tray icon management.

use std::mem::size_of;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, POINT},
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION,
                IsWindowVisible, LoadIconW, MF_GRAYED, MF_SEPARATOR, MF_STRING,
                SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
                TrackPopupMenu, WM_USER,
            },
        },
    },
};

/// Custom message for tray icon events.
pub const WM_TRAYICON: u32 = WM_USER + 1;

/// Tray icon ID.
const TRAY_ICON_ID: u32 = 1;

/// Tray menu item IDs.
pub const IDM_SHOW: u32 = 1001;
pub const IDM_SETTINGS: u32 = 1002;
pub const IDM_LAUNCH_AT_LOGIN: u32 = 1003;
pub const IDM_QUIT: u32 = 1004;

/// Adds a system tray icon for the window.
pub fn add_tray_icon(hwnd: HWND) -> Result<()> {
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
pub fn remove_tray_icon(hwnd: HWND) {
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
pub fn show_tray_menu(hwnd: HWND) {
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
