//! Typed bundle plan for the rolldown bundler stage.
//!
//! [`BundlePlan`] is the single typed input to [`Bundler::bundle`].  It is
//! constructed by the CLI's `BuildPlanBuilder` from session data (loader.json,
//! resource outputs, compile environment) and passed to the bundler.
//!
//! The plan covers the full `arkts-pack.js::generateConfig` observable fields:
//! - Compile entry inputs (from `loader.json`)
//! - Output options (`getOutputConfig`)
//! - Compile environment (`getCompileEnv`)
//! - Manual chunks (`manualChunks`)
//! - Watch configuration (`getWatchConfig`)
//! - Cache policy (`shouldInvalidCache` / rollup cache)
//! - Source map policy
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BundlePlan` | `arkts-pack.js::generateConfig` combined input |
//! | `BundleInput` | `getCompileInputObject` entry set |
//! | `BundleOutputOptions` | `getOutputConfig` output configuration |
//! | `BundleCachePolicy` | `rollup-cache-service.js` cache configuration |
//! | `SourceMapPolicy` | `getOutputConfig` sourcemaps configuration |
//! | `LoaderResourceJoin` | `ArkCompile` joining GenerateLoaderJson + CompileResource |

use std::path::PathBuf;

use arkdown_core::{ArkdownError, BuildMode, OutputTarget};
use arkdown_ets_transform::ModuleAnalyzerKind;
use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// BundleInput
// ---------------------------------------------------------------------------

/// Compile entry inputs for the rolldown bundle.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getCompileInputObject` — the set of source
/// file paths that are the entry points for the Rolldown bundle.
#[derive(Debug, Clone)]
pub struct BundleInput {
    /// Absolute path to the `loader.json` file.
    ///
    /// Retained for diagnostics and artifact provenance.  The bundler receives
    /// parsed entries through [`BundleInput::entries`] and does not parse
    /// `loader.json` itself.
    pub loader_json_path: PathBuf,
    /// Absolute project root used as rolldown's `cwd`.
    pub project_root: PathBuf,
    /// Module root directory (used to resolve relative entry paths).
    pub module_root: PathBuf,
    /// OHOS module name (e.g., `"entry"`, `"feature"`).
    pub module_name: String,
    /// Map of module/dependency name → absolute module root path from
    /// `loader.json#modulePathMap`.
    pub module_path_map: Vec<(String, PathBuf)>,
    /// Map of package/module name → absolute root path used to reproduce
    /// hvigor `ModuleMode` source record ownership.
    ///
    /// This is intentionally separate from `loader.json#modulePathMap`: hvigor
    /// keeps `modulePathMap` narrow, while `ModuleSourceFile.processModuleInfos`
    /// uses `rollup.getModuleInfo(id).meta` for every compiled source file.
    pub module_record_roots: Vec<(String, PathBuf)>,
    /// Ordered compile entries produced by `GenerateLoaderJson`.
    pub entries: Vec<String>,
    /// Runtime entries loaded directly by OHOS from `module.json`.
    pub runtime_entry_sources: Vec<String>,
    /// Whether hvigor `supportChunks` is enabled for this module.
    pub support_chunks: bool,
    /// Package metadata used by hvigor-format source map keys and
    /// `entry-package-info` / `package-info` fields.
    pub source_map_packages: BundleSourceMapPackageInfo,
    /// Package metadata used by hvigor runtime normalized OHM URLs.
    pub runtime_packages: BundleRuntimePackageInfo,
    /// ByteCode HAR package aliases from `loader.json#byteCodeHarInfo`.
    pub byte_code_har_packages: Vec<String>,
}

/// Package metadata required by hvigor's `SourceMapGenerator`.
///
/// # Upstream correspondence
///
/// Mirrors `projectConfig.entryPackageName`, `projectConfig.entryModuleVersion`,
/// and `rollup.getModuleInfo(id).meta.dependencyPkgInfo` consumed by
/// `generate_sourcemap.js::fillSourceMapPackageInfo`.
#[derive(Debug, Clone, Default)]
pub struct BundleSourceMapPackageInfo {
    /// Current entry package name.
    pub entry_package_name: String,
    /// Current entry package version.
    pub entry_package_version: String,
    /// Dependency package versions keyed by package name.
    pub dependency_package_versions: FxHashMap<String, String>,
}

/// Package metadata required by hvigor's `pkgContextInfo`-backed normalized
/// OHM URL generation.
#[derive(Debug, Clone, Default)]
pub struct BundleRuntimePackageInfo {
    /// Package contexts keyed by package name and dependency aliases.
    pub packages: FxHashMap<String, BundleRuntimePackageContext>,
}

#[derive(Debug, Clone, Default)]
pub struct BundleRuntimePackageContext {
    /// `pkgContextInfo[package].packageName`.
    pub package_name: String,
    /// `pkgContextInfo[package].bundleName`.
    pub bundle_name: String,
    /// `pkgContextInfo[package].moduleName`.
    pub module_name: String,
    /// `pkgContextInfo[package].version`.
    pub version: String,
    /// `pkgContextInfo[package].entryPath`.
    pub entry_path: String,
    /// `pkgContextInfo[package].isSO`.
    pub is_so: bool,
}

// ---------------------------------------------------------------------------
// BundleOutputOptions
// ---------------------------------------------------------------------------

/// Output configuration for the Rolldown bundle.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getOutputConfig`, which configures the
/// rollup/rolldown `output` option including format, directory, and source
/// maps.
#[derive(Debug, Clone)]
pub struct BundleOutputOptions {
    /// Output directory for JS chunks.
    pub output_dir: PathBuf,
    /// Output directory for hvigor ModuleMode source cache files.
    ///
    /// Corresponds to `projectConfig.cachePath`, for example
    /// `build/default/cache/default/default@CompileArkTS/esmodule/release`.
    pub module_source_output_dir: Option<PathBuf>,
    /// Source map output directory (for rolldown `sourcemap: "file"`).
    pub source_map_dir: Option<PathBuf>,
    /// The output entry file name pattern.
    pub entry_file_names: Option<String>,
    /// The output chunk file name pattern.
    pub chunk_file_names: Option<String>,
    /// Whether to preserve symbol links.
    ///
    /// Corresponds to `generateConfig.preserveSymlinks`.
    pub preserve_symlinks: bool,
    /// Whether to use a shim for missing exports.
    ///
    /// Corresponds to `generateConfig.shimMissingExports`.
    pub shim_missing_exports: bool,
}

impl Default for BundleOutputOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            module_source_output_dir: None,
            source_map_dir: None,
            entry_file_names: None,
            chunk_file_names: None,
            preserve_symlinks: false,
            shim_missing_exports: false,
        }
    }
}

// ---------------------------------------------------------------------------
// BundleCachePolicy
// ---------------------------------------------------------------------------

/// Cache policy for the rolldown bundle.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-cache-service.js` cache configuration.
/// The `cache_directory` is the hvigor `build/default/cache/bundle/` directory.
#[derive(Debug, Clone)]
pub struct BundleCachePolicy {
    /// Cache root directory (e.g., `build/default/cache/bundle/`).
    pub cache_directory: Option<PathBuf>,
    /// Whether rollup cache reading is enabled for this build.
    pub read_cache: bool,
    /// Whether rollup cache writing is enabled for this build.
    pub write_cache: bool,
    /// Compile environment key string for cache invalidation.
    ///
    /// Corresponds to `generateConfig.compileEnv` — when this changes, the
    /// entire bundle cache is invalidated.
    pub compile_env_key: String,
}

impl Default for BundleCachePolicy {
    fn default() -> Self {
        Self {
            cache_directory: None,
            read_cache: false,
            write_cache: false,
            compile_env_key: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// SourceMapPolicy
// ---------------------------------------------------------------------------

/// Source map generation policy.
///
/// # Upstream correspondence
///
/// Corresponds to `getOutputConfig.sourcemap` configuration and
/// `sourceMaps.map` output path in hvigor release builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMapPolicy {
    /// No source maps generated.
    None,
    /// Inline source maps embedded in each JS chunk.
    Inline,
    /// External source map files (`sourceMaps.map`).
    External,
}

impl Default for SourceMapPolicy {
    fn default() -> Self {
        Self::None
    }
}

// ---------------------------------------------------------------------------
// LoaderResourceJoin
// ---------------------------------------------------------------------------

/// The join of `GenerateLoaderJson` and `CompileResource` outputs.
///
/// The bundler receives both the loader.json compile entries and the compiled
/// resource index path.  This type captures both so the `BundlePlan` can be
/// the single input to the bundle stage.
///
/// # Upstream correspondence
///
/// Corresponds to the `ArkCompile` task's dependency on both
/// `GenerateLoaderJson` and `CompileResource` in the hvigor task graph.
#[derive(Debug, Clone)]
pub struct LoaderResourceJoin {
    /// Absolute path to the `loader.json` manifest.
    pub loader_json_path: PathBuf,
    /// Absolute path to the compiled `resources.index`.
    pub resources_index_path: Option<PathBuf>,
    /// Absolute path to the `ResourceTable.txt` for linking.
    pub resource_table_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// BundlePluginSet
// ---------------------------------------------------------------------------

/// The set of rolldown plugin names to activate for this bundle.
///
/// `BundlePluginSet` records which built-in plugins are included in this build
/// so that the `LifecycleParityReport` can verify all required hooks are covered.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getBuiltInPlugins` output.
#[derive(Debug, Clone, Default)]
pub struct BundlePluginSet {
    /// List of plugin names in activation order.
    pub names: Vec<String>,
}

impl BundlePluginSet {
    /// Create from a list of plugin name strings.
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            names: names.into_iter().map(|n| n.into()).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// BundlePlan
// ---------------------------------------------------------------------------

/// The complete typed plan for the rolldown bundle stage.
///
/// `BundlePlan` is the single input to [`crate::bundler::Bundler::bundle_with_plan`].
/// It is constructed by the CLI's `BuildPlanBuilder` from session data and
/// covers all fields observable in `arkts-pack.js::generateConfig`.
///
/// # Upstream correspondence
///
/// Replaces the ad-hoc `ArkdownConfig` + `LoaderJson` pair in `Bundler::bundle`
/// per SPEC-048.
#[derive(Debug, Clone)]
pub struct BundlePlan {
    /// Compile entry inputs.
    pub input: BundleInput,
    /// Output configuration.
    pub output: BundleOutputOptions,
    /// Source map policy.
    pub source_map: SourceMapPolicy,
    /// Cache policy.
    pub cache: BundleCachePolicy,
    /// Resource join (loader.json + compiled resource paths).
    pub loader_resource: LoaderResourceJoin,
    /// Plugin name set (for parity tracking).
    pub plugins: BundlePluginSet,
    /// Build mode (affects source maps, optimization, tree-shaking).
    pub build_mode: BuildMode,
    /// Target API version (from `compatibleSdkVersion`).
    pub target_api_version: u32,
    /// Bundle name (from `AppScope/app.json5`).
    pub bundle_name: String,
    /// Whether `useNormalizedOHMUrl` is active.
    pub use_normalized_ohm_url: bool,
    /// Whether SDK `etsTransform` should emit JavaScript for ETS/TS inputs.
    ///
    /// Corresponds to hvigor `projectConfig.shouldEmitJs`, which is derived in
    /// `AbstractCompileNode.initCommonArkConfig()`.
    pub should_emit_js: bool,
    /// Module analyzer used for hvigor type/value split parity.
    pub module_analyzer: ModuleAnalyzerKind,
    /// Output target kind (HAP / HSP / HAR).
    ///
    /// Used by the plugin factory to decide whether to activate
    /// `HarDeduplicatePlugin` (HSP builds only).
    pub output_target: OutputTarget,
    /// Override `shimMissingExports` from `arkdown.config.json`.
    ///
    /// When `Some`, overrides the bundler default (`true`).
    pub shim_missing_exports_override: Option<bool>,
    /// Override tree-shaking from `arkdown.config.json`.
    ///
    /// When `Some`, overrides the build-mode-derived tree-shake setting.
    pub treeshake_override: Option<bool>,
    /// Override the rolldown `context` string from `arkdown.config.json`.
    ///
    /// When `Some`, overrides the default `"globalThis"`.
    pub context_override: Option<String>,
}

impl BundlePlan {
    /// Validate the plan.
    ///
    /// Returns an error if required fields are missing or inconsistent.
    pub fn validate(&self) -> Result<(), ArkdownError> {
        if !self.input.loader_json_path.exists() {
            return Err(ArkdownError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "loader.json not found: {}",
                    self.input.loader_json_path.display()
                ),
            )));
        }
        Ok(())
    }

    /// Return the entry input configuration.
    pub fn entry_inputs(&self) -> &BundleInput {
        &self.input
    }

    /// Return the output options.
    pub fn output_options(&self) -> &BundleOutputOptions {
        &self.output
    }

    /// Return the active plugin names.
    pub fn plugins(&self) -> &BundlePluginSet {
        &self.plugins
    }

    /// Return the loader.json path.
    pub fn loader_json(&self) -> &std::path::Path {
        &self.loader_resource.loader_json_path
    }

    /// Return the resource outputs.
    pub fn resource_outputs(&self) -> &LoaderResourceJoin {
        &self.loader_resource
    }

    /// Return `true` if this is a debug build.
    pub fn is_debug(&self) -> bool {
        matches!(self.build_mode, BuildMode::Debug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plan(loader_json_path: PathBuf) -> BundlePlan {
        BundlePlan {
            input: BundleInput {
                loader_json_path: loader_json_path.clone(),
                project_root: PathBuf::from("/project"),
                module_root: PathBuf::from("/project/entry"),
                module_name: "entry".to_string(),
                module_path_map: Vec::new(),
                module_record_roots: Vec::new(),
                entries: vec!["./ets/pages/Index.ets".to_string()],
                runtime_entry_sources: Vec::new(),
                support_chunks: false,
                source_map_packages: BundleSourceMapPackageInfo {
                    entry_package_name: "entry".to_string(),
                    entry_package_version: "1.0.0".to_string(),
                    dependency_package_versions: FxHashMap::default(),
                },
                runtime_packages: BundleRuntimePackageInfo::default(),
                byte_code_har_packages: Vec::new(),
            },
            output: BundleOutputOptions::default(),
            source_map: SourceMapPolicy::None,
            cache: BundleCachePolicy::default(),
            loader_resource: LoaderResourceJoin {
                loader_json_path,
                resources_index_path: None,
                resource_table_path: None,
            },
            plugins: BundlePluginSet::default(),
            build_mode: BuildMode::Debug,
            target_api_version: 12,
            bundle_name: "com.example.app".to_string(),
            use_normalized_ohm_url: true,
            should_emit_js: true,
            module_analyzer: ModuleAnalyzerKind::default(),
            output_target: OutputTarget::Hap,
            shim_missing_exports_override: None,
            treeshake_override: None,
            context_override: None,
        }
    }

    #[test]
    fn validate_fails_for_missing_loader_json() {
        let plan = make_plan(PathBuf::from("/nonexistent/loader.json"));
        assert!(plan.validate().is_err());
    }
}
