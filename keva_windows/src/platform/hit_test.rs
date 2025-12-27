//! Window hit testing for resize borders.

use crate::render::theme::RESIZE_BORDER;
use windows::Win32::{
    Foundation::{HWND, LRESULT, RECT},
    UI::WindowsAndMessaging::{
        GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP,
        HTTOPLEFT, HTTOPRIGHT,
    },
};

/// Determines which part of the window the cursor is over for resize.
///
/// Returns an HT* constant wrapped in LRESULT.
/// The x, y coordinates are in screen space.
/// Window dragging is handled by CSS `app-region: drag` in the WebView.
pub fn hit_test(hwnd: HWND, screen_x: i32, screen_y: i32) -> LRESULT {
    let mut rect = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut rect) };

    let left = rect.left;
    let top = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;

    let on_left = screen_x >= left && screen_x < left + RESIZE_BORDER;
    let on_right = screen_x >= right - RESIZE_BORDER && screen_x < right;
    let on_top = screen_y >= top && screen_y < top + RESIZE_BORDER;
    let on_bottom = screen_y >= bottom - RESIZE_BORDER && screen_y < bottom;

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
        HTCLIENT
    };

    LRESULT(result as isize)
}
