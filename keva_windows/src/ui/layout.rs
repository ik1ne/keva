//! Layout calculations for UI components.
//!
//! This module contains layout calculations for the three-pane UI:
//! - Search bar at top
//! - Key list on left
//! - Value editor on right

/// A rectangle in pixel coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Returns the right edge of the rectangle.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Returns the bottom edge of the rectangle.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Checks if a point is inside this rectangle.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

// Layout dimensions.
const SEARCH_BAR_HEIGHT: f32 = 48.0;
const SEARCH_ICON_SIZE: f32 = 40.0;
const SEARCH_ICON_MARGIN: f32 = 4.0;
const LEFT_PANE_WIDTH_RATIO: f32 = 0.35;
const PANE_DIVIDER_WIDTH: f32 = 1.0;

/// Layout information for the window.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// The search bar area (entire top bar).
    pub search_bar: Rect,
    /// The search icon (drag handle).
    pub search_icon: Rect,
    /// The search text input area.
    pub search_input: Rect,
    /// The left pane (key list area).
    pub left_pane: Rect,
    /// The right pane (preview/editor area).
    pub right_pane: Rect,
    /// The divider between left and right panes.
    pub divider: Rect,
}

impl Layout {
    /// Computes layout for the given window dimensions.
    pub fn compute(width: u32, height: u32) -> Self {
        let width = width as f32;
        let height = height as f32;

        // Search bar at top
        let search_bar = Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: SEARCH_BAR_HEIGHT,
        };

        // Search icon on the left of search bar
        let search_icon = Rect {
            x: SEARCH_ICON_MARGIN,
            y: (SEARCH_BAR_HEIGHT - SEARCH_ICON_SIZE) / 2.0,
            width: SEARCH_ICON_SIZE,
            height: SEARCH_ICON_SIZE,
        };

        // Search input takes remaining space in search bar
        let search_input = Rect {
            x: search_icon.right() + SEARCH_ICON_MARGIN,
            y: SEARCH_ICON_MARGIN,
            width: width - search_icon.right() - SEARCH_ICON_MARGIN * 2.0,
            height: SEARCH_BAR_HEIGHT - SEARCH_ICON_MARGIN * 2.0,
        };

        // Content area below search bar
        let content_top = SEARCH_BAR_HEIGHT;
        let content_height = height - content_top;

        // Left pane width (35% of window width)
        let left_pane_width = (width * LEFT_PANE_WIDTH_RATIO).floor();

        let left_pane = Rect {
            x: 0.0,
            y: content_top,
            width: left_pane_width,
            height: content_height,
        };

        let divider = Rect {
            x: left_pane_width,
            y: content_top,
            width: PANE_DIVIDER_WIDTH,
            height: content_height,
        };

        let right_pane = Rect {
            x: left_pane_width + PANE_DIVIDER_WIDTH,
            y: content_top,
            width: width - left_pane_width - PANE_DIVIDER_WIDTH,
            height: content_height,
        };

        Self {
            search_bar,
            search_icon,
            search_input,
            left_pane,
            right_pane,
            divider,
        }
    }

    /// Checks if the given screen coordinates are over the search icon.
    ///
    /// The coordinates should be relative to the window client area.
    pub fn is_over_search_icon(&self, x: f32, y: f32) -> bool {
        self.search_icon.contains(x, y)
    }
}
