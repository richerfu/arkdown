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

impl std::fmt::Display for OhosModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            OhosModuleType::Entry => "entry",
            OhosModuleType::Feature => "feature",
            OhosModuleType::Shared => "shared",
            OhosModuleType::Har => "har",
        })
    }
}

impl OhosModuleType {
    /// Parse the module type from the `module.json5` `type` field string.
    ///
    /// Mirrors hvigor's type mapping: unknown strings fall back to `Entry`.
    pub fn from_type_str(s: &str) -> Self {
        match s {
            "feature" => OhosModuleType::Feature,
            "shared" => OhosModuleType::Shared,
            "har" => OhosModuleType::Har,
            _ => OhosModuleType::Entry, // "entry" or unknown → Entry
        }
    }

    /// Returns `true` if this module type should be included when building
    /// for the given `target` artifact type.
    ///
    /// Mirrors hvigor's `module.modules` filter in `CompileArkTS`:
    /// - HAP builds include `entry` and `feature` modules.
    /// - HSP builds include `shared` modules only.
    /// - HAR builds include `har` modules only.
    /// - Preview/Test builds include all module types.
    pub fn matches_output_target(&self, target: &OutputTarget) -> bool {
        match target {
            OutputTarget::Hap => matches!(self, OhosModuleType::Entry | OhosModuleType::Feature),
            OutputTarget::Hsp => matches!(self, OhosModuleType::Shared),
            OutputTarget::Har => matches!(self, OhosModuleType::Har),
            OutputTarget::Preview | OutputTarget::Test => true,
        }
    }
}

/// Compilation mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompileMode {
    /// ES module mode (for ArkTS stage model, API >= 9).
    EsModule,
    /// JS bundle mode (legacy FA model).
    JsBundle,
}

impl std::fmt::Display for CompileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CompileMode::EsModule => "esmodule",
            CompileMode::JsBundle => "jsbundle",
        })
    }
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
    pub compatible_sdk_version_stage: Option<String>,
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
    /// Whether hvigor's `buildOption.preloadSystemSo` collection is enabled.
    /// This gates startup/resource handling for ByteCode HAR `META-INF/preload.json`.
    #[serde(default)]
    pub preload_system_so: bool,
    /// SO file paths listed under `buildOption.preloadSo` in `build-profile.json5`.
    /// Each path is resolved relative to the project root; files are embedded in the
    /// HAP under `libs/arm64-v8a/`.
    #[serde(default)]
    pub preload_so: Vec<PathBuf>,
    /// Permissions to remove from HAR/HSP dependency `requestPermissions`.
    /// Matches hvigor's `MergeProfile.getRemovePermissionSet()` behavior.
    #[serde(default)]
    pub remove_permissions: Vec<String>,
    /// AOT compile mode from `buildOption.aotCompileMode` or
    /// `buildOption.arkOptions.aotCompileMode`: `""`, `"type"`, or `"partial"`.
    /// API 10+ ignores this value in hvigor and derives the mode from `hostPGO`.
    #[serde(default)]
    pub aot_compile_mode: Option<String>,
    /// Whether `buildOption.arkOptions.hostPGO` requests partial AOT on API 10+.
    #[serde(default)]
    pub host_pgo: Option<bool>,
    /// Path to AOT profile data (`.ap` file) for PGO-based AOT. Relative paths
    /// are module-root relative, matching hvigor's `TargetTaskService.getApPath()`.
    #[serde(default)]
    pub ap_path: Option<PathBuf>,
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
    /// Whether to allow the bundler to split output into multiple chunks
    /// (`buildOption.arkOptions.supportChunks` in `build-profile.json5`).
    #[serde(default)]
    pub support_chunks: bool,
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

impl ObfuscationConfig {
    /// Returns `true` when obfuscation should run for the given build mode.
    ///
    /// Obfuscation is only active in `Release` builds with
    /// `enable_obfuscation = true`.
    pub fn is_enabled_for_mode(&self, build_mode: &BuildMode) -> bool {
        matches!(build_mode, BuildMode::Release) && self.enable_obfuscation
    }
}

/// Resource compiler options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResOptions {
    pub res_path: Option<PathBuf>,
    /// Whether to include AppScope/resources in the resource compilation.
    /// Defaults to `true` in hvigor; set to `false` for shared modules that
    /// don't need the app-scope resources.
    #[serde(default = "default_true")]
    pub include_app_scope_res: bool,
    /// Additional resource directories to include (from `buildOption.resOptions.directories`).
    #[serde(default)]
    pub resource_directories: Vec<PathBuf>,
}

fn default_true() -> bool {
    true
}

/// Source language options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOption {
    pub source_language: Option<String>,
    /// Worker entry files from `buildOption.sourceOption.workers` in
    /// `build-profile.json5`. Resolved to absolute paths relative to the
    /// project root during config resolution.
    #[serde(default)]
    pub workers: Vec<PathBuf>,
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

impl SdkConfig {
    /// Derive the `ets/` root directory from `ets_loader_path`.
    ///
    /// `ets_loader_path` is e.g. `<sdk>/ets/build-tools/ets-loader`, so the
    /// ETS root is two levels up: `<sdk>/ets`.
    fn ets_root(&self) -> Option<&std::path::Path> {
        self.ets_loader_path.parent()?.parent()
    }

    /// Derive the SDK component root from `ets_loader_path`.
    ///
    /// `ets_loader_path` is e.g. `<sdk>/ets/build-tools/ets-loader`, so the
    /// component root is three levels up: `<sdk>`.
    fn component_root(&self) -> Option<&std::path::Path> {
        self.ets_root()?.parent()
    }

    /// Return the SDK PAC schema path when this SDK provides one.
    ///
    /// hvigor skips PAC/privacy processing when `toolchains/paccheck/pacSchema.json`
    /// is absent, so callers should treat `None` as "task should not run".
    pub fn pac_schema_path(&self) -> Option<std::path::PathBuf> {
        let path = self
            .component_root()?
            .join("toolchains/paccheck/pacSchema.json");
        path.is_file().then_some(path)
    }

    /// SDK permission oracle used by hvigor for requestPermissions checks.
    ///
    /// The previewer component stores SDK `definePermissions` in
    /// `<sdk>/previewer/common/resources/module.json`.
    pub fn permission_module_json_path(&self) -> Option<std::path::PathBuf> {
        let path = self
            .component_root()?
            .join("previewer/common/resources/module.json");
        path.is_file().then_some(path)
    }

    /// Candidate directories containing `@ohos/*` API declaration files.
    ///
    /// Returns `["<sdk>/ets/api", "<sdk>/js/api"]` in priority order.
    pub fn ets_api_dirs(&self) -> Vec<std::path::PathBuf> {
        let Some(ets_root) = self.ets_root() else {
            return vec![];
        };
        let mut dirs = vec![ets_root.join("api")];
        if let Some(sdk_root) = ets_root.parent() {
            dirs.push(sdk_root.join("js/api"));
        }
        dirs
    }

    /// Candidate directories containing `@kit.*` declaration files.
    ///
    /// Returns `["<sdk>/ets/kits"]`.
    pub fn ets_kits_dirs(&self) -> Vec<std::path::PathBuf> {
        self.ets_root()
            .map(|ets_root| vec![ets_root.join("kits")])
            .unwrap_or_default()
    }
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
