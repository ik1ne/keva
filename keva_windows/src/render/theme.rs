//! Theme and layout constants.

use windows::UI::ViewManagement::{UIColorType, UISettings};

/// Window dimensions.
pub const WINDOW_WIDTH: i32 = 800;
pub const WINDOW_HEIGHT: i32 = 600;
pub const MIN_WINDOW_WIDTH: i32 = 400;
pub const MIN_WINDOW_HEIGHT: i32 = 300;

/// Theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    /// Detects the system theme preference using UISettings.
    /// Returns Dark if detection fails (safe default for dark backgrounds).
    pub fn detect_system() -> Self {
        // Use UISettings to get the foreground color and determine if it's light (= dark mode)
        // Dark mode has light foreground text on dark background
        let Ok(settings) = UISettings::new() else {
            eprintln!("[Theme] UISettings::new() failed, defaulting to Dark");
            return Theme::Dark;
        };

        let Ok(foreground) = settings.GetColorValue(UIColorType::Foreground) else {
            eprintln!("[Theme] GetColorValue failed, defaulting to Dark");
            return Theme::Dark;
        };

        // https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-windows-themes#know-when-dark-mode-is-enabled
        // Calculate perceived brightness: light foreground means dark mode
        // Formula: (5*G + 2*R + B) > (8 * 128)
        let brightness = 5 * foreground.G as u32 + 2 * foreground.R as u32 + foreground.B as u32;
        let is_foreground_light = brightness > 8 * 128;

        if is_foreground_light {
            Theme::Dark // Light foreground = dark mode
        } else {
            Theme::Light // Dark foreground = light mode
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}
