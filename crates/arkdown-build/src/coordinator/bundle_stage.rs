//! Bundling stage implementation.

use super::*;
use crate::visual_codegen::VisualCodegenAdapter;

struct TransformResourceInventoryLoader {
    dependency_tables: FxHashMap<PathBuf, ResourceReferenceInventory>,
}

impl TransformResourceInventoryLoader {
    fn new() -> Self {
        Self {
            dependency_tables: FxHashMap::default(),
        }
    }

    fn load(
        &mut self,
        loader_json: &LoaderJson,
        module_layout: &ModuleBuildLayout,
    ) -> Result<TransformResourceInventory> {
        let current = ResourceReferenceInventory::from_rawfile_dir(
            &module_layout.res_resources_dir().join("rawfile"),
        )?;
        let mut entries = loader_json.hsp_resources_map.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(name, _)| *name);
        let mut modules = FxHashMap::default();
        for (module_name, resource_table) in entries {
            let resource_table = PathBuf::from(resource_table);
            if !self.dependency_tables.contains_key(&resource_table) {
                let inventory = if resource_table.is_file() {
                    ResourceReferenceInventory::from_resource_table(&resource_table)?
                } else {
                    let rawfiles = resource_table
                        .parent()
                        .map(|parent| parent.join("resources").join("rawfile"))
                        .unwrap_or_default();
                    ResourceReferenceInventory::from_rawfile_dir(&rawfiles)?
                };
                self.dependency_tables
                    .insert(resource_table.clone(), inventory);
            }
            let inventory = self
                .dependency_tables
                .get(&resource_table)
                .expect("resource table was inserted above");
            modules.insert(
                module_name.clone(),
                ResourceModuleReference::new(
                    inventory.resources().clone(),
                    inventory.rawfiles().clone(),
                    inventory.resource_table_available(),
                    inventory.rawfile_root_available(),
                ),
            );
        }
        Ok(TransformResourceInventory {
            rawfiles: Arc::new(current.rawfiles().clone()),
            modules: Arc::new(modules),
        })
    }
}

struct TransformResourceInventory {
    rawfiles: Arc<FxHashSet<String>>,
    modules: Arc<FxHashMap<String, ResourceModuleReference>>,
}

pub(super) struct CoordinatorBundleStageInput<'a> {
    pub(super) args: &'a BuildRequest,
    pub(super) config: &'a Arc<ArkdownConfig>,
    pub(super) session: &'a Arc<BuildSession>,
    pub(super) package_plugins: &'a [PackagePlugin],
    pub(super) collect_importers: bool,
    pub(super) is_incremental_watch: bool,
    pub(super) watch_changed_files: Vec<PathBuf>,
    pub(super) coverage_mode: bool,
    pub(super) disable_abc_cache: bool,
    pub(super) module_analyzer: ModuleAnalyzerKind,
    pub(super) checker_mode: CheckerMode,
    pub(super) project_root: &'a Path,
    pub(super) progress: &'a mut Progress,
    pub(super) custom_cfg: &'a arkdown_config::ArkdownCustomConfig,
    pub(super) intermediate_writer: &'a IntermediateArtifactWriter,
    pub(super) har_dedup_project_plan: &'a HarDedupProjectPlan,
    pub(super) loader_jsons: &'a mut [ModuleBuildRecord],
    pub(super) package_resolution_plans: &'a BTreeMap<String, Arc<PackageResolutionPlan>>,
    pub(super) local_native_dependencies: &'a BTreeMap<String, Arc<[NativeCmakeLocalDependency]>>,
    pub(super) all_bc_har_deps: &'a [Vec<ByteCodeHarScanEntry>],
    pub(super) sdk_declaration_roots: &'a [PathBuf],
    pub(super) component_metadata: &'a ComponentMetadata,
    pub(super) insight_intent_schema_catalog: &'a InsightIntentSchemaCatalog,
    pub(super) bundle_runtime: Option<&'a mut BundleRuntime>,
}

pub(super) struct CoordinatorBundleStageOutput {
    pub(super) bundle_outs: Vec<(BundleStageOutput,)>,
    pub(super) widget_bundle_outs: Vec<Option<WidgetCompileArktsOutput>>,
    pub(super) module_obfuscation_contexts: Vec<ModuleObfuscationContext>,
}

impl BuildCoordinator {
    pub(super) async fn run_bundle_stage(
        &self,
        input: CoordinatorBundleStageInput<'_>,
    ) -> Result<CoordinatorBundleStageOutput> {
        let CoordinatorBundleStageInput {
            args,
            config,
            session,
            package_plugins,
            collect_importers,
            is_incremental_watch,
            watch_changed_files,
            coverage_mode,
            disable_abc_cache,
            module_analyzer,
            checker_mode,
            project_root,
            progress,
            custom_cfg,
            intermediate_writer,
            har_dedup_project_plan,
            loader_jsons,
            package_resolution_plans,
            local_native_dependencies,
            all_bc_har_deps,
            sdk_declaration_roots,
            component_metadata,
            insight_intent_schema_catalog,
            mut bundle_runtime,
        } = input;

        // ── Step 3: Bundling ──────────────────────────────────────────────
        progress.step(3, 6, "Bundling");
        let mut bundle_outs: Vec<(BundleStageOutput,)> = Vec::new();
        let mut widget_bundle_outs: Vec<Option<WidgetCompileArktsOutput>> = Vec::new();
        let mut transform_resource_inventory_loader = TransformResourceInventoryLoader::new();
        let module_obfuscation_contexts = loader_jsons
            .iter()
            .map(|(module, _, _, _, module_target_name, module_config)| {
                ModuleObfuscationContext::resolve(
                    module_config.as_ref(),
                    session,
                    module,
                    &module_config.product.build_option,
                )
                .map(|mut context| {
                    context.target_name = Some(module_target_name.clone());
                    context
                })
            })
            .collect::<Result<Vec<_>>>()?;
        for (
            (module, manifest, loader_json, resource_refs, module_target_name, module_config),
            module_obfuscation,
        ) in loader_jsons.iter().zip(module_obfuscation_contexts.iter())
        {
            let module_build_option = &module_config.product.build_option;
            let module_layout = ModuleBuildLayout::for_output_target_product(
                &module_config.output_target,
                &module.src_path,
                &module_config.product.name,
                module_target_name,
            );
            let quickfix_task_plan = QuickFixTaskPlan::for_action(args.action);
            let hot_reload_task_plan = HotReloadTaskPlan::for_action(args.action);
            let module_package_plugins = package_plugins
                .iter()
                .filter(|plugin| {
                    plugin.applies_to_module_target(&module.src_path, module_target_name)
                })
                .cloned()
                .collect::<Vec<_>>();
            let module_package_resolution =
                package_resolution_plans.get(&module.name).ok_or_else(|| {
                    anyhow::anyhow!(
                        "missing package resolution plan for module '{}'",
                        module.name
                    )
                })?;
            let module_context = session.module_context(&module.name).ok_or_else(|| {
                anyhow::anyhow!("missing build session module context for {}", module.name)
            })?;
            let cache_dir = PathBuf::from(&loader_json.cache_path);
            let plugin_context_cache =
                PluginContextCacheRunner::new(&module_package_plugins, cache_dir.as_path());
            // `this.share.projectConfig` is module-specific in latest hvigor. Seed
            // only the plugins participating in this module before cache hooks or
            // Rolldown hooks execute.
            let loader_json_path = module_layout.loader_json();
            let share = HvigorCompilePluginShareBuilder::new(HvigorCompilePluginShareInput {
                config: module_config.as_ref(),
                args,
                module,
                session: session.as_ref(),
                module_context: &module_context,
                package_resolution: module_package_resolution.as_ref(),
                loader_json,
                loader_json_path: &loader_json_path,
                target_name: module_target_name,
                coverage_enabled: coverage_mode,
                arkts_language_version: session.arkts_language_version,
            })
            .with_plugin_cache_snapshot(plugin_context_cache.load()?)
            .build()?;
            for plugin in &module_package_plugins {
                plugin.seed_hvigor_context(&share).await?;
            }
            let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
            let output_dir = PathBuf::from(&loader_json.ace_module_build);
            let static_interop_request = module_layout
                .arkts_interop_dir()
                .join("declgen-v2-request.json");
            let versioned_cached_abc =
                output_dir.join(session.arkts_language_version.merged_abc_file_name());
            // `BaseMode::mergeAbcFiles` intentionally emits no
            // `modules_static.abc` for a 1.2 product containing only dynamic
            // sources. In that case the dynamic merged ABC is the primary
            // cache artifact. A static or hybrid module still selects the
            // versioned static output.
            let cached_abc = if session.arkts_language_version.is_static()
                && !versioned_cached_abc.is_file()
                && static_interop_request.is_file()
                && output_dir.join("modules.abc").is_file()
            {
                output_dir.join("modules.abc")
            } else {
                versioned_cached_abc
            };
            if !BuildPolicy::new(module_config.as_ref())
                .module_requires_arkts_compilation(module)?
            {
                bundle_outs.push((BundleStageOutput::skipped_original_har(
                    cached_abc, output_dir,
                ),));
                widget_bundle_outs.push(None);
                continue;
            }
            if session.arkts_language_version == arkdown_core::ArktsLanguageVersion::V1_1 {
                let ace_module_root = PathBuf::from(&loader_json.module_root_path).join("ets");
                let checker_roots =
                    crate::compile_project_config::CompileProjectEntryCollector::new(
                        loader_json,
                        &ace_module_root,
                    )
                    .collect();
                let semantic = crate::project_checker::ProjectSemanticChecker::new(
                    project_root,
                    &checker_roots,
                    module_package_resolution.as_ref(),
                )
                .with_sdk_declaration_roots(&session.source_store, sdk_declaration_roots)
                .run()?;
                if semantic.errors > 0 {
                    anyhow::bail!(
                        "ARKDOWN_ETS_CHECK_FAILED: {} semantic error(s) in module '{}':\n{}",
                        semantic.errors,
                        module.name,
                        semantic.diagnostics.join("\n")
                    );
                }
            }
            let widget_pass = WidgetCompileArktsPass::new(
                module_config.as_ref(),
                module,
                manifest,
                loader_json,
                &module_context.deps,
            );
            // Static `BaseMode` builds one dependency graph from the complete
            // `compileFiles` inventory and links it into `modules_static.abc`.
            // The separate `widgets.abc` pass belongs to ets2abc's dynamic
            // frontend and must not run alongside the static SDK driver.
            let widget_compile_plan = if session.arkts_language_version.is_static() {
                None
            } else {
                widget_pass.plan()?
            };
            let task_plugin_hash =
                module_package_plugins
                    .iter()
                    .fold(0xcbf29ce484222325u64, |mut hash, plugin| {
                        for byte in plugin.cache_identity().bytes() {
                            hash ^= u64::from(byte);
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                        hash
                    });
            let task_input_snapshot = TaskInputSnapshot::for_module(session.as_ref(), &module.name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "missing session input fingerprints for module '{}'",
                        module.name
                    )
                })?;
            let bundle_compile_env = CompileEnvKeyBuilder::new(
                module_config.as_ref(),
                module,
                loader_json,
                module_obfuscation.is_enabled(&config.build_mode),
                module_target_name,
                args.hvigor_properties.rollup_cache_use_source_hash,
                self.session.arkts_language_version,
            )
            .with_preview_mode(
                args.preview
                    || args.preview_transform_mode
                    || matches!(config.output_target, arkdown_core::OutputTarget::Preview),
            )
            .with_check_entry(args.check_entry)
            .with_crossplatform(args.is_crossplatform, args.ignore_crossplatform_check)
            .with_ui_transform_optimization(args.ui_transform_optimization)
            .with_use_get_current_bundle_name(args.integrated_hsp_use_get_current_bundle_name)
            .build()?;
            let task_inputs =
                TaskInputs::compute(module_config.as_ref(), loader_json, &task_input_snapshot)
                    .with_module_analyzer(module_analyzer)
                    .with_arkts_language_version(self.session.arkts_language_version)
                    .with_effective_obfuscation(
                        module_obfuscation.config.as_ref(),
                        &config.build_mode,
                    )
                    .with_compile_env(bundle_compile_env.clone())
                    .with_plugin_hash(task_plugin_hash)
                    .with_additional_obfuscation_rule_files(
                        &module_obfuscation.dependency_rule_files,
                    )
                    .with_custom_type_inputs(
                        &loader_json.compiler_types,
                        &loader_json.compiler_type_declaration_inputs,
                    )?;

            // Compute deterministic cache-group snapshots for this module.
            // Mirrors hvigor's invalid-cache-plugin.ts group tracking.
            let group_snapshots = build_cache_group_snapshots(
                &bundle_compile_env,
                module_config.as_ref(),
                module,
                loader_json,
                &module_package_plugins,
                project_root,
                args.byte_code_har_integrated_optimization,
            );

            // Build the cache bridge — loads group_keys.json and diffs against current snapshots.
            let cache_bridge = BundleCacheBridge::from_plan(
                bundle_compile_env.clone(),
                Some(cache_dir.clone()),
                group_snapshots.clone(),
            );

            let widget_cache_ready = widget_compile_plan
                .as_ref()
                .is_none_or(|plan| plan.modules_abc_path.exists());
            let declaration_cache_ready = !matches!(module.module_type, OhosModuleType::Shared)
                || module_layout.declarations_dir().is_dir();
            let static_dynamic_abc_cache_ready = !session.arkts_language_version.is_static()
                || !module_layout
                    .arkts_interop_dir()
                    .join("declgen-v2-request.json")
                    .is_file()
                || output_dir.join("modules.abc").is_file();
            let unit_test_coverage_cache_ready =
                !matches!(config.output_target, OutputTarget::Test)
                    || !coverage_mode
                    || module_layout.test_init_coverage_json().is_file();
            let aot_output_dir = module_layout.aot_output_dir();
            let aot_cache_ready = AotArtifactWriter::new(
                &module.name,
                loader_json.an_build_mode.as_deref(),
                loader_json.an_build_out_put.as_deref(),
                loader_json.ap_path.as_deref(),
                &aot_output_dir,
                coverage_mode,
            )
            .cache_ready();
            let mut cache_hit = quickfix_task_plan.is_none()
                && hot_reload_task_plan.is_none()
                && !disable_abc_cache
                && task_inputs.matches_cached(&cache_dir)
                && cached_abc.exists()
                && widget_cache_ready
                && declaration_cache_ready
                && static_dynamic_abc_cache_ready
                && unit_test_coverage_cache_ready
                && aot_cache_ready;
            if cache_hit {
                // System-level check: compile-env + file changes + cache group changes.
                let system_request = ShouldInvalidCacheRequest {
                    compile_env: bundle_compile_env.clone(),
                    changed_files: watch_changed_files.clone(),
                    module_name: Some(module.name.clone()),
                    cache_dir: Some(cache_dir.clone()),
                    is_watch: is_incremental_watch,
                    cache_hit,
                    group_snapshots: group_snapshots.clone(),
                    module_deps: vec![],
                };
                let system_result = cache_bridge.ask_plugins(&system_request);
                if system_result.should_invalidate {
                    tracing::debug!(
                        module = %module.name,
                        reason = ?system_result.reason,
                        global = system_result.global_invalidation,
                        changed_groups = %system_result.changed_groups.explain(),
                        "cache invalidated by system check"
                    );
                    cache_hit = false;
                }
            }
            if cache_hit {
                cache_hit = !PluginCacheInvalidationRunner::new(
                    &module_package_plugins,
                    cache_dir.as_path(),
                )
                .should_invalidate()
                .await?;
            }

            if cache_hit {
                plugin_context_cache.commit().await?;

                let additional_abc_paths = (session.arkts_language_version.is_static()
                    && cached_abc.file_name().and_then(|name| name.to_str())
                        == Some("modules_static.abc")
                    && static_interop_request.is_file())
                .then(|| output_dir.join("modules.abc"))
                .into_iter()
                .collect();
                bundle_outs.push((BundleStageOutput::from_cache(cached_abc, output_dir)
                    .with_additional_abc_paths(additional_abc_paths),));
                widget_bundle_outs.push(None);
                continue;
            }

            // SDK `watchChangedFiles.beforeBuild` writes this handoff before the
            // Rollup transform starts. Preserve the raw event kinds: Hvigor treats
            // updates as modified and create/delete events as removed.
            if hot_reload_task_plan.is_some() {
                if let Some(invalidation) = args.watch_invalidation.as_ref() {
                    HotReloadProcessor::new(&module_layout, project_root)
                        .write_watch_changed_file_list(
                            loader_json.patch_config.as_ref(),
                            &invalidation.raw_events,
                        )?;
                }
            }

            // Clean stale bundled .js files from previous builds.
            if output_dir.exists() {
                for entry in walkdir::WalkDir::new(&output_dir)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }

            let ohos_test_templates_after_migration = module_target_name == "ohosTest"
                && module_context
                    .module
                    .ohos_test_module_json_value
                    .as_deref()
                    .is_some_and(|module_json| {
                        module_layout
                            .source()
                            .ohos_test_templates_after_migration(module_json)
                    });
            let ohos_test_template_layout = ModuleBuildLayout::for_ohos_test_template(
                &module.src_path,
                &module_config.product.name,
            );
            let pkg_context_builder = PkgContextInfoInputBuilder::new(
                module_config.as_ref(),
                session,
                module,
                &module_context,
            )
            .with_local_native_dependencies(
                local_native_dependencies
                    .get(&module.name)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([])),
            )
            .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots());
            let source_map_packages = pkg_context_builder.source_map_package_info()?;
            let runtime_pkg_contexts = PkgContextInfoGenerator::new()
                .generate(&pkg_context_builder.build()?)
                .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
            let runtime_packages =
                BundleRuntimePackageInfoBuilder::new(&runtime_pkg_contexts).build();
            let loader_json_out = module_layout.loader_json();
            let mut module_record_roots = Vec::new();
            for node in module_context.deps.nodes.values() {
                if !matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar
                        | DependencyNodeType::OhpmSource
                        | DependencyNodeType::IntegratedHsp
                ) {
                    continue;
                }
                let Some(package_root) = node.package_root.clone() else {
                    continue;
                };
                module_record_roots.push((node.package_name.clone(), package_root.clone()));
                if node.alias != node.package_name {
                    module_record_roots.push((node.alias.clone(), package_root));
                }
            }
            module_record_roots
                .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            module_record_roots.dedup();

            let mut bundle_plan = BundlePlanBuilder::new(BundlePlanBuilderInput {
                loader_json,
                loader_json_path: &loader_json_out,
                output_dir: PathBuf::from(&loader_json.ace_module_build),
                config: module_config.as_ref(),
                module_target_name,
                custom_config: Some(custom_cfg),
                module_analyzer,
                arkts_language_version: self.session.arkts_language_version,
                should_emit_js: coverage_mode,
                package_info: BundlePlanPackageInfo::from_module_context(
                    source_map_packages,
                    runtime_packages,
                    &module_context,
                ),
            })
            .build();
            bundle_plan.preview_mode |= args.preview || args.preview_transform_mode;
            bundle_plan.check_entry = args.check_entry;
            bundle_plan.cache = arkdown::BundleCachePolicy {
                cache_directory: Some(
                    module_layout
                        .default_cache_dir("bundle")
                        .join("oxc-transform"),
                ),
                read_cache: !disable_abc_cache,
                write_cache: !disable_abc_cache,
                compile_env_key: bundle_compile_env.clone(),
                transform_environment_key: task_inputs.transform_cache_environment_key(),
            };
            if ohos_test_templates_after_migration {
                // The ohosTest module's ets root is the generated template tree
                // (`build/<product>/intermediates/src/ohosTest/ets`); entry page
                // paths resolve against it (`testability/pages/Index`).
                let template_ets = ohos_test_template_layout.ohos_test_ets_root();
                if template_ets.is_dir() {
                    bundle_plan.input.project_path = template_ets;
                }
            }
            if let Some(mock_config) = bundle_plan.input.mock_config.as_mut() {
                mock_config.dependencies = module_context
                    .deps
                    .edges
                    .iter()
                    .filter_map(|edge| {
                        let node = module_context.deps.nodes.get(&edge.package_name)?;
                        let main_file_path = edge.package_root.join(
                            node.package
                                .as_ref()
                                .and_then(|package| package.package_json.main.as_deref())
                                .unwrap_or("index.js"),
                        );
                        Some(BundleMockDependency {
                            dependency_name: edge.alias.clone(),
                            package_name: node.package_name.clone(),
                            package_root: edge.package_root.clone(),
                            main_file_path,
                        })
                    })
                    .collect();
            }
            bundle_plan.input.module_record_roots = module_record_roots;
            // hvigor's widget pass compiles the form card sources into the main
            // es2abc input (`filesInfo.txt`) in addition to `widgets.abc`, but only
            // on the form widget module itself (`needSubmitArkTsWidget`).
            if matches!(module.module_type, OhosModuleType::Shared)
                && Self::module_json5_has_form_extension_module(&module.src_path)
            {
                for form_entry in &loader_json.form_entry_sources {
                    if !bundle_plan.input.entries.contains(form_entry) {
                        bundle_plan.input.entries.push(form_entry.clone());
                    }
                }
            }
            let preload_system_so_enabled = module_target_name != "ohosTest"
                && module_build_option.preload_system_so
                && module.module_type != OhosModuleType::Shared
                && match config.output_target {
                    OutputTarget::Hap => true,
                    OutputTarget::Har => {
                        module_build_option.har_artifact_type
                            == arkdown_core::HarArtifactType::Bytecode
                    }
                    _ => false,
                };
            if preload_system_so_enabled {
                let module_json = module_context.module.module_json.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "module '{}' is missing module.json5 for preloadSystemSo planning",
                        module.name
                    )
                })?;
                bundle_plan.input.preload_system_so_entries = PreloadSystemSoPlan::new(
                    &module.src_path,
                    module_json,
                    module_context.deps.as_ref(),
                )
                .entries()?;
            }

            let static_interop = if session.arkts_language_version.is_static() {
                let loader_json_path = module_layout.loader_json();
                let compiler = StaticArktsCompiler::new(StaticArktsCompileInput {
                    config: module_config.as_ref(),
                    module,
                    manifest,
                    loader_json,
                    module_context: &module_context,
                    module_layout: &module_layout,
                    loader_json_path: &loader_json_path,
                    module_source_files: &[],
                    node_executable: args.resolved_node_executable(),
                    output_abc: &cached_abc,
                    preview_mode: args.preview
                        || args.preview_transform_mode
                        || matches!(config.output_target, OutputTarget::Preview),
                    check_entry: args.check_entry,
                    is_crossplatform: args.is_crossplatform,
                    ui_transform_optimization: args.ui_transform_optimization,
                    bundle_type: &session.project.bundle_type,
                    obfuscation: None,
                })?;
                let interop = compiler.prepare_interop()?;
                interop.redirect_entries(
                    &bundle_plan.input.module_root,
                    &mut bundle_plan.input.entries,
                )?;
                interop.redirect_entries(
                    &bundle_plan.input.module_root,
                    &mut bundle_plan.input.form_entry_sources,
                )?;
                interop.redirect_entries(
                    &bundle_plan.input.module_root,
                    &mut bundle_plan.input.runtime_entry_sources,
                )?;
                interop.redirect_entries(
                    &bundle_plan.input.module_root,
                    &mut bundle_plan.input.page_entry_sources,
                )?;
                Some(interop)
            } else {
                None
            };

            let package_resolution = package_resolution_plans
                .get(&module.name)
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "missing package resolution plan for module '{}'",
                        module.name
                    )
                })?;

            // Compute source-set overlays for build-target overrides.
            let source_set_overlays: Vec<(PathBuf, PathBuf)> = {
                let context_layout = ModuleBuildLayout::new(&module_context.module.root);
                let base_ets = context_layout.source().ets();
                if module_target_name != "default" {
                    let target_ets = context_layout.source().target_ets(module_target_name);
                    if target_ets.is_dir() && target_ets != base_ets {
                        vec![(base_ets, target_ets)]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            };

            let all_module_roots: Vec<(String, PathBuf)> = config
                .modules
                .iter()
                .map(|m| (m.name.clone(), m.src_path.clone()))
                .collect();
            let mut dependency_source_roots: Vec<PathBuf> = module_context
                .deps
                .nodes
                .values()
                .filter(|node| {
                    matches!(
                        node.node_type,
                        DependencyNodeType::SourceHar
                            | DependencyNodeType::OhpmSource
                            | DependencyNodeType::IntegratedHsp
                    )
                })
                .filter_map(|node| node.package_root.clone())
                .collect();
            if let Some(interop) = static_interop
                .as_ref()
                .filter(|interop| interop.requires_dynamic_abc)
            {
                dependency_source_roots.push(interop.bridge_root.clone());
            }
            let native_declaration_roots: Vec<PathBuf> = module_context
                .deps
                .nodes
                .values()
                .filter(|node| matches!(node.node_type, DependencyNodeType::NativeSo))
                .filter_map(|node| node.package_root.clone())
                .collect();
            let transform_snapshot_start = Instant::now();
            let dependency_package_roots: Vec<(String, PathBuf)> = module_context
                .deps
                .nodes
                .values()
                .filter_map(|node| {
                    node.package_root
                        .clone()
                        .map(|root| (node.package_name.clone(), root))
                })
                .collect();
            // Local unit-test builds additionally compile the generated
            // testability templates and the `src/test` suite (hvigor
            // `UnitTestArkTS` inputs; `GenerateUnitTestTemplate` output).
            let test_source_roots: Vec<PathBuf> =
                if matches!(config.output_target, OutputTarget::Test) {
                    vec![
                        module_layout.testability_dir(),
                        module_layout.test_sources_dir(),
                    ]
                } else if module_target_name == "ohosTest" {
                    let mut roots = Vec::with_capacity(2);
                    if ohos_test_templates_after_migration {
                        roots.push(ohos_test_template_layout.ohos_test_ets_root());
                    }
                    roots.push(module_layout.source().target_ets(module_target_name));
                    roots
                } else {
                    Vec::new()
                };
            let visual_sources =
                if is_incremental_watch && !session.arkts_language_version.is_static() {
                    VisualCodegenAdapter::new(
                        &session.source_store,
                        &all_module_roots,
                        &dependency_source_roots,
                    )
                    .generate()?
                } else {
                    FxHashMap::default()
                };
            let transform_sources = TransformSourceSnapshotBuilder::new(
                &all_module_roots,
                &dependency_source_roots,
                &native_declaration_roots,
                &source_set_overlays,
            )
            .with_source_store(&session.source_store)
            .with_test_source_roots(&test_source_roots)
            .with_package_roots(&dependency_package_roots)
            .with_sdk_declaration_roots(sdk_declaration_roots)
            .with_custom_type_declaration_inputs(&loader_json.compiler_type_declaration_inputs)
            .build()?
            .with_visual_sources(visual_sources);
            tracing::info!(
              module = %module.name,
              elapsed_ms = transform_snapshot_start.elapsed().as_millis(),
              "transform source snapshot built"
            );

            let plugin_factory_start = Instant::now();
            let transform_resource_inventory =
                transform_resource_inventory_loader.load(loader_json, &module_layout)?;
            let integrated_hsp = module_build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false);
            let mut system_api = SystemApiCheckOptions::default()
                .with_compatible_sdk_version(module_config.product.compatible_sdk_version.clone())
                .with_compile_sdk_version(config.sdk_config.compile_api_version.to_string())
                .with_runtime_os(module_config.product.runtime_os.clone())
                .with_atomic_service(session.project.bundle_type == "atomicService")
                .with_ohos_test_source(module_target_name == "ohosTest")
                .with_crossplatform(args.is_crossplatform, args.ignore_crossplatform_check)
                .with_crossplatform_collection(args.is_crossplatform && !args.watch)
                .with_strict_api_compatibility(matches!(
                    module_build_option.strict_mode.api_compatibility_check,
                    Some(arkdown_core::ApiCompatibilityCheckLevel::Error)
                ))
                .with_request_permissions(
                    module_context
                        .module
                        .module_json
                        .as_ref()
                        .into_iter()
                        .flat_map(|manifest| manifest.module.request_permissions.iter())
                        .filter_map(|permission| {
                            permission.get("name").and_then(serde_json::Value::as_str)
                        })
                        .map(str::to_string),
                );
            if args.is_crossplatform && !args.watch {
                system_api =
                    system_api.with_module_prefixes(session.sdk.module_prefixes.iter().cloned());
                for dependency in session.sdk.crossplatform_dependencies.iter() {
                    system_api = system_api.with_crossplatform_dependency(
                        dependency.api_module.clone(),
                        dependency.function.clone(),
                        dependency.modules.iter().cloned(),
                        dependency.components.iter().cloned(),
                    );
                }
            }
            if let Some(capabilities) =
                module_context
                    .module
                    .module_json
                    .as_ref()
                    .and_then(|manifest| {
                        session
                            .sdk
                            .system_capabilities
                            .select(&manifest.module.device_types)
                    })
            {
                system_api = system_api
                    .with_system_capabilities(capabilities.intersection, capabilities.union);
            }
            let mut factory_input = PluginFactoryInput {
                target_name: module_target_name.clone(),
                sdk_api_dirs: config
                    .sdk_config
                    .ets_api_dirs_for(session.arkts_language_version),
                sdk_kits_dirs: config
                    .sdk_config
                    .ets_kits_dirs_for(session.arkts_language_version),
                sdk_arkts_dirs: config
                    .sdk_config
                    .ets_arkts_dirs_for(session.arkts_language_version),
                // hvigor `arkts-pack.getResolveModules()` searches the active source
                // root before the generated profile directory. Keep these as typed
                // paths so the resolver works identically on Windows and POSIX hosts.
                resolve_source_roots: vec![
                    PathBuf::from(&loader_json.module_root_path),
                    module_layout
                        .build_profile_for_module(&module.module_type, module_target_name)
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "cannot derive generated profile lookup root for module '{}'",
                                module.name
                            )
                        })?,
                ],
                all_module_roots,
                resource_refs: resource_refs.clone(),
                resource_context: ResourceTransformContext {
                    min_api_version: parse_api_version(
                        &module_config.product.compatible_sdk_version,
                    )
                    .unwrap_or(loader_json.api_version),
                    compile_har: matches!(module.module_type, OhosModuleType::Har),
                    byte_code_har: loader_json.byte_code_har.unwrap_or(false),
                    integrated_hsp,
                    use_get_current_bundle_name: args.integrated_hsp_use_get_current_bundle_name,
                    ui_transform_optimization: args.ui_transform_optimization,
                    reset_bundle_name: integrated_hsp,
                    allow_empty_bundle_name: false,
                    bundle_name: module_config.product.bundle_name.clone(),
                    module_name: loader_json.module_name.clone(),
                    validate_resource_modules: true,
                    validate_rawfiles: true,
                    rawfiles: transform_resource_inventory.rawfiles,
                    resource_modules: transform_resource_inventory.modules,
                },
                component_metadata: component_metadata.clone(),
                insight_intent_schema_catalog: insight_intent_schema_catalog.clone(),
                system_api,
                transform_sources,
                navigation_routes: NavigationRouteMap::from_loader_entries(&loader_json.router_map),
                package_resolution,
                source_set_overlays: source_set_overlays.clone(),
                source_redirects: Arc::new(
                    static_interop
                        .as_ref()
                        .map(|interop| interop.source_redirects.clone())
                        .unwrap_or_default(),
                ),
                checker_mode,
                tsc_target: module_build_option
                    .ark_options
                    .as_ref()
                    .map(|options| options.tsc_config.target_es_version)
                    .unwrap_or_default(),
                case_sensitive_check: module_build_option.strict_mode.case_sensitive_check,
                no_external_import_by_path: module_build_option
                    .strict_mode
                    .is_no_external_import_by_path(),
                har_dedup: HarDedupFactoryMode::Disabled,
                disabled_plugins: custom_cfg.plugins.disabled.clone(),
                package_plugins: module_package_plugins.clone(),
                auto_lazy_import: module_build_option
                    .ark_options
                    .as_ref()
                    .and_then(|options| options.auto_lazy_import)
                    .unwrap_or(false),
                auto_lazy_filter: module_build_option
                    .ark_options
                    .as_ref()
                    .and_then(|options| options.auto_lazy_filter.clone()),
                expand_import_path: module_build_option
                    .ark_options
                    .as_ref()
                    .and_then(|options| options.expand_import_path.clone()),
                hsp_har_package_names: loader_json
                    .hsp_name_ohm_map
                    .keys()
                    .chain(loader_json.har_name_ohm_map.keys())
                    .cloned()
                    .collect(),
                // `meta.pkgName` for `isNotAutoLazyImport`: the owning module's
                // `oh-package.json5#name`.
                module_pkg_name: module_context
                    .module
                    .package
                    .as_ref()
                    .map(|package| package.name.clone()),
            };
            let collect_factory_output = if module_har_dedup_plan.is_empty() {
                None
            } else {
                let mut collect_input = factory_input.clone();
                collect_input.har_dedup = HarDedupFactoryMode::Collect {
                    collect_file: module_layout.source_har_duplicate_files_info(),
                    source_har_roots: module_har_dedup_plan.source_har_roots().to_vec(),
                };
                // The hvigor collection task runs the SDK compiler plugins but not
                // package-supplied compile plugins or their lifecycle hooks.
                collect_input.package_plugins.clear();
                Some(PluginFactory::new(&bundle_plan, collect_input).build_output()?)
            };
            if !module_har_dedup_plan.is_empty() {
                factory_input.har_dedup = HarDedupFactoryMode::RealBuild {
                    collect_file: module_layout.source_har_duplicate_files_info(),
                };
            }
            let widget_bundle_plan = widget_compile_plan
                .as_ref()
                .map(|plan| widget_pass.bundle_plan(&bundle_plan, plan));

            let bundle_stage_start = Instant::now();
            #[cfg(feature = "memory-diagnostics")]
            arkdown::memory_diagnostics::dump_with(
                "build:before-bundle-stage",
                format_args!("module={}", module.name),
            );
            if let Some(collect_factory_output) = collect_factory_output {
                let collect_out = BundleStage::run(
                    BundleStageInput {
                        plugins: collect_factory_output.plugins,
                        ets_transform_plan: Some(collect_factory_output.ets_transform_plan),
                        ets_transform_plugin: Some(collect_factory_output.ets_transform_plugin),
                        plan: bundle_plan.clone(),
                        incremental_changes: None,
                    },
                    None,
                )
                .await?;
                let bytecode_module_ids = module_har_dedup_plan.collect_bytecode_module_ids(
                    collect_out.bundle_output.external_module_requests.iter(),
                );
                module_har_dedup_plan.write_bytecode_module_ids(
                    &module_layout.bytecode_har_duplicate_files_info(),
                    &bytecode_module_ids,
                )?;
            }
            let plugin_factory_output =
                PluginFactory::new(&bundle_plan, factory_input).build_output()?;
            let plugins = plugin_factory_output.plugins;
            let widget_plugins = plugins.clone();
            let ets_transform_plan = plugin_factory_output.ets_transform_plan;
            let ets_transform_plugin = plugin_factory_output.ets_transform_plugin;
            tracing::info!(
              module = %module.name,
              elapsed_ms = plugin_factory_start.elapsed().as_millis(),
              "bundle plugin factory completed"
            );

            let mut bundle_out = BundleStage::run(
                BundleStageInput {
                    plugins,
                    ets_transform_plan: Some(Arc::clone(&ets_transform_plan)),
                    ets_transform_plugin: Some(ets_transform_plugin),
                    plan: bundle_plan,
                    incremental_changes: (args.watch
                        || args.preview
                        || (args.action.is_hot_reload() && args.watch_invalidation.is_none())
                        || (args.watch_invalidation.is_some()
                            && args.skip_resource
                            && args.skip_profile))
                        .then(|| watch_changed_files.clone()),
                },
                bundle_runtime.as_deref_mut(),
            )
            .await?;
            if args.is_crossplatform && !args.watch {
                CrossplatformCollectionWriter::new(project_root, cache_dir.as_path())
                    .write(&bundle_out.bundle_output.crossplatform_collections)?;
            }
            if session.arkts_language_version == arkdown_core::ArktsLanguageVersion::V1_1 {
                let intent_modules = bundle_out
                    .bundle_output
                    .insight_intent_sources
                    .iter_mut()
                    .map(|source| InsightIntentModuleInput {
                        source_file_path: source.source_file_path.clone(),
                        decorator_file: source.decorator_file.clone(),
                        intents: std::mem::take(&mut source.metadata.intents)
                            .into_iter()
                            .map(|intent| InsightIntentInput {
                                data: intent.data,
                                entity_type_names: intent.entity_type_names,
                            })
                            .collect(),
                        entities: std::mem::take(&mut source.metadata.entities)
                            .into_iter()
                            .map(|entity| InsightIntentEntityInput {
                                class_name: entity.class_name,
                                data: entity.data,
                                parent_class_name: entity.parent_class_name,
                            })
                            .collect(),
                    })
                    .collect();
                let dependency_profiles = loader_json
                    .byte_code_har_info
                    .values()
                    .filter_map(|dependency| dependency.insight_intent.as_deref())
                    .map(PathBuf::from)
                    .collect();
                InsightIntentProfileProcessor::new()
                    .process(InsightIntentProfileInput {
                        profile_path: &module_layout.res_profile_dir().join("insight_intent.json"),
                        cache_path: &PathBuf::from(&loader_json.cache_path)
                            .join("insight_compile_cache.json"),
                        module_json_path: &module_layout.res_module_json(),
                        bundle_name: &config.product.bundle_name,
                        module_name: &module.name,
                        modules: intent_modules,
                        dependency_profiles,
                    })
                    .map_err(|error| {
                        anyhow::anyhow!("InsightIntent build-end processing failed: {error}")
                    })?;
            }
            // ets-loader's SourceMapGenerator runs during the Rollup pass, before
            // CompileArkTS. Stage the map at the same boundary so its previous cache
            // is still available for the current-pass-first merge.
            if let Some(source_maps_path) = bundle_out.bundle_output.source_maps_path.clone() {
                // Latest ProcessHarArtifacts does not materialize
                // `intermediates/source_map` for a ByteCode HAR, unlike HAP/HSP and a
                // source HAR. Keep that distinction in the Rust-owned handoff rather
                // than making the packer infer it from an optional file.
                let package_source_map_intermediate = module_layout.source_maps_intermediate();
                let package_source_map_handoff = if module.module_type == OhosModuleType::Har
                    && module_build_option.har_artifact_type == HarArtifactType::Bytecode
                {
                    SourceMapPackageHandoff::Suppress(&package_source_map_intermediate)
                } else {
                    SourceMapPackageHandoff::Materialize(&package_source_map_intermediate)
                };
                let source_maps_path = intermediate_writer.stage_source_maps(
                    &source_maps_path,
                    &PathBuf::from(&loader_json.cache_path).join("sourceMaps.json"),
                    &PathBuf::from(&loader_json.cache_path).join("sourceMaps.map"),
                    package_source_map_handoff,
                    &config.build_mode,
                    &bundle_out.bundle_output.source_map_input_fingerprints,
                )?;
                bundle_out.bundle_output.source_maps_path = Some(source_maps_path);
            }
            let widget_bundle_out = if let (Some(widget_compile_plan), Some(widget_bundle_plan)) =
                (widget_compile_plan, widget_bundle_plan)
            {
                let mut bundle_out = BundleStage::run(
                    BundleStageInput {
                        plugins: widget_plugins,
                        ets_transform_plan: Some(Arc::clone(&ets_transform_plan)),
                        ets_transform_plugin: None,
                        plan: widget_bundle_plan,
                        incremental_changes: None,
                    },
                    None,
                )
                .await?;
                bundle_out.bundle_output.modules_abc_path =
                    widget_compile_plan.modules_abc_path.clone();
                let output = WidgetCompileArktsOutput {
                    bundle_out,
                    plan: widget_compile_plan,
                };
                widget_pass.write_component_collection(&output, &ets_transform_plan)?;
                Some(output)
            } else {
                None
            };
            plugin_context_cache.commit().await?;
            #[cfg(feature = "memory-diagnostics")]
            arkdown::memory_diagnostics::dump_with(
                "build:after-bundle-stage",
                format_args!(
                    "module={} chunks={} module_source_files={} npm_entries={} source_maps={}",
                    module.name,
                    bundle_out.bundle_output.chunk_files.len(),
                    bundle_out.bundle_output.module_source_files.len(),
                    bundle_out.bundle_output.npm_entries.len(),
                    bundle_out.bundle_output.source_maps_path.is_some()
                ),
            );
            tracing::info!(
              module = %module.name,
              elapsed_ms = bundle_stage_start.elapsed().as_millis(),
              "bundle stage run completed"
            );
            if preload_system_so_enabled {
                PreloadSystemSoArtifactWriter::new(
                    &module_layout.preload_system_so_json(),
                    &module_layout.res_profile_dir(),
                    &module_layout.res_module_json(),
                    &module.module_type,
                    &bundle_out.bundle_output.preload_system_so_tasks,
                )
                .write_preload_artifact()?;
            }
            let mut code_resource_roots = vec![module_layout.source().ets()];
            code_resource_roots.extend(
                source_set_overlays
                    .iter()
                    .map(|(_, target_root)| target_root.clone()),
            );
            let code_resource_options = module_config
                .product
                .build_option
                .res_options
                .as_ref()
                .map(|options| options.copy_code_resource.clone())
                .unwrap_or_default();
            CodeResourceCopyPlan::new(
                code_resource_roots,
                module_layout.source().src_main().join("share"),
                bundle_out.bundle_output.output_dir.clone(),
                module_layout
                    .default_cache_dir("code_resources")
                    .join("manifest.json"),
                code_resource_options,
            )
            .execute()?;
            if matches!(module.module_type, OhosModuleType::Shared) {
                let declaration_packages =
                    HarDeclarationDependencyPackageInventory::new(&module_context.deps).collect();
                HarDeclarationGenerator::new(
                    &module.src_path,
                    &module_layout.source().ets(),
                    module_layout.declarations_dir(),
                    &module.name,
                )
                .with_arkts_language_version(self.session.arkts_language_version)
                .with_dependency_declaration_packages(declaration_packages)
                .with_module_entry_source(
                    module_context
                        .module
                        .package
                        .as_deref()
                        .and_then(|package| package.package_json.main.as_deref()),
                )
                .with_package_bundled_declarations(
                    module_build_option
                        .ark_options
                        .as_ref()
                        .is_some_and(|options| options.bundled_declare),
                    module_context
                        .module
                        .package
                        .as_deref()
                        .and_then(|package| package.package_json.main.as_deref()),
                    module_context
                        .module
                        .package
                        .as_deref()
                        .and_then(|package| package.package_json.oh_exports.as_deref()),
                )
                .generate()
                .map_err(|error| anyhow::anyhow!("generate HSP declarations failed: {error}"))?;
            }
            // beforeBuildEnd is supplied by the internal Rolldown gap adapter and
            // executes immediately before Rolldown's native buildEnd hooks.

            if module_package_plugins
                .iter()
                .any(PackagePlugin::has_should_invalid_cache)
            {
                PluginCacheModuleInventory::from_module_sources(
                    cache_dir.as_path(),
                    &bundle_out.bundle_output.module_source_files,
                    args.hvigor_properties.rollup_cache_use_source_hash,
                )?
                .save(cache_dir.as_path())?;
            } else {
                PluginCacheModuleInventory::clear(cache_dir.as_path())?;
            }

            // Persist cache-group hashes so the next build can diff against them.
            // This matches hvigor's `buildEnd` + `beforeBuildEnd` cache writes in
            // `invalid-cache-plugin.ts`.
            if let Err(e) = cache_bridge.commit_group_snapshots(&group_snapshots) {
                tracing::debug!(error = %e, "failed to persist cache group hashes (non-fatal)");
            }

            bundle_outs.push((bundle_out,));
            widget_bundle_outs.push(widget_bundle_out);
        }
        progress.done();

        // Post-bundle: process importees and update loader.json.
        for (
            (
                (module, _manifest, loader_json, _resource_refs, module_target_name, module_config),
                (bundle_out,),
            ),
            bc_har_deps,
        ) in loader_jsons
            .iter_mut()
            .zip(bundle_outs.iter_mut())
            .zip(all_bc_har_deps.iter())
        {
            let processor = ProfileProcessor::new(module_config.clone());
            if bundle_out.bypasses_compilation() {
                continue;
            }
            let module_layout = ModuleBuildLayout::for_output_target_product(
                &config.output_target,
                &module.src_path,
                &config.product.name,
                module_target_name,
            );
            let module_src_main = PathBuf::from(&loader_json.module_root_path);
            let importee_cfg = ProcessImporteeConfig {
                bundle_name: config.product.bundle_name.clone(),
                module_name: loader_json.module_name.clone(),
                module_src_main,
            };
            let result = ProcessImporteeRunner::new(
                &bundle_out.bundle_output.module_source_files,
                &bundle_out.bundle_output.entry_module_ids,
                &bundle_out.bundle_output.module_imports,
                &importee_cfg,
            )
            .process();
            loader_json.non_entry_importees = result.non_entry_importees;
            loader_json.compiled_importees = result.compiled_importees;

            let module_build_option = &module_config.product.build_option;
            let is_bytecode_har = matches!(module.module_type, OhosModuleType::Har)
                && matches!(
                    module_build_option.har_artifact_type,
                    HarArtifactType::Bytecode
                );
            let bundled_dependencies = module_build_option
                .ark_options
                .as_ref()
                .is_some_and(|options| options.bundled_dependencies);
            HarImporteeCache::new(&loader_json.cache_path).write_compile_output(
                &bundle_out.bundle_output.har_importees,
                is_bytecode_har,
                bundled_dependencies,
                args.byte_code_har_integrated_optimization,
            )?;

            // Hvigor enables collectImportersPlugin only for hot-reload builds.
            // Normal CompileArkTS builds must not emit importer_map intermediates.
            if collect_importers {
                let mut importer_map: FxHashMap<String, Vec<String>> = FxHashMap::default();
                let compiled_module_ids = bundle_out
                    .bundle_output
                    .module_source_files
                    .iter()
                    .map(|source| source.module_id.as_str())
                    .collect::<FxHashSet<_>>();
                for (importer, imports) in &bundle_out.bundle_output.module_imports {
                    if !compiled_module_ids.contains(importer.as_str()) {
                        continue;
                    }
                    for imported in imports {
                        if compiled_module_ids.contains(imported.as_str()) {
                            importer_map
                                .entry(imported.clone())
                                .or_default()
                                .push(importer.clone());
                        }
                    }
                }
                for importers in importer_map.values_mut() {
                    importers.sort();
                    importers.dedup();
                }
                let out_dir = module_layout.importer_map_dir();
                std::fs::create_dir_all(&out_dir).ok();
                let json = serde_json::to_value(&importer_map).unwrap_or_default();
                intermediate_writer
                    .write_json_pretty(&module_layout.importer_map_json(), &json)
                    .unwrap_or_else(
                        |e| tracing::warn!(error = %e, "failed to write importer_map.json"),
                    );
            }

            let bundle_name = &config.product.bundle_name;
            for dep in bc_har_deps {
                if let Some(ohm_url) = dep.ohm_url(bundle_name) {
                    if !loader_json.compiled_importees.contains(&ohm_url) {
                        loader_json.compiled_importees.push(ohm_url);
                    }
                }
            }

            // Remove stale .d.ts/.d.ets files no longer in the current bundle output.
            // Mirrors hvigor's removeRedundantDeclaredFiles (rm-declared-files.js).
            if !bundle_out.bundle_output.chunk_files.is_empty() {
                let current_stems: std::collections::HashSet<String> = bundle_out
                    .bundle_output
                    .chunk_files
                    .iter()
                    .filter_map(|(name, _)| {
                        std::path::Path::new(name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
                let out_dir = &bundle_out.bundle_output.output_dir;
                if out_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(out_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            if ext == "d.ts" || ext == "d.ets" {
                                let stem = path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("")
                                    .to_string();
                                if !current_stems.contains(&stem) {
                                    tracing::debug!(
                                        path = %path.display(),
                                        "removing stale declaration file (removeRedundantDeclaredFiles)"
                                    );
                                    let _ = std::fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }

            let loader_json_out = module_layout.loader_json();
            processor
                .write_loader_json(loader_json, &loader_json_out)
                .map_err(|e| anyhow::anyhow!("update loader.json with importees failed: {}", e))?;

            bundle_out.release_chunk_graph();
        }

        Ok(CoordinatorBundleStageOutput {
            bundle_outs,
            widget_bundle_outs,
            module_obfuscation_contexts,
        })
    }
}
