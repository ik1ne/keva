use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use thiserror::Error;

/// User-facing application configuration, persisted as config.toml.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub shortcuts: ShortcutsConfig,
    #[serde(default)]
    pub lifecycle: LifecycleConfig,
}

impl AppConfig {
    /// Returns the config file path within the given data directory.
    pub fn path(data_dir: &Path) -> std::path::PathBuf {
        data_dir.join("config.toml")
    }

    /// Loads config from a TOML file. Returns default config if file doesn't exist.
    pub fn load(path: &Path) -> Result<Self, AppConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Saves config to a TOML file.
    pub fn save(&self, path: &Path) -> Result<(), AppConfigError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validates config values and returns list of validation errors.
    /// Returns empty vec if config is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.lifecycle.trash_ttl_days == 0 {
            errors.push("trash_ttl_days must be at least 1".to_string());
        }

        if self.lifecycle.purge_ttl_days == 0 {
            errors.push("purge_ttl_days must be at least 1".to_string());
        }

        errors
    }

    /// Returns a validated config, replacing invalid values with defaults.
    pub fn with_defaults_for_invalid(&self) -> Self {
        let defaults = Self::default();
        Self {
            general: self.general.clone(),
            shortcuts: self.shortcuts.clone(),
            lifecycle: LifecycleConfig {
                trash_ttl_days: if self.lifecycle.trash_ttl_days == 0 {
                    defaults.lifecycle.trash_ttl_days
                } else {
                    self.lifecycle.trash_ttl_days
                },
                purge_ttl_days: if self.lifecycle.purge_ttl_days == 0 {
                    defaults.lifecycle.purge_ttl_days
                } else {
                    self.lifecycle.purge_ttl_days
                },
            },
        }
    }
}

/// General application settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_true")]
    pub show_tray_icon: bool,
    #[serde(default)]
    pub welcome_shown: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            show_tray_icon: true,
            welcome_shown: false,
        }
    }
}

/// Theme preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    #[default]
    System,
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Theme::Dark => write!(f, "dark"),
            Theme::Light => write!(f, "light"),
            Theme::System => write!(f, "system"),
        }
    }
}

/// Keyboard shortcut settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default = "default_global_shortcut")]
    pub global_shortcut: String,
    #[serde(default = "default_focus_search")]
    pub focus_search: String,
    #[serde(default = "default_copy_markdown")]
    pub copy_markdown: String,
    #[serde(default = "default_copy_html")]
    pub copy_html: String,
    #[serde(default = "default_copy_files")]
    pub copy_files: String,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            global_shortcut: default_global_shortcut(),
            focus_search: default_focus_search(),
            copy_markdown: default_copy_markdown(),
            copy_html: default_copy_html(),
            copy_files: default_copy_files(),
        }
    }
}

fn default_global_shortcut() -> String {
    "Ctrl+Alt+KeyK".to_string()
}

fn default_focus_search() -> String {
    "Ctrl+KeyS".to_string()
}

fn default_copy_markdown() -> String {
    "Ctrl+KeyT".to_string()
}

fn default_copy_html() -> String {
    "Ctrl+KeyR".to_string()
}

fn default_copy_files() -> String {
    "Ctrl+KeyF".to_string()
}

/// Lifecycle/TTL settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LifecycleConfig {
    #[serde(default = "default_trash_ttl_days")]
    pub trash_ttl_days: u32,
    #[serde(default = "default_purge_ttl_days")]
    pub purge_ttl_days: u32,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            trash_ttl_days: default_trash_ttl_days(),
            purge_ttl_days: default_purge_ttl_days(),
        }
    }
}

fn default_trash_ttl_days() -> u32 {
    30
}

fn default_purge_ttl_days() -> u32 {
    7
}

fn default_true() -> bool {
    true
}

/// Errors that can occur when loading or saving config.
#[derive(Debug, Error)]
pub enum AppConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}
