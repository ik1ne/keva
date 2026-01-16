//! Window hit testing for resize borders and app-region: drag areas.

use crate::webview::WEBVIEW;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION, ICoreWebView2CompositionController4,
};
use windows::Win32::{
    Foundation::{HWND, LRESULT, POINT, RECT},
    UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
        HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, SM_CXPADDEDBORDER, SM_CXSIZEFRAME,
        SM_CYSIZEFRAME,
    },
};
use windows::core::Interface;

/// Determines which part of the window the cursor is over for resize/drag.
///
/// Uses system metrics for proper DPI-aware resize border sizes.
/// Returns an HT* constant wrapped in LRESULT.
/// The x, y coordinates are in screen space.
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

    // Check resize borders first (higher priority than caption)
    if on_top && on_left {
        return LRESULT(HTTOPLEFT as isize);
    } else if on_top && on_right {
        return LRESULT(HTTOPRIGHT as isize);
    } else if on_bottom && on_left {
        return LRESULT(HTBOTTOMLEFT as isize);
    } else if on_bottom && on_right {
        return LRESULT(HTBOTTOMRIGHT as isize);
    } else if on_left {
        return LRESULT(HTLEFT as isize);
    } else if on_right {
        return LRESULT(HTRIGHT as isize);
    } else if on_top {
        return LRESULT(HTTOP as isize);
    } else if on_bottom {
        return LRESULT(HTBOTTOM as isize);
    }

    // Check WebView's non-client regions (app-region: drag areas)
    if let Some(wv) = WEBVIEW.get()
        && let Ok(cc4) = wv
            .composition_controller
            .cast::<ICoreWebView2CompositionController4>()
    {
        // Convert screen to client coordinates
        let point = POINT {
            x: screen_x - left,
            y: screen_y - top,
        };

        let mut region_kind = Default::default();
        if unsafe { cc4.GetNonClientRegionAtPoint(point, &mut region_kind) }.is_ok()
            && region_kind == COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION
        {
            return LRESULT(HTCAPTION as isize);
        }
    }

    LRESULT(HTCLIENT as isize)
}
