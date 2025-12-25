//! Window hit testing for resize and drag.

use crate::render::theme::RESIZE_BORDER;
use windows::Win32::{
    Foundation::{HWND, LRESULT, RECT},
    UI::WindowsAndMessaging::{
        GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTLEFT, HTRIGHT, HTTOP,
        HTTOPLEFT, HTTOPRIGHT,
    },
};

/// Determines which part of the window the cursor is over for resize/drag.
///
/// Returns an HT* constant wrapped in LRESULT.
pub fn hit_test(hwnd: HWND, x: i32, y: i32) -> LRESULT {
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
