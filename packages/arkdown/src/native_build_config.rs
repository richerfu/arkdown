use std::path::PathBuf;

use arkdown_config::HvigorRuntimePropertyPlan;
use arkdown_path::lexical_normalize;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;

use arkdown_session::BuildScriptRuntimePathPlan;

#[napi(object)]
pub struct RuntimeNativeBuildState {
  pub properties: Value,
  pub ext_params: Value,
  pub core_start_params: Value,
  pub default_cache: bool,
  pub mode: String,
}

pub struct RuntimeBuildConfigOverlay {
  pub default_cache: Option<bool>,
  pub mode: Option<String>,
  pub enable_sign_task: Option<bool>,
  pub fast_build_app: Option<bool>,
  pub ohos_debug_asan: Option<bool>,
  pub ohos_enable_hwasan: Option<bool>,
  pub ohos_enable_tsan: Option<bool>,
  pub ohos_enable_ubsan: Option<bool>,
  pub ohos_enable_binxo: Option<bool>,
  pub project_hvigor_overrides_json: Option<String>,
  pub module_hvigor_overrides_json: Option<String>,
  pub runtime_context_overrides_json: Option<String>,
  pub pack_compress_level: Option<u32>,
  pub align_device_types: Option<Vec<String>>,
  pub collect_debug_symbol: Option<bool>,
  pub ui_transform_optimization: Option<bool>,
  pub integrated_hsp_use_get_current_bundle_name: Option<bool>,
  pub byte_code_har_integrated_optimization: Option<bool>,
  pub keep_dependency: Option<bool>,
  pub parameter_file_override: Option<String>,
  pub hvigor_runtime_properties_json: Option<String>,
  pub hvigor_cache_dir: Option<String>,
}

impl RuntimeBuildConfigOverlay {
  pub fn from_runtime(
    state: &RuntimeNativeBuildState,
    fast_build_app: bool,
    property_plan: HvigorRuntimePropertyPlan,
    path_plan: BuildScriptRuntimePathPlan,
    project_hvigor_overrides: &Value,
    module_hvigor_overrides: &Value,
    runtime_context_overrides: Option<&Value>,
  ) -> napi::Result<Self> {
    let runtime_properties = property_plan.runtime_properties;
    let native_properties = property_plan.native_properties;
    Ok(Self {
      default_cache: Some(state.default_cache),
      mode: Some(state.mode.clone()),
      enable_sign_task: Some(native_properties.enable_sign_task),
      fast_build_app: Some(fast_build_app),
      ohos_debug_asan: Self::optional_bool(&runtime_properties, "asanConfig"),
      ohos_enable_hwasan: Self::optional_bool(&runtime_properties, "hwasanConfig"),
      ohos_enable_tsan: Self::optional_bool(&runtime_properties, "tsanConfig"),
      ohos_enable_ubsan: Self::optional_bool(&runtime_properties, "ubsanConfig"),
      ohos_enable_binxo: Self::optional_bool(&runtime_properties, "binxoConfig"),
      project_hvigor_overrides_json: Self::serialize_nonempty(
        project_hvigor_overrides,
        "project hvigor overrides",
      )?,
      module_hvigor_overrides_json: Self::serialize_nonempty(
        module_hvigor_overrides,
        "module hvigor overrides",
      )?,
      runtime_context_overrides_json: runtime_context_overrides
        .filter(|value| !value.is_null())
        .map(|value| Self::serialize(value, "runtime context overrides"))
        .transpose()?,
      pack_compress_level: native_properties.pack_compress_level,
      align_device_types: Self::string_array(native_properties.align_device_types)?,
      collect_debug_symbol: native_properties.collect_debug_symbol,
      ui_transform_optimization: Some(native_properties.ui_transform_optimization),
      integrated_hsp_use_get_current_bundle_name: Some(
        native_properties.integrated_hsp_use_get_current_bundle_name,
      ),
      byte_code_har_integrated_optimization: Some(
        native_properties.byte_code_har_integrated_optimization,
      ),
      keep_dependency: Some(native_properties.keep_dependency),
      parameter_file_override: path_plan.parameter_file_override,
      hvigor_runtime_properties_json: Some(Self::serialize(
        &Value::Object(runtime_properties),
        "hvigor runtime properties",
      )?),
      hvigor_cache_dir: Some(path_plan.hvigor_cache_dir.to_string_lossy().into_owned()),
    })
  }

  fn optional_bool(values: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    values.get(key).and_then(Value::as_bool)
  }

  fn string_array(value: Value) -> napi::Result<Option<Vec<String>>> {
    let Some(values) = value.as_array() else {
      return Ok(None);
    };
    if values.is_empty() {
      return Ok(None);
    }
    values
      .iter()
      .map(|value| {
        value.as_str().map(str::to_string).ok_or_else(|| {
          Error::new(
            Status::InvalidArg,
            "ohos.align.deviceTypes must contain only strings".to_string(),
          )
        })
      })
      .collect::<napi::Result<Vec<_>>>()
      .map(Some)
  }

  fn serialize_nonempty(value: &Value, label: &str) -> napi::Result<Option<String>> {
    match value.as_object() {
      Some(entries) if entries.is_empty() => Ok(None),
      _ => Self::serialize(value, label).map(Some),
    }
  }

  fn serialize(value: &Value, label: &str) -> napi::Result<String> {
    serde_json::to_string(value).map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("failed to serialize {label}: {error}"),
      )
    })
  }
}

/// V8 bridge for the Rust-owned native build request boundary. Arbitrary
/// extension values retain identity, while plugin/runtime-only fields cannot
/// leak into the typed native command.
pub struct NativeBuildConfigNapiAdapter<'env> {
  env: Env,
  config: Object<'env>,
  process_executable: String,
}

impl<'env> NativeBuildConfigNapiAdapter<'env> {
  const GLUE_FIELDS: [&'static str; 25] = [
    "compilePluginPath",
    "hvigorPluginFile",
    "tasks",
    "task",
    "executionMode",
    "prop",
    "configOverrides",
    "hvigorProperties",
    "extParams",
    "sync",
    "stacktrace",
    "typeCheck",
    "enableBuildScriptTypeCheck",
    "parallel",
    "daemon",
    "generateBuildProfile",
    "analyze",
    "optimizationStrategy",
    "hotCompile",
    "hotReloadBuild",
    "nodeHome",
    "maxOldSpaceSize",
    "maxSemiSpaceSize",
    "xmx",
    "logLevel",
  ];
  const RUNTIME_OWNED_FIELDS: [&'static str; 15] = [
    "projectHvigorOverridesJson",
    "moduleHvigorOverridesJson",
    "runtimeContextOverridesJson",
    "runtimeTaskInputsJson",
    "nativeTaskRootsJson",
    "uiTransformOptimization",
    "integratedHspUseGetCurrentBundleName",
    "byteCodeHarIntegratedOptimization",
    "keepDependency",
    "parameterFileOverride",
    "runtimeNodeExecutable",
    "enableSignTask",
    "fastBuildApp",
    "hvigorRuntimePropertiesJson",
    "hvigorCacheDir",
  ];

  pub fn new(env: Env, config: Object<'env>, process_executable: String) -> Self {
    Self {
      env,
      config,
      process_executable,
    }
  }

  pub fn prepare(&self, overlay: Option<RuntimeBuildConfigOverlay>) -> napi::Result<Object<'env>> {
    let mut output = Object::new(&self.env)?;
    for key in Object::keys(&self.config)? {
      if !self.config.has_own_property(&key)? || self.is_private_field(&key) {
        continue;
      }
      if let Some(value) = self.config.get::<Unknown<'env>>(&key)? {
        output.set(&key, value)?;
      }
    }
    output.set("runtimeNodeExecutable", self.runtime_node_executable()?)?;
    if let Some(overlay) = overlay {
      self.apply_overlay(&mut output, overlay)?;
    }
    Ok(output)
  }

  fn is_private_field(&self, key: &str) -> bool {
    Self::GLUE_FIELDS.contains(&key) || Self::RUNTIME_OWNED_FIELDS.contains(&key)
  }

  fn runtime_node_executable(&self) -> napi::Result<String> {
    let Some(node_home) = self.config.get::<String>("nodeHome")? else {
      return Ok(self.process_executable.clone());
    };
    let home = PathBuf::from(node_home);
    let absolute_home = if home.is_absolute() {
      home
    } else {
      std::env::current_dir()
        .map_err(|error| napi::Error::from_reason(format!("resolve Node home: {error}")))?
        .join(home)
    };
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    Ok(
      lexical_normalize(&absolute_home.join(executable))
        .to_string_lossy()
        .into_owned(),
    )
  }

  fn apply_overlay(
    &self,
    output: &mut Object<'env>,
    overlay: RuntimeBuildConfigOverlay,
  ) -> napi::Result<()> {
    if output.get::<Unknown<'env>>("cache")?.is_none() {
      if let Some(value) = overlay.default_cache {
        output.set("cache", value)?;
      }
    }
    self.set_optional(output, "mode", overlay.mode)?;
    self.set_optional(output, "enableSignTask", overlay.enable_sign_task)?;
    self.set_optional(output, "fastBuildApp", overlay.fast_build_app)?;
    self.set_optional(output, "ohosDebugAsan", overlay.ohos_debug_asan)?;
    self.set_optional(output, "ohosEnableHwasan", overlay.ohos_enable_hwasan)?;
    self.set_optional(output, "ohosEnableTsan", overlay.ohos_enable_tsan)?;
    self.set_optional(output, "ohosEnableUbsan", overlay.ohos_enable_ubsan)?;
    self.set_optional(output, "ohosEnableBinxo", overlay.ohos_enable_binxo)?;
    self.set_optional(
      output,
      "projectHvigorOverridesJson",
      overlay.project_hvigor_overrides_json,
    )?;
    self.set_optional(
      output,
      "moduleHvigorOverridesJson",
      overlay.module_hvigor_overrides_json,
    )?;
    self.set_optional(
      output,
      "runtimeContextOverridesJson",
      overlay.runtime_context_overrides_json,
    )?;
    self.set_optional(output, "packCompressLevel", overlay.pack_compress_level)?;
    self.set_optional(output, "alignDeviceTypes", overlay.align_device_types)?;
    self.set_optional(output, "collectDebugSymbol", overlay.collect_debug_symbol)?;
    self.set_optional(
      output,
      "uiTransformOptimization",
      overlay.ui_transform_optimization,
    )?;
    self.set_optional(
      output,
      "integratedHspUseGetCurrentBundleName",
      overlay.integrated_hsp_use_get_current_bundle_name,
    )?;
    self.set_optional(
      output,
      "byteCodeHarIntegratedOptimization",
      overlay.byte_code_har_integrated_optimization,
    )?;
    self.set_optional(output, "keepDependency", overlay.keep_dependency)?;
    self.set_optional(
      output,
      "parameterFileOverride",
      overlay.parameter_file_override,
    )?;
    self.set_optional(
      output,
      "hvigorRuntimePropertiesJson",
      overlay.hvigor_runtime_properties_json,
    )?;
    self.set_optional(output, "hvigorCacheDir", overlay.hvigor_cache_dir)?;
    Ok(())
  }

  fn set_optional<T: ToNapiValue>(
    &self,
    output: &mut Object<'env>,
    key: &str,
    value: Option<T>,
  ) -> napi::Result<()> {
    if let Some(value) = value {
      output.set(key, value)?;
    }
    Ok(())
  }
}
