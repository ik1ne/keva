use super::LifecycleConfig;
use std::time::Duration;

/// TTL configuration passed to maintenance/GC operations.
#[derive(Clone, Copy)]
pub struct GcConfig {
    pub trash_ttl: Duration,
    pub purge_ttl: Duration,
}

impl From<&LifecycleConfig> for GcConfig {
    fn from(config: &LifecycleConfig) -> Self {
        Self {
            trash_ttl: Duration::from_secs(config.trash_ttl_days as u64 * 24 * 60 * 60),
            purge_ttl: Duration::from_secs(config.purge_ttl_days as u64 * 24 * 60 * 60),
        }
    }
}
