//! Raw schema for the distro filter profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level distro filter profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawDistroFilterProfile {
    #[serde(default)]
    pub api_version: Option<serde_json::Value>,
    #[serde(default)]
    pub screen_shape: Option<serde_json::Value>,
    #[serde(default)]
    pub screen_density: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
