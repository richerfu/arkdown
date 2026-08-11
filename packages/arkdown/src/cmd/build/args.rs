//! Build command arguments and top-level `BuildCommand` entry point.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use arkdown::plugins::ets_transform::ModuleAnalyzerKind;
use arkdown::plugins::factory::PackagePlugin;
use arkdown_watcher::WatchInvalidationBatch;
use clap::{Args, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum AbcBackendChoice {
  Auto,
  #[default]
  Sdk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ModuleAnalyzerChoice {
  #[default]
  Oxc,
  Yuku,
}

impl ModuleAnalyzerChoice {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Oxc => "oxc",
      Self::Yuku => "yuku",
    }
  }
}

impl From<ModuleAnalyzerChoice> for ModuleAnalyzerKind {
  fn from(value: ModuleAnalyzerChoice) -> Self {
    match value {
      ModuleAnalyzerChoice::Oxc => Self::Oxc,
      ModuleAnalyzerChoice::Yuku => Self::Yuku,
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

#[derive(Args, Clone, Default)]
pub struct BuildArgs {
  /// Build target: hap, hsp, har, app, test
  ///
  /// `test` mirrors hvigor's `hvigorw test` task: local unit-test build root
  /// (`.test/`), GenerateUnitTestTemplate, TestAbility injection, and test
  /// compilation (no HAP packaging). Hvigor injects `buildRoot=.test`,
  /// `isLocalTest=true`, `unitTestMode=true`, `ohos-test-coverage=true` and
  /// `unit.test.replace.page` defaults for the `test` task; arkdown derives
  /// the same behavior from the target alone.
  #[arg(long, default_value = "hap")]
  pub target: String,
  /// Scope filter for local unit tests (`-s class` parameter in the generated
  /// `Index.ets`; hvigor extra config `scope`). Optional — hvigor leaves the
  /// template parameter empty when unset.
  #[arg(long, default_value = None)]
  pub scope: Option<String>,
  /// Code-coverage instrumentation for local unit tests (hvigor extra config
  /// `ohos-test-coverage`). Defaults to `true` for `--target test`, matching
  /// hvigor's `setTestDefaultParameters` injection.
  #[arg(long, default_value = None)]
  pub ohos_test_coverage: Option<bool>,
  /// Optional source-path filter forwarded to hvigor's
  /// `testFrameworkPar.coveragePathFilter`.
  #[arg(long = "coverage-path-filter")]
  pub coverage_path_filter: Option<String>,
  /// Hvigor coverage selection mode: `black`, `white`, or `full`.
  #[arg(long = "coverage-mode", value_parser = ["black", "white", "full"])]
  pub coverage_mode: Option<String>,
  /// Replacement page used by `ReplaceUnitTestIndexFile` and Previewer.
  #[arg(long = "unit-test-replace-page")]
  pub unit_test_replace_page: Option<String>,
  /// Fast build app: hvigor `packOptions.fastBuildApp` — skip the normal HAP
  /// packaging and copy the compiled intermediates into
  /// `intermediates/fastApp/<target>/` for IDE fast deployment
  /// (hvigor `FastPackageHap`).
  #[arg(long = "fast-build-app", default_value = "false")]
  pub fast_build_app: bool,
  /// Build mode: debug, release
  #[arg(long, default_value = "debug")]
  pub mode: String,
  /// Scope to a specific module
  #[arg(long)]
  pub module: Option<String>,
  /// Product name
  #[arg(long)]
  pub product: Option<String>,
  /// Project root (default: current directory)
  #[arg(long)]
  pub project: Option<PathBuf>,
  /// Named target configuration to use (from build-profile.json5 targets[].name).
  ///
  /// When specified, overrides the automatic target selection (buildModeSet + product
  /// matching) and forces the use of the named target configuration.
  /// This implements the multi-target product execution model: different targets can
  /// be built separately to produce multiple artifacts from the same source.
  ///
  /// Example: `arkdown build --build-target-name phone` uses the "phone" target.
  #[arg(long)]
  pub build_target_name: Option<String>,
  /// Override the application build version for this invocation.
  ///
  /// Mirrors hvigor extra config `buildVersion` and takes precedence over the
  /// selected product and `AppScope/app.json5`.
  #[arg(long = "build-version")]
  pub build_version: Option<String>,
  /// Watch mode: rebuild on source file changes (M1 — SPEC-023)
  #[arg(long, default_value = "false")]
  pub watch: bool,
  /// Coverage instrumentation mode: prepare build for code-coverage collection (F20)
  /// Enables build infrastructure for coverage but does not yet inject Istanbul counters.
  #[arg(long, default_value = "false")]
  pub coverage: bool,
  /// Enable AddressSanitizer for the generated package.
  ///
  /// Mirrors hvigor extra config `ohos-debug-asan=true`.
  #[arg(long = "ohos-debug-asan", default_value = "false")]
  pub ohos_debug_asan: bool,
  /// Enable HWAddressSanitizer for the generated package.
  ///
  /// Mirrors hvigor extra config `ohos-enable-hwasan=true`.
  #[arg(long = "ohos-enable-hwasan", default_value = "false")]
  pub ohos_enable_hwasan: bool,
  /// Preview mode: watch resource directories and trigger resource recompilation (M3).
  /// Implies --watch; focuses change detection on src/main/resources/ for faster
  /// resource-only rebuilds during DevEco IDE preview sessions.
  #[arg(long, default_value = "false")]
  pub preview: bool,
  /// Disable the ABC compile cache (parity mode).
  ///
  /// When set, the ABC compilation step skips the coarse-grained `TaskInputs`
  /// cache hit check and always recompiles from source.  This is the equivalent
  /// of hvigor's `--no-cache` flag for the ABC compilation stage.
  ///
  /// Section 3.6: "实现 upstream ProgramCache/protoBin 或 parity 模式禁用 cache".
  /// Use this flag when comparing arkdown ABC output with hvigor's output to
  /// ensure both tools run the full compilation pipeline rather than returning
  /// a cached artifact.
  #[arg(long = "disable-abc-cache", default_value = "false")]
  pub disable_abc_cache: bool,
  /// Override the ABC ProgramCache `.protoBin` file path (upstream `es2abc --cache-file`).
  ///
  /// When specified, this path is used as the `--cache-file` argument for the ABC
  /// compilation step instead of the automatically derived per-module cache path.
  /// Supports both a bare path (`file.protoBin`) and a manifest reference
  /// (`@modules.cache`) matching upstream es2abc semantics.
  ///
  /// Section 3.6: "实现 upstream ProgramCache/protoBin".
  #[arg(long = "cache-file")]
  pub cache_file: Option<PathBuf>,
  /// ABC compiler backend. `auto` and `sdk` both use SDK es2abc for hvigor parity.
  #[arg(long = "abc-backend", value_enum, default_value = "sdk")]
  pub abc_backend: AbcBackendChoice,
  /// ArkTS checker mode. `sdk` (the parity default) runs the installed
  /// etsChecker semantic pass plus OXC structural checks; `oxc` keeps only
  /// the Rust structural pass; `off` disables checker diagnostics while
  /// preserving ETS transforms.
  #[arg(long, default_value = "sdk")]
  pub checker: Option<String>,
  /// Module analyzer used for hvigor type/value split. `yuku` requires a build
  /// compiled with ARKDOWN_YUKU_ROOT and is opt-in for parity experiments.
  #[arg(long = "module-analyzer", value_enum, default_value = "oxc")]
  pub module_analyzer: ModuleAnalyzerChoice,
  /// Enable coarse build cache and ABC ProgramCache handoff.
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  pub cache: bool,

  // ── Watch-mode hot-rebuild flags ──────────────────────────────────────────
  // These are set programmatically by WatchStage for hot (ETS/TS source only)
  // changes; never exposed as CLI flags.
  /// Skip resource compilation in this rebuild (hot-reload optimization).
  ///
  /// Set by the watch stage when only ETS/TS source files changed and no
  /// resource files need recompilation.  Mirrors hvigor's hot rebuild path.
  #[clap(skip)]
  pub skip_resource: bool,

  /// Skip profile processing in this rebuild (hot-reload optimization).
  ///
  /// Set by the watch stage when only ETS/TS source files changed and
  /// `module.json5`/`build-profile.json5` have not been modified.
  #[clap(skip)]
  pub skip_profile: bool,

  /// Typed changed-file batch supplied by the watch coordinator.
  ///
  /// This is intentionally internal: hvigor passes the watcher change set to
  /// `shouldInvalidCache`; it is not a user-facing command-line option.
  #[clap(skip)]
  pub watch_invalidation: Option<WatchInvalidationBatch>,

  /// Project hvigorfile overrides evaluated by the Node runtime.
  #[clap(skip)]
  pub project_hvigor_overrides: Option<arkdown_config::ProjectHvigorOverrides>,

  /// Highest-precedence module build options keyed by normalized module name.
  #[clap(skip)]
  pub module_hvigor_overrides: BTreeMap<String, serde_json::Value>,

  /// Complete in-memory OHOS context replacements owned by the TypeScript
  /// plugin runtime. Watch rebuilds must retain these values instead of
  /// silently reloading the original files.
  #[clap(skip)]
  pub runtime_context_overrides: Option<Arc<arkdown_config::RuntimeContextOverrides>>,

  /// Custom task input paths declared through Hvigor TaskInput. The Rust
  /// watcher adds these to its typed watch inventory.
  #[clap(skip)]
  pub custom_task_input_paths: Vec<PathBuf>,

  /// Numeric SDK compression level mapped from hvigor-config's
  /// `ohos.pack.compressLevel` (`fast`/`standard`/`ultimate`).
  #[clap(skip)]
  pub pack_compress_level: Option<u32>,

  /// Device types injected by DevEco through `ohos.align.deviceTypes`.
  #[clap(skip)]
  pub align_device_types: Vec<String>,

  /// Explicit `ohos.collect.debugSymbol` override.
  #[clap(skip)]
  pub collect_debug_symbol: Option<bool>,

  /// Runtime-owned `ohos.uiTransform.Optimization` property.
  #[clap(skip)]
  pub ui_transform_optimization: bool,

  /// Runtime-owned `ohos.integratedHsp.useGetCurrentBundleName` property.
  #[clap(skip)]
  pub integrated_hsp_use_get_current_bundle_name: bool,

  /// Runtime-owned `ohos.byteCodeHar.integratedOptimization` property.
  #[clap(skip)]
  pub byte_code_har_integrated_optimization: bool,

  /// Runtime-owned `hvigor.keepDependency` property.
  #[clap(skip = true)]
  pub keep_dependency: bool,

  /// Hvigor CLI `parameterFile` extra-parameter override.
  #[clap(skip)]
  pub parameter_file_override: Option<PathBuf>,

  /// Node executable supplied by the same-process JavaScript runtime.
  #[clap(skip)]
  pub node_executable: Option<PathBuf>,

  /// Explicit partial hvigor roots selected by the Node task graph.
  #[clap(skip)]
  pub native_task_roots: Vec<NativeTaskRoot>,
}

pub struct BuildCommand {
  args: BuildArgs,
  package_plugins: Vec<PackagePlugin>,
  task_observer: Option<Arc<dyn super::coordinator::BuildTaskObserver>>,
}

impl BuildCommand {
  pub fn new(args: BuildArgs) -> Self {
    Self {
      args,
      package_plugins: Vec::new(),
      task_observer: None,
    }
  }

  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  pub fn with_task_observer(
    mut self,
    task_observer: Arc<dyn super::coordinator::BuildTaskObserver>,
  ) -> Self {
    self.task_observer = Some(task_observer);
    self
  }

  pub async fn execute(self) -> Result<()> {
    use arkdown_config::ArkdownConfigLoader;
    use arkdown_session::{BuildScriptOverrideInventory, BuildSessionBuilder};

    let mut args = self.args;
    let package_plugins = self.package_plugins;
    let task_observer = self.task_observer;
    let project_root = args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    let mode = args.build_mode()?;
    let target = args.output_target()?;
    let module_analyzer = ModuleAnalyzerKind::from(args.module_analyzer);
    if !module_analyzer.is_available() {
      anyhow::bail!(
        "module analyzer 'yuku' was requested but native yuku support is not compiled; rebuild with ARKDOWN_YUKU_ROOT"
      );
    }

    let config_loader =
      ArkdownConfigLoader::new(&project_root, mode, target, args.product.as_deref(), None);
    let mut config = if let Some(runtime_overrides) = args.runtime_context_overrides.as_deref() {
      config_loader
        .with_runtime_overrides(runtime_overrides)
        .load()
    } else {
      config_loader.load()
    }
    .map_err(|e| anyhow::anyhow!("{}", e))?;
    let static_inventory =
      if args.project_hvigor_overrides.is_none() && args.module_hvigor_overrides.is_empty() {
        Some(
          BuildScriptOverrideInventory::discover(&config)
            .map_err(|error| anyhow::anyhow!("evaluate hvigorfile overrides: {error}"))?,
        )
      } else {
        None
      };
    if let Some(inventory) = static_inventory.as_ref() {
      args.project_hvigor_overrides = inventory.project_overrides().cloned();
      args
        .module_hvigor_overrides
        .clone_from(inventory.module_build_options());
    }
    if let Some(overrides) = args.project_hvigor_overrides.as_ref() {
      let fallback_source = project_root.join("hvigorfile.ts");
      let source_path = static_inventory
        .as_ref()
        .and_then(BuildScriptOverrideInventory::project_source)
        .unwrap_or(fallback_source.as_path());
      overrides
        .apply_to(&mut config, source_path)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(build_version) = args.build_version.as_ref() {
      config.product.build_version = Some(build_version.clone());
    }
    let config = std::sync::Arc::new(config);
    let mut session_builder = BuildSessionBuilder::new(config.clone());
    if let Some(runtime_overrides) = args.runtime_context_overrides.clone() {
      session_builder = session_builder.with_runtime_overrides(runtime_overrides);
    }
    if let Some(parameter_file_override) = args.parameter_file_override.clone() {
      session_builder = session_builder.with_parameter_file_override(parameter_file_override);
    }
    let session = std::sync::Arc::new(
      session_builder
        .build()
        .map_err(|e| anyhow::anyhow!("{}", e))?,
    );
    // `assembleApp` routes to the project-level coordinator; every other
    // target runs the module coordinator.
    if args.target == "app" {
      let mut coordinator = super::app_coordinator::AppBuildCoordinator::new(
        session,
        config,
        args,
      )
      .with_package_plugins(package_plugins);
      if let Some(task_observer) = task_observer {
        coordinator = coordinator.with_task_observer(task_observer);
      }
      coordinator.execute().await?;
      return Ok(());
    }
    let mut coordinator = super::coordinator::BuildCoordinator::new(session, config, args)
      .with_package_plugins(package_plugins);
    if let Some(task_observer) = task_observer {
      coordinator = coordinator.with_task_observer(task_observer);
    }
    let output = coordinator.execute().await?;
    tracing::debug!(
      artifacts = output.artifact_paths.len(),
      "build command completed"
    );
    Ok(())
  }
}

impl BuildArgs {
  pub(crate) fn resolved_node_executable(&self) -> PathBuf {
    if let Some(executable) = self.node_executable.as_ref() {
      return executable.clone();
    }
    if let Some(node_home) = std::env::var_os("NODE_HOME") {
      return PathBuf::from(node_home)
        .join(arkdown_path::HostPlatform::CURRENT.executable_name("node"));
    }
    PathBuf::from(arkdown_path::HostPlatform::CURRENT.executable_name("node"))
  }

  pub(crate) fn build_mode(&self) -> Result<arkdown_core::BuildMode> {
    match self.mode.as_str() {
      "debug" => Ok(arkdown_core::BuildMode::Debug),
      "release" => Ok(arkdown_core::BuildMode::Release),
      mode => anyhow::bail!("unsupported build mode '{mode}'; expected 'debug' or 'release'"),
    }
  }

  pub(crate) fn output_target(&self) -> Result<arkdown_core::OutputTarget> {
    match self.target.as_str() {
      "hap" => Ok(arkdown_core::OutputTarget::Hap),
      "hsp" => Ok(arkdown_core::OutputTarget::Hsp),
      "har" => Ok(arkdown_core::OutputTarget::Har),
      // APP is a project-level aggregate (`assembleApp`); the module-level
      // output target is neutral.
      "app" => Ok(arkdown_core::OutputTarget::Hap),
      // Local unit test build (`hvigorw test`): `.test` build root, test
      // template generation, TestAbility injection, no HAP packaging.
      "test" => Ok(arkdown_core::OutputTarget::Test),
      target => {
        anyhow::bail!(
          "unsupported build target '{target}'; expected 'hap', 'hsp', 'har', 'app', or 'test'"
        )
      }
    }
  }
}
