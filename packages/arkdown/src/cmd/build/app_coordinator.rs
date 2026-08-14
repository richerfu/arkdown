//! Project-level `assembleApp` orchestration.
//!
//! Hvigor owns APP assembly on the project node, after all applicable HAP/HSP
//! module tasks. This coordinator keeps that entire graph inside one lifecycle
//! cycle so TypeScript task plugins observe the same dependency completion
//! state across module and project boundaries.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use arkdown::plugins::factory::PackagePlugin;
use arkdown_core::{ArkdownConfig, OutputTarget};
use arkdown_pack::{
  AppArchiveUnpacker, AppArtifactInput, AppArtifactStager, AppModulePackageKind, AppNormalizePlan,
  AppPackageMode, AppPackagePlan, AppPackageRunner, ProjectPackInfoBuilder, RemoteHspCollector,
  RemoteHspDependencyScope, StagedAppArtifacts, UnpackedAppArtifacts,
};
use arkdown_profile::{
  PrivacyProfileProcessor, ProjectPrivacyProfileInput, ProjectPrivacyProfileOutput,
};
use arkdown_session::{BuildSession, DuplicateDependencyValidator, SdkToolKind};
use arkdown_sign::SignBackendRunner;

use crate::cmd::native_build::ProjectDebugSymbolCollector;

use super::args::BuildArgs;
use super::coordinator::{
  BuildCoordinator, BuildOutput, BuildTaskLifecycle, BuildTaskObserver, NativeTaskCutoffReached,
};
use super::layout::ModuleBuildLayout;

pub struct AppBuildCoordinator {
  session: Arc<BuildSession>,
  config: Arc<ArkdownConfig>,
  args: BuildArgs,
  package_plugins: Vec<PackagePlugin>,
  task_observer: Option<Arc<dyn BuildTaskObserver>>,
}

impl AppBuildCoordinator {
  pub fn new(session: Arc<BuildSession>, config: Arc<ArkdownConfig>, args: BuildArgs) -> Self {
    Self {
      session,
      config,
      args,
      package_plugins: Vec::new(),
      task_observer: None,
    }
  }

  pub fn with_package_plugins(mut self, plugins: Vec<PackagePlugin>) -> Self {
    self.package_plugins = plugins;
    self
  }

  pub fn with_task_observer(mut self, observer: Arc<dyn BuildTaskObserver>) -> Self {
    self.task_observer = Some(observer);
    self
  }

  pub async fn execute(self) -> Result<BuildOutput> {
    let observer = self.task_observer.clone();
    match self.execute_impl().await {
      Ok(output) => Ok(output),
      Err(error) if error.is::<NativeTaskCutoffReached>() => {
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

  async fn execute_impl(self) -> Result<BuildOutput> {
    if self.args.preview {
      anyhow::bail!(
        "ARKDOWN_UNSUPPORTED_APP_PREVIEW: project APP packaging is not a previewer-resource task"
      );
    }

    let mut lifecycle = BuildTaskLifecycle::new(
      self.task_observer.clone(),
      self.args.native_task_roots.clone(),
    );
    let project_name = self.project_name()?;
    lifecycle.begin_cycle().await?;

    lifecycle.marker(&project_name, None, "PreBuildApp").await?;
    if lifecycle
      .before(&project_name, None, "DuplicateDependencyCheck")
      .await?
    {
      if self
        .config
        .product
        .build_option
        .strict_mode
        .duplicate_dependency_check
      {
        DuplicateDependencyValidator::new(&self.session.project).validate()?;
      }
      lifecycle
        .after(&project_name, None, "DuplicateDependencyCheck")
        .await?;
    }

    // HSP first ensures a local shared package is available before an entry
    // module that depends on it is assembled. Hvigor's DAG establishes the
    // same ordering through module dependency edges.
    let mut child_artifacts = self.execute_child(OutputTarget::Hsp, "hsp").await?;
    child_artifacts.extend(self.execute_child(OutputTarget::Hap, "hap").await?);
    let mut app_artifacts = self.local_app_artifacts(child_artifacts)?;
    app_artifacts.extend(self.remote_app_artifacts().await?);

    let output_layout = AppProjectOutputLayout::new(
      &self.config.project_root,
      &project_name,
      &self.config.product.name,
      self.config.product.artifact_name.as_deref(),
    );
    fs_err::create_dir_all(output_layout.output_dir())?;

    let selected_package_file_names = if lifecycle
      .before(&project_name, None, "MakeProjectPackInfo")
      .await?
    {
      let artifacts = app_artifacts.clone();
      let pack_info_path = output_layout.pack_info();
      let align_device_types = self.args.align_device_types.clone();
      let output = tokio::task::spawn_blocking(move || {
        ProjectPackInfoBuilder::new(&artifacts)
          .with_aligned_device_types(&align_device_types)
          .write(&pack_info_path)
      })
      .await
      .context("MakeProjectPackInfo worker failed")??;
      lifecycle
        .after(&project_name, None, "MakeProjectPackInfo")
        .await?;
      output.package_file_names
    } else if self.args.align_device_types.is_empty() {
      BTreeSet::new()
    } else {
      ProjectPackInfoBuilder::read_package_file_names(&output_layout.pack_info())?
    };

    let privacy_profile = if lifecycle
      .before(&project_name, None, "ProcessProjectPrivacyProfile")
      .await?
    {
      let config = self.config.clone();
      let schema_path = config
        .sdk_config
        .pac_schema_path_for_runtime(&config.product.runtime_os);
      let output = tokio::task::spawn_blocking(move || {
        PrivacyProfileProcessor::new().process_project_privacy_profile(ProjectPrivacyProfileInput {
          project_root: &config.project_root,
          product_name: &config.product.name,
          modules: &config.modules,
          pac_schema_path: schema_path.as_deref(),
        })
      })
      .await
      .context("ProcessProjectPrivacyProfile worker failed")??;
      lifecycle
        .after(&project_name, None, "ProcessProjectPrivacyProfile")
        .await?;
      output
    } else {
      None
    };

    let package_runner = self.package_runner()?;
    let pack_res_path = if lifecycle
      .before(&project_name, None, "GeneratePackRes")
      .await?
    {
      let generated = self
        .generate_pack_res(&package_runner, &output_layout)
        .await?;
      lifecycle
        .after(&project_name, None, "GeneratePackRes")
        .await?;
      generated
    } else {
      output_layout
        .pack_res()
        .is_file()
        .then(|| output_layout.pack_res())
    };

    let unsigned_app = output_layout.unsigned_app();
    if lifecycle.before(&project_name, None, "PackageApp").await? {
      let staged = self
        .package_app(
          &package_runner,
          &output_layout,
          &app_artifacts,
          &selected_package_file_names,
          privacy_profile.as_ref(),
          pack_res_path.as_deref(),
        )
        .await?;
      if let Err(error) = self.collect_project_debug_symbols().await {
        tracing::warn!(
          error = %format!("{error:#}"),
          "packing app-symbol.zip failed"
        );
      }
      lifecycle.after(&project_name, None, "PackageApp").await?;
      drop(staged);
    } else {
    }

    if lifecycle
      .before(&project_name, None, "SignPackagesFromApp")
      .await?
    {
      self
        .sign_packages_from_app(&package_runner, &output_layout)
        .await?;
      lifecycle
        .after(&project_name, None, "SignPackagesFromApp")
        .await?;
    }

    let sign_enabled = lifecycle.before(&project_name, None, "SignApp").await?;
    let final_apps = if sign_enabled {
      let outputs = self.sign_apps(&output_layout).await?;
      lifecycle.after(&project_name, None, "SignApp").await?;
      outputs
    } else {
      vec![unsigned_app]
    };

    lifecycle.marker(&project_name, None, "assembleApp").await?;
    lifecycle.finish_cycle().await?;
    Ok(BuildOutput {
      artifact_paths: final_apps,
    })
  }

  async fn execute_child(&self, output_target: OutputTarget, target: &str) -> Result<Vec<PathBuf>> {
    let selected_module = self.args.module.as_deref();
    let has_module = self.config.modules.iter().any(|module| {
      module.module_type.matches_output_target(&output_target)
        && selected_module
          .map(|selected| module.name == selected)
          .unwrap_or(true)
    });
    if !has_module {
      return Ok(Vec::new());
    }

    let mut config = self.config.as_ref().clone();
    config.output_target = output_target;
    let config = Arc::new(config);
    let mut args = self.args.clone();
    args.target = target.to_string();
    args.watch = false;
    args.preview = false;
    let mut coordinator = BuildCoordinator::new(self.session.clone(), config, args)
      .with_package_plugins(self.package_plugins.clone())
      .within_task_cycle();
    if let Some(observer) = self.task_observer.clone() {
      coordinator = coordinator.with_task_observer(observer);
    }
    Ok(coordinator.execute().await?.artifact_paths)
  }

  fn local_app_artifacts(&self, paths: Vec<PathBuf>) -> Result<Vec<AppArtifactInput>> {
    let mut artifacts = Vec::with_capacity(paths.len());
    for path in paths {
      let unsigned_path = self.unsigned_module_artifact(&path);
      let module = self
        .config
        .modules
        .iter()
        .find(|module| unsigned_path.starts_with(&module.src_path))
        .with_context(|| {
          format!(
            "APP artifact {} is outside every configured module root",
            unsigned_path.display()
          )
        })?;
      if self.args.fast_build_app {
        let target_name = unsigned_path
          .file_name()
          .and_then(OsStr::to_str)
          .context("fast APP artifact directory has no UTF-8 target name")?;
        let kind = if module.module_type.matches_output_target(&OutputTarget::Hsp) {
          AppModulePackageKind::Hsp
        } else {
          AppModulePackageKind::Hap
        };
        let destination = ModuleBuildLayout::for_target_product(
          &module.src_path,
          &self.config.product.name,
          target_name,
        )
        .outputs_root()
        .join(target_name)
        .join("app")
        .join(&module.name);
        artifacts.push(AppArtifactInput::local_fast_directory(
          unsigned_path,
          module.name.clone(),
          kind,
          destination,
        ));
      } else {
        artifacts.push(AppArtifactInput::local(unsigned_path, module.name.clone()));
      }
    }
    artifacts.sort_by_key(|artifact| match &artifact.role {
      arkdown_pack::AppArtifactRole::Local { module_name } => self
        .config
        .modules
        .iter()
        .position(|module| module.name == *module_name)
        .unwrap_or(usize::MAX),
      arkdown_pack::AppArtifactRole::RemoteHsp => usize::MAX,
    });
    Ok(artifacts)
  }

  async fn remote_app_artifacts(&self) -> Result<Vec<AppArtifactInput>> {
    let scopes = self
      .config
      .modules
      .iter()
      .filter_map(|module| {
        let context = self.session.module_context(&module.name)?;
        Some(RemoteHspDependencyScope::new(
          module.src_path.clone(),
          context
            .deps
            .nodes
            .values()
            .map(|node| node.package_name.clone()),
        ))
      })
      .collect::<Vec<_>>();
    let project_root = self.config.project_root.clone();
    let bundle_name = self.config.product.bundle_name.clone();
    let version_code = self.session.project.version_code;
    let build_version = self.session.project.build_version.clone();
    let integrated_output = self
      .config
      .project_root
      .join("build")
      .join("cache")
      .join(&self.config.product.name)
      .join("integrated_hsp");
    tokio::task::spawn_blocking(move || {
      RemoteHspCollector::new(&project_root, &scopes).collect_for_app(
        &bundle_name,
        version_code,
        build_version.as_deref(),
        &integrated_output,
      )
    })
    .await
    .context("remote HSP collection worker failed")?
    .map_err(anyhow::Error::from)
  }

  fn unsigned_module_artifact(&self, artifact: &Path) -> PathBuf {
    let Some(file_name) = artifact.file_name().and_then(OsStr::to_str) else {
      return artifact.to_path_buf();
    };
    let Some(extension) = artifact.extension().and_then(OsStr::to_str) else {
      return artifact.to_path_buf();
    };
    let signed_suffix = format!("-signed.{extension}");
    let Some(stem) = file_name.strip_suffix(&signed_suffix) else {
      return artifact.to_path_buf();
    };
    let unsigned = artifact.with_file_name(format!("{stem}-unsigned.{extension}"));
    unsigned
      .is_file()
      .then_some(unsigned)
      .unwrap_or_else(|| artifact.to_path_buf())
  }

  fn package_runner(&self) -> Result<AppPackageRunner> {
    let package_tool = self
      .session
      .sdk
      .tool_path(SdkToolKind::PackageTool)
      .context("selected SDK does not expose app_packing_tool.jar")?;
    Ok(AppPackageRunner::new(package_tool.path))
  }

  async fn generate_pack_res(
    &self,
    runner: &AppPackageRunner,
    output_layout: &AppProjectOutputLayout<'_>,
  ) -> Result<Option<PathBuf>> {
    let entry_card = self.config.project_root.join("EntryCard");
    let output = output_layout.pack_res();
    if !entry_card.is_dir() {
      if output.is_file() {
        fs_err::remove_file(&output)?;
      }
      return Ok(None);
    }
    let runner = runner.clone();
    let pack_info = output_layout.pack_info();
    let worker_output = output.clone();
    tokio::task::spawn_blocking(move || {
      runner.generate_pack_res(&entry_card, &pack_info, &worker_output)
    })
    .await
    .context("GeneratePackRes worker failed")??;
    Ok(Some(output))
  }

  async fn package_app(
    &self,
    runner: &AppPackageRunner,
    output_layout: &AppProjectOutputLayout<'_>,
    artifacts: &[AppArtifactInput],
    selected_package_file_names: &BTreeSet<String>,
    privacy_profile: Option<&ProjectPrivacyProfileOutput>,
    pack_res_path: Option<&Path>,
  ) -> Result<StagedAppArtifacts> {
    let artifacts = artifacts.to_vec();
    let staged = tokio::task::spawn_blocking(move || AppArtifactStager::new(&artifacts).stage())
      .await
      .context("PackageApp staging worker failed")??;
    let (hap_paths, hsp_paths) = self
      .normalize_app_artifacts(runner, output_layout, &staged, selected_package_file_names)
      .await?;
    let mut plan = AppPackagePlan::new(
      output_layout.pack_info(),
      hap_paths,
      hsp_paths,
      output_layout.unsigned_app(),
    );
    if self.args.fast_build_app {
      plan.mode = AppPackageMode::FastApp;
    }
    plan.pac_json_path = privacy_profile.map(|output| output.processed_path.clone());
    plan.pack_res_path = pack_res_path.map(Path::to_path_buf);
    plan.compress_level = self.app_compress_level();
    let runner = runner.clone();
    tokio::task::spawn_blocking(move || runner.pack(&plan))
      .await
      .context("PackageApp worker failed")??;
    Ok(staged)
  }

  async fn normalize_app_artifacts(
    &self,
    runner: &AppPackageRunner,
    output_layout: &AppProjectOutputLayout<'_>,
    staged: &StagedAppArtifacts,
    selected_package_file_names: &BTreeSet<String>,
  ) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    if self.args.align_device_types.is_empty() {
      return Ok((staged.hap_paths.clone(), staged.hsp_paths.clone()));
    }
    let input_paths = staged
      .hap_paths
      .iter()
      .chain(&staged.hsp_paths)
      .filter(|path| {
        matches!(
          path.extension().and_then(OsStr::to_str),
          Some("hap" | "hsp")
        )
      })
      .cloned()
      .collect::<Vec<_>>();
    if input_paths.is_empty() {
      return Ok((staged.hap_paths.clone(), staged.hsp_paths.clone()));
    }
    let plan = AppNormalizePlan::new(
      input_paths,
      self.args.align_device_types.clone(),
      output_layout.normalized_packages(),
    );
    let runner = runner.clone();
    tokio::task::spawn_blocking(move || runner.normalize_device_types(&plan))
      .await
      .context("PackageApp generalNormalize worker failed")??;

    let select = |paths: &[PathBuf]| {
      paths
        .iter()
        .filter_map(|path| {
          if !matches!(
            path.extension().and_then(OsStr::to_str),
            Some("hap" | "hsp")
          ) {
            return Some(path.clone());
          }
          let file_name = path.file_name()?;
          selected_package_file_names
            .contains(file_name.to_str()?)
            .then(|| output_layout.normalized_packages().join(file_name))
        })
        .collect::<Vec<_>>()
    };
    Ok((select(&staged.hap_paths), select(&staged.hsp_paths)))
  }

  async fn collect_project_debug_symbols(&self) -> Result<()> {
    let collector = ProjectDebugSymbolCollector::new(
      self.config.project_root.clone(),
      self.config.product.name.clone(),
      self
        .args
        .build_target_name
        .clone()
        .unwrap_or_else(|| "default".to_string()),
      self.config.build_mode.clone(),
      self.config.modules.clone(),
      self.args.collect_debug_symbol,
    );
    tokio::task::spawn_blocking(move || collector.collect())
      .await
      .context("project debug-symbol worker failed")??;
    Ok(())
  }

  async fn sign_packages_from_app(
    &self,
    runner: &AppPackageRunner,
    output_layout: &AppProjectOutputLayout<'_>,
  ) -> Result<()> {
    for stale in [
      output_layout.all_unsigned_app(),
      output_layout.all_signed_app(),
    ] {
      if stale.is_file() {
        fs_err::remove_file(stale)?;
      }
    }
    if !self.args.enable_sign_task {
      return Ok(());
    }
    let Some(signing_config) = self.config.product.signing_config.clone() else {
      return Ok(());
    };
    if !self.config.product.build_option.app_with_signed_pkg {
      return Ok(());
    }

    let unsigned_app = output_layout.unsigned_app();
    let worker_app = unsigned_app.clone();
    let unpacked =
      tokio::task::spawn_blocking(move || AppArchiveUnpacker::new(&worker_app).unpack())
        .await
        .context("SignPackagesFromApp unpack worker failed")??;

    let signed = self
      .sign_unpacked_packages(&unpacked, &signing_config)
      .await;
    let result = match signed {
      Ok((hap_paths, hsp_paths)) => {
        let mut plan = AppPackagePlan::new(
          output_layout.pack_info(),
          hap_paths,
          hsp_paths,
          output_layout.all_unsigned_app(),
        );
        plan.pac_json_path = output_layout
          .pac_json()
          .is_file()
          .then(|| output_layout.pac_json());
        plan.pack_res_path = output_layout
          .pack_res()
          .is_file()
          .then(|| output_layout.pack_res());
        plan.replace_pack_info = Some(false);
        plan.compress_level = self.app_compress_level();
        let runner = runner.clone();
        tokio::task::spawn_blocking(move || runner.pack(&plan))
          .await
          .context("SignPackagesFromApp pack worker failed")?
          .map_err(anyhow::Error::from)
      }
      Err(error) => Err(error),
    };

    let cleanup_app = unsigned_app;
    let cleanup_unpacked = unpacked;
    let cleanup = tokio::task::spawn_blocking(move || {
      AppArchiveUnpacker::new(&cleanup_app).cleanup(&cleanup_unpacked)
    })
    .await
    .context("SignPackagesFromApp cleanup worker failed")?;
    result?;
    cleanup?;
    Ok(())
  }

  async fn sign_unpacked_packages(
    &self,
    unpacked: &UnpackedAppArtifacts,
    signing_config: &arkdown_core::SigningConfig,
  ) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let inputs = unpacked
      .hap_paths
      .iter()
      .chain(&unpacked.hsp_paths)
      .cloned()
      .collect::<Vec<_>>();
    let signed_dir = unpacked.root_path.join("signed");
    let config = signing_config.clone();
    let signed = tokio::task::spawn_blocking(move || {
      fs_err::create_dir_all(&signed_dir)?;
      inputs
        .iter()
        .map(|input| {
          let file_name = input
            .file_name()
            .context("embedded APP package has no filename")?;
          let output = signed_dir.join(file_name);
          SignBackendRunner::new().sign_or_skip(input, &output, Some(&config))?;
          Ok(output)
        })
        .collect::<Result<Vec<_>>>()
    })
    .await
    .context("SignPackagesFromApp signing worker failed")??;

    let hap_count = unpacked.hap_paths.len();
    Ok((signed[..hap_count].to_vec(), signed[hap_count..].to_vec()))
  }

  async fn sign_apps(&self, output_layout: &AppProjectOutputLayout<'_>) -> Result<Vec<PathBuf>> {
    let unsigned = output_layout.unsigned_app();
    if !unsigned.is_file() {
      anyhow::bail!("PackageApp output is missing: {}", unsigned.display());
    }
    for stale in [output_layout.signed_app(), output_layout.all_signed_app()] {
      if stale.is_file() {
        fs_err::remove_file(stale)?;
      }
    }
    if !self.args.enable_sign_task {
      return Ok(vec![unsigned]);
    }
    let Some(signing_config) = self.config.product.signing_config.clone() else {
      return Ok(vec![unsigned]);
    };
    let signed = output_layout.signed_app();
    let worker_unsigned = unsigned.clone();
    let worker_signed = signed.clone();
    let normal_signing_config = signing_config.clone();
    tokio::task::spawn_blocking(move || {
      SignBackendRunner::new().sign_or_skip(
        &worker_unsigned,
        &worker_signed,
        Some(&normal_signing_config),
      )
    })
    .await
    .context("SignApp worker failed")??;
    let mut outputs = vec![signed];
    if self.config.product.build_option.app_with_signed_pkg {
      let all_unsigned = output_layout.all_unsigned_app();
      if !all_unsigned.is_file() {
        anyhow::bail!(
          "SignPackagesFromApp output is missing: {}",
          all_unsigned.display()
        );
      }
      let all_signed = output_layout.all_signed_app();
      let worker_config = signing_config;
      let worker_unsigned = all_unsigned;
      let worker_signed = all_signed.clone();
      tokio::task::spawn_blocking(move || {
        SignBackendRunner::new().sign_or_skip(
          &worker_unsigned,
          &worker_signed,
          Some(&worker_config),
        )
      })
      .await
      .context("SignApp all-package worker failed")??;
      outputs.push(all_signed);
    }
    Ok(outputs)
  }

  fn project_name(&self) -> Result<String> {
    self
      .config
      .project_root
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .context("project root has no project name")
  }

  fn app_compress_level(&self) -> Option<String> {
    self
      .args
      .pack_compress_level
      .filter(|_| self.config.sdk_config.sdk_component_api_version >= 12)
      .map(|level| level.to_string())
  }
}

struct AppProjectOutputLayout<'a> {
  project_root: &'a Path,
  project_name: &'a str,
  product_name: &'a str,
  artifact_name: Option<&'a str>,
}

impl<'a> AppProjectOutputLayout<'a> {
  fn new(
    project_root: &'a Path,
    project_name: &'a str,
    product_name: &'a str,
    artifact_name: Option<&'a str>,
  ) -> Self {
    Self {
      project_root,
      project_name,
      product_name,
      artifact_name,
    }
  }

  fn output_dir(&self) -> PathBuf {
    self
      .project_root
      .join("build")
      .join("outputs")
      .join(self.product_name)
  }

  fn pack_info(&self) -> PathBuf {
    self.output_dir().join("pack.info")
  }

  fn pack_res(&self) -> PathBuf {
    self.output_dir().join("pack.res")
  }

  fn pac_json(&self) -> PathBuf {
    self.output_dir().join("pac.json")
  }

  fn normalized_packages(&self) -> PathBuf {
    self.output_dir().join("package_app")
  }

  fn unsigned_app(&self) -> PathBuf {
    let name = self.artifact_name.map_or_else(
      || format!("{}-{}-unsigned.app", self.project_name, self.product_name),
      |artifact_name| format!("{artifact_name}-unsigned.app"),
    );
    self.output_dir().join(name)
  }

  fn signed_app(&self) -> PathBuf {
    let name = self.artifact_name.map_or_else(
      || format!("{}-{}-signed.app", self.project_name, self.product_name),
      |artifact_name| format!("{artifact_name}.app"),
    );
    self.output_dir().join(name)
  }

  fn all_unsigned_app(&self) -> PathBuf {
    let name = self.artifact_name.map_or_else(
      || {
        format!(
          "{}-{}-all-unsigned.app",
          self.project_name, self.product_name
        )
      },
      |artifact_name| format!("{artifact_name}-all-unsigned.app"),
    );
    self.output_dir().join(name)
  }

  fn all_signed_app(&self) -> PathBuf {
    let name = self.artifact_name.map_or_else(
      || format!("{}-{}-all-signed.app", self.project_name, self.product_name),
      |artifact_name| format!("{artifact_name}-all.app"),
    );
    self.output_dir().join(name)
  }
}
