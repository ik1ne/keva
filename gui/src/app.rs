use crate::panels::{InspectorPanel, KeyListPanel, SearchBarPanel};
use crate::theme::{
    DRAG_BORDER_PX, LEFT_PANEL_DEFAULT_WIDTH, LEFT_PANEL_MIN_WIDTH, RESIZE_BORDER_PX,
    TOTAL_BORDER_PX,
};
use eframe::egui::{self, CursorIcon, Pos2, Rect};
use egui::viewport::ResizeDirection;

pub struct KevaApp {
    search_text: String,
}

impl KevaApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Theme is applied in main.rs before this is called
        Self {
            search_text: String::new(),
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // Only handle keyboard shortcuts when window has focus
        if !ctx.input(|i| i.focused) {
            return;
        }

        // Esc -> hide window
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        // Cmd+Q (macOS) / Ctrl+Q (Windows) -> quit
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn handle_window_chrome(&self, ctx: &egui::Context) {
        let pointer = ctx.input(|i| i.pointer.clone());
        let Some(pos) = pointer.hover_pos() else {
            return;
        };

        let screen_rect = ctx.input(|i| i.viewport_rect());

        // Check resize zones first (outermost)
        if let Some(resize_dir) = self.get_resize_direction(pos, screen_rect) {
            ctx.set_cursor_icon(resize_direction_to_cursor(resize_dir));
            if pointer.primary_pressed() {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(resize_dir));
            }
            return;
        }

        // Check drag zone (between resize and content)
        // Use default cursor - Grab feels wrong for window dragging
        if self.is_in_drag_zone(pos, screen_rect) {
            ctx.set_cursor_icon(CursorIcon::Default);
            if pointer.primary_pressed() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
    }

    fn get_resize_direction(&self, pos: Pos2, rect: Rect) -> Option<ResizeDirection> {
        let border = RESIZE_BORDER_PX;

        let on_left = pos.x < rect.min.x + border;
        let on_right = pos.x > rect.max.x - border;
        let on_top = pos.y < rect.min.y + border;
        let on_bottom = pos.y > rect.max.y - border;

        match (on_left, on_right, on_top, on_bottom) {
            (true, false, true, false) => Some(ResizeDirection::NorthWest),
            (true, false, false, true) => Some(ResizeDirection::SouthWest),
            (false, true, true, false) => Some(ResizeDirection::NorthEast),
            (false, true, false, true) => Some(ResizeDirection::SouthEast),
            (true, false, false, false) => Some(ResizeDirection::West),
            (false, true, false, false) => Some(ResizeDirection::East),
            (false, false, true, false) => Some(ResizeDirection::North),
            (false, false, false, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn is_in_drag_zone(&self, pos: Pos2, rect: Rect) -> bool {
        let outer = RESIZE_BORDER_PX;
        let inner = RESIZE_BORDER_PX + DRAG_BORDER_PX;

        // Inside outer border (not at resize edge)
        let in_outer = pos.x >= rect.min.x + outer
            && pos.x <= rect.max.x - outer
            && pos.y >= rect.min.y + outer
            && pos.y <= rect.max.y - outer;

        // Inside inner content area
        let in_inner = pos.x >= rect.min.x + inner
            && pos.x <= rect.max.x - inner
            && pos.y >= rect.min.y + inner
            && pos.y <= rect.max.y - inner;

        // Drag zone is between outer and inner
        in_outer && !in_inner
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        // Add padding for drag border, but keep theme's panel fill
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ctx.style().as_ref())
                    .inner_margin(TOTAL_BORDER_PX),
            )
            .show(ctx, |ui| {
                // Top: Search bar
                if SearchBarPanel::show(ui, &mut self.search_text) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.add_space(4.0);

                // Left panel - allocate remaining space to prevent snapping
                egui::SidePanel::left("key_list")
                    .min_width(LEFT_PANEL_MIN_WIDTH)
                    .default_width(LEFT_PANEL_DEFAULT_WIDTH)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        KeyListPanel::show(ui);
                        // Fill remaining space to prevent panel from shrinking
                        ui.allocate_space(ui.available_size());
                    });

                // Right panel (takes remaining space)
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    InspectorPanel::show(ui);
                });
            });
    }
}

fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    }
}

impl eframe::App for KevaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);
        self.handle_window_chrome(ctx);
        self.render_ui(ctx);

        // Request continuous repaint for responsive UI
        ctx.request_repaint();
    }
}
