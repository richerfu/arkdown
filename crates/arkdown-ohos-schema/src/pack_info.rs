//! Raw schema for `pack.info` — the HAP package info metadata.

use serde::{Deserialize, Serialize};

/// Top-level `pack.info` structure.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfo {
    /// Summary section.
    pub summary: RawPackInfoSummary,
    /// Package list.
    #[serde(default)]
    pub packages: Vec<RawPackInfoPackage>,
}

/// Summary section of `pack.info`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfoSummary {
    #[serde(default)]
    pub app: RawPackInfoApp,
    #[serde(default)]
    pub modules: Vec<RawPackInfoModuleSummary>,
}

/// App summary in `pack.info`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfoApp {
    #[serde(default)]
    pub bundle_name: String,
    #[serde(default)]
    pub version: RawPackInfoVersion,
}

/// Version in `pack.info`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfoVersion {
    #[serde(default)]
    pub code: u64,
    #[serde(default)]
    pub name: String,
}

/// Module summary in `pack.info`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfoModuleSummary {
    #[serde(rename = "mainAbility", default)]
    pub main_ability: String,
    #[serde(rename = "moduleName", default)]
    pub module_name: String,
    #[serde(rename = "moduleType", default)]
    pub module_type: String,
    #[serde(default)]
    pub device_type: Vec<String>,
}

/// A package entry in `pack.info`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPackInfoPackage {
    #[serde(rename = "packageName", default)]
    pub package_name: String,
    #[serde(rename = "moduleType", default)]
    pub module_type: String,
    #[serde(default)]
    pub delivery_with_install: bool,
    #[serde(default)]
    pub device_type: Vec<String>,
}
