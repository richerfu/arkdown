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

use arkdown_core::{BuildMode, OutputTarget};
use arkdown_plugin_ets_transform::{load_system_resource_references, EtsTransformPlugin};

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
        let resource_refs = load_system_resource_references(
            &config.sdk_config.ets_loader_path,
            &config.product.bundle_name,
            &module.name,
        )
        .unwrap_or_default();

        let transformer = EtsTransformPlugin::with_resource_references(
            std::sync::Arc::new(config.clone()),
            resource_refs,
        );

        let ets_root = module.src_path.join("src/main/ets");
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
                    eprintln!("check: cannot read {}: {e}", path.display());
                    total_errors += 1;
                    continue;
                }
            };

            // Run the transform (struct validation is embedded in the transform).
            // Any diagnostic errors are printed to stderr by the transform plugin.
            if let Err(e) = transformer.do_transform(&source, &path.to_string_lossy().into_owned())
            {
                eprintln!("check: {}: {e}", path.display());
                total_errors += 1;
            }
        }
    }

    if total_errors > 0 {
        eprintln!("\ncheck: {total_errors} error(s) in {total_files} file(s)");
        std::process::exit(1);
    } else {
        println!("check: {total_files} file(s) passed");
    }

    Ok(())
}
