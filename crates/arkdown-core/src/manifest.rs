use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{CompileMode, OhosModuleType};

/// Compiled ArkCompiler bytecode artifact for one OHOS module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbcArtifact {
    /// Absolute path to the output `.abc` file.
    pub path: PathBuf,
    /// API version this was compiled for (from `compatibleSdkVersion`).
    pub api_version: u32,
    /// Whether debug info is included.
    pub has_debug_info: bool,
    /// Source maps chain (`.chunk.js.map` files used as input to `es2abc`).
    pub source_maps: Vec<PathBuf>,
    /// Path to `es2abc` incremental cache file.
    pub cache_path: Option<PathBuf>,
}

/// ByteCode HAR dependency information (from `loader.json` `byteCodeHarInfo`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeHarInfo {
    pub package_name: String,
    pub abc_path: PathBuf,
    pub compatible_sdk_version: String,
    pub compatible_sdk_version_stage: Option<String>,
    pub source_maps_path: Option<PathBuf>,
    pub insight_intent_path: Option<PathBuf>,
}

/// Compiled OHOS resources (output of `arkdown-resource`/`restool`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhosResource {
    /// Path to compiled `resources.index` file.
    pub resources_index: PathBuf,
    /// Path to `ResourceTable.txt` (used by HSP consumers).
    pub resource_table: Option<PathBuf>,
    /// Profile JSON files written to `resources/base/profile/`.
    pub profiles: Vec<ResourceProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceProfile {
    /// Profile name, e.g. `"routerMap"`, `"startup_config"`.
    pub name: String,
    pub path: PathBuf,
}

/// Processed `module.json` (output of `ProcessProfile` + `MergeProfile`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HapManifest {
    pub module_name: String,
    pub module_type: OhosModuleType,
    pub device_types: Vec<String>,
    /// e.g. `"ark3.5.0.0"`
    pub virtual_machine: String,
    pub compile_mode: CompileMode,
    pub abilities: Vec<AbilityEntry>,
    pub extension_abilities: Vec<ExtensionAbilityEntry>,
    pub dependencies: Vec<HspDependency>,
    /// `$profile:` router map reference (if router map is generated).
    pub router_map: Option<String>,
    /// e.g. `"$profile:main_pages"`
    pub pages: Option<String>,
    pub main_element: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityEntry {
    pub name: String,
    pub src_entry: String,
    pub exported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_window_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_window_background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionAbilityEntry {
    pub name: String,
    pub src_entry: String,
    pub extension_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HspDependency {
    pub bundle_name: String,
    pub module_name: String,
}

/// Raw parsed `module.json5` (before processing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleJson5 {
    pub module: ModuleJson5Module,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleJson5Module {
    #[serde(default)]
    pub name: String,
    /// `"entry"` | `"feature"` | `"shared"` | `"har"`
    #[serde(rename = "type", default)]
    pub type_: String,
    pub main_element: Option<String>,
    #[serde(default)]
    pub device_types: Vec<String>,
    pub pages: Option<String>,
    #[serde(default)]
    pub abilities: Vec<AbilityConfig>,
    #[serde(default)]
    pub extension_abilities: Vec<ExtensionAbilityConfig>,
    pub src_entry: Option<String>,
    #[serde(default)]
    pub metadata: Vec<MetadataEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityConfig {
    pub name: String,
    /// e.g. `"./ets/entryability/EntryAbility.ets"`
    pub src_entry: String,
    #[serde(default)]
    pub exported: bool,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "startWindowIcon", default)]
    pub start_window_icon: Option<String>,
    #[serde(rename = "startWindowBackground", default)]
    pub start_window_background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionAbilityConfig {
    pub name: String,
    pub src_entry: String,
    pub extension_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub uris: Vec<SkillUri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUri {
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub name: String,
    pub value: Option<String>,
    pub resource: Option<String>,
}

/// Raw parsed `oh-package.json5`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhPackageJson5 {
    pub model_version: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub main: Option<String>,
    pub types: Option<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dynamic_dependencies: HashMap<String, String>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
    /// Internal metadata (used by SDK, ByteCode HAR metadata).
    pub metadata: Option<PackageMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    #[serde(default)]
    pub custom_types: Vec<String>,
    #[serde(default)]
    pub declaration_entry: Vec<String>,
    pub runtime_only: Option<crate::config::RuntimeOnlyConfig>,
    #[serde(default)]
    pub workers: Vec<String>,
    #[serde(default)]
    pub dependency_pkg_version: HashMap<String, String>,
}
