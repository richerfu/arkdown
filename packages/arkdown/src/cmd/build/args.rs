//! Build command arguments and top-level `BuildCommand` entry point.

use std::path::PathBuf;

use anyhow::Result;
use arkdown::plugins::factory::PackagePlugin;
use clap::{Args, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum AbcBackendChoice {
  Auto,
  #[default]
  Sdk,
}

#[derive(Args, Default)]
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
    use arkdown_core::{BuildMode, OutputTarget};
    use arkdown_session::BuildSessionBuilder;

    let project_root = self
      .args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    let mode = match self.args.mode.as_str() {
      "release" => BuildMode::Release,
      _ => BuildMode::Debug,
    };
    let target = match self.args.target.as_str() {
      "hsp" => OutputTarget::Hsp,
      "har" => OutputTarget::Har,
      _ => OutputTarget::Hap,
    };

    let config = std::sync::Arc::new(
      load_config(
        &project_root,
        mode,
        target,
        self.args.product.as_deref(),
        None,
      )
      .map_err(|e| anyhow::anyhow!("{}", e))?,
    );
    let session = std::sync::Arc::new(
      BuildSessionBuilder::new(config.clone())
        .build()
        .map_err(|e| anyhow::anyhow!("{}", e))?,
    );
    let output = super::coordinator::BuildCoordinator::new(session, config, self.args)
      .with_package_plugins(self.package_plugins)
      .execute()
      .await?;
    tracing::debug!(
      artifacts = output.artifact_paths.len(),
      "build command completed"
    );
    Ok(())
  }
}
