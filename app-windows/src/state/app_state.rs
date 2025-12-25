//! Central application state.

/// Application state container.
///
/// This struct will grow with each milestone to hold:
/// - Focus state (search bar / left pane / right pane)
/// - Selection state (current key, previous key)
/// - Editing state (dirty flag, auto-save timer)
/// - Search state (query, results)
/// - Configuration state
pub struct AppState {
    // Will be populated as milestones are implemented
}

impl AppState {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
