//! Keva Windows application.
//!
//! A borderless window with system tray integration for the Keva clipboard manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod platform;
mod render;
mod state;
mod storage;
mod templates;
mod webview;

use windows::core::Result;

fn main() -> Result<()> {
    storage::init();
    platform::window::run()
}
