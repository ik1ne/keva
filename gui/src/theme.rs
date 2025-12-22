use gpui::{Pixels, Size, WindowOptions, px};

// Colors
pub const BG_COLOR: u32 = 0x323232;
pub const TEXT_COLOR: u32 = 0xffffff;
pub const PANEL_BORDER_COLOR: u32 = 0x4a4a4a;
pub const INPUT_BG_COLOR: u32 = 0x3c3c3c;

// Layout
pub const DRAG_BORDER_PX: f32 = 3.0;
pub const SEARCH_BAR_HEIGHT: f32 = 40.0;
pub const LEFT_PANEL_MIN_WIDTH: f32 = 150.0;
pub const LEFT_PANEL_DEFAULT_WIDTH: f32 = 250.0;

// Window
pub const WINDOW_MIN_SIZE: Size<Pixels> = Size {
    width: px(400.0),
    height: px(300.0),
};

/// Creates WindowOptions for Keva's borderless window.
pub fn window_options() -> WindowOptions {
    WindowOptions {
        titlebar: None,
        window_min_size: Some(WINDOW_MIN_SIZE),
        is_movable: true,
        ..Default::default()
    }
}
