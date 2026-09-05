use std::sync::Arc;

use crate::native_dependency::{NativeDependencyStageInput, NativeDependencyStagePlan};
use anyhow::Result;
use arkdown_config::build_profile::EffectiveNativeBuildOption;
use arkdown_native::{NativeLibCollectionPolicy, NativeLibCollector, NativeLibPackageInfo};

/// Executes the hvigor local-native dependency boundary before dependent
/// module stages consume its code model or ProcessLibs output.
pub(super) struct NativeDependencyStageExecutor<'a> {
    base_policy: &'a NativeLibCollectionPolicy,
    release_cmake_build_type: Option<&'a str>,
}

impl<'a> NativeDependencyStageExecutor<'a> {
    pub(super) fn new(
        base_policy: &'a NativeLibCollectionPolicy,
        release_cmake_build_type: Option<&'a str>,
    ) -> Self {
        Self {
            base_policy,
            release_cmake_build_type,
        }
    }

    pub(super) async fn execute(&mut self, plan: &NativeDependencyStagePlan) -> Result<()> {
        for wave in plan.waves() {
            for input in wave {
                self.execute_module(input).await?;
            }
        }
        Ok(())
    }

    async fn execute_module(&mut self, input: &NativeDependencyStageInput) -> Result<()> {
        let native =
            EffectiveNativeBuildOption::from_build_option(&input.config.product.build_option);
        let self_package = input
            .module_context
            .module
            .package
            .as_ref()
            .map(|package| {
                NativeLibPackageInfo::local(package.name.clone(), package.version.clone())
            })
            .unwrap_or_else(|| {
                NativeLibPackageInfo::local(input.module.name.clone(), String::new())
            });
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

        collector.configure_cmake_dependencies().map_err(|error| {
            anyhow::anyhow!("ConfigureCmake failed for dependency '{module_name}': {error}")
        })?;
        collector.configure_native().map_err(|error| {
            anyhow::anyhow!("BuildNativeWithCmake failed for dependency '{module_name}': {error}")
        })?;
        collector
            .build_native()
            .map_err(|error| anyhow::anyhow!("BuildNativeWithNinja failed: {error}"))?;

        let processed = collector.process_libs().map_err(|error| {
            anyhow::anyhow!("ProcessLibs failed for dependency '{module_name}': {error}")
        })?;
        let processed = if collector.is_binxo_enabled() {
            collector.binxo_libs(&processed).map_err(|error| {
                anyhow::anyhow!("DoNativeBinxo failed for dependency '{module_name}': {error}")
            })?
        } else {
            processed
        };
        collector.strip_libs(&processed).map_err(|error| {
            anyhow::anyhow!("DoNativeStrip failed for dependency '{module_name}': {error}")
        })?;
        Ok(())
    }
}
