//! Target/module/native/dependency planning helpers for the build coordinator.

use super::*;

pub(super) struct TargetBuildExecutionPlan {
  pub(super) target_names: Vec<String>,
}

impl TargetBuildExecutionPlan {
  pub(super) fn new(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    args: &BuildArgs,
  ) -> Self {
    if Self::is_device_test(args) {
      let has_ohos_test = config.modules.iter().any(|module| {
        args.module.as_ref().is_none_or(|name| name == &module.name)
          && ModuleSourceLayout::new(&module.src_path)
            .ohos_test_module_json5()
            .is_file()
      });
      let mut target_names = Vec::with_capacity(2);
      // Build the test HAP first so the default target's
      // GenerateDeviceCoverage action can consume both completed packages.
      // Hvigor's DAG imposes the same dependency without prescribing worker
      // scheduling order.
      if has_ohos_test {
        target_names.push("ohosTest".to_string());
      }
      target_names.push("default".to_string());
      return Self { target_names };
    }
    if let Some(align_target) = Self::aligned_target_for_action(&config.output_target, args) {
      return Self {
        target_names: vec![align_target],
      };
    }
    if let Some(target_name) = args.build_target_name.as_ref() {
      return Self {
        target_names: vec![target_name.clone()],
      };
    }
    let build_mode_name = config.build_mode.as_str();
    let mut applicable_target_names = Vec::new();
    for module in config
      .modules
      .iter()
      .filter(|module| args.module.as_ref().is_none_or(|name| name == &module.name))
    {
      for target in
        build_options.applicable_targets(&module.name, &config.product.name, build_mode_name)
      {
        applicable_target_names.push(target.name);
      }
    }
    Self::from_implicit_targets(applicable_target_names)
  }

  pub(super) fn is_device_test(args: &BuildArgs) -> bool {
    args.native_task_roots.iter().any(|root| {
      matches!(
        root.task_name.as_str(),
        "onDeviceTest" | "GenerateDeviceCoverage"
      )
    })
  }

  pub(super) fn aligned_target_for_action(
    output_target: &OutputTarget,
    args: &BuildArgs,
  ) -> Option<String> {
    let test_action = matches!(output_target, OutputTarget::Test)
      || args.build_target_name.as_deref() == Some("ohosTest");
    (!test_action)
      .then(|| args.hvigor_properties.align_target.clone())
      .flatten()
  }

  pub(super) fn from_implicit_targets(target_names: impl IntoIterator<Item = String>) -> Self {
    let mut selected = Vec::new();
    for target_name in target_names {
      // Hvigor 6.24.3 `ModuleTaskService.targetNeedPack()` treats an ordinary
      // assemble as target `all`, but explicitly excludes `ohosTest`. Device
      // tests and `module=<name>@ohosTest` select that target through the
      // explicit branch above.
      if target_name == "ohosTest" || selected.contains(&target_name) {
        continue;
      }
      selected.push(target_name);
    }
    Self {
      target_names: selected,
    }
  }
}

pub(super) struct ModuleTargetSelection {
  pub(super) target: EffectiveTargetConfig,
}

impl ModuleTargetSelection {
  pub(super) fn resolve_build(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    requested_target: Option<&str>,
    properties: &HvigorRuntimeProperties,
  ) -> Self {
    let build_mode_name = config.build_mode.as_str();
    let is_test_action =
      matches!(config.output_target, OutputTarget::Test) || requested_target == Some("ohosTest");
    if !is_test_action {
      if let Some(align_target) = properties.align_target.as_deref() {
        let declared = build_options.declared_targets(&module.name);
        let candidates = std::iter::once(align_target)
          .chain(properties.fallback_targets.iter().map(String::as_str))
          .chain(std::iter::once("default"));
        if let Some(target) = Self::first_named_target(&declared, candidates) {
          return Self { target };
        }
        return Self::default_target();
      }
    }
    if let Some(requested_target) = requested_target {
      let applicable =
        build_options.applicable_targets(&module.name, &config.product.name, build_mode_name);
      let requested = applicable
        .into_iter()
        .find(|target| target.name == requested_target);
      if let Some(target) = requested {
        return Self { target };
      }
      // The `ohosTest` target is created dynamically (hvigor accepts
      // `-p module=entry@ohosTest` without a build-profile entry as long as
      // `src/ohosTest/module.json5` exists).
      if requested_target == "ohosTest"
        && ModuleSourceLayout::new(&module.src_path)
          .ohos_test_module_json5()
          .is_file()
      {
        return Self::named_synthetic_target("ohosTest");
      }
      tracing::debug!(
        module = %module.name,
        requested_target,
        "requested target does not apply to this module; using its effective target"
      );
    }
    let target = build_options
      .effective_target(&module.name, &config.product.name, build_mode_name)
      .unwrap_or_else(Self::default_target_config);
    Self { target }
  }

  pub(super) fn resolve_dependency(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    super_target: Option<&str>,
    properties: &HvigorRuntimeProperties,
  ) -> Self {
    if super_target == Some("ohosTest") {
      let build_mode_name = config.build_mode.as_str();
      if let Some(target) = build_options
        .applicable_targets(&module.name, &config.product.name, build_mode_name)
        .into_iter()
        .find(|target| target.name == "ohosTest")
      {
        return Self { target };
      }
      if ModuleSourceLayout::new(&module.src_path)
        .ohos_test_module_json5()
        .is_file()
      {
        return Self::named_synthetic_target("ohosTest");
      }
      return Self::default_target();
    }
    if matches!(config.output_target, OutputTarget::Test) {
      return Self::default_target();
    }

    let build_mode_name = config.build_mode.as_str();
    let targets = if properties.align_target.is_some() {
      build_options.declared_targets(&module.name)
    } else {
      build_options.applicable_targets(&module.name, &config.product.name, build_mode_name)
    };
    let candidates = properties
      .align_target
      .as_deref()
      .into_iter()
      .chain(super_target.filter(|target| *target != "all"))
      .chain(properties.fallback_targets.iter().map(String::as_str))
      .chain(std::iter::once("default"));
    if let Some(target) = Self::first_named_target(&targets, candidates) {
      return Self { target };
    }
    let target = build_options
      .effective_target(&module.name, &config.product.name, build_mode_name)
      .unwrap_or_else(Self::default_target_config);
    Self { target }
  }

  pub(super) fn first_named_target<'a>(
    targets: &[EffectiveTargetConfig],
    candidates: impl IntoIterator<Item = &'a str>,
  ) -> Option<EffectiveTargetConfig> {
    for candidate in candidates {
      if let Some(target) = targets.iter().find(|target| target.name == candidate) {
        return Some(target.clone());
      }
    }
    None
  }

  pub(super) fn named_synthetic_target(name: &str) -> Self {
    Self {
      target: EffectiveTargetConfig {
        name: name.to_string(),
        ..EffectiveTargetConfig::default()
      },
    }
  }

  pub(super) fn default_target() -> Self {
    Self {
      target: Self::default_target_config(),
    }
  }

  pub(super) fn default_target_config() -> EffectiveTargetConfig {
    EffectiveTargetConfig {
      name: "default".to_string(),
      ..EffectiveTargetConfig::default()
    }
  }
}

#[derive(Clone)]
pub(super) struct ModuleBuildConfiguration {
  pub(super) target_name: String,
  pub(super) target: EffectiveTargetConfig,
  pub(super) config: Arc<ArkdownConfig>,
}

impl ModuleBuildConfiguration {
  pub(super) fn resolve_build(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    requested_target: Option<&str>,
    build_script_override: Option<&serde_json::Value>,
    properties: &HvigorRuntimeProperties,
  ) -> Result<Self> {
    let target = ModuleTargetSelection::resolve_build(
      config,
      build_options,
      module,
      requested_target,
      properties,
    )
    .target;
    Self::resolve_for_target(config, build_options, module, target, build_script_override)
  }

  pub(super) fn resolve_dependency(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    super_target: Option<&str>,
    build_script_override: Option<&serde_json::Value>,
    properties: &HvigorRuntimeProperties,
  ) -> Result<Self> {
    let target = ModuleTargetSelection::resolve_dependency(
      config,
      build_options,
      module,
      super_target,
      properties,
    )
    .target;
    Self::resolve_for_target(config, build_options, module, target, build_script_override)
  }

  pub(super) fn resolve_for_target(
    config: &ArkdownConfig,
    build_options: &BuildOptionProfileInventory,
    module: &ModuleConfig,
    mut target: EffectiveTargetConfig,
    build_script_override: Option<&serde_json::Value>,
  ) -> Result<Self> {
    let target_name = target.name.clone();
    let mut resolver = EffectiveModuleBuildOptionResolver::new(
      build_options,
      &module.name,
      &config.product.name,
      config.build_mode.as_str(),
      &target_name,
    );
    if let Some(build_script_override) = build_script_override {
      resolver = resolver.with_build_script_override(build_script_override.clone());
    }
    let build_option = resolver.resolve().with_context(|| {
      format!(
        "resolve effective build option for module '{}' target '{}'",
        module.name, target_name
      )
    })?;
    target.artifact_name = build_options
      .module_target_artifact_name(&module.name, &target_name)
      .map(str::to_string)
      .or(target.artifact_name);
    let mut resolved = config.clone();
    resolved.product.build_option = build_option;
    // `ModuleTargetData.getModuleTargetOutputFileName` reads the module
    // target's output override. Product output naming belongs to APP
    // packaging and must not leak into HAP/HSP/HAR archive names.
    resolved.product.artifact_name = target.artifact_name.clone();
    // Hvigor always packages the synthetic `ohosTest` target as a HAP,
    // regardless of whether the owning module normally produces HAP, HSP,
    // or HAR artifacts (`deviceTestComputeDepends` always points the test
    // side at `SignHap`).
    if target_name == "ohosTest" {
      resolved.output_target = OutputTarget::Hap;
    }
    Ok(Self {
      target_name,
      target,
      config: Arc::new(resolved),
    })
  }
}

/// Builds the local target-service inventory consumed by Hvigor's
/// `ConfigureCmake.PackageResolver`. This keeps dependency target selection in
/// the coordinator and passes only resolved immutable native data to the
/// native domain owner.
pub(super) struct NativeCmakeLocalDependencyInventoryBuilder<'a> {
  pub(super) config: &'a ArkdownConfig,
  pub(super) session: &'a BuildSession,
  pub(super) args: &'a BuildArgs,
  pub(super) target_name: &'a str,
  pub(super) dependencies: &'a DependencyGraphSnapshot,
}

impl<'a> NativeCmakeLocalDependencyInventoryBuilder<'a> {
  pub(super) fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    args: &'a BuildArgs,
    target_name: &'a str,
    dependencies: &'a DependencyGraphSnapshot,
  ) -> Self {
    Self {
      config,
      session,
      args,
      target_name,
      dependencies,
    }
  }

  pub(super) fn build(&self) -> Result<Vec<NativeCmakeLocalDependency>> {
    let dependency_roots = self
      .dependencies
      .nodes
      .values()
      .filter_map(|node| node.package_root.as_deref())
      .map(canonicalize_or_owned)
      .collect::<FxHashSet<_>>();
    let mut dependencies = Vec::new();
    for module in self
      .config
      .modules
      .iter()
      .filter(|module| dependency_roots.contains(&canonicalize_or_owned(&module.src_path)))
    {
      let resolved = ModuleBuildConfiguration::resolve_dependency(
        self.config,
        self.session.build_options.as_ref(),
        module,
        Some(self.target_name),
        self.args.module_hvigor_overrides.get(&module.name),
        &self.args.hvigor_properties,
      )?;
      let native = arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
        &resolved.config.product.build_option,
      );
      let Some(external) = native.external_native_options.as_ref() else {
        continue;
      };
      let default_cmake = module
        .src_path
        .join("src")
        .join("main")
        .join("cpp")
        .join("CMakeLists.txt");
      let has_native_source =
        external.path.as_ref().is_some_and(|path| path.is_file()) || default_cmake.is_file();
      if !has_native_source {
        continue;
      }
      dependencies.push(NativeCmakeLocalDependency::new(
        module.src_path.clone(),
        resolved.config.product.name.clone(),
        resolved.target_name,
        resolved.config.build_mode.as_str().to_string(),
        module.module_type.clone(),
        native.native_lib,
      ));
    }
    Ok(dependencies)
  }
}

/// Projects hvigor's cross-module native task edges into dependency-first
/// coarse stage waves without adding HAR/HSP modules to the artifact build
/// set. `ConfigureCmake` and `BuildNativeWithNinja` consume these modules as
/// native prerequisites only.
pub(super) struct NativeDependencyStagePlanBuilder<'a> {
  pub(super) config: &'a ArkdownConfig,
  pub(super) session: &'a BuildSession,
  pub(super) args: &'a BuildArgs,
}

impl<'a> NativeDependencyStagePlanBuilder<'a> {
  pub(super) fn new(
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    args: &'a BuildArgs,
  ) -> Self {
    Self {
      config,
      session,
      args,
    }
  }

  pub(super) fn build(
    &self,
    roots: &[&ModuleConfig],
    root_configs: &[ModuleBuildConfiguration],
  ) -> Result<NativeDependencyStagePlan> {
    let mut planned = Vec::new();
    let mut visiting = FxHashSet::default();
    let mut visited = FxHashMap::default();
    for (root, resolved) in roots.iter().zip(root_configs) {
      for dependency in self.direct_local_dependencies(root)? {
        self.visit(
          &dependency,
          &resolved.target_name,
          &mut visiting,
          &mut visited,
          &mut planned,
        )?;
      }
    }
    let mut waves = BTreeMap::<usize, Vec<NativeDependencyStageInput>>::new();
    for (wave, input) in planned {
      waves.entry(wave).or_default().push(input);
    }
    Ok(NativeDependencyStagePlan::new(
      waves.into_values().collect(),
    ))
  }

  pub(super) fn visit(
    &self,
    module: &ModuleConfig,
    super_target: &str,
    visiting: &mut FxHashSet<(PathBuf, String)>,
    visited: &mut FxHashMap<(PathBuf, String), Option<usize>>,
    planned: &mut Vec<(usize, NativeDependencyStageInput)>,
  ) -> Result<Option<usize>> {
    let resolved = ModuleBuildConfiguration::resolve_dependency(
      self.config,
      self.session.build_options.as_ref(),
      module,
      Some(super_target),
      self.args.module_hvigor_overrides.get(&module.name),
      &self.args.hvigor_properties,
    )?;
    let key = (
      canonicalize_or_owned(&module.src_path),
      resolved.target_name.clone(),
    );
    if let Some(wave) = visited.get(&key) {
      return Ok(*wave);
    }
    if !visiting.insert(key.clone()) {
      anyhow::bail!(
        "NATIVE_DEPENDENCY_CYCLE: local native module '{}' target '{}' participates in a dependency cycle",
        module.name,
        resolved.target_name
      );
    }
    let mut dependency_wave = None;
    for dependency in self.direct_local_dependencies(module)? {
      let wave = self.visit(
        &dependency,
        &resolved.target_name,
        visiting,
        visited,
        planned,
      )?;
      dependency_wave = dependency_wave.max(wave);
    }
    visiting.remove(&key);

    let native = arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
      &resolved.config.product.build_option,
    );
    let Some(external) = native.external_native_options.as_ref() else {
      visited.insert(key, dependency_wave);
      return Ok(dependency_wave);
    };
    let default_cmake = module
      .src_path
      .join("src")
      .join("main")
      .join("cpp")
      .join("CMakeLists.txt");
    if !external.path.as_ref().is_some_and(|path| path.is_file()) && !default_cmake.is_file() {
      visited.insert(key, dependency_wave);
      return Ok(dependency_wave);
    }
    let module_context = self.session.module_context(&module.name).ok_or_else(|| {
      anyhow::anyhow!(
        "missing build session module context for native dependency '{}'",
        module.name
      )
    })?;
    let local_dependencies = Arc::from(
      NativeCmakeLocalDependencyInventoryBuilder::new(
        self.config,
        self.session,
        self.args,
        &resolved.target_name,
        module_context.deps.as_ref(),
      )
      .build()?,
    );
    let wave = dependency_wave.map_or(0, |wave| wave + 1);
    planned.push((
      wave,
      NativeDependencyStageInput {
        module: module.clone(),
        module_context,
        target_name: resolved.target_name,
        config: resolved.config,
        local_dependencies,
      },
    ));
    visited.insert(key, Some(wave));
    Ok(Some(wave))
  }

  pub(super) fn direct_local_dependencies(
    &self,
    module: &ModuleConfig,
  ) -> Result<Vec<ModuleConfig>> {
    let context = self.session.module_context(&module.name).ok_or_else(|| {
      anyhow::anyhow!(
        "missing build session module context while planning native dependencies for '{}'",
        module.name
      )
    })?;
    let host_root = canonicalize_or_owned(&context.deps.host_root);
    let mut dependencies = Vec::new();
    let mut seen = FxHashSet::default();
    for edge in &context.deps.edges {
      if edge.kind == DependencyEdgeKind::DevDependency
        || canonicalize_or_owned(&edge.issuer_root) != host_root
      {
        continue;
      }
      let dependency_root = canonicalize_or_owned(&edge.package_root);
      let Some(dependency) = self
        .config
        .modules
        .iter()
        .find(|candidate| canonicalize_or_owned(&candidate.src_path) == dependency_root)
      else {
        continue;
      };
      if seen.insert(dependency.name.as_str()) {
        dependencies.push(dependency.clone());
      }
    }
    Ok(dependencies)
  }
}

/// Builds the exact dependency custom-type input list consumed by
/// `ArkCompile.collectCustomTypes()`. Local source HARs use their effective
/// target build option; packaged HARs use `metadata.customTypes`.
pub(super) struct DependencyCustomTypeInputBuilder<'a> {
  pub(super) dependencies: &'a DependencyGraphSnapshot,
  pub(super) config: &'a ArkdownConfig,
  pub(super) build_options: &'a BuildOptionProfileInventory,
  pub(super) target_name: &'a str,
  pub(super) module_overrides: &'a BTreeMap<String, serde_json::Value>,
  pub(super) properties: &'a HvigorRuntimeProperties,
}

impl<'a> DependencyCustomTypeInputBuilder<'a> {
  pub(super) fn new(
    dependencies: &'a DependencyGraphSnapshot,
    config: &'a ArkdownConfig,
    build_options: &'a BuildOptionProfileInventory,
    target_name: &'a str,
    module_overrides: &'a BTreeMap<String, serde_json::Value>,
    properties: &'a HvigorRuntimeProperties,
  ) -> Self {
    Self {
      dependencies,
      config,
      build_options,
      target_name,
      module_overrides,
      properties,
    }
  }

  pub(super) fn collect(&self) -> Result<Vec<DependencyCustomTypeInput>> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for edge in &self.dependencies.edges {
      let Some(node) = self.dependencies.nodes.get(&edge.package_name) else {
        continue;
      };
      if !matches!(
        node.node_type,
        DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
      ) || !seen.insert(node.package_name.as_str())
      {
        continue;
      }
      let Some(package_root) = node.package_root.clone() else {
        continue;
      };
      let local_module = self.config.modules.iter().find(|module| {
        canonicalize_or_owned(&module.src_path) == canonicalize_or_owned(&package_root)
      });
      let configured_types = if let Some(module) = local_module {
        ModuleBuildConfiguration::resolve_dependency(
          self.config,
          self.build_options,
          module,
          Some(self.target_name),
          self.module_overrides.get(&module.name),
          self.properties,
        )?
        .config
        .product
        .build_option
        .ark_options
        .as_ref()
        .map(|options| options.types.clone())
        .unwrap_or_default()
      } else {
        node
          .package
          .as_deref()
          .and_then(|package| package.package_json.extra.get("metadata"))
          .and_then(|metadata| metadata.get("customTypes"))
          .and_then(serde_json::Value::as_array)
          .map(|types| {
            types
              .iter()
              .filter_map(serde_json::Value::as_str)
              .map(str::to_string)
              .collect()
          })
          .unwrap_or_default()
      };
      let mut declared_dependencies = BTreeSet::new();
      if let Some(package) = node.package.as_deref() {
        declared_dependencies.extend(package.package_json.dependencies.keys().cloned());
        declared_dependencies.extend(package.package_json.dev_dependencies.keys().cloned());
      }
      output.push(DependencyCustomTypeInput {
        package_name: node.package_name.clone(),
        package_root,
        configured_types,
        declared_dependencies,
      });
    }
    Ok(output)
  }
}

/// Recursively copy a directory tree (hvigor `fs-extra.copy` semantics).
pub(super) fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
  for entry in walkdir::WalkDir::new(source) {
    let entry = entry.map_err(|error| anyhow::anyhow!("copy tree walk: {error}"))?;
    let relative = entry
      .path()
      .strip_prefix(source)
      .map_err(|error| anyhow::anyhow!("copy tree strip prefix: {error}"))?;
    let target = destination.join(relative);
    if entry.file_type().is_dir() {
      std::fs::create_dir_all(&target)?;
    } else if entry.file_type().is_file() {
      if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::copy(entry.path(), &target)?;
    }
  }
  Ok(())
}

pub(super) struct BuildTimingProbe<'a> {
  pub(super) stage: &'static str,
  pub(super) module: Option<&'a str>,
  pub(super) start: Instant,
}

impl<'a> BuildTimingProbe<'a> {
  pub(super) fn global(stage: &'static str) -> Self {
    Self {
      stage,
      module: None,
      start: Instant::now(),
    }
  }

  pub(super) fn module(module: &'a str, stage: &'static str) -> Self {
    Self {
      stage,
      module: Some(module),
      start: Instant::now(),
    }
  }
}

impl Drop for BuildTimingProbe<'_> {
  fn drop(&mut self) {
    let elapsed_ms = self.start.elapsed().as_millis();
    if let Some(module) = self.module {
      tracing::debug!(
          module = %module,
          stage = self.stage,
          elapsed_ms,
          "profile substage completed"
      );
    } else {
      tracing::debug!(stage = self.stage, elapsed_ms, "profile substage completed");
    }
  }
}
