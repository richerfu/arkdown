use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;

use arkdown::Bundler;
use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};
use arkdown_config::load_config;
use arkdown_core::{BuildMode, ModuleJson5, ModuleJson5Module, OutputTarget};
use arkdown_pack::{PackInput, Packager};
use arkdown_profile::ProfileProcessor;
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};

use crate::progress::Progress;

#[derive(Args)]
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
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let project_root = args
        .project
        .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let start = Instant::now();

    let target = match args.target.as_str() {
        "hsp" => OutputTarget::Hsp,
        "har" => OutputTarget::Har,
        _ => OutputTarget::Hap,
    };
    let mode = match args.mode.as_str() {
        "release" => BuildMode::Release,
        _ => BuildMode::Debug,
    };

    let progress = Progress::new();

    // Step 1: Config
    progress.step(1, 6, "Parsing config");
    let config = load_config(
        &project_root,
        mode,
        target,
        args.product.as_deref(),
        args.module.as_deref(),
    )
    .context("Failed to parse project config")?;
    progress.done();

    let config_arc = Arc::new(config);

    // Step 2: Profile processing
    progress.step(2, 6, "Processing profiles");
    let processor = ProfileProcessor::new(config_arc.clone());
    let mut loader_jsons = Vec::new();
    for module in &config_arc.modules {
        let module_json5_path = module.src_path.join("src/main/module.json5");
        let module_json5 = read_module_json5(&module_json5_path)?;
        let (manifest, loader_json) = processor
            .process_module(module, &module_json5)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        loader_jsons.push((module.clone(), manifest, loader_json));
    }
    progress.done();

    // Steps 3 & 4: Resources + Bundling
    progress.step(3, 6, "Compiling resources + bundling");
    for (module, _manifest, loader_json) in &loader_jsons {
        // Resource compilation
        let resource_compiler = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config_arc.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: matches!(config_arc.build_mode, BuildMode::Debug),
        });
        let res_dir = module.src_path.join("src/main/resources");
        let res_out = module.src_path.join("build/default/intermediates/res");
        if res_dir.exists() {
            let _ = resource_compiler.compile(ResourceCompileInput {
                resources_dirs: vec![res_dir],
                output_dir: res_out,
            });
        }

        // Bundling
        let bundler = Bundler::new((*config_arc).clone(), vec![]);
        let _bundle_out = bundler
            .bundle(loader_json)
            .await
            .map_err(|e| anyhow::anyhow!("Bundle failed: {}", e))?;
    }
    progress.done();

    // Step 5: ABC compilation
    progress.step(5, 6, "Compiling ABC");
    let abc_compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: matches!(config_arc.build_mode, BuildMode::Debug),
        opt_level: if matches!(config_arc.build_mode, BuildMode::Release) {
            2
        } else {
            0
        },
        target_api_version: config_arc.sdk_config.compile_api_version,
        merge_abc: true,
        cache_path: None,
    });
    for (module, _manifest, loader_json) in &loader_jsons {
        let chunk_dir = PathBuf::from(loader_json.ace_module_build.as_str());
        let modules_abc = module
            .src_path
            .join("build/default/intermediates/ets/modules.abc");
        let abc_modules: Vec<AbcModule> = collect_js_chunks(&chunk_dir)?;
        if !abc_modules.is_empty() {
            abc_compiler
                .compile(abc_modules, &modules_abc)
                .map_err(|e| anyhow::anyhow!("ABC compile failed: {}", e))?;
        }
    }
    progress.done();

    // Step 6: Packaging
    progress.step(6, 6, "Packaging");
    let packager = Packager::new(config_arc.clone());
    let mut output_paths = Vec::new();
    for (module, _manifest, _loader_json) in &loader_jsons {
        let module_json = module
            .src_path
            .join("build/default/intermediates/res/module.json");
        let resources_index = module
            .src_path
            .join("build/default/intermediates/res/resources.index");
        let modules_abc = module
            .src_path
            .join("build/default/intermediates/ets/modules.abc");
        let ext = match &config_arc.output_target {
            OutputTarget::Hsp => "hsp",
            OutputTarget::Har => "har",
            _ => "hap",
        };
        let output = project_root.join(format!(
            "build/default/outputs/default/{}.{}",
            module.name, ext
        ));

        std::fs::create_dir_all(output.parent().expect("output path has no parent"))?;

        if module_json.exists() && resources_index.exists() && modules_abc.exists() {
            let out = packager
                .pack_hap(PackInput {
                    module_name: module.name.clone(),
                    abc_path: modules_abc,
                    resources_index,
                    module_json,
                    native_libs: vec![],
                    output_path: output.clone(),
                })
                .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;
            output_paths.push(out);
        }
    }
    progress.done();

    let elapsed = start.elapsed();
    if output_paths.is_empty() {
        println!(
            "\nBuild succeeded ({:.1}s) — no output files produced (missing intermediates)",
            elapsed.as_secs_f32()
        );
    } else {
        for path in &output_paths {
            println!(
                "\nBuild succeeded: {} ({:.1}s)",
                path.display(),
                elapsed.as_secs_f32()
            );
        }
    }
    Ok(())
}

fn read_module_json5(path: &std::path::Path) -> Result<ModuleJson5> {
    if !path.exists() {
        return Ok(ModuleJson5 {
            module: ModuleJson5Module {
                name: "entry".to_string(),
                type_: "entry".to_string(),
                main_element: None,
                device_types: vec!["phone".to_string()],
                pages: None,
                abilities: vec![],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
            },
        });
    }
    let content = fs_err::read_to_string(path)?;
    json5::from_str(&content).context("Failed to parse module.json5")
}

fn collect_js_chunks(chunk_dir: &std::path::Path) -> Result<Vec<AbcModule>> {
    let mut modules = Vec::new();
    if !chunk_dir.exists() {
        return Ok(modules);
    }
    for entry in walkdir::WalkDir::new(chunk_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
            let record_name = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("chunk")
                .to_string();
            let source = fs_err::read_to_string(entry.path())?;
            modules.push(AbcModule {
                record_name,
                source,
                source_map: None,
            });
        }
    }
    Ok(modules)
}
