use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Debug or Release build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildMode {
    /// No optimization, debug info included, no obfuscation.
    Debug,
    /// Tree-shaking, optimization, obfuscation (if configured).
    Release,
}

/// The target artifact type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputTarget {
    /// Harmony Ability Package — deployable app module.
    Hap,
    /// Harmony Shared Package — shared library deployed once on-device.
    Hsp,
    /// Harmony Archive — library package (source or bytecode).
    Har,
    /// Preview mode — in-IDE live preview without signing.
    Preview,
    /// Unit test build.
    Test,
}

/// OHOS module type (from `module.json5` `module.type` field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OhosModuleType {
    /// `"entry"` — main HAP module.
    Entry,
    /// `"feature"` — feature HAP module.
    Feature,
    /// `"shared"` — HSP module.
    Shared,
    /// `"har"` — HAR library module.
    Har,
}

/// Compilation mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompileMode {
    /// ES module mode (for ArkTS stage model, API >= 9).
    EsModule,
    /// JS bundle mode (legacy FA model).
    JsBundle,
}

/// Top-level arkdown configuration (parsed from `build-profile.json5` and related files).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkdownConfig {
    pub project_root: PathBuf,
    pub build_mode: BuildMode,
    pub output_target: OutputTarget,
    pub product: ProductConfig,
    pub modules: Vec<ModuleConfig>,
    pub sdk_config: SdkConfig,
}

/// Per-module build configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    pub src_path: PathBuf,
    pub module_type: OhosModuleType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConfig {
    pub name: String,
    pub bundle_name: String,
    pub compatible_sdk_version: String,
    pub runtime_os: String,
    pub signing_config: Option<SigningConfig>,
    pub build_option: BuildOption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOption {
    pub strict_mode: StrictModeConfig,
    pub ark_options: Option<ArkOptions>,
    pub res_options: Option<ResOptions>,
    pub source_option: Option<SourceOption>,
    /// SO file paths listed under `buildOption.preloadSo` in `build-profile.json5`.
    /// Each path is resolved relative to the project root; files are embedded in the
    /// HAP under `libs/arm64-v8a/`.
    #[serde(default)]
    pub preload_so: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrictModeConfig {
    pub case_sensitive_check: bool,
    pub use_normalized_ohm_url: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkOptions {
    pub obfuscation: Option<ObfuscationConfig>,
    pub types: Vec<String>,
    pub runtime_only: Option<RuntimeOnlyConfig>,
}

/// Obfuscation configuration (from `buildOption.arkOptions.obfuscation` in
/// `build-profile.json5`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObfuscationConfig {
    pub enable_obfuscation: bool,
    /// Resolved, absolute paths to every rule file listed in
    /// `ruleOptions.rules`.  Empty when no rule files are configured.
    #[serde(default)]
    pub rule_files: Vec<PathBuf>,
}

/// Resource compiler options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResOptions {
    pub res_path: Option<PathBuf>,
}

/// Source language options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOption {
    pub source_language: Option<String>,
}

/// Runtime-only package configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeOnlyConfig {
    pub packages: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    /// Path to `ets-loader` directory. Unused — ets-loader is ported to Rust (SPEC-004a).
    pub ets_loader_path: PathBuf,
    /// Path to `es2abc` binary. Unused — es2abc is ported to Rust (SPEC-009, `arkdown-abc`).
    pub es2abc_path: PathBuf,
    /// Path to `restool` binary. Unused — restool is ported to Rust (SPEC-011, `arkdown-resource`).
    pub restool_path: PathBuf,
    /// SDK API version (numeric).
    pub compile_api_version: u32,
    /// Whether the SDK has the rollup plugin (required for stage mode).
    pub has_rollup_plugin: bool,
    /// DevEco Studio SDK version string discovered from SDK metadata.
    /// Written into `module.json` as `app.compileSdkVersion`.
    #[serde(default = "default_compile_sdk_version")]
    pub compile_sdk_version: String,
}

fn default_compile_sdk_version() -> String {
    "6.0.0.47".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    pub cert_path: PathBuf,
    pub key_alias: String,
    /// Encrypted key password.
    pub key_password: String,
    pub profile_path: PathBuf,
    pub sign_alg: String,
    pub store_file: PathBuf,
    /// Encrypted store password.
    pub store_password: String,
}
