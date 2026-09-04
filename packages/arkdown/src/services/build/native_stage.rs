use std::sync::Arc;

use anyhow::Result;
use arkdown_config::build_profile::EffectiveNativeBuildOption;
use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_session::ModuleCompileContext;

use crate::services::native_build::{
  NativeCmakeLocalDependency, NativeLibCollectionPolicy, NativeLibCollector, NativeLibPackageInfo,
};

use super::coordinator::BuildTaskLifecycle;

/// One local module whose native outputs are required by a dependent module.
///
/// Latest hvigor reaches this module through the cross-node edges declared by
/// `ConfigureCmake.initTaskDepends()` and
/// `NinjaCA.getModuleDependencies(...DoNativeStrip)`. The ordinary artifact
/// module set must remain unchanged; this input represents only the native
/// dependency boundary.
pub(super) struct NativeDependencyStageInput {
  pub(super) module: ModuleConfig,
  pub(super) module_context: ModuleCompileContext,
  pub(super) config: Arc<ArkdownConfig>,
  pub(super) target_name: String,
  pub(super) local_dependencies: Arc<[NativeCmakeLocalDependency]>,
}

/// Dependency-first native module waves for one build invocation.
///
/// Waves are kept explicit even though the current executor runs them
/// sequentially. CMake projects can share generated dependency packages, so
/// parallel execution requires a disjoint-output proof before it is enabled.
#[derive(Default)]
pub(super) struct NativeDependencyStagePlan {
  waves: Vec<Vec<NativeDependencyStageInput>>,
}

impl NativeDependencyStagePlan {
  pub(super) fn new(waves: Vec<Vec<NativeDependencyStageInput>>) -> Self {
    Self { waves }
  }

  pub(super) fn is_empty(&self) -> bool {
    self.waves.is_empty()
  }
}

/// Executes the hvigor local-native dependency boundary before dependent
/// module stages consume its code model or ProcessLibs output.
pub(super) struct NativeDependencyStageExecutor<'a> {
  lifecycle: &'a mut BuildTaskLifecycle,
  base_policy: &'a NativeLibCollectionPolicy,
  release_cmake_build_type: Option<&'a str>,
}

impl<'a> NativeDependencyStageExecutor<'a> {
  pub(super) fn new(
    lifecycle: &'a mut BuildTaskLifecycle,
    base_policy: &'a NativeLibCollectionPolicy,
    release_cmake_build_type: Option<&'a str>,
  ) -> Self {
    Self {
      lifecycle,
      base_policy,
      release_cmake_build_type,
    }
  }

  pub(super) async fn execute(&mut self, plan: &NativeDependencyStagePlan) -> Result<()> {
    for wave in &plan.waves {
      for input in wave {
        self.execute_module(input).await?;
      }
    }
    Ok(())
  }

  async fn execute_module(&mut self, input: &NativeDependencyStageInput) -> Result<()> {
    let native = EffectiveNativeBuildOption::from_build_option(&input.config.product.build_option);
    let self_package = input
      .module_context
      .module
      .package
      .as_ref()
      .map(|package| NativeLibPackageInfo::local(package.name.clone(), package.version.clone()))
      .unwrap_or_else(|| NativeLibPackageInfo::local(input.module.name.clone(), String::new()));
    let policy = self
      .base_policy
      .clone()
      .with_release_cmake_build_type(self.release_cmake_build_type)
      .with_external_native_options(native.external_native_options.clone())
      .with_native_lib_option(native.native_lib)
      .with_self_package(self_package)
      .with_cmake_local_dependency_inventory(Arc::clone(&input.local_dependencies));
    let collector = NativeLibCollector::new(
      &input.module,
      &native.abi_filters,
      input.module_context.deps.as_ref(),
    )
    .with_output_variant(&input.config.product.name, &input.target_name)
    .with_policy(policy);
    let module_name = input.module.name.as_str();
    let target_name = input.target_name.as_str();

    if self
      .lifecycle
      .before(module_name, Some(target_name), "ConfigureCmake")
      .await?
    {
      collector.configure_cmake_dependencies().map_err(|error| {
        anyhow::anyhow!("ConfigureCmake failed for dependency '{module_name}': {error}")
      })?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "ConfigureCmake")
        .await?;
    }
    if self
      .lifecycle
      .before(module_name, Some(target_name), "BuildNativeWithCmake")
      .await?
    {
      collector.configure_native().map_err(|error| {
        anyhow::anyhow!("BuildNativeWithCmake failed for dependency '{module_name}': {error}")
      })?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "BuildNativeWithCmake")
        .await?;
    }
    if self
      .lifecycle
      .before(module_name, Some(target_name), "BuildNativeWithNinja")
      .await?
    {
      self
        .lifecycle
        .execute_ninja_build(module_name, target_name, &collector)
        .await?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "BuildNativeWithNinja")
        .await?;
    }

    let processed = if self
      .lifecycle
      .before(module_name, Some(target_name), "ProcessLibs")
      .await?
    {
      let outputs = collector.process_libs().map_err(|error| {
        anyhow::anyhow!("ProcessLibs failed for dependency '{module_name}': {error}")
      })?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "ProcessLibs")
        .await?;
      outputs
    } else {
      Vec::new()
    };
    let processed = if collector.is_binxo_enabled()
      && self
        .lifecycle
        .before(module_name, Some(target_name), "DoNativeBinxo")
        .await?
    {
      let outputs = collector.binxo_libs(&processed).map_err(|error| {
        anyhow::anyhow!("DoNativeBinxo failed for dependency '{module_name}': {error}")
      })?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "DoNativeBinxo")
        .await?;
      outputs
    } else {
      processed
    };
    if self
      .lifecycle
      .before(module_name, Some(target_name), "DoNativeStrip")
      .await?
    {
      collector.strip_libs(&processed).map_err(|error| {
        anyhow::anyhow!("DoNativeStrip failed for dependency '{module_name}': {error}")
      })?;
      self
        .lifecycle
        .after(module_name, Some(target_name), "DoNativeStrip")
        .await?;
    }
    Ok(())
  }
}
