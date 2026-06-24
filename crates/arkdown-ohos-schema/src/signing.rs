//! Raw schema for HAP signing configuration and profile.

use serde::{Deserialize, Serialize};

/// Signing configuration supplied from `build-profile.json5`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawSigningConfig {
    pub cert_path: String,
    pub key_alias: String,
    /// Encrypted key password.
    pub key_password: String,
    pub profile_path: String,
    pub sign_alg: String,
    pub store_file: String,
    /// Encrypted store password.
    pub store_password: String,
}

/// App provisioning profile (`*.p7b` or embedded JSON).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawProvisionProfile {
    #[serde(default)]
    pub bundle_info: Option<RawBundleInfo>,
    #[serde(default)]
    pub profile_type: Option<String>,
    #[serde(default)]
    pub validity: Option<RawProfileValidity>,
}

/// Bundle info within a provisioning profile.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBundleInfo {
    #[serde(default)]
    pub app_identifier: String,
    #[serde(default)]
    pub bundle_name: String,
    #[serde(default)]
    pub developer_id: String,
}

/// Validity dates within a provisioning profile.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawProfileValidity {
    pub not_before: i64,
    pub not_after: i64,
}
