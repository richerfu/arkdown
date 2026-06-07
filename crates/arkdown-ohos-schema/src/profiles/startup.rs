//! Raw schema for the startup profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level startup profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawStartupProfile {
    /// Startup task entries.
    #[serde(default)]
    pub startup_tasks: Vec<RawStartupTask>,
    /// Optional config entry list.
    #[serde(default)]
    pub config_entry: Option<Vec<serde_json::Value>>,
    /// Remaining raw fields.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// A single startup task entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawStartupTask {
    pub name: String,
    #[serde(default)]
    pub src_entry: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub run_on_thread: Option<String>,
    #[serde(default)]
    pub wait_on_main_thread: Option<bool>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
