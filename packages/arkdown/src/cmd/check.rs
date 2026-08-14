//! SDK-backed standalone ArkTS checking.
//!
//! `arkdown check` keeps the Rust/OXC structural validator and additionally
//! invokes the installed SDK's `etsStandaloneChecker` for TypeScript semantic
//! diagnostics. This is the same semantic owner used by Hvigor's ArkLint
//! worker: `ets-loader/lib/ets_checker.js::etsStandaloneChecker`. Normal
//! builds use the separate `etsChecker.buildStart` CompileArkTS entry.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use walkdir::WalkDir;

use arkdown::plugins::ets_transform::{
  ComponentMetadata, RolldownEtsTransformPlan, SystemResourceReferenceLoader,
  TransformSourceSnapshot,
};
use arkdown_core::{ArkdownConfig, BuildMode, ModuleConfig, OutputTarget};
use arkdown_ets_transform::RolldownEtsTransformer;
use arkdown_path::{ArtifactPathSerializer, HostPlatform};
use arkdown_session::{BuildSession, BuildSessionBuilder, ModuleCompileContext};

use super::ets_checker::{
  SdkEtsCheckerInput, SdkEtsCheckerPaths, SdkEtsCheckerPlanBuilder, SdkEtsCheckerRunner,
};

#[derive(Args, Debug)]
pub struct CheckArgs {
  /// Build target context: hap, hsp, har
  #[arg(long, default_value = "hap")]
  pub target: String,
  /// Scope to a specific module
  #[arg(long)]
  pub module: Option<String>,
  /// Project root (default: current directory)
  #[arg(long)]
  pub project: Option<PathBuf>,
  /// Node executable used to invoke the SDK semantic checker.
  #[arg(long)]
  pub node_executable: Option<PathBuf>,
}

pub fn run(args: CheckArgs) -> Result<()> {
  CheckCommandRunner::new(args).run()
}

struct CheckCommandRunner {
  args: CheckArgs,
}

impl CheckCommandRunner {
  fn new(args: CheckArgs) -> Self {
    Self { args }
  }

  fn run(self) -> Result<()> {
    let project_root = self
      .args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let target = Self::parse_target(&self.args.target);
    let config = Arc::new(
      arkdown_config::load_config(&project_root, BuildMode::Debug, target, None, None)
        .map_err(|error| anyhow::anyhow!("config error: {error}"))?,
    );
    let session = Arc::new(
      BuildSessionBuilder::new(config.clone())
        .build()
        .map_err(|error| anyhow::anyhow!("check session: {error}"))?,
    );
    let mut summary = CheckSummary::default();
    for module in &config.modules {
      if self
        .args
        .module
        .as_deref()
        .is_some_and(|name| name != module.name)
      {
        continue;
      }
      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!(
          "check session is missing module context for '{}'",
          module.name
        )
      })?;
      let module_summary = CheckModuleRunner::new(
        config.as_ref(),
        session.as_ref(),
        module,
        &module_context,
        self.node_executable(),
      )
      .run()?;
      summary.merge(module_summary);
    }
    if summary.errors > 0 {
      anyhow::bail!(
        "ARKDOWN_ETS_CHECK_FAILED: {} error(s) across {} source file(s)",
        summary.errors,
        summary.files
      );
    }
    tracing::info!(files = summary.files, "check passed");
    Ok(())
  }

  fn parse_target(target: &str) -> OutputTarget {
    match target {
      "hsp" => OutputTarget::Hsp,
      "har" => OutputTarget::Har,
      _ => OutputTarget::Hap,
    }
  }

  fn node_executable(&self) -> PathBuf {
    if let Some(executable) = self.args.node_executable.as_ref() {
      return executable.clone();
    }
    if let Some(node_home) = std::env::var_os("NODE_HOME") {
      return PathBuf::from(node_home).join(HostPlatform::CURRENT.executable_name("node"));
    }
    PathBuf::from(HostPlatform::CURRENT.executable_name("node"))
  }
}

struct CheckModuleRunner<'a> {
  config: &'a ArkdownConfig,
  session: &'a BuildSession,
  module: &'a ModuleConfig,
  module_context: &'a ModuleCompileContext,
  node_executable: PathBuf,
}

impl<'a> CheckModuleRunner<'a> {
  fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    module_context: &'a ModuleCompileContext,
    node_executable: PathBuf,
  ) -> Self {
    Self {
      config,
      session,
      module,
      module_context,
      node_executable,
    }
  }

  fn run(&self) -> Result<CheckSummary> {
    let sources = CheckSourceInventory::new(&self.module_context.module.source_roots).collect()?;
    let mut summary = CheckSummary {
      files: sources.len(),
      ..CheckSummary::default()
    };
    summary.errors += self.run_sdk_semantic_checker(&sources)?;
    summary.errors += self.run_oxc_structural_checker(&sources)?;
    Ok(summary)
  }

  fn run_sdk_semantic_checker(&self, sources: &[PathBuf]) -> Result<usize> {
    let module_json_path = self
      .module_context
      .module
      .module_json_path
      .clone()
      .ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_ETS_CHECKER_MODULE_JSON: module '{}' has no src/main/module.json5",
          self.module.name
        )
      })?;
    let paths = SdkEtsCheckerPaths::standalone(&self.module_context.module.root, module_json_path);
    let plan = SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
      config: self.config,
      session: self.session,
      module: self.module,
      module_context: self.module_context,
      module_build_option: &self.config.product.build_option,
      loader_json: None,
      target_name: "default",
      node_executable: self.node_executable.clone(),
      paths,
      entries: sources.to_vec(),
      coverage_enabled: false,
      coverage_path_filter: None,
      coverage_mode: None,
      debug_line: false,
      byte_code_har_integrated_optimization: false,
      single_file_emit: false,
      rollup_cache_use_source_hash: false,
    })
    .build()?;
    let output = SdkEtsCheckerRunner::new(&plan).run()?;
    for diagnostic in &output.diagnostics {
      match diagnostic.level.to_ascii_lowercase().as_str() {
        "error" => tracing::error!(
          module = %output.module_name,
          "ArkTS semantic check: {}",
          diagnostic.message
        ),
        "warn" | "warning" => tracing::warn!(
          module = %output.module_name,
          "ArkTS semantic check: {}",
          diagnostic.message
        ),
        _ => tracing::debug!(
          module = %output.module_name,
          level = %diagnostic.level,
          "ArkTS semantic check: {}",
          diagnostic.message
        ),
      }
    }
    Ok(output.error_count())
  }

  fn run_oxc_structural_checker(&self, sources: &[PathBuf]) -> Result<usize> {
    let resource_refs = SystemResourceReferenceLoader::new(
      &self.config.sdk_config.ets_loader_path,
      &self.config.product.bundle_name,
      &self.module.name,
    )
    .load()
    .unwrap_or_default();
    let component_metadata =
      ComponentMetadata::from_ets_loader_path(&self.config.sdk_config.ets_loader_path)?;
    let transform_plan = Arc::new(RolldownEtsTransformPlan::new(
      self.config.product.bundle_name.clone(),
      self.module.name.clone(),
      self.config.project_root.clone(),
      self.module_context.module.src_main.join("ets"),
      self
        .config
        .modules
        .iter()
        .map(|module| (module.name.clone(), module.src_path.clone()))
        .collect(),
      self.config.sdk_config.compile_api_version,
      resource_refs,
      component_metadata,
      TransformSourceSnapshot::default(),
    ));
    let transformer = RolldownEtsTransformer::new(transform_plan);
    let mut errors = 0usize;
    for path in sources
      .iter()
      .filter(|path| CheckSourceInventory::is_ets(path))
    {
      let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
          tracing::error!(path = %path.display(), error = %error, "check cannot read source");
          errors += 1;
          continue;
        }
      };
      let source_id = match ArtifactPathSerializer::path_to_slash_string(path) {
        Ok(source_id) => source_id,
        Err(error) => {
          tracing::error!(path = %path.display(), error = %error, "check cannot encode source path");
          errors += 1;
          continue;
        }
      };
      if let Err(error) = transformer.transform(&source, &source_id) {
        tracing::error!(path = %path.display(), error = %error, "ArkTS structural check failed");
        errors += 1;
      }
    }
    Ok(errors)
  }
}

struct CheckSourceInventory<'a> {
  roots: &'a [PathBuf],
}

impl<'a> CheckSourceInventory<'a> {
  fn new(roots: &'a [PathBuf]) -> Self {
    Self { roots }
  }

  fn collect(&self) -> Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    for root in self.roots {
      if !root.is_dir() {
        continue;
      }
      for entry in WalkDir::new(root).follow_links(false) {
        let entry =
          entry.map_err(|error| anyhow::anyhow!("ARKDOWN_ETS_CHECK_SOURCE_WALK: {}", error))?;
        if entry.file_type().is_file() && Self::is_checkable_source(entry.path()) {
          sources.push(entry.path().to_path_buf());
        }
      }
    }
    sources.sort();
    sources.dedup();
    Ok(sources)
  }

  fn is_checkable_source(path: &Path) -> bool {
    if path
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.ends_with(".d.ts") || name.ends_with(".d.ets"))
    {
      return false;
    }
    matches!(
      path.extension().and_then(|extension| extension.to_str()),
      Some("ets" | "ts" | "js")
    )
  }

  fn is_ets(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("ets")
      && !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".d.ets"))
  }
}

#[derive(Default)]
struct CheckSummary {
  files: usize,
  errors: usize,
}

impl CheckSummary {
  fn merge(&mut self, other: Self) {
    self.files += other.files;
    self.errors += other.errors;
  }
}
