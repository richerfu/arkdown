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
pub fn parse_module_json(content: &str, file: &Path) -> Result<RawModuleJson, ConfigError> {
    json5::from_str::<RawModuleJson>(content).map_err(|e| ConfigError::ParseError {
        file: file.to_path_buf(),
        message: e.to_string(),
    })
}

/// Load and parse `module.json5` from the given path.
///
/// `module_root` is the module directory (e.g. `{project}/entry/`).
/// The file is expected at `{module_root}/src/main/module.json5`.
pub fn load_module_json(module_root: &Path) -> Result<RawModuleJson, ConfigError> {
    let path: PathBuf = module_root.join("src").join("main").join("module.json5");
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path));
    }
    let content = fs_err::read_to_string(&path)?;
    parse_module_json(&content, &path)
}

/// Map the raw `module.type` string to [`arkdown_core::OhosModuleType`].
pub fn map_module_type(raw: &str) -> arkdown_core::OhosModuleType {
    match raw {
        "feature" => arkdown_core::OhosModuleType::Feature,
        "shared" => arkdown_core::OhosModuleType::Shared,
        "har" => arkdown_core::OhosModuleType::Har,
        _ => arkdown_core::OhosModuleType::Entry, // "entry" or unknown → Entry
    }
}

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
        let m = parse_module_json(json, Path::new("module.json5")).unwrap();
        assert_eq!(m.module.name, "entry");
        assert_eq!(m.module.module_type, "entry");
        assert_eq!(
            m.module.src_entry,
            "./ets/entryability/EntryAbility.ets"
        );
        assert_eq!(m.module.device_types, vec!["phone", "tablet"]);
    }

    #[test]
    fn test_map_module_type() {
        assert_eq!(
            map_module_type("entry"),
            arkdown_core::OhosModuleType::Entry
        );
        assert_eq!(
            map_module_type("feature"),
            arkdown_core::OhosModuleType::Feature
        );
        assert_eq!(
            map_module_type("shared"),
            arkdown_core::OhosModuleType::Shared
        );
        assert_eq!(
            map_module_type("har"),
            arkdown_core::OhosModuleType::Har
        );
        // Unknown falls back to Entry
        assert_eq!(
            map_module_type("unknown"),
            arkdown_core::OhosModuleType::Entry
        );
    }
}
