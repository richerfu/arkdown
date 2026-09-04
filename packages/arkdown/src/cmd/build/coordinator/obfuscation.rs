//! Obfuscation planning helpers for the build coordinator.

use super::*;

pub(super) struct ModuleObfuscationContext {
  pub(super) config: Option<ObfuscationConfig>,
  pub(super) dependency_rule_files: Vec<PathBuf>,
  pub(super) output_target: OutputTarget,
  pub(super) har_artifact_type: HarArtifactType,
  pub(super) target_name: Option<String>,
}

pub(super) struct ModuleArkguardPlanBuilder<'a> {
  pub(super) config: &'a ArkdownConfig,
  pub(super) session: &'a BuildSession,
  pub(super) module: &'a ModuleConfig,
  pub(super) loader_json: &'a LoaderJson,
  pub(super) bundle_out: &'a BundleStageOutput,
  pub(super) node_executable: PathBuf,
}

impl<'a> ModuleArkguardPlanBuilder<'a> {
  pub(super) fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    loader_json: &'a LoaderJson,
    bundle_out: &'a BundleStageOutput,
    node_executable: PathBuf,
  ) -> Self {
    Self {
      config,
      session,
      module,
      loader_json,
      bundle_out,
      node_executable,
    }
  }

  pub(super) fn build(&self) -> Result<(ArkguardAdapterOptions, ObfuscationModuleInput)> {
    let source_maps_path = self
      .bundle_out
      .bundle_output
      .source_maps_path
      .clone()
      .ok_or_else(|| {
        anyhow::anyhow!(
          "ArkGuard requires the pre-obfuscation sourceMaps.map for module '{}'",
          self.module.name
        )
      })?;
    let source_maps: serde_json::Map<String, serde_json::Value> =
      serde_json::from_slice(&std::fs::read(&source_maps_path)?)?;
    let insight_intent_keep = self
      .bundle_out
      .bundle_output
      .insight_intent_sources
      .iter()
      .map(|source| (source.module_id.as_str(), &source.metadata))
      .collect::<FxHashMap<_, _>>();
    let files = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .map(|source| {
        let previous_source_map =
          source_maps
            .get(&source.source_file)
            .cloned()
            .ok_or_else(|| {
              anyhow::anyhow!(
                "sourceMaps.map has no entry '{}' for ArkGuard input {}",
                source.source_file,
                source.path.display()
              )
            })?;
        let relative_path = previous_source_map
          .get("sources")
          .and_then(serde_json::Value::as_array)
          .and_then(|sources| sources.first())
          .and_then(serde_json::Value::as_str)
          .filter(|source| !source.is_empty())
          .ok_or_else(|| {
            anyhow::anyhow!(
              "source map '{}' has no original source path",
              source.source_file
            )
          })?
          .to_string();
        let original_path = PathBuf::from(&source.module_id);
        if !original_path.is_file() {
          anyhow::bail!(
            "ArkGuard original source does not exist for '{}': {}",
            source.source_file,
            original_path.display()
          );
        }
        let insight_intent = insight_intent_keep.get(source.module_id.as_str());
        Ok(ObfuscationModuleFile {
          build_path: source.path.clone(),
          original_path,
          relative_path,
          source_map_key: source.source_file.clone(),
          previous_source_map: Some(previous_source_map),
          insight_intent_global_names: insight_intent
            .map(|metadata| metadata.obfuscation_global_names.clone())
            .unwrap_or_default(),
          insight_intent_property_names: insight_intent
            .map(|metadata| metadata.obfuscation_property_names.clone())
            .unwrap_or_default(),
        })
      })
      .collect::<Result<Vec<_>>>()?;
    if files.is_empty() {
      anyhow::bail!(
        "ArkGuard source obfuscation has no CompileArkTS module sources for '{}'",
        self.module.name
      );
    }
    let module_context = self
      .session
      .module_context(&self.module.name)
      .ok_or_else(|| {
        anyhow::anyhow!(
          "missing build session module context for {}",
          self.module.name
        )
      })?;
    let package = module_context.module.package.as_ref().ok_or_else(|| {
      anyhow::anyhow!(
        "ArkGuard requires oh-package.json5 metadata for module '{}'",
        self.module.name
      )
    })?;
    let module_path_map = self
      .loader_json
      .module_path_map
      .iter()
      .map(|(name, path)| (name.clone(), PathBuf::from(path)))
      .collect();
    let adapter = ArkguardAdapterOptions {
      node_executable: self.node_executable.clone(),
      ets_loader_path: self.config.sdk_config.ets_loader_path.clone(),
      arkguard_package: self
        .config
        .sdk_config
        .ets_loader_path
        .join("node_modules")
        .join("arkguard"),
      sdk_api_paths: self
        .config
        .sdk_config
        .arkguard_api_dirs(&self.config.product.runtime_os),
      module_root: self.module.src_path.clone(),
      project_root: self.config.project_root.clone(),
      project_path: ModuleBuildLayout::new(&self.module.src_path).source().ets(),
      cache_path: PathBuf::from(&self.loader_json.cache_path),
      ace_module_build: PathBuf::from(&self.loader_json.ace_module_build),
      ace_module_json_path: ModuleBuildLayout::for_product(
        &self.module.src_path,
        &self.config.product.name,
      )
      .res_ark_module_json(),
      compile_sdk_version: self.config.sdk_config.compile_sdk_version.clone(),
      compatible_sdk_version: parse_api_version(&self.config.product.compatible_sdk_version)
        .unwrap_or(self.config.sdk_config.compile_api_version),
      compatible_sdk_version_stage: self.config.product.compatible_sdk_version_stage.clone(),
      entry_package_info: format!("{}|{}", package.name, package.version),
      package_dir: if self.loader_json.package_manager_type == "ohpm" {
        "oh_modules".to_string()
      } else {
        "node_modules".to_string()
      },
      module_path_map,
      entry_array: self.arkguard_entry_array(),
      resource_table_path: self.generated_resource_table_path(),
      local_packages: Vec::new(),
      compile_har: matches!(self.module.module_type, OhosModuleType::Har),
      compile_shared: matches!(self.module.module_type, OhosModuleType::Shared),
      use_normalized: self.loader_json.use_normalized_ohm_url,
      use_ts_har: false,
      byte_code_har_info: serde_json::to_value(&self.loader_json.byte_code_har_info)?,
    };
    Ok((
      adapter,
      ObfuscationModuleInput {
        files,
        source_maps_path: Some(source_maps_path),
      },
    ))
  }

  pub(super) fn bytecode_input(&self, module: ObfuscationModuleInput) -> BytecodeObfuscationInput {
    let module_records = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .map(|source| BytecodeObfuscationModuleRecord {
        source_path: PathBuf::from(&source.module_id),
        record_name: source.record_name.clone(),
      })
      .collect();
    // Latest SDK `module_mode.js::generateCompileContextInfo` registers only
    // explicit module-meta `ohmurl` values. Arkdown's matching meta collector
    // assigns that shape to generated ResourceTable entries (`&...&`).
    let resource_table_path = self.generated_resource_table_path();
    let mut record_name_white_list = self
      .bundle_out
      .bundle_output
      .module_source_files
      .iter()
      .filter(|source| {
        resource_table_path
          .as_ref()
          .is_some_and(|path| Path::new(&source.module_id) == path)
      })
      .map(|source| source.record_name.clone())
      .collect::<Vec<_>>();
    record_name_white_list.sort();
    record_name_white_list.dedup();
    BytecodeObfuscationInput {
      module,
      modules_abc_path: self.bundle_out.bundle_output.modules_abc_path.clone(),
      module_records,
      record_name_white_list,
    }
  }

  pub(super) fn arkguard_entry_array(&self) -> Vec<String> {
    let mut entries = self.loader_json.runtime_entry_sources.clone();
    entries.extend(self.loader_json.hvigor_compile_entry.iter().cloned());
    let mut seen = FxHashSet::default();
    entries.retain(|entry| seen.insert(entry.clone()));
    entries
  }

  pub(super) fn generated_resource_table_path(&self) -> Option<PathBuf> {
    self
      .loader_json
      .hvigor_compile_entry
      .iter()
      .map(PathBuf::from)
      .find(|path| {
        path.file_name().and_then(|name| name.to_str()) == Some("ResourceTable.ts")
          && path
            .strip_prefix(&self.module.src_path)
            .is_ok_and(|relative| {
              relative.starts_with("build")
                && relative
                  .components()
                  .any(|part| part.as_os_str() == "generated")
            })
      })
  }
}

pub(super) struct PendingBytecodeObfuscation {
  pub(super) obfuscator: Obfuscator,
  pub(super) input: BytecodeObfuscationInput,
}

impl PendingBytecodeObfuscation {
  pub(super) fn new(obfuscator: Obfuscator, input: BytecodeObfuscationInput) -> Self {
    Self { obfuscator, input }
  }

  pub(super) fn run(self, module_name: &str) -> Result<()> {
    self
      .obfuscator
      .obfuscate_bytecode(self.input)
      .map_err(|error| {
        anyhow::anyhow!(
          "Bytecode obfuscation failed for module '{}': {}",
          module_name,
          error
        )
      })?;
    Ok(())
  }
}

impl ModuleObfuscationContext {
  pub(super) fn resolve(
    config: &ArkdownConfig,
    session: &BuildSession,
    module: &ModuleConfig,
    build_option: &arkdown_core::BuildOption,
  ) -> Result<Self> {
    let effective = build_option
      .ark_options
      .as_ref()
      .and_then(|options| options.obfuscation.clone());
    let dependency_rule_files = if effective.as_ref().is_some_and(|obfuscation| {
      obfuscation.is_enabled_for_mode(&config.build_mode)
        || !obfuscation.consumer_rule_files.is_empty()
    }) {
      let module_context = session
        .module_context(&module.name)
        .ok_or_else(|| anyhow::anyhow!("missing module compile context for {}", module.name))?;
      ObfuscationDependencyRuleCollector::new(module_context.deps.as_ref()).collect()
    } else {
      Vec::new()
    };
    Ok(Self {
      config: effective,
      dependency_rule_files,
      output_target: config.output_target.clone(),
      har_artifact_type: build_option.har_artifact_type,
      target_name: None,
    })
  }

  pub(super) fn is_enabled(&self, build_mode: &BuildMode) -> bool {
    // hvigor UnitTestArkCompile/OhosTestArkCompile delete obfuscationOptions;
    // local unit-test and ohosTest builds never obfuscate.
    if matches!(self.output_target, OutputTarget::Test)
      || self.target_name.as_deref() == Some("ohosTest")
    {
      return false;
    }
    self
      .config
      .as_ref()
      .is_some_and(|config| config.is_enabled_for_mode(build_mode))
  }

  /// `ProcessObfuscationFiles.taskShouldDo()` from hvigor 6.24.3.
  pub(super) fn should_process_consumer_rules(
    &self,
    module_context: &ModuleCompileContext,
    build_mode: &BuildMode,
  ) -> bool {
    if !matches!(self.output_target, OutputTarget::Har) {
      return false;
    }
    let target_name = self.target_name.as_deref().unwrap_or("default");
    let source_layout = ModuleSourceLayout::new(&module_context.module.root);
    let default_ets = source_layout.ets();
    let target_ets = source_layout.target_ets(target_name);
    let has_ets_source = default_ets.is_dir()
      || (target_name != "default" && target_ets.is_dir())
      || module_context
        .module
        .source_roots
        .iter()
        .any(|root| root == &default_ets || root == &target_ets);
    let is_obfuscated_har = !build_mode.is_debuggable()
      && (self
        .config
        .as_ref()
        .is_some_and(|config| config.enable_obfuscation)
        || matches!(self.har_artifact_type, HarArtifactType::Obfuscation));
    has_ets_source
      && !is_obfuscated_har
      && self
        .config
        .as_ref()
        .is_some_and(|config| !config.consumer_rule_files.is_empty())
  }

  /// `ProcessObfuscationFiles.doTaskAction()`.
  pub(super) fn process_consumer_rules(
    &self,
    module_context: &ModuleCompileContext,
    build_mode: &BuildMode,
    compile_api_version: u32,
    output_path: PathBuf,
  ) -> Result<()> {
    if !self.should_process_consumer_rules(module_context, build_mode)
      || compile_api_version < 10
      || build_mode.is_debuggable()
    {
      return Ok(());
    }
    let config = self.config.as_ref().ok_or_else(|| {
      anyhow::anyhow!("ProcessObfuscationFiles lost its resolved obfuscation configuration")
    })?;
    ConsumerObfuscationFileWriter::new(config.consumer_rule_files.clone(), output_path)
      .with_dependency_rule_files(self.dependency_rule_files.clone())
      .write()
      .map_err(|error| anyhow::anyhow!("Generate obfuscation.txt failed: {error}"))
  }
}
