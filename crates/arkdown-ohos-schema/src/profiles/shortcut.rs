//! Raw schema for the shortcut profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level shortcut profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawShortcutProfile {
    #[serde(default)]
    pub shortcuts: Vec<RawShortcutEntry>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A single shortcut entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawShortcutEntry {
    pub shortcut_id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
