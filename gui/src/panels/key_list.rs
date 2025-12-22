use eframe::egui;

pub struct KeyListPanel;

impl KeyListPanel {
    pub fn show(ui: &mut egui::Ui) {
        ui.heading("Keys");
        ui.label("(No keys loaded - M1 placeholder)");
    }
}
