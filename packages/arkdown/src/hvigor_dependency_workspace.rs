//! Hvigor plugin dependency workspace preparation.
//!
//! Parity owners: `hvigorw.js::initProjectWorkSpace`,
//! `offlinePluginConversion`, and `isHvigorDependencyUseNpm` in the
//! @ohos/hvigor 6.24.3 wrapper shipped with DevEco Studio 6.1.1.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use arkdown_path::lexical_normalize;
use arkdown_session::BuildScriptProjectCacheIdentity;
use indexmap::IndexMap;
use oxc_resolver::{ResolveOptions, Resolver};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const HVIGOR_DEPENDENCY_USE_NPM: &str = "hvigor.dependency.useNpm";
const MINIMUM_PNPM_NODE_VERSION: &str = "16.0.0";
const PNPM_VERSION: &str = "10.28.2";

static WORKSPACE_INSTALL_LOCKS: LazyLock<Mutex<BTreeMap<PathBuf, Weak<Mutex<()>>>>> =
  LazyLock::new(|| Mutex::new(BTreeMap::new()));

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorPluginWorkspaceRequest {
  project_root: PathBuf,
  hvigor_user_home: PathBuf,
  home_dir: PathBuf,
  process_executable: PathBuf,
  node_version: String,
  #[serde(default)]
  project_config: Value,
  #[serde(default)]
  user_config: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorPluginWorkspaceDiagnostic {
  code: &'static str,
  message: String,
  #[serde(flatten)]
  details: IndexMap<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorPluginWorkspaceResponse {
  #[serde(skip_serializing_if = "Option::is_none")]
  workspace: Option<PathBuf>,
  #[serde(skip_serializing_if = "Option::is_none")]
  node_modules: Option<PathBuf>,
  installed: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  package_manager: Option<HvigorPackageManagerKind>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<HvigorPluginWorkspaceDiagnostic>,
}

#[derive(Debug)]
enum HvigorPluginWorkspaceError {
  InvalidProjectRoot(String),
  InvalidDependency {
    name: String,
    value: Value,
  },
  InvalidDependencyPath {
    name: String,
    path: PathBuf,
  },
  WorkspaceLock,
  CreateDirectory {
    path: PathBuf,
    message: String,
  },
  ReadFile {
    path: PathBuf,
    message: String,
  },
  WriteFile {
    path: PathBuf,
    message: String,
  },
  RemoveFile {
    path: PathBuf,
    message: String,
  },
  SerializeManifest(String),
  InvalidNodeVersion(String),
  PackageManagerStart {
    path: PathBuf,
    message: String,
  },
  PackageManagerFailed {
    path: PathBuf,
    status: Option<i32>,
  },
  PnpmBootstrapMissing {
    path: PathBuf,
  },
  InstalledPackageUnresolved {
    name: String,
    workspace: PathBuf,
  },
  Cleanup {
    workspace: PathBuf,
    install_message: String,
    cleanup_message: String,
  },
}

impl HvigorPluginWorkspaceError {
  fn diagnostic(self) -> HvigorPluginWorkspaceDiagnostic {
    match self {
      Self::InvalidProjectRoot(message) => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_INVALID_PROJECT_ROOT",
        message,
        details: IndexMap::new(),
      },
      Self::InvalidDependency { name, value } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_DEPENDENCY_INVALID",
        message: format!(
          "hvigor dependency '{name}' must use a string package version or local package path"
        ),
        details: IndexMap::from([
          ("dependency".to_string(), Value::String(name)),
          ("value".to_string(), value),
        ]),
      },
      Self::InvalidDependencyPath { name, path } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_DEPENDENCY_PATH_INVALID",
        message: format!(
          "hvigor dependency '{name}' resolves to a non-Unicode package path {}",
          path.display()
        ),
        details: IndexMap::from([
          ("dependency".to_string(), Value::String(name)),
          ("path".to_string(), Self::path_value(&path)),
        ]),
      },
      Self::WorkspaceLock => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_LOCK_FAILED",
        message: "failed to acquire the hvigor plugin workspace installation lock".to_string(),
        details: IndexMap::new(),
      },
      Self::CreateDirectory { path, message } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_CREATE_FAILED",
        message: format!(
          "failed to create hvigor plugin workspace directory {}: {message}",
          path.display()
        ),
        details: IndexMap::from([("path".to_string(), Self::path_value(&path))]),
      },
      Self::ReadFile { path, message } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_READ_FAILED",
        message: format!(
          "failed to read hvigor plugin workspace file {}: {message}",
          path.display()
        ),
        details: IndexMap::from([("path".to_string(), Self::path_value(&path))]),
      },
      Self::WriteFile { path, message } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_WRITE_FAILED",
        message: format!(
          "failed to write hvigor plugin workspace file {}: {message}",
          path.display()
        ),
        details: IndexMap::from([("path".to_string(), Self::path_value(&path))]),
      },
      Self::RemoveFile { path, message } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_REMOVE_FAILED",
        message: format!(
          "failed to remove stale hvigor plugin workspace file {}: {message}",
          path.display()
        ),
        details: IndexMap::from([("path".to_string(), Self::path_value(&path))]),
      },
      Self::SerializeManifest(message) => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_MANIFEST_INVALID",
        message: format!("failed to serialize hvigor plugin dependencies: {message}"),
        details: IndexMap::new(),
      },
      Self::InvalidNodeVersion(version) => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_NODE_VERSION_INVALID",
        message: format!("invalid Node.js version '{version}' for hvigor dependency installation"),
        details: IndexMap::from([("nodeVersion".to_string(), Value::String(version))]),
      },
      Self::PackageManagerStart { path, message } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_DEPENDENCY_INSTALL_START_FAILED",
        message: format!(
          "failed to start hvigor plugin dependency manager {}: {message}",
          path.display()
        ),
        details: IndexMap::from([("packageManagerPath".to_string(), Self::path_value(&path))]),
      },
      Self::PackageManagerFailed { path, status } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_DEPENDENCY_INSTALL_FAILED",
        message: format!(
          "hvigor plugin dependency manager {} failed{}",
          path.display(),
          status.map_or_else(String::new, |status| format!(" with status {status}"))
        ),
        details: IndexMap::from([
          ("packageManagerPath".to_string(), Self::path_value(&path)),
          (
            "status".to_string(),
            status.map_or(Value::Null, |status| Value::Number(status.into())),
          ),
        ]),
      },
      Self::PnpmBootstrapMissing { path } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_PNPM_BOOTSTRAP_FAILED",
        message: format!(
          "pnpm installation completed without creating the expected executable {}",
          path.display()
        ),
        details: IndexMap::from([("pnpmPath".to_string(), Self::path_value(&path))]),
      },
      Self::InstalledPackageUnresolved { name, workspace } => {
        HvigorPluginWorkspaceDiagnostic {
          code: "HVIGOR_PLUGIN_DEPENDENCY_UNRESOLVED",
          message: format!(
            "installed hvigor dependency '{name}' cannot be resolved from {}",
            workspace.display()
          ),
          details: IndexMap::from([
            ("dependency".to_string(), Value::String(name)),
            ("workspace".to_string(), Self::path_value(&workspace)),
          ]),
        }
      }
      Self::Cleanup {
        workspace,
        install_message,
        cleanup_message,
      } => HvigorPluginWorkspaceDiagnostic {
        code: "HVIGOR_PLUGIN_WORKSPACE_CLEANUP_FAILED",
        message: format!(
          "hvigor plugin dependency installation failed ({install_message}) and the partial workspace could not be removed: {cleanup_message}"
        ),
        details: IndexMap::from([("workspace".to_string(), Self::path_value(&workspace))]),
      },
    }
  }

  fn path_value(path: &Path) -> Value {
    Value::String(path.to_string_lossy().into_owned())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum HvigorPackageManagerKind {
  Npm,
  Pnpm,
}

#[derive(Debug)]
struct HvigorPackageManager {
  kind: HvigorPackageManagerKind,
  executable: PathBuf,
}

#[derive(Debug, Serialize)]
struct HvigorWorkspaceManifest<'a> {
  dependencies: &'a BTreeMap<String, String>,
}

#[derive(Debug)]
pub(crate) struct HvigorPluginWorkspacePlan {
  project_root: PathBuf,
  hvigor_user_home: PathBuf,
  home_dir: PathBuf,
  process_executable: PathBuf,
  workspace: PathBuf,
  dependencies: BTreeMap<String, String>,
  use_npm: bool,
}

impl HvigorPluginWorkspacePlan {
  fn from_request(
    request: &HvigorPluginWorkspaceRequest,
  ) -> Result<Self, HvigorPluginWorkspaceError> {
    let identity = BuildScriptProjectCacheIdentity::new(&request.project_root)
      .map_err(|error| HvigorPluginWorkspaceError::InvalidProjectRoot(error.to_string()))?;
    let project_root = identity.project_root().to_path_buf();
    let workspace = identity.workspace(&request.hvigor_user_home);
    let dependencies = Self::dependencies(&project_root, &request.project_config)?;
    let node_version_text = request.node_version.trim_start_matches('v');
    let node_version = Version::parse(node_version_text)
      .map_err(|_| HvigorPluginWorkspaceError::InvalidNodeVersion(request.node_version.clone()))?;
    let minimum_pnpm_node_version =
      Version::parse(MINIMUM_PNPM_NODE_VERSION).expect("valid minimum Node.js version");
    let use_npm = Self::configured_use_npm(&request.project_config, &request.user_config)
      || node_version < minimum_pnpm_node_version;
    Ok(Self {
      project_root,
      hvigor_user_home: lexical_normalize(&request.hvigor_user_home),
      home_dir: lexical_normalize(&request.home_dir),
      process_executable: lexical_normalize(&request.process_executable),
      workspace,
      dependencies,
      use_npm,
    })
  }

  fn dependencies(
    project_root: &Path,
    project_config: &Value,
  ) -> Result<BTreeMap<String, String>, HvigorPluginWorkspaceError> {
    let Some(dependencies) = project_config.get("dependencies") else {
      return Ok(BTreeMap::new());
    };
    if dependencies.is_null() {
      return Ok(BTreeMap::new());
    }
    let Some(dependencies) = dependencies.as_object() else {
      return Err(HvigorPluginWorkspaceError::InvalidDependency {
        name: "dependencies".to_string(),
        value: dependencies.clone(),
      });
    };
    let mut normalized = BTreeMap::new();
    for (name, value) in dependencies {
      let Some(specification) = value.as_str() else {
        return Err(HvigorPluginWorkspaceError::InvalidDependency {
          name: name.clone(),
          value: value.clone(),
        });
      };
      normalized.insert(
        name.clone(),
        Self::normalize_dependency(project_root, name, specification)?,
      );
    }
    Ok(normalized)
  }

  fn normalize_dependency(
    project_root: &Path,
    name: &str,
    specification: &str,
  ) -> Result<String, HvigorPluginWorkspaceError> {
    if !specification.starts_with("file:") && !specification.ends_with(".tgz") {
      return Ok(specification.to_string());
    }
    let relative = specification.strip_prefix("file:").unwrap_or(specification);
    let package_path = lexical_normalize(&project_root.join("hvigor").join(relative));
    package_path.to_str().map(ToOwned::to_owned).ok_or_else(|| {
      HvigorPluginWorkspaceError::InvalidDependencyPath {
        name: name.to_string(),
        path: package_path,
      }
    })
  }

  fn configured_use_npm(project_config: &Value, user_config: &Value) -> bool {
    let properties = project_config
      .get("properties")
      .filter(|value| !value.is_null())
      .or_else(|| {
        user_config
          .get("properties")
          .filter(|value| !value.is_null())
      });
    properties
      .and_then(|properties| properties.get(HVIGOR_DEPENDENCY_USE_NPM))
      .is_some_and(Self::javascript_truthy)
  }

  fn javascript_truthy(value: &Value) -> bool {
    match value {
      Value::Null => false,
      Value::Bool(value) => *value,
      Value::Number(value) => value
        .as_f64()
        .is_some_and(|value| value != 0.0 && !value.is_nan()),
      Value::String(value) => !value.is_empty(),
      Value::Array(_) | Value::Object(_) => true,
    }
  }

  fn node_modules(&self) -> PathBuf {
    self.workspace.join("node_modules")
  }

  fn package_manager_kind(&self) -> HvigorPackageManagerKind {
    if self.use_npm {
      HvigorPackageManagerKind::Npm
    } else {
      HvigorPackageManagerKind::Pnpm
    }
  }

  fn npm_executable(&self) -> PathBuf {
    let executable = if cfg!(windows) { "npm.cmd" } else { "npm" };
    self
      .process_executable
      .parent()
      .unwrap_or_else(|| Path::new(""))
      .join(executable)
  }

  fn wrapper_tools(&self) -> PathBuf {
    self.hvigor_user_home.join("wrapper").join("tools")
  }

  fn pnpm_executable(&self) -> PathBuf {
    let executable = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    self
      .wrapper_tools()
      .join("node_modules")
      .join(".bin")
      .join(executable)
  }
}

pub(crate) struct HvigorPluginWorkspaceRunner {
  request: HvigorPluginWorkspaceRequest,
}

impl HvigorPluginWorkspaceRunner {
  pub(crate) fn from_json(request_json: &str) -> anyhow::Result<Self> {
    Ok(Self {
      request: serde_json::from_str(request_json)?,
    })
  }

  pub(crate) async fn response_json(self) -> anyhow::Result<String> {
    let response = tokio::task::spawn_blocking(move || match self.execute() {
      Ok((plan, installed)) => HvigorPluginWorkspaceResponse {
        workspace: Some(plan.workspace.clone()),
        node_modules: Some(plan.node_modules()),
        installed,
        package_manager: Some(plan.package_manager_kind()),
        error: None,
      },
      Err(error) => HvigorPluginWorkspaceResponse {
        workspace: None,
        node_modules: None,
        installed: false,
        package_manager: None,
        error: Some(error.diagnostic()),
      },
    })
    .await?;
    serde_json::to_string(&response).map_err(Into::into)
  }

  fn execute(self) -> Result<(HvigorPluginWorkspacePlan, bool), HvigorPluginWorkspaceError> {
    let plan = HvigorPluginWorkspacePlan::from_request(&self.request)?;
    let install_lock = Self::workspace_install_lock(&plan.workspace)?;
    let _guard = install_lock
      .lock()
      .map_err(|_| HvigorPluginWorkspaceError::WorkspaceLock)?;
    if plan.dependencies.is_empty() || self.workspace_is_current(&plan)? {
      return Ok((plan, false));
    }
    let install_result = self.install(&plan);
    match install_result {
      Ok(()) => Ok((plan, true)),
      Err(error) => {
        let install_message = format!("{error:?}");
        match std::fs::remove_dir_all(&plan.workspace) {
          Ok(()) => Err(error),
          Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => Err(error),
          Err(cleanup_error) => Err(HvigorPluginWorkspaceError::Cleanup {
            workspace: plan.workspace.clone(),
            install_message,
            cleanup_message: cleanup_error.to_string(),
          }),
        }
      }
    }
  }

  fn workspace_install_lock(
    workspace: &Path,
  ) -> Result<Arc<Mutex<()>>, HvigorPluginWorkspaceError> {
    let mut locks = WORKSPACE_INSTALL_LOCKS
      .lock()
      .map_err(|_| HvigorPluginWorkspaceError::WorkspaceLock)?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(workspace).and_then(Weak::upgrade) {
      return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(workspace.to_path_buf(), Arc::downgrade(&lock));
    Ok(lock)
  }

  fn workspace_is_current(
    &self,
    plan: &HvigorPluginWorkspacePlan,
  ) -> Result<bool, HvigorPluginWorkspaceError> {
    let manifest_path = plan.workspace.join("package.json");
    let source = match std::fs::read_to_string(&manifest_path) {
      Ok(source) => source,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
      Err(error) => {
        return Err(HvigorPluginWorkspaceError::ReadFile {
          path: manifest_path,
          message: error.to_string(),
        });
      }
    };
    let manifest = match serde_json::from_str::<Value>(&source) {
      Ok(manifest) => manifest,
      Err(_) => return Ok(false),
    };
    let installed_dependencies = manifest
      .get("dependencies")
      .cloned()
      .and_then(|value| serde_json::from_value::<BTreeMap<String, String>>(value).ok())
      .unwrap_or_default();
    if installed_dependencies != plan.dependencies {
      return Ok(false);
    }
    let resolver = Self::package_resolver(&plan.workspace);
    Ok(
      plan
        .dependencies
        .keys()
        .all(|name| resolver.resolve(&plan.workspace, name).is_ok()),
    )
  }

  fn package_resolver(workspace: &Path) -> Resolver {
    Resolver::new(ResolveOptions {
      cwd: Some(workspace.to_path_buf()),
      condition_names: vec!["node".to_string(), "require".to_string()],
      extensions: [".js", ".json", ".node", ".ts"]
        .into_iter()
        .map(str::to_string)
        .collect(),
      main_fields: vec!["main".to_string()],
      main_files: vec!["index".to_string()],
      modules: vec!["node_modules".to_string()],
      symlinks: true,
      ..ResolveOptions::default()
    })
  }

  fn install(&self, plan: &HvigorPluginWorkspacePlan) -> Result<(), HvigorPluginWorkspaceError> {
    self.create_directory(&plan.workspace)?;
    self.remove_stale_pnpmfile(plan)?;
    self.write_workspace_manifest(plan)?;
    let package_manager = self.package_manager(plan)?;
    let npmrc = self.prepare_npmrc(plan, package_manager.kind)?;
    let mut arguments = vec![OsString::from("install")];
    if Self::is_ci() {
      arguments.push(OsString::from("--no-frozen-lockfile"));
    }
    self.run_package_manager(
      &package_manager.executable,
      &arguments,
      &plan.workspace,
      npmrc.as_deref(),
    )?;
    let resolver = Self::package_resolver(&plan.workspace);
    if let Some(name) = plan
      .dependencies
      .keys()
      .find(|name| resolver.resolve(&plan.workspace, name).is_err())
    {
      return Err(HvigorPluginWorkspaceError::InstalledPackageUnresolved {
        name: name.clone(),
        workspace: plan.workspace.clone(),
      });
    }
    Ok(())
  }

  fn create_directory(&self, path: &Path) -> Result<(), HvigorPluginWorkspaceError> {
    std::fs::create_dir_all(path).map_err(|error| HvigorPluginWorkspaceError::CreateDirectory {
      path: path.to_path_buf(),
      message: error.to_string(),
    })
  }

  fn remove_stale_pnpmfile(
    &self,
    plan: &HvigorPluginWorkspacePlan,
  ) -> Result<(), HvigorPluginWorkspaceError> {
    let path = plan.workspace.join(".pnpmfile.js");
    match std::fs::remove_file(&path) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(HvigorPluginWorkspaceError::RemoveFile {
        path,
        message: error.to_string(),
      }),
    }
  }

  fn write_workspace_manifest(
    &self,
    plan: &HvigorPluginWorkspacePlan,
  ) -> Result<(), HvigorPluginWorkspaceError> {
    let path = plan.workspace.join("package.json");
    let source = serde_json::to_vec(&HvigorWorkspaceManifest {
      dependencies: &plan.dependencies,
    })
    .map_err(|error| HvigorPluginWorkspaceError::SerializeManifest(error.to_string()))?;
    std::fs::write(&path, source).map_err(|error| HvigorPluginWorkspaceError::WriteFile {
      path,
      message: error.to_string(),
    })
  }

  fn package_manager(
    &self,
    plan: &HvigorPluginWorkspacePlan,
  ) -> Result<HvigorPackageManager, HvigorPluginWorkspaceError> {
    if plan.use_npm {
      return Ok(HvigorPackageManager {
        kind: HvigorPackageManagerKind::Npm,
        executable: plan.npm_executable(),
      });
    }
    let executable = plan.pnpm_executable();
    if !executable.is_file()
      || !plan
        .wrapper_tools()
        .join("node_modules")
        .join("pnpm")
        .join("package.json")
        .is_file()
    {
      self.bootstrap_pnpm(plan)?;
    }
    if !executable.is_file() {
      return Err(HvigorPluginWorkspaceError::PnpmBootstrapMissing { path: executable });
    }
    Ok(HvigorPackageManager {
      kind: HvigorPackageManagerKind::Pnpm,
      executable,
    })
  }

  fn bootstrap_pnpm(
    &self,
    plan: &HvigorPluginWorkspacePlan,
  ) -> Result<(), HvigorPluginWorkspaceError> {
    let tools = plan.wrapper_tools();
    self.create_directory(&tools)?;
    let package_json = tools.join("package.json");
    let source = serde_json::to_vec(&serde_json::json!({
      "dependencies": { "pnpm": PNPM_VERSION }
    }))
    .map_err(|error| HvigorPluginWorkspaceError::SerializeManifest(error.to_string()))?;
    std::fs::write(&package_json, source).map_err(|error| {
      HvigorPluginWorkspaceError::WriteFile {
        path: package_json,
        message: error.to_string(),
      }
    })?;
    self.run_package_manager(
      &plan.npm_executable(),
      &[OsString::from("install"), OsString::from("pnpm")],
      &tools,
      None,
    )
  }

  fn prepare_npmrc(
    &self,
    plan: &HvigorPluginWorkspacePlan,
    package_manager: HvigorPackageManagerKind,
  ) -> Result<Option<PathBuf>, HvigorPluginWorkspaceError> {
    let workspace_npmrc = plan.workspace.join(".npmrc");
    let project_npmrc = plan.project_root.join(".npmrc");
    let mut use_workspace_npmrc = false;
    if project_npmrc.is_file() {
      let mut source = String::new();
      let home_npmrc = plan.home_dir.join(".npmrc");
      if home_npmrc.is_file() {
        source.push_str(&self.read_text(&home_npmrc)?);
      }
      if !source.is_empty() && !source.ends_with('\n') {
        source.push('\n');
      }
      source.push_str(&self.read_text(&project_npmrc)?);
      self.write_text(&workspace_npmrc, &source)?;
      use_workspace_npmrc = true;
    }
    if package_manager == HvigorPackageManagerKind::Pnpm {
      let parent_npmrc = plan.workspace.parent().map(|parent| parent.join(".npmrc"));
      if let Some(parent_npmrc) = parent_npmrc {
        self.remove_optional_file(&parent_npmrc)?;
      }
      let mut source = if workspace_npmrc.is_file() {
        self.read_text(&workspace_npmrc)?
      } else {
        String::new()
      };
      if !source
        .lines()
        .any(|line| line.trim_start().starts_with("store-dir"))
      {
        if !source.is_empty() && !source.ends_with('\n') {
          source.push('\n');
        }
        source.push_str("store-dir=");
        source.push_str(&self.path_text(&plan.hvigor_user_home.join("caches"))?);
        source.push('\n');
        self.write_text(&workspace_npmrc, &source)?;
      }
      self.remove_incompatible_pnpm_modules(plan, &source)?;
      use_workspace_npmrc = true;
    }
    Ok(use_workspace_npmrc.then_some(workspace_npmrc))
  }

  fn remove_incompatible_pnpm_modules(
    &self,
    plan: &HvigorPluginWorkspacePlan,
    npmrc: &str,
  ) -> Result<(), HvigorPluginWorkspaceError> {
    let modules_yaml = plan.node_modules().join(".modules.yaml");
    if !modules_yaml.is_file() {
      return Ok(());
    }
    let modules = self.read_text(&modules_yaml)?;
    let installed_store = modules.lines().find_map(|line| {
      line
        .trim_start()
        .strip_prefix("storeDir:")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    });
    let configured_store = npmrc.lines().find_map(|line| {
      line
        .trim_start()
        .strip_prefix("store-dir")
        .and_then(|value| value.trim_start().strip_prefix('='))
        .map(str::trim)
        .filter(|value| !value.is_empty())
    });
    if let (Some(installed_store), Some(configured_store)) = (installed_store, configured_store) {
      let installed_path = PathBuf::from(installed_store);
      let installed_root = lexical_normalize(
        installed_path
          .parent()
          .unwrap_or_else(|| Path::new(installed_store)),
      );
      let configured_root = lexical_normalize(Path::new(configured_store));
      if installed_root != configured_root {
        self.remove_optional_file(&modules_yaml)?;
      }
    }
    Ok(())
  }

  fn run_package_manager(
    &self,
    executable: &Path,
    arguments: &[OsString],
    cwd: &Path,
    npmrc: Option<&Path>,
  ) -> Result<(), HvigorPluginWorkspaceError> {
    let mut command = Command::new(executable);
    command
      .args(arguments)
      .current_dir(cwd)
      .env("npm_config_update-notifier", "false")
      .env("npm_config_auto-install-peers", "false")
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit());
    if let Some(npmrc) = npmrc {
      command.env("npm_config_userconfig", npmrc);
    }
    let status =
      command
        .status()
        .map_err(|error| HvigorPluginWorkspaceError::PackageManagerStart {
          path: executable.to_path_buf(),
          message: error.to_string(),
        })?;
    if status.success() {
      Ok(())
    } else {
      Err(Self::command_failure(executable, status))
    }
  }

  fn command_failure(executable: &Path, status: ExitStatus) -> HvigorPluginWorkspaceError {
    HvigorPluginWorkspaceError::PackageManagerFailed {
      path: executable.to_path_buf(),
      status: status.code(),
    }
  }

  fn read_text(&self, path: &Path) -> Result<String, HvigorPluginWorkspaceError> {
    std::fs::read_to_string(path).map_err(|error| HvigorPluginWorkspaceError::ReadFile {
      path: path.to_path_buf(),
      message: error.to_string(),
    })
  }

  fn write_text(&self, path: &Path, source: &str) -> Result<(), HvigorPluginWorkspaceError> {
    std::fs::write(path, source).map_err(|error| HvigorPluginWorkspaceError::WriteFile {
      path: path.to_path_buf(),
      message: error.to_string(),
    })
  }

  fn remove_optional_file(&self, path: &Path) -> Result<(), HvigorPluginWorkspaceError> {
    match std::fs::remove_file(path) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(HvigorPluginWorkspaceError::RemoveFile {
        path: path.to_path_buf(),
        message: error.to_string(),
      }),
    }
  }

  fn path_text(&self, path: &Path) -> Result<String, HvigorPluginWorkspaceError> {
    path.to_str().map(ToOwned::to_owned).ok_or_else(|| {
      HvigorPluginWorkspaceError::InvalidDependencyPath {
        name: "store-dir".to_string(),
        path: path.to_path_buf(),
      }
    })
  }

  fn is_ci() -> bool {
    if std::env::var("CI").is_ok_and(|value| value == "false") {
      return false;
    }
    [
      "BUILD_ID",
      "BUILD_NUMBER",
      "CI",
      "CI_APP_ID",
      "CI_BUILD_ID",
      "CI_BUILD_NUMBER",
      "CI_NAME",
      "CONTINUOUS_INTEGRATION",
      "RUN_ID",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
  }
}

#[cfg(test)]
#[path = "hvigor_dependency_workspace_tests.rs"]
mod tests;
