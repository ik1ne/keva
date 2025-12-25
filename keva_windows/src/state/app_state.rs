//! Central application state.

use crate::ui::Layout;
use windows::Win32::Foundation::HWND;

/// Application state container.
///
/// This struct will grow with each milestone to hold:
/// - Focus state (search bar / left pane / right pane)
/// - Selection state (current key, previous key)
/// - Editing state (dirty flag, auto-save timer)
/// - Search state (query, results)
/// - Configuration state
pub struct AppState {
    /// Handle to the search bar EDIT control.
    pub search_edit: Option<HWND>,
    /// Computed layout based on window dimensions.
    pub layout: Layout,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            search_edit: None,
            layout: Layout::default(),
        }
    }

    /// Updates the layout for the given window dimensions.
    pub fn update_layout(&mut self, width: u32, height: u32) {
        self.layout = Layout::compute(width, height);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
