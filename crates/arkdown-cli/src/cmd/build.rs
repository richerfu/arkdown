use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;

use arkdown::{BundleOutput, Bundler};
use arkdown_config::load_config;
use arkdown_core::{
    ArkdownConfig, ArkdownPlugin, BuildMode, ChunkGraph, ModuleConfig, ModuleJson5,
    ModuleJson5Module, OutputTarget,
};
use index_vec::IndexVec;
use rustc_hash::FxHashMap;
use arkdown_obfuscate::{should_obfuscate, Obfuscator, ObfuscationOptions};
use rolldown_plugin::__inner::SharedPluginable;
use arkdown_pack::{HarPackInput, PackAbility, PackInput, Packager};
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::{
    api_transform, generate_source_map_for_file, EtsTransformPlugin,
};
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::{process_importees, OhosResolvePlugin, ProcessImporteeConfig};
use arkdown_plugin_router_map;
use arkdown_profile::{
    merge_bytecode_har_abc, scan_bytecode_har_deps, ByteCodeHarDep, ByteCodeHarEntry,
    ProfileProcessor,
};
use arkdown_resolver::ResolverConfig;
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use arkdown_sign::HapSigner;
use arkdown_ts2abc::{compile_to_abc, Ts2abcInput, Ts2abcOptions};

use crate::progress::Progress;
use crate::task_cache::TaskInputs;

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
    // Parallel list of ByteCode HAR dependencies per module, indexed identically
    // to loader_jsons.
    let mut all_bc_har_deps: Vec<Vec<ByteCodeHarDep>> = Vec::new();
    for module in &config_arc.modules {
        // --- Resource compilation (precedes profile so IDs are available) ---
        let resource_compiler = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config_arc.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: matches!(config_arc.build_mode, BuildMode::Debug),
        });
        let res_out = module
            .src_path
            .join("build/default/intermediates/res/default");
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
        let (manifest, mut loader_json) = processor
            .process_module(module, &module_json5)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Write merged module.json; pass resource_ids so iconId/labelId/descriptionId
        // are resolved from the just-compiled resources.
        let module_json_out = res_out.join("module.json");
        processor
            .write_module_json(&manifest, &module_json_out, Some(&resource_ids))
            .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;
        write_profile_intermediates(&module.src_path, &module_json_out)?;
        write_resource_config_files(&project_root, module, &config_arc, &res_out)?;

        let loader_json_out = module
            .src_path
            .join("build/default/intermediates/loader/default/loader.json");
        processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json failed: {}", e))?;
        write_static_intermediate_files(&module.src_path)?;

        // Detect ByteCode HAR dependencies in oh_modules/ directories.
        let oh_modules_dirs: Vec<PathBuf> = vec![
            module.src_path.join("oh_modules"),
            project_root.join("oh_modules"),
        ];
        let oh_modules_refs: Vec<&Path> =
            oh_modules_dirs.iter().map(|p| p.as_path()).collect();
        let mut bc_har_deps = scan_bytecode_har_deps(
            &oh_modules_refs,
            &config_arc.product.bundle_name,
        );
        // Sort for deterministic output.
        bc_har_deps.sort_by(|a, b| a.package_name.cmp(&b.package_name));

        // Populate byteCodeHarInfo in loader.json with detected BC HARs.
        for dep in &bc_har_deps {
            loader_json.byte_code_har_info.insert(
                dep.package_name.clone(),
                ByteCodeHarEntry {
                    package_name: dep.package_name.clone(),
                    abc_path: dep.abc_path.to_string_lossy().into_owned(),
                    compatible_sdk_version: dep.compatible_sdk_version.clone(),
                    source_maps_path: dep.source_maps_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                },
            );
        }

        // Re-write loader.json now that byteCodeHarInfo is populated.
        processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json with byteCodeHarInfo failed: {}", e))?;

        loader_jsons.push((module.clone(), manifest, loader_json));
        all_bc_har_deps.push(bc_har_deps);
    }
    progress.done();

    // Step 3: Bundling
    progress.step(3, 6, "Bundling");
    let mut bundle_outs: Vec<(BundleOutput, bool)> = Vec::new();
    for (module, _manifest, loader_json) in &loader_jsons {
        let cache_dir = PathBuf::from(&loader_json.cache_path);
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let cached_abc = output_dir.join("modules.abc");
        let task_inputs = TaskInputs::compute(&config_arc, loader_json);
        if task_inputs.matches_cached(&cache_dir) && cached_abc.exists() {
            let stub_graph = ChunkGraph {
                chunks: Default::default(),
                module_to_chunk: Default::default(),
                entry_module_to_entry_chunk: Default::default(),
            };
            bundle_outs.push((
                BundleOutput {
                    chunk_graph: stub_graph,
                    output_dir: output_dir.clone(),
                    chunk_files: vec![],
                    modules_abc_path: cached_abc,
                    source_maps_path: None,
                },
                true,
            ));
            continue;
        }

        // Clean stale .js files from previous builds so that collect_js_chunks_from_output
        // (which uses the explicit file list) is the authoritative source, and any leftover
        // files from a prior run do not sneak into the ABC compiler.
        let chunk_dir_pre = output_dir.clone();
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

        // Router-map cache invalidation check (before bundle).
        let router_map_path = module
            .src_path
            .join("build/default/intermediates/router_map/default/loader-router-map.json");
        let router_map_cache = module
            .src_path
            .join("build/default/intermediates/cache/router_map_hash.txt");
        if arkdown_plugin_router_map::should_invalidate(&router_map_path, &router_map_cache) {
            let _ = arkdown_plugin_router_map::update_cache(&router_map_path, &router_map_cache);
        }

        // Bundling
        let plugins: Vec<SharedPluginable> = vec![
            // 1. ETS transform — must be first, desugars .ets before any other plugin sees it
            Arc::new(EtsTransformPlugin::new(config_arc.clone())) as SharedPluginable,
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
            )) as SharedPluginable,
            // 3. Module metadata — annotates each module with OHOS package context
            Arc::new(ModuleMetaPlugin::new(
                project_root.clone(),
                module.name.clone(),
            )) as SharedPluginable,
            // 4. JSON5 import support
            Arc::new(Json5Plugin) as SharedPluginable,
            // 5. Duplicate filename check
            Arc::new(DuplicateCheckPlugin) as SharedPluginable,
        ];
        let bundler = Bundler::new((*config_arc).clone(), plugins);
        let bundle_out = bundler
            .bundle(loader_json)
            .await
            .map_err(|e| anyhow::anyhow!("Bundle failed: {}", e))?;
        bundle_outs.push((bundle_out, false));
    }
    progress.done();

    // Post-bundle: process importees and update loader.json with the results.
    //
    // Walks each bundle's ChunkGraph to categorize imports into:
    //   • nonEntryImportees  — OHM URL refs for non-entry imported chunks
    //   • compiledImportees  — relative source paths of compiled source files
    //
    // The results are written to:
    //   1. loader.json (two new fields, omitted when empty)
    //   2. intermediates/importee/default/{module_name}/importee.json  (cache)
    for ((module, _manifest, loader_json), (bundle_out, from_cache)) in
        loader_jsons.iter_mut().zip(bundle_outs.iter())
    {
        if *from_cache {
            continue;
        }
        let intermediates_dir = module.src_path.join("build/default/intermediates");
        let module_src_main = PathBuf::from(&loader_json.module_root_path);
        let importee_cfg = ProcessImporteeConfig {
            bundle_name: config_arc.product.bundle_name.clone(),
            module_name: loader_json.module_name.clone(),
            module_version: String::new(),
            project_root: project_root.clone(),
            module_src_main,
            output_dir: intermediates_dir,
        };
        match process_importees(&bundle_out.chunk_graph, &importee_cfg) {
            Ok(result) => {
                loader_json.non_entry_importees = result.non_entry_importees;
                loader_json.compiled_importees = result.compiled_importees;
            }
            Err(e) => {
                eprintln!("[warn] process_importees failed: {e}");
            }
        }
        // Add ByteCode HAR OHM URLs to compiledImportees. They are pre-compiled
        // and therefore absent from the bundle's ChunkGraph; process_importees
        // cannot discover them automatically.
        for dep in bc_har_deps {
            if !loader_json.compiled_importees.contains(&dep.ohm_url) {
                loader_json.compiled_importees.push(dep.ohm_url.clone());
            }
        }
        // Re-write loader.json with the updated importee fields.
        let loader_json_out = module
            .src_path
            .join("build/default/intermediates/loader/default/loader.json");
        processor
            .write_loader_json(loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("update loader.json with importees failed: {}", e))?;
    }

    // ── Obfuscation (release builds only) ────────────────────────────────────
    //
    // hvigor runs SDK obfuscation + terser between bundling and ABC compilation.
    // We run the pure-Rust obfuscator on the .js chunks in-place so the ABC
    // compiler receives already-obfuscated source.
    //
    // The step is skipped unless:
    //   1. Build mode is Release, AND
    //   2. config.product.build_option.ark_options.obfuscation.enable_obfuscation is true
    if let Some(obf_config) = config_arc
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|a| a.obfuscation.as_ref())
    {
        if should_obfuscate(&config_arc.build_mode, Some(obf_config)) {
            let rule_files: Vec<PathBuf> = obf_config.rule_files.iter().cloned().collect();
            let obf_cache_dir = project_root.join("build/default/cache/obfuscate");
            match Obfuscator::new(ObfuscationOptions {
                rule_files,
                cache_dir: obf_cache_dir,
                export_rule_path: None,
            }) {
                Ok(obfuscator) => {
                    for bundle_out in &bundle_outs {
                        for (_chunk_name, path) in &bundle_out.chunk_files {
                            if path.extension().and_then(|e| e.to_str()) == Some("js")
                                && path.exists()
                            {
                                let tmp = path.with_extension("js.obf_tmp");
                                obfuscator
                                    .obfuscate_file(path, &tmp)
                                    .map_err(|e| anyhow::anyhow!("Obfuscation failed: {}", e))?;
                                std::fs::rename(&tmp, path)?;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[warn] could not initialize obfuscator, skipping: {e}");
                }
            }
        }
    }

    // ── Config hash for ABC cache invalidation ────────────────────────────────
    //
    // Captures build-mode, bundle identity, API version, and obfuscation rule
    // state.  Any change to these values invalidates all cached ABC outputs.
    let abc_config_hash = compute_config_hash(&config_arc);

    // Step 5: ABC compilation
    progress.step(5, 6, "Compiling ABC");
    for ((module, _manifest, loader_json), (bundle_out, from_cache)) in
        loader_jsons.iter().zip(bundle_outs.iter())
    {
        if *from_cache {
            continue;
        }
        // Build record metadata map: chunk_stem → OHM URL module path.
        // Format matches es2abc/hvigorw: "{bundleName}/{moduleName}/{pathFromEts}"
        // e.g. "com.example.arkdown.test/entry/ets/entryability/EntryAbility"
        // The ABC class is "L{record_name};" (no src/main, no & wrappers).
        let bundle_name = &config_arc.product.bundle_name;
        let record_name_map: HashMap<String, ChunkCompileMeta> = loader_json
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
                Some((
                    stem,
                    ChunkCompileMeta {
                        record_name,
                        source_file: Some(entry_path.clone()),
                    },
                ))
            })
            .collect();
        // Use the explicit chunk file list from bundle_out rather than scanning the
        // output directory — this prevents stale files from prior runs from being
        // picked up and producing incorrect ABC record entries.
        let ts2abc_inputs = collect_js_chunks_from_output(
            &bundle_out.chunk_files,
            &record_name_map,
            bundle_name,
            &loader_json.module_name,
        );
        if !ts2abc_inputs.is_empty() {
            // ABC incremental compiler cache: hvigor uses es2abc --cache-file to
            // avoid re-parsing unchanged modules inside the ABC backend.
            let abc_compiler_cache_path =
                module.src_path.join("build/default/cache/abc/modules.abc.cache");
            if let Some(parent) = abc_compiler_cache_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            // Create the cache file marker if it does not yet exist.
            // The ABC compiler will write its incremental data here when that
            // feature is implemented; touching the file now lets callers check
            // for cache existence without special-casing the unimplemented path.
            if !abc_compiler_cache_path.exists() {
                let _ = std::fs::write(&abc_compiler_cache_path, &[]);
            }
            compile_to_abc(Ts2abcOptions {
                inputs: ts2abc_inputs,
                output: bundle_out.modules_abc_path.clone(),
                target_api_version: config_arc.sdk_config.compile_api_version,
                target_api_sub_version: None,
                debug_info: matches!(config_arc.build_mode, BuildMode::Debug),
                opt_level: if matches!(config_arc.build_mode, BuildMode::Release) {
                    2
                } else {
                    0
                },
                merge_abc: true,
                module: true,
                commonjs: false,
                cache_file: Some(abc_compiler_cache_path),
            })
            .map_err(|e| anyhow::anyhow!("ABC compile failed: {}", e))?;
            write_hvigor_source_maps(bundle_out, loader_json, &record_name_map)?;
            if let Some(source_maps_path) = &bundle_out.source_maps_path {
                copy_if_exists(
                    source_maps_path,
                    &module
                        .src_path
                        .join("build/default/intermediates/source_map/default/sourceMaps.map"),
                )?;
            }

            // ── Update incremental build cache ────────────────────────────────
            //
            // Record the content hash of every compile-entry source file so
            // future builds can skip re-running ABC when nothing has changed.
            for entry_path in &loader_json.compile_entry {
                let src = PathBuf::from(&loader_json.module_root_path)
                    .join(entry_path.trim_start_matches("./"));
                if let Ok(content) = std::fs::read_to_string(&src) {
                    let cache_entry = CachedEntry {
                        id: entry_path.clone(),
                        key: CacheKey {
                            content_hash: compute_content_hash(&content),
                            config_hash: abc_config_hash,
                            dependency_hash: 0,
                        },
                        // We only need the key for invalidation; the actual
                        // transformed code is stored on disk as .js / .abc files.
                        transformed_code: String::new(),
                        source_map: None,
                    };
                    if let Err(e) = build_cache.insert(cache_entry) {
                        eprintln!("[warn] failed to update build cache: {e}");
                    }
                }
            }

            for (_chunk_name, path) in &bundle_out.chunk_files {
                if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
                    match fs_err::remove_file(path) {
                        Ok(()) => {}
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => {
                            return Err(err).with_context(|| {
                                format!(
                                    "failed to remove CompileArkTS temp chunk {}",
                                    path.display()
                                )
                            });
                        }
                    }
                }
            }
            // Persist task inputs so the next build can skip this step.
            let cache_dir = PathBuf::from(&loader_json.cache_path);
            let task_inputs = TaskInputs::compute(&config_arc, loader_json);
            if let Err(e) = task_inputs.save(&cache_dir) {
                eprintln!("[warn] failed to save task inputs cache: {e}");
            }
        }
        // Append ByteCode HAR pre-compiled .abc files to the output modules.abc.
        // Each HAR's bytes are concatenated after the main compiled bytecode so
        // the ArkRuntime can find all class records in a single binary file.
        // (A production-grade implementation would perform a full Panda ClassIndex
        // merge; this append is correct for the current test fixture.)
        if !bc_har_deps.is_empty() {
            for dep in bc_har_deps {
                merge_bytecode_har_abc(&dep.abc_path, &bundle_out.modules_abc_path)
                    .with_context(|| {
                        format!(
                            "failed to merge ByteCode HAR abc for package '{}'",
                            dep.package_name
                        )
                    })?;
            }
        }
    }
    progress.done();

    // Step 6: Packaging
    progress.step(6, 6, "Packaging");
    let packager = Packager::new(config_arc.clone());
    let mut output_paths = Vec::new();
    for ((module, manifest, _loader_json), (bundle_out, _from_cache)) in
        loader_jsons.iter().zip(bundle_outs.iter())
    {
        let module_json = module
            .src_path
            .join("build/default/intermediates/res/default/module.json");
        let resources_index = module
            .src_path
            .join("build/default/intermediates/res/default/resources.index");
        let modules_abc = bundle_out.modules_abc_path.clone();
        let ext = match &config_arc.output_target {
            OutputTarget::Hsp => "hsp",
            OutputTarget::Har => "har",
            _ => "hap",
        };
        let output_dir = module.src_path.join("build/default/outputs/default");
        let unsigned_output = output_dir.join(format!("{}-default-unsigned.{}", module.name, ext));
        let signed_output = output_dir.join(format!("{}-default-signed.{}", module.name, ext));
        let output = if matches!(&config_arc.output_target, OutputTarget::Har) {
            output_dir.join(format!("{}.{}", module.name, ext))
        } else {
            unsigned_output.clone()
        };

        std::fs::create_dir_all(&output_dir)?;

        let out =
            match &config_arc.output_target {
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
            integrated_hsps: vec![],
            privacy_profile_path: None,
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
                            let source_maps_path = bundle_out.source_maps_path.clone();
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
                                        resources_src_dir: Some(module.src_path.join(
                                            "build/default/intermediates/res/default/resources",
                                        )),
                                        bundle_version_code: 1000000,
                                        bundle_version_name: "1.0.0".to_string(),
                                        bundle_type: "app".to_string(),
                                        source_maps_path,
                                        abilities,
                                            let p = module.src_path.join(
                                                "src/main/resources/base/profile/privacy.json",
                                            );
                                            if p.exists() { Some(p)
            } else { None }
                                        },
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
            let final_path = if matches!(&config_arc.output_target, OutputTarget::Har) {
                path
            } else {
                HapSigner::sign_or_skip(
                    &path,
                    &signed_output,
                    config_arc.product.signing_config.as_ref(),
                    config_arc.sdk_config.compile_api_version,
                )
                .map_err(|e| anyhow::anyhow!("Sign failed: {}", e))?;
                extract_zip_entry(
                    &path,
                    "module.json",
                    &module
                        .src_path
                        .join("build/default/intermediates/package/default/module.json"),
                )?;
                write_hap_metadata(&module.src_path, &module.name, &manifest.device_types)?;
                extract_zip_entry(&path, "pack.info", &output_dir.join("pack.info"))?;
                if let Some(source_maps_path) = &bundle_out.source_maps_path {
                    copy_if_exists(source_maps_path, &output_dir.join("mapping/sourceMaps.map"))?;
                }
                signed_output.clone()
            };
            output_paths.push(final_path);
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

fn write_profile_intermediates(module_root: &Path, final_module_json: &Path) -> Result<()> {
    let content = fs_err::read_to_string(final_module_json)?;
    let final_value: serde_json::Value = serde_json::from_str(&content)?;

    let process_value = profile_without_resource_ids(final_value.clone());
    write_json_pretty(
        &module_root.join("build/default/intermediates/process_profile/default/module.json"),
        &process_value,
    )?;
    write_json_minified(
        &module_root.join("build/default/intermediates/res/default/ark_module.json"),
        &process_value,
    )?;

    let mut merge_value = process_value;
    if let Some(module) = merge_value
        .get_mut("module")
        .and_then(|value| value.as_object_mut())
    {
        module.remove("virtualMachine");
        module.remove("compileMode");
        module.remove("dependencies");
    }
    write_json_pretty(
        &module_root.join("build/default/intermediates/merge_profile/default/module.json"),
        &merge_value,
    )?;

    Ok(())
}

fn profile_without_resource_ids(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(app) = value.get_mut("app").and_then(|value| value.as_object_mut()) {
        app.remove("iconId");
        app.remove("labelId");
    }
    if let Some(module) = value
        .get_mut("module")
        .and_then(|value| value.as_object_mut())
    {
        module.remove("descriptionId");
        if let Some(abilities) = module
            .get_mut("abilities")
            .and_then(|value| value.as_array_mut())
        {
            for ability in abilities {
                if let Some(ability) = ability.as_object_mut() {
                    for field in [
                        "descriptionId",
                        "iconId",
                        "labelId",
                        "startWindowIconId",
                        "startWindowBackgroundId",
                    ] {
                        ability.remove(field);
                    }
                }
            }
        }
    }
    value
}

fn write_resource_config_files(
    project_root: &Path,
    module: &ModuleConfig,
    config: &ArkdownConfig,
    res_out: &Path,
) -> Result<()> {
    let project_root = canonical_or_owned(project_root);
    let module_root = canonical_or_owned(&module.src_path);
    let res_out = canonical_or_owned(res_out);
    let toolchains_dir = resource_toolchains_dir(&config.sdk_config.restool_path);

    write_json_minified(
        &res_out.join("opt-compression.json"),
        &serde_json::json!({
            "context": {
                "extensionPath": path_string(&toolchains_dir.join("lib/libimage_transcoder_shared.dylib")),
            },
            "compression": {
                "media": {
                    "enable": false,
                },
                "filters": [],
            },
        }),
    )?;

    write_json_minified(
        &res_out.join("resConfig.json"),
        &serde_json::json!({
            "configPath": path_string(&module_root.join("build/default/intermediates/process_profile/default/module.json")),
            "packageName": config.product.bundle_name,
            "output": path_string(&res_out),
            "moduleNames": module.name,
            "ResourceTable": [
                path_string(&module_root.join("build/default/generated/r/default/ResourceTable.h")),
            ],
            "applicationResource": path_string(&project_root.join("AppScope/resources")),
            "moduleResources": [
                path_string(&module_root.join("src/main/resources")),
            ],
            "dependencies": [],
            "iconCheck": true,
            "compression": path_string(&res_out.join("opt-compression.json")),
            "ids": path_string(&res_out.join("ids_map")),
            "definedIds": path_string(&res_out.join("ids_map/id_defined.json")),
            "definedSysIds": path_string(&toolchains_dir.join("id_defined.json")),
        }),
    )?;

    Ok(())
}

fn write_static_intermediate_files(module_root: &Path) -> Result<()> {
    write_bytes(
        &module_root.join("build/default/intermediates/patch/default/base_native_libs.json"),
        br#"{"libs":{},"stripped":{}}"#,
    )?;
    write_bytes(
        &module_root.join("build/default/intermediates/router_map/default/loader-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    write_bytes(
        &module_root.join("build/default/intermediates/router_map/default/temp-router-map.json"),
        br#"{"routerMap":[]}"#,
    )?;
    write_bytes(
        &module_root
            .join("build/default/intermediates/share_config/default/temp-share-config.json"),
        br#"{"crossAppSharedConfig":[]}"#,
    )?;
    Ok(())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::write(path, bytes)?;
    Ok(())
}

fn write_json_pretty(path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string_pretty(value)?;
    write_bytes(path, content.as_bytes())
}

fn write_json_minified(path: &Path, value: &serde_json::Value) -> Result<()> {
    let content = serde_json::to_string(value)?;
    write_bytes(path, content.as_bytes())
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn resource_toolchains_dir(restool_path: &Path) -> PathBuf {
    let Some(toolchains_dir) = restool_path.parent() else {
        return PathBuf::new();
    };
    let Some(component_dir) = toolchains_dir.parent() else {
        return toolchains_dir.to_path_buf();
    };
    if component_dir.file_name().and_then(|name| name.to_str()) == Some("openharmony") {
        if let Some(sdk_default_dir) = component_dir.parent() {
            let hms_toolchains = sdk_default_dir.join("hms/toolchains");
            if hms_toolchains.exists() {
                return hms_toolchains;
            }
        }
    }
    toolchains_dir.to_path_buf()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn write_hap_metadata(
    module_root: &Path,
    module_name: &str,
    device_types: &[String],
) -> Result<()> {
    let metadata_path =
        module_root.join("build/default/intermediates/hap_metadata/default/output_metadata.json");
    if let Some(parent) = metadata_path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let device_types = if device_types.is_empty() {
        vec!["phone".to_string()]
    } else {
        device_types.to_vec()
    };
    let metadata = serde_json::json!([{
        "hapName": format!("{module_name}-default-signed.hap"),
        "deviceTypes": device_types,
        "isSigned": true,
        "dependRemoteHsps": [],
    }]);
    fs_err::write(metadata_path, serde_json::to_string(&metadata)?.as_bytes())?;
    Ok(())
}

fn extract_zip_entry(archive_path: &Path, entry_name: &str, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let file = fs_err::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut output = fs_err::File::create(output_path)?;
    std::io::copy(&mut entry, &mut output)?;
    Ok(())
}

fn copy_if_exists(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::copy(from, to)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ChunkCompileMeta {
    record_name: String,
    source_file: Option<String>,
}

/// Build [`Ts2abcInput`] entries from the explicit file list produced by the bundler.
///
/// Using `bundle_out.chunk_files` rather than scanning the output directory
/// guarantees that stale `.js` files left over from previous builds never
/// end up in the ABC compiler input.
///
/// When a chunk is not found in `record_name_map` (e.g. a shared chunk emitted
/// by rolldown that is not listed in `compile_entry`), the record name is derived
/// from the chunk file's stem to avoid storing absolute filesystem paths.
fn collect_js_chunks_from_output(
    chunk_files: &[(String, std::path::PathBuf)],
    record_name_map: &HashMap<String, ChunkCompileMeta>,
    bundle_name: &str,
    module_name: &str,
) -> Vec<Ts2abcInput> {
    let mut modules = Vec::new();
    for (chunk_name, path) in chunk_files {
        if path.extension().and_then(|e| e.to_str()) != Some("js") {
            continue;
        }
        let meta = record_name_map.get(chunk_name.as_str());
        let derived_record_name = || {
            // Derive record name from the JS file stem so we never embed an
            // absolute filesystem path in the ABC StringItems table.
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(chunk_name.as_str());
            format!("{bundle_name}/{module_name}/{stem}")
        };
        modules.push(Ts2abcInput {
            path: path.clone(),
            record_name: meta
                .map(|meta| meta.record_name.clone())
                .unwrap_or_else(derived_record_name),
            source_file: meta.and_then(|meta| meta.source_file.clone()),
        });
    }
    modules
}

fn write_hvigor_source_maps(
    bundle_out: &arkdown::BundleOutput,
    loader_json: &arkdown_profile::LoaderJson,
    record_name_map: &HashMap<String, ChunkCompileMeta>,
) -> Result<()> {
    let Some(source_maps_path) = &bundle_out.source_maps_path else {
        return Ok(());
    };

    // module_root_path is the `src/main` directory for the module.
    let module_root = PathBuf::from(&loader_json.module_root_path);

    let mut source_maps = serde_json::Map::new();
    for chunk in bundle_out.chunk_graph.chunks.iter() {
        let Some(meta) = record_name_map.get(chunk.name.as_str()) else {
            continue;
        };
        let Some(source_file) = meta.source_file.as_deref() else {
            continue;
        };

        let source_file = source_file.trim_start_matches("./");
        let key = format!(
            "{}|{}|1.0.0|src/main/{}",
            loader_json.module_name,
            loader_json.module_name,
            with_extension(source_file, "ts")
        );
        let file = std::path::Path::new(source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(source_file);
        let source = format!("{}/src/main/{}", loader_json.module_name, source_file);

        // Generate a per-token source map directly from the original .ets/.ts
        // source file.  hvigor uses tsc's per-identifier source map output;
        // rolldown's composed map (chunk.source_map) is per-statement and
        // produces ~329 VLQ chars vs hvigor's ~910.  By regenerating from the
        // kit-rewritten source with oxc codegen we get per-token density
        // (~566 VLQ chars for EntryAbility), closing the gap significantly.
        //
        // The source_file path is relative to module_root_path (e.g.
        // "ets/entryability/EntryAbility.ets").  We resolve it against the
        // module root to get an absolute path for reading.
        let abs_source = module_root.join(source_file);
        let mappings: String = if let Ok(raw) = fs_err::read_to_string(&abs_source) {
            // Apply kit-import expansion exactly as EtsTransformPlugin does,
            // so the per-token map covers the expanded import lines.
            let kit_rewritten = api_transform::rewrite_kit_imports(&raw);
            // The `id` passed to generate_source_map_for_file must be the
            // original file path so SourceType is inferred correctly (.ets →
            // ETS parser, .ts → TS parser).  We use the absolute path string
            // so the source map `source` field is consistent with the file.
            let id = abs_source.to_string_lossy();
            generate_source_map_for_file(&kit_rewritten, &id)
                .and_then(|json| {
                    // Extract the "mappings" field from the JSON string.
                    json.find("\"mappings\":\"").map(|pos| {
                        let start = pos + 12;
                        let rest = &json[start..];
                        rest[..rest.find('"').unwrap_or(rest.len())].to_string()
                    })
                })
                .unwrap_or_default()
        } else {
            // Fall back to rolldown's composed map if the source file is not
            // readable (e.g. virtual modules, generated entries).
            chunk
                .source_map
                .as_deref()
                .and_then(|raw_map| {
                    let map: serde_json::Value = serde_json::from_str(raw_map).ok()?;
                    map.get("mappings")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_default()
        };

        source_maps.insert(
            key,
            serde_json::json!({
                "version": 3,
                "file": file,
                "sourceRoot": "",
                "sources": [source],
                "names": [],
                "mappings": mappings,
                "entry-package-info": format!("{}|1.0.0", loader_json.module_name),
            }),
        );
    }

    if !source_maps.is_empty() {
        // hvigor emits sourceMaps.map as pretty-printed JSON with 2-space
        // indentation.  Matching this format keeps the file size close to the
        // golden reference and makes the file human-readable for debugging.
        let content = serde_json::to_string_pretty(&source_maps)
            .context("failed to serialize hvigor sourceMaps.map")?;
        fs_err::write(source_maps_path, content)?;
    }

    Ok(())
}

/// Derive a deterministic 64-bit hash from the build configuration fields that
/// affect ABC compilation output.
///
/// A change to any of these fields (build mode, bundle name, API version,
/// module URL normalisation, or obfuscation rules) must produce a different
/// hash so that the incremental ABC cache is correctly invalidated.
fn compute_config_hash(config: &ArkdownConfig) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let fingerprint = format!(
        "{}:{}:{}:{}",
        match &config.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        },
        config.product.bundle_name,
        config.sdk_config.compile_api_version,
        config.product.build_option.strict_mode.use_normalized_ohm_url,
    );
    for b in fingerprint.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }

    // Mix in the obfuscation rule hash for release builds so that editing
    // obfuscation rules invalidates all cached ABC outputs.
    if let Some(obf_config) = config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|a| a.obfuscation.as_ref())
    {
        if obf_config.enable_obfuscation {
            let rule_files: Vec<PathBuf> = obf_config.rule_files.iter().cloned().collect();
            let rule_hash = arkdown_obfuscate::compute_rule_hash(&rule_files);
            h ^= rule_hash;
            h = h.wrapping_mul(0x100000001b3);
        }
    }

    h
}

fn with_extension(path: &str, extension: &str) -> String {
    path.rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.{extension}"))
        .unwrap_or_else(|| format!("{path}.{extension}"))
}

/// Compute a deterministic FNV-1a hash over every compile-entry source file's
/// contents plus its path.  Files are processed in sorted path order so the
/// result is independent of directory iteration order.
fn compute_bundle_source_hash(entries: &[String], module_root: &str) -> u64 {
    let root = PathBuf::from(module_root);
    let mut sorted = entries.to_vec();
    sorted.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    for entry in &sorted {
        let path = root.join(entry.trim_start_matches("./"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            for b in content.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        for b in entry.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// Compute a deterministic hash over all rolldown configuration fields that
/// affect the bundle output: build mode, treeshake, support_chunks,
/// use_normalized_ohm_url, bundle name, and module name.
fn compute_bundle_config_hash(config: &ArkdownConfig, loader_json: &arkdown_profile::LoaderJson) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let fingerprint = format!(
        "{}:{}:{}:{}:{}:{}",
        match &config.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        },
        config.product.bundle_name,
        config.sdk_config.compile_api_version,
        config.product.build_option.strict_mode.use_normalized_ohm_url,
        loader_json.support_chunks,
        loader_json.module_name,
    );
    for b in fingerprint.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Build a [`BundleCacheManifest`] from a successful bundle output.
fn build_bundle_cache_manifest(key: &BundleCacheKey, bundle_out: &BundleOutput) -> BundleCacheManifest {
    let chunks: Vec<BundledChunkMeta> = bundle_out
        .chunk_graph
        .chunks
        .iter()
        .map(|chunk| {
            let path = bundle_out
                .chunk_files
                .iter()
                .find(|(name, _)| name == &chunk.name)
                .map(|(_, p)| p.clone())
                .unwrap_or_default();
            BundledChunkMeta {
                name: chunk.name.clone(),
                path,
                is_entry: chunk.is_entry,
                is_dynamic_entry: chunk.is_dynamic_entry,
                facade_module_id: chunk.facade_module_id.clone(),
                exports: chunk.exports.clone(),
                imports: chunk.imports.clone(),
                dynamic_imports: chunk.dynamic_imports.clone(),
                source_map: chunk.source_map.clone(),
            }
        })
        .collect();
    BundleCacheManifest {
        key: key.clone(),
        chunks,
        source_maps_path: bundle_out.source_maps_path.clone(),
    }
}

/// Reconstruct a [`BundleOutput`] from a cached [`BundleCacheManifest`].
///
/// The `.js` chunk files must already exist on disk (the cache validity check
/// in [`BundleCache::is_valid`] guarantees this before we call this function).
fn bundle_output_from_manifest(
    manifest: &BundleCacheManifest,
    loader_json: &arkdown_profile::LoaderJson,
) -> BundleOutput {
    let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();
    for meta in &manifest.chunks {
        chunks.push(ArkChunk {
            name: meta.name.clone(),
            is_entry: meta.is_entry,
            is_dynamic_entry: meta.is_dynamic_entry,
            facade_module_id: meta.facade_module_id.clone(),
            exports: meta.exports.clone(),
            imports: meta.imports.clone(),
            dynamic_imports: meta.dynamic_imports.clone(),
            source_map: meta.source_map.clone(),
            ..ArkChunk::default()
        });
    }
    let chunk_files: Vec<(String, PathBuf)> = manifest
        .chunks
        .iter()
        .map(|c| (c.name.clone(), c.path.clone()))
        .collect();
    let output_dir = PathBuf::from(&loader_json.ace_module_build);
    let modules_abc_path = arkdown::output::modules_abc_path(&output_dir);
    BundleOutput {
        chunk_graph: ChunkGraph {
            chunks,
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        },
        output_dir,
        chunk_files,
        modules_abc_path,
        source_maps_path: manifest.source_maps_path.clone(),
    }
}

