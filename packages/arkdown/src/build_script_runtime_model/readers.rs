//! NAPI config readers and model/plugin adapters.

use super::*;

pub(super) struct RuntimeBuildScriptConfigReader<'env> {
  object: Object<'env>,
}

impl<'env> RuntimeBuildScriptConfigReader<'env> {
  pub(super) const PARAMETER_FIELDS: [&'static str; 16] = [
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

  pub(super) fn new(object: Object<'env>) -> Self {
    Self { object }
  }

  pub(super) fn value(&self, key: &str) -> Result<Option<Value>> {
    self.object.get(key)
  }

  pub(super) fn string(&self, key: &str) -> Result<Option<String>> {
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

  pub(super) fn boolean(&self, key: &str) -> Result<Option<bool>> {
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

  pub(super) fn path(&self, key: &str) -> Result<Option<PathBuf>> {
    self.string(key).map(|value| value.map(PathBuf::from))
  }

  pub(super) fn parameter_config(&self) -> Result<Value> {
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

  pub(super) fn empty_object() -> Value {
    Value::Object(Map::new())
  }
}

pub(super) struct RuntimeBuildScriptModuleReader<'env> {
  reader: RuntimeBuildScriptConfigReader<'env>,
}

impl<'env> RuntimeBuildScriptModuleReader<'env> {
  pub(super) fn new(object: Object<'env>) -> Self {
    Self {
      reader: RuntimeBuildScriptConfigReader::new(object),
    }
  }

  pub(super) fn value(&self, key: &str) -> Result<Option<Value>> {
    self.reader.value(key)
  }

  pub(super) fn string(&self, key: &str) -> Result<Option<String>> {
    self.reader.string(key)
  }

  pub(super) fn boolean(&self, key: &str) -> Result<Option<bool>> {
    self.reader.boolean(key)
  }

  pub(super) fn required_string(&self, key: &str) -> Result<String> {
    self.string(key)?.ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        format!("ARKDOWN_INVALID_BUILD_SCRIPT_RUNTIME_MODULE: {key} must be a string"),
      )
    })
  }
}

pub(super) struct BuildScriptRuntimeModelNapiAdapter;

impl BuildScriptRuntimeModelNapiAdapter {
  pub(super) fn encode<T>(value: &T, label: &str) -> Result<Value>
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

  pub(super) fn model_error(error: arkdown_session::BuildScriptRuntimeModelError) -> Error {
    Error::new(
      Status::GenericFailure,
      format!("ARKDOWN_BUILD_SCRIPT_RUNTIME_MODEL_FAILED: {error}"),
    )
  }

  pub(super) fn sdk_error(error: arkdown_session::HvigorSdkDetailsError) -> Error {
    Error::new(
      Status::GenericFailure,
      format!("ARKDOWN_HVIGOR_SDK_DETAILS_FAILED: {error}"),
    )
  }

  pub(super) fn session_model_error(error: arkdown_session::BuildScriptRuntimeModelError) -> Error {
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
