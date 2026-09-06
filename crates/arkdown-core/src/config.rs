use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Whether `runtime_os` selects the HarmonyOS (HMS) toolchain variant.
///
/// Canonical predicate for every runtime-OS branch in the workspace. Anything
/// that is not HarmonyOS (i.e. OpenHarmony) takes the OpenHarmony path, so
/// "is OpenHarmony" must be spelled `!is_harmony_os(..)` — never a separate
/// `eq_ignore_ascii_case("OpenHarmony")` check, which would disagree on any
/// third runtime-OS string.
pub fn is_harmony_os(runtime_os: &str) -> bool {
    runtime_os.eq_ignore_ascii_case("HarmonyOS")
}

/// Hvigor build mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildMode {
    /// No optimization, debug info included, no obfuscation.
    Debug,
    /// Tree-shaking, optimization, obfuscation (if configured).
    Release,
    /// Device-test build. Hvigor preserves the `test` mode for target and
    /// build-option binding, while its preset compile behavior is debug-like.
    Test,
}

impl BuildMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::Test => "test",
        }
    }

    pub const fn is_debuggable(&self) -> bool {
        matches!(self, Self::Debug | Self::Test)
    }

    pub const fn is_release(&self) -> bool {
        matches!(self, Self::Release)
    }
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
    /// Product-level output artifact name from `app.products[].output.artifactName`.
    ///
    /// Latest hvigor uses this for both module and project package naming. A
    /// missing value preserves the project/module-derived default names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_name: Option<String>,
    pub bundle_name: String,
    pub compatible_sdk_version: String,
    pub compatible_sdk_version_stage: Option<String>,
    /// Product-level `targetSdkVersion` from `build-profile.json5`.
    ///
    /// Hvigor uses the complete version text (for example `5.0.5(17)`) when
    /// encoding `targetAPIVersion`; the selected SDK only supplies the compile
    /// toolchain and must not replace these version components.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_sdk_version: Option<String>,
    pub runtime_os: String,
    pub signing_config: Option<SigningConfig>,
    pub build_option: BuildOption,
    /// Product-level `versionCode` override from `build-profile.json5`.
    ///
    /// When present, overrides the `versionCode` field from `AppScope/app.json5`
    /// in the compiled `module.json` app section.  Matches hvigor's product-level
    /// app override semantics.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version_code: Option<u64>,
    /// Product-level `versionName` override from `build-profile.json5`.
    ///
    /// When present, overrides the `versionName` field from `AppScope/app.json5`
    /// in the compiled `module.json` app section.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version_name: Option<String>,
    /// Effective build version from the selected product or the highest-precedence
    /// build invocation override.
    ///
    /// Hvigor resolves `buildVersion` as command extra config → product →
    /// `AppScope/app.json5` and propagates it to BuildProfile, module metadata,
    /// pack.info, and integrated-HSP normalization.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub build_version: Option<String>,
    /// Effective app-profile fields supplied by the selected product or by
    /// `project/hvigorfile.*` `config.ohos.overrides.appOpt`.
    ///
    /// Hvigor applies these fields after `AppScope/app.json5`; build-script
    /// values are folded into this overlay after product values.
    #[serde(default)]
    pub app_options: AppOptionOverride,
}

/// App-profile values that override `AppScope/app.json5`.
///
/// Upstream: `AppOpt` in
/// `hvigor-ohos-plugin/src/options/build/app-ohos-config.d.ts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppOptionOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
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
    /// `buildOption.packOptions.deduplicateHar`.
    #[serde(default)]
    pub deduplicate_har: bool,
    /// `buildOption.packOptions.appWithSignedPkg`.
    ///
    /// When enabled, hvigor emits an additional APP whose embedded HAP/HSP
    /// archives are signed before the outer APP is signed.
    #[serde(default)]
    pub app_with_signed_pkg: bool,
    /// `buildOption.packOptions.enableSourceCodeCheck` (default true): warn
    /// when the packaged outputs contain stray source code files.
    #[serde(default = "default_true_enable_source_code_check")]
    pub enable_source_code_check: bool,
    /// `buildOption.generateSharedTgz` for HSP outputs.
    ///
    /// `None` preserves hvigor's mode-dependent default: enabled for release
    /// builds and disabled for debug builds.
    #[serde(default)]
    pub generate_shared_tgz: Option<bool>,
    /// Absolute path to `buildOption.packingOptions.customizedOptions.basePackage`.
    ///
    /// Hvigor resolves this HAR base ABC relative to the module root before it
    /// constructs the ArkTS compiler configuration.
    #[serde(default)]
    pub customized_har_base_package: Option<PathBuf>,
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
    /// ABI filter list from `buildOption.externalNativeOptions.abiFilters`.
    /// Controls which native SO library directories are collected.
    /// Empty means default: `["arm64-v8a"]`.
    #[serde(default)]
    pub abi_filters: Vec<String>,
    /// Native CMake configuration from `buildOption.externalNativeOptions`.
    /// `None` means hvigor's native configure/build tasks must not run.
    #[serde(default)]
    pub external_native_options: Option<ExternalNativeOptions>,
    /// HAR artifact representation selected by `buildOption.artifactType`.
    #[serde(default)]
    pub har_artifact_type: HarArtifactType,
    /// Native library collection options from `buildOption.nativeLib`.
    #[serde(default)]
    pub native_lib: NativeLibOption,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HarArtifactType {
    #[default]
    Original,
    Obfuscation,
    Bytecode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalNativeOptions {
    /// Absolute path to the configured `CMakeLists.txt`; `None` selects the
    /// conventional script when the option object is present.
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub abi_filters: Vec<String>,
    pub cpp_flags: Option<String>,
    pub c_flags: Option<String>,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeLibOption {
    /// `buildOption.nativeLib.collectAllLibs`; when true hvigor scans `**/*`
    /// instead of `**/*.so*(.*[0-9])`.
    #[serde(default)]
    pub collect_all_libs: bool,
    /// `TargetTaskService.getNativeLibOption().filter`, merged from
    /// `napiLibFilterOption` and `nativeLib.filter`.
    #[serde(default)]
    pub filter: NativeLibFilterOption,
    /// Native C/C++ header directories exported under `include/` in HARs.
    #[serde(default)]
    pub header_paths: Vec<PathBuf>,
    /// `nativeLib.librariesInfo`: link-interface metadata exported through
    /// ConfigureCmake-generated imported targets.
    #[serde(default)]
    pub libraries_info: Vec<NativeLibraryInfo>,
    /// Whether transitive HAR runtime libraries are excluded from HAR output.
    #[serde(default = "default_true")]
    pub exclude_from_har: bool,
    /// Whether HSP native libraries are omitted from the generated interface
    /// HAR. Hvigor ignores this switch when `headerPath` is configured because
    /// consumers need both headers and link libraries in that case.
    #[serde(default)]
    pub exclude_so_from_interface_har: bool,
    /// Glob patterns from `nativeLib.excludeSoFromBinXO`.
    #[serde(default)]
    pub exclude_so_from_binxo: Vec<String>,
    /// Native debug-symbol/strip policy.
    #[serde(default)]
    pub debug_symbol: NativeDebugSymbolOption,
    /// Whether `nativeLib.debugSymbol` was present in the effective Hvigor
    /// configuration. The SDK omits `debugSymbol` from native cache JSON when
    /// the option is absent, even though strip behavior still defaults true.
    #[serde(skip)]
    pub debug_symbol_configured: bool,
}

impl Default for NativeLibOption {
    fn default() -> Self {
        Self {
            collect_all_libs: false,
            filter: NativeLibFilterOption::default(),
            header_paths: Vec::new(),
            libraries_info: Vec::new(),
            exclude_from_har: true,
            exclude_so_from_interface_har: false,
            exclude_so_from_binxo: Vec::new(),
            debug_symbol: NativeDebugSymbolOption::default(),
            debug_symbol_configured: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeLibraryInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub link_libraries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeDebugSymbolOption {
    #[serde(default = "default_true")]
    pub strip: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for NativeDebugSymbolOption {
    fn default() -> Self {
        Self {
            strip: true,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeLibFilterOption {
    /// `filter.excludes`.
    #[serde(default)]
    pub excludes: Vec<String>,
    /// `filter.pickFirsts`.
    #[serde(default)]
    pub pick_firsts: Vec<String>,
    /// `filter.pickLasts`.
    #[serde(default)]
    pub pick_lasts: Vec<String>,
    /// `filter.enableOverride`.
    #[serde(default)]
    pub enable_override: bool,
    /// `filter.select`.
    #[serde(default)]
    pub select: Vec<NativeLibPackageSelection>,
}

impl NativeLibFilterOption {
    pub fn is_configured(&self) -> bool {
        self.enable_override
            || !self.excludes.is_empty()
            || !self.pick_firsts.is_empty()
            || !self.pick_lasts.is_empty()
            || !self.select.is_empty()
    }

    pub fn has_order_sensitive_rules(&self) -> bool {
        self.enable_override || !self.pick_firsts.is_empty() || !self.pick_lasts.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeLibPackageSelection {
    /// `select[].package`.
    #[serde(default)]
    pub package: String,
    /// `select[].version`.
    #[serde(default)]
    pub version: Option<String>,
    /// `select[].include`.
    #[serde(default)]
    pub include: Option<Vec<String>>,
    /// `select[].exclude`.
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
    /// `select[].includePattern`.
    #[serde(default)]
    pub include_pattern: Option<Vec<String>>,
    /// `select[].excludePattern`.
    #[serde(default)]
    pub exclude_pattern: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiCompatibilityCheckLevel {
    #[default]
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrictModeConfig {
    pub case_sensitive_check: bool,
    pub use_normalized_ohm_url: bool,
    /// `strictMode.noExternalImportByPath`; absence is significant because
    /// hvigor defaults it to `true` only when normalized OHM URLs are enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_external_import_by_path: Option<bool>,
    /// Project APP preflight from hvigor's
    /// `DuplicateDependencyCheck.doTaskAction`.
    #[serde(default)]
    pub duplicate_dependency_check: bool,
    /// Source checker compatibility diagnostic level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_compatibility_check: Option<ApiCompatibilityCheckLevel>,
}

impl StrictModeConfig {
    /// Effective value returned by hvigor
    /// `TargetTaskService.isNoExternalImportByPath()`.
    pub fn is_no_external_import_by_path(&self) -> bool {
        self.no_external_import_by_path
            .unwrap_or(self.use_normalized_ohm_url)
    }
}

#[cfg(test)]
mod strict_mode_tests {
    use super::StrictModeConfig;

    #[test]
    fn no_external_import_defaults_to_normalized_ohm_url_like_hvigor() {
        let normalized = StrictModeConfig {
            case_sensitive_check: false,
            use_normalized_ohm_url: true,
            no_external_import_by_path: None,
            duplicate_dependency_check: false,
            api_compatibility_check: None,
        };
        assert!(normalized.is_no_external_import_by_path());

        let explicitly_relaxed = StrictModeConfig {
            no_external_import_by_path: Some(false),
            ..normalized
        };
        assert!(!explicitly_relaxed.is_no_external_import_by_path());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkOptions {
    pub obfuscation: Option<ObfuscationConfig>,
    pub types: Vec<String>,
    pub runtime_only: Option<RuntimeOnlyConfig>,
    /// Constants emitted into the module's generated `BuildProfile.ets`.
    ///
    /// Hvigor preserves declaration order and only accepts primitive
    /// string/number/boolean values. Validation happens in
    /// `arkdown-profile::BuildProfileWriter`.
    #[serde(default)]
    pub build_profile_fields: serde_json::Map<String, serde_json::Value>,
    /// Whether to allow the bundler to split output into multiple chunks
    /// (`buildOption.arkOptions.supportChunks` in `build-profile.json5`).
    #[serde(default)]
    pub support_chunks: bool,
    /// Explicit `buildOption.arkOptions.byteCodeHar` value.
    ///
    /// Presence is significant in hvigor 6.24.3: when this field is absent,
    /// `strictMode.useNormalizedOHMUrl=true` makes a HAR a ByteCode HAR by
    /// default, while an explicit `false` disables that default.
    #[serde(default)]
    pub byte_code_har: Option<bool>,
    /// Whether a ByteCode HAR embeds and compiles its source-HAR dependencies
    /// (`buildOption.arkOptions.bundledDependencies` in hvigor).
    #[serde(default)]
    pub bundled_dependencies: bool,
    /// Merge every declaration reachable from a HAR/HSP public entry into a
    /// self-contained declaration bundle.
    ///
    /// This is `buildOption.arkOptions.bundle.bundledDeclare` in the SDK
    /// compiler.  It is deliberately separate from `bundledDependencies`:
    /// the former controls type declaration flattening, while the latter
    /// controls ByteCode HAR source embedding.
    #[serde(default)]
    pub bundled_declare: bool,
    /// HSP dependencies are embedded into the host package only when this
    /// hvigor switch is explicitly enabled.
    #[serde(default)]
    pub integrated_hsp: Option<bool>,
    /// Widget card configuration from `buildOption.arkOptions.widget`
    /// (hvigor 6.24.3). `transitiveDeps` switches on the per-ByteCode-HAR
    /// grouping of transitively blocked compile files written to the widget
    /// loader.json `pkgName2OtherCompileFiles` field.
    #[serde(default)]
    pub widget: Option<WidgetArkOptions>,
    /// `buildOption.arkOptions.branchElimination` — es2abc `--branch-elimination`
    /// optimization for non-debuggable (release) builds without bytecode
    /// obfuscation (`abstract-compile-node.js::declareInputs`,
    /// `module_mode.js` es2abc command construction).
    #[serde(default)]
    pub branch_elimination: Option<bool>,
    /// `buildOption.arkOptions.transformLib` — AOP transform lib path passed
    /// to es2abc as `--transform-lib` when non-empty.
    #[serde(default)]
    pub transform_lib: Option<std::path::PathBuf>,
    /// `buildOption.arkOptions.skipOhModulesLint` — passed to the checker as
    /// `skipOhModulesLint` (skip ArkTS lint for `oh_modules`).
    #[serde(default)]
    pub skip_oh_modules_lint: Option<bool>,
    /// `buildOption.arkOptions.reExportCheckMode` — `noCheck` | `compatible`
    /// | `strict` re-export checking mode for the checker.
    #[serde(default)]
    pub re_export_check_mode: Option<String>,
    /// `buildOption.arkOptions.autoLazyImport` — automatic lazy-import
    /// transform (SDK `process_lazy_import.js::transformLazyImport`). Defaults
    /// to the `ohos.defaults.autoLazyImport` property when absent.
    #[serde(default)]
    pub auto_lazy_import: Option<bool>,
    /// `buildOption.arkOptions.autoLazyFilter` — include/exclude package
    /// filters for the automatic lazy-import transform.
    #[serde(default)]
    pub auto_lazy_filter: Option<AutoLazyFilter>,
    /// `buildOption.arkOptions.expandImportPath` — expand package-level
    /// imports into direct source-file imports (SDK `import_path_expand.js`).
    /// Only effective for non-HAR modules.
    #[serde(default)]
    pub expand_import_path: Option<ExpandImportPath>,
    /// TypeScript emission settings from `arkOptions.tscConfig`.
    ///
    /// The installed ets-loader accepts only `ES2017` and `ES2021`; absent or
    /// unsupported values fall back to `ES2021` in
    /// `ets_checker.js::getTargetESVersion`.
    #[serde(default)]
    pub tsc_config: ArkTscConfig,
}

/// Widget card configuration from `buildOption.arkOptions.widget`.
///
/// Mirrors the installed hvigor 6.24.3 schema
/// `ohos-module-build-profile-schema.json#/properties/arkOptions/properties/widget`:
/// only `transitiveDeps` exists (default `false`).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WidgetArkOptions {
    /// Collect transitively blocked dependency compile files for widget
    /// compilation, grouped per topmost ByteCode HAR package.
    #[serde(default)]
    pub transitive_deps: bool,
}

/// `buildOption.arkOptions.autoLazyFilter` — include/exclude lists for the
/// automatic lazy-import transform. Hvigor joins each list with `,` when
/// passing them to the loader.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoLazyFilter {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// `buildOption.arkOptions.expandImportPath` — `enable` + `exclude` for the
/// import path expansion transform (SDK `import_path_expand.js`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpandImportPath {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// TypeScript emission configuration owned by `arkOptions.tscConfig`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArkTscConfig {
    #[serde(default)]
    pub target_es_version: ArkTscTarget,
}

/// The two JavaScript targets accepted by the installed hvigor ets checker.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArkTscTarget {
    Es2017,
    #[default]
    Es2021,
}

impl ArkTscTarget {
    /// Apply `ets_checker.js::getTargetESVersion`'s accepted-value contract.
    pub fn from_hvigor_option(value: Option<&str>) -> Self {
        match value {
            Some("ES2017") => Self::Es2017,
            Some("ES2021") | None | Some(_) => Self::Es2021,
        }
    }

    /// Return the target spelling consumed by OXC's structured transformer.
    pub fn es_target(self) -> &'static str {
        match self {
            Self::Es2017 => "es2017",
            Self::Es2021 => "es2021",
        }
    }
}

#[cfg(test)]
mod ark_tsc_target_tests {
    use super::ArkTscTarget;

    #[test]
    fn matches_latest_ets_checker_target_contract() {
        assert_eq!(
            ArkTscTarget::from_hvigor_option(Some("ES2017")),
            ArkTscTarget::Es2017
        );
        assert_eq!(
            ArkTscTarget::from_hvigor_option(Some("ES2021")),
            ArkTscTarget::Es2021
        );
        assert_eq!(
            ArkTscTarget::from_hvigor_option(Some("ES2022")),
            ArkTscTarget::Es2021
        );
        assert_eq!(ArkTscTarget::default(), ArkTscTarget::Es2021);
    }
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
    /// Resolved, absolute paths to every consumer rule file listed in
    /// `consumerRules` or `consumerFiles`.  Empty when no consumer rules are
    /// configured.
    #[serde(default)]
    pub consumer_rule_files: Vec<PathBuf>,
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
    #[serde(default)]
    pub compression: Option<serde_json::Value>,
    #[serde(default)]
    pub copy_code_resource: CopyCodeResource,
    #[serde(default)]
    pub res_compile_threads: Option<usize>,
    #[serde(default)]
    pub ignore_resource_pattern: Vec<String>,
    #[serde(default)]
    pub qualifiers_config: Option<serde_json::Value>,
    /// Remote HAR dependency names whose resources are excluded from the
    /// bundled-resource map (`har-target-util.js::isNeedExclude`).
    #[serde(default)]
    pub exclude_har_res: Vec<String>,
    /// `resOptions.idDefinedFilePath`; relative paths are project-root based.
    #[serde(default)]
    pub id_defined_file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyCodeResource {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub includes: Option<Vec<String>>,
}

impl Default for CopyCodeResource {
    fn default() -> Self {
        Self {
            enable: true,
            excludes: Vec::new(),
            includes: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_true_enable_source_code_check() -> bool {
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
    /// Packages excluded from remote dynamic-import resource collection.
    #[serde(default)]
    pub exclude_packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    /// Path to `ets-loader` directory for SDK metadata and parity inputs.
    pub ets_loader_path: PathBuf,
    /// Path to `es2abc` binary for the SDK ABC backend.
    pub es2abc_path: PathBuf,
    /// Path to `restool` binary for the SDK resource backend.
    pub restool_path: PathBuf,
    /// Target API version passed to compilers and SDK tools.
    pub compile_api_version: u32,
    /// API version of the selected SDK component.
    ///
    /// This is intentionally distinct from `compile_api_version`: current
    /// hvigor gates SDK-provided artifacts such as `ResourceTable.ts` on the
    /// installed component version even when a project targets an older API.
    #[serde(default = "default_sdk_component_api_version")]
    pub sdk_component_api_version: u32,
    /// Whether the SDK has the rollup plugin (required for stage mode).
    pub has_rollup_plugin: bool,
    /// DevEco Studio SDK version string discovered from SDK metadata.
    /// Written into `module.json` as `app.compileSdkVersion`.
    #[serde(default = "default_compile_sdk_version")]
    pub compile_sdk_version: String,
}

/// Installed ArkTS 1.2 SDK tool and declaration layout.
///
/// The current source tree packages the static frontend under
/// `ets/static/`, independently from the legacy `ets/build-tools/ets-loader`
/// directory. Keeping the derived layout typed prevents compiler stages from
/// inventing paths or passing dynamic-frontend tools to a static build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticArktsSdkLayout {
    pub root: PathBuf,
    pub driver_entry: PathBuf,
    pub panda_sdk: PathBuf,
    pub es2panda: PathBuf,
    pub abc_linker: PathBuf,
    pub dependency_analyzer: PathBuf,
    pub ui_syntax_plugin: PathBuf,
    pub ui_plugin: PathBuf,
    pub memo_plugin: PathBuf,
    pub api: PathBuf,
    pub kits: PathBuf,
    pub arkts: PathBuf,
}

impl StaticArktsSdkLayout {
    fn executable_name(stem: &str) -> String {
        if cfg!(windows) {
            format!("{stem}.exe")
        } else {
            stem.to_string()
        }
    }
}

fn default_compile_sdk_version() -> String {
    "6.0.0.47".to_string()
}

fn default_sdk_component_api_version() -> u32 {
    12
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

    /// Derive the ArkTS 1.2 static SDK layout selected by the explicit
    /// `--arkts-version 1.2` build mode.
    pub fn static_arkts_layout(&self) -> Option<StaticArktsSdkLayout> {
        let root = self.ets_root()?.join("static");
        let build_tools = root.join("build-tools");
        let panda_sdk = build_tools.join("ets2panda");
        let panda_bin = panda_sdk.join("bin");
        let ui_plugins = build_tools.join("ui-plugins").join("lib");
        Some(StaticArktsSdkLayout {
            root: root.clone(),
            driver_entry: build_tools
                .join("driver")
                .join("build-system")
                .join("dist")
                .join("entry.js"),
            panda_sdk,
            es2panda: panda_bin.join(StaticArktsSdkLayout::executable_name("es2panda")),
            abc_linker: panda_bin.join(StaticArktsSdkLayout::executable_name("ark_link")),
            dependency_analyzer: panda_bin
                .join(StaticArktsSdkLayout::executable_name("dependency_analyzer")),
            ui_syntax_plugin: ui_plugins.join("ui-syntax-plugins").join("index"),
            ui_plugin: ui_plugins.join("ui-plugins").join("index"),
            memo_plugin: ui_plugins.join("memo-plugins").join("index"),
            api: root.join("api"),
            kits: root.join("kits"),
            arkts: root.join("arkts"),
        })
    }

    /// Return the SDK PAC schema path when this SDK provides one.
    ///
    /// hvigor skips PAC/privacy processing when `toolchains/paccheck/pacSchema.json`
    /// is absent, so callers should treat `None` as "task should not run".
    pub fn pac_schema_path(&self) -> Option<std::path::PathBuf> {
        let path = self
            .component_root()?
            .join("toolchains")
            .join("paccheck")
            .join("pacSchema.json");
        path.is_file().then_some(path)
    }

    /// Return the PAC schema selected by hvigor for the product runtime OS.
    ///
    /// HarmonyOS projects use `HosSdkInfo`, whose PAC schema belongs to the
    /// sibling `hms` component even though ArkTS compilation still uses the
    /// OpenHarmony ETS loader. OpenHarmony products continue to use the
    /// selected component's toolchains schema.
    pub fn pac_schema_path_for_runtime(&self, runtime_os: &str) -> Option<std::path::PathBuf> {
        if is_harmony_os(runtime_os) {
            let path = self
                .component_root()?
                .parent()?
                .join("hms")
                .join("toolchains")
                .join("paccheck")
                .join("pacSchema.json");
            if path.is_file() {
                return Some(path);
            }
        }
        self.pac_schema_path()
    }

    /// Return the modulecheck schema selected by hvigor for the runtime OS.
    ///
    /// `HosSdkInfo` takes `app.json`, `module.json`, and `systemTheme.json`
    /// from the sibling HMS toolchains component. Other modulecheck schemas
    /// remain owned by the selected OpenHarmony component. OpenHarmony builds
    /// always use the selected component directly.
    pub fn modulecheck_schema_path_for_runtime(
        &self,
        runtime_os: &str,
        file_name: &str,
    ) -> Option<std::path::PathBuf> {
        let component_root = self.component_root()?;
        let schema_component = if is_harmony_os(runtime_os)
            && matches!(file_name, "app.json" | "module.json" | "systemTheme.json")
        {
            component_root.parent()?.join("hms")
        } else {
            component_root.to_path_buf()
        };
        let path = schema_component
            .join("toolchains")
            .join("modulecheck")
            .join(file_name);
        path.is_file().then_some(path)
    }

    /// SDK permission oracle used by hvigor for requestPermissions checks.
    ///
    /// The previewer component stores SDK `definePermissions` in
    /// `<sdk>/previewer/common/resources/module.json`.
    pub fn permission_module_json_path(&self) -> Option<std::path::PathBuf> {
        let path = self
            .component_root()?
            .join("previewer")
            .join("common")
            .join("resources")
            .join("module.json");
        path.is_file().then_some(path)
    }

    /// Previewer component selected by OHOS/HMOS SDK manager for local tests.
    ///
    /// The selected ETS component and Previewer are siblings under the same
    /// runtime component root (`default/openharmony` for OpenHarmony and HMOS
    /// compilation). Returning the component root matches
    /// `GenerateUnitTestResult.getPreviewerPath()`.
    pub fn previewer_component_path(&self) -> Option<std::path::PathBuf> {
        let path = self.component_root()?.join("previewer");
        path.join("common").join("bin").is_dir().then_some(path)
    }

    /// HMS Previewer sibling required by the HarmonyOS Previewer `-hsp`
    /// argument. OpenHarmony callers intentionally do not request it.
    pub fn hms_previewer_component_path(&self) -> Option<std::path::PathBuf> {
        let path = self
            .component_root()?
            .parent()?
            .join("hms")
            .join("previewer");
        path.join("uni-package.json").is_file().then_some(path)
    }

    /// HMS ArkTS declaration component selected for HarmonyOS compilation.
    ///
    /// `HosSdkInfo.getHmsArkDir()` returns the base directory of the sibling
    /// HMS ETS component. The component is discovered relative to the selected
    /// OpenHarmony ETS loader, matching SDK-manager layouts on every host
    /// platform without relying on an IDE installation path.
    pub fn hms_ets_component_path(&self) -> Option<std::path::PathBuf> {
        let path = self.component_root()?.parent()?.join("hms").join("ets");
        path.join("uni-package.json").is_file().then_some(path)
    }

    /// Locate the `@ohos/coverage` implementation owned by the installed
    /// hvigor OHOS plugin. DevEco and the standalone command-line tools place
    /// hvigor at different distribution-relative roots; both candidates are
    /// derived from the selected SDK component, never from a host-specific
    /// absolute installation path.
    pub fn hvigor_coverage_package_path(&self) -> Option<std::path::PathBuf> {
        let sdk_root = self.component_root()?.parent()?.parent()?;
        let distribution_root = sdk_root.parent()?;
        [
            distribution_root.join("hvigor"),
            distribution_root.join("tools").join("hvigor"),
        ]
        .into_iter()
        .map(|hvigor_root| {
            hvigor_root
                .join("hvigor-ohos-plugin")
                .join("node_modules")
                .join("@ohos")
                .join("coverage")
        })
        .find(|path| {
            path.join("lib")
                .join("src")
                .join("commandLine")
                .join("index.js")
                .is_file()
        })
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
            dirs.push(sdk_root.join("js").join("api"));
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

    /// Candidate directories containing `@arkts.*` declaration files.
    ///
    /// Returns `["<sdk>/ets/arkts"]`.
    pub fn ets_arkts_dirs(&self) -> Vec<std::path::PathBuf> {
        self.ets_root()
            .map(|ets_root| vec![ets_root.join("arkts")])
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Name of the selected `app.signingConfigs` entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
