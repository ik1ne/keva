//! Keva Windows application.
//!
//! A borderless window with system tray integration for the Keva clipboard manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod platform;
mod render;
mod state;
mod ui;

use app::App;
use windows::core::Result;

fn main() -> Result<()> {
    let app = match App::new() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to initialize Keva: {e}");
            return Ok(());
        }
    };

    platform::window::run(app)
}
