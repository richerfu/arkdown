//! Raw schema for `loader.json`.
//!
//! The bundler reads `loader.json` (written by `arkdown-profile`) instead of
//! `build-profile.json5` or `module.json5`. The `compileEntry` array is the
//! definitive list of entry points.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level `loader.json` structure.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawLoaderJson {
    /// Bundle name, e.g. `"com.example.app"`.
    #[serde(default)]
    pub bundle_name: String,
    /// OHOS module name, e.g. `"entry"`.
    #[serde(default)]
    pub module_name: String,
    /// Entry point configurations.
    #[serde(default)]
    pub compile_entry: Vec<RawCompileEntry>,
    /// Module path map: module-specifier → resolved artifact path.
    #[serde(default)]
    pub module_path_map: BTreeMap<String, String>,
    /// HAR name to OHM URL map.
    #[serde(default)]
    pub har_name_ohm_map: BTreeMap<String, String>,
    /// HSP name to OHM URL map.
    #[serde(default)]
    pub hsp_name_ohm_map: BTreeMap<String, String>,
    /// Remaining raw fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A single entry in `loader.json` `compileEntry`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCompileEntry {
    /// Entry kind, e.g. `"ability"`, `"pages"`, `"worker"`.
    #[serde(rename = "type")]
    pub entry_type: String,
    /// Source file path.
    #[serde(default)]
    pub source_file: String,
    /// Build function name for dynamic component entries.
    #[serde(default)]
    pub build_function: Option<String>,
    /// Whether this is a system library.
    #[serde(default)]
    pub is_ns: Option<bool>,
}
