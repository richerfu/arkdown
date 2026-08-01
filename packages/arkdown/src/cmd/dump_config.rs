//! Dump the resolved effective build configuration as JSON.
//!
//! Implements the "hvigor effective-config oracle dump 对齐模式" from
//! the gap analysis (section 3.1).  The output can be compared against
//! `hvigor`'s `--dump-config` flag to detect differences in config
//! resolution without running a full build.
//!
//! ## Usage
//!
//! ```text
//! arkdown dump-config --project . --product default --mode debug
//! ```
//!
//! The output JSON includes:
//! - `product`: resolved product configuration (bundleName, compatibleSdkVersion, etc.)
//! - `build_mode`: `"debug"` or `"release"`
//! - `output_target`: `"hap"`, `"hsp"`, or `"har"`
//! - `modules`: list of module configurations with effective target info
//! - `sdk_config`: SDK path configuration
//! - `version_overrides`: product-level versionCode/versionName if set

use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use serde_json::{json, Value};

/// Dump the resolved effective configuration as JSON for oracle comparison.
#[derive(Args, Debug)]
pub struct DumpConfigArgs {
  /// Project root (default: current directory)
  #[arg(long)]
  pub project: Option<PathBuf>,

  /// Product name to resolve (default: "default")
  #[arg(long, default_value = "default")]
  pub product: String,

  /// Build mode: "debug" or "release" (default: "debug")
  #[arg(long, default_value = "debug")]
  pub mode: String,

  /// Output target: "hap", "hsp", or "har" (default: "hap")
  #[arg(long, default_value = "hap")]
  pub target: String,

  /// Module name filter (dump only this module's effective target config)
  #[arg(long)]
  pub module: Option<String>,

  /// Pretty-print output (default: true)
  #[arg(long, default_value = "true")]
  pub pretty: bool,
}

pub fn run(args: DumpConfigArgs) -> anyhow::Result<()> {
  let project_root = args
    .project
    .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

  let build_mode = match args.mode.as_str() {
    "release" => arkdown_core::BuildMode::Release,
    _ => arkdown_core::BuildMode::Debug,
  };
  let output_target = match args.target.as_str() {
    "hsp" => arkdown_core::OutputTarget::Hsp,
    "har" => arkdown_core::OutputTarget::Har,
    _ => arkdown_core::OutputTarget::Hap,
  };
  let config = arkdown_config::load_config(
    &project_root,
    build_mode,
    output_target,
    Some(args.product.as_str()),
    args.module.as_deref(),
  )?;

  // Build effective target config for each module.
  let build_mode_name = args.mode.as_str();
  let mut modules_out = Vec::new();
  for module in &config.modules {
    let effective_target = arkdown_config::build_profile::select_effective_target(
      &project_root,
      &module.name,
      &config.product.name,
      build_mode_name,
    );

    let applicable_count = arkdown_config::build_profile::count_applicable_targets_for_module(
      &project_root,
      &module.name,
      &config.product.name,
      build_mode_name,
    );

    let module_json = json!({
        "name": module.name,
        "src_path": module.src_path.display().to_string(),
        "module_type": format!("{:?}", module.module_type),
        "effective_target": effective_target.as_ref().map(|t| json!({
            "name": t.name,
            "build_mode_set": t.build_mode_set,
            "resource_directories": t.resource_directories
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>(),
            "distribution_filter": t.distribution_filter,
            "source_abilities_count": t.source_abilities.len(),
            "source_roots": t.source_roots,
        })),
        "applicable_target_count": applicable_count,
        "multi_target_warning": applicable_count > 1,
    });
    modules_out.push(module_json);
  }

  // Build the full effective config output.
  let output: Value = json!({
      "_schema": "arkdown effective-config v1 (section 3.1 oracle dump)",
      "_compare_with": "hvigor --dump-config or hvigorw build -Pmodule=<name> --dump-config",
      "project_root": project_root.display().to_string(),
      "product": {
          "name": config.product.name,
          "bundle_name": config.product.bundle_name,
          "compatible_sdk_version": config.product.compatible_sdk_version,
          "compatible_sdk_version_stage": config.product.compatible_sdk_version_stage,
          "runtime_os": config.product.runtime_os,
          "version_code_override": config.product.version_code,
          "version_name_override": config.product.version_name,
          "build_option": {
              "use_normalized_ohm_url": config.product.build_option.strict_mode.use_normalized_ohm_url,
              "case_sensitive_check": config.product.build_option.strict_mode.case_sensitive_check,
              "preload_system_so": config.product.build_option.preload_system_so,
              "abi_filters": config.product.build_option.abi_filters,
              "native_lib": {
                  "collect_all_libs": config.product.build_option.native_lib.collect_all_libs,
                  "filter": config.product.build_option.native_lib.filter,
              },
              "aot_compile_mode": config.product.build_option.aot_compile_mode,
          }
      },
      "build_mode": args.mode,
      "output_target": args.target,
      "sdk": {
          "compile_sdk_version": config.sdk_config.compile_sdk_version,
          "compile_api_version": config.sdk_config.compile_api_version,
          "sdk_component_api_version": config.sdk_config.sdk_component_api_version,
          "has_rollup_plugin": config.sdk_config.has_rollup_plugin,
          "ets_loader_path": config.sdk_config.ets_loader_path.display().to_string(),
          "es2abc_path": config.sdk_config.es2abc_path.display().to_string(),
      },
      "modules": modules_out,
  });

  let stdout = std::io::stdout();
  let mut stdout = stdout.lock();
  if args.pretty {
    serde_json::to_writer_pretty(&mut stdout, &output)?
  } else {
    serde_json::to_writer(&mut stdout, &output)?
  }
  stdout.write_all(b"\n")?;

  Ok(())
}
