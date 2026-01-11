mod app;
mod core;
mod gc;

pub use app::{
    AppConfig, AppConfigError, GeneralConfig, LifecycleConfig, ShortcutsConfig, Theme,
};
pub use core::Config;
pub use gc::GcConfig;
