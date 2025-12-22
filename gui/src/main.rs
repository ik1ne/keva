mod app;
mod panels;
mod search;
mod theme;

use app::KevaApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(theme::WINDOW_DEFAULT_SIZE)
            .with_min_inner_size(theme::WINDOW_MIN_SIZE)
            .with_decorations(false)
            .with_transparent(false),
        persist_window: false,
        // Disable vsync to reduce input latency
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "Keva",
        options,
        Box::new(|cc| {
            // Force dark theme first, then apply our custom visuals
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            theme::apply_theme(&cc.egui_ctx);
            Ok(Box::new(KevaApp::new(cc)))
        }),
    )
}
