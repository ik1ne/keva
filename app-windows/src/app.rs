//! Application state and keva_core integration.

use std::path::PathBuf;
use std::time::Duration;

use keva_core::core::KevaCore;
use keva_core::types::{Config, Key, SavedConfig};
use windows::Win32::Foundation::HWND;

use crate::renderer::Renderer;

/// Application state.
pub struct App {
    /// The keva_core storage instance.
    pub core: KevaCore,
    /// Cached list of active keys.
    keys: Vec<Key>,
    /// Direct2D renderer.
    renderer: Renderer,
}

/// Error type for App initialization.
#[derive(Debug)]
pub enum AppError {
    Storage(keva_core::core::error::StorageError),
    Renderer(windows::core::Error),
}

impl From<keva_core::core::error::StorageError> for AppError {
    fn from(e: keva_core::core::error::StorageError) -> Self {
        AppError::Storage(e)
    }
}

impl From<windows::core::Error> for AppError {
    fn from(e: windows::core::Error) -> Self {
        AppError::Renderer(e)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Storage(e) => write!(f, "storage error: {e}"),
            AppError::Renderer(e) => write!(f, "renderer error: {e}"),
        }
    }
}

impl App {
    /// Creates a new App instance, initializing keva_core and renderer.
    pub fn new() -> Result<Self, AppError> {
        let base_path = data_dir();

        // Ensure data directory exists
        if let Err(e) = std::fs::create_dir_all(&base_path) {
            eprintln!("Failed to create data directory: {e}");
        }

        let config = Config {
            base_path,
            saved: SavedConfig {
                trash_ttl: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
                purge_ttl: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
                inline_threshold_bytes: 1024 * 1024,               // 1 MB
            },
        };

        let core = KevaCore::open(config)?;
        let keys = core.active_keys()?;
        let renderer = Renderer::new()?;

        eprintln!("Loaded {} keys", keys.len());

        Ok(Self {
            core,
            keys,
            renderer,
        })
    }

    /// Returns the list of active keys.
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    /// Reloads the key list from storage.
    pub fn reload_keys(&mut self) -> Result<(), keva_core::core::error::StorageError> {
        self.keys = self.core.active_keys()?;
        Ok(())
    }

    /// Paints the window content using Direct2D.
    pub fn paint(&mut self, hwnd: HWND) {
        if let Err(e) = self.renderer.ensure_target(hwnd) {
            eprintln!("Failed to ensure render target: {e}");
            return;
        }

        if let Err(e) = self.renderer.render(&self.keys) {
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

/// Returns the Keva data directory.
///
/// Uses `%USERPROFILE%\.keva` on Windows, or `KEVA_DATA_DIR` env var if set.
fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("KEVA_DATA_DIR") {
        return PathBuf::from(dir);
    }

    // Use %USERPROFILE%\.keva on Windows
    if let Ok(home) = std::env::var("USERPROFILE") {
        return PathBuf::from(home).join(".keva");
    }

    // Fallback to current directory
    PathBuf::from(".keva")
}
