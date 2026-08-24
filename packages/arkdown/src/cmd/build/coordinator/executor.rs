//! Coarse sequential build-stage executor owned by [`BuildCoordinator`].

use super::bundle_stage::CoordinatorBundleStageInput;
use super::bytecode_stage::CoordinatorBytecodeStageInput;
use super::package_sign_stage::PackageSignStageInput;
use super::*;

impl BuildCoordinator {
  /// Internal build pipeline implementation.
  ///
  /// Called by [`BuildCoordinator::execute`], which wraps it to guarantee
  /// unconditional `cleanUp` hook invocation.
  pub(crate) async fn execute_impl(self) -> Result<BuildOutput> {
    let start = Instant::now();

    let args = &self.args;
    let config = &self.config;
    let session = &self.session;
    let package_plugins = &self.package_plugins;
    let mut task_lifecycle =
      BuildTaskLifecycle::new(self.task_observer.clone(), args.native_task_roots.clone());
    if self.owns_task_cycle {
      task_lifecycle.begin_cycle().await?;
    }

    let hot_reload_mode = HotReloadTaskPlan::requested(&args.native_task_roots);
    let watch_mode =
      args.watch || args.preview || (hot_reload_mode && args.watch_invalidation.is_none());
    let collect_importers = args.watch || args.preview || hot_reload_mode;
    let is_incremental_watch = collect_importers || args.watch_invalidation.is_some();
    let watch_changed_files = args
      .watch_invalidation
      .as_ref()
      .map(|batch| batch.changed_files.clone())
      .unwrap_or_default();
    let preview_mode = args.preview;
    let preview_incremental_resources = matches!(config.output_target, OutputTarget::Preview)
      && args.hvigor_properties.previewer_compile_res_incremental;
    if preview_incremental_resources
      && !session
        .sdk
        .tools()
        .supports_preview_compile_resource_increment(arkdown_core::is_harmony_os(
          &config.product.runtime_os,
        ))
    {
      anyhow::bail!(
        "ARKDOWN_PREVIEW_INCREMENTAL_RESOURCE_SDK: compileResInc=true requires component API >= 9 and a toolchains component supporting incremental preview resources (SDK {})",
        session.sdk.compile_sdk_version
      );
    }
    // Local and on-device test builds default to code-coverage
    // instrumentation: hvigor injects `ohos-test-coverage=true` for `test`
    // and `onDeviceTest`; the CLI flag can still override it.
    let ohos_test_coverage = args.ohos_test_coverage.unwrap_or_else(|| {
      matches!(config.output_target, OutputTarget::Test)
        || TargetBuildExecutionPlan::is_device_test(args)
    });
    let coverage_mode = args.coverage || ohos_test_coverage;
    let disable_abc_cache = args.disable_abc_cache || !args.cache;
    let cli_cache_file = args.cache_file.clone();
    let abc_backend = args.abc_backend;
    let named_target = args.build_target_name.clone();
    let module_analyzer = ModuleAnalyzerKind::from(args.module_analyzer);

    let checker_selection = ArkTsCheckerSelection::parse(args.checker.as_deref())?;
    let checker_mode = checker_selection.oxc_mode();

    let project_root = args
      .project
      .clone()
      .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
    let project_layout = ProjectBuildLayout::new(&project_root);
    let quickfix_version_store =
      if QuickFixTaskPlan::patch_artifacts_requested(&args.native_task_roots) {
        Some(QuickFixVersionStore::from_runtime(
          args.hvigor_cache_dir.as_deref(),
        )?)
      } else {
        None
      };
    let quickfix_patch_version = quickfix_version_store
      .as_ref()
      .map(QuickFixVersionStore::current)
      .transpose()?;
    let mut quickfix_version_advanced = false;

    if let Some(ref named) = named_target {
      tracing::info!(
          target = %named,
          "multi-target: building with named target configuration '{}'",
          named
      );
    }

    let mut progress = Progress::new();

    let custom_cfg = session.custom_config.as_ref();
    let intermediate_writer = IntermediateArtifactWriter::new(&config.sdk_config.restool_path);

    let build_modules = ModuleSelectionPlanner::build_modules_for_target(
      config.as_ref(),
      args.module.as_deref(),
      &args.native_task_roots,
    )?;
    let build_module_configs = build_modules
      .iter()
      .map(|module| {
        ModuleBuildConfiguration::resolve_build(
          config.as_ref(),
          session.build_options.as_ref(),
          module,
          named_target.as_deref(),
          args.module_hvigor_overrides.get(&module.name),
          &args.hvigor_properties,
        )
      })
      .collect::<Result<Vec<_>>>()?;
    let mut local_package_source_roots = LocalPackageSourceRootInventory::default();
    for (module, resolved) in build_modules.iter().zip(&build_module_configs) {
      let module_layout = ModuleBuildLayout::for_output_target_product(
        &resolved.config.output_target,
        &module.src_path,
        &resolved.config.product.name,
        &resolved.target_name,
      );
      let generated_profile_root = module_layout
        .build_profile_for_module(&module.module_type, &resolved.target_name)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
          anyhow::anyhow!(
            "cannot derive generated profile root for module '{}' target '{}'",
            module.name,
            resolved.target_name
          )
        })?;
      let source_roots = resolved
        .target
        .source_roots
        .iter()
        .map(|root| module.src_path.join(root))
        .chain(std::iter::once(module.src_path.join("src").join("main")))
        .chain(std::iter::once(generated_profile_root));
      local_package_source_roots.insert(&module.src_path, source_roots);
    }
    let mut har_dedup_config = config.as_ref().clone();
    har_dedup_config.product.build_option.deduplicate_har = build_module_configs
      .iter()
      .any(|resolved| resolved.config.product.build_option.deduplicate_har);
    let har_dedup_project_plan =
      HarDedupProjectPlan::collect(&har_dedup_config, session.project.as_ref())?;
    #[cfg(feature = "memory-diagnostics")]
    arkdown::memory_diagnostics::dump_with(
      "build:start",
      format_args!(
        "modules={} target={:?} mode={}",
        build_modules.len(),
        config.output_target,
        args.mode
      ),
    );

    if coverage_mode {
      tracing::info!(
        "coverage mode enabled — instrumenting executable statements before ABC compilation"
      );
    }
    let extra_sanitizer_flags = SanitizerProfileFlags::new(
      args
        .hvigor_properties
        .asan_config
        .unwrap_or(args.ohos_debug_asan),
      args
        .hvigor_properties
        .tsan_config
        .unwrap_or(args.ohos_enable_tsan),
      args
        .hvigor_properties
        .hwasan_config
        .unwrap_or(args.ohos_enable_hwasan),
      args
        .hvigor_properties
        .ubsan_config
        .unwrap_or(args.ohos_enable_ubsan),
    );
    let native_sdk_toolchain = NativeSdkToolchain::new(
      session.sdk.sdk_root.clone(),
      session
        .sdk
        .tool_path(SdkToolKind::Cmake)
        .map(|tool| tool.path),
      session
        .sdk
        .tool_path(SdkToolKind::NativeNinja)
        .map(|tool| tool.path),
      session
        .sdk
        .tool_path(SdkToolKind::LlvmStrip)
        .map(|tool| tool.path),
      session
        .sdk
        .tool_path(SdkToolKind::Binxo)
        .map(|tool| tool.path),
    )
    .with_zig_enabled(args.enable_zig)?;
    let hwasan_config = args
      .hvigor_properties
      .hwasan_config
      .or(args.ohos_enable_hwasan.then_some(true));
    let binxo_config = args
      .hvigor_properties
      .binxo_config
      .or(args.ohos_enable_binxo.then_some(true));
    let sanitizer_native_policy = NativeLibCollectionPolicy::default()
      .with_sanitizers(
        extra_sanitizer_flags.native_sanitizers(),
        extra_sanitizer_flags
          .hwasan
          .then(|| {
            session
              .sdk
              .tool_path(SdkToolKind::LlvmReadElf)
              .map(|tool| tool.path)
          })
          .flatten(),
      )
      .with_build_mode(args.mode.clone())
      .with_sdk_toolchain(native_sdk_toolchain.clone())
      .with_binxo_config(hwasan_config, binxo_config);

    let native_dependency_plan = NativeDependencyStagePlanBuilder::new(config, session, args)
      .build(&build_modules, &build_module_configs)?;
    if !native_dependency_plan.is_empty() {
      let _timer = BuildTimingProbe::global("native_dependency_waves");
      NativeDependencyStageExecutor::new(
        &mut task_lifecycle,
        &sanitizer_native_policy,
        args.hvigor_properties.release_cmake_build_type.as_deref(),
      )
      .execute(&native_dependency_plan)
      .await?;
    }

    // ── Step 2: Profile + Resource (per module) ───────────────────────
    progress.step(2, 6, "Processing profiles");

    let permission_validator = arkdown_profile::PermissionValidator::new();
    let privacy_profile_processor = arkdown_profile::PrivacyProfileProcessor::new();
    let process_profile_processor = arkdown_profile::ProcessProfileProcessor::new();
    let permission_reason_processor = arkdown_profile::PermissionReasonProcessor::new();
    let share_config_processor = arkdown_profile::ShareConfigProcessor::new();
    let startup_config_processor = arkdown_profile::StartupConfigProcessor::new();
    let distro_filter_processor = arkdown_profile::DistroFilterProcessor::new();

    let mut loader_jsons = Vec::new();
    let mut package_resolution_plans: BTreeMap<String, Arc<PackageResolutionPlan>> =
      BTreeMap::new();
    let mut local_native_dependencies: BTreeMap<String, Arc<[NativeCmakeLocalDependency]>> =
      BTreeMap::new();
    let mut module_pack_info_plans = Vec::new();
    let mut all_bc_har_deps: Vec<Vec<ByteCodeHarScanEntry>> = Vec::new();
    let mut privacy_profile_paths: Vec<Option<PathBuf>> = Vec::new();

    let sdk_permissions = {
      let _timer = BuildTimingProbe::global("sdk_permission_data");
      if BuildPolicy::new(config.as_ref()).uses_sdk_permission_validation() {
        if let Some(module_json_path) = config.sdk_config.permission_module_json_path() {
          permission_validator
            .collect_sdk_permission_data(&module_json_path)
            .map_err(|e| {
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
      }
    };

    let app_scope_bundle_name = session.project.bundle_name.clone();
    let pac_schema_path = config
      .sdk_config
      .pac_schema_path_for_runtime(&config.product.runtime_os);
    // Match ets-loader's `globalModulePaths`: HarmonyOS component SDKs add
    // declaration roots through `ets/sdkConfig.json`. The same snapshot must
    // feed both semantic metadata and the transform source store so SDK
    // `@Builder` functions are classified before ArkUI lowering.
    let sdk_declaration_roots = config
      .sdk_config
      .arkguard_api_dirs(&config.product.runtime_os);
    let component_metadata = {
      let _timer = BuildTimingProbe::global("component_metadata");
      ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
        &config.sdk_config.ets_loader_path,
        &sdk_declaration_roots,
      )
      .map_err(|e| {
        anyhow::anyhow!(
          "SDK component metadata load failed from {}: {}",
          config.sdk_config.ets_loader_path.display(),
          e
        )
      })?
    };

    for (module, resolved_module) in build_modules.into_iter().zip(build_module_configs) {
      let _module_timer = BuildTimingProbe::module(&module.name, "module_total");
      let module_target_name = resolved_module.target_name;
      let module_target = resolved_module.target;
      let module_config = resolved_module.config;
      let module_build_option = &module_config.product.build_option;
      let sanitizer_profile_flags = extra_sanitizer_flags
        .with_cmake_arguments(module_build_option.external_native_options.as_ref());
      if module_build_option.external_native_options.is_some() {
        if sanitizer_profile_flags.enabled_count() > 1 {
          // BuildNativeWithCmake.checkExclusiveSan is advisory: hvigor only
          // evaluates it for modules that own an external native build and
          // still forwards every enabled sanitizer afterwards.
          tracing::warn!(
            module = %module.name,
            "Sanitizers are mutually exclusive. Enable only one of ASan, TSan, HWASan, or UBSan when using CMake."
          );
        }
        let compatible_api = parse_api_version(&module_config.product.compatible_sdk_version)
          .unwrap_or(config.sdk_config.compile_api_version);
        if sanitizer_profile_flags.asan && compatible_api <= 9 {
          tracing::warn!("To use ASan, first upgrade the SDK to API version 10 or later.");
        }
        if (sanitizer_profile_flags.tsan
          || sanitizer_profile_flags.hwasan
          || sanitizer_profile_flags.ubsan)
          && compatible_api <= 11
        {
          tracing::warn!(
            "To use TSan, HWASan, or UBSan, first upgrade the SDK to API version 12 or later."
          );
        }
      }
      let processor = ProfileProcessor::new(module_config.clone())
        .with_har_exclude_hsp_dependencies(args.hvigor_properties.har_exclude_hsp_dependencies)
        .with_compile_lib_entry_file(args.hvigor_properties.compile_lib_entry_file);
      // Local unit-test builds (`--target test`) use the `.test` build root
      // (hvigor `setTestDefaultParameters` injects `buildRoot=.test`);
      // named targets (e.g. `ohosTest`) key intermediate paths by target.
      let module_layout = ModuleBuildLayout::for_output_target_product(
        &module_config.output_target,
        &module.src_path,
        &module_config.product.name,
        &module_target_name,
      );
      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;
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
      // `ModulePathInfoIml.getOhosTestIntermediatesSrc*` is independent of
      // the active target build root: generated ohosTest templates always
      // live below `.test/<product>`, while target outputs use
      // `build/<product>/.../ohosTest`.
      let ohos_test_template_layout =
        ModuleBuildLayout::for_ohos_test_template(&module.src_path, &module_config.product.name);
      let package_resolution = Arc::new(
        ModulePackageResolutionPlanner::new(&module_context, &local_package_source_roots)
          .build()?,
      );
      package_resolution_plans.insert(module.name.clone(), package_resolution);
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      let skip_profile_writes = args.skip_profile;

      if task_lifecycle.is_observed() && task_lifecycle.before(&module.name, None, "clean").await? {
        HvigorModuleCleanStage::new(&project_root, &module.src_path, &native_sdk_toolchain)
          .with_native_variants(
            vec![config.product.name.clone()],
            vec![module_target_name.clone()],
            config.build_mode.as_str(),
          )
          .execute()
          .map_err(|error| anyhow::anyhow!("clean failed: {error}"))?;
        task_lifecycle.after(&module.name, None, "clean").await?;
      }
      let pre_build_validator =
        PreBuildValidator::new(config, session, &module, module_build_option);
      if task_lifecycle
        .before(&module.name, Some(&module_target_name), "PreBuild")
        .await?
      {
        if !skip_profile_writes {
          // `PreBuild.doTaskAction` owns schema, source, API, and structural
          // validation. `PreCheckSyscap` is a separate dependent task in
          // hvigor 6.24.3 and is deliberately not folded into this action.
          pre_build_validator
            .validate()
            .map_err(|error| anyhow::anyhow!("pre-build validation failed: {error}"))?;
        }
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "PreBuild")
          .await?;
      }
      // hvigor `GenerateUnitTestTemplate` writes the three local unit-test
      // templates into `<module>/.test/testability/` (declared output). It
      // runs for the `test` command entry (`hvigorw test`).
      if matches!(config.output_target, OutputTarget::Test)
        && task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GenerateUnitTestTemplate",
          )
          .await?
      {
        let _timer = BuildTimingProbe::module(&module.name, "generate_unit_test_template");
        if !module_layout.test_ability_ets().is_file() {
          let coverage = args.ohos_test_coverage.unwrap_or(true);
          let scope = args.scope.as_deref().unwrap_or("");
          // hvigor embeds the absolute module path (`pathInfo.getModulePath()`)
          // into the template; canonicalize so `--project .` does not leak
          // `./` segments into the generated file.
          let configured_module_path = if module.src_path.has_root() {
            module.src_path.clone()
          } else {
            config.project_root.join(&module.src_path)
          };
          let module_path = canonicalize_or_owned(&configured_module_path);
          crate::cmd::build::unit_test::generate_unit_test_template(
            &module_path,
            &module_config.product.name,
            &crate::cmd::build::unit_test::UnitTestTemplateParams {
              bundle_name: &config.product.bundle_name,
              module_name: &module.name,
              scope,
              coverage,
              module_path: &module_path,
            },
          )
          .map_err(|error| {
            anyhow::anyhow!(
              "GenerateUnitTestTemplate failed for module '{}': {error}",
              module.name
            )
          })?;
        }
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "GenerateUnitTestTemplate",
          )
          .await?;
      }
      // hvigor `GenerateOhosTestTemplate`: the `ohosTest` target build
      // (`-p module=entry@ohosTest`) copies the SDK test templates into
      // `.test/<product>/intermediates/src/ohosTest/`.
      if ohos_test_templates_after_migration
        && task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GenerateOhosTestTemplate",
          )
          .await?
      {
        let _timer = BuildTimingProbe::module(&module.name, "generate_ohos_test_template");
        if !crate::cmd::build::unit_test::has_ohos_test_template(
          &module.src_path,
          &module_config.product.name,
        ) {
          crate::cmd::build::unit_test::generate_ohos_test_template(
            &module.src_path,
            &module_config.product.name,
            config.sdk_config.compile_api_version,
          )
          .map_err(|error| {
            anyhow::anyhow!(
              "GenerateOhosTestTemplate failed for module '{}': {error}",
              module.name
            )
          })?;
        }
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "GenerateOhosTestTemplate",
          )
          .await?;
      }
      let module_info_task_name = if module.module_type == OhosModuleType::Har {
        "CreateHarModuleInfo"
      } else {
        "CreateModuleInfo"
      };
      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            module_info_task_name,
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          module_info_task_name,
        )
        .await?
      {
        let is_bytecode_har = module.module_type == OhosModuleType::Har
          && module_build_option.har_artifact_type == arkdown_core::HarArtifactType::Bytecode;
        let is_integrated_hsp = module.module_type == OhosModuleType::Shared
          && module_build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.integrated_hsp)
            .unwrap_or(false);
        let should_write_module_info = is_bytecode_har
          || args.ui_transform_optimization
          || (is_integrated_hsp && args.integrated_hsp_use_get_current_bundle_name);
        intermediate_writer.write_module_info(
          &module.src_path,
          &module_config.product.name,
          &module_target_name,
          should_write_module_info,
        )?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            module_info_task_name,
          )
          .await?;
      }
      let project_remote_hsp_exists = config.project_root.join("oh_modules").join(".hsp").is_dir();
      let module_remote_hsp_exists = module.src_path.join("oh_modules").join(".hsp").is_dir();
      if project_remote_hsp_exists
        || task_lifecycle.has_root(
          &module.name,
          Some(&module_target_name),
          "SignProjectRemoteHsp",
        )
      {
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "SignProjectRemoteHsp",
          )
          .await?
        {
          if args.enable_sign_task {
            let signer = RemoteHspSigner::new(config.clone(), session.clone());
            let module_name = module.name.clone();
            tokio::task::spawn_blocking(move || {
              signer.run_for_module(&module_name, RemoteHspSignScope::Project)
            })
            .await
            .context("SignProjectRemoteHsp worker failed")??;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "SignProjectRemoteHsp",
            )
            .await?;
        }
      }
      if project_remote_hsp_exists
        || module_remote_hsp_exists
        || task_lifecycle.has_root(
          &module.name,
          Some(&module_target_name),
          "SignModuleRemoteHsp",
        )
      {
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "SignModuleRemoteHsp",
          )
          .await?
        {
          if args.enable_sign_task {
            let signer = RemoteHspSigner::new(config.clone(), session.clone());
            let module_name = module.name.clone();
            tokio::task::spawn_blocking(move || {
              signer.run_for_module(&module_name, RemoteHspSignScope::Module)
            })
            .await
            .context("SignModuleRemoteHsp worker failed")??;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "SignModuleRemoteHsp",
            )
            .await?;
        }
      }
      if task_lifecycle
        .before(&module.name, Some(&module_target_name), "GenerateMetadata")
        .await?
      {
        let related_entries = module_context
          .module
          .related_entry_modules
          .as_deref()
          .unwrap_or_default()
          .iter()
          .filter_map(|entry_name| {
            let entry_module = config
              .modules
              .iter()
              .find(|candidate| candidate.name == *entry_name)?;
            let entry_context = session.module_context(entry_name)?;
            let device_types = entry_context
              .module
              .module_json
              .as_ref()
              .map(|profile| profile.module.device_types.clone())
              .unwrap_or_default();
            let related_target = ModuleTargetSelection::resolve_build(
              config,
              session.build_options.as_ref(),
              entry_module,
              Some(&module_target_name),
              &args.hvigor_properties,
            )
            .target;
            Some(RelatedEntryMetadata {
              module_name: entry_name.clone(),
              device_types,
              distro_filter: related_target.distribution_filter,
            })
          })
          .collect();
        let device_types = module_context
          .module
          .module_json
          .as_ref()
          .map(|profile| profile.module.device_types.as_slice())
          .unwrap_or_default();
        intermediate_writer.write_package_metadata(PackageMetadataInput {
          project_root: &config.project_root,
          module_root: &module.src_path,
          output_target: &module_config.output_target,
          product_name: &module_config.product.name,
          target_name: &module_target_name,
          module_name: &module.name,
          module_type: &module.module_type,
          artifact_name: module_config.product.artifact_name.as_deref(),
          device_types,
          is_signed: args.enable_sign_task && config.product.signing_config.is_some(),
          related_entries,
        })?;
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "GenerateMetadata")
          .await?;
      }
      let native_build_option =
        arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
          module_build_option,
        );
      let self_package = module_context
        .module
        .package
        .as_ref()
        .map(|package| NativeLibPackageInfo::local(package.name.clone(), package.version.clone()))
        .unwrap_or_else(|| NativeLibPackageInfo::local(module.name.clone(), String::new()));
      let cmake_local_dependencies = Arc::from(
        NativeCmakeLocalDependencyInventoryBuilder::new(
          config,
          session,
          args,
          &module_target_name,
          module_context.deps.as_ref(),
        )
        .build()?,
      );
      local_native_dependencies.insert(module.name.clone(), Arc::clone(&cmake_local_dependencies));
      let native_policy = sanitizer_native_policy
        .clone()
        .with_release_cmake_build_type(args.hvigor_properties.release_cmake_build_type.as_deref())
        .with_external_native_options(native_build_option.external_native_options.clone())
        .with_native_lib_option(native_build_option.native_lib.clone())
        .with_self_package(self_package)
        .with_cmake_local_dependency_inventory(Arc::clone(&cmake_local_dependencies));
      let native_collector = NativeLibCollector::new(
        module,
        &native_build_option.abi_filters,
        module_context.deps.as_ref(),
      )
      .with_output_variant(&module_config.product.name, &module_target_name)
      .with_policy(native_policy);
      let native_collector = if matches!(config.output_target, OutputTarget::Har) {
        native_collector
      } else {
        native_collector.excluding_dependency_roots(module_har_dedup_plan.duplicate_package_roots())
      };
      if task_lifecycle
        .before(&module.name, Some(&module_target_name), "ConfigureCmake")
        .await?
      {
        native_collector
          .configure_cmake_dependencies()
          .map_err(|error| anyhow::anyhow!("ConfigureCmake failed: {error}"))?;
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "ConfigureCmake")
          .await?;
      }
      if task_lifecycle.has_root(&module.name, None, "compileNative") {
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "BuildNativeWithCmake",
          )
          .await?
        {
          native_collector
            .configure_native()
            .map_err(|error| anyhow::anyhow!("BuildNativeWithCmake failed: {error}"))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "BuildNativeWithCmake",
            )
            .await?;
        }
        task_lifecycle
          .marker(&module.name, None, "compileNative")
          .await?;
      }
      let build_profile_task_name = if module.module_type == OhosModuleType::Har {
        "CreateHarBuildProfile"
      } else {
        "CreateBuildProfile"
      };
      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            build_profile_task_name,
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          build_profile_task_name,
        )
        .await?
      {
        let _timer = BuildTimingProbe::module(&module.name, "build_profile");
        let build_profile_path =
          module_layout.build_profile_for_module(&module.module_type, &module_target_name);
        let module_profile_path = module.src_path.join("build-profile.json5");
        let empty_fields = serde_json::Map::new();
        let fields = module_build_option
          .ark_options
          .as_ref()
          .map(|options| &options.build_profile_fields)
          .unwrap_or(&empty_fields);
        let kind = if module.module_type == OhosModuleType::Har {
          let version = module_context
            .module
            .package
            .as_deref()
            .map(|package| package.version.as_str())
            .ok_or_else(|| {
              anyhow::anyhow!(
                "HAR module '{}' requires oh-package.json5 for BuildProfile.ets generation",
                module.name
              )
            })?;
          BuildProfileKind::Har { version }
        } else {
          BuildProfileKind::Application {
            bundle_name: &session.project.bundle_name,
            bundle_type: &session.project.bundle_type,
            version_code: session.project.version_code,
            version_name: &session.project.version_name,
            product_name: &config.product.name,
            build_version: session.project.build_version.as_deref(),
          }
        };
        BuildProfileWriter::new()
          .write(BuildProfileInput {
            output_path: &build_profile_path,
            profile_path: &module_profile_path,
            kind,
            target_name: &module_target_name,
            build_mode: &config.build_mode,
            fields,
          })
          .map_err(|error| anyhow::anyhow!("CreateBuildProfile failed: {error}"))?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            build_profile_task_name,
          )
          .await?;
      }
      task_lifecycle
        .marker(&module.name, None, "GenerateBuildProfile")
        .await?;
      if task_lifecycle
        .before(&module.name, Some(&module_target_name), "PreCheckSyscap")
        .await?
      {
        pre_build_validator
          .validate_pre_check_syscap()
          .map_err(|error| anyhow::anyhow!("PreCheckSyscap failed: {error}"))?;
        task_lifecycle
          .after(&module.name, Some(&module_target_name), "PreCheckSyscap")
          .await?;
      }

      let module_json5_path = module_context
        .module
        .module_json_path
        .clone()
        .unwrap_or_else(|| module_layout.source().module_json5());
      // The `ohosTest` target builds the `src/ohosTest` module (`module.json5`
      // lives there with its own module name, e.g. `entry_test`). hvigor
      // `MergeProfile` spreads `getOhosTestAbilityConfig()` underneath the
      // module document when the source module.json5 lacks `srcEntry`
      // abilities (templates-after-source-code-migration).
      let (module_json5, module_json_source_value) = if module_target_name == "ohosTest" {
        let ohos_test_json5 = module_layout.source().ohos_test_module_json5();
        let raw_source = module_context
          .module
          .ohos_test_module_json_value
          .as_deref()
          .cloned()
          .ok_or_else(|| {
            anyhow::anyhow!(
              "missing ohosTest module.json5 snapshot for '{}' at '{}'",
              module.name,
              ohos_test_json5.display()
            )
          })?;
        let mut parsed: arkdown_core::ModuleJson5 = serde_json::from_value(raw_source.clone())
          .map_err(|error| {
            anyhow::anyhow!(
              "cannot decode ohosTest module.json5 '{}': {error}",
              ohos_test_json5.display()
            )
          })?;
        if ohos_test_templates_after_migration {
          // `getOhosTestAbilityConfig()`: description/mainElement/pages +
          // the synthetic TestAbility, overlaid by the source module.json5
          // (hvigor `MergeProfile`: `{...getOhosTestAbilityConfig(),
          // ...o.module}` — the source module document wins on conflicts).
          let base = serde_json::json!({
            "module": {
              "description": "$string:module_test_desc",
              "mainElement": "TestAbility",
              "pages": "$profile:test_pages",
              "abilities": [{
                "name": "TestAbility",
                "srcEntry": "./ets/testability/TestAbility.ets",
                "description": "$string:TestAbility_desc",
                "label": "$string:TestAbility_label",
                "exported": true,
              }],
            }
          });
          // Parse the raw source document as a JSON value so missing fields
          // stay missing (serde `ModuleJson5Module` would serialize empty
          // arrays for absent abilities and overwrite the template).
          // hvigor `o.module = {...getOhosTestAbilityConfig(), ...o.module}`
          // merges at the module-document level: the source document wins on
          // conflicting keys, the template fills the rest.
          let merged = match (base, raw_source) {
            (serde_json::Value::Object(mut base), serde_json::Value::Object(src)) => {
              if let (
                Some(serde_json::Value::Object(base_module)),
                Some(serde_json::Value::Object(src_module)),
              ) = (base.get_mut("module"), src.get("module"))
              {
                base_module.extend(src_module.clone());
              } else {
                base.extend(src);
              }
              serde_json::Value::Object(base)
            }
            (_, src) => src,
          };
          parsed = serde_json::from_value(merged.clone())
            .map_err(|error| anyhow::anyhow!("merge ohosTest module config: {error}"))?;
          (parsed, merged)
        } else {
          (parsed, raw_source)
        }
      } else {
        let parsed = module_context
          .module
          .module_json
          .as_ref()
          .cloned()
          .ok_or_else(|| anyhow::anyhow!("missing module.json5 snapshot for {}", module.name))?;
        let raw = module_context
          .module
          .module_json_value
          .as_deref()
          .cloned()
          .map(Ok)
          .unwrap_or_else(|| serde_json::to_value(&parsed))
          .map_err(|error| anyhow::anyhow!("encode module.json5 snapshot: {error}"))?;
        (parsed, raw)
      };
      let profile_dependency_context = {
        let _timer = BuildTimingProbe::module(&module.name, "profile_dependency_context");
        arkdown_profile::ProfileDependencyContext::from_session_graph(
          module_config.as_ref(),
          module,
          module_context.deps.as_ref(),
          config
            .product
            .version_code
            .unwrap_or(session.project.version_code),
        )
        .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
      };

      {
        let _timer = BuildTimingProbe::module(&module.name, "validate_compile_mode");
        process_profile_processor
          .validate_compile_mode(&module_json5, &module_json5_path.display().to_string())
          .map_err(|e| anyhow::anyhow!("{}", e))?;
      }

      // For hot rebuilds (skip_profile=true): run profile processing in
      // memory but skip all disk writes.  module.json5 and oh-package.json5
      // have not changed, so the previous build's disk outputs are still
      // valid.  We still need the in-memory `manifest` + `loader_json` for
      // the bundle and pack stages.
      //
      // # Upstream correspondence
      //
      // Mirrors hvigor's GenerateLoaderJson task being marked as cached when
      // no declared input has changed: the task body is not re-executed (disk
      // writes skipped) but the in-memory data is available to downstream
      // stages.
      if skip_profile_writes {
        tracing::debug!(
            module = %module.name,
            "hot rebuild: skip_profile=true — running profile in-memory, skipping writes"
        );
      }

      let declared_device_types: Vec<String> = if module_json5.module.device_types.is_empty() {
        vec!["phone".to_string()]
      } else {
        module_json5.module.device_types.clone()
      };
      let manifest_device_types =
        Self::target_device_types(&module.name, &declared_device_types, &module_target)?;

      // ── Resource compilation ──────────────────────────────────────
      let res_out = module_layout.res_default_dir();
      let res_layout = ResourceOutputLayout::new(&res_out);
      let har_dedup_enabled = module_build_option.deduplicate_har
        && !matches!(
          config.output_target,
          OutputTarget::Preview | OutputTarget::Test
        )
        && args.hvigor_properties.with_deduplicate.unwrap_or(true);
      if har_dedup_enabled
        && task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessIdDefinedFile",
          )
          .await?
      {
        let configured = module_build_option
          .res_options
          .as_ref()
          .and_then(|options| options.id_defined_file_path.as_deref())
          .map(|configured| {
            if configured.is_absolute() {
              configured.to_path_buf()
            } else {
              config.project_root.join(configured)
            }
          });
        IdDefinedFileProcessor::new(configured.as_deref(), &res_layout.id_defined_json())
          .process()
          .map_err(|error| anyhow::anyhow!("ProcessIdDefinedFile failed: {error}"))?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "ProcessIdDefinedFile",
          )
          .await?;
      }
      let skip_res = args.skip_resource;
      if skip_res {
        tracing::debug!(
            module = %module.name,
            "hot rebuild: skipping resource compilation (skip_resource=true)"
        );
      }

      let mut resource_refs = FxHashMap::default();

      // Preload SO list
      let preload_so_paths = &module_build_option.preload_so;
      if !preload_so_paths.is_empty() {
        let _timer = BuildTimingProbe::module(&module.name, "preload_so_list");
        let preload_so_out = module_layout.preload_so_dir();
        std::fs::create_dir_all(&preload_so_out).ok();
        let so_list: Vec<String> = preload_so_paths
          .iter()
          .map(|path| ArtifactPathSerializer::path_to_slash_string(path))
          .collect::<Result<Vec<_>, _>>()
          .map_err(|error| anyhow::anyhow!("serialize preloadSo path failed: {error}"))?;
        let json = serde_json::json!({ "soFileList": so_list });
        if !skip_profile_writes {
          intermediate_writer
            .write_json_pretty(&module_layout.preload_so_list_json(), &json)
            .unwrap_or_else(|e| {
              tracing::warn!(error = %e, "failed to write preload_so_list.json");
            });
        }
      }

      // ── Profile processing ────────────────────────────────────────
      let ability_names: Vec<String> = module_json5
        .module
        .abilities
        .iter()
        .map(|a| a.name.clone())
        .collect();
      {
        let _timer = BuildTimingProbe::module(&module.name, "validate_request_permissions");
        permission_validator
          .validate_request_permissions(arkdown_profile::RequestPermissionValidationInput {
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
          })
          .map_err(|e| anyhow::anyhow!("validate request permissions failed: {}", e))?;
      }

      if matches!(module_json5.module.type_.as_str(), "shared" | "har")
        || matches!(&config.output_target, OutputTarget::Hsp | OutputTarget::Har)
      {
        let _timer = BuildTimingProbe::module(&module.name, "permission_reason_resources");
        permission_reason_processor
          .process_permission_reason_resources(arkdown_profile::PermissionReasonResourceInput {
            module_root: &module.src_path,
            target_name: &module_target_name,
            request_permissions: &module_json5.module.request_permissions,
          })
          .map_err(|e| anyhow::anyhow!("process permission reason resources failed: {}", e))?;
      } else if matches!(&config.output_target, OutputTarget::Hap) {
        let _timer = BuildTimingProbe::module(&module.name, "dependency_permission_reason_merge");
        permission_reason_processor
          .merge_dependency_permission_reason_resources_from_dependency_graph(
            arkdown_profile::DependencyPermissionReasonGraphResourceInput {
              module_root: &module.src_path,
              project_root: &project_root,
              target_name: &module_target_name,
              dependency_graph: module_context.deps.as_ref(),
            },
          )
          .map_err(|e| {
            anyhow::anyhow!("merge dependency permission reason resources failed: {}", e)
          })?;
      }

      {
        let _timer = BuildTimingProbe::module(&module.name, "shortcut_profiles");
        arkdown_profile::ShortcutProfileProcessor::new(&module.name)
          .process_shortcut_profiles(arkdown_profile::ShortcutProfileInput {
            module_root: &module.src_path,
            metadata: &module_json5.module.metadata,
            app_bundle_name: &app_scope_bundle_name,
            product_bundle_name: &config.product.bundle_name,
            output_profile_dir: &res_layout.profile_dir(),
          })
          .map_err(|e| anyhow::anyhow!("process shortcut profiles failed: {}", e))?;
      }

      let pkg_context_info = if module_build_option.strict_mode.use_normalized_ohm_url {
        let _timer = BuildTimingProbe::module(&module.name, "pkg_context_info_generate");
        let input =
          PkgContextInfoInputBuilder::new(module_config.as_ref(), session, module, &module_context)
            .with_local_native_dependencies(Arc::clone(&cmake_local_dependencies))
            .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
            .build()?;
        Some(
          PkgContextInfoGenerator::new()
            .generate(&input)
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?,
        )
      } else {
        None
      };

      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgContextInfo",
          )
          .await?;
      } else if task_lifecycle
        .before(
          &module.name,
          Some(&module_target_name),
          "GeneratePkgContextInfo",
        )
        .await?
      {
        if let Some(pkg_context_info) = pkg_context_info.as_ref() {
          let _timer = BuildTimingProbe::module(&module.name, "pkg_context_info_write");
          PkgContextInfoGenerator::new()
            .write_generated(pkg_context_info, &module_layout.pkg_context_info_json())
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
        }
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgContextInfo",
          )
          .await?;
      }

      // `GeneratePkgSdkInfo` is registered globally, but latest hvigor only
      // schedules it through the HAP/HSP pack task graph. A normal HAR build
      // therefore has no `intermediates/loader/.../pkgSdkInfo.json`; retain a
      // directly selected native task root for plugin/task-runner parity.
      let runs_pkg_sdk_info = module.module_type != OhosModuleType::Har
        || task_lifecycle.has_root(
          &module.name,
          Some(&module_target_name),
          "GeneratePkgSdkInfo",
        );
      if runs_pkg_sdk_info {
        if skip_profile_writes {
          task_lifecycle
            .up_to_date(
              &module.name,
              Some(&module_target_name),
              "GeneratePkgSdkInfo",
            )
            .await?;
        } else if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GeneratePkgSdkInfo",
          )
          .await?
        {
          if !module_build_option.strict_mode.use_normalized_ohm_url {
            arkdown_profile::PkgSdkInfoGenerator::new(
              module_context.module.as_ref(),
              module_context.deps.as_ref(),
            )
            .write(&module_layout.pkg_sdk_info_json())
            .map_err(|error| anyhow::anyhow!("GeneratePkgSdkInfo failed: {error}"))?;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "GeneratePkgSdkInfo",
            )
            .await?;
        }
      }

      let privacy_profile_path = {
        let _timer = BuildTimingProbe::module(&module.name, "module_privacy_profile");
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessModulePrivacyProfile",
          )
          .await?
        {
          let output = if BuildPolicy::module_processes_privacy_profile(&module.module_type) {
            privacy_profile_processor
              .process_module_privacy_profile(arkdown_profile::PrivacyProfileInput {
                module_root: &module.src_path,
                target_name: &module_target_name,
                pac_schema_path: pac_schema_path.as_deref(),
              })
              .map_err(|e| anyhow::anyhow!("ProcessModulePrivacyProfile failed: {}", e))?
              .map(|output| output.processed_path)
          } else {
            None
          };
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessModulePrivacyProfile",
            )
            .await?;
          output
        } else {
          None
        }
      };

      let custom_type_inventory = {
        let configured_types = module_build_option
          .ark_options
          .as_ref()
          .map(|options| options.types.as_slice())
          .unwrap_or_default();
        let base_ets_root = module_layout.source().ets();
        let target_ets_root = module_layout.source().target_ets(&module_target_name);
        let has_arkts_sources = module_context
          .module
          .source_roots
          .iter()
          .any(|root| root.file_name().and_then(|name| name.to_str()) == Some("ets"))
          || base_ets_root.is_dir()
          || target_ets_root.is_dir();
        let module_profile_path = module.src_path.join("build-profile.json5");
        let profile_path = if module_profile_path.is_file() {
          module_profile_path
        } else {
          project_root.join("build-profile.json5")
        };
        let dependency_custom_types = args
          .hvigor_properties
          .dependencies_types_enable
          .then(|| {
            DependencyCustomTypeInputBuilder::new(
              module_context.deps.as_ref(),
              config.as_ref(),
              session.build_options.as_ref(),
              &module_target_name,
              &args.module_hvigor_overrides,
              &args.hvigor_properties,
            )
            .collect()
          })
          .transpose()?
          .unwrap_or_default();
        CustomTypeResolver::new(
          &module.name,
          &module.src_path,
          &profile_path,
          module_config.sdk_config.compile_api_version,
          has_arkts_sources,
          module_context.deps.as_ref(),
        )
        .resolve_with_dependencies(configured_types, &dependency_custom_types)
        .map_err(|error| anyhow::anyhow!("custom types validation failed: {error}"))?
      };

      // Hvigor writes MergeProfile and ProcessProfile at distinct task
      // boundaries. Build the canonical profile once in memory, then emit
      // only each task's declared artifacts before its lifecycle `after`.
      let form_widget_abilities = merge_form_widget_abilities(config, &module_json5)?;
      let merge_profile_enabled = if skip_profile_writes {
        false
      } else {
        task_lifecycle
          .before(&module.name, Some(&module_target_name), "MergeProfile")
          .await?
      };
      let (manifest, mut loader_json, processed_profile_value, pack_info_plans) = {
        let _timer = BuildTimingProbe::module(&module.name, "process_module");
        let (manifest, loader_json) = processor
          .process_module_with_dependency_context_and_target(
            module,
            &module_json5,
            &profile_dependency_context,
            &module_target_name,
          )
          .map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut processed_profile_value = processor
          .module_json_value_with_session_context(
            &manifest,
            None,
            &profile_dependency_context,
            Some(&module_json_source_value),
            &module_target.source_abilities,
          )
          .map_err(|error| anyhow::anyhow!("construct module profile failed: {error}"))?;
        sanitizer_profile_flags
          .apply_to_value(&mut processed_profile_value)
          .map_err(|error| anyhow::anyhow!("apply sanitizer profile flags failed: {error}"))?;
        let pack_info_plans = Self::pack_info_plans(
          config.as_ref(),
          module,
          module_config.as_ref(),
          session.as_ref(),
          args,
          &module_target_name,
          &module_target,
          &module_layout,
        )?;

        if skip_profile_writes {
          task_lifecycle
            .up_to_date(&module.name, Some(&module_target_name), "MergeProfile")
            .await?;
        } else if merge_profile_enabled {
          intermediate_writer.write_merge_profile_intermediate(
            &module_layout,
            &processed_profile_value,
            module_json5.module.extension_abilities.len(),
          )?;
          task_lifecycle
            .after(&module.name, Some(&module_target_name), "MergeProfile")
            .await?;
        }

        if skip_profile_writes {
          task_lifecycle
            .up_to_date(&module.name, Some(&module_target_name), "MakePackInfo")
            .await?;
        } else if task_lifecycle
          .before(&module.name, Some(&module_target_name), "MakePackInfo")
          .await?
        {
          let packager = Packager::new(module_config.clone());
          for plan in &pack_info_plans {
            packager
              .write_pack_info(&plan.input, &plan.output_path)
              .map_err(|error| anyhow::anyhow!("MakePackInfo failed: {error}"))?;
          }
          task_lifecycle
            .after(&module.name, Some(&module_target_name), "MakePackInfo")
            .await?;
        }

        if skip_profile_writes {
          task_lifecycle
            .up_to_date(&module.name, Some(&module_target_name), "ProcessProfile")
            .await?;
        } else {
          if !task_lifecycle
            .before(&module.name, Some(&module_target_name), "ProcessProfile")
            .await?
          {
            anyhow::bail!(
              "ARKDOWN_DISABLED_REQUIRED_HVIGOR_TASK: {}:{}@ProcessProfile is required to construct the in-memory manifest",
              module.name,
              module_target_name
            );
          }
          intermediate_writer.write_process_profile_intermediates(
            &module_layout,
            &processed_profile_value,
            module_json5.module.extension_abilities.len(),
            &form_widget_abilities,
          )?;
          task_lifecycle
            .after(&module.name, Some(&module_target_name), "ProcessProfile")
            .await?;
        }
        (
          manifest,
          loader_json,
          processed_profile_value,
          pack_info_plans,
        )
      };
      if matches!(
        module_build_option.har_artifact_type,
        arkdown_core::HarArtifactType::Bytecode
      ) {
        loader_json.include_byte_code_har_module_info(
          &module_layout.generated_module_info(&module_target_name),
        );
      }
      if let Some(pkg_context_info) = pkg_context_info.as_ref() {
        let bundled_dependencies = module_build_option
          .ark_options
          .as_ref()
          .is_some_and(|options| options.bundled_dependencies);
        let package_names = module_context
          .deps
          .bytecode_har_ohm_nodes_in_traversal_order(
            bundled_dependencies,
            args.byte_code_har_integrated_optimization,
          )
          .into_iter()
          .map(|node| node.package_name.clone())
          .collect::<Vec<_>>();
        loader_json.apply_bytecode_har_ohm_packages(&package_names, pkg_context_info);
      }
      loader_json.compiler_types = custom_type_inventory.compiler_types;
      loader_json.compiler_type_declaration_inputs = custom_type_inventory.declaration_inputs;
      if let Some(package) = module_context.module.package.as_deref() {
        let _timer = BuildTimingProbe::module(&module.name, "oh_exports");
        OhExportsProcessor::new()
          .process(
            package,
            &module_context.module.module_type,
            &mut loader_json,
          )
          .map_err(|error| anyhow::anyhow!("process oh-exports failed: {error}"))?;
      }

      // Profile disk writes.  Skipped for hot rebuilds (skip_profile=true)
      // since module.json5 / oh-package.json5 have not changed and the
      // previous build's outputs are still valid.
      let module_json_out = res_out.join("module.json");
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "write_module_json_initial");
        processor
          .write_module_json_value(&processed_profile_value, &module_json_out)
          .map_err(|e| anyhow::anyhow!("write module.json failed: {}", e))?;
      }

      // `ProcessRouterMap` feeds BasePackHapTask. Source/bytecode HARs carry
      // their router profile through `ProcessHarArtifacts` instead, so normal
      // HAR assembly must not create the two loader-router intermediates.
      // Preserve explicit native task invocation for the plugin task graph.
      let runs_router_map_task = module.module_type != OhosModuleType::Har
        || task_lifecycle.has_root(&module.name, Some(&module_target_name), "ProcessRouterMap");
      if runs_router_map_task {
        if skip_profile_writes {
          task_lifecycle
            .up_to_date(&module.name, Some(&module_target_name), "ProcessRouterMap")
            .await?;
        } else if task_lifecycle
          .before(&module.name, Some(&module_target_name), "ProcessRouterMap")
          .await?
        {
          let _timer = BuildTimingProbe::module(&module.name, "router_map_intermediates");
          intermediate_writer.write_router_map_intermediates(
            module,
            &loader_json,
            module_config.as_ref(),
            &res_out,
          )?;
          intermediate_writer.apply_compile_resource_module_metadata(
            &module_json_out,
            !loader_json.router_map.is_empty(),
            &res_layout.profile_dir().join("insight_intent.json"),
          )?;
          task_lifecycle
            .after(&module.name, Some(&module_target_name), "ProcessRouterMap")
            .await?;
        }
      }

      let process_share_config = if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "ProcessShareConfig",
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessShareConfig",
          )
          .await?
      };
      let share_out = if process_share_config || skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "share_config");
        let collect_dependency_share_config = module.module_type != OhosModuleType::Har
          || module_build_option
            .ark_options
            .as_ref()
            .is_some_and(|options| options.bundled_dependencies);
        let workspace_module_roots = module_config
          .modules
          .iter()
          .map(|project_module| canonicalize_or_owned(&project_module.src_path))
          .collect::<FxHashSet<_>>();
        let dependency_packages = if collect_dependency_share_config {
          module_context
            .deps
            .nodes
            .values()
            .filter(|node| {
              matches!(
                node.node_type,
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
              )
            })
            .filter_map(|node| {
              node.package_root.as_ref().map(|package_root| {
                arkdown_profile::ShareConfigDependencyInput {
                  package_name: node.package_name.clone(),
                  package_root: package_root.clone(),
                  is_local: workspace_module_roots.contains(&canonicalize_or_owned(package_root)),
                }
              })
            })
            .collect()
        } else {
          Vec::new()
        };
        let share_out = share_config_processor
          .merge_share_config(arkdown_profile::ShareConfigInput {
            module_root: &module.src_path,
            dependency_packages,
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
        if process_share_config {
          let share_config_out = module_layout.share_config_temp_json();
          // Latest `ProcessShareConfig.processHarShareConfig` only writes its
          // declared output when a HAR actually has target share-config
          // entries. HAP/HSP use `processHapOrHspShareConfig`, which writes an
          // empty handoff as well. Remove a stale HAR handoff when a config is
          // deleted so CompileResource cannot consume obsolete state.
          let should_write_temp_share_config =
            !matches!(module.module_type, OhosModuleType::Har) || !share_out.entries.is_empty();
          if should_write_temp_share_config {
            share_config_processor
              .write_share_config(&share_out, &share_config_out)
              .map_err(|e| anyhow::anyhow!("write share config failed: {}", e))?;
          } else if share_config_out.is_file() {
            std::fs::remove_file(&share_config_out).map_err(|error| {
              anyhow::anyhow!(
                "remove stale HAR share-config handoff {} failed: {error}",
                share_config_out.display()
              )
            })?;
          }
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessShareConfig",
            )
            .await?;
        }
        share_out
      } else {
        arkdown_profile::ShareConfigOutput { entries: vec![] }
      };

      let process_startup_config = if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "ProcessStartupConfig",
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessStartupConfig",
          )
          .await?
      };
      let startup_out = if process_startup_config || skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "startup_config");
        let startup_out = startup_config_processor
          .merge_startup_config(arkdown_profile::StartupConfigInput {
            project_root: &config.project_root,
            module_root: &module.src_path,
            module_name: &module.name,
            bundle_name: Some(config.product.bundle_name.as_str()),
            app_startup: module_json5.module.app_startup.clone(),
            collect_preload_system_so: module_build_option.preload_system_so
              && matches!(
                module.module_type,
                arkdown_core::OhosModuleType::Entry | arkdown_core::OhosModuleType::Feature
              )
              && !matches!(
                config.output_target,
                OutputTarget::Preview | OutputTarget::Test
              ),
          })
          .map_err(|e| anyhow::anyhow!("ProcessStartupConfig failed: {}", e))?;
        if process_startup_config {
          startup_config_processor
            .write_startup_config(&startup_out, &module_layout.startup_config_temp_json())
            .map_err(|e| anyhow::anyhow!("write startup config failed: {}", e))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessStartupConfig",
            )
            .await?;
        }
        startup_out
      } else {
        arkdown_profile::StartupConfigOutput { config: None }
      };

      if !skip_profile_writes {
        {
          let _timer = BuildTimingProbe::module(&module.name, "startup_profile_ref_update");
          ModuleJsonReader::new(module_json_out.parent().unwrap_or(&module_json_out))
            .ensure_startup_profile_ref(&module_json_out, &startup_out, &intermediate_writer)
            .map_err(|e| anyhow::anyhow!("update startup module.json failed: {}", e))?;
        }
        {
          let _timer = BuildTimingProbe::module(&module.name, "distro_filter_profile");
          distro_filter_processor
            .process_resolved_distro_filter_profile(
              arkdown_profile::DistroFilterProfileInput {
                project_root: &project_root,
                module_name: &module.name,
                module_root: &module.src_path,
                product_name: &config.product.name,
                target_name: Some(&module_target_name),
                build_mode_name: Some(config.build_mode.as_str()),
                compile_api_version: config.sdk_config.compile_api_version,
                metadata: &module_json5.module.metadata,
                output_profile_dir: &res_layout.profile_dir(),
                module_json_path: Some(&module_json_out),
              },
              module_target.distribution_filter.as_ref(),
            )
            .map_err(|e| anyhow::anyhow!("process distro filter profile failed: {}", e))?;
        }
        // hvigor `UnitTestProcessProfile` (runs before CompileResource so
        // restool annotates the injected TestAbility): when the generated
        // testability template exists (`<module>/.test/testability/TestAbility.ets`),
        // append the synthetic TestAbility to the processed profile and
        // overwrite the ark module json / module.json with the same document.
        if matches!(config.output_target, OutputTarget::Test)
          && module_layout.test_ability_ets().is_file()
        {
          let process_profile_json = module_layout.process_profile_module_json();
          let process_value = std::fs::read_to_string(&process_profile_json).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile cannot read processed profile '{}': {error}",
              process_profile_json.display()
            )
          })?;
          let mut document: serde_json::Value =
            serde_json::from_str(&process_value).map_err(|error| {
              anyhow::anyhow!(
                "UnitTestProcessProfile cannot parse processed profile '{}': {error}",
                process_profile_json.display()
              )
            })?;
          crate::cmd::build::unit_test::inject_unit_test_ability(
            &mut document,
            module_json5.module.type_.as_str(),
          )
          .map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile failed for module '{}': {error}",
              module.name
            )
          })?;
          // hvigor `outputJSONSync(..., { spaces: "\t" })` for both files.
          let serialized = {
            let mut out = Vec::new();
            let mut serializer = serde_json::Serializer::with_formatter(
              &mut out,
              serde_json::ser::PrettyFormatter::with_indent(b"\t"),
            );
            serde::Serialize::serialize(&document, &mut serializer)
              .map_err(|error| anyhow::anyhow!("UnitTestProcessProfile serialize: {error}"))?;
            out
          };
          std::fs::write(&process_profile_json, &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write '{}': {error}",
              process_profile_json.display()
            )
          })?;
          std::fs::write(&module_layout.res_ark_module_json(), &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write ark_module.json '{}': {error}",
              module_layout.res_ark_module_json().display()
            )
          })?;
          std::fs::write(&module_json_out, &serialized).map_err(|error| {
            anyhow::anyhow!(
              "UnitTestProcessProfile write module.json '{}': {error}",
              module_json_out.display()
            )
          })?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "UnitTestProcessProfile",
            )
            .await?;
        }

        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_initial");
        let mut module_res_dirs: Vec<PathBuf> =
          ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
            .select()
            .into_iter()
            .filter(|d| d.exists())
            .collect();
        // The `ohosTest` target compiles the generated template resources
        // (`.test/<product>/intermediates/src/ohosTest/resources`) that define
        // `$string:module_test_desc` / `$string:TestAbility_*` / test_pages.
        if ohos_test_templates_after_migration {
          let template_resources = ohos_test_template_layout.ohos_test_resources_root();
          if template_resources.is_dir() && !module_res_dirs.contains(&template_resources) {
            module_res_dirs.push(template_resources);
          }
        }
        if module_res_dirs.is_empty() {
          intermediate_writer.write_resource_config_files(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
          )?;
        } else {
          intermediate_writer.write_resource_config_files_with_dirs_and_names(
            &project_root,
            module,
            module_config.as_ref(),
            &res_out,
            &module_res_dirs,
            &[],
            if module_target_name == "ohosTest" {
              std::slice::from_ref(&module_json5.module.name)
            } else {
              &[]
            },
            &module_target_name,
          )?;
        }
      }

      let process_resource_task_name = if preview_incremental_resources {
        "PreviewProcessResource"
      } else {
        "ProcessResource"
      };
      let process_resource_enabled = if skip_res {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            process_resource_task_name,
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            process_resource_task_name,
          )
          .await?
      };
      let (app_scope_res, res_dirs) = {
        let _timer = BuildTimingProbe::module(&module.name, "resource_dir_select");
        let include_app_scope =
          ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
            .include_app_scope();
        let mut res_dirs = ResourceInputRootCollector::default();
        let app_scope_res = canonicalize_or_owned(&project_layout.app_scope_resources());
        if include_app_scope && app_scope_res.exists() {
          res_dirs.insert_resolved(app_scope_res.clone());
        }
        if let Some(res_opts) = module_build_option.res_options.as_ref() {
          for dir in &res_opts.resource_directories {
            res_dirs.insert_existing(dir.clone());
          }
        }
        for resource_module in
          ModuleSelectionPlanner::resource_modules_for_build(config.as_ref(), module)
        {
          let resource_module_config = ModuleBuildConfiguration::resolve_dependency(
            config.as_ref(),
            session.build_options.as_ref(),
            resource_module,
            Some(&module_target_name),
            args.module_hvigor_overrides.get(&resource_module.name),
            &args.hvigor_properties,
          )?;
          for module_res_dir in ResourceDirSelector::new(
            resource_module_config.config.as_ref(),
            resource_module,
            &resource_module_config.target,
          )
          .select()
          {
            res_dirs.insert_existing(module_res_dir);
          }
        }
        for dependency_res_dir in
          HarResourceDependencySelector::new(&module.src_path, module_context.deps.as_ref())
            .select()
        {
          if module_har_dedup_plan
            .duplicate_package_roots()
            .iter()
            .any(|root| dependency_res_dir.starts_with(root))
          {
            continue;
          }
          // `HarResourceDependencySelector` already returns canonical roots;
          // preserve that startup resolution instead of repeating filesystem
          // canonicalization for every dependency.
          res_dirs.insert_resolved(dependency_res_dir);
        }
        // `AbstractResource.getRestoolConfigBuilder` registers the permission
        // reason `resource_str` intermediate after all HAR resource roots.
        let resource_str = module_layout.res_resource_str_dir();
        res_dirs.insert_existing(resource_str);
        // The `ohosTest` target compiles the generated template resources
        // (`.test/<product>/intermediates/src/ohosTest/resources`) that define
        // `$string:module_test_desc` / `$string:TestAbility_*` / test_pages.
        if ohos_test_templates_after_migration {
          let template_resources = ohos_test_template_layout.ohos_test_resources_root();
          res_dirs.insert_existing(template_resources);
        }
        (app_scope_res, res_dirs.into_paths())
      };
      if !skip_profile_writes {
        let _timer = BuildTimingProbe::module(&module.name, "resource_config_write_final");
        let mut current_module_resource_dirs = ResourceInputRootCollector::default();
        for dir in ResourceDirSelector::new(module_config.as_ref(), module, &module_target).select()
        {
          current_module_resource_dirs.insert_existing(dir);
        }
        if ohos_test_templates_after_migration {
          current_module_resource_dirs
            .insert_existing(ohos_test_template_layout.ohos_test_resources_root());
        }
        let current_module_resource_dirs = current_module_resource_dirs.into_paths();
        let dependency_resource_dirs: Vec<PathBuf> = res_dirs
          .iter()
          .filter(|dir| **dir != app_scope_res)
          .filter(|dir| !current_module_resource_dirs.contains(dir))
          .cloned()
          .collect();
        intermediate_writer.write_resource_config_files_with_dirs_and_names(
          &project_root,
          module,
          module_config.as_ref(),
          &res_out,
          &current_module_resource_dirs,
          &dependency_resource_dirs,
          if module_target_name == "ohosTest" {
            std::slice::from_ref(&module_json5.module.name)
          } else {
            &[]
          },
          &module_target_name,
        )?;
      }
      if process_resource_enabled {
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            process_resource_task_name,
          )
          .await?;
      }

      let resource_backend =
        ResourceBackendRunner::default_sdk(config.sdk_config.restool_path.clone());
      let resource_compile_config = ResourceCompileConfig {
        package_name: config.product.bundle_name.clone(),
        module_name: module.name.clone(),
        // The `ohosTest` target packs the `src/ohosTest` module under its own
        // module name (e.g. `entry_test`); restool `--modules` must match the
        // processed profile name.
        module_names: if module_target_name == "ohosTest" {
          vec![module_json5.module.name.clone()]
        } else {
          config.modules.iter().map(|m| m.name.clone()).collect()
        },
        device_types: manifest_device_types,
        is_debug: config.build_mode.is_debuggable(),
      };

      let compile_resource_task_name = if preview_incremental_resources {
        "PreviewCompileResource"
      } else {
        "CompileResource"
      };
      let compile_resource_enabled = if skip_res || res_dirs.is_empty() {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            compile_resource_task_name,
          )
          .await?;
        false
      } else {
        task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            compile_resource_task_name,
          )
          .await?
      };
      if compile_resource_enabled {
        let _timer = BuildTimingProbe::module(&module.name, "resource_compile_restool");
        match resource_backend.compile(
          &resource_compile_config,
          ResourceCompileInput {
            resources_dirs: res_dirs,
            output_dir: res_out.clone(),
          },
        ) {
          Ok(out) => {
            resource_refs = out
              .resource_infos
              .into_iter()
              .map(|(name, info)| {
                (
                  CompiledResourceKey::new(&name).into_string(),
                  ResourceReference {
                    id: info.id,
                    runtime_type: info.runtime_type,
                    bundle_name: config.product.bundle_name.clone(),
                    module_name: module.name.clone(),
                  },
                )
              })
              .collect();
            let resource_ids = out.resource_ids;
            if !skip_profile_writes {
              let _timer = BuildTimingProbe::module(&module.name, "resource_ids_profile_rewrite");
              processor
                .annotate_module_json_resource_ids(&module_json_out, &resource_ids)
                .map_err(|e| anyhow::anyhow!("annotate module.json resource IDs failed: {}", e))?;
            }
          }
          Err(e) => {
            return Err(anyhow::anyhow!(
              "resource compilation failed for module '{}': {}",
              module.name,
              e
            ));
          }
        }
      }
      // Restool materializes source resources into its output directory and
      // can overwrite processed profiles. Hvigor's CompileResource task then
      // emits the runtime-only startup/share profiles and updates module.json,
      // so perform the same handoff after restool returns.
      if compile_resource_enabled && !skip_profile_writes {
        if let Some(compiled_startup_config) = startup_out.compiled_resource_profile() {
          let profile_path = module_layout.res_profile_dir().join("startup_config.json");
          intermediate_writer
            .write_json_minified(&profile_path, &compiled_startup_config)
            .map_err(|e| anyhow::anyhow!("write startup resource profile failed: {}", e))?;
        }
        if !share_out.entries.is_empty() {
          let profile_name = module_json5
            .module
            .cross_app_shared_config
            .profile_name()
            .unwrap_or("default_share_config");
          let profile_path = module_layout
            .res_profile_dir()
            .join(format!("{profile_name}.json"));
          share_config_processor
            .write_compiled_share_config(&share_out, profile_name, &profile_path, &module_json_out)
            .map_err(|e| anyhow::anyhow!("write compiled share config failed: {}", e))?;
        }
        sanitizer_profile_flags
          .apply_to_module_json(&module_json_out)
          .map_err(|e| anyhow::anyhow!("apply sanitizer profile flags failed: {}", e))?;
      }
      {
        let _timer = BuildTimingProbe::module(&module.name, "system_resource_references");
        resource_refs.extend(
          SystemResourceReferenceLoader::new(
            &config.sdk_config.ets_loader_path,
            &config.product.bundle_name,
            &module.name,
          )
          .load()?,
        );
      }
      if compile_resource_enabled {
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            compile_resource_task_name,
          )
          .await?;
      }

      if matches!(module_config.output_target, OutputTarget::Har) {
        let mut process_obfuscation = ModuleObfuscationContext::resolve(
          module_config.as_ref(),
          session,
          &module,
          module_build_option,
        )?;
        process_obfuscation.target_name = Some(module_target_name.clone());
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ProcessObfuscationFiles",
          )
          .await?
        {
          process_obfuscation.process_consumer_rules(
            &module_context,
            &module_config.build_mode,
            module_config.sdk_config.compile_api_version,
            module_layout.obfuscation_export_rule_path(),
          )?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ProcessObfuscationFiles",
            )
            .await?;
        }
      }

      if matches!(config.output_target, OutputTarget::Preview) {
        let package_name = module_context
          .module
          .package
          .as_ref()
          .map(|package| package.name.as_str())
          .filter(|name| !name.is_empty())
          .unwrap_or(&module.name);
        let preview = crate::cmd::build::preview::PreviewAssetProcessor::new(
          &project_root,
          &module.src_path,
          &module.name,
          &module.module_type,
          &module_config.product.name,
          &module_target_name,
          &manifest.compile_mode,
          package_name,
        );
        if preview_incremental_resources
          && task_lifecycle
            .before(
              &module.name,
              Some(&module_target_name),
              "CopyPreviewProfile",
            )
            .await?
        {
          preview.copy_profile_for_incremental_resource()?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "CopyPreviewProfile",
            )
            .await?;
        }
        task_lifecycle
          .marker(
            &module.name,
            Some(&module_target_name),
            "PreviewHookCompileResource",
          )
          .await?;
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "ReplacePreviewerPage",
          )
          .await?
        {
          preview.replace_page(
            args.hvigor_properties.previewer_replace_page.as_deref(),
            args.hvigor_properties.compile_hsp_dependencies_truthy,
            args.hvigor_properties.main_module.as_deref(),
          )?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "ReplacePreviewerPage",
            )
            .await?;
        }
        if preview_incremental_resources {
          let current_module_resources =
            ResourceDirSelector::new(module_config.as_ref(), module, &module_target)
              .select()
              .into_iter()
              .filter(|path| path.is_dir())
              .collect::<Vec<_>>();
          let har_resources =
            HarResourceDependencySelector::new(&module.src_path, module_context.deps.as_ref())
              .select()
              .into_iter()
              .filter(|resource| {
                !module_har_dedup_plan
                  .duplicate_package_roots()
                  .iter()
                  .any(|root| resource.starts_with(root))
              })
              .collect::<Vec<_>>();
          // CompileResourcesUtil.getRestoolModuleNames(): module directory
          // name, manifest module name, then every project submodule name,
          // deduplicated in first-seen order.
          let mut compile_module_names = Vec::with_capacity(config.modules.len() + 2);
          for name in std::iter::once(module.name.as_str())
            .chain(std::iter::once(module_json5.module.name.as_str()))
            .chain(config.modules.iter().map(|module| module.name.as_str()))
          {
            if !compile_module_names.iter().any(|current| current == name) {
              compile_module_names.push(name.to_string());
            }
          }
          preview.persist_incremental_resource_parameters(
            app_scope_res.is_dir().then_some(app_scope_res.as_path()),
            &current_module_resources,
            &har_resources,
            &compile_module_names,
          )?;
        }
      }

      let loader_json_out = module_layout.loader_json();

      // Scan ByteCode HAR dependencies.
      //
      // Primary path: consume `DependencyGraphSnapshot::bytecode_har_nodes()`
      // from the session dependency graph.  When the session graph has
      // `BytecodeHar` nodes with `package_root` set, we read their
      // `oh-package.json5` directly rather than doing an independent
      // `oh_modules/` filesystem walk.
      //
      // # Upstream correspondence
      //
      // Mirrors hvigor's `DependencyManager` providing typed node lists to
      // `generate-loader-json.ts` instead of re-scanning `oh_modules/`.
      let module_context = session
        .module_context(&module.name)
        .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
      let mut bc_har_deps = {
        let _timer = BuildTimingProbe::module(&module.name, "bytecode_har_scan");
        let typed_roots: Vec<ByteCodeHarDependencyRoot> =
          if module_build_option.strict_mode.use_normalized_ohm_url
            && !matches!(config.output_target, OutputTarget::Har)
          {
            module_context
              .deps
              .bytecode_har_nodes_in_traversal_order()
              .into_iter()
              .filter_map(|node| {
                node
                  .package_root
                  .as_ref()
                  .map(|root| ByteCodeHarDependencyRoot {
                    package_name: node.package_name.clone(),
                    package_root: root.clone(),
                    is_har_dependency: node
                      .module_profile
                      .as_ref()
                      .and_then(|profile| profile.module_type.as_ref())
                      .is_some_and(|module_type| matches!(module_type, OhosModuleType::Har)),
                  })
              })
              .collect()
          } else {
            Vec::new()
          };

        let bc_har_deps = ByteCodeHarMetadataWriter::scan_from_typed_roots(&typed_roots);

        if !typed_roots.is_empty() {
          tracing::debug!(
              module = %module.name,
              count = bc_har_deps.len(),
              "ByteCode HAR scan: used {} typed session-graph roots",
              typed_roots.len()
          );
        }

        bc_har_deps
      };

      bc_har_deps
        .retain(|entry| !module_har_dedup_plan.contains_bytecode_package(&entry.package_name));

      loader_json.apply_bytecode_har_scan_entries(
        &bc_har_deps,
        &module.src_path,
        &config.project_root,
        config.output_target.clone(),
      );

      if module_target_name == "ohosTest" {
        // The ohosTest loader names the test module (`entry_test`) and maps
        // it to the same source path (hvigor `modulePathMap`). Apply this
        // before GenerateLoaderJson writes the SDK handoff so the file and
        // the in-memory compiler model cannot diverge.
        loader_json.module_name = module_json5.module.name.clone();
        let test_module_path =
          std::fs::canonicalize(&module.src_path).unwrap_or_else(|_| module.src_path.clone());
        loader_json.module_path_map.insert(
          module_json5.module.name.clone(),
          arkdown_path::ArtifactPathSerializer::path_to_slash_string(&test_module_path)?,
        );
      }

      if skip_profile_writes {
        task_lifecycle
          .up_to_date(
            &module.name,
            Some(&module_target_name),
            "GenerateLoaderJson",
          )
          .await?;
      } else {
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "GenerateLoaderJson",
          )
          .await?
        {
          let _timer = BuildTimingProbe::module(&module.name, "loader_json_write");
          processor
            .write_loader_json(&loader_json, &loader_json_out)
            .map_err(|e| anyhow::anyhow!("write loader.json with byteCodeHarInfo failed: {}", e))?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "GenerateLoaderJson",
            )
            .await?;
        }
      }

      if matches!(config.output_target, OutputTarget::Preview) {
        task_lifecycle
          .marker(&module.name, None, "buildPreviewerResource")
          .await?;
        if task_lifecycle
          .before(
            &module.name,
            Some(&module_target_name),
            "PreviewUpdateAssets",
          )
          .await?
        {
          let package_name = module_context
            .module
            .package
            .as_ref()
            .map(|package| package.name.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&module.name);
          crate::cmd::build::preview::PreviewAssetProcessor::new(
            &project_root,
            &module.src_path,
            &module.name,
            &module.module_type,
            &module_config.product.name,
            &module_target_name,
            &manifest.compile_mode,
            package_name,
          )
          .update_assets()?;
          task_lifecycle
            .after(
              &module.name,
              Some(&module_target_name),
              "PreviewUpdateAssets",
            )
            .await?;
        }
      }

      // hvigor `ReplaceUnitTestIndexFile` (after CompileResource): rewrite
      // the res module.json pages and emit `main_pages.json` from the
      // `unit.test.replace.page` default.
      if matches!(config.output_target, OutputTarget::Test) {
        let process_profile_json = module_layout.process_profile_module_json();
        let process_value = std::fs::read_to_string(&process_profile_json)?;
        let document: serde_json::Value = serde_json::from_str(&process_value)?;
        crate::cmd::build::unit_test::replace_unit_test_index_file(
          &module_layout.res_module_json(),
          &document,
          &module_layout.res_profile_dir(),
          module_json5.module.type_.as_str(),
          args
            .unit_test_replace_page
            .as_deref()
            .unwrap_or(crate::cmd::build::unit_test::UNIT_TEST_REPLACE_PAGE),
        )
        .map_err(|error| {
          anyhow::anyhow!(
            "ReplaceUnitTestIndexFile failed for module '{}': {error}",
            module.name
          )
        })?;
        task_lifecycle
          .after(
            &module.name,
            Some(&module_target_name),
            "ReplaceUnitTestIndexFile",
          )
          .await?;
        // `BuildUnitTestHook` is an empty hvigor hook task whose boundary
        // closes only after GenerateLoaderJson, CompileResource and
        // ReplaceUnitTestIndexFile have completed. The Rust coordinator owns
        // those actions directly, so it must expose the same hook boundary to
        // the same-process task engine before UnitTestArkTS starts.
        task_lifecycle
          .marker(&module.name, Some(&module_target_name), "BuildUnitTestHook")
          .await?;
      }

      if ReloadResourceTaskPlan::requested_for_module(&args.native_task_roots, &module.name) {
        HotReloadProcessor::new(&module_layout, &project_root).validate_resource_model(
          &loader_json.compile_mode,
          config.sdk_config.compile_api_version,
        )?;
        // BuildHotReloadResource is targetless and completes only after its
        // target-scoped GenerateLoaderJson and CompileResource dependencies.
        task_lifecycle
          .marker(&module.name, None, "buildHotReloadResource")
          .await?;
      }
      loader_jsons.push((
        module.clone(),
        manifest,
        loader_json,
        resource_refs,
        module_target_name,
        module_config,
      ));
      module_pack_info_plans.push(pack_info_plans);
      all_bc_har_deps.push(bc_har_deps);
      privacy_profile_paths.push(privacy_profile_path);
    }
    progress.done();

    // `LintArkTS` is not an alias for the normal CompileArkTS checker hook.
    // Hvigor's HAR task invokes `etsStandaloneChecker` with exactly one
    // package entry after PreBuild/ProcessProfile/CreateHarBuildProfile. Run
    // that SDK-owned phase only when the task graph selected the linter root;
    // completing the root then terminates the partial native build before
    // bundling, just like the upstream task boundary.
    let mut ark_linter_plans = Vec::new();
    let mut ark_linter_tasks = Vec::new();
    for (module, _manifest, loader_json, _resource_refs, module_target_name, module_config) in
      &loader_jsons
    {
      if !task_lifecycle.has_root(&module.name, Some(module_target_name), "LintArkTS")
        || !task_lifecycle
          .before(&module.name, Some(module_target_name), "LintArkTS")
          .await?
      {
        continue;
      }
      if !matches!(module.module_type, OhosModuleType::Har) {
        anyhow::bail!(
          "ARKDOWN_ARK_LINTER_MODULE_TYPE: LintArkTS is only available for HAR modules; '{}' is {:?}",
          module.name,
          module.module_type
        );
      }
      let module_context = session.module_context(&module.name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for {}", module.name)
      })?;
      let package_resolution = package_resolution_plans.get(&module.name).ok_or_else(|| {
        anyhow::anyhow!(
          "missing package resolution plan for module '{}'",
          module.name
        )
      })?;
      let module_layout = ModuleBuildLayout::for_output_target_product(
        &config.output_target,
        &module.src_path,
        &config.product.name,
        module_target_name,
      );
      let generated_profile_path = module_layout
        .build_profile_for_module(&module.module_type, module_target_name)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
          anyhow::anyhow!(
            "ArkLinter cannot derive generated profile directory for module '{}'",
            module.name
          )
        })?;
      let paths = SdkEtsCheckerPaths::build(
        module_layout.res_profile_dir(),
        module_layout.res_ark_module_json(),
        module_layout.resource_table_txt(),
        module_layout.res_resources_dir().join("rawfile"),
        module_layout.source().ets(),
        module_layout.source().src_main().join("supervisual"),
        module_layout.loader_json(),
        PathBuf::from(&loader_json.cache_path),
        PathBuf::from(&loader_json.ace_module_build),
        generated_profile_path,
        module_layout.generated_resource_table_typescript(),
      );
      ark_linter_plans.push(
        SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
          config: module_config.as_ref(),
          session: session.as_ref(),
          module,
          module_context: &module_context,
          package_resolution,
          module_build_option: &module_config.product.build_option,
          loader_json: None,
          target_name: module_target_name,
          node_executable: args.resolved_node_executable(),
          paths,
          entries: vec![ArkLinterEntryResolver::new(&module.src_path).resolve()?],
          coverage_enabled: false,
          coverage_path_filter: None,
          coverage_mode: None,
          debug_line: false,
          byte_code_har_integrated_optimization: args.byte_code_har_integrated_optimization,
          single_file_emit: false,
          rollup_cache_use_source_hash: args.hvigor_properties.rollup_cache_use_source_hash,
        })
        .build()?,
      );
      ark_linter_tasks.push((module.name.clone(), module_target_name.clone()));
    }
    if !ark_linter_plans.is_empty() {
      SdkSemanticCheckStage::new(ark_linter_plans).run()?;
      for (module_name, target_name) in ark_linter_tasks {
        task_lifecycle
          .after(&module_name, Some(&target_name), "LintArkTS")
          .await?;
      }
    }

    // Latest ets-loader registers `etsChecker` before `etsTransform` and runs
    // it from `buildStart`.  Profiles/loader entries are now complete, while
    // no Rolldown transform has run yet, which is the equivalent native
    // boundary. The SDK checker remains an explicit stage so transform hooks
    // never read SDK/configuration state.
    if checker_selection.runs_sdk_semantic_checker() {
      let semantic_checker_start = Instant::now();
      let mut checker_plans = Vec::with_capacity(loader_jsons.len());
      for (module, _manifest, loader_json, _resource_refs, module_target_name, module_config) in
        &loader_jsons
      {
        if !BuildPolicy::new(module_config.as_ref()).module_requires_arkts_compilation(module)? {
          continue;
        }
        let module_context = session.module_context(&module.name).ok_or_else(|| {
          anyhow::anyhow!("missing build session module context for {}", module.name)
        })?;
        let package_resolution = package_resolution_plans.get(&module.name).ok_or_else(|| {
          anyhow::anyhow!(
            "missing package resolution plan for module '{}'",
            module.name
          )
        })?;
        let module_layout = ModuleBuildLayout::for_output_target_product(
          &config.output_target,
          &module.src_path,
          &config.product.name,
          module_target_name,
        );
        let generated_profile_path = module_layout
          .build_profile_for_module(&module.module_type, module_target_name)
          .parent()
          .map(Path::to_path_buf)
          .ok_or_else(|| {
            anyhow::anyhow!(
              "SDK etsChecker cannot derive generated profile directory for module '{}'",
              module.name
            )
          })?;
        let ace_module_root = if module_target_name == "ohosTest" {
          let source = module_layout.source();
          let target_ets = source.target_ets(module_target_name);
          if module_context
            .module
            .ohos_test_module_json_value
            .as_deref()
            .is_some_and(|module_json| source.ohos_test_templates_after_migration(module_json))
          {
            ModuleBuildLayout::for_ohos_test_template(&module.src_path, &module_config.product.name)
              .ohos_test_ets_root()
          } else {
            target_ets
          }
        } else {
          module_layout.source().ets()
        };
        let ace_super_visual_path = if module_target_name == "ohosTest" {
          module_layout.source().ohos_test_dir().join("supervisual")
        } else {
          module_layout.source().src_main().join("supervisual")
        };
        let paths = SdkEtsCheckerPaths::build(
          module_layout.res_profile_dir(),
          module_layout.res_ark_module_json(),
          module_layout.resource_table_txt(),
          module_layout.res_resources_dir().join("rawfile"),
          ace_module_root.clone(),
          ace_super_visual_path,
          module_layout.loader_json(),
          PathBuf::from(&loader_json.cache_path),
          PathBuf::from(&loader_json.ace_module_build),
          generated_profile_path,
          module_layout.generated_resource_table_typescript(),
        );
        let entries = SdkEtsCheckerEntryCollector::new(loader_json, &ace_module_root).collect();
        checker_plans.push(
          SdkEtsCheckerPlanBuilder::new(SdkEtsCheckerInput {
            config: module_config.as_ref(),
            session: session.as_ref(),
            module,
            module_context: &module_context,
            package_resolution,
            module_build_option: &module_config.product.build_option,
            loader_json: Some(loader_json),
            target_name: module_target_name,
            node_executable: args.resolved_node_executable(),
            paths,
            entries,
            coverage_enabled: coverage_mode,
            coverage_path_filter: args.coverage_path_filter.as_deref(),
            coverage_mode: args.coverage_mode.as_deref(),
            debug_line: false,
            byte_code_har_integrated_optimization: args.byte_code_har_integrated_optimization,
            single_file_emit: module_config.build_mode.is_debuggable()
              && args.hvigor_properties.single_file_emit
              && !coverage_mode,
            rollup_cache_use_source_hash: args.hvigor_properties.rollup_cache_use_source_hash,
          })
          .build()?,
        );
      }
      if !checker_plans.is_empty() {
        SdkSemanticCheckStage::new(checker_plans).run()?;
        tracing::info!(
          elapsed_ms = semantic_checker_start.elapsed().as_millis(),
          "SDK etsChecker semantic stage completed"
        );
      }
    }

    // ── Step 3: Bundling ──────────────────────────────────────────────
    let bundle_stage_output = self
      .run_bundle_stage(CoordinatorBundleStageInput {
        args,
        config,
        session,
        package_plugins,
        task_lifecycle: &mut task_lifecycle,
        collect_importers,
        is_incremental_watch,
        watch_changed_files,
        coverage_mode,
        disable_abc_cache,
        module_analyzer,
        checker_mode,
        project_root: &project_root,
        progress: &mut progress,
        custom_cfg,
        intermediate_writer: &intermediate_writer,
        har_dedup_project_plan: &har_dedup_project_plan,
        loader_jsons: &mut loader_jsons,
        package_resolution_plans: &package_resolution_plans,
        local_native_dependencies: &local_native_dependencies,
        all_bc_har_deps: &all_bc_har_deps,
        sdk_declaration_roots: &sdk_declaration_roots,
        component_metadata: &component_metadata,
      })
      .await?;
    let mut bundle_outs = bundle_stage_output.bundle_outs;
    let widget_bundle_outs = bundle_stage_output.widget_bundle_outs;
    let compile_task_active = bundle_stage_output.compile_task_active;
    let module_obfuscation_contexts = bundle_stage_output.module_obfuscation_contexts;

    // ── Obfuscation (release builds only) ────────────────────────────
    let mut pending_bytecode_obfuscations = (0..bundle_outs.len())
      .map(|_| None)
      .collect::<Vec<Option<PendingBytecodeObfuscation>>>();
    for (
      bundle_index,
      (
        (
          (module, _manifest, loader_json, _resource_refs, _module_target_name, _module_config),
          (bundle_out,),
        ),
        module_obfuscation,
      ),
    ) in loader_jsons
      .iter()
      .zip(bundle_outs.iter())
      .zip(module_obfuscation_contexts.iter())
      .enumerate()
    {
      if bundle_out.bypasses_compilation() {
        continue;
      }
      if QuickFixTaskPlan::for_module(&args.native_task_roots, &module.name).is_some()
        || HotReloadTaskPlan::for_module(&args.native_task_roots, &module.name).is_some()
      {
        // HotReloadArkCompile and ColdReloadArkCompile remove
        // `obfuscationOptions` before submitting the SDK worker. A reload
        // patch must preserve the base build's record/symbol names.
        continue;
      }
      let Some(obf_config) = module_obfuscation.config.as_ref() else {
        continue;
      };
      let obf_cache_dir = PathBuf::from(&loader_json.cache_path).join("obfuscation");
      let dependency_rule_files = module_obfuscation.dependency_rule_files.clone();
      let obfuscation_plan = ObfuscationPlan::from_build_options(
        &config.build_mode,
        Some(obf_config),
        ObfuscationOutputPaths::new(obf_cache_dir.clone()),
        None,
      );
      if obfuscation_plan.should_obfuscate() {
        obfuscation_plan
          .validate()
          .map_err(|e| anyhow::anyhow!("Obfuscation config invalid: {}", e))?;
        let rule_files = obfuscation_plan.all_rule_files();
        let arkguard_plan = ModuleArkguardPlanBuilder::new(
          config.as_ref(),
          session,
          module,
          loader_json,
          bundle_out,
          args.resolved_node_executable(),
        );
        let (arkguard, module_input) = arkguard_plan.build()?;
        let obfuscator = Obfuscator::new(ObfuscationOptions {
          enable_obfuscation: obfuscation_plan.should_obfuscate(),
          rule_files,
          dependency_rule_files: dependency_rule_files.clone(),
          cache_dir: obf_cache_dir,
          export_rule_path: None,
          arkguard: Some(arkguard),
        })
        .map_err(|e| anyhow::anyhow!("Obfuscation config invalid: {}", e))?;
        match obfuscator.mode() {
          ObfuscationMode::Bytecode => {
            pending_bytecode_obfuscations[bundle_index] = Some(PendingBytecodeObfuscation::new(
              obfuscator,
              arkguard_plan.bytecode_input(module_input),
            ));
          }
          ObfuscationMode::Source => {
            obfuscator
              .obfuscate_module(module_input)
              .map_err(|e| anyhow::anyhow!("Obfuscation failed: {}", e))?;
          }
          ObfuscationMode::Disabled => {}
        }
      }

      tracing::debug!(module = %module.name, "module obfuscation stage completed");
    }

    // ── Coverage instrumentation ──────────────────────────────────────
    if coverage_mode {
      if ohos_test_coverage {
        let test_mode = if matches!(config.output_target, OutputTarget::Test) {
          crate::cmd::build::unit_test::TestCoverageMode::UnitTest
        } else {
          crate::cmd::build::unit_test::TestCoverageMode::OhosTest
        };
        for (
          (module, _manifest, _loader_json, _resource_refs, _module_target_name, _module_config),
          (bundle_out,),
        ) in loader_jsons.iter().zip(bundle_outs.iter())
        {
          if bundle_out.bundle_output.module_source_files.is_empty() {
            continue;
          }
          crate::cmd::build::unit_test::TestCoverageInstrumentationPlan::from_build(
            &config.sdk_config,
            &config.project_root,
            &module.src_path,
            test_mode,
            args.coverage_mode.as_deref(),
            &bundle_out.bundle_output.module_source_files,
            args.resolved_node_executable(),
          )?
          .execute()
          .with_context(|| {
            format!(
              "test coverage instrumentation failed for module '{}'",
              module.name
            )
          })?;
        }
      } else {
        for (bundle_out,) in &bundle_outs {
          for (_chunk_name, path) in &bundle_out.bundle_output.chunk_files {
            if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
              if let Ok(source) = fs_err::read_to_string(path) {
                let file_id = ArtifactPathSerializer::path_to_slash_string(path)?;
                let instrumented = CoverageInstrumenter::new(&source, &file_id).instrument();
                let _ = fs_err::write(path, instrumented);
              }
            }
          }
        }
      }
    }

    // ── Step 5: ABC compilation ───────────────────────────────────────
    let bytecode_output = self
      .run_bytecode_stage(CoordinatorBytecodeStageInput {
        args,
        config,
        session,
        package_plugins,
        task_lifecycle: &mut task_lifecycle,
        hot_reload_mode,
        coverage_mode,
        disable_abc_cache,
        cli_cache_file: &cli_cache_file,
        abc_backend,
        module_analyzer,
        project_root: &project_root,
        progress: &mut progress,
        intermediate_writer: &intermediate_writer,
        har_dedup_project_plan: &har_dedup_project_plan,
        loader_jsons: &loader_jsons,
        local_native_dependencies: &local_native_dependencies,
        all_bc_har_deps: &all_bc_har_deps,
        bundle_outs: &mut bundle_outs,
        widget_bundle_outs: &widget_bundle_outs,
        compile_task_active: &compile_task_active,
        module_obfuscation_contexts: &module_obfuscation_contexts,
        pending_bytecode_obfuscations: &mut pending_bytecode_obfuscations,
      })
      .await?;
    if let Some(output) = bytecode_output.early_output {
      return Ok(output);
    }
    let bundle_package_outs = bytecode_output.bundle_package_outs;

    // ── Step 6: Packaging + Signing ───────────────────────────────────
    let mut output_paths = Vec::new();
    if let Some(output) = self
      .run_package_sign_stage(PackageSignStageInput {
        args,
        config,
        session,
        task_lifecycle: &mut task_lifecycle,
        quickfix_version_store: &quickfix_version_store,
        quickfix_patch_version,
        quickfix_version_advanced: &mut quickfix_version_advanced,
        progress: &mut progress,
        intermediate_writer: &intermediate_writer,
        har_dedup_project_plan: &har_dedup_project_plan,
        sanitizer_native_policy: &sanitizer_native_policy,
        loader_jsons: &loader_jsons,
        local_native_dependencies: &local_native_dependencies,
        module_pack_info_plans: &module_pack_info_plans,
        privacy_profile_paths: &privacy_profile_paths,
        module_obfuscation_contexts: &module_obfuscation_contexts,
        bundle_package_outs: &bundle_package_outs,
        output_paths: &mut output_paths,
        start,
      })
      .await?
    {
      return Ok(output);
    }

    // ── Watch mode ────────────────────────────────────────────────────
    if watch_mode {
      WatchStage::run(WatchStageInput::from_build(
        config.clone(),
        session.clone(),
        args,
        project_root.clone(),
        abc_backend,
        coverage_mode,
        disable_abc_cache,
        cli_cache_file.clone(),
        package_plugins.clone(),
        self.task_observer.clone(),
        preview_mode,
        false,
      ))
      .await?;
    }

    Ok(BuildOutput {
      artifact_paths: output_paths,
    })
  }
}
