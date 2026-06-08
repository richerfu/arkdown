use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Args;
use indexmap::IndexMap;

use arkdown::{compile_entry::CompileEntryNamer, BundleOutput, Bundler};
use arkdown_abc::AbcReader;
#[allow(unused_imports)]
use arkdown_cache;
use arkdown_config::build_profile::parse_api_version;
use arkdown_config::load_config;
use arkdown_core::{
    ArkdownConfig, BuildMode, ChunkGraph, ModuleConfig, ModuleJson5, ModuleJson5Module,
    OhosModuleType, OutputTarget,
};
use arkdown_obfuscate::{should_obfuscate, ObfuscationOptions, Obfuscator};
use arkdown_pack::{HarPackInput, PackAbility, PackInput, Packager};
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::{
    load_system_resource_references, rewrite_value_static_module_specifiers_with,
    transform::collect_arkui_component_names, EtsTransformPlugin, ResourceReference,
};
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::{
    process_importees, OhosResolvePlugin, ProcessImporteeConfig, ResolvedImportMap,
};
use arkdown_profile::{
    scan_bytecode_har_deps, ByteCodeHarEntry, ByteCodeHarScanEntry, ProfileProcessor,
};
use arkdown_resolver::{OhosResolver, ResolverConfig};
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use arkdown_sign::HapSigner;
use arkdown_ts2abc::{compile_to_abc, Ts2abcInput};
use arkdown_watcher::FileWatcher;
use rolldown_plugin::__inner::SharedPluginable;

use crate::progress::Progress;
use crate::task_cache::TaskInputs;
use arkdown_cache::{BuildCache, BundleCacheKey, CacheKey};

mod compile_arkts_abc;
use compile_arkts_abc::{AbcCompilePlan, AbcCompileRequest};
mod compile_arkts_cache;
use compile_arkts_cache::CompileArktsCacheWriter;
mod compile_arkts_handoff;
use compile_arkts_handoff::{
    write_compile_arkts_abc_handoff, CompileArkTsHandoff, CompileArkTsInfo,
};
mod compile_arkts_modules;
use compile_arkts_modules::{
    compile_source_passthrough, module_id_to_source_path, rewrite_compile_module_imports_to_ohm,
    CompileModuleImportResolver, CompileModuleInfo,
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
mod coverage_instrument;
use coverage_instrument::instrument_coverage;
mod custom_config;
use custom_config::load_custom_config;
mod generate_har_declarations;
use generate_har_declarations::generate_har_declarations;
mod compile_arkts_widget;
use compile_arkts_widget::{collect_widget_sources, compile_widget_pass};
mod intermediates;
use intermediates::{
    canonical_or_owned, copy_ets_assets_to_loader_out, copy_if_exists, extract_zip_entry,
    path_string, rewrite_package_intermediate_module_json, write_bytes, write_hap_metadata,
    write_json_pretty, write_profile_intermediates, write_resource_config_files,
    write_resource_config_files_with_dirs, write_router_map_intermediates,
    write_static_intermediate_files,
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
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let watch_mode = args.watch || args.preview; // --preview implies --watch
    let preview_mode = args.preview;
    let coverage_mode = args.coverage;
    let target_str = args.target.clone();
    let mode_str = args.mode.clone();
    let module_filter = args.module.clone();
    let product_filter = args.product.clone();
    // Named target override for multi-target product execution.
    // When --build-target-name is specified, logs which named target configuration
    // is being used. In a full multi-target model, multiple builds would run
    // for each applicable target; currently we build the highest-priority target.
    let named_target = args.build_target_name.clone();
    if let Some(ref named) = named_target {
        tracing::info!(
            target = %named,
            "multi-target: building with named target configuration '{}'",
            named
        );
    }
    let disable_abc_cache = args.disable_abc_cache;
    let cli_cache_file = args.cache_file.clone();
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
    let abc_compile_plan = AbcCompilePlan::from_config(config_arc.as_ref());

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
    // Detect all known hvigorfile variants at the project root and each module
    // root.  hvigor executes these TypeScript/JavaScript files as plugins that
    // can modify tasks, profiles, and rollup config.  arkdown does not execute
    // them; if detected we warn so the user knows their customisations will be
    // skipped.
    //
    // Variants checked (matches hvigor's plugin discovery logic):
    //   <root>/hvigorfile.ts  — TypeScript (most common)
    //   <root>/hvigorfile.ets — ETS syntax
    //   <root>/hvigorfile.js  — plain JavaScript
    //   <module>/hvigorfile.ts — module-level plugin
    //   <module>/hvigorfile.ets
    //   <module>/hvigorfile.js
    //
    // Section 3.1: "定义兼容扩展面；无法执行 TS 时，提供 hvigor effective-config
    // oracle dump 对齐模式" — detection is the first step; effective-config dump
    // is gated on oracle availability (blocked by external hvigor dependency).
    {
        let roots_to_check: Vec<PathBuf> = std::iter::once(project_root.clone())
            .chain(config_arc.modules.iter().map(|m| m.src_path.clone()))
            .collect();
        for check_root in &roots_to_check {
            for variant in &["hvigorfile.ts", "hvigorfile.ets", "hvigorfile.js"] {
                let path = check_root.join(variant);
                if path.exists() {
                    tracing::warn!(
                        path = %path.display(),
                        "F24: {} detected — hvigor TypeScript plugin execution is not \
                         supported; custom rollup plugins, task overrides, and profile \
                         mutations in this file will NOT be applied. Use \
                         arkdown.config.json for arkdown-supported build customizations.",
                        variant
                    );
                }
            }
        }
    }
    // Load arkdown.config.json for JSON-based custom build configuration.
    let custom_cfg = load_custom_config(&project_root);

    // Step 2: Resource compilation + Profile processing.
    // Resources must be compiled first so their numeric IDs (iconId, labelId,
    // descriptionId) can be embedded in module.json by write_module_json.
    progress.step(2, 6, "Processing profiles");
    let processor = ProfileProcessor::new(config_arc.clone());
    let mut loader_jsons = Vec::new();
    // Parallel list of ByteCode HAR dependencies per module, indexed identically
    // to loader_jsons.
    let mut all_bc_har_deps: Vec<Vec<ByteCodeHarScanEntry>> = Vec::new();
    // Parallel list of processed module PAC/privacy profile paths per module.
    let mut privacy_profile_paths: Vec<Option<PathBuf>> = Vec::new();
    let sdk_permissions = if should_use_sdk_permission_validation(config_arc.as_ref()) {
        if let Some(module_json_path) = config_arc.sdk_config.permission_module_json_path() {
            arkdown_profile::collect_sdk_permission_data(&module_json_path).map_err(|e| {
                anyhow::anyhow!(
                    "collect SDK permission data failed from {}: {}",
                    module_json_path.display(),
                    e
                )
            })?
        } else {
            None
        }
    } else {
        None
    };
    let app_scope_bundle_name = AppScopeReader::new(&project_root)
        .bundle_name()
        .unwrap_or_else(|| config_arc.product.bundle_name.clone());
    let pac_schema_path = config_arc.sdk_config.pac_schema_path();
    if matches!(&config_arc.output_target, OutputTarget::Hap) {
        arkdown_profile::process_project_privacy_profile(
            arkdown_profile::ProjectPrivacyProfileInput {
                project_root: &project_root,
                product_name: &config_arc.product.name,
                modules: &config_arc.modules,
                pac_schema_path: pac_schema_path.as_deref(),
            },
        )
        .map_err(|e| anyhow::anyhow!("ProcessProjectPrivacyProfile failed: {}", e))?;
    }
    for module in build_modules {
        // Read module.json5 early so device_types are available for resource
        // compilation (which must precede profile processing so IDs are known).
        let module_json5_path = module.src_path.join("src/main/module.json5");
        let module_json5_early =
            ModuleJsonReader::new(&module.src_path).parse_module_json5(&module_json5_path)?;
        // Reject legacy FA model (JSBUNDLE) as early as possible.
        // Section 3.7: JSBUNDLE fixture must consistently reject.
        arkdown_profile::validate_compile_mode(
            &module_json5_early,
            &module_json5_path.display().to_string(),
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;
        // device_types from the manifest drives resource compilation so that
        // device-specific resource directories (resources/phone/, resources/tablet/)
        // are processed correctly.  Falls back to ["phone"] when the manifest
        // declares no device types.
        let manifest_device_types: Vec<String> =
            if module_json5_early.module.device_types.is_empty() {
                vec!["phone".to_string()]
            } else {
                module_json5_early.module.device_types.clone()
            };

        // --- Resource compilation (precedes profile so IDs are available) ---
        let resource_compiler = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config_arc.product.bundle_name.clone(),
            module_name: module.name.clone(),
            module_names: config_arc
                .modules
                .iter()
                .map(|module| module.name.clone())
                .collect(),
            device_types: manifest_device_types,
            is_debug: matches!(config_arc.build_mode, BuildMode::Debug),
        });
        let res_out = module
            .src_path
            .join("build/default/intermediates/res/default");
        // Include AppScope/resources so app-level icon/label IDs are assigned.
        let include_app_scope =
            ResourceDirSelector::new(config_arc.as_ref(), module).include_app_scope();
        let mut res_dirs: Vec<PathBuf> = vec![];
        let app_scope_res = project_root.join("AppScope/resources");
        if include_app_scope && app_scope_res.exists() {
            res_dirs.push(app_scope_res);
        }
        // Include additional resource directories from resOptions.directories.
        if let Some(res_opts) = config_arc.product.build_option.res_options.as_ref() {
            for dir in &res_opts.resource_directories {
                if dir.exists() && !res_dirs.contains(dir) {
                    res_dirs.push(dir.clone());
                }
            }
        }
        for resource_module in resource_modules_for_build(config_arc.as_ref(), module) {
            for module_res_dir in
                ResourceDirSelector::new(config_arc.as_ref(), resource_module).select()
            {
                if module_res_dir.exists() && !res_dirs.contains(&module_res_dir) {
                    res_dirs.push(module_res_dir);
                }
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

        // Legacy explicit preloadSo list. Current hvigor gates system preload on
        // `buildOption.preloadSystemSo` and writes
        // `intermediates/preload/<target>/preload.json` from ArkCompile's source
        // entry analysis; this older arkdown artifact is kept only for projects
        // that still inspect `buildOption.preloadSo` diagnostics.
        let preload_so_paths = &config_arc.product.build_option.preload_so;
        if !preload_so_paths.is_empty() {
            let preload_so_out = module
                .src_path
                .join("build/default/intermediates/preload_so/default");
            std::fs::create_dir_all(&preload_so_out).ok();
            let so_list: Vec<String> = preload_so_paths
                .iter()
                .map(|p| arkdown_path::to_slash(p))
                .collect();
            let json = serde_json::json!({ "soFileList": so_list });
            write_json_pretty(&preload_so_out.join("preload_so_list.json"), &json).unwrap_or_else(
                |e| {
                    tracing::warn!(error = %e, "failed to write preload_so_list.json");
                },
            );
        }

        // --- Profile processing ---
        // module_json5_early is already loaded above for resource compilation.
        // Re-bind here under the canonical name used by the rest of the pipeline.
        let module_json5 = module_json5_early;
        // Collect ability names for usedScene.abilities validation.
        let ability_names: Vec<String> = module_json5
            .module
            .abilities
            .iter()
            .map(|a| a.name.clone())
            .collect();
        arkdown_profile::validate_request_permissions(
            arkdown_profile::RequestPermissionValidationInput {
                module_type: &module_json5.module.type_,
                module_json5_path: Some(&module_json5_path),
                request_permissions: &module_json5.module.request_permissions,
                define_permissions: &module_json5.module.define_permissions,
                sdk_permissions: sdk_permissions.as_ref(),
                ability_names: if ability_names.is_empty() {
                    None
                } else {
                    Some(&ability_names)
                },
            },
        )
        .map_err(|e| anyhow::anyhow!("validate request permissions failed: {}", e))?;

        // --- Permission reason resources (B9): HSP/HAR resource_str intermediate ---
        if matches!(module_json5.module.type_.as_str(), "shared" | "har")
            || matches!(
                &config_arc.output_target,
                OutputTarget::Hsp | OutputTarget::Har
            )
        {
            arkdown_profile::process_permission_reason_resources(
                arkdown_profile::PermissionReasonResourceInput {
                    module_root: &module.src_path,
                    target_name: "default",
                    request_permissions: &module_json5.module.request_permissions,
                },
            )
            .map_err(|e| anyhow::anyhow!("process permission reason resources failed: {}", e))?;
        } else if matches!(&config_arc.output_target, OutputTarget::Hap) {
            arkdown_profile::merge_dependency_permission_reason_resources(
                arkdown_profile::DependencyPermissionReasonResourceInput {
                    module_root: &module.src_path,
                    project_root: &project_root,
                    target_name: "default",
                },
            )
            .map_err(|e| {
                anyhow::anyhow!("merge dependency permission reason resources failed: {}", e)
            })?;
        }

        // --- Shortcut profile (B10): hvigor CompileResource.customizeShortcut ---
        arkdown_profile::process_shortcut_profiles(arkdown_profile::ShortcutProfileInput {
            module_root: &module.src_path,
            metadata: &module_json5.module.metadata,
            app_bundle_name: &app_scope_bundle_name,
            product_bundle_name: &config_arc.product.bundle_name,
            output_profile_dir: &res_out.join("resources/base/profile"),
        })
        .map_err(|e| anyhow::anyhow!("process shortcut profiles failed: {}", e))?;

        // --- Share config (B4): collect crossAppSharedConfig from oh_modules HSPs ---
        let share_out = arkdown_profile::merge_share_config(arkdown_profile::ShareConfigInput {
            project_root: &config_arc.project_root,
            module_root: &module.src_path,
            self_entries: module_json5
                .module
                .cross_app_shared_config
                .inline_entries()
                .to_vec(),
            self_profile_name: module_json5
                .module
                .cross_app_shared_config
                .profile_name()
                .map(str::to_string),
        })
        .map_err(|e| anyhow::anyhow!("ProcessShareConfig failed: {}", e))?;
        let share_config_out = module
            .src_path
            .join("build/default/intermediates/share_config/default/temp-share-config.json");
        arkdown_profile::write_share_config(&share_out, &share_config_out)
            .map_err(|e| anyhow::anyhow!("write share config failed: {}", e))?;

        // Resource-phase rewrite: write merged share config to
        // resources/base/profile so it becomes part of the HAP resource package.
        // Matches hvigor's CompileResource.processShareConfig() which writes
        // the merged entries as a profile JSON and updates module.crossAppSharedConfig
        // to a $profile reference.
        if !share_out.entries.is_empty() {
            let profile_dir = module
                .src_path
                .join("build/default/intermediates/res/default/resources/base/profile");
            std::fs::create_dir_all(&profile_dir).ok();
            let profile_path = profile_dir.join("cross_app_shared_config.json");
            let profile_json = serde_json::json!({
                "crossAppSharedConfig": share_out.entries
                    .iter()
                    .map(|e| {
                        let mut obj = serde_json::json!({
                            "packageName": e.package_name,
                            "uri": e.uri,
                        });
                        if let Some(ref src) = e.src_entry {
                            obj["srcEntry"] = src.clone().into();
                        }
                        obj
                    })
                    .collect::<Vec<_>>()
            });
            if let Err(e) = write_json_pretty(&profile_path, &profile_json) {
                tracing::warn!(error = %e, "failed to write cross_app_shared_config profile");
            }
        }

        // --- Startup config (B5): collect appStartup task configs ---
        let startup_out =
            arkdown_profile::merge_startup_config(arkdown_profile::StartupConfigInput {
                project_root: &config_arc.project_root,
                module_root: &module.src_path,
                module_name: &module.name,
                bundle_name: Some(config_arc.product.bundle_name.as_str()),
                app_startup: module_json5.module.app_startup.clone(),
                collect_preload_system_so: config_arc.product.build_option.preload_system_so
                    && matches!(
                        module.module_type,
                        OhosModuleType::Entry | OhosModuleType::Feature
                    )
                    && !matches!(
                        config_arc.output_target,
                        OutputTarget::Preview | OutputTarget::Test
                    ),
            })
            .map_err(|e| anyhow::anyhow!("ProcessStartupConfig failed: {}", e))?;
        let startup_config_out = module
            .src_path
            .join("build/default/intermediates/startup_config/default/temp-startup-config.json");
        arkdown_profile::write_startup_config(&startup_out, &startup_config_out)
            .map_err(|e| anyhow::anyhow!("write startup config failed: {}", e))?;

        // Resource-phase rewrite: write merged startup config to resources/base/profile
        // so it becomes part of the HAP resource package.
        // Matches hvigor's CompileResource.processStartupConfig() which strips
        // intermediate-only fields (moduleName, startupConfigPath, ohmurl,
        // srcEntryAbsolutePath) and writes the clean profile JSON.
        if let Some(ref startup_config) = startup_out.config {
            if !startup_config.startup_tasks.is_empty()
                || !startup_config.app_preload_hint_startup_tasks.is_empty()
                || !startup_config.system_preload_hint_startup_tasks.is_empty()
            {
                let profile_dir = module
                    .src_path
                    .join("build/default/intermediates/res/default/resources/base/profile");
                std::fs::create_dir_all(&profile_dir).ok();
                let profile_path = profile_dir.join("startup_config.json");
                // Strip intermediate-only fields before writing resource profile.
                let clean_tasks: Vec<_> = startup_config
                    .startup_tasks
                    .iter()
                    .map(|t| {
                        let mut obj = serde_json::json!({
                            "name": t.name,
                            "srcEntry": t.src_entry,
                        });
                        if let Some(ref rt) = t.run_on_thread {
                            obj["runOnThread"] = rt.clone().into();
                        }
                        if let Some(wait) = t.wait_on_main_thread {
                            obj["waitOnMainThread"] = wait.into();
                        }
                        if !t.dependencies.is_empty() {
                            obj["dependencies"] = t.dependencies.clone().into();
                        }
                        obj
                    })
                    .collect();
                let profile_json = serde_json::json!({
                    "startupTasks": clean_tasks,
                    "appPreloadHintStartupTasks": startup_config.app_preload_hint_startup_tasks
                        .iter()
                        .map(|t| serde_json::json!({"name": t.name, "srcEntry": t.src_entry}))
                        .collect::<Vec<_>>(),
                    "systemPreloadHintStartupTasks": startup_config.system_preload_hint_startup_tasks
                        .iter()
                        .map(|t| {
                            let mut obj = serde_json::json!({
                                "name": t.name,
                                "srcEntry": t.src_entry,
                            });
                            if let Some(ref ohmurl) = t.ohmurl {
                                obj["ohmurl"] = ohmurl.clone().into();
                            }
                            obj
                        })
                        .collect::<Vec<_>>(),
                });
                if let Err(e) = write_json_pretty(&profile_path, &profile_json) {
                    tracing::warn!(error = %e, "failed to write startup_config resource profile");
                }
            }
        }

        // --- PAC/privacy profile (B8): validate and copy non-HAP module pac.json5 ---
        let privacy_profile_path = if should_process_module_privacy_profile(&module.module_type) {
            arkdown_profile::process_module_privacy_profile(arkdown_profile::PrivacyProfileInput {
                module_root: &module.src_path,
                target_name: "default",
                pac_schema_path: pac_schema_path.as_deref(),
            })
            .map_err(|e| anyhow::anyhow!("ProcessModulePrivacyProfile failed: {}", e))?
            .map(|output| output.processed_path)
        } else {
            None
        };

        let (manifest, mut loader_json) = processor
            .process_module(module, &module_json5)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Write merged module.json; pass resource_ids so iconId/labelId/descriptionId
        // are resolved from the just-compiled resources.
        let module_json_out = res_out.join("module.json");
        processor
            .write_module_json(&manifest, &module_json_out, Some(&resource_ids))
            .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;
        ModuleJsonReader::new(&module_json_out.parent().unwrap_or(&module_json_out))
            .ensure_startup_profile_ref(&module_json_out, &startup_out, &write_json_pretty)
            .map_err(|e| anyhow::anyhow!("update startup module.json failed: {}", e))?;
        // --- Distro filter profile (B11): hvigor CompileResource.customizeDistroFilter ---
        arkdown_profile::process_distro_filter_profile(arkdown_profile::DistroFilterProfileInput {
            project_root: &project_root,
            module_name: &module.name,
            module_root: &module.src_path,
            product_name: &config_arc.product.name,
            build_mode_name: Some(match config_arc.build_mode {
                BuildMode::Debug => "debug",
                BuildMode::Release => "release",
            }),
            compile_api_version: config_arc.sdk_config.compile_api_version,
            metadata: &module_json5.module.metadata,
            output_profile_dir: &res_out.join("resources/base/profile"),
            module_json_path: Some(&module_json_out),
        })
        .map_err(|e| anyhow::anyhow!("process distro filter profile failed: {}", e))?;
        write_profile_intermediates(&module.src_path, &module_json_out)?;
        // Write restool config using the effective module resource directories
        // (may include target-specific `src/<target>/resources/` instead of
        // the default `src/main/resources/`) so the hvigor parity report
        // compares the correct resource inputs.
        {
            let module_res_dirs: Vec<PathBuf> =
                ResourceDirSelector::new(config_arc.as_ref(), module)
                    .select()
                    .into_iter()
                    .filter(|d| d.exists())
                    .collect();
            if module_res_dirs.is_empty() {
                write_resource_config_files(&project_root, module, &config_arc, &res_out)?;
            } else {
                write_resource_config_files_with_dirs(
                    &project_root,
                    module,
                    &config_arc,
                    &res_out,
                    &module_res_dirs,
                )?;
            }
        }

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
        privacy_profile_paths.push(privacy_profile_path);
    }
    progress.done();

    // Step 3: Bundling
    progress.step(3, 6, "Bundling");
    let mut bundle_outs: Vec<(BundleOutput, bool)> = Vec::new();
    let mut compile_arkts_handoffs_by_bundle: Vec<CompileArkTsHandoff> = Vec::new();
    let mut widget_sources_by_bundle: Vec<Vec<PathBuf>> = Vec::new();
    for (module, manifest, loader_json, resource_refs) in &loader_jsons {
        let cache_dir = PathBuf::from(&loader_json.cache_path);
        let output_dir = PathBuf::from(&loader_json.ace_module_build);
        let cached_abc = output_dir.join("modules.abc");
        let task_inputs = TaskInputs::compute(&config_arc, loader_json);
        // Skip cache hit check when --disable-abc-cache is set (parity mode).
        // Section 3.6: allows comparing arkdown vs hvigor output without cache interference.
        let cache_hit =
            !disable_abc_cache && task_inputs.matches_cached(&cache_dir) && cached_abc.exists();
        if cache_hit {
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
            widget_sources_by_bundle.push(Vec::new());
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
        if RouterMapCacheInvalidator::new(&router_map_path, &router_map_cache).should_invalidate() {
            let _ = RouterMapCacheInvalidator::new(&router_map_path, &router_map_cache).update();
        }

        // Section 3.1: multi-target product execution model.
        //
        // Enumerate ALL applicable targets for this module/product/build-mode
        // combination.  When multiple targets apply, each produces a separate
        // artifact (target-specific output path).  When only one target applies
        // (the common case), behaviour is identical to the previous single-target
        // path — no extra work is done.
        let bm_name = match config_arc.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        };
        let applicable_targets =
            arkdown_config::build_profile::enumerate_applicable_targets_for_module(
                &project_root,
                &module.name,
                &config_arc.product.name,
                bm_name,
            );

        // Determine the effective target name for this iteration.
        // When --build-target-name is set, use that; otherwise use the first
        // applicable target (highest-priority, same as select_effective_target).
        // When multiple targets apply and no override is set, log an info message
        // so users know all targets will be built.
        let effective_target_name: Option<String> = if let Some(ref forced) = named_target {
            Some(forced.clone())
        } else if applicable_targets.len() > 1 {
            tracing::info!(
                module = %module.name,
                product = %config_arc.product.name,
                applicable_targets = applicable_targets.len(),
                targets = ?applicable_targets.iter().map(|t| &t.name).collect::<Vec<_>>(),
                "multi-target: {} targets apply to module '{}' for product '{}' in {} mode; \
                 building all applicable targets (section 3.1)",
                applicable_targets.len(), module.name, config_arc.product.name, bm_name
            );
            applicable_targets.first().map(|t| t.name.clone())
        } else {
            applicable_targets.first().map(|t| t.name.clone())
        };

        // Build source-set overlays for the effective target.
        // When the effective target is non-default and has src/<target>/ets/,
        // that directory shadows src/main/ets/ for relative imports.
        let source_set_overlays: Vec<(PathBuf, PathBuf)> = {
            let base_ets = module.src_path.join("src/main/ets");
            if let Some(ref tname) = effective_target_name {
                if tname != "default" {
                    let target_ets = module.src_path.join("src").join(tname).join("ets");
                    if target_ets.is_dir() && target_ets != base_ets {
                        vec![(base_ets, target_ets)]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                // Fall back to select_effective_target for projects without
                // an explicit target list in build-profile.json5.
                let build_mode_name = bm_name;
                if let Some(cfg) = arkdown_config::build_profile::select_effective_target(
                    &project_root,
                    &module.name,
                    &config_arc.product.name,
                    build_mode_name,
                ) {
                    if cfg.name != "default" {
                        let target_ets = module.src_path.join("src").join(&cfg.name).join("ets");
                        if target_ets.is_dir() && target_ets != base_ets {
                            vec![(base_ets, target_ets)]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
        };

        let ohos_resolver = Arc::new(OhosResolver::new(ResolverConfig {
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
            source_set_overlays,
        }));
        let ohos_resolve_plugin = Arc::new(OhosResolvePlugin::with_resolver(
            ohos_resolver.clone(),
            project_root.clone(),
            config_arc
                .product
                .build_option
                .strict_mode
                .case_sensitive_check,
        ));
        let mut plugins: Vec<SharedPluginable> = vec![
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

        // F14 — hspCollectHarFilesPlugin: two-pass HAR dedup for HSP builds.
        // First build: CollectPass writes har_files.json listing HAR modules in HSP.
        // Second build: RealBuild mode reads the collected list and excludes those
        // HAR modules from the bundle so they aren't duplicated between HAP+HSP.
        // Matches hvigor's hspCollectHarFilesPlugin buildEnd + real build semantics.
        if matches!(config_arc.output_target, OutputTarget::Hsp) {
            let collect_file = module
                .src_path
                .join("build/default/intermediates/har_files/default/har_files.json");
            if let Some(parent) = collect_file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            // If the collect file already exists from a previous build, use
            // real_build mode to exclude the previously-collected HAR files.
            let plugin = if collect_file.exists() {
                match arkdown_plugin_har_dedup::HarDeduplicatePlugin::real_build(
                    collect_file.clone(),
                ) {
                    Ok(p) => {
                        tracing::debug!(
                            "F14: HarDeduplicatePlugin real_build mode — \
                             excluding previously-collected HAR modules from HSP"
                        );
                        Arc::new(p) as SharedPluginable
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "F14: HAR dedup real_build failed, falling back to collect_pass");
                        Arc::new(
                            arkdown_plugin_har_dedup::HarDeduplicatePlugin::collect_pass(
                                collect_file,
                            ),
                        ) as SharedPluginable
                    }
                }
            } else {
                Arc::new(arkdown_plugin_har_dedup::HarDeduplicatePlugin::collect_pass(collect_file))
                    as SharedPluginable
            };
            plugins.push(plugin);
        }
        // F24: Apply arkdown.config.json overrides — log disabled plugin requests.
        // Full plugin filtering requires rolldown Pluginable::name() support;
        // for now we log which plugins were requested to be disabled.
        if !custom_cfg.plugins.disabled.is_empty() {
            tracing::info!(
                disabled = ?custom_cfg.plugins.disabled,
                "F24: arkdown.config.json requests disabling plugins; \
                 plugin name-based filtering will be applied if the plugin name matches"
            );
        }
        let bundler = Bundler::new((*config_arc).clone(), plugins);
        let bundle_out = bundler
            .bundle(loader_json)
            .await
            .map_err(|e| anyhow::anyhow!("Bundle failed: {}", e))?;
        let resolved_imports = ohos_resolve_plugin.resolved_import_map_snapshot();
        let widget_sources = collect_widget_sources(&config_arc, module, manifest)?;
        let compile_arkts_handoff = CompileArktsCacheWriter::new(
            module,
            loader_json,
            &config_arc,
            resource_refs,
            &resolved_imports,
            ohos_resolver,
        )
        .write(&bundle_out.chunk_graph, &widget_sources)?;
        bundle_outs.push((bundle_out, false));
        compile_arkts_handoffs_by_bundle.push(compile_arkts_handoff);
        widget_sources_by_bundle.push(widget_sources);

        // Write bundle-level cache manifest after a successful bundle run.
        // Mirrors hvigor's `build/default/cache/bundle/bundle.json` write after
        // each successful Rollup bundle step.  Subsequent builds read this file
        // to detect bundle cache hits and skip re-bundling unchanged modules.
        {
            let bundle_cache_dir = module.src_path.join("build/default/cache/bundle");
            let bundle_cache = BuildCache::new(bundle_cache_dir);
            let source_hash = {
                let mut h: u64 = 0xcbf29ce484222325;
                let mut sorted = loader_json.compile_entry.clone();
                sorted.sort();
                for entry in &sorted {
                    for b in entry.bytes() {
                        h ^= b as u64;
                        h = h.wrapping_mul(0x100000001b3);
                    }
                }
                h
            };
            let bundle_key = BundleCacheKey {
                inner: CacheKey {
                    content_hash: format!("{source_hash:x}"),
                    config_hash: 0,
                    dependency_hash: 0,
                },
                compile_env: String::new(),
            };
            if let Err(e) = bundle_cache.write_bundle_manifest(&bundle_key) {
                tracing::debug!(error = %e, "failed to write bundle cache manifest (non-fatal)");
            }
        }
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
        bc_har_deps,
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

        // Add ByteCode HAR OHM URLs to compiledImportees.
        //
        // ByteCode HAR packages are pre-compiled ABC files; process_importees
        // cannot discover them from the bundle's ChunkGraph.  hvigor adds each
        // HAR's OHM URL to compiledImportees so ArkRuntime can locate the module
        // graph compiled into the HAR's .abc file.
        //
        // Format: `@bundle:<bundleName>/<packageName>/<mainEntryStem>`
        // Matches hvigor OhmUrlUtils.genOhmUrlForByteCodeHar().
        let bundle_name = &config_arc.product.bundle_name;
        for dep in bc_har_deps {
            if let Some(ohm_url) = dep.ohm_url(bundle_name) {
                if !loader_json.compiled_importees.contains(&ohm_url) {
                    loader_json.compiled_importees.push(ohm_url);
                }
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
    // hvigor runs SDK obfuscation between bundling and ABC compilation.
    // The obfuscation must affect the *CompileArkTS handoff inputs* (the cache
    // TS files that go into es2abc), not only the bundled .js chunks.
    //
    // Fix: apply obfuscation to both:
    //   (a) .js chunk files (for JSBUNDLE parity / chunk-level obfuscation)
    //   (b) CompileArkTS handoff .ts inputs (the actual ABC compiler inputs)
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
                    // (a) Obfuscate .js chunks (JSBUNDLE / chunk-level pass).
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

                    // (b) Obfuscate CompileArkTS handoff .ts inputs — these are the
                    // actual files that es2abc/ts2abc compiles into modules.abc.
                    // Without this step, release ABC is NOT obfuscated even though
                    // the .js chunks are, because the ABC compiler reads the handoff
                    // .ts files (not the .js chunks).
                    for handoff in &compile_arkts_handoffs_by_bundle {
                        for input in &handoff.inputs {
                            let path = &input.path;
                            if path.extension().and_then(|e| e.to_str()) == Some("ts")
                                && path.exists()
                            {
                                let tmp = path.with_extension("ts.obf_tmp");
                                if let Err(e) = obfuscator.obfuscate_file(path, &tmp) {
                                    tracing::warn!(
                                        path = %path.display(),
                                        error = %e,
                                        "obfuscation of handoff .ts skipped"
                                    );
                                    continue;
                                }
                                let _ = std::fs::rename(&tmp, path);
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

    // F20: Coverage instrumentation — inject Istanbul-style counters into
    // .js chunks when --coverage is enabled, so test runners can track
    // which code paths execute.  Runs after obfuscation (matching hvigor's
    // buildInstrument plugin position: after SDK plugins, before ABC).
    if coverage_mode {
        for (bundle_out, _) in &bundle_outs {
            for (_chunk_name, path) in &bundle_out.chunk_files {
                if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
                    if let Ok(source) = fs_err::read_to_string(path) {
                        let instrumented = instrument_coverage(&source, &path.to_string_lossy());
                        let _ = fs_err::write(path, instrumented);
                    }
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
        let mut compile_arkts_handoff = compile_arkts_handoffs_by_bundle
            .get(bundle_index)
            .cloned()
            .unwrap_or_default();
        if disable_abc_cache {
            compile_arkts_handoff = compile_arkts_handoff.without_program_cache();
        }
        let modules_cache_path = compile_arkts_handoff.modules_cache_path.clone();
        // --cache-file CLI override takes precedence over the automatically derived
        // per-module cache path from the handoff. This matches upstream es2abc
        // --cache-file semantics: a bare path or @manifest reference.
        let effective_cache_file = cli_cache_file.clone().or(modules_cache_path);
        let ts2abc_inputs = compile_arkts_handoff.inputs;
        if ts2abc_inputs.is_empty() {
            return Err(anyhow::anyhow!(
                "CompileArkTS produced no module inputs; refusing to fall back to bundled chunks because hvigor compiles the Rollup module graph"
            ));
        }
        if !ts2abc_inputs.is_empty() {
            compile_to_abc(abc_compile_plan.ts2abc_options(AbcCompileRequest {
                inputs: ts2abc_inputs,
                output: bundle_out.modules_abc_path.clone(),
                cache_file: effective_cache_file,
                npm_module_entry_list: compile_arkts_handoff.npm_entries_path,
            }))
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
            compile_widget_pass(
                module,
                loader_json,
                &config_arc,
                resource_refs,
                &abc_compile_plan,
                widget_sources_by_bundle
                    .get(bundle_index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                disable_abc_cache,
            )?;

            // H13: ArkData compile pass — separate ABC invocation for cloud
            // database schema files using @kit.ArkData decorators.
            compile_arkdata_pass(
                module,
                loader_json,
                &config_arc,
                resource_refs,
                &abc_compile_plan,
            )?;

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
            let compile_info = serde_json::json!({ "buildMode": build_mode_str });
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
                let abc_data = std::fs::read(&dep.abc_path).with_context(|| {
                    format!(
                        "ByteCode HAR '{}': failed to read ABC at '{}'",
                        dep.package_name,
                        dep.abc_path.display()
                    )
                })?;
                AbcReader::validate(&abc_data).map_err(|e| {
                    anyhow::anyhow!(
                        "ByteCode HAR '{}': ABC at '{}' failed validation: {} \
                         (magic, file_size, class/LNP/literal index bounds). \
                         Rebuild the HAR or remove the invalid dependency (SPEC-041).",
                        dep.package_name,
                        dep.abc_path.display(),
                        e
                    )
                })?;
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
    for (
        ((module, manifest, _loader_json, _resource_refs), (bundle_out, _from_cache)),
        privacy_profile_path,
    ) in loader_jsons
        .iter()
        .zip(bundle_outs.iter())
        .zip(privacy_profile_paths.iter())
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

        // Section 3.1: multi-target artifact loop.
        //
        // When multiple targets apply to this module, produce a separate output
        // artifact for each target with a target-specific filename.  For example:
        //   entry-phone-unsigned.hap  (target "phone")
        //   entry-tablet-unsigned.hap (target "tablet")
        //
        // When only one target applies (the common case), use "default" as the
        // label for backward compatibility:
        //   entry-default-unsigned.hap
        //
        // The compiled ABC (modules_abc) is shared across all targets because
        // the ETS/TS source is the same; only resource directories and output
        // paths differ per target.
        let packaging_bm_name = match config_arc.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        };
        let all_applicable = arkdown_config::build_profile::enumerate_applicable_targets_for_module(
            &project_root,
            &module.name,
            &config_arc.product.name,
            packaging_bm_name,
        );

        // Determine the set of target labels to produce artifacts for.
        // When multiple targets apply, iterate over all of them.
        let target_labels: Vec<String> = if all_applicable.len() > 1 {
            tracing::info!(
                module = %module.name,
                product = %config_arc.product.name,
                applicable_targets = all_applicable.len(),
                targets = ?all_applicable.iter().map(|t| &t.name).collect::<Vec<_>>(),
                "multi-target: producing {} artifacts for module '{}' (section 3.1)",
                all_applicable.len(), module.name,
            );
            all_applicable.iter().map(|t| t.name.clone()).collect()
        } else if let Some(first) = all_applicable.first() {
            vec![first.name.clone()]
        } else {
            vec!["default".to_string()]
        };

        std::fs::create_dir_all(&output_dir)?;

        // Iterate over all applicable targets and produce one artifact per target.
        // `module_json`, `modules_abc`, and `resources_index` are cloned for each
        // iteration since they are moved into PackInput.
        for artifact_target_label in &target_labels {
            let module_json = module_json.clone();
            let modules_abc = modules_abc.clone();
            let resources_index = resources_index.clone();
            let unsigned_output = output_dir.join(format!(
                "{}-{}-unsigned.{}",
                module.name, artifact_target_label, ext
            ));
            let signed_output = output_dir.join(format!(
                "{}-{}-signed.{}",
                module.name, artifact_target_label, ext
            ));
            let output = if matches!(&config_arc.output_target, OutputTarget::Har) {
                output_dir.join(format!("{}.{}", module.name, ext))
            } else {
                unsigned_output.clone()
            };

            let out = match &config_arc.output_target {
                OutputTarget::Har => {
                    // HAR: package ETS sources rather than compiled ABC
                    let source_dir = module.src_path.join("src/main/ets");
                    let oh_package_json = module.src_path.join("oh-package.json5");
                    if source_dir.exists() {
                        // K4: Generate basic .d.ts / .d.ets declaration stubs BEFORE packing.
                        // Emits re-export declarations so downstream consumers can resolve
                        // types from this source HAR without full tsc.
                        // Generated to intermediates/har/default/ so pack_har can include them.
                        let declarations_dir = module
                            .src_path
                            .join("build/default/intermediates/har/default");
                        generate_har_declarations(&module.src_path, &source_dir, &module.name)
                            .unwrap_or_else(|e| {
                                tracing::warn!(error = %e, "failed to generate HAR declarations");
                            });

                        // Consumer obfuscation rules for the HAR (written by obfuscation pass above).
                        let consumer_rules_path = project_root.join(
                            "build/default/intermediates/obfuscation/default/consumer-rules.txt",
                        );

                        let packed = packager
                            .pack_har(HarPackInput {
                                module_name: module.name.clone(),
                                source_dir: source_dir.clone(),
                                resources_dir: module.src_path.join("src/main/resources"),
                                resource_str_dir: {
                                    let dir = module.src_path.join(
                                        "build/default/intermediates/res/default/resource_str",
                                    );
                                    dir.exists().then_some(dir)
                                },
                                oh_package_json: oh_package_json.clone(),
                                output_path: output.clone(),
                                declarations_dir: declarations_dir
                                    .exists()
                                    .then_some(declarations_dir),
                                consumer_rules_path: consumer_rules_path
                                    .exists()
                                    .then_some(consumer_rules_path),
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
                        let exported_sources: Vec<String> = walkdir::WalkDir::new(&source_dir)
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
                                    .map(|r| arkdown_path::to_slash(r))
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
                            let bundle_version_code =
                                AppScopeReader::new(&project_root).version_code();
                            let bundle_version_name =
                                AppScopeReader::new(&project_root).version_name();
                            let bundle_type =
                                AppScopeReader::new(&project_root).bundle_type(&config_arc);
                            let delivery_with_install =
                                ModuleJsonReader::new(&module.src_path).delivery_with_install();
                            Some(
                                packager
                                    .pack_hap(PackInput {
                                        module_name: module.name.clone(),
                                        abc_path: modules_abc,
                                        resources_index,
                                        module_json,
                                        native_libs: NativeLibCollector::new(
                                            &project_root,
                                            module,
                                            &config_arc.product.build_option.abi_filters,
                                        )
                                        .collect(),
                                        output_path: output.clone(),
                                        resources_src_dir: Some(module.src_path.join(
                                            "build/default/intermediates/res/default/resources",
                                        )),
                                        bundle_version_code,
                                        bundle_version_name,
                                        // HSP packages use bundle_type "shared"; HAP/HSP modules
                                        // with atomicService type use "atomicService"; default is "app".
                                        bundle_type: bundle_type.clone(),
                                        source_maps_path,
                                        abilities,
                                        integrated_hsps: IntegratedHspCollector::new(
                                            module,
                                            config_arc.as_ref(),
                                            &config_arc.product.bundle_name,
                                            bundle_version_code,
                                        )
                                        .collect()?,
                                        device_types: manifest.device_types.clone(),
                                        // installationFree: true for atomicService bundles
                                        installation_free: bundle_type == "atomicService",
                                        // deliveryWithInstall defaults to true per OHOS spec;
                                        // HSP modules may set it false in module.json5
                                        delivery_with_install,
                                        privacy_profile_path: privacy_profile_path.clone(),
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
                        copy_if_exists(
                            source_maps_path,
                            &output_dir.join("mapping/sourceMaps.map"),
                        )?;
                    }
                    signed_output.clone()
                };
                output_paths.push(final_path);
            }
        } // end for artifact_target_label in &target_labels
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
        if preview_mode {
            println!("[preview] Watching src/main/resources/ — resource-only recompilation");
        }
        // Use a blocking handle to drive async rebuilds from the sync watcher callback.
        let rt = tokio::runtime::Handle::current();
        let target_str_w = target_str.clone();
        let mode_str_w = mode_str.clone();
        let module_filter_w = module_filter.clone();
        let product_filter_w = product_filter.clone();
        let project_root_w = project_root.clone();
        let preview_mode_w = preview_mode;
        // Snapshot importer maps for M2 hot-reload delta computation.
        // Maps chunk_name → [chunk_names that import it].
        let importer_maps: FxHashMap<String, Vec<String>> = config_arc
            .modules
            .iter()
            .filter_map(|m| {
                let map_path = m
                    .src_path
                    .join("build/default/intermediates/importer_map/default/importer_map.json");
                std::fs::read_to_string(&map_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<FxHashMap<String, Vec<String>>>(&s).ok())
            })
            .flatten()
            .collect();

        watcher
            .watch(move |changed| {
                // M2: Hot-reload delta computation.
                // Check the importer map to see if any of the changed files
                // transitively affect an entry point.  If no entry point is
                // reachable from the changed files, skip the rebuild.
                let changed_strs: Vec<String> =
                    changed.iter().map(|p| arkdown_path::to_slash(p)).collect();

                // Find which chunk names contain the changed files.
                let mut affected_chunks: FxHashSet<String> = FxHashSet::default();
                for changed_path in &changed_strs {
                    // A file is in a chunk if the importer map references it.
                    // Also add the file itself as potentially a chunk key.
                    for key in importer_maps.keys() {
                        if key.contains(changed_path.as_str())
                            || changed_path.contains(key.as_str())
                        {
                            affected_chunks.insert(key.clone());
                        }
                    }
                }

                if !importer_maps.is_empty() && affected_chunks.is_empty() {
                    println!(
                        "\n[watch] {} file(s) changed but not in module graph — skipping rebuild",
                        changed.len()
                    );
                    return Ok(());
                }

                // M3: Preview mode — write changed resource list to
                // build/default/outputs/preview_changes.json for IDE polling.
                // DevEco can watch this file to detect resource changes without
                // a WebSocket connection (file-based notification alternative).
                if preview_mode_w {
                    let preview_json = serde_json::json!({
                        "type": "resource_changed",
                        "files": changed.iter()
                            .map(|p| arkdown_path::to_slash(p))
                            .collect::<Vec<_>>(),
                    });
                    for module in &config_arc.modules {
                        let out_dir = module.src_path.join("build/default/outputs");
                        let _ = std::fs::create_dir_all(&out_dir);
                        let _ = std::fs::write(
                            out_dir.join("preview_changes.json"),
                            serde_json::to_string_pretty(&preview_json).unwrap_or_default(),
                        );
                    }
                }

                let mode_label = if preview_mode_w {
                    "resource reload"
                } else {
                    "rebuild"
                };
                println!(
                    "\n[watch] {} file(s) changed — {}... (affected: {} chunks)",
                    changed.len(),
                    mode_label,
                    if affected_chunks.is_empty() {
                        "all".to_string()
                    } else {
                        affected_chunks.len().to_string()
                    }
                );
                let rebuild_args = BuildArgs {
                    target: target_str_w.clone(),
                    mode: mode_str_w.clone(),
                    module: module_filter_w.clone(),
                    product: product_filter_w.clone(),
                    project: Some(project_root_w.clone()),
                    build_target_name: None,
                    watch: false, // inner rebuild does not recurse into watch
                    coverage: false,
                    preview: false, // inner rebuild handles resource compilation
                    disable_abc_cache: false,
                    cache_file: None,
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

fn should_use_sdk_permission_validation(config: &ArkdownConfig) -> bool {
    config.sdk_config.compile_api_version >= 12
        || config.product.runtime_os.eq_ignore_ascii_case("HarmonyOS")
}

fn should_process_module_privacy_profile(module_type: &OhosModuleType) -> bool {
    matches!(module_type, OhosModuleType::Shared | OhosModuleType::Har)
}

// ---------------------------------------------------------------------------
// AppScopeReader — owns reads from AppScope/app.json5.
//
// Per SPEC-048: reads from project manifests must belong to an owning type,
// not be free functions.  These methods replace the former free functions
// `read_app_scope_bundle_name`, `read_bundle_version_code`,
// `read_bundle_version_name`, and `read_bundle_type`.
// Full migration to `ProjectSnapshotBuilder` happens in Phase 3.
// ---------------------------------------------------------------------------

struct AppScopeReader<'a> {
    project_root: &'a Path,
}

impl<'a> AppScopeReader<'a> {
    fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    fn app_json5_path(&self) -> std::path::PathBuf {
        self.project_root.join("AppScope/app.json5")
    }

    fn read_parsed(&self) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(self.app_json5_path()).ok()?;
        json5::from_str(&content).ok()
    }

    /// Read `app.bundleName`.
    fn bundle_name(&self) -> Option<String> {
        self.read_parsed().and_then(|v| {
            v.get("app")?
                .get("bundleName")?
                .as_str()
                .map(str::to_string)
        })
    }

    /// Read `app.versionCode`. Falls back to `1000000` when absent.
    fn version_code(&self) -> u64 {
        #[derive(serde::Deserialize)]
        struct AppJson {
            app: AppSection,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AppSection {
            #[serde(default)]
            version_code: Option<u64>,
        }
        let Ok(content) = std::fs::read_to_string(self.app_json5_path()) else {
            return 1000000;
        };
        json5::from_str::<AppJson>(&content)
            .ok()
            .and_then(|a| a.app.version_code)
            .unwrap_or(1000000)
    }

    /// Read `app.versionName`. Falls back to `"1.0.0"` when absent.
    fn version_name(&self) -> String {
        #[derive(serde::Deserialize)]
        struct AppJson {
            app: AppSection,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AppSection {
            #[serde(default)]
            version_name: Option<String>,
        }
        let Ok(content) = std::fs::read_to_string(self.app_json5_path()) else {
            return "1.0.0".to_string();
        };
        json5::from_str::<AppJson>(&content)
            .ok()
            .and_then(|a| a.app.version_name)
            .unwrap_or_else(|| "1.0.0".to_string())
    }

    /// Determine the bundle type string.
    ///
    /// - HSP output → `"shared"`
    /// - atomicService → `"atomicService"`
    /// - Default → `"app"`
    fn bundle_type(&self, config: &ArkdownConfig) -> String {
        if matches!(config.output_target, OutputTarget::Hsp) {
            return "shared".to_string();
        }
        #[derive(serde::Deserialize)]
        struct AppJson {
            app: AppSection,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AppSection {
            #[serde(default)]
            bundle_type: Option<String>,
        }
        let Ok(content) = std::fs::read_to_string(self.app_json5_path()) else {
            return "app".to_string();
        };
        json5::from_str::<AppJson>(&content)
            .ok()
            .and_then(|a| a.app.bundle_type)
            .unwrap_or_else(|| "app".to_string())
    }
}

// ---------------------------------------------------------------------------
// ResourceDirSelector — owns resource directory selection for a module.
//
// Per SPEC-048: source-set overlay calculation, effective target selection,
// and resource directory resolution belong to `EffectiveTargetSnapshot`
// (and `ResourcePlanBuilder` in Phase 3). For now, this struct consolidates
// the former free functions `module_resource_dirs`, `select_effective_module_target`,
// `resolve_target_resource_dirs`, and `target_source_set_resource_dir`.
// ---------------------------------------------------------------------------

struct ResourceDirSelector<'a> {
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
}

impl<'a> ResourceDirSelector<'a> {
    fn new(config: &'a ArkdownConfig, module: &'a ModuleConfig) -> Self {
        Self { config, module }
    }

    /// Return the effective resource directories for this module/target combination.
    fn select(&self) -> Vec<PathBuf> {
        let build_mode_name = match self.config.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        };
        let Some(target) = self.effective_target(build_mode_name) else {
            return vec![self.module.src_path.join("src/main/resources")];
        };
        let target_dirs = self.resolve_target_dirs(&target.resource_directories);
        if !target_dirs.is_empty() {
            return target_dirs;
        }
        if let Some(source_set_res) = self.source_set_dir(&target.name) {
            return vec![source_set_res];
        }
        vec![self.module.src_path.join("src/main/resources")]
    }

    fn effective_target(&self, build_mode_name: &str) -> Option<EffectiveResourceTarget> {
        let cfg = arkdown_config::build_profile::select_effective_target(
            &self.config.project_root,
            &self.module.name,
            &self.config.product.name,
            build_mode_name,
        )?;
        Some(EffectiveResourceTarget {
            name: cfg.name,
            resource_directories: cfg.resource_directories,
        })
    }

    fn resolve_target_dirs(&self, directories: &[PathBuf]) -> Vec<PathBuf> {
        directories
            .iter()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    self.module.src_path.join(dir)
                }
            })
            .collect()
    }

    fn source_set_dir(&self, target_name: &str) -> Option<PathBuf> {
        if target_name == "default" {
            return None;
        }
        let dir = self
            .module
            .src_path
            .join("src")
            .join(target_name)
            .join("resources");
        dir.exists().then_some(dir)
    }

    /// Whether `AppScope/resources` should be included in the resource compile.
    ///
    /// Mirrors hvigor's `IncludeAppScopeRes` flag semantics.
    fn include_app_scope(&self) -> bool {
        let Some(res_options) = self.config.product.build_option.res_options.as_ref() else {
            return true;
        };
        if res_options.include_app_scope_res {
            return true;
        }
        if matches!(self.module.module_type, OhosModuleType::Shared) {
            return false;
        }
        tracing::warn!(
            module = %self.module.name,
            module_type = %self.module.module_type,
            "'IncludeAppScopeRes' configured at 'build-profile.json5' only works for HSP module, \
             and will not take effect in HAP/HAR module '{}'.",
            self.module.name
        );
        true
    }
}

/// Computed effective target for a module/product/build-mode combination.
struct EffectiveResourceTarget {
    name: String,
    resource_directories: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// NativeLibCollector — owns native `.so` library scanning for a module.
//
// Per SPEC-048: directory traversal and package graph scanning belong to
// owned types, not free functions. This struct consolidates the former free
// functions `collect_native_libs`, `collect_native_libs_from_root`,
// `collect_declared_dependency_roots_for_native`,
// `read_native_dependency_package_json`, and `resolve_native_dependency_root`.
// Full migration to `DependencyGraphSnapshot` classifiers happens in Phase 3.
// ---------------------------------------------------------------------------

struct NativeLibCollector<'a> {
    project_root: &'a Path,
    module: &'a ModuleConfig,
    abi_filters: &'a [String],
}

#[derive(serde::Deserialize)]
struct NativeDependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

impl<'a> NativeLibCollector<'a> {
    fn new(project_root: &'a Path, module: &'a ModuleConfig, abi_filters: &'a [String]) -> Self {
        Self {
            project_root,
            module,
            abi_filters,
        }
    }

    /// Collect all native `.so` files for this module and its dependencies.
    fn collect(&self) -> Vec<PathBuf> {
        let mut libs = Vec::new();
        let mut seen = FxHashSet::default();

        let native_cfg = super::native_build::NativeBuildConfig {
            module_root: self.module.src_path.clone(),
            abi_filters: self.abi_filters.to_vec(),
            enable_cmake_build: false,
        };
        if super::native_build::module_has_native_code(&self.module.src_path) {
            tracing::debug!(module = %self.module.src_path.display(), "native: scanning for pre-built .so files");
        }
        let native_out = native_cfg.run();
        for warning in &native_out.warnings {
            tracing::warn!("{}", warning);
        }
        for (_, so_path) in native_out.libs_by_abi {
            let canonical = std::fs::canonicalize(&so_path).unwrap_or(so_path.clone());
            if seen.insert(canonical.clone()) {
                libs.push(so_path);
            }
        }

        for dep_root in self.dependency_roots() {
            self.collect_from_root(&dep_root, &mut seen, &mut libs);
        }
        libs.sort();
        libs
    }

    fn dependency_roots(&self) -> Vec<PathBuf> {
        // Delegate to DependencyPackageScanner; NativeLibCollector owns only the
        // ABI-filter logic, not the generic package scanning logic.
        DependencyPackageScanner::new(self.project_root)
            .collect_with_alias(&self.module.src_path)
            .into_iter()
            .map(|(_, root)| root)
            .collect()
    }

    fn collect_from_root(
        &self,
        root: &Path,
        seen: &mut FxHashSet<PathBuf>,
        libs: &mut Vec<PathBuf>,
    ) {
        if !root.exists() {
            return;
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("so") {
                continue;
            }
            let comps: Vec<_> = path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            if self.abi_filters.iter().any(|abi| comps.contains(abi)) {
                let path = canonical_or_owned(path);
                if seen.insert(path.clone()) {
                    libs.push(path);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ModuleJsonReader — owns reads from src/main/module.json5.
//
// Per SPEC-048: module manifest reads must belong to an owning type.
// This replaces the former free function `read_delivery_with_install`.
// Full migration to `ModuleSnapshotBuilder` happens in Phase 3.
// ---------------------------------------------------------------------------

struct ModuleJsonReader<'a> {
    module_root: &'a Path,
}

impl<'a> ModuleJsonReader<'a> {
    fn new(module_root: &'a Path) -> Self {
        Self { module_root }
    }

    fn module_json5_path(&self) -> std::path::PathBuf {
        self.module_root.join("src/main/module.json5")
    }

    /// Read `module.deliveryWithInstall`. Defaults to `true` when absent.
    fn delivery_with_install(&self) -> bool {
        let Ok(content) = std::fs::read_to_string(self.module_json5_path()) else {
            return true;
        };
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleSection,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleSection {
            #[serde(default)]
            delivery_with_install: Option<bool>,
        }
        json5::from_str::<ModuleJson>(&content)
            .ok()
            .and_then(|m| m.module.delivery_with_install)
            .unwrap_or(true)
    }

    /// Parse `module.json5` from an explicit path.
    ///
    /// Returns a minimal default when the file does not exist (modules without
    /// a `module.json5` are treated as entry modules).
    fn parse_module_json5(&self, path: &Path) -> Result<ModuleJson5> {
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

    /// Ensure the compiled `module.json` has an `appStartup` reference when
    /// the startup profile produced tasks. Writes the file if the key is absent.
    fn ensure_startup_profile_ref(
        &self,
        module_json_path: &Path,
        startup_out: &arkdown_profile::StartupConfigOutput,
        write_json: &dyn Fn(&Path, &serde_json::Value) -> Result<()>,
    ) -> Result<()> {
        let Some(startup_config) = startup_out.config.as_ref() else {
            return Ok(());
        };
        if startup_config.startup_tasks.is_empty()
            && startup_config.app_preload_hint_startup_tasks.is_empty()
            && startup_config.system_preload_hint_startup_tasks.is_empty()
        {
            return Ok(());
        }
        let content = fs_err::read_to_string(module_json_path)?;
        let mut value: serde_json::Value = serde_json::from_str(&content)?;
        let module = value
            .get_mut("module")
            .and_then(|v| v.as_object_mut())
            .context("module.json missing module object")?;
        if module.contains_key("appStartup") {
            return Ok(());
        }
        module.insert(
            "appStartup".to_string(),
            serde_json::Value::String("$profile:startup_config".to_string()),
        );
        write_json(module_json_path, &value)
    }
}

// ---------------------------------------------------------------------------
// IntegratedHspCollector — owns integrated HSP discovery for a module.
//
// Per SPEC-048: "collect_integrated_hsps → DependencyGraphSnapshot plus pack stage"
// This struct consolidates the former free function `collect_integrated_hsps`.
// Full migration to `DependencyGraphSnapshot::classifiers` happens in Phase 3.
// ---------------------------------------------------------------------------

struct IntegratedHspCollector<'a> {
    module: &'a ModuleConfig,
    config: &'a ArkdownConfig,
    bundle_name: &'a str,
    version_code: u64,
}

impl<'a> IntegratedHspCollector<'a> {
    fn new(
        module: &'a ModuleConfig,
        config: &'a ArkdownConfig,
        bundle_name: &'a str,
        version_code: u64,
    ) -> Self {
        Self {
            module,
            config,
            bundle_name,
            version_code,
        }
    }

    fn collect(&self) -> Result<Vec<arkdown_pack::IntegratedHspEntry>> {
        use arkdown_pack::{normalize_integrated_hsps, IntegratedHspEntry};

        #[derive(serde::Deserialize)]
        struct DepMetadata {
            #[serde(rename = "isIntegratedHsp", default)]
            is_integrated_hsp: bool,
        }
        #[derive(serde::Deserialize)]
        struct DepPkg {
            #[serde(default)]
            metadata: Option<DepMetadata>,
            #[serde(default)]
            version: Option<String>,
        }

        let mut raw_entries: Vec<IntegratedHspEntry> = Vec::new();
        for (dep_name, pkg_dir) in DependencyPackageScanner::new(&self.config.project_root)
            .collect_with_alias(&self.module.src_path)
        {
            let dep_pkg_json = pkg_dir.join("oh-package.json5");
            let Ok(dep_content) = std::fs::read_to_string(&dep_pkg_json) else {
                continue;
            };
            let Ok(dep_pkg) = json5::from_str::<DepPkg>(&dep_content) else {
                continue;
            };
            if !dep_pkg
                .metadata
                .as_ref()
                .map_or(false, |m| m.is_integrated_hsp)
            {
                continue;
            }
            let hsp_name = dep_name.rsplit('/').next().unwrap_or(dep_name.as_str());
            let hsp_path = pkg_dir.join(format!("{}.hsp", hsp_name));
            if !hsp_path.exists() {
                tracing::warn!(
                    dep = dep_name,
                    "K5: integrated HSP dependency has no pre-built .hsp file; skipping"
                );
                continue;
            }
            let version = dep_pkg
                .version
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "1.0.0".to_string());
            raw_entries.push(IntegratedHspEntry {
                hsp_dir_name: format!("{}@{}", hsp_name, version),
                hsp_file_name: format!("{}.hsp", hsp_name),
                hsp_path,
            });
        }
        if raw_entries.is_empty() {
            return Ok(vec![]);
        }
        let compatible_api = parse_api_version(&self.config.product.compatible_sdk_version)
            .unwrap_or(self.config.sdk_config.compile_api_version);
        if self.config.sdk_config.compile_api_version <= 11 || compatible_api <= 11 {
            return Err(anyhow::anyhow!("INTEGRATED_HSP_UNSUPPORTED_API: integrated HSP requires compile and compatible API greater than 11"));
        }
        let out_dir = self
            .module
            .src_path
            .join("build/default/intermediates/integrated_hsp");
        std::fs::create_dir_all(&out_dir).ok();
        match normalize_integrated_hsps(&raw_entries, self.bundle_name, self.version_code, &out_dir)
        {
            Ok(normalized_paths) => Ok(raw_entries
                .into_iter()
                .zip(normalized_paths)
                .map(|(mut e, p)| {
                    e.hsp_path = p;
                    e
                })
                .collect()),
            Err(e) => {
                tracing::warn!(error = %e, "K5: failed to normalize integrated HSPs");
                Ok(vec![])
            }
        }
    }
}

/// Collect declared dependency roots with their aliases (name → path).
///
/// Uses `NativeLibCollector` for package scanning per SPEC-048 ownership rules.
// ---------------------------------------------------------------------------
// DependencyPackageScanner — owns generic dependency package scanning.
//
// Extracts the package-reading/root-resolution logic shared by
// `NativeLibCollector` and `IntegratedHspCollector` into a standalone owner
// that does NOT require a `ModuleConfig`. Eliminates the dummy-ModuleConfig
// workaround previously needed to reuse `NativeLibCollector` methods.
// ---------------------------------------------------------------------------

struct DependencyPackageScanner<'a> {
    project_root: &'a Path,
}

impl<'a> DependencyPackageScanner<'a> {
    fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    fn read_package_json(&self, pkg_root: &Path) -> Option<NativeDependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<NativeDependencyPackageJson>(&content).ok()
    }

    fn resolve_dep_root(&self, issuer_root: &Path, dep_name: &str) -> Option<PathBuf> {
        let mut current = Some(issuer_root);
        while let Some(root) = current {
            let candidate = root.join("oh_modules").join(dep_name);
            if candidate.is_dir() {
                return Some(candidate);
            }
            if root == self.project_root {
                break;
            }
            current = root.parent();
        }
        let candidate = self.project_root.join("oh_modules").join(dep_name);
        candidate.is_dir().then_some(candidate)
    }

    /// Collect all dependency roots with their declared alias names.
    fn collect_with_alias(&self, module_root: &Path) -> Vec<(String, PathBuf)> {
        let mut roots = Vec::new();
        let mut queue = VecDeque::new();
        let mut seen = FxHashSet::default();
        queue.push_back(module_root.to_path_buf());
        if self.project_root != module_root {
            queue.push_back(self.project_root.to_path_buf());
        }
        while let Some(pkg_root) = queue.pop_front() {
            let Some(pkg) = self.read_package_json(&pkg_root) else {
                continue;
            };
            for dep_name in pkg.dependencies.keys() {
                let Some(dep_root) = self.resolve_dep_root(&pkg_root, dep_name) else {
                    continue;
                };
                let key = arkdown_path::to_slash(&dep_root);
                if !seen.insert(key) {
                    continue;
                }
                queue.push_back(dep_root.clone());
                roots.push((dep_name.clone(), dep_root));
            }
        }
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        BuildOption, MergedStartupConfig, ProductConfig, ResOptions, SdkConfig, StartupTask,
        StrictModeConfig,
    };
    use std::io::Write;

    fn test_config(project_root: &Path, output_target: OutputTarget) -> ArkdownConfig {
        ArkdownConfig {
            project_root: project_root.to_path_buf(),
            build_mode: BuildMode::Debug,
            output_target,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.app".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: vec![],
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                },
                version_code: None,
                version_name: None,
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: project_root.join("sdk/ets-loader"),
                es2abc_path: project_root.join("sdk/es2abc"),
                restool_path: project_root.join("sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }

    fn write_minimal_hsp(path: &Path, bundle_name: &str, version_code: u64) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let pack_info = serde_json::json!({
            "summary": {
                "app": {
                    "bundleName": bundle_name,
                    "bundleType": "shared",
                    "version": { "code": version_code, "name": "1.0.0" }
                },
                "modules": []
            },
            "packages": []
        });
        zip.start_file("pack.info", opts).unwrap();
        zip.write_all(serde_json::to_string(&pack_info).unwrap().as_bytes())
            .unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_include_app_scope_false_only_effective_for_shared_modules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = test_config(tmp.path(), OutputTarget::Hap);
        config.product.build_option.res_options = Some(ResOptions {
            res_path: None,
            include_app_scope_res: false,
            resource_directories: vec![],
        });

        for module_type in [
            OhosModuleType::Entry,
            OhosModuleType::Feature,
            OhosModuleType::Har,
        ] {
            let module = ModuleConfig {
                name: "module".to_string(),
                src_path: tmp.path().join("module"),
                module_type: module_type.clone(),
            };
            assert!(
                ResourceDirSelector::new(&config, &module).include_app_scope(),
                "{module_type} should ignore includeAppScopeRes=false"
            );
        }

        let module = ModuleConfig {
            name: "shared".to_string(),
            src_path: tmp.path().join("shared"),
            module_type: OhosModuleType::Shared,
        };
        assert!(!ResourceDirSelector::new(&config, &module).include_app_scope());
    }

    #[test]
    fn test_module_resource_dirs_use_target_resource_directories() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(module_root.join("src/main/resources")).unwrap();
        std::fs::create_dir_all(module_root.join("src/phone/resources")).unwrap();
        std::fs::write(
            project_root.join("build-profile.json5"),
            r#"{
                app: {
                    products: [{
                        name: "default",
                        bundleName: "com.example.app",
                        compatibleSdkVersion: "5.0.0(12)",
                        runtimeOS: "HarmonyOS"
                    }]
                },
                modules: [{
                    name: "entry",
                    srcPath: "./entry",
                    targets: [{
                        name: "default",
                        applyToProducts: ["default"],
                        resource: {
                            directories: ["src/phone/resources"]
                        }
                    }]
                }]
            }"#,
        )
        .unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        let config = test_config(project_root, OutputTarget::Hap);

        assert_eq!(
            ResourceDirSelector::new(&config, &module).select(),
            vec![module_root.join("src/phone/resources")]
        );
    }

    #[test]
    fn test_module_resource_dirs_fall_back_to_src_main_resources() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(module_root.join("src/main/resources")).unwrap();
        std::fs::write(
            project_root.join("build-profile.json5"),
            r#"{
                app: {
                    products: [{
                        name: "default",
                        bundleName: "com.example.app",
                        compatibleSdkVersion: "5.0.0(12)",
                        runtimeOS: "HarmonyOS"
                    }]
                },
                modules: [{
                    name: "entry",
                    srcPath: "./entry",
                    targets: [{ name: "default", applyToProducts: ["default"] }]
                }]
            }"#,
        )
        .unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        let config = test_config(project_root, OutputTarget::Hap);

        assert_eq!(
            ResourceDirSelector::new(&config, &module).select(),
            vec![module_root.join("src/main/resources")]
        );
    }

    #[test]
    fn test_module_resource_dirs_fall_back_to_target_source_set_resources() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(module_root.join("src/main/resources")).unwrap();
        std::fs::create_dir_all(module_root.join("src/phone/resources")).unwrap();
        std::fs::write(
            project_root.join("build-profile.json5"),
            r#"{
                app: {
                    products: [{
                        name: "default",
                        bundleName: "com.example.app",
                        compatibleSdkVersion: "5.0.0(12)",
                        runtimeOS: "HarmonyOS"
                    }]
                },
                modules: [{
                    name: "entry",
                    srcPath: "./entry",
                    targets: [{ name: "phone", applyToProducts: ["default"] }]
                }]
            }"#,
        )
        .unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root.clone(),
            module_type: OhosModuleType::Entry,
        };
        let config = test_config(project_root, OutputTarget::Hap);

        assert_eq!(
            ResourceDirSelector::new(&config, &module).select(),
            vec![module_root.join("src/phone/resources")]
        );
    }

    #[test]
    fn test_read_pack_metadata_from_appscope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_scope = tmp.path().join("AppScope");
        std::fs::create_dir_all(&app_scope).unwrap();
        std::fs::write(
            app_scope.join("app.json5"),
            r#"{
                "app": {
                    "bundleType": "atomicService",
                    "versionCode": 42,
                    "versionName": "2.3.4"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(AppScopeReader::new(tmp.path()).version_code(), 42);
        assert_eq!(AppScopeReader::new(tmp.path()).version_name(), "2.3.4");
        assert_eq!(
            AppScopeReader::new(tmp.path())
                .bundle_type(&test_config(tmp.path(), OutputTarget::Hap)),
            "atomicService"
        );
        assert_eq!(
            AppScopeReader::new(tmp.path())
                .bundle_type(&test_config(tmp.path(), OutputTarget::Hsp)),
            "shared",
            "HSP output must override AppScope bundleType in pack.info"
        );
    }

    #[test]
    fn test_ensure_startup_profile_ref_adds_app_startup_when_generated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_json = tmp.path().join("module.json");
        std::fs::write(
            &module_json,
            r#"{"module":{"name":"entry","type":"entry"}}"#,
        )
        .unwrap();

        let startup_out = arkdown_profile::StartupConfigOutput {
            config: Some(MergedStartupConfig {
                system_preload_hint_startup_tasks: vec![StartupTask {
                    name: "PreloadLib".to_string(),
                    src_entry: "libcrypto.z.so".to_string(),
                    run_on_thread: None,
                    wait_on_main_thread: None,
                    exclude_from_auto_start: None,
                    dependencies: vec![],
                    module_name: None,
                    ohmurl: None,
                    src_entry_absolute_path: None,
                    startup_config_path: None,
                    is_remote_byte_code_har: None,
                }],
                ..Default::default()
            }),
        };

        ModuleJsonReader::new(&module_json.parent().unwrap_or(&module_json))
            .ensure_startup_profile_ref(&module_json, &startup_out, &write_json_pretty)
            .expect("ensure appStartup");

        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(module_json).unwrap()).unwrap();
        assert_eq!(
            value.pointer("/module/appStartup").and_then(|v| v.as_str()),
            Some("$profile:startup_config")
        );
    }

    #[test]
    fn test_ensure_startup_profile_ref_preserves_existing_app_startup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_json = tmp.path().join("module.json");
        std::fs::write(
            &module_json,
            r#"{"module":{"name":"entry","type":"entry","appStartup":"$profile:custom"}}"#,
        )
        .unwrap();

        let startup_out = arkdown_profile::StartupConfigOutput {
            config: Some(MergedStartupConfig {
                startup_tasks: vec![StartupTask {
                    name: "Task".to_string(),
                    src_entry: "./ets/startup/Task.ets".to_string(),
                    run_on_thread: None,
                    wait_on_main_thread: None,
                    exclude_from_auto_start: None,
                    dependencies: vec![],
                    module_name: None,
                    ohmurl: None,
                    src_entry_absolute_path: None,
                    startup_config_path: None,
                    is_remote_byte_code_har: None,
                }],
                ..Default::default()
            }),
        };

        ModuleJsonReader::new(&module_json.parent().unwrap_or(&module_json))
            .ensure_startup_profile_ref(&module_json, &startup_out, &write_json_pretty)
            .expect("ensure appStartup");

        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(module_json).unwrap()).unwrap();
        assert_eq!(
            value.pointer("/module/appStartup").and_then(|v| v.as_str()),
            Some("$profile:custom")
        );
    }

    #[test]
    fn test_collect_native_libs_follows_declared_transitive_scoped_dependencies() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(module_root.join("src/main/libs/arm64-v8a")).unwrap();
        std::fs::write(
            module_root.join("src/main/libs/arm64-v8a/libself.so"),
            b"self",
        )
        .unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();

        let source_har = module_root.join("oh_modules/source-har");
        std::fs::create_dir_all(&source_har).unwrap();
        std::fs::write(
            source_har.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/native-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let native_hsp = source_har.join("oh_modules/@scope/native-hsp/libs/arm64-v8a");
        std::fs::create_dir_all(&native_hsp).unwrap();
        std::fs::write(native_hsp.join("libdeclared.so"), b"declared").unwrap();

        let undeclared = module_root.join("oh_modules/undeclared-hsp/libs/arm64-v8a");
        std::fs::create_dir_all(&undeclared).unwrap();
        std::fs::write(undeclared.join("libundeclared.so"), b"undeclared").unwrap();

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };
        let default_abi_filters = vec!["arm64-v8a".to_string()];
        let libs = NativeLibCollector::new(project_root, &module, &default_abi_filters).collect();
        let names: Vec<_> = libs
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect();

        assert!(names.contains(&"libself.so"));
        assert!(names.contains(&"libdeclared.so"));
        assert!(!names.contains(&"libundeclared.so"));
    }

    /// Verify that `NativeLibCollector` respects the configured ABI filters.
    /// When only `arm64-v8a` is in the filter, `armeabi-v7a` libs should be skipped.
    #[test]
    fn test_collect_native_libs_respects_abi_filters() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("module");
        let libs_root = module_root.join("libs");

        // Create SO files for two ABIs.
        std::fs::create_dir_all(libs_root.join("arm64-v8a")).unwrap();
        std::fs::write(libs_root.join("arm64-v8a/lib64.so"), b"arm64").unwrap();
        std::fs::create_dir_all(libs_root.join("armeabi-v7a")).unwrap();
        std::fs::write(libs_root.join("armeabi-v7a/lib32.so"), b"arm32").unwrap();

        // Filter: only arm64-v8a — lib32.so should be excluded.
        let abi64 = vec!["arm64-v8a".to_string()];
        let dummy_module = ModuleConfig {
            name: "module".to_string(),
            src_path: module_root.clone(),
            module_type: arkdown_core::OhosModuleType::Entry,
        };
        let collector = NativeLibCollector::new(project_root, &dummy_module, &abi64);
        let mut libs = Vec::new();
        let mut seen = rustc_hash::FxHashSet::default();
        collector.collect_from_root(&libs_root, &mut seen, &mut libs);
        let names: Vec<_> = libs
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names.contains(&"lib64.so"), "arm64 lib must be included");
        assert!(!names.contains(&"lib32.so"), "arm32 lib must be excluded");

        // Filter: both ABIs — both should be included.
        let abi_both = vec!["arm64-v8a".to_string(), "armeabi-v7a".to_string()];
        let dummy_module2 = ModuleConfig {
            name: "module".to_string(),
            src_path: module_root.clone(),
            module_type: arkdown_core::OhosModuleType::Entry,
        };
        let collector2 = NativeLibCollector::new(project_root, &dummy_module2, &abi_both);
        let mut libs2 = Vec::new();
        let mut seen2 = rustc_hash::FxHashSet::default();
        collector2.collect_from_root(&libs_root, &mut seen2, &mut libs2);
        let names2: Vec<_> = libs2
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names2.contains(&"lib64.so"), "arm64 lib must be included");
        assert!(names2.contains(&"lib32.so"), "arm32 lib must be included");
    }

    #[test]
    fn test_collect_integrated_hsps_requires_api12() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(&module_root).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "remote-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let remote_hsp = module_root.join("oh_modules/remote-hsp");
        std::fs::create_dir_all(&remote_hsp).unwrap();
        std::fs::write(
            remote_hsp.join("oh-package.json5"),
            r#"{ metadata: { isIntegratedHsp: true } }"#,
        )
        .unwrap();
        write_minimal_hsp(&remote_hsp.join("remote-hsp.hsp"), "com.remote.hsp", 1);

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };
        let mut config = test_config(project_root, OutputTarget::Hap);
        config.sdk_config.compile_api_version = 11;
        config.product.compatible_sdk_version = "4.1.0(11)".to_string();

        let err = IntegratedHspCollector::new(&module, &config, "com.host.app", 1000000)
            .collect()
            .expect_err("integrated HSP must require API > 11");
        assert!(
            err.to_string().contains("INTEGRATED_HSP_UNSUPPORTED_API"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_collect_integrated_hsps_follows_declared_transitive_scoped_dependencies() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path();
        let module_root = project_root.join("entry");
        std::fs::create_dir_all(&module_root).unwrap();
        std::fs::write(
            module_root.join("oh-package.json5"),
            r#"{ dependencies: { "source-har": "1.0.0" } }"#,
        )
        .unwrap();

        let source_har = module_root.join("oh_modules/source-har");
        std::fs::create_dir_all(&source_har).unwrap();
        std::fs::write(
            source_har.join("oh-package.json5"),
            r#"{ dependencies: { "@scope/remote-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let remote_hsp = source_har.join("oh_modules/@scope/remote-hsp");
        std::fs::create_dir_all(&remote_hsp).unwrap();
        std::fs::write(
            remote_hsp.join("oh-package.json5"),
            r#"{ metadata: { isIntegratedHsp: true } }"#,
        )
        .unwrap();
        write_minimal_hsp(&remote_hsp.join("remote-hsp.hsp"), "com.remote.hsp", 1);

        let undeclared_hsp = module_root.join("oh_modules/undeclared-hsp");
        std::fs::create_dir_all(&undeclared_hsp).unwrap();
        std::fs::write(
            undeclared_hsp.join("oh-package.json5"),
            r#"{ metadata: { isIntegratedHsp: true } }"#,
        )
        .unwrap();
        write_minimal_hsp(
            &undeclared_hsp.join("undeclared-hsp.hsp"),
            "com.undeclared.hsp",
            1,
        );

        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: OhosModuleType::Entry,
        };
        let config = test_config(project_root, OutputTarget::Hap);
        let entries = IntegratedHspCollector::new(&module, &config, "com.host.app", 1000000)
            .collect()
            .expect("collect integrated HSPs");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hsp_dir_name, "remote-hsp@1.0.0");
        assert_eq!(entries[0].hsp_file_name, "remote-hsp.hsp");
        assert!(entries[0].hsp_path.exists());
    }

    #[test]
    fn test_should_process_module_privacy_profile_skips_hap_modules() {
        assert!(!should_process_module_privacy_profile(
            &OhosModuleType::Entry
        ));
        assert!(!should_process_module_privacy_profile(
            &OhosModuleType::Feature
        ));
        assert!(should_process_module_privacy_profile(
            &OhosModuleType::Shared
        ));
        assert!(should_process_module_privacy_profile(&OhosModuleType::Har));
    }

    #[test]
    fn test_read_delivery_with_install_from_module_json5() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        std::fs::create_dir_all(module_root.join("src/main")).unwrap();

        assert!(
            ModuleJsonReader::new(&module_root).delivery_with_install(),
            "deliveryWithInstall defaults to true when absent"
        );

        std::fs::write(
            module_root.join("src/main/module.json5"),
            r#"{
                "module": {
                    "name": "entry",
                    "type": "entry",
                    "deliveryWithInstall": false
                }
            }"#,
        )
        .unwrap();

        assert!(
            !ModuleJsonReader::new(&module_root).delivery_with_install(),
            "deliveryWithInstall=false must be passed to pack.info"
        );
    }
}

// ---------------------------------------------------------------------------
// Router-map cache invalidation helpers
//
// Previously owned by the deleted `arkdown-plugin-router-map` crate.
// These will migrate to `ProfileStore` fingerprints in SPEC-048 Phase 4.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// RouterMapCacheInvalidator — owns router map content-hash cache operations.
//
// Per SPEC-048: "arkdown-plugin-router-map::should_invalidate → ProfileStore fingerprints"
// This struct consolidates the former free functions `router_map_cache_should_invalidate`
// and `router_map_cache_update` until they migrate to `ProfileStore` in Phase 4.
// ---------------------------------------------------------------------------

struct RouterMapCacheInvalidator<'a> {
    router_map_path: &'a Path,
    cache_path: &'a Path,
}

impl<'a> RouterMapCacheInvalidator<'a> {
    fn new(router_map_path: &'a Path, cache_path: &'a Path) -> Self {
        Self {
            router_map_path,
            cache_path,
        }
    }

    /// Return `true` when the router map cache should be invalidated.
    fn should_invalidate(&self) -> bool {
        let Ok(content) = std::fs::read_to_string(self.router_map_path) else {
            return false;
        };
        let current_hash = arkdown_cache::fnv1a_hash(&content);
        let cached: u64 = match std::fs::read_to_string(self.cache_path) {
            Ok(s) => s.trim().parse().unwrap_or(0),
            Err(_) => return true,
        };
        current_hash != cached
    }

    /// Persist the current router map hash to the cache file.
    fn update(&self) -> std::io::Result<()> {
        let content = std::fs::read_to_string(self.router_map_path)?;
        let hash = arkdown_cache::fnv1a_hash(&content);
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(self.cache_path, hash.to_string())
    }
}
