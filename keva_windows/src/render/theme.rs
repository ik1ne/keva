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

// Colors.
pub const COLOR_BG: D2D1_COLOR_F = rgb(26, 26, 26);
pub const COLOR_SEARCH_ICON_BG: D2D1_COLOR_F = rgb(45, 45, 45);
pub const COLOR_SEARCH_ICON: D2D1_COLOR_F = rgb(150, 150, 150);

