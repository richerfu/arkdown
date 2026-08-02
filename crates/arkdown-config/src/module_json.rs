use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::ConfigError;

// ---------------------------------------------------------------------------
// Raw deserialization structs for module.json5
// ---------------------------------------------------------------------------

/// Top-level wrapper in `module.json5`.
#[derive(Debug, Deserialize)]
pub struct RawModuleJson {
    pub module: RawModuleEntry,
}

impl RawModuleJson {
    pub fn parse(content: &str, file: &Path) -> Result<RawModuleJson, ConfigError> {
        json5::from_str::<RawModuleJson>(content).map_err(|e| ConfigError::ParseError {
            file: file.to_path_buf(),
            message: e.to_string(),
        })
    }

    pub fn load(module_root: &Path) -> Result<RawModuleJson, ConfigError> {
        let path: PathBuf = module_root.join("src").join("main").join("module.json5");
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path));
        }
        let content = fs_err::read_to_string(&path)?;
        Self::parse(&content, &path)
    }

    pub fn from_value(value: &serde_json::Value, file: &Path) -> Result<Self, ConfigError> {
        serde_json::from_value(value.clone()).map_err(|error| ConfigError::ParseError {
            file: file.to_path_buf(),
            message: error.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawModuleEntry {
    #[serde(default)]
    pub name: String,
    /// Module type: `"entry"`, `"feature"`, `"shared"`, or `"har"`.
    #[serde(rename = "type", default)]
    pub module_type: String,
    /// Primary entry source file (e.g. `"./ets/entryability/EntryAbility.ets"`).
    #[serde(default)]
    pub src_entry: String,
    #[serde(default)]
    pub device_types: Vec<String>,
    #[serde(default)]
    pub abilities: Vec<serde_json::Value>,
    #[serde(default)]
    pub extension_abilities: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse the contents of a `module.json5` file.

/// Load and parse `module.json5` from the given path.
///
/// `module_root` is the module directory (e.g. `{project}/entry/`).
/// The file is expected at `{module_root}/src/main/module.json5`.

/// Map the raw `module.type` string to [`arkdown_core::OhosModuleType`].
// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_module_json_minimal() {
        let json = r#"{
            module: {
                name: "entry",
                type: "entry",
                srcEntry: "./ets/entryability/EntryAbility.ets",
                deviceTypes: ["phone", "tablet"],
                abilities: [],
                extensionAbilities: []
            }
        }"#;
        let m = RawModuleJson::parse(json, Path::new("module.json5")).unwrap();
        assert_eq!(m.module.name, "entry");
        assert_eq!(m.module.module_type, "entry");
        assert_eq!(m.module.src_entry, "./ets/entryability/EntryAbility.ets");
        assert_eq!(m.module.device_types, vec!["phone", "tablet"]);
    }

    #[test]
    fn test_parse_module_json_with_abilities() {
        let json = r#"{
            module: {
                name: "entry",
                type: "entry",
                srcEntry: "./ets/entryability/EntryAbility.ets",
                deviceTypes: ["phone"],
                abilities: [{
                    "name": "EntryAbility",
                    "srcEntry": "./ets/entryability/EntryAbility.ets",
                    "description": "$string:ability_description",
                    "exported": true
                }]
            }
        }"#;
        let m = RawModuleJson::parse(json, Path::new("module.json5")).unwrap();
        assert_eq!(m.module.abilities.len(), 1);
        let ability = &m.module.abilities[0];
        assert_eq!(ability["name"], "EntryAbility");
        assert_eq!(ability["srcEntry"], "./ets/entryability/EntryAbility.ets");
    }

    #[test]
    fn test_parse_module_json_har_type() {
        let json = r#"{
            module: {
                name: "mylib",
                type: "har",
                srcEntry: "./ets/index.ets",
                deviceTypes: ["phone", "tablet"]
            }
        }"#;
        let m = RawModuleJson::parse(json, Path::new("module.json5")).unwrap();
        assert_eq!(m.module.module_type, "har");
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str(&m.module.module_type),
            arkdown_core::OhosModuleType::Har
        );
    }

    #[test]
    fn test_parse_module_json_missing_optional_fields() {
        // Only required: `module` key. All inner fields have defaults.
        let json = r#"{ module: {} }"#;
        let m = RawModuleJson::parse(json, Path::new("module.json5")).unwrap();
        assert!(m.module.name.is_empty());
        assert!(m.module.device_types.is_empty());
        assert!(m.module.abilities.is_empty());
    }

    #[test]
    fn test_map_module_type() {
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str("entry"),
            arkdown_core::OhosModuleType::Entry
        );
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str("feature"),
            arkdown_core::OhosModuleType::Feature
        );
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str("shared"),
            arkdown_core::OhosModuleType::Shared
        );
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str("har"),
            arkdown_core::OhosModuleType::Har
        );
        // Unknown falls back to Entry
        assert_eq!(
            arkdown_core::OhosModuleType::from_type_str("unknown"),
            arkdown_core::OhosModuleType::Entry
        );
    }
}
