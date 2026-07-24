//! Build command arguments and top-level `BuildCommand` entry point.

use std::collections::BTreeMap;
use std::path::PathBuf;

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

#[derive(Args, Clone, Default)]
pub struct BuildArgs {
  /// Build target: hap, hsp, har
  #[arg(long, default_value = "hap")]
  pub target: String,
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
  /// ArkTS checker mode. `oxc` runs the OXC-backed checker; `off` disables
  /// OXC checker diagnostics while keeping ETS transforms enabled.
  #[arg(long, default_value = "oxc")]
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
}

pub struct BuildCommand {
  args: BuildArgs,
  package_plugins: Vec<PackagePlugin>,
}

impl BuildCommand {
  pub fn new(args: BuildArgs) -> Self {
    Self {
      args,
      package_plugins: Vec::new(),
    }
  }

  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  pub async fn execute(self) -> Result<()> {
    use arkdown_config::load_config;
    use arkdown_session::{BuildScriptOverrideInventory, BuildSessionBuilder};

    let mut args = self.args;
    let package_plugins = self.package_plugins;
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

    let mut config = load_config(&project_root, mode, target, args.product.as_deref(), None)
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
    let session = std::sync::Arc::new(
      BuildSessionBuilder::new(config.clone())
        .build()
        .map_err(|e| anyhow::anyhow!("{}", e))?,
    );
    let output = super::coordinator::BuildCoordinator::new(session, config, args)
      .with_package_plugins(package_plugins)
      .execute()
      .await?;
    tracing::debug!(
      artifacts = output.artifact_paths.len(),
      "build command completed"
    );
    Ok(())
  }
}

impl BuildArgs {
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
      target => {
        anyhow::bail!("unsupported build target '{target}'; expected 'hap', 'hsp', or 'har'")
      }
    }
  }
}
