//! Theme and layout constants.

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// Creates a D2D1_COLOR_F from RGB values (0-255).
const fn rgb(r: u8, g: u8, b: u8) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

// Window dimensions.
pub const WINDOW_WIDTH: i32 = 800;
pub const WINDOW_HEIGHT: i32 = 600;
pub const MIN_WINDOW_WIDTH: i32 = 400;
pub const MIN_WINDOW_HEIGHT: i32 = 300;

// Window chrome.
pub const RESIZE_BORDER: i32 = 5;

// Colors - background.
pub const COLOR_BG: D2D1_COLOR_F = rgb(26, 26, 26);
pub const COLOR_SEARCH_BAR_BG: D2D1_COLOR_F = rgb(36, 36, 36);
pub const COLOR_LEFT_PANE_BG: D2D1_COLOR_F = rgb(30, 30, 30);
pub const COLOR_RIGHT_PANE_BG: D2D1_COLOR_F = rgb(26, 26, 26);
pub const COLOR_DIVIDER: D2D1_COLOR_F = rgb(50, 50, 50);

// Colors - search icon.
pub const COLOR_SEARCH_ICON_BG: D2D1_COLOR_F = rgb(45, 45, 45);
pub const COLOR_SEARCH_ICON: D2D1_COLOR_F = rgb(150, 150, 150);

// Colors - search input.
pub const COLOR_SEARCH_INPUT_BG: D2D1_COLOR_F = rgb(45, 45, 45);
pub const COLOR_SEARCH_PLACEHOLDER: D2D1_COLOR_F = rgb(120, 120, 120);
pub const COLOR_SEARCH_TEXT: D2D1_COLOR_F = rgb(230, 230, 230);

// GDI COLORREF values for EDIT control.
use windows::Win32::Foundation::COLORREF;

/// Creates a COLORREF from RGB values (0-255). COLORREF uses 0x00BBGGRR format.
const fn colorref(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

pub const EDIT_BG_COLORREF: COLORREF = colorref(45, 45, 45);
pub const EDIT_TEXT_COLORREF: COLORREF = colorref(230, 230, 230);

/// Main window background color for GDI (matches COLOR_BG).
pub const WINDOW_BG_COLORREF: COLORREF = colorref(26, 26, 26);
