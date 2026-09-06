//! N-API-facing build service.
//!
//! The npm JavaScript CLI owns argument parsing and process lifecycle. This
//! service accepts an already typed request and never reads command-line argv.

use std::sync::Arc;

use crate::bundle_plan::BundleRuntime;
use crate::coordinator::BuildOutput;
use crate::request::BuildRequest;
use anyhow::Result;
use arkdown::plugins::factory::PackagePlugin;

pub struct BuildService {
    request: BuildRequest,
    package_plugins: Vec<PackagePlugin>,
}

impl BuildService {
    pub fn new(request: BuildRequest) -> Self {
        Self {
            request,
            package_plugins: Vec::new(),
        }
    }

    pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
        self.package_plugins = package_plugins;
        self
    }

    pub async fn execute(self) -> Result<BuildOutput> {
        let retain_bundle_graph = self.request.watch
            || self.request.preview
            || (self.request.action.is_hot_reload() && self.request.watch_invalidation.is_none());
        if retain_bundle_graph {
            let mut runtime = BundleRuntime::default();
            // Erase the coordinator future here so the public N-API build
            // future does not recursively embed the complete stage/watch
            // state machine in its layout.
            let result = Box::pin(self.execute_inner(Some(&mut runtime))).await;
            let close_result = runtime.close().await;
            let output = result?;
            close_result?;
            Ok(output)
        } else {
            Box::pin(self.execute_inner(None)).await
        }
    }

    pub(crate) async fn execute_with_bundle_runtime(
        self,
        runtime: &mut BundleRuntime,
    ) -> Result<BuildOutput> {
        self.execute_inner(Some(runtime)).await
    }

    async fn execute_inner(
        self,
        bundle_runtime: Option<&mut BundleRuntime>,
    ) -> Result<BuildOutput> {
        use arkdown_config::ArkdownConfigLoader;
        use arkdown_session::BuildSessionBuilder;

        let request = self.request;
        let package_plugins = self.package_plugins;
        let project_root = request
            .project
            .clone()
            .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

        let mode = request.build_mode()?;
        let target = request.output_target()?;
        let mut config = ArkdownConfigLoader::new(
            &project_root,
            mode,
            target,
            request.product.as_deref(),
            None,
        )
        .load()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
        if let Some(build_version) = request.build_version.as_ref() {
            config.product.build_version = Some(build_version.clone());
        }
        let arkts_version = request
            .arkts_version
            .unwrap_or(config.product.arkts_version);
        let config = Arc::new(config);

        let mut session_builder =
            BuildSessionBuilder::new(config.clone()).with_arkts_language_version(arkts_version);
        if let Some(parameter_file_override) = request.parameter_file_override.clone() {
            session_builder = session_builder.with_parameter_file_override(parameter_file_override);
        }
        let session = tokio::task::spawn_blocking(move || {
            session_builder
                .build()
                .map(Arc::new)
                .map_err(|error| anyhow::anyhow!("{error}"))
        })
        .await
        .map_err(|error| anyhow::anyhow!("build startup worker failed: {error}"))??;

        if request.target == "app" {
            let coordinator =
                super::app_coordinator::AppBuildCoordinator::new(session, config, request)
                    .with_package_plugins(package_plugins);
            return coordinator.execute().await;
        }

        let coordinator = super::coordinator::BuildCoordinator::new(session, config, request)
            .with_package_plugins(package_plugins);
        let output = match bundle_runtime {
            Some(runtime) => coordinator.execute_with_bundle_runtime(runtime).await?,
            None => coordinator.execute().await?,
        };
        tracing::debug!(artifacts = output.artifact_paths.len(), "build completed");
        Ok(output)
    }
}
