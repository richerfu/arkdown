//! Rust owner for Hvigor's post-configuration OHPM dependency-map install.
//!
//! Upstream correspondence:
//! `@ohos/hvigor-ohos-plugin` 6.24.2
//! `OhpmLoadInstallService.ohpmLoadInstallForBuild` and
//! `OhPackageLoader.generateDependencyMap`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapRequest {
  enabled: bool,
  clean_only: bool,
  project_root: PathBuf,
  cache_dir: PathBuf,
  #[serde(default)]
  target_name: String,
  #[serde(default)]
  project_package: Value,
  #[serde(default)]
  project_properties: Value,
  #[serde(default)]
  parameter_file_override: Option<String>,
  #[serde(default)]
  modules: Vec<OhpmDependencyMapModule>,
  #[serde(default)]
  package_model_updated: bool,
  #[serde(default)]
  enforce: bool,
  #[serde(default)]
  executable: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapModule {
  name: String,
  root: PathBuf,
  #[serde(default)]
  package: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapResponse {
  changed: bool,
  installed: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  warning: Option<OhpmDependencyMapDiagnostic>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<OhpmDependencyMapDiagnostic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapFile<'a> {
  base_path: &'a Path,
  #[serde(skip_serializing_if = "Option::is_none")]
  target_name: Option<&'a str>,
  root_dependency: &'static str,
  dependency_map: BTreeMap<String, String>,
  modules: Vec<OhpmDependencyMapFileModule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapFileModule {
  name: String,
  src_path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OhpmDependencyMapDiagnostic {
  code: &'static str,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  path: Option<PathBuf>,
  #[serde(skip_serializing_if = "Option::is_none")]
  status: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  args: Option<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
enum OhpmDependencyMapError {
  #[error("invalid module name {0:?} in the OHPM dependency map")]
  InvalidModuleName(String),
  #[error("failed to create OHPM dependency-map directory {path}: {source}")]
  CreateDirectory {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("failed to read OHPM dependency-map file {path}: {source}")]
  ReadFile {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("failed to write OHPM dependency-map file {path}: {source}")]
  WriteFile {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("failed to remove stale OHPM dependency-map directory {path}: {source}")]
  RemoveDirectory {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("failed to serialize OHPM dependency map: {0}")]
  Serialize(#[from] serde_json::Error),
  #[error("failed to execute OHPM {executable}: {source}")]
  Execute {
    executable: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("OHPM {executable} exited with status {status:?}")]
  Failed {
    executable: PathBuf,
    status: Option<i32>,
    args: Vec<String>,
  },
}

impl OhpmDependencyMapError {
  fn diagnostic(&self) -> OhpmDependencyMapDiagnostic {
    let (code, path, status, args) = match self {
      Self::InvalidModuleName(_) => ("INVALID_HVIGOR_MODULE_NAME", None, None, None),
      Self::CreateDirectory { path, .. }
      | Self::ReadFile { path, .. }
      | Self::WriteFile { path, .. }
      | Self::RemoveDirectory { path, .. } => (
        "OHPM_DEPENDENCY_MAP_IO_FAILED",
        Some(path.clone()),
        None,
        None,
      ),
      Self::Serialize(_) => ("OHPM_DEPENDENCY_MAP_SERIALIZE_FAILED", None, None, None),
      Self::Execute { executable, .. } => {
        ("OHPM_INSTALL_FAILED", Some(executable.clone()), None, None)
      }
      Self::Failed {
        executable,
        status,
        args,
      } => (
        "OHPM_INSTALL_FAILED",
        Some(executable.clone()),
        *status,
        Some(args.clone()),
      ),
    };
    OhpmDependencyMapDiagnostic {
      code,
      message: self.to_string(),
      path,
      status,
      args,
    }
  }
}

pub(crate) struct OhpmDependencyMapRunner {
  request: OhpmDependencyMapRequest,
}

impl OhpmDependencyMapRunner {
  pub(crate) fn from_value(request_value: Value) -> anyhow::Result<Self> {
    Ok(Self {
      request: serde_json::from_value(request_value)?,
    })
  }

  pub(crate) async fn response_value(self) -> anyhow::Result<Value> {
    let response = tokio::task::spawn_blocking(move || self.response())
      .await
      .map_err(|error| anyhow::anyhow!("OHPM dependency-map worker failed: {error}"))?;
    serde_json::to_value(response).map_err(Into::into)
  }

  fn response(self) -> OhpmDependencyMapResponse {
    match self.run() {
      Ok(response) => response,
      Err(error) => OhpmDependencyMapResponse {
        changed: false,
        installed: false,
        warning: None,
        error: Some(error.diagnostic()),
      },
    }
  }

  fn run(self) -> Result<OhpmDependencyMapResponse, OhpmDependencyMapError> {
    if !self.request.enabled || self.request.clean_only {
      return Ok(OhpmDependencyMapResponse {
        changed: false,
        installed: false,
        warning: None,
        error: None,
      });
    }

    let target_dir = lexical_normalize(
      &self
        .request
        .cache_dir
        .join("dependencyMap")
        .join(&self.request.target_name),
    );
    let map_file = target_dir.join("dependencyMap.json5");
    let had_dependency_map = map_file.is_file();
    Self::create_dir(&target_dir)?;

    let parameter_file = self.parameter_file();
    let mut project_package = self.request.project_package.clone();
    if let Some(package) = project_package.as_object_mut() {
      match parameter_file.as_ref() {
        Some(parameter_file) => {
          package.insert(
            "parameterFile".to_string(),
            Value::String(parameter_file.clone()),
          );
        }
        None => {
          package.remove("parameterFile");
        }
      }
    }
    let mut changed =
      Self::write_json_if_changed(&target_dir.join("oh-package.json5"), &project_package)?;

    let mut dependency_map = BTreeMap::new();
    let mut modules = Vec::with_capacity(self.request.modules.len());
    let mut module_names = BTreeSet::new();
    for module in &self.request.modules {
      Self::validate_module_name(&module.name)?;
      let module_dir = target_dir.join(&module.name);
      Self::create_dir(&module_dir)?;
      changed |=
        Self::write_json_if_changed(&module_dir.join("oh-package.json5"), &module.package)?;
      dependency_map.insert(
        module.name.clone(),
        format!("./{}/oh-package.json5", module.name),
      );
      modules.push(OhpmDependencyMapFileModule {
        name: module.name.clone(),
        src_path: Self::relative_path(&map_file, &module.root),
      });
      module_names.insert(OsString::from(&module.name));
    }

    for entry in
      std::fs::read_dir(&target_dir).map_err(|source| OhpmDependencyMapError::ReadFile {
        path: target_dir.clone(),
        source,
      })?
    {
      let entry = entry.map_err(|source| OhpmDependencyMapError::ReadFile {
        path: target_dir.clone(),
        source,
      })?;
      if !entry
        .file_type()
        .map_err(|source| OhpmDependencyMapError::ReadFile {
          path: entry.path(),
          source,
        })?
        .is_dir()
        || module_names.contains(&entry.file_name())
      {
        continue;
      }
      let path = entry.path();
      std::fs::remove_dir_all(&path).map_err(|source| OhpmDependencyMapError::RemoveDirectory {
        path: path.clone(),
        source,
      })?;
      changed = true;
    }

    let map = OhpmDependencyMapFile {
      base_path: &map_file,
      target_name: (!self.request.target_name.is_empty())
        .then_some(self.request.target_name.as_str()),
      root_dependency: "./oh-package.json5",
      dependency_map,
      modules,
    };
    changed |= Self::write_json_if_changed(&map_file, &serde_json::to_value(map)?)?;

    if !self.request.package_model_updated
      && !(had_dependency_map && changed)
      && !self.request.enforce
    {
      return Ok(OhpmDependencyMapResponse {
        changed,
        installed: false,
        warning: None,
        error: None,
      });
    }

    let executable = self
      .request
      .executable
      .filter(|path| path.is_file())
      .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "ohpm.cmd" } else { "ohpm" }));
    match Command::new(&executable)
      .arg("-v")
      .current_dir(&self.request.project_root)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()
    {
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
        return Ok(OhpmDependencyMapResponse {
          changed,
          installed: false,
          warning: Some(OhpmDependencyMapDiagnostic {
            code: "OHPM_EXECUTABLE_NOT_FOUND",
            message: "need to configure ohpm environment in your system, please see official guide to configure ohpm environment".to_string(),
            path: Some(executable),
            status: None,
            args: None,
          }),
          error: None,
        });
      }
      Err(source) => {
        return Err(OhpmDependencyMapError::Execute { executable, source });
      }
    }

    let mut args = vec![
      "i".to_string(),
      "--all".to_string(),
      "--target_path".to_string(),
      target_dir.to_string_lossy().into_owned(),
    ];
    if let Some(parameter_file) = parameter_file {
      args.push(format!("--parameterFile={parameter_file}"));
    }
    let status = Command::new(&executable)
      .args(&args)
      .current_dir(&self.request.project_root)
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .status()
      .map_err(|source| OhpmDependencyMapError::Execute {
        executable: executable.clone(),
        source,
      })?;
    if !status.success() {
      return Err(OhpmDependencyMapError::Failed {
        executable,
        status: status.code(),
        args,
      });
    }

    Ok(OhpmDependencyMapResponse {
      changed,
      installed: true,
      warning: None,
      error: None,
    })
  }

  fn parameter_file(&self) -> Option<String> {
    let value = self
      .request
      .parameter_file_override
      .as_deref()
      .filter(|value| !value.is_empty())
      .or_else(|| {
        self
          .request
          .project_package
          .get("parameterFile")
          .and_then(Value::as_str)
      })?;
    Some(Self::replace_parameters(
      value,
      &self.request.project_properties,
    ))
  }

  fn replace_parameters(value: &str, properties: &Value) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("${") {
      output.push_str(&remaining[..start]);
      let expression = &remaining[start + 2..];
      let Some(end) = expression.find('}') else {
        output.push_str(&remaining[start..]);
        return output;
      };
      let key = expression[..end].trim();
      let mut current = properties;
      let mut resolved = true;
      for component in key.split('.') {
        let Some(next) = current.get(component) else {
          resolved = false;
          break;
        };
        current = next;
      }
      if resolved {
        match current {
          Value::String(value) => output.push_str(value),
          Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
          Value::Number(value) => output.push_str(&value.to_string()),
          _ => output.push_str(&remaining[start..start + end + 3]),
        }
      } else {
        output.push_str(&remaining[start..start + end + 3]);
      }
      remaining = &expression[end + 1..];
    }
    output.push_str(remaining);
    output
  }

  fn validate_module_name(name: &str) -> Result<(), OhpmDependencyMapError> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains('/') || name.contains('\\') {
      return Err(OhpmDependencyMapError::InvalidModuleName(name.to_string()));
    }
    Ok(())
  }

  fn create_dir(path: &Path) -> Result<(), OhpmDependencyMapError> {
    std::fs::create_dir_all(path).map_err(|source| OhpmDependencyMapError::CreateDirectory {
      path: path.to_path_buf(),
      source,
    })
  }

  fn write_json_if_changed(path: &Path, value: &Value) -> Result<bool, OhpmDependencyMapError> {
    let content = serde_json::to_vec(value)?;
    match std::fs::read(path) {
      Ok(current) if current == content => return Ok(false),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(source) => {
        return Err(OhpmDependencyMapError::ReadFile {
          path: path.to_path_buf(),
          source,
        });
      }
    }
    std::fs::write(path, content).map_err(|source| OhpmDependencyMapError::WriteFile {
      path: path.to_path_buf(),
      source,
    })?;
    Ok(true)
  }

  /// Platform-native lexical equivalent of Node's `path.relative`.
  fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = lexical_normalize(from);
    let to = lexical_normalize(to);
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    if !Self::same_anchor(&from_components, &to_components) {
      return to;
    }
    let common = from_components
      .iter()
      .zip(&to_components)
      .take_while(|(left, right)| left == right)
      .count();
    let mut relative = PathBuf::new();
    for component in &from_components[common..] {
      if matches!(component, Component::Normal(_)) {
        relative.push("..");
      }
    }
    for component in &to_components[common..] {
      relative.push(component.as_os_str());
    }
    relative
  }

  fn same_anchor(left: &[Component<'_>], right: &[Component<'_>]) -> bool {
    match (left.first(), right.first()) {
      (Some(Component::Prefix(left)), Some(Component::Prefix(right))) => left == right,
      (Some(Component::RootDir), Some(Component::RootDir)) => true,
      (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => false,
      (Some(Component::RootDir), _) | (_, Some(Component::RootDir)) => false,
      _ => true,
    }
  }
}
