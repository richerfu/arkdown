use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;

use arkdown::{compile_entry::CompileEntryNamer, BundleOutput, Bundler};
use arkdown_abc::AbcReader;
use arkdown_config::load_config;
use arkdown_core::{
    ArkdownConfig, BuildMode, ChunkGraph, ExtensionAbilityEntry, ModuleConfig, ModuleJson5,
    ModuleJson5Module, OhosModuleType, OutputTarget,
};
use arkdown_obfuscate::{should_obfuscate, ObfuscationOptions, Obfuscator};
use arkdown_pack::{HarPackInput, PackAbility, PackInput, Packager};
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::{
    load_system_resource_references, rewrite_static_module_specifiers_with,
    transform::collect_arkui_component_names, EtsTransformPlugin, ResourceReference,
};
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::{
    process_importees, OhosResolvePlugin, ProcessImporteeConfig, ResolvedImportMap,
};
use arkdown_plugin_router_map;
use arkdown_profile::{
    scan_bytecode_har_deps, ByteCodeHarEntry, ByteCodeHarScanEntry, ProfileProcessor,
};
use arkdown_resolver::ResolverConfig;
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use arkdown_sign::HapSigner;
use arkdown_ts2abc::{compile_to_abc, Ts2abcInput, Ts2abcOptions};
use arkdown_watcher::FileWatcher;
use rolldown_plugin::__inner::SharedPluginable;

use crate::progress::Progress;
use crate::task_cache::TaskInputs;

mod compile_arkts_cache;
use compile_arkts_cache::CompileArktsCacheWriter;
mod compile_arkts_handoff;
use compile_arkts_handoff::{
    write_compile_arkts_abc_handoff, CompileArkTsHandoff, CompileArkTsInfo,
};
mod compile_arkts_modules;
use compile_arkts_modules::{
    compile_source_passthrough, module_id_to_source_path, rewrite_compile_module_imports_to_ohm,
    CompileModuleInfo,
};
mod compile_arkts_output;
use compile_arkts_output::{
    collect_unresolved_external_imports, format_unresolved_import_errors, write_hvigor_source_maps,
    ChunkCompileMeta,
};
mod compile_arkts_package_entries;
use compile_arkts_package_entries::{is_package_entry_file, PackageEntryInfos};
mod compile_arkts_arkdata;
use compile_arkts_arkdata::compile_arkdata_pass;
mod compile_arkts_widget;
use compile_arkts_widget::compile_widget_pass;
mod intermediates;
use intermediates::{
    canonical_or_owned, copy_ets_assets_to_loader_out, copy_if_exists, extract_zip_entry,
    path_string, rewrite_package_intermediate_module_json, write_bytes, write_hap_metadata,
    write_json_pretty, write_profile_intermediates, write_resource_config_files,
    write_router_map_intermediates, write_static_intermediate_files,
};
mod module_plan;
use module_plan::{build_modules_for_target, resource_modules_for_build};

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
    /// Watch mode: rebuild on source file changes (M1 — SPEC-023)
    #[arg(long, default_value = "false")]
    pub watch: bool,
    /// Coverage instrumentation mode: prepare build for code-coverage collection (F20)
    /// Enables build infrastructure for coverage but does not yet inject Istanbul counters.
    #[arg(long, default_value = "false")]
    pub coverage: bool,
    /// Preview mode: watch resource directories and trigger resource recompilation (M3).
    /// Implies --watch; focuses change detection on src/main/resources/ for faster
    /// resource-only rebuilds during DevEco IDE preview sessions.
    #[arg(long, default_value = "false")]
    pub preview: bool,
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let watch_mode = args.watch || args.preview; // --preview implies --watch
    let preview_mode = args.preview;
    let coverage_mode = args.coverage;
    let target_str = args.target.clone();
    let mode_str = args.mode.clone();
    let module_filter = args.module.clone();
    let product_filter = args.product.clone();
    let project_root = args
        .project
        .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let start = Instant::now();

    let target = match target_str.as_str() {
        "hsp" => OutputTarget::Hsp,
        "har" => OutputTarget::Har,
        _ => OutputTarget::Hap,
    };
    let mode = match mode_str.as_str() {
        "release" => BuildMode::Release,
        _ => BuildMode::Debug,
    };

    let progress = Progress::new();

    // Step 1: Config
    progress.step(1, 6, "Parsing config");
    let config = load_config(&project_root, mode, target, product_filter.as_deref(), None)
        .context("Failed to parse project config")?;
    if let Some(module_name) = module_filter.as_deref() {
        if !config
            .modules
            .iter()
            .any(|module| module.name == module_name)
        {
            return Err(anyhow::anyhow!("module not found: {module_name}"));
        }
    }
    progress.done();

    let config_arc = Arc::new(config);
    let build_modules = build_modules_for_target(config_arc.as_ref(), module_filter.as_deref());

    // F20: Coverage instrumentation mode.
    // When --coverage is set, the build is prepared for code coverage collection.
    // Full Istanbul-style counter injection is not yet implemented; this flag
    // establishes the build infrastructure that would be used by a coverage runner.
    if coverage_mode {
        tracing::info!(
            "coverage mode enabled — build prepared for code coverage (F20); \
             Istanbul counter injection not yet implemented"
        );
    }

    // F24: Custom compile plugins (hvigorfile.ts equivalent).
    // If a `hvigorfile.ts` exists at the project root, warn the user that
    // TypeScript plugin execution is not supported.  Users can instead place
    // a `arkdown.config.json` file at the project root with build overrides.
    //
    // For projects that only use hvigorfile.ts to configure standard hvigor
    // options (not custom rollup plugins), this warning can be ignored.
    let hvigorfile_path = project_root.join("hvigorfile.ts");
    if hvigorfile_path.exists() {
        tracing::warn!(
            path = %hvigorfile_path.display(),
            "F24: hvigorfile.ts detected — TypeScript plugin execution is not supported; \
             use arkdown.config.json for build customizations that arkdown supports"
        );
    }

    // Step 2: Resource compilation + Profile processing.
    // Resources must be compiled first so their numeric IDs (iconId, labelId,
    // descriptionId) can be embedded in module.json by write_module_json.
    progress.step(2, 6, "Processing profiles");
    let processor = ProfileProcessor::new(config_arc.clone());
    let mut loader_jsons = Vec::new();
    // Parallel list of ByteCode HAR dependencies per module, indexed identically
    // to loader_jsons.
    let mut all_bc_har_deps: Vec<Vec<ByteCodeHarScanEntry>> = Vec::new();
    for module in build_modules {
        // --- Resource compilation (precedes profile so IDs are available) ---
        let resource_compiler = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config_arc.product.bundle_name.clone(),
            module_name: module.name.clone(),
            module_names: config_arc
                .modules
                .iter()
                .map(|module| module.name.clone())
                .collect(),
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
        for resource_module in resource_modules_for_build(config_arc.as_ref(), module) {
            let module_res_dir = resource_module.src_path.join("src/main/resources");
            if module_res_dir.exists() && !res_dirs.contains(&module_res_dir) {
                res_dirs.push(module_res_dir);
            }
        }
        let mut resource_refs = FxHashMap::default();
        let resource_ids: FxHashMap<String, u32> = if !res_dirs.is_empty() {
            match resource_compiler.compile(ResourceCompileInput {
                resources_dirs: res_dirs,
                output_dir: res_out.clone(),
            }) {
                Ok(out) => {
                    resource_refs = out
                        .resource_infos
                        .into_iter()
                        .map(|(name, info)| {
                            (
                                format!("app.{name}"),
                                ResourceReference {
                                    id: info.id,
                                    runtime_type: info.runtime_type,
                                    bundle_name: config_arc.product.bundle_name.clone(),
                                    module_name: module.name.clone(),
                                },
                            )
                        })
                        .collect();
                    out.resource_ids
                }
                Err(e) => {
                    tracing::warn!(error = %e, "resource compilation failed");
                    FxHashMap::default()
                }
            }
        } else {
            FxHashMap::default()
        };
        resource_refs.extend(load_system_resource_references(
            &config_arc.sdk_config.ets_loader_path,
            &config_arc.product.bundle_name,
            &module.name,
        )?);

        // --- C3: ProcessCompiledResources — preloaded system SO file list ---
        // Matches hvigor `addPreloadSystemSoFileList()`: when `buildOption.preloadSo`
        // lists system SO files, write them to an intermediate JSON so the packager
        // and installer can propagate the preload metadata.
        let preload_so_paths = &config_arc.product.build_option.preload_so;
        if !preload_so_paths.is_empty() {
            let preload_so_out = module
                .src_path
                .join("build/default/intermediates/preload_so/default");
            std::fs::create_dir_all(&preload_so_out).ok();
            let so_list: Vec<String> = preload_so_paths
                .iter()
                .map(|p| {
                    p.to_str().unwrap_or_default().replace('\\', "/").to_string()
                })
                .collect();
            let json = serde_json::json!({ "soFileList": so_list });
            write_json_pretty(
                &preload_so_out.join("preload_so_list.json"),
                &json,
            )
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to write preload_so_list.json");
            });
        }

        // --- Profile processing ---
        let module_json5_path = module.src_path.join("src/main/module.json5");
        let module_json5 = read_module_json5(&module_json5_path)?;

        // --- Share config (B4): collect crossAppSharedConfig from oh_modules HSPs ---
        let share_out = arkdown_profile::merge_share_config(
            arkdown_profile::ShareConfigInput {
                module_root: &module.src_path,
                self_entries: module_json5.module.cross_app_shared_config.clone(),
            },
        )
        .map_err(|e| anyhow::anyhow!("ProcessShareConfig failed: {}", e))?;
        let share_config_out = module.src_path.join(
            "build/default/intermediates/share_config/default/temp-share-config.json",
        );
        arkdown_profile::write_share_config(&share_out, &share_config_out)
            .map_err(|e| anyhow::anyhow!("write share config failed: {}", e))?;

        // --- Startup config (B5): collect appStartup task configs ---
        let startup_out = arkdown_profile::merge_startup_config(
            arkdown_profile::StartupConfigInput {
                module_root: &module.src_path,
                module_name: &module.name,
                app_startup: module_json5.module.app_startup.clone(),
            },
        )
        .map_err(|e| anyhow::anyhow!("ProcessStartupConfig failed: {}", e))?;
        let startup_config_out = module.src_path.join(
            "build/default/intermediates/startup_config/default/temp-startup-config.json",
        );
        arkdown_profile::write_startup_config(&startup_out, &startup_config_out)
            .map_err(|e| anyhow::anyhow!("write startup config failed: {}", e))?;

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
        let mut bc_har_deps = scan_bytecode_har_deps(&module.src_path, &project_root);
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
                    source_maps_path: dep
                        .source_maps_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    insight_intent: dep
                        .insight_intent
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                },
            );
        }

        // Re-write loader.json now that byteCodeHarInfo is populated.
        processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json with byteCodeHarInfo failed: {}", e))?;
        write_router_map_intermediates(module, &loader_json, &config_arc, &res_out)?;

        loader_jsons.push((module.clone(), manifest, loader_json, resource_refs));
        all_bc_har_deps.push(bc_har_deps);
    }
    progress.done();

    // Step 3: Bundling
    progress.step(3, 6, "Bundling");
    let mut bundle_outs: Vec<(BundleOutput, bool)> = Vec::new();
    let mut compile_arkts_handoffs_by_bundle: Vec<CompileArkTsHandoff> = Vec::new();
    for (module, _manifest, loader_json, resource_refs) in &loader_jsons {
        let cache_dir = PathBuf::from(&loader_json.cache_path);
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let cached_abc = output_dir.join("modules.abc");
        let task_inputs = TaskInputs::compute(&config_arc, loader_json);
        if task_inputs.matches_cached(&cache_dir) && cached_abc.exists() {
            bundle_outs.push((
                BundleOutput {
                    chunk_graph: ChunkGraph::default(),
                    output_dir: output_dir.clone(),
                    chunk_files: vec![],
                    modules_abc_path: cached_abc,
                    source_maps_path: None,
                },
                true,
            ));
            compile_arkts_handoffs_by_bundle.push(CompileArkTsHandoff::default());
            continue;
        }

        // Clean stale bundled .js files from previous builds. hvigor hands the
        // Rollup module graph to CompileArkTS in ESMODULE mode, so these files
        // are only temporary rolldown artifacts for arkdown.
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
        let ohos_resolve_plugin = Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: project_root.clone(),
                sdk_api_dirs: config_arc.sdk_config.ets_api_dirs(),
                sdk_kits_dirs: config_arc.sdk_config.ets_kits_dirs(),
                pkg_name_to_pkg_info: FxHashMap::default(),
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
            },
            project_root.clone(),
            config_arc
                .product
                .build_option
                .strict_mode
                .case_sensitive_check,
        ));
        let plugins: Vec<SharedPluginable> = vec![
            // 1. ETS transform — must be first, desugars .ets before any other plugin sees it
            Arc::new(EtsTransformPlugin::with_resource_references(
                config_arc.clone(),
                resource_refs.clone(),
            )) as SharedPluginable,
            // 2. OHOS module resolver — maps specifiers to OHM URLs / local paths
            ohos_resolve_plugin.clone() as SharedPluginable,
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
        let resolved_imports = ohos_resolve_plugin.resolved_import_map_snapshot();
        let compile_arkts_handoff = write_compile_arkts_cache(
            module,
            loader_json,
            &config_arc,
            &bundle_out.chunk_graph,
            resource_refs,
            &resolved_imports,
        )?;
        bundle_outs.push((bundle_out, false));
        compile_arkts_handoffs_by_bundle.push(compile_arkts_handoff);
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
    for (
        ((module, _manifest, loader_json, _resource_refs), (bundle_out, from_cache)),
        _bc_har_deps,
    ) in loader_jsons
        .iter_mut()
        .zip(bundle_outs.iter())
        .zip(all_bc_har_deps.iter())
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
                tracing::warn!(error = %e, "process_importees failed");
            }
        }
        // F13 — collectImportersPlugin: write reverse importer map.
        // Maps each chunk name to the list of chunk names that import it.
        // Used by the hot-reload delta engine (M2) to compute the minimal set
        // of modules to recompile on a file change.
        // Matches hvigor's collectImportersPlugin moduleParsed hook semantics.
        {
            // Build reverse map: chunk_name → [chunk_names that import it].
            let chunks = &bundle_out.chunk_graph.chunks;
            let mut importer_map: FxHashMap<String, Vec<String>> = FxHashMap::default();
            for (_idx, chunk) in chunks.iter_enumerated() {
                for &import_idx in &chunk.cross_chunk_imports {
                    if let Some(imported) = chunks.get(import_idx) {
                        importer_map
                            .entry(imported.name.clone())
                            .or_default()
                            .push(chunk.name.clone());
                    }
                }
            }
            let out_dir = module
                .src_path
                .join("build/default/intermediates/importer_map/default");
            std::fs::create_dir_all(&out_dir).ok();
            let json = serde_json::to_value(&importer_map).unwrap_or_default();
            write_json_pretty(&out_dir.join("importer_map.json"), &json).unwrap_or_else(
                |e| tracing::warn!(error = %e, "failed to write importer_map.json"),
            );
        }

        // Add ByteCode HAR OHM URLs to compiledImportees. They are pre-compiled
        // and therefore absent from the bundle's ChunkGraph; process_importees
        // cannot discover them automatically.
        // ByteCode HAR OHM URLs not yet populated in loader.json
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
                    for (bundle_out, _) in &bundle_outs {
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
                    tracing::warn!(error = %e, "could not initialize obfuscator, skipping");
                }
            }

            // I3: Generate consumer obfuscation config for HAR publishing.
            // When building a source HAR with obfuscation enabled, write the
            // obfuscation rule files to an intermediate consumer-rules file so
            // that downstream consumers of this HAR know which symbols to keep.
            // Matches hvigor's generateConsumerObConfigFile() behavior.
            if target_str == "har" {
                let consumer_dir =
                    project_root.join("build/default/intermediates/obfuscation/default");
                std::fs::create_dir_all(&consumer_dir).ok();
                let consumer_path = consumer_dir.join("consumer-rules.txt");
                let mut consumer_content = String::new();
                for rule_file in &obf_config.rule_files {
                    if let Ok(content) = std::fs::read_to_string(rule_file) {
                        consumer_content.push_str(&content);
                        consumer_content.push('\n');
                    }
                }
                if let Err(e) = std::fs::write(&consumer_path, &consumer_content) {
                    tracing::warn!(
                        error = %e,
                        "failed to write consumer obfuscation rules"
                    );
                }
            }
        }
    }

    // Step 5: ABC compilation
    progress.step(5, 6, "Compiling ABC");
    for (
        bundle_index,
        (((module, _manifest, loader_json, resource_refs), (bundle_out, from_cache)), bc_har_deps),
    ) in loader_jsons
        .iter()
        .zip(bundle_outs.iter())
        .zip(all_bc_har_deps.iter())
        .enumerate()
    {
        if *from_cache {
            continue;
        }
        let unresolved_imports = collect_unresolved_external_imports(bundle_out);
        if !unresolved_imports.is_empty() {
            return Err(anyhow::anyhow!(
                "ArkTS Compiler Error\n{}",
                format_unresolved_import_errors(&unresolved_imports)
            ));
        }
        // Build record metadata map: chunk_stem → OHM URL module path.
        // Format matches es2abc/hvigorw: "{bundleName}/{moduleName}/{pathFromEts}"
        // e.g. "com.example.arkdown.test/entry/ets/entryability/EntryAbility"
        // The ABC class is "L{record_name};" (no src/main, no & wrappers).
        let namer = CompileEntryNamer::new(&loader_json.compile_entry);
        let record_name_map: FxHashMap<String, ChunkCompileMeta> = loader_json
            .compile_entry
            .iter()
            .enumerate()
            .filter_map(|(index, entry_path)| {
                let path = std::path::Path::new(entry_path);
                path.file_stem()?.to_str()?;
                let chunk_name = namer.chunk_name(entry_path, index);
                Some((
                    chunk_name,
                    ChunkCompileMeta {
                        source_file: Some(entry_path.clone()),
                    },
                ))
            })
            .collect();
        let compile_arkts_handoff = compile_arkts_handoffs_by_bundle
            .get(bundle_index)
            .cloned()
            .unwrap_or_default();
        let ts2abc_inputs = compile_arkts_handoff.inputs;
        if ts2abc_inputs.is_empty() {
            return Err(anyhow::anyhow!(
                "CompileArkTS produced no module inputs; refusing to fall back to bundled chunks because hvigor compiles the Rollup module graph"
            ));
        }
        if !ts2abc_inputs.is_empty() {
            // ABC incremental compiler cache: hvigor uses es2abc --cache-file to
            // avoid re-parsing unchanged modules inside the ABC backend.
            // Pass the *directory* — CacheFile::path_for() appends the filename
            // internally. Previously this erroneously passed the full file path,
            // causing path_for() to produce "…/modules.abc.cache/modules.abc.cache".
            let abc_cache_dir = module.src_path.join("build/default/cache/abc");
            std::fs::create_dir_all(&abc_cache_dir).ok();
            compile_to_abc(Ts2abcOptions {
                inputs: ts2abc_inputs,
                output: bundle_out.modules_abc_path.clone(),
                target_api_version: config_arc.sdk_config.compile_api_version,
                target_api_sub_version: config_arc.product.compatible_sdk_version_stage.clone(),
                debug_info: matches!(config_arc.build_mode, BuildMode::Debug),
                opt_level: if matches!(config_arc.build_mode, BuildMode::Release) {
                    2
                } else {
                    0
                },
                merge_abc: true,
                module: true,
                commonjs: false,
                cache_file: Some(abc_cache_dir),
                npm_module_entry_list: compile_arkts_handoff.npm_entries_path,
            })
            .map_err(|e| anyhow::anyhow!("ABC compile failed: {}", e))?;
            write_hvigor_source_maps(
                bundle_out,
                module,
                loader_json,
                &config_arc,
                &record_name_map,
            )?;
            if let Some(source_maps_path) = &bundle_out.source_maps_path {
                copy_if_exists(
                    source_maps_path,
                    &module
                        .src_path
                        .join("build/default/intermediates/source_map/default/sourceMaps.map"),
                )?;
            }
            copy_ets_assets_to_loader_out(module, loader_json)?;
            compile_widget_pass(module, _manifest, loader_json, &config_arc, resource_refs)?;

            // H13: ArkData compile pass — separate ABC invocation for cloud
            // database schema files using @kit.ArkData decorators.
            compile_arkdata_pass(module, loader_json, &config_arc, resource_refs)?;

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
                tracing::warn!(error = %e, "failed to save task inputs cache");
            }

            // G8: Write compileInfo.json with buildMode to arkCompileCachePath.
            // Matches hvigor ArkCompile::writeBuildMode() which writes this file
            // so that subsequent builds can detect build-mode changes and
            // invalidate the ABC cache when switching debug ↔ release.
            let build_mode_str = match config_arc.build_mode {
                BuildMode::Debug => "debug",
                BuildMode::Release => "release",
            };
            let compile_info_path = cache_dir.join("compileInfo.json");
            let compile_info =
                serde_json::json!({ "buildMode": build_mode_str });
            if let Err(e) = fs_err::write(
                &compile_info_path,
                serde_json::to_string(&compile_info).unwrap_or_default(),
            ) {
                tracing::warn!(
                    path = %compile_info_path.display(),
                    error = %e,
                    "failed to write compileInfo.json"
                );
            }
        }
        // ByteCode HAR ABC integration (SPEC-041).
        //
        // The previous implementation appended HAR `.abc` bytes directly to
        // `modules.abc`.  This was INCORRECT: raw byte concatenation does not
        // update the Panda header, class index, string table, or literal-array
        // index in the merged file; ArkRuntime cannot parse the result.
        //
        // The correct approach (per hvigor and Panda architecture):
        //   * ByteCode HARs remain as SEPARATE `.abc` files on disk.
        //   * `loader_json.byteCodeHarInfo` maps each HAR's package name to its
        //     `.abc` path and SDK metadata.
        //   * ArkRuntime loads each HAR's ABC independently via `byteCodeHarInfo`.
        //
        // SPEC-041 tracks the full E2E implementation (HAP path, sourceMaps,
        // declarationEntry, normalized OHM URL alignment with hvigor).
        // Invalid or missing ABC in a ByteCode HAR dependency is a hard build
        // error — silently ignoring it would produce a broken runtime package.
        if !bc_har_deps.is_empty() {
            for dep in bc_har_deps {
                if !dep.abc_path.exists() {
                    anyhow::bail!(
                        "ByteCode HAR '{}': ABC file not found at '{}'. \
                         Ensure the HAR was built with bytecode output enabled (SPEC-041).",
                        dep.package_name,
                        dep.abc_path.display()
                    );
                }
                let abc_data = std::fs::read(&dep.abc_path)
                    .with_context(|| format!(
                        "ByteCode HAR '{}': failed to read ABC at '{}'",
                        dep.package_name, dep.abc_path.display()
                    ))?;
                AbcReader::validate(&abc_data)
                    .map_err(|e| anyhow::anyhow!(
                        "ByteCode HAR '{}': ABC at '{}' failed validation: {} \
                         (magic, file_size, class/LNP/literal index bounds). \
                         Rebuild the HAR or remove the invalid dependency (SPEC-041).",
                        dep.package_name, dep.abc_path.display(), e
                    ))?;
                tracing::debug!(
                    package = %dep.package_name,
                    abc_path = %dep.abc_path.display(),
                    "ByteCode HAR dep validated; loaded via byteCodeHarInfo (SPEC-041)"
                );
            }
        }
    }
    progress.done();

    // Step 6: Packaging
    progress.step(6, 6, "Packaging");
    let packager = Packager::new(config_arc.clone());
    let mut output_paths = Vec::new();
    for ((module, manifest, _loader_json, _resource_refs), (bundle_out, _from_cache)) in
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

        let out = match &config_arc.output_target {
            OutputTarget::Har => {
                // HAR: package ETS sources rather than compiled ABC
                let source_dir = module.src_path.join("src/main/ets");
                let oh_package_json = module.src_path.join("oh-package.json5");
                if source_dir.exists() {
                    let packed = packager
                        .pack_har(HarPackInput {
                            module_name: module.name.clone(),
                            source_dir: source_dir.clone(),
                            resources_dir: module.src_path.join("src/main/resources"),
                            oh_package_json: oh_package_json.clone(),
                            output_path: output.clone(),
                        })
                        .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;

                    // B7: Generate HAR module-info (har-extend-info.json).
                    // Mirrors hvigor CreateHarModuleInfo → har/har-extend-info.js:
                    // writes metadata about the HAR's exported API surface so
                    // downstream consumers can understand what the HAR provides.
                    let har_info_out = module
                        .src_path
                        .join("build/default/intermediates/har/default/har-extend-info.json");
                    if let Some(parent) = har_info_out.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    // Collect exported .ets source files.
                    let exported_sources: Vec<String> =
                        walkdir::WalkDir::new(&source_dir)
                            .into_iter()
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.file_type().is_file()
                                    && e.path()
                                        .extension()
                                        .and_then(|x| x.to_str())
                                        .map_or(false, |x| x == "ets")
                            })
                            .filter_map(|e| {
                                e.path()
                                    .strip_prefix(&source_dir)
                                    .ok()
                                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                            })
                            .collect();
                    let har_info = serde_json::json!({
                        "moduleName": module.name,
                        "moduleType": "har",
                        "exportedSources": exported_sources,
                        "abilities": manifest.abilities.iter().map(|a| serde_json::json!({
                            "name": a.name,
                            "srcEntry": a.src_entry,
                        })).collect::<Vec<_>>(),
                    });
                    write_json_pretty(&har_info_out, &har_info).unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "failed to write har-extend-info.json");
                    });

                    Some(packed)
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
                        // I5 — moveReleaseMap: source maps are not embedded in
                        // the HAP for release builds (hvigor ArkCompile::moveReleaseMap).
                        // The map is still copied to the outputs/mapping/ directory
                        // below for developer use; it just doesn't end up inside the
                        // signed archive.
                        let raw_source_maps_path =
                            bundle_out.source_maps_path.clone().or_else(|| {
                                let candidate = module.src_path.join(
                                    "build/default/intermediates/loader_out/default/ets/sourceMaps.map",
                                );
                                candidate.exists().then_some(candidate)
                            });
                        let source_maps_path =
                            if matches!(config_arc.build_mode, BuildMode::Release) {
                                None // excluded from HAP in release; moved to mapping/ below
                            } else {
                                raw_source_maps_path.clone()
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
                                    native_libs: collect_native_libs(&project_root, module),
                                    output_path: output.clone(),
                                    resources_src_dir: Some(
                                        module.src_path.join(
                                            "build/default/intermediates/res/default/resources",
                                        ),
                                    ),
                                    bundle_version_code: 1000000,
                                    bundle_version_name: "1.0.0".to_string(),
                                    bundle_type: "app".to_string(),
                                    source_maps_path,
                                    abilities,
                                    integrated_hsps: collect_integrated_hsps(
                                        module,
                                        &project_root,
                                        &config_arc.product.bundle_name,
                                        1000000,
                                    ),
                                    privacy_profile_path: {
                                        let p = module
                                            .src_path
                                            .join("src/main/resources/base/profile/privacy.json");
                                        if p.exists() {
                                            Some(p)
                                        } else {
                                            None
                                        }
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
                // L3: HAR signing (optional).
                // When a signing config is present, sign the HAR archive using
                // the same HAP signing pipeline (both are ZIP-based archives).
                // Matches hvigor's optional har-sign-command-build.js behavior.
                if config_arc.product.signing_config.is_some() {
                    HapSigner::sign_or_skip(
                        &path,
                        &signed_output,
                        config_arc.product.signing_config.as_ref(),
                        config_arc.sdk_config.compile_api_version,
                    )
                    .map_err(|e| anyhow::anyhow!("HAR sign failed: {}", e))?;
                    signed_output.clone()
                } else {
                    path
                }
            } else {
                HapSigner::sign_or_skip(
                    &path,
                    &signed_output,
                    config_arc.product.signing_config.as_ref(),
                    config_arc.sdk_config.compile_api_version,
                )
                .map_err(|e| anyhow::anyhow!("Sign failed: {}", e))?;
                let package_module_json = module
                    .src_path
                    .join("build/default/intermediates/package/default/module.json");
                extract_zip_entry(&path, "module.json", &package_module_json)?;
                rewrite_package_intermediate_module_json(&package_module_json)?;
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

    // M1: Watch mode (SPEC-023) — rebuild on file changes.
    // After the initial build, watch source directories for changes and
    // trigger an incremental rebuild on every detected change batch.
    // Mirrors hvigor's `--watch` flag and chokidar-based watcher with
    // `buildDelay: 100` ms debounce.
    if watch_mode {
        let config_for_watch = config_arc.clone();
        // M3: Preview mode — watch only src/main/resources/ for resource-only reloads.
        // Full M3 (IDE WebSocket live preview) is not implemented; this provides
        // the file-watching infrastructure for resource detection.
        let watch_dirs: Vec<PathBuf> = if preview_mode {
            config_for_watch
                .modules
                .iter()
                .map(|m| m.src_path.join("src/main/resources"))
                .filter(|p| p.exists())
                .collect()
        } else {
            config_for_watch
                .modules
                .iter()
                .flat_map(|m| FileWatcher::dirs_for_module(&m.src_path))
                .collect()
        };
        let watcher = FileWatcher::new(config_for_watch, watch_dirs);
        println!("\nWatching for changes... (Ctrl+C to stop)");
        // Use a blocking handle to drive async rebuilds from the sync watcher callback.
        let rt = tokio::runtime::Handle::current();
        let target_str_w = target_str.clone();
        let mode_str_w = mode_str.clone();
        let module_filter_w = module_filter.clone();
        let product_filter_w = product_filter.clone();
        let project_root_w = project_root.clone();
        watcher
            .watch(move |changed| {
                println!(
                    "\n[watch] {} file(s) changed — rebuilding...",
                    changed.len()
                );
                let rebuild_args = BuildArgs {
                    target: target_str_w.clone(),
                    mode: mode_str_w.clone(),
                    module: module_filter_w.clone(),
                    product: product_filter_w.clone(),
                    project: Some(project_root_w.clone()),
                    watch: false, // inner rebuild does not recurse into watch
                    coverage: false,
                    preview: false,
                };
                if let Err(e) = rt.block_on(run(rebuild_args)) {
                    eprintln!("[watch] rebuild failed: {e:#}");
                }
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!("Watch error: {e}"))?;
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
                ..Default::default()
            },
        });
    }
    let content = fs_err::read_to_string(path)?;
    json5::from_str(&content).context("Failed to parse module.json5")
}

fn collect_native_libs(project_root: &Path, module: &ModuleConfig) -> Vec<PathBuf> {
    let mut libs = Vec::new();
    let mut seen = FxHashSet::default();
    for root in [
        module.src_path.join("src/main/libs"),
        module.src_path.join("libs"),
        module.src_path.join("oh_modules"),
        project_root.join("oh_modules"),
    ] {
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("so") {
                continue;
            }
            if path
                .components()
                .any(|component| component.as_os_str().to_string_lossy().as_ref() == "arm64-v8a")
            {
                let path = canonical_or_owned(path);
                if seen.insert(path.clone()) {
                    libs.push(path);
                }
            }
        }
    }
    libs.sort();
    libs
}

fn write_compile_arkts_cache(
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &Arc<ArkdownConfig>,
    chunk_graph: &ChunkGraph,
    resource_refs: &FxHashMap<String, ResourceReference>,
    resolved_imports: &ResolvedImportMap,
) -> Result<CompileArkTsHandoff> {
    CompileArktsCacheWriter::new(module, loader_json, config, resource_refs, resolved_imports)
        .write(chunk_graph)
}

/// K5 — ProcessIntegratedHsp: discover integrated (embedded) HSP dependencies.
///
/// Reads `oh-package.json5` for the module and collects any HSP dependencies
/// whose metadata has `isIntegratedHsp: true`.  These must be embedded inside
/// the HAP archive at `libs/<dir_name>/<file_name>.hsp`.
///
/// Mirrors hvigor's `ProcessIntegratedHsp` / `getIntegratedHspDependencies()`.
fn collect_integrated_hsps(
    module: &ModuleConfig,
    project_root: &Path,
    bundle_name: &str,
    version_code: u64,
) -> Vec<arkdown_pack::IntegratedHspEntry> {
    use arkdown_pack::{normalize_integrated_hsps, IntegratedHspEntry};

    #[derive(serde::Deserialize)]
    struct OhPkgMinimal {
        #[serde(default)]
        dependencies: std::collections::HashMap<String, String>,
    }

    #[derive(serde::Deserialize)]
    struct DepMetadata {
        #[serde(rename = "isIntegratedHsp", default)]
        is_integrated_hsp: bool,
    }

    #[derive(serde::Deserialize)]
    struct DepPkg {
        #[serde(default)]
        metadata: Option<DepMetadata>,
    }

    let pkg_json_path = module.src_path.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json_path) else {
        return vec![];
    };
    let Ok(pkg) = json5::from_str::<OhPkgMinimal>(&content) else {
        return vec![];
    };

    let oh_modules_roots = [
        module.src_path.join("oh_modules"),
        project_root.join("oh_modules"),
    ];

    let mut raw_entries: Vec<IntegratedHspEntry> = Vec::new();
    for (dep_name, _dep_ver) in &pkg.dependencies {
        // Find the package directory.
        let pkg_dir = oh_modules_roots
            .iter()
            .map(|r| r.join(dep_name))
            .find(|p| p.is_dir());
        let Some(pkg_dir) = pkg_dir else {
            continue;
        };

        // Check if this dep is marked as an integrated HSP.
        let dep_pkg_json = pkg_dir.join("oh-package.json5");
        let Ok(dep_content) = std::fs::read_to_string(&dep_pkg_json) else {
            continue;
        };
        let Ok(dep_pkg) = json5::from_str::<DepPkg>(&dep_content) else {
            continue;
        };
        if !dep_pkg.metadata.map_or(false, |m| m.is_integrated_hsp) {
            continue;
        }

        // Look for a pre-built .hsp file.
        let hsp_name = dep_name.rsplit('/').next().unwrap_or(dep_name);
        let hsp_path = pkg_dir.join(format!("{}.hsp", hsp_name));
        if !hsp_path.exists() {
            tracing::warn!(
                dep = dep_name,
                "K5: integrated HSP dependency has no pre-built .hsp file; skipping"
            );
            continue;
        }

        raw_entries.push(IntegratedHspEntry {
            hsp_dir_name: format!("{}@1.0.0", hsp_name),
            hsp_file_name: format!("{}.hsp", hsp_name),
            hsp_path,
        });
    }

    if raw_entries.is_empty() {
        return vec![];
    }

    let out_dir = module
        .src_path
        .join("build/default/intermediates/integrated_hsp");
    std::fs::create_dir_all(&out_dir).ok();

    // Normalize the HSPs (patch pack.info) and create entries pointing to
    // the normalized output files for the packer.
    match normalize_integrated_hsps(&raw_entries, bundle_name, version_code, &out_dir) {
        Ok(normalized_paths) => {
            raw_entries
                .into_iter()
                .zip(normalized_paths)
                .map(|(mut entry, normalized_path)| {
                    entry.hsp_path = normalized_path;
                    entry
                })
                .collect()
        }
        Err(e) => {
            tracing::warn!(error = %e, "K5: failed to normalize integrated HSPs");
            vec![]
        }
    }
}
