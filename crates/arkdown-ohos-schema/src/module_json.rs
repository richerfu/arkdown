//! Raw schema for `src/main/module.json5`.

use serde::{Deserialize, Serialize};

/// Top-level struct for `module.json5`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawModuleJson {
    pub module: RawModuleEntry,
}

/// The `module` section of `module.json5`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawModuleEntry {
    /// Module name (e.g. `"entry"`, `"hsp-lib"`).
    #[serde(default)]
    pub name: String,
    /// Module type: `"entry"`, `"feature"`, `"shared"`, or `"har"`.
    #[serde(rename = "type", default)]
    pub module_type: String,
    /// Primary entry source file.
    #[serde(default)]
    pub src_entry: String,
    /// Supported device types.
    #[serde(default)]
    pub device_types: Vec<String>,
    /// Ability declarations.
    #[serde(default)]
    pub abilities: Vec<serde_json::Value>,
    /// Extension ability declarations.
    #[serde(default)]
    pub extension_abilities: Vec<serde_json::Value>,
    /// Proxy data declarations.
    #[serde(default)]
    pub proxy_data: Vec<serde_json::Value>,
    /// Router map configuration reference.
    #[serde(default)]
    pub router_map: Option<String>,
    /// Permission declarations.
    #[serde(default)]
    pub request_permissions: Vec<serde_json::Value>,
    /// Description resource reference.
    #[serde(default)]
    pub description: Option<String>,
    /// Main pages profile reference.
    #[serde(default)]
    pub main_element: Option<String>,
    /// Whether this module is installable on-demand.
    #[serde(default)]
    pub install_free: Option<bool>,
    /// Worker entry files (relative paths).
    #[serde(default)]
    pub workers: Vec<String>,
    /// Forms profile reference.
    #[serde(default)]
    pub ext_ability_forms: Vec<serde_json::Value>,
    /// Shortcut declarations.
    #[serde(default)]
    pub shortcuts: Vec<serde_json::Value>,
    /// Compile options.
    #[serde(default)]
    pub compile_option: Option<serde_json::Value>,
    /// Remaining raw fields preserved for round-trip.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_module_json() {
        let json = r#"{"module":{"name":"entry","type":"entry","srcEntry":"./ets/entryability/EntryAbility.ets","deviceTypes":["phone"]}}"#;
        let m: RawModuleJson = serde_json::from_str(json).unwrap();
        assert_eq!(m.module.name, "entry");
        assert_eq!(m.module.module_type, "entry");
        assert_eq!(
            m.module.src_entry,
            "./ets/entryability/EntryAbility.ets"
        );
    }

    #[test]
    fn test_parse_module_json_defaults() {
        let json = r#"{"module":{}}"#;
        let m: RawModuleJson = serde_json::from_str(json).unwrap();
        assert_eq!(m.module.name, "");
        assert_eq!(m.module.module_type, "");
        assert!(m.module.abilities.is_empty());
    }
}
