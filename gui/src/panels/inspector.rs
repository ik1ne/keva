use eframe::egui;

pub struct InspectorPanel;

impl InspectorPanel {
    pub fn show(ui: &mut egui::Ui) {
        ui.heading("Inspector");
        ui.label("(Select a key to view - M1 placeholder)");
    }
}
