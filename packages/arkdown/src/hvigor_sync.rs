use std::collections::HashSet;
use std::path::{Path, PathBuf};

use indexmap::IndexSet;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::runtime_task::{HvigorTaskInfo, HvigorTaskInfoCatalog, RuntimeTaskNodeKind};

const HVIGOR_PARITY_VERSION: &str = "6.24.3";
const SDK_METADATA_PATHS: &[&[&str]] = &[
  &["sdk-pkg.json"],
  &["openharmony", "toolchains", "oh-uni-package.json"],
  &["hms", "toolchains", "uni-package.json"],
  &["toolchains", "oh-uni-package.json"],
  &["toolchains", "uni-package.json"],
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorCacheStateRequest {
  project_root: PathBuf,
  #[serde(default)]
  configured_cache_dir: Option<PathBuf>,
  #[serde(default)]
  sdk_path: Option<PathBuf>,
  home_dir: PathBuf,
  #[serde(default)]
  compile_sdk_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorSyncModelRequest {
  cache: HvigorCacheStateRequest,
  project_name: String,
  selected_product_name: String,
  mode: String,
  #[serde(default)]
  config_build_version: Option<Value>,
  #[serde(default)]
  parameter_build_version: Option<Value>,
  project_profile: Value,
  #[serde(default)]
  app_json: Option<Value>,
  #[serde(default)]
  project_hvigor_overrides: Value,
  #[serde(default)]
  properties: Map<String, Value>,
  #[serde(default)]
  configured_build_dir: Option<PathBuf>,
  #[serde(default)]
  modules: Vec<HvigorSyncModuleRequest>,
  #[serde(default)]
  selected_node_names: Option<Vec<String>>,
  #[serde(default)]
  project_custom_tasks: Vec<HvigorCustomTaskInfo>,
  #[serde(default)]
  custom_models: Map<String, Value>,
  #[serde(default)]
  custom_model_task_info: Map<String, Value>,
  sync_version_changed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorSyncModuleRequest {
  name: String,
  root: PathBuf,
  kind: RuntimeTaskNodeKind,
  #[serde(default)]
  belong_project_path: Option<PathBuf>,
  #[serde(default)]
  module_profile: Value,
  #[serde(default)]
  selected_targets: Vec<HvigorSyncTargetRequest>,
  #[serde(default)]
  custom_tasks: Vec<HvigorCustomTaskInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorSyncTargetRequest {
  name: String,
  #[serde(default)]
  build_option: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HvigorCustomTaskInfo {
  task_name: String,
  #[serde(default = "HvigorCustomTaskInfo::default_group")]
  group: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  source_plugin_id: String,
}

impl HvigorCustomTaskInfo {
  fn default_group() -> String {
    "Customize".to_string()
  }
}

/// Rust owner for hvigor cache layout, SDK inventory, and version invalidation.
pub struct HvigorCacheState {
  request: HvigorCacheStateRequest,
  cache_root: PathBuf,
  sdk_inventory: HvigorSdkInventory,
}

impl HvigorCacheState {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    let request = serde_json::from_str(request_json)
      .map_err(|error| anyhow::anyhow!("ARKDOWN_INVALID_HVIGOR_CACHE_REQUEST: {error}"))?;
    Self::new(request)
  }

  pub fn new(request: HvigorCacheStateRequest) -> anyhow::Result<Self> {
    let cache_root = Self::cache_root_for(&request)?;
    let sdk_inventory = HvigorSdkInventory::new(request.sdk_path.as_deref(), &request.home_dir)?;
    Ok(Self {
      request,
      cache_root,
      sdk_inventory,
    })
  }

  pub fn version_changed(&self) -> bool {
    let cache_dir = self.cache_root.join("cache");
    let cached = ["meta.json", "arkdown-sync-meta.json"]
      .into_iter()
      .find_map(|file_name| {
        std::fs::read_to_string(cache_dir.join(file_name))
          .ok()
          .and_then(|source| serde_json::from_str::<Value>(&source).ok())
      });
    if cached
      .as_ref()
      .and_then(|value| value.get("hvigorVersion"))
      .and_then(Value::as_str)
      != Some(HVIGOR_PARITY_VERSION)
    {
      return true;
    }
    let Some(cached_toolchain) = cached
      .as_ref()
      .and_then(|value| value.get("toolChainsVersion"))
      .and_then(Value::as_str)
    else {
      return false;
    };
    let installed_versions = self.sdk_inventory.installed_toolchain_versions();
    !installed_versions.is_empty() && !installed_versions.contains(cached_toolchain)
  }

  pub async fn write_build_meta(&self) -> anyhow::Result<()> {
    let selected_sdk = self.sdk_inventory.selected_metadata();
    let mut metadata = Map::new();
    if let Some(compile_sdk_version) = selected_sdk
      .as_ref()
      .and_then(HvigorSdkInventory::compile_sdk_version)
      .or_else(|| self.request.compile_sdk_version.clone())
    {
      metadata.insert(
        "compileSdkVersion".to_string(),
        Value::String(compile_sdk_version),
      );
    }
    metadata.insert(
      "hvigorVersion".to_string(),
      Value::String(HVIGOR_PARITY_VERSION.to_string()),
    );
    if let Some(version) = selected_sdk
      .as_ref()
      .and_then(|sdk| sdk.get("version"))
      .and_then(Value::as_str)
    {
      metadata.insert(
        "toolChainsVersion".to_string(),
        Value::String(version.to_string()),
      );
    }
    let cache_dir = self.cache_root.join("cache");
    tokio::fs::create_dir_all(&cache_dir)
      .await
      .map_err(|error| {
        anyhow::anyhow!(
          "ARKDOWN_HVIGOR_CACHE_CREATE_FAILED: failed to create {}: {error}",
          cache_dir.display()
        )
      })?;
    let mut source = serde_json::to_string(&metadata)?;
    source.push('\n');
    let meta_path = cache_dir.join("meta.json");
    tokio::fs::write(&meta_path, source).await.map_err(|error| {
      anyhow::anyhow!(
        "ARKDOWN_HVIGOR_BUILD_META_WRITE_FAILED: failed to write {}: {error}",
        meta_path.display()
      )
    })
  }

  fn cache_root_for(request: &HvigorCacheStateRequest) -> anyhow::Result<PathBuf> {
    let Some(configured) = request
      .configured_cache_dir
      .as_ref()
      .filter(|configured| configured.is_absolute())
    else {
      return Ok(request.project_root.join(".hvigor"));
    };
    let configured = if configured.exists() {
      dunce::canonicalize(configured).map_err(|error| {
        anyhow::anyhow!(
          "ARKDOWN_HVIGOR_CACHE_CANONICALIZE_FAILED: failed to resolve {}: {error}",
          configured.display()
        )
      })?
    } else {
      configured.clone()
    };
    let project_name = request.project_root.file_name().ok_or_else(|| {
      anyhow::anyhow!(
        "ARKDOWN_INVALID_HVIGOR_PROJECT_ROOT: project root has no final component: {}",
        request.project_root.display()
      )
    })?;
    Ok(configured.join(project_name).join(".hvigor"))
  }
}

pub(crate) struct HvigorSdkInventory {
  roots: Vec<PathBuf>,
}

impl HvigorSdkInventory {
  pub(crate) fn new(explicit_sdk_path: Option<&Path>, home_dir: &Path) -> anyhow::Result<Self> {
    let mut roots = Vec::new();
    if let Some(explicit_sdk_path) = explicit_sdk_path {
      Self::add_root(&mut roots, explicit_sdk_path)?;
    }
    for variable in [
      "DEVECO_SDK_HOME",
      "OHOS_BASE_SDK_HOME",
      "OHOS_SDK_HOME",
      "OHOS_NDK_HOME",
      "OHOS_SDK_NATIVE",
    ] {
      if let Some(value) = std::env::var_os(variable) {
        Self::add_root(&mut roots, Path::new(&value))?;
      }
    }
    if cfg!(target_os = "macos") {
      for candidate in [
        PathBuf::from("/Applications/DevEco-Studio.app/Contents/sdk"),
        PathBuf::from("/Applications/DevEco Studio.app/Contents/sdk"),
        home_dir.join("Applications/DevEco-Studio.app/Contents/sdk"),
        home_dir.join("Library/Huawei/Sdk"),
        home_dir.join("Library/OpenHarmony/Sdk"),
      ] {
        Self::add_root(&mut roots, &candidate)?;
      }
    } else if cfg!(target_os = "windows") {
      let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
          std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .map(|path| path.join("Local"))
        });
      if let Some(local_app_data) = local_app_data {
        for candidate in [
          local_app_data.join("Huawei").join("Sdk"),
          local_app_data
            .join("Programs")
            .join("Huawei")
            .join("DevEco Studio")
            .join("sdk"),
        ] {
          Self::add_root(&mut roots, &candidate)?;
        }
      }
      for variable in ["PROGRAMFILES", "ProgramW6432"] {
        if let Some(program_files) = std::env::var_os(variable).map(PathBuf::from) {
          for candidate in [
            program_files
              .join("Huawei")
              .join("DevEco Studio")
              .join("sdk"),
            program_files.join("DevEco Studio").join("sdk"),
          ] {
            Self::add_root(&mut roots, &candidate)?;
          }
        }
      }
    } else {
      for candidate in [
        home_dir.join("DevEco-Studio/sdk"),
        home_dir.join("deveco-studio/sdk"),
        PathBuf::from("/opt/DevEco-Studio/sdk"),
        PathBuf::from("/opt/deveco-studio/sdk"),
        PathBuf::from("/opt/huawei/sdk"),
      ] {
        Self::add_root(&mut roots, &candidate)?;
      }
    }
    Ok(Self { roots })
  }

  pub(crate) fn native_ninja_tool(&self) -> Option<PathBuf> {
    let executable = if cfg!(windows) { "ninja.exe" } else { "ninja" };
    for sdk_root in &self.roots {
      for component_root in self.component_roots(sdk_root) {
        for native_root in [
          component_root.join("openharmony").join("native"),
          component_root.join("native"),
        ] {
          let candidate = native_root
            .join("build-tools")
            .join("cmake")
            .join("bin")
            .join(executable);
          if candidate.is_file() {
            return Some(candidate);
          }
        }
      }
    }
    None
  }

  fn selected_metadata(&self) -> Option<Map<String, Value>> {
    let mut selected: Option<Map<String, Value>> = None;
    for sdk_root in &self.roots {
      for component_root in self.component_roots(sdk_root) {
        for relative_path in SDK_METADATA_PATHS {
          let metadata_path = relative_path
            .iter()
            .fold(component_root.clone(), |path, component| {
              path.join(component)
            });
          let Some(metadata) = self.read_metadata(&metadata_path) else {
            continue;
          };
          let Some(version) = metadata.get("version").and_then(Value::as_str) else {
            continue;
          };
          if selected.is_none() {
            selected = Some(metadata.clone());
          }
          let selected_version = selected
            .as_ref()
            .and_then(|value| value.get("version"))
            .and_then(Value::as_str);
          if selected_version == Some(version)
            && metadata
              .get("platformVersion")
              .and_then(Value::as_str)
              .is_some()
            && metadata
              .get("apiVersion")
              .is_some_and(|value| !value.is_null())
          {
            let mut merged = selected.take().unwrap_or_default();
            merged.extend(metadata);
            return Some(merged);
          }
        }
      }
    }
    selected
  }

  fn installed_toolchain_versions(&self) -> IndexSet<String> {
    for sdk_root in &self.roots {
      let mut versions = IndexSet::new();
      for component_root in self.component_roots(sdk_root) {
        for relative_path in SDK_METADATA_PATHS {
          let metadata_path = relative_path
            .iter()
            .fold(component_root.clone(), |path, component| {
              path.join(component)
            });
          if let Some(version) = self
            .read_metadata(&metadata_path)
            .and_then(|metadata| metadata.get("version").cloned())
            .and_then(|version| version.as_str().map(str::to_string))
          {
            versions.insert(version);
          }
        }
      }
      if !versions.is_empty() {
        return versions;
      }
    }
    IndexSet::new()
  }

  fn compile_sdk_version(metadata: &Map<String, Value>) -> Option<String> {
    let platform_version = metadata.get("platformVersion")?.as_str()?;
    let api_version = metadata.get("apiVersion")?;
    if api_version.is_null() || api_version == &Value::Bool(false) {
      return None;
    }
    let api_version = api_version
      .as_str()
      .map(str::to_string)
      .unwrap_or_else(|| api_version.to_string());
    Some(format!("{platform_version}({api_version})"))
  }

  fn component_roots(&self, sdk_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![sdk_root.to_path_buf(), sdk_root.join("default")];
    let Ok(entries) = std::fs::read_dir(sdk_root) else {
      return roots;
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    for entry in entries {
      let Ok(file_type) = entry.file_type() else {
        continue;
      };
      if !file_type.is_dir() && !file_type.is_symlink() {
        continue;
      }
      let child = entry.path();
      if !roots.contains(&child) {
        roots.push(child.clone());
      }
      let default = child.join("default");
      if !roots.contains(&default) {
        roots.push(default);
      }
    }
    roots
  }

  fn read_metadata(&self, metadata_path: &Path) -> Option<Map<String, Value>> {
    let source = std::fs::read_to_string(metadata_path).ok()?;
    let raw = serde_json::from_str::<Value>(&source).ok()?;
    let metadata = raw
      .get("data")
      .and_then(Value::as_object)
      .or_else(|| raw.as_object())?;
    Some(metadata.clone())
  }

  fn add_root(roots: &mut Vec<PathBuf>, candidate: &Path) -> anyhow::Result<()> {
    if candidate.as_os_str().is_empty() {
      return Ok(());
    }
    let candidate = if candidate.is_absolute() {
      candidate.to_path_buf()
    } else {
      std::env::current_dir()
        .map_err(|error| {
          anyhow::anyhow!(
            "ARKDOWN_CURRENT_DIRECTORY_READ_FAILED: failed to resolve SDK path {}: {error}",
            candidate.display()
          )
        })?
        .join(candidate)
    };
    if !roots.contains(&candidate) {
      roots.push(candidate);
    }
    Ok(())
  }
}

/// Rust owner for latest-hvigor IDE tooling models and taskInfo output.
pub struct HvigorSyncModelWriter {
  request: HvigorSyncModelRequest,
  cache_state: HvigorCacheState,
  selected_node_names: Option<HashSet<String>>,
}

impl HvigorSyncModelWriter {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    let request = serde_json::from_str(request_json)
      .map_err(|error| anyhow::anyhow!("ARKDOWN_INVALID_HVIGOR_SYNC_REQUEST: {error}"))?;
    Self::new(request)
  }

  pub fn new(request: HvigorSyncModelRequest) -> anyhow::Result<Self> {
    let cache_state = HvigorCacheState::new(HvigorCacheStateRequest {
      project_root: request.cache.project_root.clone(),
      configured_cache_dir: request.cache.configured_cache_dir.clone(),
      sdk_path: request.cache.sdk_path.clone(),
      home_dir: request.cache.home_dir.clone(),
      compile_sdk_version: request.cache.compile_sdk_version.clone(),
    })?;
    let selected_node_names = request
      .selected_node_names
      .as_ref()
      .map(|names| names.iter().cloned().collect());
    Ok(Self {
      request,
      cache_state,
      selected_node_names,
    })
  }

  pub async fn write(self) -> anyhow::Result<()> {
    let (output, task_info) = self.build_models()?;
    let output_source = serde_json::to_string_pretty(&output)?;
    let task_info_source = serde_json::to_string_pretty(&task_info)?;
    let sync_dir = self.cache_state.cache_root.join("outputs").join("sync");
    let cache_dir = self.cache_state.cache_root.join("cache");
    tokio::try_join!(
      tokio::fs::create_dir_all(&sync_dir),
      tokio::fs::create_dir_all(&cache_dir),
    )
    .map_err(|error| {
      anyhow::anyhow!(
        "ARKDOWN_HVIGOR_SYNC_DIRECTORY_CREATE_FAILED: failed to create {}: {error}",
        self.cache_state.cache_root.display()
      )
    })?;

    let selected_sdk = self.cache_state.sdk_inventory.selected_metadata();
    let mut sync_metadata = Map::new();
    sync_metadata.insert(
      "hvigorVersion".to_string(),
      Value::String(HVIGOR_PARITY_VERSION.to_string()),
    );
    if let Some(version) = selected_sdk
      .as_ref()
      .and_then(|sdk| sdk.get("version"))
      .and_then(Value::as_str)
    {
      sync_metadata.insert(
        "toolChainsVersion".to_string(),
        Value::String(version.to_string()),
      );
    }
    let mut sync_metadata_source = serde_json::to_string_pretty(&sync_metadata)?;
    sync_metadata_source.push('\n');
    let output_path = sync_dir.join("output.json");
    let task_info_path = sync_dir.join("taskInfo.json");
    let sync_metadata_path = cache_dir.join("arkdown-sync-meta.json");
    tokio::try_join!(
      tokio::fs::write(&output_path, output_source),
      tokio::fs::write(&task_info_path, task_info_source),
      tokio::fs::write(&sync_metadata_path, sync_metadata_source),
    )
    .map_err(|error| {
      anyhow::anyhow!(
        "ARKDOWN_HVIGOR_SYNC_WRITE_FAILED: failed to write sync output under {}: {error}",
        sync_dir.display()
      )
    })?;
    Ok(())
  }

  fn build_models(&self) -> anyhow::Result<(Map<String, Value>, Map<String, Value>)> {
    let mut output = Map::new();
    let mut task_info = Map::new();
    output.insert("version".to_string(), Value::from(1));
    for module in &self.request.modules {
      if !self.is_selected(&module.name) {
        continue;
      }
      let model_id = format!("ohos-module-{}", module.name);
      output.insert(model_id.clone(), self.module_model(module)?);
      task_info.insert(model_id, self.module_task_info(module)?);
    }
    if self.is_selected(&self.request.project_name) {
      output.insert("ohos-project".to_string(), self.project_model()?);
      task_info.insert("ohos-project".to_string(), self.project_task_info()?);
    }
    for (model_id, model) in &self.request.custom_models {
      output.insert(model_id.clone(), model.clone());
      task_info.insert(
        model_id.clone(),
        self
          .request
          .custom_model_task_info
          .get(model_id)
          .cloned()
          .unwrap_or_else(|| Value::Array(Vec::new())),
      );
    }
    Ok((output, task_info))
  }

  fn project_model(&self) -> anyhow::Result<Value> {
    let project_build_root = self.build_root(&self.request.cache.project_root)?;
    let profile = self.request.project_profile.clone();
    let product = Self::values_at(&profile, &["app", "products"])
      .into_iter()
      .find(|candidate| {
        candidate.get("name").and_then(Value::as_str)
          == Some(self.request.selected_product_name.as_str())
      });
    let app = self
      .request
      .app_json
      .as_ref()
      .and_then(|app_json| app_json.get("app"));
    let app_opt = self.request.project_hvigor_overrides.get("appOpt");
    let mut model = Map::new();
    model.insert(
      "SELECT_PRODUCT_NAME".to_string(),
      Value::String(self.request.selected_product_name.clone()),
    );
    model.insert(
      "MODULE_BUILD_DIR".to_string(),
      self.path_value(&project_build_root),
    );
    let bundle_name = Self::first_non_null([
      app_opt.and_then(|value| value.get("bundleName")),
      product.and_then(|value| value.get("bundleName")),
      app.and_then(|value| value.get("bundleName")),
    ])
    .cloned()
    .unwrap_or_else(|| Value::String("com.example.myapplication".to_string()));
    model.insert("BUNDLE_NAME".to_string(), bundle_name);

    let version_code = Self::first_non_null([
      app_opt.and_then(|value| value.get("versionCode")),
      app.and_then(|value| value.get("versionCode")),
    ]);
    let version_name = Self::first_non_null([
      app_opt.and_then(|value| value.get("versionName")),
      app.and_then(|value| value.get("versionName")),
    ]);
    let build_version = Self::first_non_null([
      self.request.config_build_version.as_ref(),
      self.request.parameter_build_version.as_ref(),
      product.and_then(|value| value.get("buildVersion")),
      app.and_then(|value| value.get("buildVersion")),
    ]);
    for (key, value) in [
      ("VERSION_CODE", version_code),
      ("VERSION_NAME", version_name),
      ("BUILD_VERSION", build_version),
    ] {
      if value.is_some_and(Self::is_js_truthy) {
        model.insert(key.to_string(), value.cloned().unwrap_or(Value::Null));
      }
    }
    let mut build_path = Map::new();
    build_path.insert(
      "OUTPUT_PATH".to_string(),
      self.path_value(
        &project_build_root
          .join("outputs")
          .join(&self.request.selected_product_name),
      ),
    );
    model.insert("BUILD_PATH".to_string(), Value::Object(build_path));

    let mut overall_project_paths = IndexSet::new();
    overall_project_paths.insert(self.request.cache.project_root.clone());
    let profile_modules = Self::values_at(&profile, &["modules"]);
    let mut modules = Vec::with_capacity(self.request.modules.len());
    for module in &self.request.modules {
      let configured = profile_modules.iter().copied().find(|candidate| {
        candidate.get("name").and_then(Value::as_str) == Some(module.name.as_str())
      });
      let mut model_module = configured
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
      let src_path = configured
        .and_then(|value| value.get("srcPath"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map_or_else(
          || module.root.clone(),
          |value| self.resolve_project_path(value),
        );
      let belong_project_path = module
        .belong_project_path
        .clone()
        .unwrap_or_else(|| self.request.cache.project_root.clone());
      overall_project_paths.insert(belong_project_path.clone());
      model_module.insert("srcPath".to_string(), self.path_value(&src_path));
      model_module.insert(
        "belongProjectPath".to_string(),
        self.path_value(&belong_project_path),
      );
      modules.push(Value::Object(model_module));
    }
    model.insert("MODULES".to_string(), Value::Array(modules));
    model.insert("PROFILE_OPT".to_string(), profile);
    model.insert(
      "CONFIG_PROPERTIES".to_string(),
      Value::Object(self.request.properties.clone()),
    );
    model.insert(
      "OVERALL_PROJECT_PATHS".to_string(),
      Value::Array(
        overall_project_paths
          .iter()
          .map(|path| self.path_value(path))
          .collect(),
      ),
    );
    model.insert(
      "BUILD_CACHE_DIR".to_string(),
      self.configured_build_dir_value()?,
    );
    Ok(Value::Object(model))
  }

  fn module_model(&self, module: &HvigorSyncModuleRequest) -> anyhow::Result<Value> {
    if matches!(module.kind, RuntimeTaskNodeKind::App) {
      anyhow::bail!(
        "ARKDOWN_INVALID_HVIGOR_SYNC_MODULE_KIND: module {} cannot use app task metadata",
        module.name
      );
    }
    let module_build_root = self.build_root(&module.root)?;
    let product_build_root = module_build_root.join(&self.request.selected_product_name);
    let raw_targets = Self::values_at(&module.module_profile, &["targets"]);
    let synthetic_default = Value::Object(Map::from_iter([(
      "name".to_string(),
      Value::String("default".to_string()),
    )]));
    let targets = if raw_targets.is_empty() {
      vec![&synthetic_default]
    } else {
      raw_targets
    };
    let mut target_models = Map::new();
    for target in targets {
      let target_name = target.get("name").and_then(Value::as_str).ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_INVALID_HVIGOR_SYNC_TARGET: module {} contains a target without a name",
          module.name
        )
      })?;
      let source_set = if target_name == "default" {
        "main"
      } else {
        target_name
      };
      let resources = Self::values_at(target, &["resource", "directories"]);
      let resource_paths = if resources.is_empty() {
        vec![self.path_value(&module.root.join("src").join(source_set).join("resources"))]
      } else {
        resources
          .into_iter()
          .map(|directory| {
            directory.as_str().map_or_else(
              || Value::Null,
              |directory| self.path_value(&self.resolve_module_path(&module.root, directory)),
            )
          })
          .collect()
      };
      let ets_task = if target_name == "ohosTest" {
        "OhosTestCompileArkTS"
      } else if matches!(module.kind, RuntimeTaskNodeKind::Har) {
        "HarCompileArkTS"
      } else {
        "CompileArkTS"
      };
      let js_task = if target_name == "ohosTest" {
        "OhosTestCompileJS"
      } else if matches!(module.kind, RuntimeTaskNodeKind::Har) {
        "HarCompileJS"
      } else {
        "CompileJS"
      };
      let mut build_path = Map::new();
      for (key, path) in [
        (
          "OUTPUT_PATH",
          product_build_root.join("outputs").join(target_name),
        ),
        ("INTERMEDIA_PATH", product_build_root.join("intermediates")),
        (
          "JS_ASSETS_PATH",
          product_build_root
            .join("intermediates")
            .join("loader_out")
            .join(target_name),
        ),
        (
          "JS_LITE_ASSETS_PATH",
          product_build_root
            .join("intermediates")
            .join("loader_out_lite")
            .join(target_name),
        ),
        (
          "RES_PATH",
          product_build_root
            .join("intermediates")
            .join("res")
            .join(target_name),
        ),
        (
          "RES_PROFILE_PATH",
          product_build_root
            .join("intermediates")
            .join("res")
            .join(target_name)
            .join("resources")
            .join("base")
            .join("profile"),
        ),
        (
          "ETS_SUPER_VISUAL_PATH",
          product_build_root
            .join("cache")
            .join(target_name)
            .join(format!("{target_name}@{ets_task}"))
            .join("esmodule"),
        ),
        (
          "JS_SUPER_VISUAL_PATH",
          product_build_root
            .join("cache")
            .join(target_name)
            .join(format!("{target_name}@{js_task}"))
            .join("jsbundle"),
        ),
        (
          "WORKER_LOADER",
          product_build_root
            .join("intermediates")
            .join("loader")
            .join(target_name)
            .join("loader.json"),
        ),
        (
          "MANIFEST_JSON",
          product_build_root
            .join("intermediates")
            .join("manifest")
            .join(target_name),
        ),
        (
          "OUTPUT_METADATA_JSON",
          product_build_root
            .join("intermediates")
            .join(if matches!(module.kind, RuntimeTaskNodeKind::Hsp) {
              "hsp_metadata"
            } else {
              "hap_metadata"
            })
            .join(target_name)
            .join("output_metadata.json"),
        ),
        (
          "SOURCE_MAP_DIR",
          product_build_root
            .join("intermediates")
            .join("source_map")
            .join(target_name),
        ),
      ] {
        build_path.insert(key.to_string(), self.path_value(&path));
      }
      let mut target_model = Map::new();
      target_model.insert(
        "SOURCE_ROOT".to_string(),
        self.path_value(&module.root.join("src").join(source_set)),
      );
      target_model.insert("RESOURCES_PATH".to_string(), Value::Array(resource_paths));
      target_model.insert("BUILD_PATH".to_string(), Value::Object(build_path));
      let debuggable = module
        .selected_targets
        .iter()
        .find(|selected| selected.name == target_name)
        .and_then(|selected| selected.build_option.get("debuggable"))
        .and_then(Value::as_bool)
        .unwrap_or(self.request.mode != "release");
      target_model.insert(
        "BUILD_OPTION".to_string(),
        Value::Object(Map::from_iter([(
          "debuggable".to_string(),
          Value::Bool(debuggable),
        )])),
      );
      target_models.insert(target_name.to_string(), Value::Object(target_model));
    }

    let mut model = Map::new();
    model.insert(
      "SELECT_TARGET".to_string(),
      Value::String("default".to_string()),
    );
    model.insert(
      "MODULE_BUILD_DIR".to_string(),
      self.path_value(&module_build_root),
    );
    model.insert("TARGETS".to_string(), Value::Object(target_models));
    let selected_targets = if module.selected_targets.is_empty() {
      vec![HvigorSyncTargetRequest {
        name: "default".to_string(),
        build_option: Value::Object(Map::new()),
      }]
    } else {
      module
        .selected_targets
        .iter()
        .map(|target| HvigorSyncTargetRequest {
          name: target.name.clone(),
          build_option: target.build_option.clone(),
        })
        .collect()
    };
    let mut build_options = Map::new();
    for target in selected_targets {
      let mut build_option = Map::from_iter([
        (
          "debuggable".to_string(),
          Value::Bool(self.request.mode != "release"),
        ),
        ("copyFrom".to_string(), Value::String("default".to_string())),
      ]);
      if let Some(configured) = target.build_option.as_object() {
        build_option.extend(configured.clone());
      }
      build_option.insert("name".to_string(), Value::String(self.request.mode.clone()));
      build_options.insert(
        format!("{}-{}", self.request.selected_product_name, target.name),
        Value::Object(build_option),
      );
    }
    model.insert("BUILD_OPTION".to_string(), Value::Object(build_options));
    model.insert(
      "BUILD_PROFILE_OPT".to_string(),
      self.normalized_module_profile(module),
    );
    model.insert(
      "BUILD_CACHE_DIR".to_string(),
      self.configured_build_dir_value()?,
    );
    Ok(Value::Object(model))
  }

  fn project_task_info(&self) -> anyhow::Result<Value> {
    let names = HvigorTaskInfoCatalog::project_task_names();
    let mut details = names
      .iter()
      .map(|name| HvigorTaskInfoCatalog::task_info(name))
      .collect::<anyhow::Result<Vec<_>>>()?;
    let built_in_names = names.iter().copied().collect::<HashSet<_>>();
    details.extend(
      self
        .request
        .project_custom_tasks
        .iter()
        .filter(|task| !built_in_names.contains(task.task_name.as_str()))
        .map(|task| HvigorTaskInfo {
          task_name: task.task_name.clone(),
          group: task.group.clone(),
          description: task.description.clone(),
          source_plugin_id: task.source_plugin_id.clone(),
        }),
    );
    serde_json::to_value(details).map_err(Into::into)
  }

  fn module_task_info(&self, module: &HvigorSyncModuleRequest) -> anyhow::Result<Value> {
    let target_name = module
      .selected_targets
      .first()
      .map_or("default", |target| target.name.as_str());
    let names = HvigorTaskInfoCatalog::module_task_names(
      module.kind,
      target_name,
      self.request.sync_version_changed,
    );
    let mut details = names
      .iter()
      .map(|name| HvigorTaskInfoCatalog::task_info(name))
      .collect::<anyhow::Result<Vec<_>>>()?;
    let built_in_names = names.iter().map(String::as_str).collect::<HashSet<_>>();
    details.extend(
      module
        .custom_tasks
        .iter()
        .filter(|task| !built_in_names.contains(task.task_name.as_str()))
        .map(|task| HvigorTaskInfo {
          task_name: task.task_name.clone(),
          group: task.group.clone(),
          description: task.description.clone(),
          source_plugin_id: task.source_plugin_id.clone(),
        }),
    );
    serde_json::to_value(details).map_err(Into::into)
  }

  fn normalized_module_profile(&self, module: &HvigorSyncModuleRequest) -> Value {
    let mut profile = module
      .module_profile
      .as_object()
      .cloned()
      .unwrap_or_default();
    let mut build_options = Self::values_at(&module.module_profile, &["buildOptionSet"])
      .into_iter()
      .cloned()
      .collect::<Vec<_>>();
    for required in ["default", "debug"] {
      if !build_options
        .iter()
        .any(|option| option.get("name").and_then(Value::as_str) == Some(required))
      {
        build_options.push(Value::Object(Map::from_iter([(
          "name".to_string(),
          Value::String(required.to_string()),
        )])));
      }
    }
    profile.insert("buildOptionSet".to_string(), Value::Array(build_options));
    Value::Object(profile)
  }

  fn build_root(&self, node_root: &Path) -> anyhow::Result<PathBuf> {
    let Some(configured) = self
      .request
      .configured_build_dir
      .as_ref()
      .filter(|configured| configured.is_absolute())
    else {
      return Ok(node_root.join("build"));
    };
    let configured = if configured.exists() {
      dunce::canonicalize(configured).map_err(|error| {
        anyhow::anyhow!(
          "ARKDOWN_HVIGOR_BUILD_DIR_CANONICALIZE_FAILED: failed to resolve {}: {error}",
          configured.display()
        )
      })?
    } else {
      configured.clone()
    };
    let relative = node_root
      .strip_prefix(&self.request.cache.project_root)
      .map_err(|_| {
        anyhow::anyhow!(
          "ARKDOWN_HVIGOR_MODULE_OUTSIDE_PROJECT: module root {} is outside project {}",
          node_root.display(),
          self.request.cache.project_root.display()
        )
      })?;
    Ok(
      configured
        .join(&self.request.project_name)
        .join(relative)
        .join("build"),
    )
  }

  fn configured_build_dir_value(&self) -> anyhow::Result<Value> {
    let Some(configured) = self.request.configured_build_dir.as_ref() else {
      return Ok(Value::String(String::new()));
    };
    let configured = if configured.exists() {
      dunce::canonicalize(configured).map_err(|error| {
        anyhow::anyhow!(
          "ARKDOWN_HVIGOR_BUILD_DIR_CANONICALIZE_FAILED: failed to resolve {}: {error}",
          configured.display()
        )
      })?
    } else {
      configured.clone()
    };
    Ok(self.path_value(&configured))
  }

  fn is_selected(&self, node_name: &str) -> bool {
    self
      .selected_node_names
      .as_ref()
      .is_none_or(|selected| selected.contains(node_name))
  }

  fn resolve_project_path(&self, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
      path.to_path_buf()
    } else {
      self.request.cache.project_root.join(path)
    }
  }

  fn resolve_module_path(&self, module_root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
      path.to_path_buf()
    } else {
      module_root.join(path)
    }
  }

  fn path_value(&self, path: &Path) -> Value {
    Value::String(path.to_string_lossy().into_owned())
  }

  fn values_at<'a>(root: &'a Value, path: &[&str]) -> Vec<&'a Value> {
    let value = path.iter().try_fold(root, |value, key| value.get(*key));
    match value {
      Some(Value::Array(values)) => values.iter().collect(),
      Some(Value::Null) | None => Vec::new(),
      Some(value) => vec![value],
    }
  }

  fn first_non_null<'a, const N: usize>(values: [Option<&'a Value>; N]) -> Option<&'a Value> {
    values.into_iter().flatten().find(|value| !value.is_null())
  }

  fn is_js_truthy(value: &Value) -> bool {
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
}
