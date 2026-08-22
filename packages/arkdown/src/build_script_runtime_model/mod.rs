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
  project_hvigor_config: &'a Value,
  user_hvigor_config: &'a Value,
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

#[napi(object)]
pub struct RuntimeOhpmDependencyState {
  pub properties: Value,
  pub ext_params: Value,
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
pub struct RuntimeStandaloneCommandState {
  pub command_kind: String,
  pub node_root: String,
  pub node_name: String,
  pub project_task: bool,
  pub node_kind: Option<Value>,
  pub tasks: Value,
  pub module: Option<Value>,
  pub json: bool,
  pub resolve: bool,
}

#[napi(object)]
pub struct RuntimeOhosContextPathOptions {
  pub build_cache_dir: Option<String>,
  pub build_dir: Option<String>,
  pub preview_mode: bool,
  pub local_test: bool,
  pub ohos_test: bool,
}

#[napi(object)]
pub struct RuntimeOhosContextReadRequest {
  pub operation: String,
  pub root: Option<String>,
  pub argument: Option<Value>,
  pub build_mode: String,
  pub debug_line: bool,
  pub path_options: RuntimeOhosContextPathOptions,
}

#[napi(object)]
pub struct RuntimeOhosContextWriteRequest {
  pub operation: String,
  pub root: Option<String>,
  pub value: Value,
}

#[derive(Clone, Copy)]
enum RuntimeOhosContextReadOperation {
  ProjectName,
  ProjectPath,
  BuildRootPath,
  BuildProductOutputPath,
  BuildMode,
  BuildProfile,
  CurrentProduct,
  AppJson,
  SdkDetails,
  DebugLine,
  FastBuildApp,
  AppWithSignedPkg,
  DependencyInfo,
  RemoteHspDependencyInfo,
  AllModuleNameHash,
  Dependencies,
  DevDependencies,
  DynamicDependencies,
  Overrides,
  ModuleName,
  ModulePath,
  ModuleType,
  BuildProductRootPath,
  Targets,
  TargetArtifactNames,
  TargetArtifactName,
  ModuleJson,
  Version,
}

impl RuntimeOhosContextReadOperation {
  fn parse(value: &str) -> Result<Self> {
    match value {
      "getProjectName" => Ok(Self::ProjectName),
      "getProjectPath" => Ok(Self::ProjectPath),
      "getBuildRootPath" => Ok(Self::BuildRootPath),
      "getBuildProductOutputPath" => Ok(Self::BuildProductOutputPath),
      "getBuildMode" => Ok(Self::BuildMode),
      "getBuildProfileOpt" => Ok(Self::BuildProfile),
      "getCurrentProduct" => Ok(Self::CurrentProduct),
      "getAppJsonOpt" => Ok(Self::AppJson),
      "getSdkDetails" => Ok(Self::SdkDetails),
      "isDebugLineEnable" => Ok(Self::DebugLine),
      "getFastBuildApp" => Ok(Self::FastBuildApp),
      "getAppWithSignedPkg" => Ok(Self::AppWithSignedPkg),
      "getOhpmDependencyInfo" => Ok(Self::DependencyInfo),
      "getOhpmRemoteHspDependencyInfo" => Ok(Self::RemoteHspDependencyInfo),
      "getAllModuleNameHash" => Ok(Self::AllModuleNameHash),
      "getDependenciesOpt" => Ok(Self::Dependencies),
      "getDevDependenciesOpt" => Ok(Self::DevDependencies),
      "getDynamicDependenciesOpt" => Ok(Self::DynamicDependencies),
      "getOverrides" => Ok(Self::Overrides),
      "getModuleName" => Ok(Self::ModuleName),
      "getModulePath" => Ok(Self::ModulePath),
      "getModuleType" => Ok(Self::ModuleType),
      "getBuildProductRootPath" => Ok(Self::BuildProductRootPath),
      "targets" => Ok(Self::Targets),
      "getTargetArtifactNames" => Ok(Self::TargetArtifactNames),
      "getTargetArtifactName" => Ok(Self::TargetArtifactName),
      "getModuleJsonOpt" => Ok(Self::ModuleJson),
      "getVersion" => Ok(Self::Version),
      _ => Err(Error::new(
        Status::InvalidArg,
        format!("unknown OHOS context read operation {value:?}"),
      )),
    }
  }

  const fn package_field(self) -> Option<&'static str> {
    match self {
      Self::Dependencies => Some("dependencies"),
      Self::DevDependencies => Some("devDependencies"),
      Self::DynamicDependencies => Some("dynamicDependencies"),
      Self::Overrides => Some("overrides"),
      Self::Version => Some("version"),
      _ => None,
    }
  }
}

#[derive(Clone, Copy)]
enum RuntimeOhosContextWriteOperation {
  BuildProfile,
  AppJson,
  Dependencies,
  DevDependencies,
  DynamicDependencies,
  Overrides,
  ModuleJson,
  Version,
}

impl RuntimeOhosContextWriteOperation {
  fn parse(value: &str) -> Result<Self> {
    match value {
      "setBuildProfileOpt" => Ok(Self::BuildProfile),
      "setAppJsonOpt" => Ok(Self::AppJson),
      "setDependenciesOpt" => Ok(Self::Dependencies),
      "setDevDependenciesOpt" => Ok(Self::DevDependencies),
      "setDynamicDependenciesOpt" => Ok(Self::DynamicDependencies),
      "setOverrides" => Ok(Self::Overrides),
      "setModuleJsonOpt" => Ok(Self::ModuleJson),
      "setVersion" => Ok(Self::Version),
      _ => Err(Error::new(
        Status::InvalidArg,
        format!("unknown OHOS context write operation {value:?}"),
      )),
    }
  }

  const fn package_field(self) -> Option<&'static str> {
    match self {
      Self::Dependencies => Some("dependencies"),
      Self::DevDependencies => Some("devDependencies"),
      Self::DynamicDependencies => Some("dynamicDependencies"),
      Self::Overrides => Some("overrides"),
      _ => None,
    }
  }
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

struct RuntimeOhosContextNapiAdapter<'a> {
  runtime: &'a mut RuntimeBuildScriptSession,
  root: Option<PathBuf>,
  argument: Option<Value>,
  build_mode: String,
  debug_line: bool,
  path_options: HvigorContextPathOptions,
}

mod ohos_context;
mod readers;
mod session;

use readers::{
  BuildScriptPluginPathNapiAdapter, BuildScriptRuntimeModelNapiAdapter,
  RuntimeBuildScriptConfigReader, RuntimeBuildScriptModuleReader,
};

pub use session::RuntimeBuildScriptSession;
