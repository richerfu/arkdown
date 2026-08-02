use napi::bindgen_prelude::{Array, Object, Unknown, ValueType};
use napi::JsValue;
use napi::{Error, Result, Status};

use arkdown_config::{
  BuildScriptBuildOptionValidationRequest, BuildScriptPluginPathResolver,
  BuildScriptRuntimePlanRequest, BuildScriptRuntimePlanner, BuildScriptSchemaValidator,
};
use arkdown_session::{
  BuildScriptFileInventory, BuildScriptModuleModelLoader, BuildScriptRuntimeModelLoader,
};

pub(crate) struct BuildScriptRuntimeModelNapiAdapter;

impl BuildScriptRuntimeModelNapiAdapter {
  pub(crate) fn load_project(request_json: &str) -> Result<String> {
    let request = Self::decode(request_json, "project runtime model")?;
    match BuildScriptRuntimeModelLoader::new(request).load() {
      Ok(snapshot) => Self::encode(&snapshot, "project runtime model"),
      Err(arkdown_session::BuildScriptRuntimeModelError::Schema(diagnostic)) => Self::encode(
        &serde_json::json!({ "diagnostic": diagnostic }),
        "project runtime model diagnostic",
      ),
      Err(error) => Err(Self::model_error(error)),
    }
  }

  pub(crate) fn load_module(request_json: &str) -> Result<String> {
    let request = Self::decode(request_json, "module runtime model")?;
    match BuildScriptModuleModelLoader::new(request).load() {
      Ok(snapshot) => Self::encode(&snapshot, "module runtime model"),
      Err(arkdown_session::BuildScriptRuntimeModelError::Schema(diagnostic)) => Self::encode(
        &serde_json::json!({ "diagnostic": diagnostic }),
        "module runtime model diagnostic",
      ),
      Err(error) => Err(Self::model_error(error)),
    }
  }

  pub(crate) fn discover_files(request_json: &str) -> Result<String> {
    let request = Self::decode(request_json, "build-script file inventory")?;
    let files = BuildScriptFileInventory::new(request)
      .discover()
      .map_err(Self::model_error)?;
    Self::encode(&files, "build-script file inventory")
  }

  pub(crate) fn plan(request_json: &str) -> Result<String> {
    let request: BuildScriptRuntimePlanRequest =
      Self::decode(request_json, "build-script runtime plan")?;
    let outcome = match BuildScriptRuntimePlanner::new(request).plan() {
      Ok(plan) => serde_json::json!({ "plan": plan }),
      Err(diagnostic) => serde_json::json!({ "diagnostic": diagnostic }),
    };
    Self::encode(&outcome, "build-script runtime plan")
  }

  pub(crate) fn validate_build_option(request_json: &str) -> Result<String> {
    let request: BuildScriptBuildOptionValidationRequest =
      Self::decode(request_json, "build-script build option validation")?;
    let outcome = match BuildScriptSchemaValidator::validate_request(&request) {
      Ok(()) => serde_json::json!({ "valid": true }),
      Err(diagnostic) => serde_json::json!({ "diagnostic": diagnostic }),
    };
    Self::encode(&outcome, "build-script build option validation")
  }

  fn decode<T>(request_json: &str, label: &str) -> Result<T>
  where
    T: serde::de::DeserializeOwned,
  {
    serde_json::from_str(request_json).map_err(|error| {
      Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_MODEL_REQUEST: invalid {label}: {error}"),
      )
    })
  }

  fn encode<T>(value: &T, label: &str) -> Result<String>
  where
    T: serde::Serialize,
  {
    serde_json::to_string(value).map_err(|error| {
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
  ) -> Result<String> {
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
    serde_json::to_string(&BuildScriptPluginPathPlan {
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
