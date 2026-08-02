use std::path::PathBuf;

use arkdown_path::lexical_normalize;
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
pub struct RuntimeBuildConfigOverlay {
  pub default_cache: Option<bool>,
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
  const RUNTIME_OWNED_FIELDS: [&'static str; 11] = [
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
