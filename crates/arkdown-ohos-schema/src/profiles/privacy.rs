//! Raw schema for the privacy profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level privacy profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPrivacyProfile {
    #[serde(default)]
    pub data_collect_clauses: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
