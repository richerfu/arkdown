//! Raw schema for `AppScope/app.json5`.
//!
//! This module owns only the deserialization structs. Filesystem reading
//! and bundle-name validation belong to `arkdown-config` or
//! `arkdown-session`.

use serde::{Deserialize, Serialize};

/// Top-level struct for `AppScope/app.json5`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawAppJson5 {
    pub app: RawAppJson5App,
}

/// The `app` section of `AppScope/app.json5`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawAppJson5App {
    /// Application bundle name, e.g. `"com.example.myapp"`.
    #[serde(default)]
    pub bundle_name: String,
    /// Vendor name.
    #[serde(default)]
    pub vendor: String,
    /// Integer version code.
    #[serde(default)]
    pub version_code: u64,
    /// Human-readable version string.
    #[serde(default)]
    pub version_name: String,
    /// Minimum compatible SDK version.
    #[serde(default)]
    pub min_compatible_version_code: u64,
    /// Whether the app is a system app.
    #[serde(default)]
    pub bundle_type: Option<String>,
    /// Target API version (for forward-compat declarations).
    #[serde(default)]
    pub target_api_version: Option<u32>,
    /// Compile-time API version string.
    #[serde(default)]
    pub compile_sdk_version: Option<String>,
    /// Compile-time API version number.
    #[serde(default)]
    pub compile_sdk_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_app_json5() {
        let json = r#"{"app":{"bundleName":"com.example.app","versionCode":1,"versionName":"1.0.0"}}"#;
        let app: RawAppJson5 = serde_json::from_str(json).unwrap();
        assert_eq!(app.app.bundle_name, "com.example.app");
        assert_eq!(app.app.version_code, 1);
        assert_eq!(app.app.version_name, "1.0.0");
    }

    #[test]
    fn test_parse_empty_app_json5_defaults() {
        let json = r#"{"app":{}}"#;
        let app: RawAppJson5 = serde_json::from_str(json).unwrap();
        assert_eq!(app.app.bundle_name, "");
        assert_eq!(app.app.version_code, 0);
    }
}
