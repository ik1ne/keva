//! Window hit testing for resize borders.

use windows::Win32::{
    Foundation::{HWND, LRESULT, RECT},
    UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT,
        HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME,
    },
};

/// Determines which part of the window the cursor is over for resize.
///
/// Uses system metrics for proper DPI-aware resize border sizes.
/// Returns an HT* constant wrapped in LRESULT.
/// The x, y coordinates are in screen space.
/// Window dragging is handled by CSS `app-region: drag` in the WebView.
pub fn hit_test(hwnd: HWND, screen_x: i32, screen_y: i32) -> LRESULT {
    let mut rect = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut rect) };

    // Use system metrics for resize border size (DPI-aware)
    let padded_border = unsafe { GetSystemMetrics(SM_CXPADDEDBORDER) };
    let border_x = unsafe { GetSystemMetrics(SM_CXSIZEFRAME) } + padded_border;
    let border_y = unsafe { GetSystemMetrics(SM_CYSIZEFRAME) } + padded_border;

    let left = rect.left;
    let top = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;

    let on_left = screen_x >= left && screen_x < left + border_x;
    let on_right = screen_x >= right - border_x && screen_x < right;
    let on_top = screen_y >= top && screen_y < top + border_y;
    let on_bottom = screen_y >= bottom - border_y && screen_y < bottom;

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
