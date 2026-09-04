//! Native OXC-backed standalone ArkTS checking.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use walkdir::WalkDir;

use arkdown::plugins::ets_transform::{
  ArktsSourceChecker, ComponentMetadata, SystemApiCheckOptions,
};
use arkdown_core::{ArkdownConfig, BuildMode, ModuleConfig, OutputTarget};
use arkdown_path::{ArtifactPathSerializer, HostPlatform};
use arkdown_session::{BuildSession, BuildSessionBuilder, ModuleCompileContext};

use arkdown_build::package_resolution::{
  LocalPackageSourceRootInventory, ModulePackageResolutionPlanner,
};
use arkdown_build::sdk_checker::{
  SdkEtsCheckerInput, SdkEtsCheckerPaths, SdkEtsCheckerPlanBuilder, SdkEtsCheckerRunner,
};
use arkdown_build::source_snapshot::TransformSourceSnapshotBuilder;

#[derive(Debug)]
pub struct CheckRequest {
  /// Build target context: hap, hsp, har
  pub target: String,
  /// Scope to a specific module
  pub module: Option<String>,
  /// Project root (default: current directory)
  pub project: Option<PathBuf>,
  /// Node executable used to invoke the SDK semantic checker.
  pub node_executable: Option<PathBuf>,
  /// Enable ArkUI-X cross-platform API validation.
  pub is_crossplatform: bool,
  /// Downgrade cross-platform API diagnostics to warnings.
  pub ignore_crossplatform_check: bool,
  /// ArkTS checker backend. `oxc` runs without Node. `sdk` is an explicit
  /// comparison backend while the remaining TypeScript semantic corpus moves.
  pub checker: String,
}

pub fn run(args: CheckRequest) -> Result<()> {
  CheckCommandRunner::new(args).run()
}

struct CheckCommandRunner {
  args: CheckRequest,
}

impl CheckCommandRunner {
  fn new(args: CheckRequest) -> Self {
    Self { args }
  }

  fn run(self) -> Result<()> {
    let project_root = self
      .args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let target = Self::parse_target(&self.args.target);
    let checker_backend = CheckBackend::parse(&self.args.checker)?;
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
        checker_backend,
        self.args.is_crossplatform,
        self.args.ignore_crossplatform_check,
      )
      .run()?;
      summary.merge(module_summary);
    }
    if summary.errors > 0 {
      let details = if summary.diagnostics.is_empty() {
        String::new()
      } else {
        format!("\n{}", summary.diagnostics.join("\n"))
      };
      anyhow::bail!(
        "ARKDOWN_ETS_CHECK_FAILED: {} error(s) across {} source file(s){}",
        summary.errors,
        summary.files,
        details
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
  checker_backend: CheckBackend,
  is_crossplatform: bool,
  ignore_crossplatform_check: bool,
}

impl<'a> CheckModuleRunner<'a> {
  fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    module_context: &'a ModuleCompileContext,
    node_executable: PathBuf,
    checker_backend: CheckBackend,
    is_crossplatform: bool,
    ignore_crossplatform_check: bool,
  ) -> Self {
    Self {
      config,
      session,
      module,
      module_context,
      node_executable,
      checker_backend,
      is_crossplatform,
      ignore_crossplatform_check,
    }
  }

  fn run(&self) -> Result<CheckSummary> {
    let sources = CheckSourceInventory::new(&self.module_context.module.source_roots).collect()?;
    let mut summary = CheckSummary {
      files: sources.len(),
      ..CheckSummary::default()
    };
    if self.checker_backend.runs_sdk() {
      self.run_sdk_semantic_checker(&sources, &mut summary)?;
    }
    self.run_oxc_structural_checker(&sources, &mut summary)?;
    Ok(summary)
  }

  fn run_sdk_semantic_checker(
    &self,
    sources: &[PathBuf],
    summary: &mut CheckSummary,
  ) -> Result<()> {
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
    let mut local_sources = LocalPackageSourceRootInventory::default();
    local_sources.insert(
      &self.module_context.module.root,
      [
        self.module_context.module.src_main.clone(),
        paths.generated_profile_path.clone(),
      ],
    );
    let package_resolution =
      ModulePackageResolutionPlanner::new(self.module_context, &local_sources).build()?;
    let plan = SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
      config: self.config,
      session: self.session,
      module: self.module,
      module_context: self.module_context,
      package_resolution: &package_resolution,
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
      preview_mode: false,
      check_entry: false,
      watch_mode: false,
      byte_code_har_integrated_optimization: false,
      single_file_emit: false,
      rollup_cache_use_source_hash: false,
      is_crossplatform: self.is_crossplatform,
      ignore_crossplatform_check: self.ignore_crossplatform_check,
    })
    .build()?;
    let output = SdkEtsCheckerRunner::new(&plan).run()?;
    for diagnostic in &output.diagnostics {
      match diagnostic.level.to_ascii_lowercase().as_str() {
        "error" => {
          tracing::error!(
            module = %output.module_name,
            "ArkTS semantic check: {}",
            diagnostic.message
          );
          summary
            .diagnostics
            .push(format!("[{}] {}", output.module_name, diagnostic.message));
        }
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
    summary.errors += output.error_count();
    Ok(())
  }

  fn run_oxc_structural_checker(
    &self,
    sources: &[PathBuf],
    summary: &mut CheckSummary,
  ) -> Result<()> {
    let component_metadata = Arc::new(
      ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
        &self.config.sdk_config.ets_loader_path,
        self.session.sdk.global_module_paths.as_ref(),
      )?,
    );
    let module_roots: Vec<(String, PathBuf)> = self
      .config
      .modules
      .iter()
      .map(|module| (module.name.clone(), module.src_path.clone()))
      .collect();
    let mut sdk_declaration_roots = self
      .config
      .sdk_config
      .ets_api_dirs()
      .into_iter()
      .chain(self.config.sdk_config.ets_kits_dirs())
      .chain(self.config.sdk_config.ets_arkts_dirs())
      .collect::<Vec<_>>();
    for root in self.session.sdk.global_module_paths.iter() {
      if !sdk_declaration_roots.contains(root) {
        sdk_declaration_roots.push(root.clone());
      }
    }
    let project_sources = TransformSourceSnapshotBuilder::new(&module_roots, &[], &[], &[])
      .with_sdk_declaration_roots(&sdk_declaration_roots)
      .build()?;
    let mut system_api = SystemApiCheckOptions::default()
      .with_compatible_sdk_version(self.config.product.compatible_sdk_version.clone())
      .with_compile_sdk_version(self.config.sdk_config.compile_api_version.to_string())
      .with_runtime_os(self.config.product.runtime_os.clone())
      .with_atomic_service(self.session.project.bundle_type == "atomicService")
      .with_crossplatform(self.is_crossplatform, self.ignore_crossplatform_check)
      .with_strict_api_compatibility(matches!(
        self
          .config
          .product
          .build_option
          .strict_mode
          .api_compatibility_check,
        Some(arkdown_core::ApiCompatibilityCheckLevel::Error)
      ))
      .with_request_permissions(
        self
          .module_context
          .module
          .module_json
          .as_ref()
          .into_iter()
          .flat_map(|manifest| manifest.module.request_permissions.iter())
          .filter_map(|permission| permission.get("name").and_then(serde_json::Value::as_str))
          .map(str::to_string),
      );
    if let Some(capabilities) =
      self
        .module_context
        .module
        .module_json
        .as_ref()
        .and_then(|manifest| {
          self
            .session
            .sdk
            .system_capabilities
            .select(&manifest.module.device_types)
        })
    {
      system_api =
        system_api.with_system_capabilities(capabilities.intersection, capabilities.union);
    }
    let checker = ArktsSourceChecker::new(component_metadata)
      .with_project_sources(project_sources.clone(), module_roots)
      .with_api_available_context(
        &self.config.product.compatible_sdk_version,
        &self.config.product.runtime_os,
      )
      .with_system_api_context(system_api);
    for path in sources
      .iter()
      .filter(|path| CheckSourceInventory::is_arkts_source(path))
    {
      let Some(source) = project_sources.source_text(path) else {
        continue;
      };
      let source_id = match ArtifactPathSerializer::path_to_slash_string(path) {
        Ok(source_id) => source_id,
        Err(error) => {
          tracing::error!(path = %path.display(), error = %error, "check cannot encode source path");
          summary.errors += 1;
          summary
            .diagnostics
            .push(format!("[{}] {error}", path.display()));
          continue;
        }
      };
      let report = checker.check(&source_id, &source);
      for diagnostic in report.diagnostics() {
        if diagnostic.is_error() {
          tracing::error!(path = %path.display(), diagnostic = %diagnostic, "ArkTS OXC check failed");
          summary
            .diagnostics
            .push(format!("[{}] {diagnostic}", path.display()));
        } else {
          tracing::warn!(path = %path.display(), diagnostic = %diagnostic, "ArkTS OXC check warning");
        }
      }
      summary.errors += report.error_count();
    }
    Ok(())
  }
}

#[derive(Clone, Copy)]
enum CheckBackend {
  Sdk,
  Oxc,
}

impl CheckBackend {
  fn parse(value: &str) -> Result<Self> {
    match value {
      "sdk" => Ok(Self::Sdk),
      "oxc" => Ok(Self::Oxc),
      _ => anyhow::bail!("unsupported checker '{value}'; expected 'sdk' or 'oxc'"),
    }
  }

  fn runs_sdk(self) -> bool {
    matches!(self, Self::Sdk)
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

  fn is_arkts_source(path: &Path) -> bool {
    matches!(
      path.extension().and_then(|extension| extension.to_str()),
      Some("ets" | "ts")
    ) && !path
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.ends_with(".d.ets") || name.ends_with(".d.ts"))
  }
}

#[derive(Default)]
struct CheckSummary {
  files: usize,
  errors: usize,
  diagnostics: Vec<String>,
}

impl CheckSummary {
  fn merge(&mut self, other: Self) {
    self.files += other.files;
    self.errors += other.errors;
    self.diagnostics.extend(other.diagnostics);
  }
}
