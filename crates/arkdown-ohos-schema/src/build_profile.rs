//! Raw schema for `build-profile.json5`.

use serde::{Deserialize, Serialize};

/// Top-level struct for `build-profile.json5`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawBuildProfile {
    pub app: RawApp,
    pub modules: Vec<RawBuildProfileModule>,
}

/// The `app` section of `build-profile.json5`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawApp {
    /// Signing configurations keyed by name.
    #[serde(default)]
    pub signing_configs: Vec<RawBuildProfileSigningConfig>,
    /// Product configurations.
    #[serde(default)]
    pub products: Vec<RawBuildProfileProduct>,
    /// Build mode (e.g. `"debug"`, `"release"`).
    #[serde(default)]
    pub build_mode_set: Vec<serde_json::Value>,
}

/// A signing configuration entry in `app.signingConfigs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildProfileSigningConfig {
    pub name: String,
    pub material: Option<RawSigningMaterial>,
}

/// Signing material fields inside `app.signingConfigs[].material`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSigningMaterial {
    pub cert_path: Option<String>,
    pub key_alias: Option<String>,
    pub key_password: Option<String>,
    pub profile: Option<String>,
    pub sign_alg: Option<String>,
    pub store_file: Option<String>,
    pub store_password: Option<String>,
}

/// A product entry in `app.products`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildProfileProduct {
    pub name: String,
    #[serde(default)]
    pub signing_config: Option<String>,
    #[serde(default)]
    pub compatible_sdk_version: Option<String>,
    #[serde(default)]
    pub runtime_os: Option<String>,
    #[serde(default)]
    pub build_option: Option<serde_json::Value>,
    #[serde(default)]
    pub version_code: Option<u64>,
    #[serde(default)]
    pub version_name: Option<String>,
}

/// A module entry in `build-profile.json5` `modules` array.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildProfileModule {
    pub name: String,
    /// Path to the module directory, e.g. `"./entry"`.
    pub src_path: String,
    /// Targets for this module.
    #[serde(default)]
    pub targets: Vec<RawBuildProfileTarget>,
}

/// A build target within a module entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBuildProfileTarget {
    pub name: String,
    #[serde(default)]
    pub apply_to_products: Vec<String>,
    #[serde(default)]
    pub source: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_build_profile() {
        let json = r#"{"app":{"signingConfigs":[],"products":[]},"modules":[]}"#;
        let bp: RawBuildProfile = serde_json::from_str(json).unwrap();
        assert!(bp.app.signing_configs.is_empty());
        assert!(bp.modules.is_empty());
    }

    #[test]
    fn test_parse_build_profile_with_module() {
        let json = r#"{
            "app": {"signingConfigs": [], "products": []},
            "modules": [{"name": "entry", "srcPath": "./entry", "targets": []}]
        }"#;
        let bp: RawBuildProfile = serde_json::from_str(json).unwrap();
        assert_eq!(bp.modules.len(), 1);
        assert_eq!(bp.modules[0].name, "entry");
        assert_eq!(bp.modules[0].src_path, "./entry");
    }
}
