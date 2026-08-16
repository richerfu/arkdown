use std::path::PathBuf;

use napi::bindgen_prelude::{Array, Env, JsObjectValue, Object, Unknown, ValueType};
use napi::JsValue;
use napi::{Error, Result, Status};
use napi_derive::napi;
use serde_json::{Map, Value};

use arkdown_config::{
  BuildScriptBuildOptionValidationRequest, BuildScriptPluginPathResolver,
  BuildScriptRuntimePlanRequest, BuildScriptSchemaValidator, HvigorRuntimeParameterPlanRequest,
};
use arkdown_session::{
  BuildScriptFileInventory, BuildScriptFileInventoryRequest, BuildScriptRuntimeModelLoader,
  BuildScriptRuntimeModelRequest, BuildScriptRuntimePlanUpdate, BuildScriptRuntimeSdkConfig,
  BuildScriptRuntimeSession, HvigorContextMutation, HvigorContextMutationDiagnostic,
  HvigorContextPathOptions,
};

use crate::native_build_config::{
  NativeBuildConfigNapiAdapter, RuntimeBuildConfigOverlay, RuntimeNativeBuildState,
};
use crate::runtime_task::RuntimeTaskSelectionSession as RuntimeTaskSelectionState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBuildScriptProjectView<'a> {
  project_root: &'a std::path::Path,
  project_profile: &'a Value,
  project_package: &'a Value,
  project_npm_package: &'a Value,
  #[serde(skip_serializing_if = "Option::is_none")]
  app_json: &'a Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  build_config_file: &'a Option<PathBuf>,
  fallback_module_roots: &'a [PathBuf],
}

#[napi(object)]
pub struct RuntimeBuildScriptPlanUpdate {
  pub product_name: String,
  pub build_mode_name: String,
  pub module_selection: Option<String>,
  pub properties: Value,
  pub ext_params: Value,
  pub daemon: bool,
  pub sync_version_changed: bool,
}

/// Rust-owned build-script model and plan session. JavaScript keeps only the
/// official Hvigor node identities and forwards their live state after plugin
/// callbacks mutate it.
#[napi]
pub struct RuntimeBuildScriptSession {
  session: BuildScriptRuntimeSession,
  sdk_config: BuildScriptRuntimeSdkConfig,
  hvigor_user_home: Option<PathBuf>,
  compile_sdk_version: Option<String>,
  task_selection: RuntimeTaskSelectionState,
}

#[napi(object)]
pub struct RuntimeOhpmDependencyState {
  pub properties: Value,
  pub ext_params: Value,
  pub project_package: Value,
  pub modules: Value,
  pub package_model_updated: bool,
  pub executable: Option<String>,
}

#[napi(object)]
pub struct RuntimeHvigorSyncState {
  pub config_build_version: Option<Value>,
  pub selected_node_names: Option<Vec<String>>,
  pub project_custom_tasks: Value,
  pub module_custom_tasks: Value,
  pub custom_models: Value,
  pub custom_model_task_info: Value,
}

#[napi(object)]
pub struct RuntimeCommandPlanState {
  pub execution_mode: Option<String>,
  pub sync: bool,
  pub task_names: Vec<String>,
  pub task_model: Value,
}

#[napi(object)]
pub struct RuntimeOhosContextPathOptions {
  pub build_cache_dir: Option<String>,
  pub build_dir: Option<String>,
  pub preview_mode: bool,
  pub local_test: bool,
  pub ohos_test: bool,
}

impl From<RuntimeOhosContextPathOptions> for HvigorContextPathOptions {
  fn from(value: RuntimeOhosContextPathOptions) -> Self {
    Self {
      build_cache_dir: value.build_cache_dir,
      build_dir: value.build_dir,
      preview_mode: value.preview_mode,
      local_test: value.local_test,
      ohos_test: value.ohos_test,
    }
  }
}

#[napi]
impl RuntimeBuildScriptSession {
  #[napi(constructor)]
  pub fn new(
    project_root: String,
    hvigor_user_home: Option<String>,
    config: Object<'_>,
    command_entry_tasks: Vec<String>,
  ) -> Result<Self> {
    let hvigor_user_home = hvigor_user_home.map(PathBuf::from);
    let model = BuildScriptRuntimeModelLoader::new(BuildScriptRuntimeModelRequest {
      project_root: PathBuf::from(project_root),
      hvigor_user_home: hvigor_user_home.clone(),
    })
    .load()
    .map_err(BuildScriptRuntimeModelNapiAdapter::session_model_error)?;
    let config = RuntimeBuildScriptConfigReader::new(config);
    let sdk_config = BuildScriptRuntimeSdkConfig {
      sdk_path: config.path("sdkPath")?,
      ets_loader_path: config.path("etsLoaderPath")?,
      runtime_os: config.string("runtimeOS")?,
      ets_component_version: config.string("etsComponentVersion")?,
      ets_component_release_type: config.string("etsComponentReleaseType")?,
    };
    let configured_product_name = config.string("product")?;
    let product_is_explicit = configured_product_name.is_some();
    let product_name = configured_product_name.unwrap_or_else(|| {
      model
        .project_profile
        .pointer("/app/products/0/name")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string()
    });
    let build_mode_name = config
      .string("mode")?
      .or(config.string("buildMode")?)
      .unwrap_or_else(|| "debug".to_string());
    let device_test = command_entry_tasks
      .iter()
      .any(|task| task == "onDeviceTest");
    let has_rollup_plugin_in_ets_loader = config
      .boolean("hasRollupPluginInEtsLoader")?
      .or(config.boolean("hasRollupPlugin")?);
    let compile_sdk_version = config.value("compileSdkVersion")?;
    let compile_sdk_version_text = compile_sdk_version
      .as_ref()
      .and_then(Value::as_str)
      .map(str::to_string);
    let parameter_request = HvigorRuntimeParameterPlanRequest {
      user_config: model.user_hvigor_config.clone(),
      project_config: model.project_hvigor_config.clone(),
      runtime_config: config.parameter_config()?,
      command_entry_tasks: command_entry_tasks.clone(),
      workspace_dir: std::env::var_os("WORKSPACE_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| model.project_root.clone()),
    };
    let request = BuildScriptRuntimePlanRequest {
      project_root: model.project_root.clone(),
      project_profile: model.project_profile.clone(),
      modules: Vec::new(),
      product_name,
      build_mode_name,
      module_selection: config.string("module")?,
      build_target_name: config.string("buildTargetName")?,
      device_test,
      project_oh_package_exists: model.project_oh_package_exists,
      project_npm_package_exists: model.project_npm_package_exists,
      project_kind: None,
      target: config.string("target")?,
      compile_api_version: config.value("compileApiVersion")?,
      compile_sdk_version,
      compatible_api_version: config.value("compatibleApiVersion")?,
      compatible_sdk_version: config.value("compatibleSdkVersion")?,
      command_entry_tasks,
      properties: Map::new(),
      ext_params: Map::new(),
      start_params: Map::new(),
      ets_loader_path: config.path("etsLoaderPath")?,
      js_loader_path: config.path("jsLoaderPath")?,
      has_rollup_plugin_in_ets_loader,
      has_rollup_plugin_in_js_loader: config.boolean("hasRollupPluginInJsLoader")?,
      sync_version_changed: false,
    };
    let session =
      BuildScriptRuntimeSession::new(model, request, parameter_request, product_is_explicit);
    let cache_enabled = session
      .parameter_plan()
      .core_start_params
      .get("incrementalExecution")
      .and_then(Value::as_bool)
      .unwrap_or(false);
    let task_selection =
      RuntimeTaskSelectionState::new(session.project_model().project_root.clone(), cache_enabled);
    Ok(Self {
      session,
      sdk_config,
      hvigor_user_home,
      compile_sdk_version: compile_sdk_version_text,
      task_selection,
    })
  }

  #[napi]
  pub fn project_model(&self) -> Result<Value> {
    let model = self.session.project_model();
    BuildScriptRuntimeModelNapiAdapter::encode(
      &RuntimeBuildScriptProjectView {
        project_root: &model.project_root,
        project_profile: &model.project_profile,
        project_package: &model.project_package,
        project_npm_package: &model.project_npm_package,
        app_json: &model.app_json,
        build_config_file: &model.build_config_file,
        fallback_module_roots: &model.fallback_module_roots,
      },
      "project runtime model",
    )
  }

  #[napi]
  pub fn module_descriptors(&self) -> Vec<Value> {
    self.session.module_descriptors()
  }

  #[napi]
  pub fn module_descriptor(&self, root: String) -> Option<Value> {
    self
      .session
      .module_descriptor(PathBuf::from(root).as_path())
  }

  #[napi]
  pub fn load_module_descriptor(&mut self, descriptor: Value) -> Result<Value> {
    self
      .session
      .load_module_descriptor(descriptor)
      .map_err(BuildScriptRuntimeModelNapiAdapter::model_error)
  }

  #[napi]
  pub fn replace_module_descriptors(&mut self, descriptors: Vec<Value>) -> Result<Vec<Value>> {
    self
      .session
      .replace_module_descriptors(descriptors)
      .map_err(BuildScriptRuntimeModelNapiAdapter::model_error)
  }

  #[napi(getter)]
  pub fn uses_ohpm(&self) -> bool {
    self.session.uses_ohpm()
  }

  #[napi]
  pub fn effective_compile_api_version(&self) -> u32 {
    self.session.effective_compile_api_version()
  }

  #[napi]
  pub fn selected_product(&self) -> Value {
    self.session.selected_product()
  }

  #[napi]
  pub fn selected_product_bundle_name(&self) -> Option<String> {
    self.session.selected_product_bundle_name()
  }

  #[napi]
  pub fn set_project_build_profile(&mut self, profile: Value) -> Result<Value> {
    Self::context_mutation(self.session.set_context_project_build_profile(profile))
  }

  #[napi]
  pub fn set_project_app_json(&mut self, app_json: Value) -> Result<Value> {
    Self::accepted_context_mutation(self.session.set_context_project_app_json(app_json))
  }

  #[napi]
  pub fn set_module_build_profile(&mut self, root: String, profile: Value) -> Result<Value> {
    Self::accepted_context_mutation(
      self
        .session
        .set_context_module_build_profile(PathBuf::from(root).as_path(), profile),
    )
  }

  #[napi]
  pub fn set_module_json(&mut self, root: String, module_json: Value) -> Result<Value> {
    Self::accepted_context_mutation(
      self
        .session
        .set_context_module_json(PathBuf::from(root).as_path(), module_json),
    )
  }

  #[napi]
  pub fn project_package_field(&self, field: String) -> Option<Value> {
    self.session.project_package_field(&field)
  }

  #[napi]
  pub fn module_package_field(&self, root: String, field: String) -> Option<Value> {
    self
      .session
      .module_package_field(PathBuf::from(root).as_path(), &field)
  }

  #[napi]
  pub fn set_project_package_field(&mut self, field: String, value: Value) -> Result<Value> {
    Self::context_mutation(
      self
        .session
        .set_context_project_package_field(&field, value),
    )
  }

  #[napi]
  pub fn set_module_package_field(
    &mut self,
    root: String,
    field: String,
    value: Value,
  ) -> Result<Value> {
    Self::context_mutation(self.session.set_context_module_package_field(
      PathBuf::from(root).as_path(),
      &field,
      value,
    ))
  }

  #[napi]
  pub fn set_module_version(&mut self, root: String, version: Value) -> Result<Value> {
    Self::context_mutation(
      self
        .session
        .set_context_module_version(PathBuf::from(root).as_path(), version),
    )
  }

  #[napi]
  pub fn dependency_context(&mut self, root: Option<String>) -> Result<Value> {
    let root = root.map(PathBuf::from);
    match self.session.dependency_context(root.as_deref()) {
      Ok(value) => Self::context_value_outcome(value),
      Err(error) => Self::context_diagnostic_outcome(
        if root.is_some() {
          "HVIGOR_CONTEXT_MODULE_NOT_FOUND"
        } else {
          "HVIGOR_CONTEXT_INSPECTION_FAILED"
        },
        error.to_string(),
      ),
    }
  }

  #[napi]
  pub fn all_module_name_hash(&mut self) -> Result<Value> {
    match self.session.all_module_name_hash() {
      Ok(value) => Self::context_value_outcome(value),
      Err(error) => {
        Self::context_diagnostic_outcome("HVIGOR_CONTEXT_INSPECTION_FAILED", error.to_string())
      }
    }
  }

  #[napi]
  pub fn project_paths(&self, options: RuntimeOhosContextPathOptions) -> Result<Value> {
    self
      .session
      .project_context_paths(&options.into())
      .map_err(Self::context_error)
      .and_then(|paths| {
        BuildScriptRuntimeModelNapiAdapter::encode(&paths, "Hvigor project context paths")
      })
  }

  #[napi]
  pub fn module_target_paths(
    &self,
    root: String,
    target_name: String,
    options: RuntimeOhosContextPathOptions,
  ) -> Result<Value> {
    self
      .session
      .module_target_context_paths(PathBuf::from(root).as_path(), &target_name, &options.into())
      .map_err(Self::context_error)
      .and_then(|paths| {
        BuildScriptRuntimeModelNapiAdapter::encode(&paths, "Hvigor module target context paths")
      })
  }

  #[napi]
  pub fn prepare_runtime_task_cycle(
    &mut self,
    root_paths: Vec<String>,
    execution_node_names: Vec<String>,
  ) {
    self
      .task_selection
      .prepare_runtime_cycle(root_paths, execution_node_names);
  }

  #[napi]
  pub fn prepare_command_task_cycle(
    &mut self,
    execution_node_names: Vec<String>,
    command_root_paths: Vec<String>,
    required_boundary_paths: Vec<String>,
  ) {
    self.task_selection.prepare_command_cycle(
      execution_node_names,
      command_root_paths,
      required_boundary_paths,
    );
  }

  #[napi]
  pub fn prepare_direct_task_cycle(&mut self, task_model: Value) -> Result<Vec<String>> {
    let selected_module_names = self
      .session
      .command_snapshot()
      .map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("failed to project direct task selection: {error}"),
        )
      })?
      .module_selections
      .into_iter()
      .map(|selection| selection.module_name)
      .collect();
    let cycle = crate::runtime_task::RuntimeDirectTaskCyclePlanner::from_value(
      self.session.output_target(),
      selected_module_names,
      task_model,
    )
    .map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("invalid direct runtime task model: {error:#}"),
      )
    })?
    .plan();
    self
      .task_selection
      .prepare_direct_cycle(cycle.root_paths.clone(), cycle.execution_node_names);
    Ok(cycle.root_paths)
  }

  #[napi]
  pub fn runtime_task_selection(&self, validate_boundaries: bool) -> Result<Value> {
    self
      .task_selection
      .selection(validate_boundaries)
      .map_err(|error| {
        Error::new(
          Status::GenericFailure,
          format!("ARKDOWN_RUNTIME_TASK_SELECTION_FAILED: {error}"),
        )
      })
  }

  #[napi]
  pub fn clear_runtime_task_selection(&mut self) {
    self.task_selection.clear();
  }

  #[napi]
  pub fn parameter_plan(&self) -> Result<Value> {
    BuildScriptRuntimeModelNapiAdapter::encode(
      self.session.parameter_plan(),
      "Hvigor runtime parameter plan",
    )
  }

  #[napi]
  pub fn discover_build_script_files(&self, roots: Vec<String>) -> Result<Value> {
    let files = BuildScriptFileInventory::new(BuildScriptFileInventoryRequest {
      roots: roots.into_iter().map(PathBuf::from).collect(),
    })
    .discover()
    .map_err(BuildScriptRuntimeModelNapiAdapter::model_error)?;
    BuildScriptRuntimeModelNapiAdapter::encode(&files, "build-script file inventory")
  }

  #[napi]
  pub fn capture_hvigor_overrides(
    &mut self,
    root: String,
    overrides: Option<Value>,
    effects_build_option: Option<Value>,
    source_file: String,
  ) -> Result<Value> {
    let root = PathBuf::from(root);
    let overrides = overrides.and_then(|value| value.as_object().cloned());
    if root == self.session.project_model().project_root {
      if let Some(overrides) = overrides {
        for field in ["appOpt", "signingConfig"] {
          if let Some(value) = overrides.get(field) {
            self.session.merge_project_hvigor_override(field, value);
          }
        }
      }
      return Ok(serde_json::json!({}));
    }

    let exported_build_option = overrides
      .as_ref()
      .and_then(|overrides| overrides.get("buildOption"))
      .cloned();
    if let Some(build_option) = &exported_build_option {
      let request = BuildScriptBuildOptionValidationRequest {
        build_option: build_option.clone(),
        source_file: PathBuf::from(source_file),
        field_path: "config.ohos.overrides.buildOption".to_string(),
      };
      if let Err(diagnostic) = BuildScriptSchemaValidator::validate_request(&request) {
        return Ok(serde_json::json!({ "diagnostic": diagnostic }));
      }
    }

    let captured_build_option = effects_build_option
      .filter(|value| value.as_object().is_some_and(|entries| !entries.is_empty()));
    let build_option = match (captured_build_option, exported_build_option) {
      (Some(captured), Some(exported)) => Some(BuildScriptRuntimeSession::merge_json_values(
        &captured, &exported,
      )),
      (Some(captured), None) => Some(captured),
      (None, Some(exported)) => Some(exported),
      (None, None) => None,
    };
    if let Some(build_option) = &build_option {
      let merged = self
        .session
        .module_hvigor_override(&root)
        .map(|current| BuildScriptRuntimeSession::merge_json_values(current, build_option))
        .unwrap_or_else(|| build_option.clone());
      self.session.set_module_override(&root, Some(merged));
    }
    Ok(match build_option {
      Some(build_option) => serde_json::json!({ "buildOption": build_option }),
      None => serde_json::json!({}),
    })
  }

  #[napi]
  pub fn runtime_property_plan(
    &self,
    properties: Value,
    ext_params: Value,
    core_start_params: Value,
  ) -> Result<Value> {
    let outcome = match self.session.runtime_property_plan(
      Self::object_map(properties, "properties")?,
      Self::object_map(ext_params, "extParams")?,
      Self::object_map(core_start_params, "coreStartParams")?,
    ) {
      Ok(plan) => serde_json::json!({ "plan": plan }),
      Err(diagnostic) => serde_json::json!({ "diagnostic": diagnostic }),
    };
    Ok(outcome)
  }

  #[napi]
  pub fn runtime_path_plan(&self, properties: Value, ext_params: Value) -> Result<Value> {
    let properties = Self::object_map(properties, "properties")?;
    let ext_params = Self::object_map(ext_params, "extParams")?;
    BuildScriptRuntimeModelNapiAdapter::encode(
      &self.session.runtime_path_plan(&properties, &ext_params),
      "Hvigor runtime path plan",
    )
  }

  #[napi]
  pub fn prepare_native_build_config<'env>(
    &self,
    env: Env,
    config: Object<'env>,
    state: RuntimeNativeBuildState,
    process_executable: String,
  ) -> Result<Object<'env>> {
    let properties = Self::object_map(state.properties.clone(), "properties")?;
    let ext_params = Self::object_map(state.ext_params.clone(), "extParams")?;
    let core_start_params = Self::object_map(state.core_start_params.clone(), "coreStartParams")?;
    let mut outcome = Object::new(&env)?;
    if let Some(diagnostic) = self.session.unsupported_build_diagnostic() {
      outcome.set(
        "diagnostic",
        BuildScriptRuntimeModelNapiAdapter::encode(diagnostic, "unsupported build diagnostic")?,
      )?;
      return Ok(outcome);
    }
    let property_plan = match self.session.runtime_property_plan(
      properties.clone(),
      ext_params.clone(),
      core_start_params,
    ) {
      Ok(plan) => plan,
      Err(diagnostic) => {
        outcome.set(
          "diagnostic",
          BuildScriptRuntimeModelNapiAdapter::encode(
            &diagnostic,
            "Hvigor runtime property diagnostic",
          )?,
        )?;
        return Ok(outcome);
      }
    };
    let path_plan = self.session.runtime_path_plan(&properties, &ext_params);
    let module_hvigor_overrides = BuildScriptRuntimeModelNapiAdapter::encode(
      &self.session.module_hvigor_overrides(),
      "module Hvigor overrides",
    )?;
    let runtime_context_overrides = self
      .session
      .runtime_context_overrides()
      .map(|overrides| {
        BuildScriptRuntimeModelNapiAdapter::encode(&overrides, "runtime context overrides")
      })
      .transpose()?;
    let overlay = RuntimeBuildConfigOverlay::from_runtime(
      &state,
      property_plan,
      path_plan,
      self.session.project_hvigor_overrides(),
      &module_hvigor_overrides,
      runtime_context_overrides.as_ref(),
    )?;
    let native_config =
      NativeBuildConfigNapiAdapter::new(env, config, process_executable).prepare(Some(overlay))?;
    outcome.set("config", native_config)?;
    Ok(outcome)
  }

  #[napi]
  pub async fn prepare_hvigor_plugin_workspace(
    &self,
    home_dir: String,
    process_executable: String,
    node_version: String,
  ) -> Result<Value> {
    let model = self.session.project_model();
    if model.project_hvigor_config_path.is_none() {
      return Ok(Value::Null);
    }
    let hvigor_user_home = self.hvigor_user_home.as_ref().ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        "ARKDOWN_HVIGOR_USER_HOME_MISSING: Hvigor dependency workspace requires a user home"
          .to_string(),
      )
    })?;
    let request = serde_json::json!({
      "projectRoot": model.project_root,
      "hvigorUserHome": hvigor_user_home,
      "homeDir": home_dir,
      "processExecutable": process_executable,
      "nodeVersion": node_version,
      "projectConfig": model.project_hvigor_config,
      "userConfig": model.user_hvigor_config,
    });
    let runner =
      crate::hvigor_dependency_workspace::HvigorPluginWorkspaceRunner::from_value(request)
        .map_err(|error| {
          Error::new(
            Status::InvalidArg,
            format!("invalid hvigor plugin workspace request: {error:#}"),
          )
        })?;
    runner.response_value().await.map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to prepare hvigor plugin workspace: {error:#}"),
      )
    })
  }

  #[napi]
  pub async fn prepare_ohpm_dependency_workspace(
    &self,
    state: RuntimeOhpmDependencyState,
  ) -> Result<Value> {
    let properties = Self::object_map(state.properties, "properties")?;
    let ext_params = Self::object_map(state.ext_params, "extParams")?;
    let path_plan = self.session.runtime_path_plan(&properties, &ext_params);
    let request = serde_json::json!({
      "enabled": self.session.uses_ohpm(),
      "cleanOnly": self.session.command_is_clean_only(),
      "projectRoot": self.session.project_model().project_root,
      "cacheDir": path_plan.hvigor_cache_dir,
      "targetName": self.session.build_target_name().unwrap_or_default(),
      "projectProperties": state.project_package.get("properties").cloned().unwrap_or(Value::Null),
      "projectPackage": state.project_package,
      "parameterFileOverride": ext_params.get("parameterFile").and_then(Value::as_str),
      "modules": state.modules,
      "packageModelUpdated": state.package_model_updated,
      "enforce": ext_params.get("enforce-ohpm").and_then(Value::as_str) == Some("true"),
      "executable": state.executable,
    });
    let runner = crate::ohpm_dependency_map::OhpmDependencyMapRunner::from_value(request).map_err(
      |error| {
        Error::new(
          Status::InvalidArg,
          format!("invalid OHPM dependency-map request: {error:#}"),
        )
      },
    )?;
    runner.response_value().await.map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to prepare OHPM dependency workspace: {error:#}"),
      )
    })
  }

  #[napi]
  pub fn hvigor_sync_version_changed(&self, properties: Value, ext_params: Value) -> Result<bool> {
    let request = self.hvigor_cache_state_request(properties, ext_params)?;
    crate::hvigor_sync::HvigorCacheState::from_value(request)
      .map(|state| state.version_changed())
      .map_err(|error| Error::new(Status::GenericFailure, format!("{error:#}")))
  }

  #[napi]
  pub async fn write_hvigor_build_meta(&self, properties: Value, ext_params: Value) -> Result<()> {
    let request = self.hvigor_cache_state_request(properties, ext_params)?;
    let state = crate::hvigor_sync::HvigorCacheState::from_value(request)
      .map_err(|error| Error::new(Status::InvalidArg, format!("{error:#}")))?;
    state
      .write_build_meta()
      .await
      .map_err(|error| Error::new(Status::GenericFailure, format!("{error:#}")))
  }

  #[napi]
  pub async fn write_hvigor_sync_model(
    &self,
    state: RuntimeHvigorSyncState,
    ext_params: Value,
  ) -> Result<()> {
    let ext_params = Self::object_map(ext_params, "extParams")?;
    let snapshot = self.session.sync_snapshot().map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("failed to project Hvigor sync model: {error}"),
      )
    })?;
    let mut request = serde_json::to_value(snapshot).map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to serialize Hvigor sync snapshot: {error}"),
      )
    })?;
    let properties = request
      .get("properties")
      .cloned()
      .unwrap_or_else(RuntimeBuildScriptConfigReader::empty_object);
    let property_map = Self::object_map(properties.clone(), "sync properties")?;
    let path_plan = self.session.runtime_path_plan(&property_map, &ext_params);
    let cache = self.hvigor_cache_state_request(properties, Value::Object(ext_params.clone()))?;
    let module_custom_tasks = state.module_custom_tasks.as_object().ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        "ARKDOWN_INVALID_HVIGOR_SYNC_REQUEST: moduleCustomTasks must be an object".to_string(),
      )
    })?;
    {
      let request = request.as_object_mut().ok_or_else(|| {
        Error::new(
          Status::InvalidArg,
          "ARKDOWN_INVALID_HVIGOR_SYNC_REQUEST: sync model request must be an object".to_string(),
        )
      })?;
      request.insert("cache".to_string(), cache);
      if let Some(build_version) = state.config_build_version {
        request.insert("configBuildVersion".to_string(), build_version);
      }
      if let Some(build_version) = ext_params.get("buildVersion") {
        request.insert("parameterBuildVersion".to_string(), build_version.clone());
      }
      if let Some(configured_build_dir) = path_plan.configured_build_dir {
        request.insert(
          "configuredBuildDir".to_string(),
          Value::String(configured_build_dir),
        );
      }
      if let Some(selected_node_names) = state.selected_node_names {
        request.insert(
          "selectedNodeNames".to_string(),
          serde_json::to_value(selected_node_names).map_err(|error| {
            Error::new(
              Status::GenericFailure,
              format!("failed to serialize selected Hvigor sync nodes: {error}"),
            )
          })?,
        );
      }
      request.insert("projectCustomTasks".to_string(), state.project_custom_tasks);
      request.insert("customModels".to_string(), state.custom_models);
      request.insert(
        "customModelTaskInfo".to_string(),
        state.custom_model_task_info,
      );
      request.insert(
        "projectHvigorOverrides".to_string(),
        self.session.project_hvigor_overrides().clone(),
      );
      for module in request
        .get_mut("modules")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
      {
        let Some(module) = module.as_object_mut() else {
          continue;
        };
        let tasks = module
          .get("name")
          .and_then(Value::as_str)
          .and_then(|name| module_custom_tasks.get(name))
          .cloned()
          .unwrap_or_else(|| Value::Array(Vec::new()));
        module.insert("customTasks".to_string(), tasks);
      }
    }
    let writer = crate::hvigor_sync::HvigorSyncModelWriter::from_value(request)
      .map_err(|error| Error::new(Status::InvalidArg, format!("{error:#}")))?;
    writer
      .write()
      .await
      .map_err(|error| Error::new(Status::GenericFailure, format!("{error:#}")))
  }

  #[napi]
  pub fn runtime_command_plan(&self, state: RuntimeCommandPlanState) -> Result<Value> {
    let snapshot = self.session.command_snapshot().map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("failed to project Hvigor command plan: {error}"),
      )
    })?;
    let mut request = serde_json::to_value(snapshot).map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to serialize Hvigor command snapshot: {error}"),
      )
    })?;
    let request = request.as_object_mut().ok_or_else(|| {
      Error::new(
        Status::GenericFailure,
        "ARKDOWN_INVALID_RUNTIME_COMMAND_SNAPSHOT: command snapshot must be an object".to_string(),
      )
    })?;
    request.insert("sync".to_string(), Value::Bool(state.sync));
    request.insert(
      "taskNames".to_string(),
      serde_json::to_value(state.task_names).map_err(|error| {
        Error::new(
          Status::GenericFailure,
          format!("failed to serialize live Hvigor command entry tasks: {error}"),
        )
      })?,
    );
    if let Some(execution_mode) = state.execution_mode {
      request.insert("executionMode".to_string(), Value::String(execution_mode));
    }
    let task_model = state.task_model.as_object().ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        "ARKDOWN_INVALID_RUNTIME_TASK_MODEL: taskModel must be an object".to_string(),
      )
    })?;
    request.extend(task_model.clone());
    crate::runtime_task::RuntimeCommandPlanner::response_value(Value::Object(request.clone()))
      .map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("invalid runtime command plan request: {error:#}"),
        )
      })
  }

  #[napi]
  pub async fn run_hvigor_clean(&self, node_root: String, project_task: bool) -> Result<Value> {
    let snapshot = self
      .session
      .clean_snapshot(PathBuf::from(node_root).as_path(), project_task)
      .map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("failed to project Hvigor clean request: {error}"),
        )
      })?;
    let mut request = serde_json::to_value(snapshot).map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to serialize Hvigor clean snapshot: {error}"),
      )
    })?;
    if let Some(sdk_path) = &self.sdk_config.sdk_path {
      request
        .as_object_mut()
        .expect("clean snapshot serializes as an object")
        .insert(
          "sdkPath".to_string(),
          Value::String(sdk_path.to_string_lossy().into_owned()),
        );
    }
    let runner =
      crate::hvigor_maintenance::HvigorCleanRunner::from_value(request).map_err(|error| {
        Error::new(
          Status::InvalidArg,
          format!("invalid hvigor clean request: {error:#}"),
        )
      })?;
    runner
      .response_value()
      .await
      .map_err(|error| Error::new(Status::GenericFailure, format!("{error:#}")))
  }

  #[napi]
  pub fn set_module_override(&mut self, root: String, value: Option<Value>) {
    self
      .session
      .set_module_override(PathBuf::from(root).as_path(), value);
  }

  #[napi]
  pub fn upsert_module(&mut self, node: Object<'_>) -> Result<()> {
    let node = RuntimeBuildScriptModuleReader::new(node);
    if node.boolean("isProject")?.unwrap_or(false) {
      self.session.set_project_kind(node.string("kind")?);
      return Ok(());
    }
    self
      .session
      .upsert_module(arkdown_config::BuildScriptRuntimeModulePlanRequest {
        name: node.required_string("name")?,
        root: PathBuf::from(node.required_string("root")?),
        descriptor: node
          .value("descriptor")?
          .unwrap_or_else(RuntimeBuildScriptConfigReader::empty_object),
        build_profile: node
          .value("moduleProfile")?
          .unwrap_or_else(RuntimeBuildScriptConfigReader::empty_object),
        build_script_override: None,
        kind: node.string("kind")?,
        package: node
          .value("modulePackage")?
          .unwrap_or_else(RuntimeBuildScriptConfigReader::empty_object),
      });
    Ok(())
  }

  #[napi]
  pub fn invalidate(&mut self) {
    self.session.invalidate();
  }

  #[napi]
  pub fn plan(&mut self, update: RuntimeBuildScriptPlanUpdate) -> Result<Value> {
    let properties = Self::object_map(update.properties, "properties")?;
    let ext_params = Self::object_map(update.ext_params, "extParams")?;
    let mut start_params = Map::new();
    start_params.insert("daemon".to_string(), Value::Bool(update.daemon));
    let outcome = match self.session.plan(BuildScriptRuntimePlanUpdate {
      product_name: update.product_name,
      build_mode_name: update.build_mode_name,
      module_selection: update.module_selection,
      properties,
      ext_params,
      start_params,
      sync_version_changed: update.sync_version_changed,
    }) {
      Ok(plan) => serde_json::json!({ "plan": plan }),
      Err(diagnostic) => serde_json::json!({ "diagnostic": diagnostic }),
    };
    Ok(outcome)
  }

  #[napi]
  pub fn module_plan(&self, root: String) -> Option<Value> {
    self
      .session
      .module_plan(PathBuf::from(root).as_path())
      .and_then(|module| serde_json::to_value(module).ok())
  }

  #[napi]
  pub fn sdk_details(&mut self) -> Result<Value> {
    let details = self
      .session
      .sdk_details(&self.sdk_config)
      .map_err(BuildScriptRuntimeModelNapiAdapter::sdk_error)?;
    BuildScriptRuntimeModelNapiAdapter::encode(details, "Hvigor SDK details")
  }

  fn context_mutation(
    result: std::result::Result<HvigorContextMutation, HvigorContextMutationDiagnostic>,
  ) -> Result<Value> {
    match result {
      Ok(mutation) => Self::accepted_context_mutation(mutation),
      Err(diagnostic) => BuildScriptRuntimeModelNapiAdapter::encode(
        &serde_json::json!({ "diagnostic": diagnostic }),
        "Hvigor context mutation diagnostic",
      ),
    }
  }

  fn accepted_context_mutation(mutation: HvigorContextMutation) -> Result<Value> {
    BuildScriptRuntimeModelNapiAdapter::encode(&mutation, "Hvigor context mutation")
  }

  fn context_value_outcome(value: impl serde::Serialize) -> Result<Value> {
    BuildScriptRuntimeModelNapiAdapter::encode(
      &serde_json::json!({ "value": value }),
      "Hvigor context snapshot",
    )
  }

  fn context_diagnostic_outcome(code: &str, message: impl Into<String>) -> Result<Value> {
    BuildScriptRuntimeModelNapiAdapter::encode(
      &serde_json::json!({
        "diagnostic": {
          "code": code,
          "message": message.into(),
        },
      }),
      "Hvigor context diagnostic",
    )
  }

  fn context_error(error: impl std::fmt::Display) -> Error {
    Error::new(
      Status::GenericFailure,
      format!("ARKDOWN_OHOS_CONTEXT_SESSION_FAILED: {error}"),
    )
  }

  fn object_map(value: Value, label: &str) -> Result<Map<String, Value>> {
    value.as_object().cloned().ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_PLAN_UPDATE: {label} must be an object"),
      )
    })
  }

  fn hvigor_cache_state_request(&self, properties: Value, ext_params: Value) -> Result<Value> {
    let properties = Self::object_map(properties, "properties")?;
    let ext_params = Self::object_map(ext_params, "extParams")?;
    let path_plan = self.session.runtime_path_plan(&properties, &ext_params);
    Ok(serde_json::json!({
      "projectRoot": self.session.project_model().project_root,
      "configuredCacheDir": path_plan.configured_cache_dir,
      "sdkPath": self.sdk_config.sdk_path,
      "compileSdkVersion": self.compile_sdk_version,
    }))
  }
}

struct RuntimeBuildScriptConfigReader<'env> {
  object: Object<'env>,
}

impl<'env> RuntimeBuildScriptConfigReader<'env> {
  const PARAMETER_FIELDS: [&'static str; 16] = [
    "prop",
    "configOverrides",
    "cache",
    "parallel",
    "enableBuildScriptTypeCheck",
    "typeCheck",
    "stacktrace",
    "daemon",
    "analyze",
    "optimizationStrategy",
    "logLevel",
    "hotCompile",
    "hotReloadBuild",
    "customizedParam",
    "hvigorProperties",
    "extParams",
  ];

  fn new(object: Object<'env>) -> Self {
    Self { object }
  }

  fn value(&self, key: &str) -> Result<Option<Value>> {
    self.object.get(key)
  }

  fn string(&self, key: &str) -> Result<Option<String>> {
    let Some(value) = self.object.get::<Unknown<'env>>(key)? else {
      return Ok(None);
    };
    match value.get_type()? {
      ValueType::Undefined | ValueType::Null => Ok(None),
      ValueType::String => unsafe { value.cast::<String>() }.map(Some),
      _ => Err(Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_CONFIG: {key} must be a string"),
      )),
    }
  }

  fn boolean(&self, key: &str) -> Result<Option<bool>> {
    let Some(value) = self.object.get::<Unknown<'env>>(key)? else {
      return Ok(None);
    };
    match value.get_type()? {
      ValueType::Undefined | ValueType::Null => Ok(None),
      ValueType::Boolean => unsafe { value.cast::<bool>() }.map(Some),
      _ => Err(Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_CONFIG: {key} must be a boolean"),
      )),
    }
  }

  fn path(&self, key: &str) -> Result<Option<PathBuf>> {
    self.string(key).map(|value| value.map(PathBuf::from))
  }

  fn parameter_config(&self) -> Result<Value> {
    let mut entries = Map::new();
    for key in Self::PARAMETER_FIELDS {
      if !self.object.has_own_property(key)? {
        continue;
      }
      if let Some(value) = self.object.get::<Value>(key)? {
        entries.insert(key.to_string(), value);
      }
    }
    Ok(Value::Object(entries))
  }

  fn empty_object() -> Value {
    Value::Object(Map::new())
  }
}

struct RuntimeBuildScriptModuleReader<'env> {
  reader: RuntimeBuildScriptConfigReader<'env>,
}

impl<'env> RuntimeBuildScriptModuleReader<'env> {
  fn new(object: Object<'env>) -> Self {
    Self {
      reader: RuntimeBuildScriptConfigReader::new(object),
    }
  }

  fn value(&self, key: &str) -> Result<Option<Value>> {
    self.reader.value(key)
  }

  fn string(&self, key: &str) -> Result<Option<String>> {
    self.reader.string(key)
  }

  fn boolean(&self, key: &str) -> Result<Option<bool>> {
    self.reader.boolean(key)
  }

  fn required_string(&self, key: &str) -> Result<String> {
    self.string(key)?.ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_MODULE: {key} must be a string"),
      )
    })
  }
}

pub(crate) struct BuildScriptRuntimeModelNapiAdapter;

impl BuildScriptRuntimeModelNapiAdapter {
  fn encode<T>(value: &T, label: &str) -> Result<Value>
  where
    T: serde::Serialize,
  {
    serde_json::to_value(value).map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("ARKDOWN_BUILD_SCRIPT_RUNTIME_MODEL_SERIALIZE_FAILED: failed to serialize {label}: {error}"),
      )
    })
  }

  fn model_error(error: arkdown_session::BuildScriptRuntimeModelError) -> Error {
    Error::new(
      Status::GenericFailure,
      format!("ARKDOWN_BUILD_SCRIPT_RUNTIME_MODEL_FAILED: {error}"),
    )
  }

  fn sdk_error(error: arkdown_session::HvigorSdkDetailsError) -> Error {
    Error::new(
      Status::GenericFailure,
      format!("ARKDOWN_HVIGOR_SDK_DETAILS_FAILED: {error}"),
    )
  }

  fn session_model_error(error: arkdown_session::BuildScriptRuntimeModelError) -> Error {
    if let arkdown_session::BuildScriptRuntimeModelError::Schema(diagnostic) = error {
      let encoded = serde_json::to_string(&diagnostic).unwrap_or_else(|_| {
        "{\"code\":\"INVALID_HVIGOR_MODEL_FILE\",\"message\":\"failed to serialize build-script diagnostic\"}"
          .to_string()
      });
      return Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_BUILD_SCRIPT_RUNTIME_DIAGNOSTIC:{encoded}"),
      );
    }
    Self::model_error(error)
  }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildScriptPluginPathPlan {
  hvigor_plugin_files: Vec<std::path::PathBuf>,
  compile_plugin_files: Vec<std::path::PathBuf>,
}

pub(crate) struct BuildScriptPluginPathNapiAdapter;

impl BuildScriptPluginPathNapiAdapter {
  pub(crate) fn collect(
    value: Unknown<'_>,
    base_dir: String,
    include_compile_plugin_file: bool,
  ) -> Result<Value> {
    let resolver = BuildScriptPluginPathResolver::new(base_dir);
    let hvigor_plugin_files = Self::collect_fields(
      value,
      &[
        &["hvigorPluginFile"],
        &["arkOptions", "hvigorPluginFile"],
        &["buildOption", "arkOptions", "hvigorPluginFile"],
      ],
      &resolver,
    )?;
    let mut compile_fields: Vec<&[&str]> = vec![
      &["compilePluginPath"],
      &["arkOptions", "compilePluginPath"],
      &["buildOption", "arkOptions", "compilePluginPath"],
    ];
    if include_compile_plugin_file {
      compile_fields.extend([
        &["compilePluginFile"][..],
        &["arkOptions", "compilePluginFile"][..],
        &["buildOption", "arkOptions", "compilePluginFile"][..],
      ]);
    }
    let compile_plugin_files = Self::collect_fields(value, &compile_fields, &resolver)?;
    serde_json::to_value(&BuildScriptPluginPathPlan {
      hvigor_plugin_files,
      compile_plugin_files,
    })
    .map_err(|error| {
      Error::new(
        Status::GenericFailure,
        format!("failed to serialize build-script plugin path plan: {error}"),
      )
    })
  }

  pub(crate) fn resolve(candidate: String, base_dir: String) -> Option<String> {
    BuildScriptPluginPathResolver::new(base_dir)
      .resolve(&candidate)
      .map(|path| path.to_string_lossy().into_owned())
  }

  fn collect_fields(
    value: Unknown<'_>,
    fields: &[&[&str]],
    resolver: &BuildScriptPluginPathResolver,
  ) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for field in fields {
      let Some(value) = Self::nested_property(value, field)? else {
        continue;
      };
      for candidate in Self::string_values(value)? {
        if let Some(file) = resolver.resolve(&candidate) {
          files.push(file);
        }
      }
    }
    Ok(files)
  }

  fn nested_property<'env>(
    mut value: Unknown<'env>,
    fields: &[&str],
  ) -> Result<Option<Unknown<'env>>> {
    for field in fields {
      if value.get_type()? != ValueType::Object || value.is_array()? {
        return Ok(None);
      }
      let object = unsafe { value.cast::<Object<'env>>()? };
      let Some(property) = object.get::<Unknown<'env>>(field)? else {
        return Ok(None);
      };
      value = property;
    }
    Ok(Some(value))
  }

  fn string_values(value: Unknown<'_>) -> Result<Vec<String>> {
    match value.get_type()? {
      ValueType::String => Ok(vec![unsafe { value.cast::<String>()? }]),
      ValueType::Object if value.is_array()? => {
        let array = unsafe { value.cast::<Array<'_>>()? };
        let mut values = Vec::with_capacity(array.len() as usize);
        for index in 0..array.len() {
          let Some(value) = array.get::<Unknown<'_>>(index)? else {
            continue;
          };
          if value.get_type()? == ValueType::String {
            values.push(unsafe { value.cast::<String>()? });
          }
        }
        Ok(values)
      }
      _ => Ok(Vec::new()),
    }
  }
}
