//! Application coordinator.

use windows::Win32::Foundation::HWND;

use crate::state::AppState;

/// Application coordinator, owns state and services.
pub struct App {
    state: AppState,
    #[expect(dead_code)]
    hwnd: HWND,
}

impl App {
    /// Creates a new App instance bound to the given window.
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            state: AppState::new(),
        }
    }

    /// Returns a reference to the app state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Returns a mutable reference to the app state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }
}
