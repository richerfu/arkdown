//! [`BuildCoordinator`] entry-point and multi-target execution helpers.

use super::*;

impl BuildCoordinator {
  pub(crate) fn arkts_compile_task_name(
    output_target: &OutputTarget,
    module_type: &OhosModuleType,
    target_name: &str,
  ) -> &'static str {
    if matches!(output_target, OutputTarget::Test) {
      "UnitTestArkTS"
    } else if matches!(output_target, OutputTarget::Preview) {
      "PreviewArkTS"
    } else if target_name == "ohosTest" {
      "OhosTestCompileArkTS"
    } else if matches!(module_type, OhosModuleType::Har) {
      "HarCompileArkTS"
    } else {
      "CompileArkTS"
    }
  }

  /// Select the concrete compiler task requested through Hvigor's public task
  /// graph. `BuildArkTS`/`BuildJS` and `CompileJS` remain registered by the
  /// 6.24.3 plugin even though current Stage projects normally reach
  /// `CompileArkTS`. A direct invocation must therefore publish the selected
  /// task's lifecycle boundary instead of silently running `CompileArkTS` and
  /// leaving the requested root unfinished.
  pub(crate) fn selected_arkts_compile_task_name(
    roots: &[NativeTaskRoot],
    module_name: &str,
    target_name: &str,
    output_target: &OutputTarget,
    module_type: &OhosModuleType,
  ) -> &'static str {
    roots
      .iter()
      .find_map(|root| {
        (root.module_name == module_name && root.target_name.as_deref() == Some(target_name))
          .then(|| match root.task_name.as_str() {
            "BuildArkTS" => Some("BuildArkTS"),
            "BuildJS" => Some("BuildJS"),
            "CompileArkTS" => Some("CompileArkTS"),
            "CompileJS" => Some("CompileJS"),
            "HarBuildArkTS" => Some("HarBuildArkTS"),
            "HarCompileArkTS" => Some("HarCompileArkTS"),
            _ => None,
          })
          .flatten()
      })
      .unwrap_or_else(|| Self::arkts_compile_task_name(output_target, module_type, target_name))
  }

  /// hvigor `FastPackageHap` (`packOptions.fastBuildApp`): copy the compiled
  /// intermediates into `intermediates/fastApp/<target>/` — `ets/` assets,
  /// compiled resources + `resources.index`, package context, module.json and
  /// pack.info with the owning bundle name / aligned device types.
  pub(super) fn run_fast_package(
    loader_jsons: &[(
      ModuleConfig,
      HapManifest,
      LoaderJson,
      FxHashMap<String, ResourceReference>,
      String,
      Arc<ArkdownConfig>,
    )],
    enabled_modules: &[bool],
    pack_info_plans: &[Vec<ModulePackInfoPlan>],
  ) -> Result<Vec<PathBuf>> {
    let mut artifact_paths = Vec::new();
    for (
      (module, _manifest, _loader_json, _resource_refs, module_target_name, module_config),
      enabled,
      plans,
    ) in loader_jsons
      .iter()
      .zip(enabled_modules)
      .zip(pack_info_plans)
      .map(|((module, enabled), plans)| (module, enabled, plans))
    {
      if !*enabled {
        continue;
      }
      let module_layout = ModuleBuildLayout::for_output_target_product(
        &module_config.output_target,
        &module.src_path,
        &module_config.product.name,
        module_target_name,
      );
      let fast_app = module_layout
        .intermediates()
        .join("fastApp")
        .join(module_target_name);
      std::fs::create_dir_all(&fast_app)?;

      // ets assets (compiled ABC) → fastApp/ets
      let ets_assets = module_layout.loader_out_dir().join("ets");
      if ets_assets.is_dir() {
        copy_tree(&ets_assets, &fast_app.join("ets"))?;
      }
      // compiled resources → fastApp/resources
      let resource_dir = module_layout.res_resources_dir();
      if resource_dir.is_dir() {
        copy_tree(&resource_dir, &fast_app.join("resources"))?;
      }
      // resources.index → fastApp/resources.index
      let resources_index = module_layout.res_resources_index();
      if resources_index.is_file() {
        std::fs::copy(&resources_index, fast_app.join("resources.index"))?;
      }
      // package context / sdk info (conditional on normalized OHM url)
      let use_normalized = module_config
        .product
        .build_option
        .strict_mode
        .use_normalized_ohm_url;
      let pkg_context = module_layout.pkg_context_info_json();
      if use_normalized && pkg_context.is_file() {
        std::fs::copy(&pkg_context, fast_app.join("pkgContextInfo.json"))?;
      }
      let pkg_sdk_info = module_layout.pkg_sdk_info_json();
      if !use_normalized && pkg_sdk_info.is_file() {
        std::fs::copy(&pkg_sdk_info, fast_app.join("pkgSdkInfo.json"))?;
      }
      // module.json with the owning bundle name. Normal packaging extracts
      // this intermediate from the archive, while FastPackage runs before an
      // archive exists and therefore reads ProcessProfile output directly.
      let package_module_json = module_layout
        .default_intermediate_dir("package")
        .join("module.json");
      let module_json_source = if package_module_json.is_file() {
        package_module_json
      } else {
        module_layout.merge_profile_module_json()
      };
      if module_json_source.is_file() {
        let mut document: serde_json::Value =
          serde_json::from_slice(&std::fs::read(&module_json_source)?)?;
        if let Some(app) = document
          .get_mut("app")
          .and_then(serde_json::Value::as_object_mut)
        {
          app.insert(
            "bundleName".into(),
            module_config.product.bundle_name.clone().into(),
          );
        }
        std::fs::write(fast_app.join("module.json"), serde_json::to_vec(&document)?)?;
      }
      // Stage-model fast APP packaging uses the first related entry exactly
      // like `BasePackHapTask.copyIntermediates` (`allPackInfoPath[0]`).
      let pack_info = plans
        .first()
        .map(|plan| plan.output_path.clone())
        .unwrap_or_else(|| module_layout.output_pack_info());
      if !pack_info.is_file() {
        anyhow::bail!(
          "FastPackage requires MakePackInfo output '{}'",
          pack_info.display()
        );
      }
      std::fs::copy(pack_info, fast_app.join("pack.info"))?;
      artifact_paths.push(fast_app);
    }
    Ok(artifact_paths)
  }

  pub(crate) fn pack_info_input(
    module: &ModuleConfig,
    module_config: &ArkdownConfig,
    session: &BuildSession,
    target_name: &str,
    module_json: &serde_json::Value,
    resources_src_dirs: Vec<PathBuf>,
    device_types: Vec<String>,
  ) -> PackInfoInput {
    let bundle_type = module_config
      .product
      .app_options
      .bundle_type
      .clone()
      .unwrap_or_else(|| session.project.bundle_type.clone());
    PackInfoInput {
      module_name: module.name.clone(),
      package_name: ArtifactOutputName::new(
        module_config.product.artifact_name.as_deref(),
        &module.name,
        target_name,
        "hap",
      )
      .pack_info(),
      module_json: module_json.clone(),
      // MakePackInfo reads form JSON from the source resource model; it runs
      // before CompileResource and must not depend on restool output.
      resources_src_dirs,
      bundle_version_code: module_config
        .product
        .version_code
        .unwrap_or(session.project.version_code),
      bundle_version_name: module_config
        .product
        .version_name
        .clone()
        .unwrap_or_else(|| session.project.version_name.clone()),
      bundle_build_version: module_config
        .product
        .build_version
        .clone()
        .or_else(|| session.project.build_version.clone()),
      min_compatible_version_code: session.project.min_compatible_version_code,
      bundle_type: bundle_type.clone(),
      additional_modules: Vec::new(),
      device_types,
      package_device_types: Vec::new(),
      installation_free: module_json
        .pointer("/module/installationFree")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(bundle_type == "atomicService"),
      delivery_with_install: module_json
        .pointer("/module/deliveryWithInstall")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true),
    }
  }

  pub(crate) fn target_device_types(
    module_name: &str,
    module_device_types: &[String],
    target: &EffectiveTargetConfig,
  ) -> Result<Vec<String>> {
    if target.device_types.is_empty() {
      return Ok(module_device_types.to_vec());
    }
    let invalid = target
      .device_types
      .iter()
      .filter(|device_type| !module_device_types.contains(device_type))
      .cloned()
      .collect::<Vec<_>>();
    if !invalid.is_empty() {
      anyhow::bail!(
        "INVALID_TARGET_DEVICETYPE: target '{}' of module '{}' contains deviceType {:?} not declared by module.json5 {:?}",
        target.name,
        module_name,
        invalid,
        module_device_types
      );
    }
    Ok(target.device_types.clone())
  }

  #[allow(clippy::too_many_arguments)]
  pub(super) fn pack_info_plans(
    config: &ArkdownConfig,
    module: &ModuleConfig,
    module_config: &ArkdownConfig,
    session: &BuildSession,
    args: &BuildRequest,
    target_name: &str,
    target: &EffectiveTargetConfig,
    module_layout: &ModuleBuildLayout<'_>,
  ) -> Result<Vec<ModulePackInfoPlan>> {
    let module_context = session
      .module_context(&module.name)
      .ok_or_else(|| anyhow::anyhow!("missing build session module context for {}", module.name))?;
    let mut declared_device_types = module_context
      .module
      .module_json
      .as_ref()
      .map(|module_json| module_json.module.device_types.clone())
      .unwrap_or_default();
    if declared_device_types.is_empty() {
      declared_device_types.push("phone".to_string());
    }
    let current_device_types =
      Self::target_device_types(&module.name, &declared_device_types, target)?;
    let current_module_json = if target_name == "ohosTest" {
      module_context
        .module
        .ohos_test_module_json_value
        .as_deref()
        .or(module_context.module.module_json_value.as_deref())
    } else {
      module_context.module.module_json_value.as_deref()
    }
    .cloned()
    .ok_or_else(|| anyhow::anyhow!("missing module.json5 snapshot for {}", module.name))?;
    let form_resources = PackInfoFormResourceResolver::new(session.project.as_ref());
    let resources_src_dirs =
      form_resources.resolve(&module.name, target_name, &current_module_json)?;
    let mut base = Self::pack_info_input(
      module,
      module_config,
      session,
      target_name,
      &current_module_json,
      resources_src_dirs,
      current_device_types.clone(),
    );
    if target_name == "ohosTest" {
      let default_module_json = module_context
        .module
        .module_json_value
        .as_deref()
        .cloned()
        .ok_or_else(|| {
          anyhow::anyhow!("missing default module.json5 snapshot for {}", module.name)
        })?;
      base.additional_modules.push(PackInfoModuleInput {
        module_name: module.name.clone(),
        resources_src_dirs: form_resources.resolve(
          &module.name,
          "default",
          &default_module_json,
        )?,
        module_json: default_module_json,
        device_types: current_device_types.clone(),
      });
    }

    let related_entries = if matches!(module.module_type, OhosModuleType::Feature) {
      module_context
        .module
        .related_entry_modules
        .as_deref()
        .unwrap_or_default()
    } else {
      &[]
    };
    if related_entries.is_empty() {
      return Ok(vec![ModulePackInfoPlan {
        input: base,
        output_path: module_layout.output_pack_info(),
        related_entry: None,
      }]);
    }

    let mut plans = Vec::with_capacity(related_entries.len());
    for entry_name in related_entries {
      let entry_module = config
        .modules
        .iter()
        .find(|candidate| candidate.name == *entry_name)
        .ok_or_else(|| {
          anyhow::anyhow!(
            "RELATED_ENTRY_MODULE_NOT_FOUND: feature '{}' references missing entry module '{}'",
            module.name,
            entry_name
          )
        })?;
      if !matches!(entry_module.module_type, OhosModuleType::Entry) {
        anyhow::bail!(
          "INVALID_RELATED_ENTRY_MODULE_TYPE: feature '{}' references '{}' with module type '{}'",
          module.name,
          entry_name,
          entry_module.module_type
        );
      }
      let entry_context = session.module_context(entry_name).ok_or_else(|| {
        anyhow::anyhow!("missing build session module context for related entry '{entry_name}'")
      })?;
      let entry_build = ModuleBuildConfiguration::resolve_dependency(
        config,
        session.build_options.as_ref(),
        entry_module,
        Some(target_name),
        &args.hvigor_properties,
      )?;
      let mut entry_declared_device_types = entry_context
        .module
        .module_json
        .as_ref()
        .map(|module_json| module_json.module.device_types.clone())
        .unwrap_or_default();
      if entry_declared_device_types.is_empty() {
        entry_declared_device_types.push("phone".to_string());
      }
      let entry_device_types = Self::target_device_types(
        entry_name,
        &entry_declared_device_types,
        &entry_build.target,
      )?;
      let current_device_type_set = current_device_types.iter().collect::<FxHashSet<_>>();
      let shared_device_types = entry_device_types
        .iter()
        .filter(|device_type| current_device_type_set.contains(device_type))
        .cloned()
        .collect::<Vec<_>>();
      if shared_device_types.is_empty() {
        tracing::warn!(
          feature = %module.name,
          entry = %entry_name,
          "The device types of the feature and related entry are different; Hvigor emits an invalid HAP with an empty related-entry deviceType"
        );
      }
      let entry_source_value = if entry_build.target.name == "ohosTest" {
        entry_context
          .module
          .ohos_test_module_json_value
          .as_deref()
          .or(entry_context.module.module_json_value.as_deref())
      } else {
        entry_context.module.module_json_value.as_deref()
      }
      .cloned()
      .ok_or_else(|| {
        anyhow::anyhow!("missing module.json5 snapshot for related entry '{entry_name}'")
      })?;
      let mut input = base.clone();
      let related_archive_module = format!("{}-{}", module.name, entry_name);
      input.package_name = ArtifactOutputName::new(
        module_config.product.artifact_name.as_deref(),
        &related_archive_module,
        target_name,
        "hap",
      )
      .pack_info();
      input.additional_modules = vec![PackInfoModuleInput {
        module_name: entry_name.clone(),
        resources_src_dirs: form_resources.resolve(
          entry_name,
          &entry_build.target.name,
          &entry_source_value,
        )?,
        module_json: entry_source_value,
        device_types: shared_device_types.clone(),
      }];
      if target_name == "ohosTest" {
        let default_module_json = module_context
          .module
          .module_json_value
          .as_deref()
          .cloned()
          .ok_or_else(|| {
            anyhow::anyhow!("missing default module.json5 snapshot for {}", module.name)
          })?;
        let default_entry_json = entry_context
          .module
          .module_json_value
          .as_deref()
          .cloned()
          .ok_or_else(|| {
            anyhow::anyhow!(
              "missing default module.json5 snapshot for related entry '{entry_name}'"
            )
          })?;
        input.additional_modules.extend([
          PackInfoModuleInput {
            module_name: module.name.clone(),
            resources_src_dirs: form_resources.resolve(
              &module.name,
              "default",
              &default_module_json,
            )?,
            module_json: default_module_json,
            device_types: current_device_types.clone(),
          },
          PackInfoModuleInput {
            module_name: entry_name.clone(),
            resources_src_dirs: form_resources.resolve(
              entry_name,
              "default",
              &default_entry_json,
            )?,
            module_json: default_entry_json,
            device_types: shared_device_types,
          },
        ]);
      }
      input.package_device_types = entry_device_types;
      plans.push(ModulePackInfoPlan {
        input,
        output_path: module_layout
          .target_output_dir()
          .join(entry_name)
          .join("pack.info"),
        related_entry: Some(entry_name.clone()),
      });
    }
    Ok(plans)
  }
  /// `isFormWidgetModule`: an HSP whose `module.json5` declares
  /// `formExtensionModule` (hvigor `module-model-impl.ts`).
  pub(crate) fn module_json5_has_form_extension_module(module_root: &Path) -> bool {
    let module_json_path = module_root.join("src/main/module.json5");
    std::fs::read_to_string(&module_json_path)
      .ok()
      .and_then(|content| json5::from_str::<arkdown_core::ModuleJson5>(&content).ok())
      .is_some_and(|json| json.module.form_extension_module.is_some())
  }

  /// Construct a new coordinator from a completed build session and typed request.
  pub fn new(session: Arc<BuildSession>, config: Arc<ArkdownConfig>, args: BuildRequest) -> Self {
    Self {
      session,
      config,
      args,
      package_plugins: vec![],
      task_observer: None,
      owns_task_cycle: true,
    }
  }

  /// Attach package-supplied Rolldown plugins collected by the Node.js API.
  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  /// Attach the same-process task observer used by the Node hvigor runtime.
  pub fn with_task_observer(mut self, task_observer: Arc<dyn BuildTaskObserver>) -> Self {
    self.task_observer = Some(task_observer);
    self
  }

  /// Execute inside a cycle owned by a project-level coordinator.
  ///
  /// `assembleApp` builds HSP/HAP children and then runs project tasks as one
  /// hvigor graph. Suppressing child cycle markers keeps TypeScript custom
  /// task completion and incremental snapshots alive across those stages.
  pub(crate) fn within_task_cycle(mut self) -> Self {
    self.owns_task_cycle = false;
    self
  }

  /// Execute the full build pipeline and return the produced artifacts.
  ///
  pub async fn execute(self) -> Result<BuildOutput> {
    let observer = self.task_observer.clone();
    let owns_task_cycle = self.owns_task_cycle;
    match self.execute_targets().await {
      Ok(output) => Ok(output),
      Err(error) if owns_task_cycle && error.is::<NativeTaskCutoffReached>() => {
        BuildTaskLifecycle::new(observer, Vec::new())
          .finish_cycle()
          .await?;
        Ok(BuildOutput {
          artifact_paths: Vec::new(),
        })
      }
      Err(error) => Err(error),
    }
  }

  async fn execute_targets(self) -> Result<BuildOutput> {
    let target_plan = TargetBuildExecutionPlan::new(
      self.config.as_ref(),
      self.session.build_options.as_ref(),
      &self.args,
    );
    if target_plan.target_names.len() <= 1 || self.args.build_target_name.is_some() {
      return self.execute_impl().await;
    }

    tracing::info!(
      targets = ?target_plan.target_names,
      "multi-target: executing the complete build pipeline once per target"
    );
    let cycle_lifecycle = BuildTaskLifecycle::new(
      self.task_observer.clone(),
      self.args.native_task_roots.clone(),
    );
    if self.owns_task_cycle {
      cycle_lifecycle.begin_cycle().await?;
    }
    let mut artifact_paths = Vec::new();
    for target_name in target_plan.target_names {
      let mut args = self.args.clone();
      args.build_target_name = Some(target_name.clone());
      tracing::info!(target = %target_name, "multi-target pipeline started");
      let output = Self {
        session: self.session.clone(),
        config: self.config.clone(),
        args,
        package_plugins: self.package_plugins.clone(),
        task_observer: self.task_observer.clone(),
        owns_task_cycle: false,
      }
      .execute_impl()
      .await?;
      artifact_paths.extend(output.artifact_paths);
    }
    if self.owns_task_cycle {
      cycle_lifecycle.finish_cycle().await?;
    }
    Ok(BuildOutput { artifact_paths })
  }
}
