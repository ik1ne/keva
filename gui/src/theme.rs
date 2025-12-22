use eframe::egui::{self, Color32, Visuals};

// Border zones
pub const RESIZE_BORDER_PX: f32 = 5.0;
pub const DRAG_BORDER_PX: f32 = 3.0;
pub const TOTAL_BORDER_PX: f32 = RESIZE_BORDER_PX + DRAG_BORDER_PX;

// Search bar
pub const SEARCH_ICON_WIDTH: f32 = 32.0;
pub const SEARCH_BAR_HEIGHT: f32 = 40.0;

// Layout
pub const LEFT_PANEL_MIN_WIDTH: f32 = 150.0;
pub const LEFT_PANEL_DEFAULT_WIDTH: f32 = 250.0;

// Window
pub const WINDOW_MIN_SIZE: [f32; 2] = [400.0, 300.0];
pub const WINDOW_DEFAULT_SIZE: [f32; 2] = [800.0, 600.0];

/// Creates the Keva dark gray theme.
pub fn dark_gray_visuals() -> Visuals {
    let mut visuals = Visuals::dark();

    // Main background - dark gray instead of black
    visuals.panel_fill = Color32::from_gray(50);
    visuals.window_fill = Color32::from_gray(50);
    visuals.extreme_bg_color = Color32::from_gray(35);
    visuals.faint_bg_color = Color32::from_gray(45);

    // Widget backgrounds
    visuals.widgets.noninteractive.bg_fill = Color32::from_gray(60);
    visuals.widgets.inactive.bg_fill = Color32::from_gray(65);
    visuals.widgets.hovered.bg_fill = Color32::from_gray(75);
    visuals.widgets.active.bg_fill = Color32::from_gray(85);

    visuals
}

/// Applies the Keva theme to the context.
pub fn apply_theme(ctx: &egui::Context) {
    ctx.set_visuals(dark_gray_visuals());
}
