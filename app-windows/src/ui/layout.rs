//! Layout calculations for UI components.
//!
//! This module will contain layout calculations for the three-pane UI:
//! - Search bar at top
//! - Key list on left
//! - Value editor on right

/// Layout information for the window.
///
/// Will be populated as milestones require layout calculations.
pub struct Layout {
    // Will contain computed rects for each UI region
}

impl Layout {
    /// Computes layout for the given window dimensions.
    pub fn compute(_width: u32, _height: u32) -> Self {
        Self {}
    }
}
