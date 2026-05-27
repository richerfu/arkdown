use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::ConfigError;

// ---------------------------------------------------------------------------
// Raw deserialization struct for oh-package.json5
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawOhPackage {
    #[serde(default)]
    pub model_version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dynamic_dependencies: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse the contents of an `oh-package.json5` file.
pub fn parse_oh_package(content: &str, file: &Path) -> Result<RawOhPackage, ConfigError> {
    json5::from_str::<RawOhPackage>(content).map_err(|e| ConfigError::ParseError {
        file: file.to_path_buf(),
        message: e.to_string(),
    })
}

/// Load and parse `oh-package.json5` from a module's root directory.
///
/// `module_root` is the path to the module directory (e.g. `{project}/entry/`).
pub fn load_oh_package(module_root: &Path) -> Result<RawOhPackage, ConfigError> {
    let path: PathBuf = module_root.join("oh-package.json5");
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path));
    }
    let content = fs_err::read_to_string(&path)?;
    parse_oh_package(&content, &path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_oh_package_minimal() {
        let json = r#"{
            modelVersion: "5.0.0",
            name: "entry",
            version: "1.0.0",
        }"#;
        let pkg = parse_oh_package(json, Path::new("oh-package.json5")).unwrap();
        assert_eq!(pkg.model_version, "5.0.0");
        assert_eq!(pkg.name, "entry");
        assert_eq!(pkg.version, "1.0.0");
        assert!(pkg.dependencies.is_empty());
    }

    #[test]
    fn test_parse_oh_package_with_deps() {
        let json = r#"{
            modelVersion: "5.0.0",
            name: "entry",
            version: "1.0.0",
            dependencies: {
                "@ohos/lib": "1.2.3"
            },
            devDependencies: {
                "some-tool": "^2.0.0"
            },
            dynamicDependencies: {}
        }"#;
        let pkg = parse_oh_package(json, Path::new("oh-package.json5")).unwrap();
        assert_eq!(pkg.dependencies.len(), 1);
        assert_eq!(pkg.dependencies["@ohos/lib"], "1.2.3");
        assert_eq!(pkg.dev_dependencies.len(), 1);
    }
}
