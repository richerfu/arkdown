//! Raw schema for the cross-app share profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level cross-app share profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawShareProfile {
    /// Share targets.
    #[serde(default)]
    pub targets: Vec<RawShareTarget>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A single share target entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawShareTarget {
    pub name: String,
    #[serde(default)]
    pub module_name: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
