pub mod config;
pub use config::{
    AppConfig, AppConfigError, Config, GcConfig, GeneralConfig, LifecycleConfig, ShortcutsConfig,
    Theme,
};

pub(crate) mod key;
pub use key::{Key, KeyError, MAX_KEY_LENGTH};

pub(crate) mod metadata;

pub(crate) mod value;
pub use value::PublicValue as Value;
pub use value::{Attachment, LifecycleState, Metadata};

pub(crate) mod ttl_key;
pub use ttl_key::TtlKey;
