use indexmap::IndexMap;
use napi_derive::napi;
use serde_json::Value;

#[derive(Clone)]
enum RuntimeHvigorParameterValue {
  Json(Value),
  Undefined,
  V8Token(u32),
}

#[napi(object)]
pub struct RuntimeHvigorParameterEntry {
  pub exists: bool,
  pub kind: String,
  pub value: Option<Value>,
  pub token: Option<u32>,
}

#[napi(object)]
pub struct RuntimeHvigorParameterMutation {
  pub token: u32,
  pub replaced_token: Option<u32>,
}

/// Rust-owned mutable state behind Hvigor's `coreParameter` and
/// `ExternalParameter` APIs.
///
/// Primitive configuration values live directly in Rust. JavaScript objects
/// that must preserve V8 identity are represented by numeric tokens; the thin
/// JavaScript facade retains only those token/value pairs.
#[napi]
pub struct RuntimeHvigorParameterState {
  properties: IndexMap<String, RuntimeHvigorParameterValue>,
  ext_params: IndexMap<String, RuntimeHvigorParameterValue>,
  start_params: IndexMap<String, RuntimeHvigorParameterValue>,
  workspace_dir: String,
  next_token: u32,
}

impl Default for RuntimeHvigorParameterState {
  fn default() -> Self {
    Self::new()
  }
}

#[napi]
impl RuntimeHvigorParameterState {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      properties: IndexMap::new(),
      ext_params: IndexMap::new(),
      start_params: Self::default_start_params(),
      workspace_dir: String::new(),
      next_token: 0,
    }
  }

  #[napi]
  pub fn keys(&self, scope: String) -> napi::Result<Vec<String>> {
    Ok(self.scope(&scope)?.keys().cloned().collect())
  }

  #[napi]
  pub fn entry(&self, scope: String, key: String) -> napi::Result<RuntimeHvigorParameterEntry> {
    let Some(value) = self.scope(&scope)?.get(&key) else {
      return Ok(RuntimeHvigorParameterEntry {
        exists: false,
        kind: "missing".to_string(),
        value: None,
        token: None,
      });
    };
    Ok(match value {
      RuntimeHvigorParameterValue::Json(value) => RuntimeHvigorParameterEntry {
        exists: true,
        kind: "json".to_string(),
        value: Some(value.clone()),
        token: None,
      },
      RuntimeHvigorParameterValue::Undefined => RuntimeHvigorParameterEntry {
        exists: true,
        kind: "undefined".to_string(),
        value: None,
        token: None,
      },
      RuntimeHvigorParameterValue::V8Token(token) => RuntimeHvigorParameterEntry {
        exists: true,
        kind: "v8".to_string(),
        value: None,
        token: Some(*token),
      },
    })
  }

  #[napi]
  pub fn set_json_entry(
    &mut self,
    scope: String,
    key: String,
    value: Value,
  ) -> napi::Result<Option<u32>> {
    Ok(Self::replaced_token(
      self
        .scope_mut(&scope)?
        .insert(key, RuntimeHvigorParameterValue::Json(value)),
    ))
  }

  #[napi]
  pub fn set_undefined_entry(&mut self, scope: String, key: String) -> napi::Result<Option<u32>> {
    Ok(Self::replaced_token(
      self
        .scope_mut(&scope)?
        .insert(key, RuntimeHvigorParameterValue::Undefined),
    ))
  }

  #[napi]
  pub fn set_v8_entry(
    &mut self,
    scope: String,
    key: String,
  ) -> napi::Result<RuntimeHvigorParameterMutation> {
    let token = self.allocate_token();
    let replaced_token = Self::replaced_token(
      self
        .scope_mut(&scope)?
        .insert(key, RuntimeHvigorParameterValue::V8Token(token)),
    );
    Ok(RuntimeHvigorParameterMutation {
      token,
      replaced_token,
    })
  }

  #[napi]
  pub fn delete_entry(&mut self, scope: String, key: String) -> napi::Result<Option<u32>> {
    Ok(Self::replaced_token(
      self.scope_mut(&scope)?.shift_remove(&key),
    ))
  }

  #[napi]
  pub fn clear_scope(&mut self, scope: String) -> napi::Result<Vec<u32>> {
    let values = self.scope_mut(&scope)?;
    let tokens = values
      .values()
      .filter_map(Self::value_token)
      .collect::<Vec<_>>();
    values.clear();
    Ok(tokens)
  }

  #[napi]
  pub fn clean(&mut self) -> Vec<u32> {
    let mut tokens = Vec::new();
    for values in [&self.properties, &self.ext_params, &self.start_params] {
      tokens.extend(values.values().filter_map(Self::value_token));
    }
    self.properties.clear();
    self.ext_params.clear();
    self.start_params = Self::default_start_params();
    self.workspace_dir.clear();
    tokens
  }

  #[napi(getter)]
  pub fn workspace_dir(&self) -> String {
    self.workspace_dir.clone()
  }

  #[napi(setter)]
  pub fn set_workspace_dir(&mut self, value: String) {
    self.workspace_dir = value;
  }

  /// Materialize the public `StartParam` shape from the internal core fields.
  /// The field order follows Hvigor 6.24.3 `ExternalParameter._getStartParams`.
  #[napi]
  pub fn public_start_snapshot(&self) -> napi::Result<Value> {
    let mut output = serde_json::Map::new();
    output.insert("daemon".to_string(), self.start_json("daemon")?);
    output.insert(
      "analyze".to_string(),
      match self.start_json("analyze")?.as_i64() {
        Some(0) => Value::String("normal".to_string()),
        Some(1) => Value::String("advanced".to_string()),
        Some(2) => Value::Bool(false),
        Some(3) => Value::String("ultrafine".to_string()),
        _ => Value::Null,
      },
    );
    output.insert(
      "parallel".to_string(),
      self.start_json("parallelExecution")?,
    );
    output.insert(
      "incremental".to_string(),
      self.start_json("incrementalExecution")?,
    );
    output.insert("logLevel".to_string(), self.start_json("logLevel")?);
    output.insert(
      "typeCheck".to_string(),
      self.start_json("hvigorfileTypeCheck")?,
    );
    output.insert(
      "optimizationStrategy".to_string(),
      self.start_json("optimizationStrategy")?,
    );
    output.insert("hotCompile".to_string(), self.start_json("hotCompile")?);
    output.insert(
      "hotReloadBuild".to_string(),
      self.start_json("hotReloadBuild")?,
    );
    Ok(Value::Object(output))
  }

  /// Validate the value accepted by `ExternalParameter.setProperty` against
  /// the 6.24.3 `hvigor-config-schema.json` properties contract.
  #[napi]
  pub fn validate_property(
    &self,
    key: String,
    value: Value,
    javascript_type: String,
  ) -> napi::Result<()> {
    let valid = match key.as_str() {
      "ohos.integratedHsp.useGetCurrentBundleName"
      | "ohos.processLib.optimization"
      | "enableSignTask"
      | "hvigor.analyzeHtml"
      | "hvigor.keepDependency"
      | "ohos.compile.lib.entryfile"
      | "ohos.sign.har"
      | "hvigor.dependency.useNpm"
      | "hvigor.enableMemoryCache"
      | "hvigor.sync.cache"
      | "hvigor.incremental.optimization"
      | "hvigor.task.schedule.optimization"
      | "ohos.nativeResolver"
      | "ohos.arkCompile.noEmitJs"
      | "ohos.arkCompile.useTscResolve"
      | "ohos.collect.debugSymbol"
      | "ohos.arkCompile.emptyBundleName"
      | "ohos.arkCompile.singleFileEmit"
      | "ohos.etsLinter.useDeclarationFileSignature"
      | "ohos.har.excludeHspDependencies"
      | "ohos.obfuscationRules.optimization"
      | "ohos.byteCodeHar.integratedOptimization"
      | "ohos.uiTransform.Optimization"
      | "ohos.rollupCache.useSourceHash"
      | "ohos.rollupCache.usePathPlaceholder"
      | "ohos.arkCompile.writeRollupCache"
      | "ohos.dependencies.types.enable"
      | "ohos.defaults.autoLazyImport" => value.is_boolean(),
      "hvigor.cacheDir"
      | "ohos.buildDir"
      | "ohos.arkCompile.sourceMapDir"
      | "ohos.align.target" => value.is_string(),
      "hvigor.pool.maxSize"
      | "hvigor.pool.cache.capacity"
      | "ohos.arkCompile.maxSize"
      | "hvigor.memoryThreshold" => value.as_i64().is_some() || value.as_u64().is_some(),
      "ohos.pack.compressLevel" => matches!(value.as_str(), Some("fast" | "standard" | "ultimate")),
      "ohos.defaults.release.cmakebuildtype" => {
        matches!(value.as_str(), Some("Release" | "Debug" | "RelWithDebInfo"))
      }
      "ohos.fallback.target" => Self::valid_non_empty_string_array(&value, None, false),
      "ohos.align.deviceTypes" => Self::valid_non_empty_string_array(
        &value,
        Some(&[
          "default", "phone", "tablet", "tv", "wearable", "car", "2in1",
        ]),
        true,
      ),
      _ => value.is_boolean() || value.is_string() || value.is_number(),
    };
    if valid
      && !matches!(
        javascript_type.as_str(),
        "undefined" | "function" | "symbol" | "bigint"
      )
    {
      return Ok(());
    }
    Err(napi::Error::from_reason(format!(
      "ARKDOWN_HVIGOR_PARAMETER_VALIDATION_FAILED: property '{key}' has an invalid {javascript_type} value"
    )))
  }
}

impl RuntimeHvigorParameterState {
  fn default_start_params() -> IndexMap<String, RuntimeHvigorParameterValue> {
    IndexMap::from([
      (
        "hvigorfileTypeCheck".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(false)),
      ),
      (
        "parallelExecution".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(true)),
      ),
      (
        "incrementalExecution".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(true)),
      ),
      (
        "printStackTrace".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(false)),
      ),
      (
        "daemon".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(true)),
      ),
      (
        "analyze".to_string(),
        RuntimeHvigorParameterValue::Json(Value::from(0)),
      ),
      (
        "logLevel".to_string(),
        RuntimeHvigorParameterValue::Json(Value::String("info".to_string())),
      ),
      (
        "optimizationStrategy".to_string(),
        RuntimeHvigorParameterValue::Json(Value::String("memory".to_string())),
      ),
      (
        "hotCompile".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(false)),
      ),
      (
        "hotReloadBuild".to_string(),
        RuntimeHvigorParameterValue::Json(Value::Bool(false)),
      ),
      (
        "customizedParam".to_string(),
        RuntimeHvigorParameterValue::Undefined,
      ),
    ])
  }

  fn scope(&self, scope: &str) -> napi::Result<&IndexMap<String, RuntimeHvigorParameterValue>> {
    match scope {
      "properties" => Ok(&self.properties),
      "extParams" => Ok(&self.ext_params),
      "startParams" => Ok(&self.start_params),
      _ => Err(napi::Error::from_reason(format!(
        "unknown Hvigor parameter scope '{scope}'"
      ))),
    }
  }

  fn scope_mut(
    &mut self,
    scope: &str,
  ) -> napi::Result<&mut IndexMap<String, RuntimeHvigorParameterValue>> {
    match scope {
      "properties" => Ok(&mut self.properties),
      "extParams" => Ok(&mut self.ext_params),
      "startParams" => Ok(&mut self.start_params),
      _ => Err(napi::Error::from_reason(format!(
        "unknown Hvigor parameter scope '{scope}'"
      ))),
    }
  }

  fn allocate_token(&mut self) -> u32 {
    let token = self.next_token;
    self.next_token = self.next_token.wrapping_add(1);
    token
  }

  fn replaced_token(value: Option<RuntimeHvigorParameterValue>) -> Option<u32> {
    value.as_ref().and_then(Self::value_token)
  }

  fn value_token(value: &RuntimeHvigorParameterValue) -> Option<u32> {
    match value {
      RuntimeHvigorParameterValue::V8Token(token) => Some(*token),
      RuntimeHvigorParameterValue::Json(_) | RuntimeHvigorParameterValue::Undefined => None,
    }
  }

  fn start_json(&self, key: &str) -> napi::Result<Value> {
    match self.start_params.get(key) {
      Some(RuntimeHvigorParameterValue::Json(value)) => Ok(value.clone()),
      Some(RuntimeHvigorParameterValue::Undefined) | None => Ok(Value::Null),
      Some(RuntimeHvigorParameterValue::V8Token(_)) => Err(napi::Error::from_reason(format!(
        "Hvigor start parameter '{key}' cannot be materialized outside its V8 isolate"
      ))),
    }
  }

  fn valid_non_empty_string_array(value: &Value, allowed: Option<&[&str]>, unique: bool) -> bool {
    let Some(values) = value.as_array() else {
      return false;
    };
    if values.is_empty() {
      return false;
    }
    let mut seen = std::collections::HashSet::new();
    values.iter().all(|value| {
      let Some(value) = value.as_str() else {
        return false;
      };
      if allowed.is_some_and(|allowed| !allowed.contains(&value)) {
        return false;
      }
      !unique || seen.insert(value)
    })
  }
}
