//! M5 — ETS standalone checker (lint-only mode, SPEC-023).
//!
//! Runs the ETS struct-declaration validator on every `.ets` source file in
//! the project without producing any build output.  Mirrors hvigor's
//! `etsStandaloneChecker` SDK plugin:
//!
//! - Validates struct decorator rules (missing `@Component`, V1/V2 conflicts,
//!   missing `build()`, worker UI decorators, duplicate names, return type).
//! - Reports diagnostics to stderr in the same format as the full build.
//! - Exits non-zero if any errors are found.
//!
//! This command does NOT run TypeScript semantic type checking (that requires
//! the full `ets_checker.js` pipeline from the SDK).

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use walkdir::WalkDir;

use arkdown::plugins::ets_transform::{
  ComponentMetadata, RolldownEtsTransformPlan, SystemResourceReferenceLoader,
  TransformSourceSnapshot,
};
use arkdown_core::{BuildMode, OutputTarget};
use arkdown_ets_transform::RolldownEtsTransformer;
use arkdown_path::ArtifactPathSerializer;
use arkdown_session::paths::ModulePathLayout;

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
}

pub fn run(args: CheckArgs) -> Result<()> {
  let project_root = args
    .project
    .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

  let target = match args.target.as_str() {
    "hsp" => OutputTarget::Hsp,
    "har" => OutputTarget::Har,
    _ => OutputTarget::Hap,
  };

  let config = arkdown_config::load_config(&project_root, BuildMode::Debug, target, None, None)
    .map_err(|e| anyhow::anyhow!("config error: {e}"))?;

  let mut total_errors = 0usize;
  let mut total_files = 0usize;

  for module in &config.modules {
    if let Some(name) = args.module.as_deref() {
      if module.name != name {
        continue;
      }
    }

    // Load resource references for kit import rewriting.
    let resource_refs = SystemResourceReferenceLoader::new(
      &config.sdk_config.ets_loader_path,
      &config.product.bundle_name,
      &module.name,
    )
    .load()
    .unwrap_or_default();

    let component_metadata =
      ComponentMetadata::from_ets_loader_path(&config.sdk_config.ets_loader_path)?;
    let transform_plan = std::sync::Arc::new(RolldownEtsTransformPlan::new(
      config.product.bundle_name.clone(),
      module.name.clone(),
      config.project_root.clone(),
      config
        .modules
        .iter()
        .map(|module| (module.name.clone(), module.src_path.clone()))
        .collect(),
      config.sdk_config.compile_api_version,
      resource_refs,
      component_metadata,
      TransformSourceSnapshot::default(),
    ));
    let transformer = RolldownEtsTransformer::new(transform_plan);

    let ets_root = ModulePathLayout::new(&module.src_path).ets_source_root();
    if !ets_root.exists() {
      continue;
    }

    for entry in WalkDir::new(&ets_root)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
      .filter(|e| {
        e.path()
          .extension()
          .and_then(|ext| ext.to_str())
          .map_or(false, |ext| ext == "ets")
      })
    {
      let path = entry.path();
      total_files += 1;

      let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
          tracing::error!(path = %path.display(), error = %e, "check cannot read source");
          total_errors += 1;
          continue;
        }
      };

      // Run the transform (struct validation is embedded in the transform).
      // Any diagnostic errors are printed to stderr by the transform plugin.
      let source_id = match ArtifactPathSerializer::path_to_slash_string(path) {
        Ok(id) => id,
        Err(error) => {
          tracing::error!(
              path = %path.display(),
              error = %error,
              "check cannot encode source path"
          );
          total_errors += 1;
          continue;
        }
      };
      if let Err(e) = transformer.transform(&source, &source_id) {
        tracing::error!(path = %path.display(), error = %e, "check transform failed");
        total_errors += 1;
      }
    }
  }

  if total_errors > 0 {
    tracing::error!(total_errors, total_files, "check failed");
    std::process::exit(1);
  } else {
    tracing::info!(total_files, "check passed");
  }

  Ok(())
}
