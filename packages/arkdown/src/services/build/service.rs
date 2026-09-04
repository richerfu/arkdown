//! N-API-facing build service.
//!
//! The npm JavaScript CLI owns argument parsing and process lifecycle. This
//! service accepts an already typed request and never reads command-line argv.

use std::sync::Arc;

use anyhow::Result;
use arkdown::plugins::factory::PackagePlugin;
use arkdown_build::request::BuildRequest;

use super::coordinator::BuildTaskObserver;

pub struct BuildService {
  request: BuildRequest,
  package_plugins: Vec<PackagePlugin>,
  task_observer: Option<Arc<dyn BuildTaskObserver>>,
}

impl BuildService {
  pub fn new(request: BuildRequest) -> Self {
    Self {
      request,
      package_plugins: Vec::new(),
      task_observer: None,
    }
  }

  pub fn with_package_plugins(mut self, package_plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = package_plugins;
    self
  }

  pub fn with_task_observer(mut self, task_observer: Arc<dyn BuildTaskObserver>) -> Self {
    self.task_observer = Some(task_observer);
    self
  }

  pub async fn execute(self) -> Result<()> {
    use arkdown_config::ArkdownConfigLoader;
    use arkdown_session::BuildSessionBuilder;

    let request = self.request;
    let package_plugins = self.package_plugins;
    let task_observer = self.task_observer;
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
    let config = Arc::new(config);
    let mut session_builder = BuildSessionBuilder::new(config.clone())
      .with_arkts_language_version(request.arkts_version)
      .with_ohpm_mode(request.ohpm_mode.into());
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
      let mut coordinator =
        super::app_coordinator::AppBuildCoordinator::new(session, config, request)
          .with_package_plugins(package_plugins);
      if let Some(task_observer) = task_observer {
        coordinator = coordinator.with_task_observer(task_observer);
      }
      coordinator.execute().await?;
      return Ok(());
    }

    let mut coordinator = super::coordinator::BuildCoordinator::new(session, config, request)
      .with_package_plugins(package_plugins);
    if let Some(task_observer) = task_observer {
      coordinator = coordinator.with_task_observer(task_observer);
    }
    let output = coordinator.execute().await?;
    tracing::debug!(artifacts = output.artifact_paths.len(), "build completed");
    Ok(())
  }
}
