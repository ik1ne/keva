use crate::theme::{SEARCH_BAR_HEIGHT, SEARCH_ICON_WIDTH};
use eframe::egui::{self, CursorIcon, FontId};

pub struct SearchBarPanel;

impl SearchBarPanel {
    /// Renders the search bar. Returns true if the search icon was dragged.
    pub fn show(ui: &mut egui::Ui, search_text: &mut String) -> bool {
        let mut dragged = false;

        ui.horizontal(|ui| {
            // Search icon (drag handle) - allocate space and paint manually
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(SEARCH_ICON_WIDTH, SEARCH_BAR_HEIGHT),
                egui::Sense::drag(),
            );

            // Paint the emoji centered in the rect (no hover effect)
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "🔍",
                FontId::proportional(16.0),
                ui.visuals().text_color(),
            );

            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Default);
            }

            if response.dragged() {
                dragged = true;
            }

            // Text input
            ui.add_sized(
                [ui.available_width(), SEARCH_BAR_HEIGHT],
                egui::TextEdit::singleline(search_text).hint_text("Search keys..."),
            );
        });

        dragged
    }
}
