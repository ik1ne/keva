//! Application state.

use windows::Win32::Foundation::HWND;

use crate::render::Renderer;

/// Application state.
pub struct App {
    renderer: Renderer,
}

impl App {
    /// Creates a new App instance.
    pub fn new() -> Result<Self, windows::core::Error> {
        Ok(Self {
            renderer: Renderer::new()?,
        })
    }

    /// Paints the window content.
    pub fn paint(&mut self, hwnd: HWND) {
        if let Err(e) = self.renderer.ensure_target(hwnd) {
            eprintln!("Failed to ensure render target: {e}");
            return;
        }

        if let Err(e) = self.renderer.render() {
            eprintln!("Failed to render: {e}");
        }
    }

    /// Handles window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Err(e) = self.renderer.resize(width, height) {
            eprintln!("Failed to resize render target: {e}");
        }
    }
}
