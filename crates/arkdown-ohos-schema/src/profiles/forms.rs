//! Raw schema for the form profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level form profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawFormProfile {
    #[serde(default)]
    pub forms: Vec<RawFormEntry>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A single form entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFormEntry {
    pub name: String,
    #[serde(default)]
    pub src_entry: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
