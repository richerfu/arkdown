//! Package and sign stage implementation.

use super::*;

pub(super) struct PackageSignStageInput<'a> {
    pub(super) args: &'a BuildRequest,
    pub(super) config: &'a ArkdownConfig,
    pub(super) session: &'a BuildSession,
    pub(super) quickfix_version_store: &'a Option<QuickFixVersionStore>,
    pub(super) quickfix_patch_version: Option<u64>,
    pub(super) quickfix_version_advanced: &'a mut bool,
    pub(super) progress: &'a mut Progress,
    pub(super) intermediate_writer: &'a IntermediateArtifactWriter,
    pub(super) har_dedup_project_plan: &'a HarDedupProjectPlan,
    pub(super) sanitizer_native_policy: &'a NativeLibCollectionPolicy,
    pub(super) loader_jsons: &'a [ModuleBuildRecord],
    pub(super) local_native_dependencies: &'a BTreeMap<String, Arc<[NativeCmakeLocalDependency]>>,
    pub(super) module_pack_info_plans: &'a [Vec<ModulePackInfoPlan>],
    pub(super) privacy_profile_paths: &'a [Option<PathBuf>],
    pub(super) module_obfuscation_contexts: &'a [ModuleObfuscationContext],
    pub(super) bundle_package_outs: &'a [(BundlePackageOutput,)],
    pub(super) output_paths: &'a mut Vec<PathBuf>,
    pub(super) start: Instant,
}

impl BuildCoordinator {
    pub(super) async fn run_package_sign_stage(
        &self,
        input: PackageSignStageInput<'_>,
    ) -> Result<Option<BuildOutput>> {
        let PackageSignStageInput {
            args,
            config,
            session,
            quickfix_version_store,
            quickfix_patch_version,
            quickfix_version_advanced,
            progress,
            intermediate_writer,
            har_dedup_project_plan,
            sanitizer_native_policy,
            loader_jsons,
            local_native_dependencies,
            module_pack_info_plans,
            privacy_profile_paths,
            module_obfuscation_contexts,
            bundle_package_outs,
            output_paths,
            start,
        } = input;

        // ── Step 6: Packaging + Signing ───────────────────────────────────
        progress.step(6, 6, "Packaging");
        // Local unit-test builds produce no HAP/HSP/HAR archive — the `test`
        // task ends at the compiled assets (`intermediates/assets/<target>/ets/`)
        // plus coverage data. hvigor `Test`/`UnitTestBuild` tasks never run the
        // pack graph.
        if matches!(config.output_target, OutputTarget::Test) {
            let coverage = args.ohos_test_coverage.unwrap_or(true);
            for (
                module,
                _manifest,
                _loader_json,
                _resource_refs,
                _module_target_name,
                module_config,
            ) in loader_jsons.iter()
            {
                let artifacts = crate::unit_test::UnitTestArtifactWriter::new(
                    &module.src_path,
                    &module_config.product.name,
                );
                artifacts.copy_mock_config_json()?;
                artifacts.replace_source_map_in_coverage_mode(coverage)?;
                let module_context = session.module_context(&module.name).ok_or_else(|| {
                    anyhow::anyhow!("missing module compile context for {}", module.name)
                })?;
                crate::unit_test::UnitTestExecutionPlan::from_build(
                    crate::unit_test::UnitTestExecutionInput {
                        sdk_config: &config.sdk_config,
                        runtime_os: &config.product.runtime_os,
                        module_path: &module.src_path,
                        project_path: &config.project_root,
                        module_type: &module.module_type,
                        dependencies: module_context.deps.as_ref(),
                        coverage,
                        node_executable: args.resolved_node_executable(),
                    },
                )
                .and_then(|plan| plan.execute())
                .map_err(|error| {
                    anyhow::anyhow!(
                        "GenerateUnitTestResult failed for module '{}': {error}",
                        module.name
                    )
                })?;
            }
            progress.done();
            return Ok(Some(BuildOutput {
                artifact_paths: Vec::new(),
            }));
        }
        // hvigor `FastPackageHap` (`packOptions.fastBuildApp`): skip the HAP
        // packaging and copy the compiled intermediates into
        // `intermediates/fastApp/<target>/` for IDE fast deployment.
        if args.fast_build_app {
            let enabled_fast_packages = vec![true; loader_jsons.len()];
            let paths = Self::run_fast_package(
                loader_jsons,
                &enabled_fast_packages,
                module_pack_info_plans,
            )?;
            for (
                (module, manifest, loader_json, _, module_target_name, module_config),
                (package_out,),
            ) in loader_jsons.iter().zip(bundle_package_outs.iter())
            {
                let module_layout = ModuleBuildLayout::for_output_target_product(
                    &module_config.output_target,
                    &module.src_path,
                    &module_config.product.name,
                    module_target_name,
                );
                PackingCheck::new(
                    module_config,
                    module,
                    manifest,
                    PathBuf::from(&loader_json.ace_module_build),
                    module_layout.stripped_native_libs_dir(),
                    module_layout.res_default_dir(),
                )
                .validate()
                .map_err(|error| anyhow::anyhow!("packing-check failed: {error}"))?;

                let source_map_output = args
                    .hvigor_properties
                    .source_map_output(&config.project_root, &module.name)
                    .unwrap_or_else(|| module_layout.output_mapping_source_maps());
                intermediate_writer.publish_package_source_maps(
                    package_out.source_maps_path.as_deref(),
                    &source_map_output,
                )?;
                let source_map_primary = package_out
                    .source_maps_path
                    .clone()
                    .unwrap_or_else(|| module_layout.loader_out_source_maps());
                DebugSymbolCollector::new(
                    module.src_path.clone(),
                    module.name.clone(),
                    config.build_mode.clone(),
                    source_map_primary,
                    None,
                    PathBuf::from(&loader_json.cache_path)
                        .join("obfuscation")
                        .join("nameCache.json"),
                )
                .with_collection_override(args.collect_debug_symbol)
                .with_output_variant(&config.product.name, module_target_name)
                .collect()
                .map_err(|error| anyhow::anyhow!("CollectDebugSymbol failed: {error}"))?;
            }
            progress.done();
            return Ok(Some(BuildOutput {
                artifact_paths: paths,
            }));
        }
        for (
            (
                (
                    (
                        (
                            module,
                            manifest,
                            loader_json,
                            _resource_refs,
                            module_target_name,
                            module_config,
                        ),
                        (package_out,),
                    ),
                    privacy_profile_path,
                ),
                module_obfuscation,
            ),
            pack_info_plans,
        ) in loader_jsons
            .iter()
            .zip(bundle_package_outs.iter())
            .zip(privacy_profile_paths.iter())
            .zip(module_obfuscation_contexts.iter())
            .zip(module_pack_info_plans.iter())
        {
            let module_build_option = &module_config.product.build_option;
            let packager = Packager::new(module_config.clone()).with_compress_level(
                args.pack_compress_level
                    .filter(|_| module_config.sdk_config.sdk_component_api_version >= 12)
                    .unwrap_or(1),
            );
            let module_layout = ModuleBuildLayout::for_output_target_product(
                &config.output_target,
                &module.src_path,
                &config.product.name,
                module_target_name,
            );
            let quickfix_task_plan = QuickFixTaskPlan::for_action(args.action);
            let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
            let resources_index = module_layout.res_resources_index();
            let modules_abc = package_out.modules_abc_path.clone();
            let ext = match &module_config.output_target {
                OutputTarget::Hsp => "hsp",
                OutputTarget::Har => "har",
                _ => "hap",
            };
            // hvigor places non-default target archives under `outputs/<target>`
            // (e.g. `outputs/ohosTest/entry-ohosTest-unsigned.hap`).
            let output_dir = if module_target_name == "default" {
                module_layout.outputs_default()
            } else {
                module_layout.outputs_root().join(module_target_name)
            };

            let packaging_build_mode_name = config.build_mode.as_str();
            let all_applicable = session.build_options.applicable_targets(
                &module.name,
                &config.product.name,
                packaging_build_mode_name,
            );
            let target_labels = [module_target_name.clone()];

            std::fs::create_dir_all(&output_dir)?;
            let module_context = session.module_context(&module.name).ok_or_else(|| {
                anyhow::anyhow!("missing build session module context for {}", module.name)
            })?;

            for artifact_target_label in &target_labels {
                let interface_source_roots = all_applicable
                    .iter()
                    .find(|target| target.name == *artifact_target_label)
                    .map(|target| target.source_roots.clone())
                    .unwrap_or_default();
                let modules_abc = modules_abc.clone();
                let resources_index = resources_index.clone();
                let output_name = ArtifactOutputName::new(
                    module_config.product.artifact_name.as_deref(),
                    &module.name,
                    artifact_target_label,
                    ext,
                );
                let package_artifacts = ModulePackageArtifact::for_module_target(
                    module,
                    module_config,
                    artifact_target_label,
                    ext,
                    &output_dir,
                    pack_info_plans,
                );
                let primary_output = package_artifacts
                    .first()
                    .map(|artifact| artifact.package_path.clone())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "MakePackInfo did not produce an artifact plan for '{}:{}'",
                            module.name,
                            artifact_target_label
                        )
                    })?;
                let syscap_tool = session
                    .sdk
                    .tool_path(SdkToolKind::SyscapTool)
                    .map(|tool| tool.path)
                    .unwrap_or_else(|| session.sdk.sdk_root.join("toolchains/syscap_tool"));
                let device_capabilities = session
                    .sdk
                    .system_capabilities
                    .select(&manifest.device_types)
                    .map(|selection| {
                        selection
                            .intersection
                            .into_iter()
                            .map(|capability| capability.to_string())
                            .collect()
                    });
                let syscap_path = SyscapTransform::new(SyscapTransformInput {
                    runtime_os: &config.product.runtime_os,
                    module_name: &module.name,
                    compile_api_version: config.sdk_config.compile_api_version,
                    tool_path: &syscap_tool,
                    output_dir: module_layout.syscap_dir(),
                    is_har: matches!(module_config.output_target, OutputTarget::Har),
                    device_capabilities,
                    custom_syscap: module_context.module.syscap_value.as_deref(),
                })
                .run()
                .map_err(|error| anyhow::anyhow!("SyscapTransform failed: {error}"))?;

                let bundle_version_code = session.project.version_code;
                let bundle_version_name = session.project.version_name.clone();
                let bundle_build_version = session.project.build_version.clone();
                let bundle_type = session.project.bundle_type.clone();
                let integrated_hsp_enabled = config
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .and_then(|options| options.integrated_hsp)
                    == Some(true);
                let integrated_hsp_candidates: Vec<IntegratedHspCandidate> = module_context
                    .deps
                    .integrated_hsp_nodes()
                    .filter_map(|node| {
                        Some(IntegratedHspCandidate {
                            alias: node.alias.clone(),
                            package_root: node.package_root.clone()?,
                            version: node
                                .package
                                .as_ref()
                                .map(|package| package.version.clone())
                                .filter(|version| !version.trim().is_empty())
                                .unwrap_or_else(|| "1.0.0".to_string()),
                        })
                    })
                    .collect();
                let integrated_hsp_start = Instant::now();
                BuildPolicy::new(config).validate_integrated_hsp_api(
                    integrated_hsp_enabled && !integrated_hsp_candidates.is_empty(),
                )?;
                let integrated_hsps = IntegratedHspCollector::new(&integrated_hsp_candidates)
                    .with_enabled(integrated_hsp_enabled)
                    .collect_normalized(
                        &config.product.bundle_name,
                        bundle_version_code,
                        bundle_build_version.as_deref(),
                        &module_layout.integrated_hsp_dir(),
                    )
                    .map_err(|error| {
                        anyhow::anyhow!("K5: failed to normalize integrated HSPs: {error}")
                    })?;
                tracing::info!(
                  module = %module.name,
                  elapsed_ms = integrated_hsp_start.elapsed().as_millis(),
                  "integrated hsp collection completed"
                );

                let native_build_option =
                    arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
                        module_build_option,
                    );
                let native_header_dirs = native_build_option.native_lib.header_paths.clone();
                let exclude_so_from_interface_har =
                    native_build_option.native_lib.exclude_so_from_interface_har;
                let self_package = module_context
                    .module
                    .package
                    .as_ref()
                    .map(|package| {
                        NativeLibPackageInfo::local(package.name.clone(), package.version.clone())
                    })
                    .unwrap_or_else(|| {
                        NativeLibPackageInfo::local(module.name.clone(), String::new())
                    });
                let cmake_local_dependencies = local_native_dependencies
                    .get(&module.name)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([]));
                let native_policy = sanitizer_native_policy
                    .clone()
                    .with_release_cmake_build_type(
                        args.hvigor_properties.release_cmake_build_type.as_deref(),
                    )
                    .with_external_native_options(
                        native_build_option.external_native_options.clone(),
                    )
                    .with_native_lib_option(native_build_option.native_lib.clone())
                    .with_self_package(self_package)
                    .with_cmake_local_dependency_inventory(cmake_local_dependencies);
                let native_collector = NativeLibCollector::new(
                    module,
                    &native_build_option.abi_filters,
                    module_context.deps.as_ref(),
                )
                .with_output_variant(&module_config.product.name, artifact_target_label)
                .with_policy(native_policy);
                let native_collector = if matches!(module_config.output_target, OutputTarget::Har) {
                    native_collector
                } else {
                    native_collector
                        .excluding_dependency_roots(module_har_dedup_plan.duplicate_package_roots())
                };
                native_collector
                    .configure_native()
                    .map_err(|error| anyhow::anyhow!("BuildNativeWithCmake failed: {error}"))?;
                // hvigor 6.24.3 CompileNativeHook is a targetless module hook whose
                // only action is the dependency edge to target@BuildNativeWithCmake.
                native_collector
                    .build_native()
                    .map_err(|error| anyhow::anyhow!("BuildNativeWithNinja failed: {error}"))?;
                let native_lib_start = Instant::now();
                let processed_native_libs = native_collector
                    .process_libs()
                    .map_err(|error| anyhow::anyhow!("ProcessLibs failed: {error}"))?;
                // NativeTaskInitializer makes DoNativeStrip depend on DoNativeBinxo
                // whenever its BinXO-only predicate is enabled. Direct invocation of
                // DoNativeBinxo reaches the same action even when the predicate is
                // disabled, where BinxoValidationService reports the missing HWASan
                // combination.
                let run_binxo = native_collector.is_binxo_enabled();
                let binxo_libs = if run_binxo {
                    let outputs = native_collector
                        .binxo_libs(&processed_native_libs)
                        .map_err(|error| anyhow::anyhow!("DoNativeBinxo failed: {error}"))?;

                    outputs
                } else {
                    processed_native_libs
                };
                let native_libs = native_collector
                    .strip_libs(&binxo_libs)
                    .map_err(|error| anyhow::anyhow!("DoNativeStrip failed: {error}"))?;
                if let Some(quickfix_task_plan) = quickfix_task_plan {
                    if !quickfix_task_plan.needs_patch_artifacts() {
                        anyhow::bail!(
              "ARKDOWN_QUICKFIX_LIFECYCLE: ColdReloadArkTS completed without reaching PrepareQuickfix"
            );
                    }
                    let patch_version = quickfix_patch_version.context(
            "ARKDOWN_QUICKFIX_PATCH_VERSION: PrepareQuickfix requires a project patch version",
          )?;
                    let quickfix = QuickFixArtifactProcessor::new(QuickFixArtifactInput {
                        layout: &module_layout,
                        module_name: &module.name,
                        target_name: artifact_target_label,
                        bundle_name: &session.project.bundle_name,
                        version_code: session.project.version_code,
                        version_name: &session.project.version_name,
                        device_types: &manifest.device_types,
                        debug_symbol: native_build_option
                            .native_lib
                            .debug_symbol_configured
                            .then_some(&native_build_option.native_lib.debug_symbol),
                        patch_version,
                    });

                    quickfix
                        .prepare(loader_json.patch_config.as_ref())
                        .map_err(|error| anyhow::anyhow!("PrepareQuickfix failed: {error}"))?;

                    if !quickfix_task_plan.packages() {
                        continue;
                    }
                    let package_tool = session
                        .sdk
                        .tool_path(SdkToolKind::PackageTool)
                        .map(|tool| tool.path)
                        .context(
                            "ARKDOWN_QUICKFIX_PACKAGE_TOOL: PackageHqf requires the SDK app_packing_tool",
                        )?;
                    let package_runner = AppPackageRunner::new(package_tool);
                    let unsigned_hqf = {
                        let output = quickfix
                            .package(&package_runner)
                            .map_err(|error| anyhow::anyhow!("PackageHqf failed: {error}"))?;

                        output
                    };

                    let mut final_hqf = unsigned_hqf.clone();
                    if quickfix_task_plan.signs() {
                        if let Some(signing_config) = &config.product.signing_config {
                            let signed_hqf = quickfix.signed_hqf();
                            if signed_hqf.is_file() {
                                fs_err::remove_file(&signed_hqf)?;
                            }
                            SignBackendRunner::new()
                                .sign_or_skip(&unsigned_hqf, &signed_hqf, Some(signing_config))
                                .map_err(|error| anyhow::anyhow!("SignHqf failed: {error}"))?;
                            quickfix.update_signed_log().map_err(|error| {
                                anyhow::anyhow!("SignHqf log update failed: {error}")
                            })?;
                            final_hqf = signed_hqf;
                        }
                    }

                    if quickfix_task_plan.boundary() == QuickFixTaskBoundary::Assemble
                        && !*quickfix_version_advanced
                    {
                        quickfix_version_store
                .as_ref()
                .context(
                  "ARKDOWN_QUICKFIX_VERSION_STORE: assembleDevHqf requires the Hvigor cache",
                )?
                .advance_after_assemble(patch_version)?;
                        *quickfix_version_advanced = true;
                    }
                    if final_hqf.is_file() {
                        output_paths.push(final_hqf);
                    }
                    continue;
                }
                native_collector
                    .cache_libs()
                    .map_err(|error| anyhow::anyhow!("CacheNativeLibs failed: {error}"))?;
                native_collector
                    .validate_libs(&native_libs)
                    .map_err(|error| {
                        anyhow::anyhow!("CacheNativeLibs validation failed: {error}")
                    })?;
                tracing::info!(
                  module = %module.name,
                  elapsed_ms = native_lib_start.elapsed().as_millis(),
                  native_lib_count = native_libs.len(),
                  "native libs collected"
                );
                let interface_native_libs = native_libs.clone();

                if artifact_target_label == "ohosTest" {
                    crate::unit_test::UnitTestArtifactWriter::for_ohos_test(
                        &module.src_path,
                        &module_config.product.name,
                        artifact_target_label,
                    )
                    .copy_mock_config_json()?;
                }

                let res_module_json = module_layout.res_module_json();
                let package_module_json = module_layout.package_module_json();
                let preload_system_so_effective = PreloadSystemSoArtifactWriter::effective_path(
                    &module_layout.preload_system_so_json(),
                )?
                .is_some();
                let module_json = if matches!(module_config.output_target, OutputTarget::Har) {
                    res_module_json.clone()
                } else {
                    PackageModuleJsonOptions::new(&res_module_json).write_for_pack(
                        &package_module_json,
                        (module_build_option.deduplicate_har
                            && matches!(module.module_type, OhosModuleType::Shared))
                        .then_some(!module_har_dedup_plan.is_empty()),
                        preload_system_so_effective,
                    )?
                };

                if matches!(module_config.output_target, OutputTarget::Hap) {
                    PreloadSystemSoArtifactWriter::new(
                        &module_layout.preload_system_so_json(),
                        &module_layout.res_profile_dir(),
                        &res_module_json,
                        &module.module_type,
                        &package_out.preload_system_so_tasks,
                    )
                    .process_compiled_resources()?;
                }

                let out = match &module_config.output_target {
                    OutputTarget::Har => {
                        let source_dir = module_context
                            .module
                            .source_roots
                            .first()
                            .cloned()
                            .unwrap_or_else(|| module_layout.source().ets());
                        let package_manifest = module_context
                            .module
                            .package_path
                            .clone()
                            .unwrap_or_else(|| module_layout.oh_package_json5());
                        let is_ohpm_project = package_manifest
                            .file_name()
                            .is_some_and(|name| name == std::ffi::OsStr::new("oh-package.json5"));
                        if !source_dir.is_dir() {
                            anyhow::bail!(
                                "HAR source directory does not exist for module '{}': {}",
                                module.name,
                                source_dir.display()
                            );
                        }
                        let artifact_type = module_build_option.har_artifact_type;
                        let declarations_dir = module_layout.declarations_dir();
                        if !matches!(artifact_type, HarArtifactType::Original) {
                            let generated_module_info =
                                module_layout.generated_module_info(module_target_name);
                            let declaration_packages =
                                HarDeclarationDependencyPackageInventory::new(&module_context.deps)
                                    .collect();
                            let declaration_generator = HarDeclarationGenerator::new(
                                &module.src_path,
                                &source_dir,
                                declarations_dir.clone(),
                                &module.name,
                            )
                            .with_arkts_language_version(self.session.arkts_language_version)
                            .with_dependency_declaration_packages(declaration_packages)
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
                            );
                            let declaration_generator =
                                if matches!(artifact_type, HarArtifactType::Bytecode) {
                                    declaration_generator
                                        .with_generated_source(&generated_module_info)
                                } else {
                                    declaration_generator
                                };
                            declaration_generator.generate().map_err(|error| {
                                anyhow::anyhow!("generate HAR declarations failed: {error}")
                            })?;
                        }

                        let declaration_entries =
                            if matches!(artifact_type, HarArtifactType::Bytecode) {
                                if !loader_json.use_normalized_ohm_url {
                                    anyhow::bail!(
                    "ByteCode HAR '{}' requires strictMode.useNormalizedOHMUrl=true",
                    module.name
                  );
                                }
                                let pkg_context_input = PkgContextInfoInputBuilder::new(
                                    config,
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
                                .build()?;
                                let pkg_context_info = PkgContextInfoGenerator::new()
                                    .generate(&pkg_context_input)
                                    .map_err(|error| {
                                        anyhow::anyhow!("GeneratePkgContextInfo failed: {error}")
                                    })?;
                                ByteCodeHarDeclarationCollector::new(
                                    config,
                                    module,
                                    manifest,
                                    &pkg_context_input,
                                    &pkg_context_info,
                                )
                                .collect()?
                                .ohm_urls
                            } else {
                                Vec::new()
                            };
                        let bundled_dependencies = module_build_option
                            .ark_options
                            .as_ref()
                            .is_some_and(|options| options.bundled_dependencies);
                        let har_importees =
                            HarImporteeCache::new(&loader_json.cache_path).read()?;
                        let runtime_only_packages = module_build_option
                            .ark_options
                            .as_ref()
                            .and_then(|options| options.runtime_only.as_ref())
                            .map(|runtime_only| runtime_only.packages.as_slice())
                            .unwrap_or_default();
                        let importee_metadata = module_context.deps.har_importee_metadata(
                            &har_importees.non_entry,
                            &har_importees.compiled,
                            &har_importees.direct,
                            bundled_dependencies,
                            args.byte_code_har_integrated_optimization,
                            runtime_only_packages,
                        );
                        let compiled_package_names = importee_metadata
                            .compiled_package_names
                            .iter()
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        let affects_compilation_dependency_keys = importee_metadata
                            .affects_compilation_dependency_keys
                            .iter()
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        let dependency_pkg_version =
                            module_context.deps.bytecode_har_dependency_versions(
                                bundled_dependencies,
                                args.byte_code_har_integrated_optimization,
                                &compiled_package_names,
                                &affects_compilation_dependency_keys,
                            )?;
                        let package_parameters = module_context
                            .module
                            .package
                            .as_deref()
                            .map(|package| {
                                module_context
                                    .deps
                                    .dependency_parameters
                                    .resolve_package_parameters(&package.package_json)
                            })
                            .transpose()?
                            .map(|resolved| HarPackageParameterInput {
                                version: (!resolved.version.is_empty()).then_some(resolved.version),
                                dependencies: resolved.dependencies,
                                dev_dependencies: resolved.dev_dependencies,
                                dynamic_dependencies: resolved.dynamic_dependencies,
                            })
                            .unwrap_or_default();
                        let bundled_dependency_rewrite =
                            if matches!(artifact_type, HarArtifactType::Bytecode)
                                && bundled_dependencies
                                && !args.keep_dependency
                            {
                                module_context
                                    .module
                                    .package
                                    .as_deref()
                                    .map(|package| {
                                        module_context.deps.bundled_har_retained_dependencies(
                                            &package.package_json,
                                        )
                                    })
                                    .transpose()?
                                    .map(|retained| HarBundledDependencyInput {
                                        dependencies: retained.dependencies,
                                        dynamic_dependencies: retained.dynamic_dependencies,
                                    })
                            } else {
                                None
                            };
                        // `har-target-util.js::isNeedExclude`: `resOptions.excludeHarRes`
                        // drops remote HAR resources (local `file:` dependencies and
                        // HAR-host modules are never excluded).
                        let exclude_har_res = module_build_option
                            .res_options
                            .as_ref()
                            .map(|options| options.exclude_har_res.as_slice())
                            .unwrap_or_default();
                        let bundled_resource_snapshots = if bundled_dependencies {
                            module_context
                                .deps
                                .bundled_har_resources()
                                .into_iter()
                                .filter(|resource| {
                                    let is_local = module_context
                                        .deps
                                        .nodes
                                        .get(&resource.package_name)
                                        .and_then(|node| node.version_spec.as_deref())
                                        .is_some_and(|spec| spec.starts_with("file:"));
                                    !exclude_har_res.contains(&resource.package_name) || is_local
                                })
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        };
                        let bundled_resources = bundled_resource_snapshots
                            .iter()
                            .map(|resource| BundledHarResourceInput {
                                source_dir: resource.source_dir.clone(),
                                archive_directory: resource.archive_directory.clone(),
                            })
                            .collect::<Vec<_>>();
                        let mut bundled_resource_directories = Vec::new();
                        if bundled_dependencies {
                            let effective_target = ModuleTargetSelection::resolve_build(
                                config,
                                session.build_options.as_ref(),
                                module,
                                Some(artifact_target_label),
                                &args.hvigor_properties,
                            )
                            .target;
                            if effective_target.resource_directories.is_empty() {
                                bundled_resource_directories.push(
                                    arkdown_path::ArtifactPath::from_slash("src/main/resources"),
                                );
                            } else {
                                for directory in &effective_target.resource_directories {
                                    bundled_resource_directories.push(
                                        ArtifactPathSerializer::path_to_slash(directory).map_err(
                                            |error| {
                                                anyhow::anyhow!(
                          "invalid HAR target resource directory '{}': {error}",
                          directory.display()
                        )
                                            },
                                        )?,
                                    );
                                }
                            }
                            bundled_resource_directories.extend(
                                bundled_resource_snapshots
                                    .iter()
                                    .map(|resource| resource.metadata_directory.clone()),
                            );
                        }
                        let resource_table = module_layout.resource_table_txt();
                        let processed_module_json = module_layout.process_profile_module_json();
                        let source_maps = package_out.source_maps_path.clone().or_else(|| {
                            let candidate = module_layout.loader_out_source_maps();
                            candidate.is_file().then_some(candidate)
                        });
                        let consumer_rules_path = module_layout.obfuscation_export_rule_path();
                        let ignore_resource_patterns = config
                            .product
                            .build_option
                            .res_options
                            .as_ref()
                            .map(|options| options.ignore_resource_pattern.clone())
                            .unwrap_or_default();
                        let preload_system_so_json = PreloadSystemSoArtifactWriter::effective_path(
                            &module_layout.preload_system_so_json(),
                        )?;

                        let har_pack_input = HarPackInput {
                            module_name: module.name.clone(),
                            source_dir: source_dir.clone(),
                            resources_dir: module_layout.source().resources(),
                            resource_str_dir: {
                                let dir = module_layout.res_resource_str_dir();
                                dir.exists().then_some(dir)
                            },
                            oh_package_json: package_manifest.clone(),
                            output_path: primary_output.clone(),
                            declarations_dir: declarations_dir.exists().then_some(declarations_dir),
                            consumer_rules_path: consumer_rules_path
                                .exists()
                                .then_some(consumer_rules_path),
                        };
                        let har_variant = HarVariantInput {
                            artifact_type,
                            module_root: module.src_path.clone(),
                            compiled_output_dir: module_layout.loader_out_dir(),
                            processed_module_json: Some(processed_module_json),
                            resource_table_path: resource_table.exists().then_some(resource_table),
                            native_libs,
                            native_header_dirs,
                            preload_system_so_json,
                            source_maps_path: source_maps,
                            build_profile_path: Some(module_layout.build_profile_for_module(
                                &module.module_type,
                                artifact_target_label,
                            )),
                            declaration_entries,
                            package_parameters,
                            worker_entries: module_build_option
                                .source_option
                                .as_ref()
                                .map(|options| options.workers.clone())
                                .unwrap_or_default(),
                            runtime_only: module_build_option
                                .ark_options
                                .as_ref()
                                .and_then(|options| options.runtime_only.clone()),
                            resource_directories: module_build_option
                                .res_options
                                .as_ref()
                                .map(|options| options.resource_directories.clone())
                                .unwrap_or_default(),
                            bundled_dependency_rewrite,
                            dependency_pkg_version,
                            bundled_dependencies,
                            bundled_resources,
                            bundled_resource_directories,
                            compiled_package_names: importee_metadata.compiled_package_names,
                            extra_importees: importee_metadata.extra_importees,
                            global_dependencies: importee_metadata.global_dependencies,
                            ignore_resource_patterns,
                            debug: config.build_mode.is_debuggable(),
                            target_name: artifact_target_label.clone(),
                            package_cache_dir: Some(
                                module_layout.package_har_cache_dir(artifact_target_label),
                            ),
                        };
                        let process_package_task = if is_ohpm_project {
                            "ProcessOHPackageJson"
                        } else {
                            "ProcessPackageJson"
                        };
                        let generated_package_manifest = if is_ohpm_project {
                            module_layout.generated_pm_oh_package(artifact_target_label)
                        } else {
                            module_layout.generated_pm_package_json(artifact_target_label)
                        };
                        packager
                            .process_har_package_json(
                                &har_pack_input,
                                &har_variant,
                                &generated_package_manifest,
                            )
                            .map_err(|error| {
                                anyhow::anyhow!("{process_package_task} failed: {error}")
                            })?;
                        let package_cache_dir =
                            module_layout.package_har_cache_dir(artifact_target_label);
                        packager
                            .process_har_artifacts(
                                &har_pack_input,
                                &har_variant,
                                &package_cache_dir,
                            )
                            .map_err(|error| {
                                anyhow::anyhow!("ProcessHarArtifacts failed: {error}")
                            })?;
                        let npm_executable =
                            (!is_ohpm_project).then(|| args.resolved_npm_executable());
                        let packed = packager
                            .pack_har_artifacts(
                                &har_pack_input,
                                &package_cache_dir,
                                npm_executable.as_deref(),
                            )
                            .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;
                        vec![packed]
                    }
                    _ => {
                        if module_json.exists() && modules_abc.exists() {
                            if !resources_index.exists() {
                                if let Some(parent) = resources_index.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::fs::write(&resources_index, [])?;
                            }
                            {
                                let raw_source_maps_path = package_out
                                    .source_maps_path
                                    .clone()
                                    .filter(|path| path.is_file())
                                    .or_else(|| {
                                        let candidate = module_layout.loader_out_source_maps();
                                        candidate.is_file().then_some(candidate)
                                    });
                                let source_maps_path =
                                    if matches!(config.build_mode, BuildMode::Release) {
                                        None
                                    } else {
                                        raw_source_maps_path.clone()
                                    };
                                let delivery_with_install = module_context
                                    .module
                                    .module_json
                                    .as_ref()
                                    .and_then(|module_json| {
                                        module_json.module.delivery_with_install
                                    })
                                    .unwrap_or(true);
                                let compress_native_libs =
                                    PackageModuleJsonOptions::new(&module_json)
                                        .compress_native_libs()?;
                                let pack_hap_start = Instant::now();
                                #[cfg(feature = "memory-diagnostics")]
                                arkdown::memory_diagnostics::dump_with(
                                    "build:before-pack-hap",
                                    format_args!(
                                        "module={} native_libs={} source_maps={}",
                                        module.name,
                                        native_libs.len(),
                                        source_maps_path.is_some()
                                    ),
                                );
                                let mut packed_outputs =
                                    Vec::with_capacity(package_artifacts.len());
                                for artifact in &package_artifacts {
                                    let pack_input = PackInput {
                                        module_name: module.name.clone(),
                                        target_name: artifact_target_label.clone(),
                                        abc_path: modules_abc.clone(),
                                        resources_index: resources_index.clone(),
                                        module_json: module_json.clone(),
                                        pack_info_module_json: Some(
                                            module_layout.merge_profile_module_json(),
                                        ),
                                        pack_info_path: Some(artifact.pack_info_path.clone()),
                                        native_libs: native_libs.clone(),
                                        compress_native_libs,
                                        output_path: artifact.package_path.clone(),
                                        resources_src_dir: Some(module_layout.res_resources_dir()),
                                        bundle_version_code,
                                        bundle_version_name: bundle_version_name.clone(),
                                        bundle_build_version: bundle_build_version.clone(),
                                        bundle_type: bundle_type.clone(),
                                        source_maps_path: source_maps_path.clone(),
                                        aot_mode: loader_json.an_build_mode.clone(),
                                        pkg_context_info_path: loader_json
                                            .use_normalized_ohm_url
                                            .then(|| module_layout.pkg_context_info_json()),
                                        integrated_hsps: integrated_hsps.clone(),
                                        device_types: manifest.device_types.clone(),
                                        installation_free: bundle_type == "atomicService",
                                        delivery_with_install,
                                        privacy_profile_path: privacy_profile_path.clone(),
                                        syscap_path: syscap_path.clone(),
                                    };
                                    let packed =
                                        if matches!(module_config.output_target, OutputTarget::Hsp)
                                        {
                                            packager.pack_hsp(pack_input)
                                        } else {
                                            packager.pack_hap(pack_input)
                                        }
                                        .map_err(|e| anyhow::anyhow!("Pack failed: {}", e))?;
                                    tracing::info!(
                                      module = %module.name,
                                      output = %packed.display(),
                                      "hap packed"
                                    );
                                    packed_outputs.push(packed);
                                }

                                if matches!(module_config.output_target, OutputTarget::Hsp) {
                                    let mut interface_dependencies = BTreeMap::new();
                                    let mut interface_dynamic_dependencies = BTreeMap::new();
                                    for dependency in
                                        module_context.deps.nodes.values().filter(|dependency| {
                                            !dependency.is_transitive
                                                && matches!(
                                                    dependency.node_type,
                                                    DependencyNodeType::SharedHsp
                                                        | DependencyNodeType::IntegratedHsp
                                                )
                                        })
                                    {
                                        let version =
                                            dependency.version_spec.clone().or_else(|| {
                                                dependency
                                                    .package
                                                    .as_ref()
                                                    .map(|package| package.version.clone())
                                            });
                                        if let Some(version) = version {
                                            let target = if dependency.is_dynamic {
                                                &mut interface_dynamic_dependencies
                                            } else {
                                                &mut interface_dependencies
                                            };
                                            target.insert(dependency.alias.clone(), version);
                                        }
                                    }
                                    let resource_table_path = module_layout.resource_table_txt();
                                    let resource_str_dir = module_layout.res_resource_str_dir();
                                    let obfuscation_rules_path =
                                        module_layout.obfuscation_export_rule_path();
                                    let interface_input = HspInterfaceHarInput {
                                        module_name: module.name.clone(),
                                        module_root: module.src_path.clone(),
                                        target_name: artifact_target_label.clone(),
                                        // Hvigor PrepareSharedHarRes always reads the module's
                                        // OHPM manifest and writes `oh-package.json5`; it does
                                        // not switch this interface-HAR boundary to package.json.
                                        oh_package_json: module_layout.oh_package_json5(),
                                        processed_module_json: module_layout
                                            .process_profile_module_json(),
                                        declarations_dir: module_layout.declarations_dir(),
                                        resource_table_path: resource_table_path
                                            .is_file()
                                            .then_some(resource_table_path),
                                        resource_str_dir: resource_str_dir
                                            .is_dir()
                                            .then_some(resource_str_dir),
                                        privacy_profile_path: privacy_profile_path.clone(),
                                        obfuscation_rules_path: obfuscation_rules_path
                                            .is_file()
                                            .then_some(obfuscation_rules_path),
                                        native_libs: interface_native_libs,
                                        native_header_dirs,
                                        exclude_so_from_interface_har,
                                        output_path: output_dir
                                            .join(format!("{}.har", module.name)),
                                        compatible_sdk_version: parse_api_version(
                                            &config.product.compatible_sdk_version,
                                        )
                                        .unwrap_or(config.sdk_config.compile_api_version),
                                        compatible_sdk_type: config.product.runtime_os.clone(),
                                        use_normalized_ohm_url: config
                                            .product
                                            .build_option
                                            .strict_mode
                                            .use_normalized_ohm_url,
                                        integrated_hsp: config
                                            .product
                                            .build_option
                                            .ark_options
                                            .as_ref()
                                            .and_then(|options| options.integrated_hsp),
                                        source_roots: interface_source_roots,
                                        dependencies: interface_dependencies,
                                        dynamic_dependencies: interface_dynamic_dependencies,
                                        debug: config.build_mode.is_debuggable(),
                                        obfuscated: module_obfuscation
                                            .is_enabled(&config.build_mode),
                                    };
                                    packager.pack_hsp_interface_har(interface_input).map_err(
                                        |e| anyhow::anyhow!("PackageSharedHar failed: {}", e),
                                    )?;
                                }
                                tracing::info!(
                                  module = %module.name,
                                  elapsed_ms = pack_hap_start.elapsed().as_millis(),
                                  artifact_count = packed_outputs.len(),
                                  "hap packaging completed"
                                );
                                #[cfg(feature = "memory-diagnostics")]
                                arkdown::memory_diagnostics::dump_with(
                                    "build:after-pack-hap",
                                    format_args!(
                                        "module={} artifacts={}",
                                        module.name,
                                        packed_outputs.len()
                                    ),
                                );
                                packed_outputs
                            }
                        } else {
                            Vec::new()
                        }
                    }
                };

                if !out.is_empty() {
                    // hvigor `packing-check.js`: `.so` extension-ability srcEntry
                    // existence per ABI + stray-source-file packaging warning.
                    let packing_check = PackingCheck::new(
                        config,
                        module,
                        manifest,
                        PathBuf::from(&loader_json.ace_module_build),
                        module_layout.stripped_native_libs_dir(),
                        module_layout.res_default_dir(),
                    );
                    packing_check
                        .validate()
                        .map_err(|error| anyhow::anyhow!("packing-check failed: {error}"))?;
                    let package_sign_har =
                        if matches!(module_config.output_target, OutputTarget::Har) {
                            PackageSignHarPlan::resolve(args, config, loader_json, session)
                        } else {
                            None
                        };
                    let is_package_signed =
                        if matches!(module_config.output_target, OutputTarget::Har) {
                            package_sign_har.is_some()
                        } else {
                            args.enable_sign_task && config.product.signing_config.is_some()
                        };
                    // Hvigor SignHap/SignHsp/SignApp remove stale signed outputs even
                    // when `enableSignTask=false`, while keeping the task in the graph.
                    if !matches!(module_config.output_target, OutputTarget::Har)
                        || is_package_signed
                    {
                        for artifact in &package_artifacts {
                            if artifact.signed_path.is_file() {
                                fs_err::remove_file(&artifact.signed_path)?;
                            }
                        }
                    }
                    let final_paths = if is_package_signed {
                        let sign_start = Instant::now();
                        #[cfg(feature = "memory-diagnostics")]
                        arkdown::memory_diagnostics::dump_with(
                            "build:before-sign",
                            format_args!("module={} artifacts={}", module.name, out.len()),
                        );
                        let signed_paths = if let Some(package_sign_har) = package_sign_har.as_ref()
                        {
                            let signed_path = &package_artifacts[0].signed_path;
                            let signing_root =
                                module_layout.package_sign_har_cache_dir(artifact_target_label);
                            packager
                                .stage_har_signing_directory(
                                    &module_layout.package_har_cache_dir(artifact_target_label),
                                    &signing_root,
                                )
                                .map_err(|error| {
                                    anyhow::anyhow!("PackageSignHar staging failed: {error}")
                                })?;
                            HarSignBackendRunner::new()
                                .sign_directory(
                                    &signing_root,
                                    &package_sign_har.sdk_sign_tool,
                                    config.product.signing_config.as_ref().context(
                                        "PackageSignHar lost its resolved signing configuration",
                                    )?,
                                )
                                .map_err(|error| {
                                    anyhow::anyhow!("PackageSignHar failed: {error}")
                                })?;
                            packager
                                .pack_signed_har_directory(&signing_root, signed_path)
                                .map_err(|error| {
                                    anyhow::anyhow!("PackageSignHar repack failed: {error}")
                                })?;
                            vec![signed_path.clone()]
                        } else {
                            let mut signed_paths = Vec::with_capacity(out.len());
                            for (path, artifact) in out.iter().zip(&package_artifacts) {
                                SignBackendRunner::new()
                                    .sign_or_skip(
                                        path,
                                        &artifact.signed_path,
                                        config.product.signing_config.as_ref(),
                                    )
                                    .map_err(|e| anyhow::anyhow!("Sign failed: {}", e))?;
                                signed_paths.push(artifact.signed_path.clone());
                            }
                            signed_paths
                        };
                        tracing::info!(
                          module = %module.name,
                          elapsed_ms = sign_start.elapsed().as_millis(),
                          artifact_count = signed_paths.len(),
                          "artifacts signed"
                        );
                        #[cfg(feature = "memory-diagnostics")]
                        arkdown::memory_diagnostics::dump_with(
                            "build:after-sign",
                            format_args!("module={} artifacts={}", module.name, out.len()),
                        );
                        signed_paths
                    } else {
                        for path in &out {
                            tracing::info!(
                              module = %module.name,
                              output = %path.display(),
                              "artifact remains unsigned"
                            );
                        }
                        out.clone()
                    };
                    if !matches!(module_config.output_target, OutputTarget::Har) {
                        let package_metadata_start = Instant::now();
                        let package_module_json = module_layout.package_module_json();
                        intermediate_writer.extract_zip_entry(
                            &out[0],
                            "module.json",
                            &package_module_json,
                        )?;
                        intermediate_writer
                            .rewrite_package_intermediate_module_json(&package_module_json)?;
                        for (path, artifact) in out.iter().zip(&package_artifacts) {
                            intermediate_writer.extract_zip_entry(
                                path,
                                "pack.info",
                                &artifact.pack_info_path,
                            )?;
                        }
                        tracing::info!(
                          module = %module.name,
                          elapsed_ms = package_metadata_start.elapsed().as_millis(),
                          "package intermediate metadata written"
                        );
                    }
                    let source_map_output = args
                        .hvigor_properties
                        .source_map_output(&config.project_root, &module.name)
                        .unwrap_or_else(|| module_layout.output_mapping_source_maps());
                    intermediate_writer.publish_package_source_maps(
                        package_out.source_maps_path.as_deref(),
                        &source_map_output,
                    )?;
                    let source_map_primary = package_out
                        .source_maps_path
                        .clone()
                        .unwrap_or_else(|| module_layout.loader_out_source_maps());
                    DebugSymbolCollector::new(
                        module.src_path.clone(),
                        module.name.clone(),
                        config.build_mode.clone(),
                        source_map_primary,
                        None,
                        PathBuf::from(&loader_json.cache_path)
                            .join("obfuscation")
                            .join("nameCache.json"),
                    )
                    .with_collection_override(args.collect_debug_symbol)
                    .with_output_variant(&config.product.name, artifact_target_label)
                    .collect()
                    .map_err(|error| anyhow::anyhow!("CollectDebugSymbol failed: {error}"))?;
                    if matches!(module_config.output_target, OutputTarget::Hsp) {
                        let shared_tgz_output = output_dir.join(output_name.shared_tgz());
                        if module_build_option.generate_shared_tgz.is_some()
                            && shared_tgz_output.is_file()
                        {
                            std::fs::remove_file(&shared_tgz_output)?;
                        }
                        let should_generate_shared_tgz = module_build_option
                            .generate_shared_tgz
                            .unwrap_or(matches!(config.build_mode, BuildMode::Release));
                        if should_generate_shared_tgz {
                            packager
                                .pack_hsp_tgz(HspTgzInput {
                                    hsp_path: out[0].clone(),
                                    interface_har_path: output_dir
                                        .join(format!("{}.har", module.name)),
                                    output_path: shared_tgz_output,
                                })
                                .map_err(|error| {
                                    anyhow::anyhow!("PackageSharedTgz failed: {error}")
                                })?;
                        }
                    }
                    output_paths.extend(final_paths.iter().cloned());

                    if artifact_target_label == "default"
                        && TargetBuildExecutionPlan::is_device_test(args)
                    {
                        let target_package_path = final_paths.first().ok_or_else(|| {
                            anyhow::anyhow!(
                                "device test target '{}:{}' produced no package",
                                module.name,
                                artifact_target_label
                            )
                        })?;

                        let bundle_type = module_config
                            .product
                            .app_options
                            .bundle_type
                            .as_deref()
                            .unwrap_or(&session.project.bundle_type);
                        crate::device_test::DeviceTestExecutionPlan::from_build(
                            crate::device_test::DeviceTestExecutionInput {
                                sdk_config: &module_config.sdk_config,
                                sdk_root: &session.sdk.sdk_root,
                                project_path: &config.project_root,
                                module_root: &module.src_path,
                                module_name: &module.name,
                                ohos_test_module_json: module_context
                                    .module
                                    .ohos_test_module_json_value
                                    .as_deref(),
                                target_artifact_name: module_config
                                    .product
                                    .artifact_name
                                    .as_deref(),
                                product_name: &module_config.product.name,
                                bundle_name: &module_config.product.bundle_name,
                                bundle_type,
                                target_package_path,
                                dependencies: module_context.deps.as_ref(),
                                coverage: args.ohos_test_coverage.unwrap_or(true),
                                scope: args.scope.as_deref().unwrap_or(""),
                                node_executable: args.resolved_node_executable(),
                            },
                        )
                        .and_then(|plan| plan.execute())
                        .map_err(|error| {
                            anyhow::anyhow!(
                                "GenerateDeviceCoverage failed for module '{}': {error}",
                                module.name
                            )
                        })?;
                    }
                }
            }
        }
        progress.done();

        let elapsed = start.elapsed();
        if output_paths.is_empty() {
            tracing::info!(
                elapsed_seconds = elapsed.as_secs_f32(),
                "build succeeded with no output files produced"
            );
        } else {
            for path in output_paths.iter() {
                tracing::info!(
                    artifact = %path.display(),
                    elapsed_seconds = elapsed.as_secs_f32(),
                    "build succeeded"
                );
            }
        }

        Ok(None)
    }
}
