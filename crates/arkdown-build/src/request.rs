//! Typed build requests accepted from the JavaScript/N-API boundary.

use std::path::PathBuf;

use anyhow::Result;
use arkdown_core::ArktsLanguageVersion;
use arkdown_watcher::WatchInvalidationBatch;

/// Properties read directly by hvigor-ohos-plugin from the global Hvigor
/// parameter map. They cross the JS/native boundary as one typed value so a
/// supported property cannot be silently dropped by native orchestration.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HvigorRuntimeProperties {
    pub daemon: bool,
    pub align_target: Option<String>,
    pub source_map_dir: Option<PathBuf>,
    pub har_exclude_hsp_dependencies: bool,
    pub dependencies_types_enable: bool,
    pub single_file_emit: bool,
    pub rollup_cache_use_source_hash: bool,
    pub process_lib_optimization: bool,
    pub compile_lib_entry_file: Option<bool>,
    pub release_cmake_build_type: Option<String>,
    pub fallback_targets: Vec<String>,
    pub obfuscation_rules_optimization: bool,
    pub sign_har: bool,
    pub with_deduplicate: Option<bool>,
    pub previewer_replace_page: Option<String>,
    pub previewer_compile_res_incremental: bool,
    /// Exact string comparison used by `PreviewerArkCompile.getWorkerType`.
    /// Only the literal extra parameter `compileHspDependencies=true` enables
    /// normal-worker dependency compilation.
    pub compile_hsp_dependencies: bool,
    /// JavaScript truthiness used independently by
    /// `ReplacePreviewerPage.processPreviewProfile`. Hvigor deliberately treats
    /// a non-empty value such as `"false"` as truthy at this boundary.
    pub compile_hsp_dependencies_truthy: bool,
    pub main_module: Option<String>,
    pub page_type: Option<String>,
    /// Exact extra-config predicates consumed by AbstractMergeProfile and
    /// BuildNativeWithCmake.
    pub asan_config: Option<bool>,
    pub tsan_config: Option<bool>,
    pub ubsan_config: Option<bool>,
    /// Truthy `ohos-enable-hwasan` extra-config override interpreted with
    /// hvigor's exact string comparison (`value === "true"`). `None` means the
    /// option was absent or empty and BinXO validation must fall back to CMake
    /// arguments.
    pub hwasan_config: Option<bool>,
    /// Truthy `ohos-enable-binxo` extra-config override interpreted with
    /// hvigor's exact string comparison. `None` preserves the CMake fallback.
    pub binxo_config: Option<bool>,
}

impl HvigorRuntimeProperties {
    pub fn source_map_output(
        &self,
        project_root: &std::path::Path,
        module_name: &str,
    ) -> Option<PathBuf> {
        let configured = self.source_map_dir.as_ref()?;
        let root = if configured.is_absolute() {
            configured.clone()
        } else {
            project_root.join("hvigor").join(configured)
        };
        Some(arkdown_path::lexical_normalize(
            &root.join(module_name).join("sourceMaps.map"),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbcBackendChoice {
    Auto,
    #[default]
    Sdk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OhpmModeChoice {
    Offline,
    PreferOffline,
    #[default]
    Online,
}

/// Hvigor analyze mode serialized as `projectConfig.perf` for compile plugins.
/// Numeric values are defined by ets-loader's `AnalyzeMode` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalyzeModeChoice {
    #[default]
    Normal,
    Verbose,
    False,
    Trace,
}

impl AnalyzeModeChoice {
    pub fn perf_value(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Verbose => 1,
            Self::False => 2,
            Self::Trace => 3,
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "normal" | "default" => Some(Self::Normal),
            "verbose" => Some(Self::Verbose),
            "false" => Some(Self::False),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }
}

impl From<OhpmModeChoice> for arkdown_session::package::acquisition::AcquisitionMode {
    fn from(value: OhpmModeChoice) -> Self {
        match value {
            OhpmModeChoice::Offline => Self::Offline,
            OhpmModeChoice::PreferOffline => Self::PreferOffline,
            OhpmModeChoice::Online => Self::Online,
        }
    }
}

/// One selected native hvigor command root.
///
/// The Node task graph serializes only explicit partial-task roots here. The
/// coordinator completes their dependency closure and stops after every root
/// boundary has completed, matching hvigor's selected DAG execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTaskRoot {
    pub module_name: String,
    pub target_name: Option<String>,
    pub task_name: String,
}

#[derive(Clone, Default)]
pub struct BuildRequest {
    /// Build target: hap, hsp, har, app, preview, test
    ///
    /// `test` mirrors hvigor's `hvigorw test` task: local unit-test build root
    /// (`.test/`), GenerateUnitTestTemplate, TestAbility injection, and test
    /// compilation (no HAP packaging). Hvigor injects `buildRoot=.test`,
    /// `isLocalTest=true`, `unitTestMode=true`, `ohos-test-coverage=true` and
    /// `unit.test.replace.page` defaults for the `test` task; arkdown derives
    /// the same behavior from the target alone.
    pub target: String,
    /// Scope filter for local unit tests (`-s class` parameter in the generated
    /// `Index.ets`; hvigor extra config `scope`). Optional — hvigor leaves the
    /// template parameter empty when unset.
    pub scope: Option<String>,
    /// Code-coverage instrumentation for local unit tests (hvigor extra config
    /// `ohos-test-coverage`). Defaults to `true` for `--target test`, matching
    /// hvigor's `setTestDefaultParameters` injection.
    pub ohos_test_coverage: Option<bool>,
    /// Optional source-path filter forwarded to hvigor's
    /// `testFrameworkPar.coveragePathFilter`.
    pub coverage_path_filter: Option<String>,
    /// Hvigor coverage selection mode: `black`, `white`, or `full`.
    pub coverage_mode: Option<String>,
    /// Replacement page used by `ReplaceUnitTestIndexFile` and Previewer.
    pub unit_test_replace_page: Option<String>,
    /// Fast build app: hvigor `packOptions.fastBuildApp` — skip the normal HAP
    /// packaging and copy the compiled intermediates into
    /// `intermediates/fastApp/<target>/` for IDE fast deployment
    /// (hvigor `FastPackageHap`).
    pub fast_build_app: bool,
    /// Hvigor `enableSignTask`; false keeps signing tasks in the graph but makes
    /// their actions no-op and leaves unsigned package outputs selected.
    pub enable_sign_task: bool,
    /// Build mode: debug, release, test
    pub mode: String,
    /// Hook performance analysis level. Only `verbose` and `trace` enable
    /// Hvigor's compile-event factory.
    pub analyze: AnalyzeModeChoice,
    /// ArkTS compiler language. ArkTS 1.2 is opt-in; omitted builds retain the
    /// existing ArkTS 1.1 parser and transform pipeline.
    pub arkts_version: ArktsLanguageVersion,
    /// OHPM dependency acquisition policy used before the build session freezes
    /// package roots. Online matches the normal hvigor wrapper invocation.
    pub ohpm_mode: OhpmModeChoice,
    /// Scope to a specific module
    pub module: Option<String>,
    /// Product name
    pub product: Option<String>,
    /// Project root (default: current directory)
    pub project: Option<PathBuf>,
    /// Named target configuration to use (from build-profile.json5 targets[].name).
    ///
    /// When specified, overrides the automatic target selection (buildModeSet + product
    /// matching) and forces the use of the named target configuration.
    /// This implements the multi-target product execution model: different targets can
    /// be built separately to produce multiple artifacts from the same source.
    ///
    /// The JavaScript CLI's `--build-target-name phone` option selects the
    /// target named `phone`.
    pub build_target_name: Option<String>,
    /// Override the application build version for this invocation.
    ///
    /// Mirrors hvigor extra config `buildVersion` and takes precedence over the
    /// selected product and `AppScope/app.json5`.
    pub build_version: Option<String>,
    /// Watch mode: rebuild on source file changes.
    pub watch: bool,
    /// Coverage instrumentation mode: inject the Hvigor-compatible BJC coverage runtime
    /// and counters used by `collectCoverage` (F20).
    pub coverage: bool,
    /// Enable AddressSanitizer for the generated package.
    ///
    /// Mirrors hvigor extra config `ohos-debug-asan=true`.
    pub ohos_debug_asan: bool,
    /// Enable HWAddressSanitizer for the generated package.
    ///
    /// Mirrors hvigor extra config `ohos-enable-hwasan=true`.
    pub ohos_enable_hwasan: bool,
    /// Enable ThreadSanitizer (`ohos-enable-tsan=true`).
    pub ohos_enable_tsan: bool,
    /// Enable UndefinedBehaviorSanitizer (`ohos-enable-ubsan=true`).
    pub ohos_enable_ubsan: bool,
    /// Enable the HMS BinXO native post-processor.
    ///
    /// Mirrors hvigor extra config `ohos-enable-binxo=true`. BinXO also
    /// requires HWASan; requesting BinXO without HWASan reaches
    /// `DoNativeBinxo` and fails with the same validation boundary as hvigor.
    pub ohos_enable_binxo: bool,
    /// Compile native C/C++ sources through `zig cc` and `zig c++` while
    /// retaining the selected OHOS SDK sysroot, ABI, CMake, and Ninja setup.
    ///
    /// Zig must be available on `PATH`. Disabled by default so the native
    /// command remains identical to hvigor unless explicitly requested.
    pub enable_zig: bool,
    /// Preview mode: watch resource directories and trigger resource recompilation (M3).
    /// Implies --watch; focuses change detection on src/main/resources/ for faster
    /// resource-only rebuilds during DevEco IDE preview sessions.
    pub preview: bool,
    /// Require the previewed page to use exactly one `@Entry`, matching the
    /// source compiler's `projectConfig.checkEntry` override.
    pub check_entry: bool,
    /// Internal Preview transform state retained by incremental rebuilds without
    /// recursively starting another watcher.
    #[doc(hidden)]
    pub preview_transform_mode: bool,
    /// Disable the ABC compile cache (parity mode).
    ///
    /// When set, the ABC compilation step skips the coarse-grained `TaskInputs`
    /// cache hit check and always recompiles from source.  This is the equivalent
    /// of hvigor's `--no-cache` flag for the ABC compilation stage.
    ///
    /// Use this flag when comparing arkdown ABC output with hvigor's output to
    /// ensure both tools run the full compilation pipeline rather than returning
    /// a cached artifact.
    pub disable_abc_cache: bool,
    /// Override the ABC ProgramCache `.protoBin` file path (upstream `es2abc --cache-file`).
    ///
    /// When specified, this path is used as the `--cache-file` argument for the ABC
    /// compilation step instead of the automatically derived per-module cache path.
    /// Supports both a bare path (`file.protoBin`) and a manifest reference
    /// (`@modules.cache`) matching upstream es2abc semantics.
    ///
    pub cache_file: Option<PathBuf>,
    /// ABC compiler backend. `auto` and `sdk` both use SDK es2abc for hvigor parity.
    pub abc_backend: AbcBackendChoice,
    /// ArkTS checker mode. `oxc` is the native default; `sdk` is retained only
    /// as an explicit comparison backend while its remaining semantic corpus is
    /// migrated. `off` disables checker diagnostics while preserving transforms.
    pub checker: Option<String>,
    /// Enable the ArkUI-X `projectConfig.isCrossplatform` source contract.
    pub is_crossplatform: bool,
    /// Downgrade cross-platform API contract errors to warnings, matching
    /// `projectConfig.ignoreCrossplatformCheck`.
    pub ignore_crossplatform_check: bool,
    /// Enable coarse build cache and ABC ProgramCache handoff.
    pub cache: bool,

    // ── Watch-mode hot-rebuild flags ──────────────────────────────────────────
    // These are set programmatically by WatchStage for hot (ETS/TS source only)
    // changes; they are not accepted from the public JavaScript CLI.
    /// Skip resource compilation in this rebuild (hot-reload optimization).
    ///
    /// Set by the watch stage when only ETS/TS source files changed and no
    /// resource files need recompilation.  Mirrors hvigor's hot rebuild path.
    pub skip_resource: bool,

    /// Skip profile processing in this rebuild (hot-reload optimization).
    ///
    /// Set by the watch stage when only ETS/TS source files changed and
    /// `module.json5`/`build-profile.json5` have not been modified.
    pub skip_profile: bool,

    /// Typed changed-file batch supplied by the watch coordinator.
    ///
    /// This is intentionally internal: hvigor passes the watcher change set to
    /// `shouldInvalidCache`; it is not a user-facing command-line option.
    pub watch_invalidation: Option<WatchInvalidationBatch>,

    /// Custom task input paths declared through Hvigor TaskInput. The Rust
    /// watcher adds these to its typed watch inventory.
    pub custom_task_input_paths: Vec<PathBuf>,

    /// Numeric SDK compression level mapped from hvigor-config's
    /// `ohos.pack.compressLevel` (`fast`/`standard`/`ultimate`).
    pub pack_compress_level: Option<u32>,

    /// Device types injected by DevEco through `ohos.align.deviceTypes`.
    pub align_device_types: Vec<String>,

    /// Explicit `ohos.collect.debugSymbol` override.
    pub collect_debug_symbol: Option<bool>,

    /// Runtime-owned `ohos.uiTransform.Optimization` property.
    pub ui_transform_optimization: bool,

    /// Runtime-owned `ohos.integratedHsp.useGetCurrentBundleName` property.
    pub integrated_hsp_use_get_current_bundle_name: bool,

    /// Runtime-owned `ohos.byteCodeHar.integratedOptimization` property.
    pub byte_code_har_integrated_optimization: bool,

    /// Runtime-owned `hvigor.keepDependency` property.
    pub keep_dependency: bool,

    /// Remaining typed OHOS plugin properties supplied by the JS runtime.
    pub hvigor_properties: HvigorRuntimeProperties,

    /// Hvigor `parameterFile` extra-parameter override supplied by the JS entry.
    pub parameter_file_override: Option<PathBuf>,

    /// Node executable supplied by the same-process JavaScript runtime.
    pub node_executable: Option<PathBuf>,

    /// Project-scoped Hvigor cache directory resolved by `PathUtil` in the JS
    /// runtime. Direct package builds derive `<project>/.hvigor` at the use site.
    pub hvigor_cache_dir: Option<PathBuf>,

    /// Explicit partial hvigor roots selected by the Node task graph.
    pub native_task_roots: Vec<NativeTaskRoot>,
}

impl BuildRequest {
    pub fn resolved_node_executable(&self) -> PathBuf {
        if let Some(executable) = self.node_executable.as_ref() {
            return executable.clone();
        }
        if let Some(node_home) = std::env::var_os("NODE_HOME") {
            return PathBuf::from(node_home)
                .join(arkdown_path::HostPlatform::CURRENT.executable_name("node"));
        }
        PathBuf::from(arkdown_path::HostPlatform::CURRENT.executable_name("node"))
    }

    /// Resolve the npm launcher exactly where hvigor's `NpmCommandBuilder`
    /// looks for it: beside the Node executable that owns the current build.
    /// Windows Node installations expose `npm.cmd`; Unix installations expose
    /// `npm`. No host installation directory is assumed.
    pub fn resolved_npm_executable(&self) -> PathBuf {
        let node = self.resolved_node_executable();
        let npm = arkdown_path::HostPlatform::CURRENT.command_shim_name("npm");
        match node
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => parent.join(npm),
            None => PathBuf::from(npm),
        }
    }

    pub fn build_mode(&self) -> Result<arkdown_core::BuildMode> {
        match self.mode.as_str() {
            "debug" => Ok(arkdown_core::BuildMode::Debug),
            "release" => Ok(arkdown_core::BuildMode::Release),
            "test" => Ok(arkdown_core::BuildMode::Test),
            mode => {
                anyhow::bail!(
                    "unsupported build mode '{mode}'; expected 'debug', 'release', or 'test'"
                )
            }
        }
    }

    pub fn output_target(&self) -> Result<arkdown_core::OutputTarget> {
        match self.target.as_str() {
            "hap" => Ok(arkdown_core::OutputTarget::Hap),
            "hsp" => Ok(arkdown_core::OutputTarget::Hsp),
            "har" => Ok(arkdown_core::OutputTarget::Har),
            // APP is a project-level aggregate (`assembleApp`); the module-level
            // output target is neutral.
            "app" => Ok(arkdown_core::OutputTarget::Hap),
            "preview" => Ok(arkdown_core::OutputTarget::Preview),
            // Local unit test build (`hvigorw test`): `.test` build root, test
            // template generation, TestAbility injection, no HAP packaging.
            "test" => Ok(arkdown_core::OutputTarget::Test),
            target => {
                anyhow::bail!(
          "unsupported build target '{target}'; expected 'hap', 'hsp', 'har', 'app', 'preview', or 'test'"
        )
            }
        }
    }
}
