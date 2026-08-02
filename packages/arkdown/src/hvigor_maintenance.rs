use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cmd::native_build::{HvigorModuleCleanStage, NativeSdkToolchain};
use crate::hvigor_sync::HvigorSdkInventory;

const THIRTY_DAYS_MS: i64 = 2_592_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorMaintenanceDiagnostic {
  code: &'static str,
  message: String,
  #[serde(flatten)]
  details: IndexMap<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorMaintenanceResponse {
  #[serde(skip_serializing_if = "Option::is_none")]
  output: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<HvigorMaintenanceDiagnostic>,
}

#[derive(Debug)]
enum HvigorMaintenanceError {
  CacheRead { path: PathBuf, message: String },
  CacheMetadata { path: PathBuf, message: String },
  CacheRemove { path: PathBuf, message: String },
  PnpmStart { path: PathBuf, message: String },
  PnpmFailed,
  Clean { path: PathBuf, message: String },
  SdkInventory(String),
}

impl HvigorMaintenanceError {
  fn diagnostic(self) -> HvigorMaintenanceDiagnostic {
    match self {
      Self::CacheRead { path, message } => HvigorMaintenanceDiagnostic {
        code: "HVIGOR_CACHE_READ_FAILED",
        message: format!(
          "failed to read hvigor cache path {}: {message}",
          path.display()
        ),
        details: IndexMap::from([(
          "path".to_string(),
          Value::String(path.to_string_lossy().into_owned()),
        )]),
      },
      Self::CacheMetadata { path, message } => HvigorMaintenanceDiagnostic {
        code: "HVIGOR_CACHE_METADATA_INVALID",
        message: format!(
          "failed to read hvigor cache metadata {}: {message}",
          path.display()
        ),
        details: IndexMap::from([(
          "path".to_string(),
          Value::String(path.to_string_lossy().into_owned()),
        )]),
      },
      Self::CacheRemove { path, message } => HvigorMaintenanceDiagnostic {
        code: "HVIGOR_CACHE_REMOVE_FAILED",
        message: format!(
          "failed to remove hvigor cache {}: {message}",
          path.display()
        ),
        details: IndexMap::from([(
          "path".to_string(),
          Value::String(path.to_string_lossy().into_owned()),
        )]),
      },
      Self::PnpmStart { path, message } => HvigorMaintenanceDiagnostic {
        code: "PNPM_STORE_PRUNE_EXECUTE_FAILED",
        message: format!("Failed to execute pnpm store prune: {message}"),
        details: IndexMap::from([(
          "pnpmPath".to_string(),
          Value::String(path.to_string_lossy().into_owned()),
        )]),
      },
      Self::PnpmFailed => HvigorMaintenanceDiagnostic {
        code: "PNPM_STORE_PRUNE_EXECUTE_FAILED",
        message: "Failed to execute pnpm store prune".to_string(),
        details: IndexMap::new(),
      },
      Self::Clean { path, message } => HvigorMaintenanceDiagnostic {
        code: "UNABLE_TO_REMOVE_BUILD_DIR",
        message: format!(
          "Unable to remove build directory {}: {message}",
          path.display()
        ),
        details: IndexMap::from([(
          "path".to_string(),
          Value::String(path.to_string_lossy().into_owned()),
        )]),
      },
      Self::SdkInventory(message) => HvigorMaintenanceDiagnostic {
        code: "ARKDOWN_HVIGOR_SDK_INVENTORY_FAILED",
        message,
        details: IndexMap::new(),
      },
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorPruneRequest {
  hvigor_user_home: PathBuf,
}

pub(crate) struct HvigorPruneRunner {
  request: HvigorPruneRequest,
}

impl HvigorPruneRunner {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    Ok(Self {
      request: serde_json::from_str(request_json)?,
    })
  }

  pub async fn response_json(self) -> anyhow::Result<String> {
    let response = match tokio::task::spawn_blocking(move || self.execute()).await? {
      Ok(output) => HvigorMaintenanceResponse {
        output: Some(output),
        error: None,
      },
      Err(error) => HvigorMaintenanceResponse {
        output: None,
        error: Some(error.diagnostic()),
      },
    };
    serde_json::to_string(&response).map_err(Into::into)
  }

  fn execute(&self) -> Result<Vec<String>, HvigorMaintenanceError> {
    let mut output = vec!["Clear the cache of the hvigor file".to_string()];
    let project_caches = self.request.hvigor_user_home.join("project_caches");
    let cutoff = Self::now_millis().saturating_sub(THIRTY_DAYS_MS);
    let stale_caches = self.stale_cache_roots(&project_caches, cutoff)?;
    for cache_root in &stale_caches {
      output.push(format!(
        "Clear the hvigor cache file:  {}",
        cache_root.display()
      ));
    }
    stale_caches
      .par_iter()
      .try_for_each(|cache_root| self.remove_cache_root(cache_root))?;

    output.push("Remove unreferenced packages from the store".to_string());
    self.run_pnpm_store_prune()?;
    Ok(output)
  }

  fn stale_cache_roots(
    &self,
    project_caches: &Path,
    cutoff: i64,
  ) -> Result<Vec<PathBuf>, HvigorMaintenanceError> {
    let entries = match std::fs::read_dir(project_caches) {
      Ok(entries) => entries,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
      Err(error) => {
        return Err(HvigorMaintenanceError::CacheRead {
          path: project_caches.to_path_buf(),
          message: error.to_string(),
        });
      }
    };
    let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|error| {
      HvigorMaintenanceError::CacheRead {
        path: project_caches.to_path_buf(),
        message: error.to_string(),
      }
    })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut stale = Vec::new();
    for entry in entries {
      let file_type = entry
        .file_type()
        .map_err(|error| HvigorMaintenanceError::CacheRead {
          path: entry.path(),
          message: error.to_string(),
        })?;
      if !file_type.is_dir() {
        continue;
      }
      let cache_root = entry.path();
      let metadata_path = cache_root.join("workspace").join("metadata.json");
      if !metadata_path.is_file() {
        continue;
      }
      let source = std::fs::read_to_string(&metadata_path).map_err(|error| {
        HvigorMaintenanceError::CacheMetadata {
          path: metadata_path.clone(),
          message: error.to_string(),
        }
      })?;
      let metadata = serde_json::from_str::<Value>(&source).map_err(|error| {
        HvigorMaintenanceError::CacheMetadata {
          path: metadata_path.clone(),
          message: error.to_string(),
        }
      })?;
      if metadata
        .get("lastUpdatedTime")
        .and_then(Self::javascript_number)
        .is_some_and(|last_updated| last_updated != 0.0 && cutoff as f64 > last_updated)
      {
        stale.push(cache_root);
      }
    }
    Ok(stale)
  }

  fn remove_cache_root(&self, cache_root: &Path) -> Result<(), HvigorMaintenanceError> {
    for attempt in 0..=3 {
      match std::fs::remove_dir_all(cache_root) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) if attempt < 3 => std::thread::sleep(Duration::from_millis(100)),
        Err(error) => {
          return Err(HvigorMaintenanceError::CacheRemove {
            path: cache_root.to_path_buf(),
            message: error.to_string(),
          });
        }
      }
    }
    Ok(())
  }

  fn run_pnpm_store_prune(&self) -> Result<(), HvigorMaintenanceError> {
    let wrapper_tools = self.request.hvigor_user_home.join("wrapper").join("tools");
    let executable = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    let pnpm = wrapper_tools
      .join("node_modules")
      .join(".bin")
      .join(executable);
    let status = Command::new(&pnpm)
      .arg("store")
      .arg("prune")
      .current_dir(&wrapper_tools)
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .spawn()
      .and_then(|mut child| child.wait())
      .map_err(|error| HvigorMaintenanceError::PnpmStart {
        path: pnpm,
        message: error.to_string(),
      })?;
    if status.success() {
      Ok(())
    } else {
      Err(HvigorMaintenanceError::PnpmFailed)
    }
  }

  fn javascript_number(value: &Value) -> Option<f64> {
    value
      .as_f64()
      .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
  }

  fn now_millis() -> i64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |duration| duration.as_millis() as i64)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorCleanRequest {
  project_root: PathBuf,
  node_root: PathBuf,
  project_task: bool,
  #[serde(default)]
  configured_build_dir: Option<String>,
  #[serde(default)]
  sdk_path: Option<PathBuf>,
  home_dir: PathBuf,
  #[serde(default)]
  project_profile: Value,
  #[serde(default)]
  target_names: Vec<String>,
  #[serde(default)]
  build_mode: Option<String>,
}

pub(crate) struct HvigorCleanRunner {
  request: HvigorCleanRequest,
}

impl HvigorCleanRunner {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    Ok(Self {
      request: serde_json::from_str(request_json)?,
    })
  }

  pub async fn response_json(self) -> anyhow::Result<String> {
    let response = match tokio::task::spawn_blocking(move || self.execute()).await? {
      Ok(output) => HvigorMaintenanceResponse {
        output: Some(output),
        error: None,
      },
      Err(error) => HvigorMaintenanceResponse {
        output: None,
        error: Some(error.diagnostic()),
      },
    };
    serde_json::to_string(&response).map_err(Into::into)
  }

  fn execute(&self) -> Result<Vec<String>, HvigorMaintenanceError> {
    let configured_build_dir = self.configured_build_dir();
    let toolchain = if self.request.project_task {
      None
    } else {
      let inventory =
        HvigorSdkInventory::new(self.request.sdk_path.as_deref(), &self.request.home_dir)
          .map_err(|error| HvigorMaintenanceError::SdkInventory(format!("{error:#}")))?;
      Some(NativeSdkToolchain::for_clean(inventory.native_ninja_tool()))
    };
    let stage = match toolchain.as_ref() {
      Some(toolchain) => HvigorModuleCleanStage::new(
        &self.request.project_root,
        &self.request.node_root,
        toolchain,
      )
      .with_native_variants(
        self.native_product_names(),
        self.request.target_names.clone(),
        self
          .request
          .build_mode
          .as_deref()
          .filter(|mode| !mode.is_empty())
          .unwrap_or("debug"),
      ),
      None => HvigorModuleCleanStage::for_project(&self.request.project_root),
    };
    let diagnostic_path = if self.request.project_task {
      &self.request.project_root
    } else {
      &self.request.node_root
    };
    stage
      .with_configured_build_dir(configured_build_dir.as_deref())
      .and_then(|stage| stage.execute())
      .map(|()| Vec::new())
      .map_err(|error| HvigorMaintenanceError::Clean {
        path: diagnostic_path.clone(),
        message: error.to_string(),
      })
  }

  fn native_product_names(&self) -> Vec<String> {
    let mut product_names = Vec::new();
    let Some(products) = self
      .request
      .project_profile
      .pointer("/app/products")
      .and_then(Value::as_array)
    else {
      return product_names;
    };
    for name in products
      .iter()
      .filter_map(|product| product.get("name"))
      .filter_map(Value::as_str)
      .filter(|name| !name.is_empty())
    {
      if !product_names.iter().any(|existing| existing == name) {
        product_names.push(name.to_string());
      }
    }
    product_names
  }

  fn configured_build_dir(&self) -> Option<PathBuf> {
    let mut value = self.request.configured_build_dir.as_deref()?;
    value = value.strip_prefix('\'').unwrap_or(value);
    value = value.strip_suffix('\'').unwrap_or(value);
    value = value.strip_prefix('"').unwrap_or(value);
    value = value.strip_suffix('"').unwrap_or(value);
    Some(PathBuf::from(value))
  }
}
