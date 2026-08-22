//! Bytecode (ABC) compilation stage implementation.

use super::*;

pub(crate) struct CoordinatorBytecodeStageInput<'a> {
  pub(crate) args: &'a BuildArgs,
  pub(crate) config: &'a Arc<ArkdownConfig>,
  pub(crate) session: &'a Arc<BuildSession>,
  pub(crate) package_plugins: &'a [PackagePlugin],
  pub(crate) task_lifecycle: &'a mut BuildTaskLifecycle,
  pub(crate) hot_reload_mode: bool,
  pub(crate) coverage_mode: bool,
  pub(crate) disable_abc_cache: bool,
  pub(crate) cli_cache_file: &'a Option<PathBuf>,
  pub(crate) abc_backend: AbcBackendChoice,
  pub(crate) module_analyzer: ModuleAnalyzerKind,
  pub(crate) project_root: &'a Path,
  pub(crate) progress: &'a mut Progress,
  pub(crate) intermediate_writer: &'a IntermediateArtifactWriter,
  pub(crate) har_dedup_project_plan: &'a HarDedupProjectPlan,
  pub(crate) loader_jsons: &'a [(
    ModuleConfig,
    HapManifest,
    LoaderJson,
    FxHashMap<String, ResourceReference>,
    String,
    Arc<ArkdownConfig>,
  )],
  pub(crate) local_native_dependencies: &'a BTreeMap<String, Arc<[NativeCmakeLocalDependency]>>,
  pub(crate) all_bc_har_deps: &'a [Vec<ByteCodeHarScanEntry>],
  pub(crate) bundle_outs: &'a mut Vec<(BundleStageOutput,)>,
  pub(crate) widget_bundle_outs: &'a [Option<WidgetCompileArktsOutput>],
  pub(crate) compile_task_active: &'a [bool],
  pub(crate) module_obfuscation_contexts: &'a [ModuleObfuscationContext],
  pub(crate) pending_bytecode_obfuscations: &'a mut [Option<PendingBytecodeObfuscation>],
}

pub(crate) struct CoordinatorBytecodeStageOutput {
  pub(crate) early_output: Option<BuildOutput>,
  pub(crate) bundle_package_outs: Vec<(BundlePackageOutput,)>,
}

impl BuildCoordinator {
  pub(crate) async fn run_bytecode_stage(
    &self,
    input: CoordinatorBytecodeStageInput<'_>,
  ) -> Result<CoordinatorBytecodeStageOutput> {
    let CoordinatorBytecodeStageInput {
      args,
      config,
      session,
      package_plugins,
      task_lifecycle,
      hot_reload_mode,
      coverage_mode,
      disable_abc_cache,
      cli_cache_file,
      abc_backend,
      module_analyzer,
      project_root,
      progress,
      intermediate_writer,
      har_dedup_project_plan,
      loader_jsons,
      local_native_dependencies,
      all_bc_har_deps,
      bundle_outs,
      widget_bundle_outs,
      compile_task_active,
      module_obfuscation_contexts,
      pending_bytecode_obfuscations,
    } = input;

    // ── Step 5: ABC compilation ───────────────────────────────────────
    progress.step(5, 6, "Compiling ABC");
    for (
      bundle_index,
      (
        (
          (module, manifest, loader_json, _resource_refs, module_target_name, module_config),
          (bundle_out,),
        ),
        bc_har_deps,
      ),
    ) in loader_jsons
      .iter()
      .zip(bundle_outs.iter())
      .zip(all_bc_har_deps.iter())
      .enumerate()
    {
      let quickfix_task_plan = QuickFixTaskPlan::for_module(&args.native_task_roots, &module.name);
      let hot_reload_task_plan =
        HotReloadTaskPlan::for_module(&args.native_task_roots, &module.name);
      let compile_task_name = if hot_reload_task_plan.is_some() {
        "HotReloadArkTS"
      } else if quickfix_task_plan.is_some() {
        "ColdReloadArkTS"
      } else {
        Self::selected_arkts_compile_task_name(
          &args.native_task_roots,
          &module.name,
          module_target_name,
          &config.output_target,
          &module.module_type,
        )
      };
      let module_layout = ModuleBuildLayout::for_output_target_product(
        &config.output_target,
        &module.src_path,
        &config.product.name,
        module_target_name,
      );
      let module_package_plugins = package_plugins
        .iter()
        .filter(|plugin| plugin.applies_to_module_target(&module.src_path, module_target_name))
        .collect::<Vec<_>>();
      let module_har_dedup_plan = har_dedup_project_plan.module(&module.name);
      let module_obfuscation = &module_obfuscation_contexts[bundle_index];
      if bundle_out.bypasses_compilation() {
        continue;
      }
      let compile_target = HvigorCompileTarget::from_loader_json(loader_json, module_config);
      let effective_cache_file = if disable_abc_cache {
        None
      } else {
        cli_cache_file
          .clone()
          .or_else(|| Some(PathBuf::from(&loader_json.cache_path).join("modules.cache")))
      };

      let abc_ts2abc_backend_inner = match abc_backend {
        AbcBackendChoice::Auto => RolldownBytecodeBackendChoice::Auto,
        AbcBackendChoice::Sdk => RolldownBytecodeBackendChoice::Sdk,
      };
      let bytecode_cache_dir = PathBuf::from(&loader_json.cache_path);
      let bytecode_project_root = PathBuf::from(&loader_json.project_root_path);
      let abc_input_files = bc_har_deps
        .iter()
        .map(|dep| {
          BytecodeAbcInput::from_project_cache(
            dep.abc_path.clone(),
            dep.package_name.clone(),
            &bytecode_project_root,
            &bytecode_cache_dir,
          )
          .map_err(|error| anyhow::anyhow!(error))
        })
        .collect::<Result<Vec<_>>>()?;
      let hot_reload_compile_plan = if hot_reload_task_plan.is_some() {
        let hot_reload = HotReloadProcessor::new(&module_layout, &project_root);
        hot_reload.compile_plan(
          loader_json.patch_config.as_ref(),
          &loader_json.compile_mode,
          config.sdk_config.compile_api_version,
          args.watch_invalidation.is_some(),
          &bundle_out.bundle_output.module_source_files,
          &bundle_out.bundle_output.modules_abc_path,
          bundle_out.bundle_output.source_maps_path.as_deref(),
        )?
      } else {
        None
      };
      let compile_context_info_path = if loader_json.use_normalized_ohm_url
        && (hot_reload_task_plan.is_none() || hot_reload_compile_plan.is_some())
      {
        let module_context = session
          .module_context(&module.name)
          .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
        let pkg_context_input =
          PkgContextInfoInputBuilder::new(module_config.as_ref(), session, module, &module_context)
            .with_local_native_dependencies(
              local_native_dependencies
                .get(&module.name)
                .cloned()
                .unwrap_or_else(|| Arc::from([])),
            )
            .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
            .build()?;
        let pkg_context_info = PkgContextInfoGenerator::new()
          .generate(&pkg_context_input)
          .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
        let path = PathBuf::from(&loader_json.cache_path).join("compileContextInfo.json");
        CompileContextInfoWriter::new(
          loader_json,
          &bundle_out.bundle_output.module_source_files,
          &pkg_context_info,
          &session.project.bundle_type,
          &config.product.bundle_name,
          matches!(module.module_type, OhosModuleType::Shared)
            && Self::module_json5_has_form_extension_module(&module.src_path),
        )
        .with_compile_entry_override(
          hot_reload_compile_plan
            .as_ref()
            .and_then(|plan| plan.compile_entry_override.as_deref()),
        )
        .write(&path)
        .map_err(|e| anyhow::anyhow!("GenerateCompileContextInfo failed: {}", e))?;
        Some(path)
      } else {
        None
      };
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:before-bytecode-stage",
        format_args!(
          "module={} chunks={} module_sources={} external_requests={} npm_entries={}",
          module.name,
          bundle_out.bundle_output.chunk_files.len(),
          bundle_out.bundle_output.module_source_files.len(),
          bundle_out.bundle_output.external_module_requests.len(),
          bundle_out.bundle_output.npm_entries.len()
        ),
      );
      // `module_mode.js` es2abc command construction: `--branch-elimination`
      // only for non-debuggable builds without bytecode obfuscation, and
      // `--transform-lib` when `arkOptions.transformLib` is configured.
      let ark_options = &module_config.product.build_option.ark_options;
      let branch_elimination = config.build_mode.is_release()
        && pending_bytecode_obfuscations[bundle_index].is_none()
        && ark_options
          .as_ref()
          .and_then(|options| options.branch_elimination)
          .unwrap_or(false);
      let transform_lib = ark_options
        .as_ref()
        .and_then(|options| options.transform_lib.clone());
      let cold_reload_plan = if quickfix_task_plan.is_some() {
        ColdReloadProcessor::new(&module_layout).compile_plan(
          loader_json.patch_config.as_ref(),
          &bundle_out.bundle_output.module_source_files,
          bundle_out.bundle_output.source_maps_path.as_deref(),
        )?
      } else {
        None
      };
      let bytecode_result = if let Some(reload_plan) = hot_reload_compile_plan.as_ref() {
        let initial = reload_plan.phase == HotReloadCompilePhase::Initial;
        BytecodeStage::run(BytecodeStageInput {
          compiler: RolldownBytecodeCompiler::new(
            abc_ts2abc_backend_inner,
            config.sdk_config.es2abc_path.clone(),
            Arc::clone(&session.sdk.es2abc_options),
            compile_target.api_version(),
            config.build_mode.is_debuggable(),
          )
          .with_target_api_sub_version(compile_target.api_sub_version())
          .with_branch_elimination(false)
          .with_transform_lib(transform_lib.clone())
          .with_reload(Some(reload_plan.reload.clone())),
          module_source_files: reload_plan.module_source_files.clone(),
          external_module_requests: bundle_out.bundle_output.external_module_requests.clone(),
          npm_entries: bundle_out.bundle_output.npm_entries.clone(),
          modules_abc_path: reload_plan.modules_abc_path.clone(),
          cache_dir: bytecode_cache_dir.clone(),
          cache_file: effective_cache_file.clone(),
          abc_input_files: if initial {
            abc_input_files.clone()
          } else {
            Vec::new()
          },
          compile_context_info_path: compile_context_info_path.clone(),
          byte_code_har: initial && loader_json.byte_code_har.unwrap_or(false),
        })
      } else if hot_reload_task_plan.is_some() {
        Ok(())
      } else if let Some(reload_plan) = cold_reload_plan.as_ref() {
        BytecodeStage::run(BytecodeStageInput {
          compiler: RolldownBytecodeCompiler::new(
            abc_ts2abc_backend_inner,
            config.sdk_config.es2abc_path.clone(),
            Arc::clone(&session.sdk.es2abc_options),
            compile_target.api_version(),
            config.build_mode.is_debuggable(),
          )
          .with_target_api_sub_version(compile_target.api_sub_version())
          .with_branch_elimination(false)
          .with_transform_lib(transform_lib.clone())
          .with_reload(Some(reload_plan.reload.clone())),
          module_source_files: reload_plan.module_source_files.clone(),
          external_module_requests: bundle_out.bundle_output.external_module_requests.clone(),
          npm_entries: bundle_out.bundle_output.npm_entries.clone(),
          modules_abc_path: reload_plan.modules_abc_path.clone(),
          cache_dir: bytecode_cache_dir.clone(),
          cache_file: effective_cache_file.clone(),
          abc_input_files: Vec::new(),
          compile_context_info_path: compile_context_info_path.clone(),
          byte_code_har: false,
        })
      } else if quickfix_task_plan.is_some() {
        Ok(())
      } else {
        BytecodeStage::run(BytecodeStageInput {
          compiler: RolldownBytecodeCompiler::new(
            abc_ts2abc_backend_inner,
            config.sdk_config.es2abc_path.clone(),
            Arc::clone(&session.sdk.es2abc_options),
            compile_target.api_version(),
            config.build_mode.is_debuggable(),
          )
          .with_target_api_sub_version(compile_target.api_sub_version())
          .with_branch_elimination(branch_elimination)
          .with_transform_lib(transform_lib.clone()),
          module_source_files: bundle_out.bundle_output.module_source_files.clone(),
          external_module_requests: bundle_out.bundle_output.external_module_requests.clone(),
          npm_entries: bundle_out.bundle_output.npm_entries.clone(),
          modules_abc_path: bundle_out.bundle_output.modules_abc_path.clone(),
          cache_dir: bytecode_cache_dir,
          cache_file: effective_cache_file,
          abc_input_files: abc_input_files.clone(),
          compile_context_info_path,
          byte_code_har: loader_json.byte_code_har.unwrap_or(false),
        })
      };
      // On ABC compile failure, remove the task-inputs cache so the next build
      // is forced to recompile (mirrors hvigor's removeCompileFailedFilesFromCache).
      if bytecode_result.is_err() {
        let cache_dir_path = PathBuf::from(&loader_json.cache_path);
        let task_inputs_file = cache_dir_path.join(".task_inputs");
        if task_inputs_file.exists() {
          tracing::debug!(
            "removing failed-build cache: {} (removeCompileFailedFilesFromCache)",
            task_inputs_file.display()
          );
          let _ = std::fs::remove_file(&task_inputs_file);
        }
      }
      #[cfg(feature = "memory-diagnostics")]
      arkdown::memory_diagnostics::dump_with(
        "build:after-bytecode-stage",
        format_args!("module={} ok={}", module.name, bytecode_result.is_ok()),
      );
      bytecode_result?;

      if quickfix_task_plan.is_none() && hot_reload_task_plan.is_none() {
        AotArtifactWriter::new(
          &module.name,
          loader_json.an_build_mode.as_deref(),
          loader_json.an_build_out_put.as_deref(),
          loader_json.ap_path.as_deref(),
          &module_layout,
          coverage_mode,
        )
        .generate(&bundle_out.bundle_output.modules_abc_path)?;

        task_lifecycle
          .transform_abc(
            &module.name,
            module_target_name,
            &bundle_out.bundle_output.modules_abc_path,
            module_obfuscation.is_enabled(&config.build_mode),
          )
          .await?;

        if let Some(pending) = pending_bytecode_obfuscations[bundle_index].take() {
          pending.run(&module.name)?;
        }
      }

      if hot_reload_task_plan.is_some() {
        // HotReloadArkTS owns a persistent SDK watch worker. Its lifecycle
        // remains open across incremental callbacks and closes only when the
        // watcher terminates, so do not run normal post-ABC/AOT/package work.
        continue;
      }

      if quickfix_task_plan.is_some() {
        if compile_task_active[bundle_index] {
          task_lifecycle
            .after(&module.name, Some(module_target_name), compile_task_name)
            .await?;
        }
        task_lifecycle
          .up_to_date(&module.name, Some(module_target_name), "BuildJS")
          .await?;
        continue;
      }

      // ── P12: Widget compile pass ──────────────────────────────────
      if let Some(widget_out) = widget_bundle_outs
        .get(bundle_index)
        .and_then(Option::as_ref)
      {
        let widget_abc_input_files = bc_har_deps
          .iter()
          .map(|dep| {
            BytecodeAbcInput::from_project_cache(
              dep.abc_path.clone(),
              dep.package_name.clone(),
              &bytecode_project_root,
              &widget_out.plan.cache_dir,
            )
            .map_err(|error| anyhow::anyhow!(error))
          })
          .collect::<Result<Vec<_>>>()?;
        let widget_loader_json =
          WidgetCompileArktsPass::new(module_config.as_ref(), module, manifest, loader_json)
            .compile_context_loader_json(&widget_out.plan)?;
        // 6.24.3 `GenerateLoaderJson.doTaskAction()` writes the widget loader
        // JSON alongside the common one (`aceWidgetLoaderJson`) whenever the
        // module submits ArkTS widgets.
        let widget_loader_path =
          ModuleBuildLayout::for_product(&module.src_path, &module_config.product.name)
            .widget_loader_json();
        let widget_loader_value = serde_json::to_value(&widget_loader_json)
          .map_err(|error| anyhow::anyhow!("serialize widget_loader.json failed: {}", error))?;
        intermediate_writer
          .write_json_pretty(&widget_loader_path, &widget_loader_value)
          .map_err(|error| anyhow::anyhow!("write widget_loader.json failed: {}", error))?;
        let widget_compile_context_info_path = if loader_json.use_normalized_ohm_url {
          let module_context = session
            .module_context(&module.name)
            .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
          let pkg_context_input = PkgContextInfoInputBuilder::new(
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
          .excluding_package_roots(module_har_dedup_plan.duplicate_package_roots())
          .build()?;
          let pkg_context_info = PkgContextInfoGenerator::new()
            .generate(&pkg_context_input)
            .map_err(|e| anyhow::anyhow!("GeneratePkgContextInfo failed: {}", e))?;
          let path = widget_out.plan.cache_dir.join("compileContextInfo.json");
          CompileContextInfoWriter::new(
            &widget_loader_json,
            &widget_out.bundle_out.bundle_output.module_source_files,
            &pkg_context_info,
            &session.project.bundle_type,
            &config.product.bundle_name,
            true,
          )
          .write(&path)
          .map_err(|e| anyhow::anyhow!("GenerateWidgetCompileContextInfo failed: {}", e))?;
          Some(path)
        } else {
          None
        };
        BytecodeStage::run(BytecodeStageInput {
          compiler: RolldownBytecodeCompiler::new(
            abc_ts2abc_backend_inner,
            config.sdk_config.es2abc_path.clone(),
            Arc::clone(&session.sdk.es2abc_options),
            compile_target.api_version(),
            config.build_mode.is_debuggable(),
          )
          .with_target_api_sub_version(compile_target.api_sub_version())
          .with_branch_elimination(branch_elimination)
          .with_transform_lib(transform_lib.clone()),
          module_source_files: widget_out
            .bundle_out
            .bundle_output
            .module_source_files
            .clone(),
          external_module_requests: widget_out
            .bundle_out
            .bundle_output
            .external_module_requests
            .clone(),
          npm_entries: widget_out.bundle_out.bundle_output.npm_entries.clone(),
          modules_abc_path: widget_out.plan.modules_abc_path.clone(),
          cache_dir: widget_out.plan.cache_dir.clone(),
          cache_file: Some(widget_out.plan.cache_file.clone()),
          abc_input_files: widget_abc_input_files,
          compile_context_info_path: widget_compile_context_info_path,
          byte_code_har: widget_loader_json.byte_code_har.unwrap_or(false),
        })?;
      }

      if bundle_out.bundle_output.source_maps_path.is_some() {
        let compile_source_maps_cache =
          PathBuf::from(&loader_json.cache_path).join("sourceMaps.json");
        intermediate_writer.commit_source_map_input_fingerprints(
          &compile_source_maps_cache,
          &bundle_out.bundle_output.source_map_input_fingerprints,
        )?;
      }

      for (_chunk_name, path) in &bundle_out.bundle_output.chunk_files {
        if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
          match fs_err::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
              return Err(err).with_context(|| {
                format!("failed to remove Rolldown temp chunk {}", path.display())
              });
            }
          }
        }
      }

      let cache_dir = PathBuf::from(&loader_json.cache_path);
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
      let task_compile_env = CompileEnvKeyBuilder::new(
        module_config.as_ref(),
        module,
        loader_json,
        module_obfuscation.is_enabled(&config.build_mode),
        module_target_name,
        args.hvigor_properties.rollup_cache_use_source_hash,
      )
      .build()?;
      let task_inputs =
        TaskInputs::compute(module_config.as_ref(), loader_json, &task_input_snapshot)
          .with_module_analyzer(module_analyzer)
          .with_effective_obfuscation(module_obfuscation.config.as_ref(), &config.build_mode)
          .with_compile_env(task_compile_env)
          .with_plugin_hash(task_plugin_hash)
          .with_additional_obfuscation_rule_files(&module_obfuscation.dependency_rule_files)
          .with_custom_type_inputs(
            &loader_json.compiler_types,
            &loader_json.compiler_type_declaration_inputs,
          )?;
      if let Err(e) = task_inputs.save(&cache_dir) {
        tracing::warn!(error = %e, "failed to save task inputs cache");
      }

      let build_mode_str = config.build_mode.as_str();
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

      // ByteCode HAR validation (SPEC-041).
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
          tracing::debug!(
              package = %dep.package_name,
              abc_path = %dep.abc_path.display(),
              "ByteCode HAR dep exists; loaded via byteCodeHarInfo for SDK es2abc (SPEC-041)"
          );
        }
      }

      if matches!(config.output_target, OutputTarget::Test) {
        let artifacts = crate::cmd::build::unit_test::UnitTestArtifactWriter::new(
          &module.src_path,
          &module_config.product.name,
        );
        artifacts.stage_compiled_assets(
          &bundle_out.bundle_output.modules_abc_path,
          bundle_out.bundle_output.source_maps_path.as_deref(),
        )?;
        if matches!(module.module_type, OhosModuleType::Shared) {
          artifacts.write_hsp_build_config()?;
        }
      }

      if compile_task_active[bundle_index] {
        task_lifecycle
          .after(&module.name, Some(module_target_name), compile_task_name)
          .await?;
      }
      if matches!(config.output_target, OutputTarget::Test)
        && task_lifecycle.has_root(&module.name, None, "UnitTestBuild")
      {
        task_lifecycle
          .marker(&module.name, None, "UnitTestBuild")
          .await?;
      }
      if matches!(config.output_target, OutputTarget::Preview) {
        task_lifecycle
          .marker(&module.name, None, "PreviewBuild")
          .await?;
      }
      task_lifecycle
        .up_to_date(&module.name, Some(module_target_name), "BuildJS")
        .await?;
    }
    progress.done();

    if hot_reload_mode {
      if args.watch_invalidation.is_some() {
        // Incremental callbacks execute inside the lifetime of the initial
        // HotReloadArkTS task and therefore never emit a second task cycle or
        // enter packaging.
        if self.owns_task_cycle {
          task_lifecycle.finish_cycle().await?;
        }
        return Ok(CoordinatorBytecodeStageOutput {
          early_output: Some(BuildOutput {
            artifact_paths: Vec::new(),
          }),
          bundle_package_outs: Vec::new(),
        });
      }

      WatchStage::run(WatchStageInput::from_build(
        config.clone(),
        session.clone(),
        args,
        project_root.to_path_buf(),
        abc_backend,
        coverage_mode,
        disable_abc_cache,
        cli_cache_file.as_ref().cloned(),
        package_plugins.to_vec(),
        None,
        false,
        true,
      ))
      .await?;

      for (
        bundle_index,
        (module, _manifest, _loader_json, _resource_refs, module_target_name, _module_config),
      ) in loader_jsons.iter().enumerate()
      {
        let Some(plan) = HotReloadTaskPlan::for_module(&args.native_task_roots, &module.name)
        else {
          continue;
        };
        if compile_task_active[bundle_index] {
          task_lifecycle
            .after(&module.name, Some(module_target_name), "HotReloadArkTS")
            .await?;
        }
        if plan.boundary() == HotReloadTaskBoundary::Build {
          task_lifecycle
            .marker(&module.name, None, "HotReloadBuild")
            .await?;
        }
      }
      if self.owns_task_cycle {
        task_lifecycle.finish_cycle().await?;
      }
      return Ok(CoordinatorBytecodeStageOutput {
        early_output: Some(BuildOutput {
          artifact_paths: Vec::new(),
        }),
        bundle_package_outs: Vec::new(),
      });
    }

    if matches!(config.output_target, OutputTarget::Preview) {
      return Ok(CoordinatorBytecodeStageOutput {
        early_output: Some(BuildOutput {
          artifact_paths: Vec::new(),
        }),
        bundle_package_outs: Vec::new(),
      });
    }
    let bundle_package_outs: Vec<(BundlePackageOutput,)> = std::mem::take(bundle_outs)
      .into_iter()
      .map(|(bundle_out,)| (bundle_out.into_package_output(),))
      .collect();
    Ok(CoordinatorBytecodeStageOutput {
      early_output: None,
      bundle_package_outs,
    })
  }
}
