//! Raw schema for `oh-package.json5`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level struct for `oh-package.json5`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawOhPackage {
    /// OHPM model version, e.g. `"5.0.0"`.
    #[serde(default)]
    pub model_version: String,
    /// Package name, e.g. `"entry"` or `"@scope/lib"`.
    #[serde(default)]
    pub name: String,
    /// Package version, e.g. `"1.0.0"`.
    #[serde(default)]
    pub version: String,
    /// Description string.
    #[serde(default)]
    pub description: Option<String>,
    /// Homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
    /// License identifier.
    #[serde(default)]
    pub license: Option<String>,
    /// Runtime dependencies: alias → specifier or version.
    ///
    /// Values may be a version range (e.g. `"^1.0.0"`), a local path
    /// (e.g. `"../hsp-lib"`), or an OHPM registry specifier.
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    /// Development-only dependencies.
    #[serde(default)]
    pub dev_dependencies: IndexMap<String, String>,
    /// Dynamic (lazy-loaded) dependencies.
    #[serde(default)]
    pub dynamic_dependencies: IndexMap<String, String>,
    /// Declare the package as a HAR or HSP library.
    #[serde(default)]
    pub package_type: Option<String>,
    /// Module entry point for the package.
    #[serde(default)]
    pub main: Option<String>,
    /// Types declaration entry.
    #[serde(default)]
    pub types: Option<String>,
    /// Remaining raw fields preserved for round-trip.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_oh_package() {
        let json = r#"{"name":"entry","version":"1.0.0","modelVersion":"5.0.0"}"#;
        let pkg: RawOhPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.name, "entry");
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.model_version, "5.0.0");
        assert!(pkg.dependencies.is_empty());
    }

    #[test]
    fn test_parse_oh_package_with_deps() {
        let json = r#"{"name":"entry","version":"1.0.0","dependencies":{"@ohos/hilog":"^1.0.0"}}"#;
        let pkg: RawOhPackage = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.dependencies.len(), 1);
        assert_eq!(pkg.dependencies["@ohos/hilog"], "^1.0.0");
    }
}
