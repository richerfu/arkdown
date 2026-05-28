use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;

use arkdown::Bundler;
use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};
use arkdown_config::load_config;
use arkdown_core::{ArkdownPlugin, BuildMode, ModuleJson5, ModuleJson5Module, OutputTarget};
use arkdown_pack::{HarPackInput, PackAbility, PackInput, Packager};
use arkdown_plugin_chunk_wrapper::ChunkWrapperPlugin;
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::EtsTransformPlugin;
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
use arkdown_profile::ProfileProcessor;
use arkdown_resolver::ResolverConfig;
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use arkdown_sign::HapSigner;

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

    // Step 2: Resource compilation + Profile processing.
    // Resources must be compiled first so their numeric IDs (iconId, labelId,
    // descriptionId) can be embedded in module.json by write_module_json.
    progress.step(2, 6, "Processing profiles");
    let processor = ProfileProcessor::new(config_arc.clone());
    let mut loader_jsons = Vec::new();
    for module in &config_arc.modules {
        // --- Resource compilation (precedes profile so IDs are available) ---
        let resource_compiler = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config_arc.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: matches!(config_arc.build_mode, BuildMode::Debug),
        });
        let res_out = module.src_path.join("build/default/intermediates/res");
        // Include AppScope/resources so app-level icon/label IDs are assigned.
        let mut res_dirs: Vec<PathBuf> = vec![];
        let app_scope_res = project_root.join("AppScope/resources");
        if app_scope_res.exists() {
            res_dirs.push(app_scope_res);
        }
        let module_res_dir = module.src_path.join("src/main/resources");
        if module_res_dir.exists() {
            res_dirs.push(module_res_dir);
        }
        let resource_ids: HashMap<String, u32> = if !res_dirs.is_empty() {
            match resource_compiler.compile(ResourceCompileInput {
                resources_dirs: res_dirs,
                output_dir: res_out.clone(),
            }) {
                Ok(out) => out.resource_ids,
                Err(e) => {
                    eprintln!("[warn] resource compilation failed: {e}");
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        // --- Profile processing ---
        let module_json5_path = module.src_path.join("src/main/module.json5");
        let module_json5 = read_module_json5(&module_json5_path)?;
        let (manifest, loader_json) = processor
            .process_module(module, &module_json5)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Write merged module.json; pass resource_ids so iconId/labelId/descriptionId
        // are resolved from the just-compiled resources.
        let module_json_out = res_out.join("module.json");
        processor
            .write_module_json(&manifest, &module_json_out, Some(&resource_ids))
            .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;

        let loader_json_out = module
            .src_path
            .join("build/default/intermediates/loader/default/loader.json");
        processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json failed: {}", e))?;

        loader_jsons.push((module.clone(), manifest, loader_json));
    }
    progress.done();

    // Step 3: Bundling
    progress.step(3, 6, "Bundling");
    let mut bundle_outs = Vec::new();
    for (module, _manifest, loader_json) in &loader_jsons {
        // Clean stale .js files from previous builds so that collect_js_chunks_from_output
        // (which uses the explicit file list) is the authoritative source, and any leftover
        // files from a prior run do not sneak into the ABC compiler.
        let chunk_dir_pre = PathBuf::from(loader_json.ace_module_build.as_str());
        if chunk_dir_pre.exists() {
            for entry in walkdir::WalkDir::new(&chunk_dir_pre)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        // Bundling
        let plugins: Vec<Arc<dyn ArkdownPlugin>> = vec![
            // 1. ETS transform — must be first, desugars .ets before any other plugin sees it
            Arc::new(EtsTransformPlugin::new(config_arc.clone())),
            // 2. OHOS module resolver — maps specifiers to OHM URLs / local paths
            Arc::new(OhosResolvePlugin::new(
                ResolverConfig {
                    project_root: project_root.clone(),
                    sdk_api_dirs: vec![],
                    sdk_kits_dirs: vec![],
                    pkg_name_to_pkg_info: HashMap::new(),
                    use_normalized_ohm_url: config_arc
                        .product
                        .build_option
                        .strict_mode
                        .use_normalized_ohm_url,
                    case_sensitive_check: config_arc
                        .product
                        .build_option
                        .strict_mode
                        .case_sensitive_check,
                    oh_modules_roots: vec![
                        module.src_path.join("oh_modules"),
                        project_root.join("oh_modules"),
                    ],
                    module_src_roots: vec![module.src_path.join("src/main/ets")],
                },
                project_root.clone(),
                config_arc
                    .product
                    .build_option
                    .strict_mode
                    .case_sensitive_check,
            )),
            // 3. Module metadata — annotates each module with OHOS package context
            Arc::new(ModuleMetaPlugin::new(
                project_root.clone(),
                module.name.clone(),
            )),
            // 4. JSON5 import support
            Arc::new(Json5Plugin),
            // 5. Duplicate filename check
            Arc::new(DuplicateCheckPlugin),
            // 6. IIFE chunk wrapper — MUST be last, transforms code output
            Arc::new(ChunkWrapperPlugin::new(loader_json.support_chunks)),
        ];
        let bundler = Bundler::new((*config_arc).clone(), plugins);
        let bundle_out = bundler
            .bundle(loader_json)
            .await
            .map_err(|e| anyhow::anyhow!("Bundle failed: {}", e))?;
        bundle_outs.push(bundle_out);
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
    for ((module, _manifest, loader_json), bundle_out) in
        loader_jsons.iter().zip(bundle_outs.iter())
    {
        let modules_abc = module
            .src_path
            .join("build/default/intermediates/ets/modules.abc");
        // Build record name map: chunk_stem → OHM URL module path.
        // Format matches es2abc/hvigorw: "{bundleName}/{moduleName}/{pathFromEts}"
        // e.g. "com.example.arkdown.test/entry/ets/entryability/EntryAbility"
        // The ABC class is "L{record_name};" (no src/main, no & wrappers).
        let bundle_name = &config_arc.product.bundle_name;
        let record_name_map: std::collections::HashMap<String, String> = loader_json
            .compile_entry
            .iter()
            .filter_map(|entry_path| {
                let path = std::path::Path::new(entry_path);
                let stem = path.file_stem()?.to_str()?.to_string();
                // entry_path like "./ets/entryability/EntryAbility.ets"
                let stripped = entry_path
                    .trim_start_matches("./")
                    .trim_end_matches(".ets")
                    .trim_end_matches(".ts")
                    .trim_end_matches(".js");
                let record_name =
                    format!("{}/{}/{}", bundle_name, loader_json.module_name, stripped);
                Some((stem, record_name))
            })
            .collect();
        // Use the explicit chunk file list from bundle_out rather than scanning the
        // output directory — this prevents stale files from prior runs from being
        // picked up and producing incorrect ABC record entries.
        let abc_modules: Vec<AbcModule> =
            collect_js_chunks_from_output(&bundle_out.chunk_files, &record_name_map)?;
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
    for (module, manifest, _loader_json) in &loader_jsons {
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

        let out = match &config_arc.output_target {
            OutputTarget::Har => {
                // HAR: package ETS sources rather than compiled ABC
                let source_dir = module.src_path.join("src/main/ets");
                let oh_package_json = module.src_path.join("oh-package.json5");
                if source_dir.exists() {
                    Some(
                        packager
                            .pack_har(HarPackInput {
                                module_name: module.name.clone(),
                                source_dir,
                                resources_dir: module.src_path.join("src/main/resources"),
                                oh_package_json,
                                output_path: output.clone(),
                            })
                            .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?,
                    )
                } else {
                    None
                }
            }
            _ => {
                // HAP / HSP: need module.json and modules.abc; resources.index is optional
                if module_json.exists() && modules_abc.exists() {
                    // Create an empty resources.index placeholder if resources were not compiled
                    if !resources_index.exists() {
                        if let Some(parent) = resources_index.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&resources_index, &[])?;
                    }
                    {
                        // Source maps produced by the bundler (may not exist yet)
                        let source_maps_path = {
                            let p = module.src_path.join(
                                "build/default/intermediates/loader_out/default/ets/sourceMaps.map",
                            );
                            if p.exists() {
                                Some(p)
                            } else {
                                None
                            }
                        };
                        // Abilities from the processed manifest
                        let abilities: Vec<PackAbility> = manifest
                            .abilities
                            .iter()
                            .map(|a| PackAbility {
                                name: a.name.clone(),
                                label: a.label.clone(),
                            })
                            .collect();
                        Some(
                            packager
                                .pack_hap(PackInput {
                                    module_name: module.name.clone(),
                                    abc_path: modules_abc,
                                    resources_index,
                                    module_json,
                                    native_libs: vec![],
                                    output_path: output.clone(),
                                    resources_src_dir: Some(
                                        module.src_path.join("src/main/resources"),
                                    ),
                                    bundle_version_code: 1000000,
                                    bundle_version_name: "1.0.0".to_string(),
                                    bundle_type: "app".to_string(),
                                    source_maps_path,
                                    abilities,
                                })
                                .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?,
                        )
                    }
                } else {
                    None
                }
            }
        };
        if let Some(path) = out {
            // Sign the HAP/HSP in-place (skip for HAR targets or if no signing config)
            if !matches!(&config_arc.output_target, OutputTarget::Har) {
                HapSigner::sign_or_skip(
                    &path,
                    &path,
                    config_arc.product.signing_config.as_ref(),
                    config_arc.sdk_config.compile_api_version,
                )
                .map_err(|e| anyhow::anyhow!("Sign failed: {}", e))?;
            }
            output_paths.push(path);
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
                description: None,
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

/// Build [`AbcModule`] entries from the explicit file list produced by the bundler.
///
/// Using `bundle_out.chunk_files` rather than scanning the output directory
/// guarantees that stale `.js` files left over from previous builds never
/// end up in the ABC compiler input.
fn collect_js_chunks_from_output(
    chunk_files: &[(String, std::path::PathBuf)],
    record_name_map: &std::collections::HashMap<String, String>,
) -> Result<Vec<AbcModule>> {
    let mut modules = Vec::new();
    for (chunk_name, path) in chunk_files {
        if path.extension().and_then(|e| e.to_str()) != Some("js") {
            continue;
        }
        // Use OHOS module path as record name if available; fall back to chunk name.
        let record_name = record_name_map
            .get(chunk_name.as_str())
            .cloned()
            .unwrap_or_else(|| chunk_name.clone());
        let source = fs_err::read_to_string(path)?;
        modules.push(AbcModule {
            record_name,
            source,
            source_map: None,
        });
    }
    Ok(modules)
}
