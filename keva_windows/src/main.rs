//! Keva Windows application.
//!
//! A borderless window with system tray integration for the Keva clipboard manager.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod keva_worker;
mod platform;
mod render;
mod templates;
mod webview;

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::core::Result;

fn main() -> Result<()> {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    platform::window::run()
}
