//! Raw schema for the permission reason profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level permission reason profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPermissionReasonProfile {
    #[serde(default)]
    pub permissions: Vec<RawPermissionEntry>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A single permission reason entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPermissionEntry {
    pub name: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
