//! Metadata types for persistent application state.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Maintenance metadata. Missing fields default to None.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceMetadata {
    #[serde(default)]
    pub last_run_at: Option<SystemTime>,
}
