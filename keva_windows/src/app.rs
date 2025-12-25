//! Application coordinator.

use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW},
};

use crate::render::Renderer;
use crate::state::AppState;
use crate::ui::Layout;

/// Application coordinator, owns state and services.
pub struct App {
    renderer: Renderer,
    state: AppState,
}

impl App {
    /// Creates a new App instance.
    pub fn new() -> Result<Self, windows::core::Error> {
        Ok(Self {
            renderer: Renderer::new()?,
            state: AppState::new(),
        })
    }

    /// Paints the window content.
    pub fn paint(&mut self, hwnd: HWND) {
        if let Err(e) = self.renderer.render(hwnd, &self.state.layout) {
            eprintln!("Failed to render: {e}");
        }
    }

    /// Handles window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.state.update_layout(width, height);
        // Render target resize is handled in render() via ensure_target()
    }

    /// Returns a reference to the app state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Returns a mutable reference to the app state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Returns the current layout.
    pub fn layout(&self) -> &Layout {
        &self.state.layout
    }

    /// Returns the current search bar text.
    pub fn get_search_text(&self) -> String {
        let Some(search_edit) = self.state.search_edit else {
            return String::new();
        };

        unsafe {
            let len = GetWindowTextLengthW(search_edit);
            if len == 0 {
                return String::new();
            }

            let mut buffer = vec![0u16; (len + 1) as usize];
            GetWindowTextW(search_edit, &mut buffer);
            String::from_utf16_lossy(&buffer[..len as usize])
        }
    }
}
