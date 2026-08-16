use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

const SOURCE_MAP_DIR: &str = "ohos.arkCompile.sourceMapDir";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorRuntimeParameterPlanRequest {
    #[serde(default)]
    pub user_config: Value,
    #[serde(default)]
    pub project_config: Value,
    #[serde(default)]
    pub runtime_config: Value,
    #[serde(default)]
    pub command_entry_tasks: Vec<String>,
    pub workspace_dir: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorRuntimeParameterPlan {
    pub properties: Map<String, Value>,
    pub ext_params: Map<String, Value>,
    pub core_start_params: Map<String, Value>,
    pub workspace_dir: PathBuf,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub undefined_property_keys: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub undefined_ext_param_keys: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub property_override_keys: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prop_ext_param_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorRuntimePropertyPlanRequest {
    #[serde(default)]
    pub properties: Map<String, Value>,
    #[serde(default)]
    pub ext_params: Map<String, Value>,
    #[serde(default)]
    pub core_start_params: Map<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorRuntimePropertyPlan {
    pub runtime_properties: Map<String, Value>,
    pub native_properties: HvigorNativePropertyPlan,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorNativePropertyPlan {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_compress_level: Option<u32>,
    pub align_device_types: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collect_debug_symbol: Option<bool>,
    pub ui_transform_optimization: bool,
    pub integrated_hsp_use_get_current_bundle_name: bool,
    pub byte_code_har_integrated_optimization: bool,
    pub keep_dependency: bool,
    pub enable_sign_task: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorRuntimePropertyDiagnostic {
    pub code: String,
    pub message: String,
    pub property: String,
}

impl HvigorRuntimePropertyDiagnostic {
    fn invalid(code: &str, property: &str, expected: &str) -> Self {
        Self {
            code: code.to_string(),
            message: format!("{property} must be {expected}"),
            property: property.to_string(),
        }
    }
}

/// Rust owner for the immutable part of Hvigor's process-global Parameter
/// initialization. JavaScript retains the live object facade because plugin
/// callbacks may mutate it after this plan has been materialized.
pub struct HvigorRuntimeParameterPlanner {
    request: HvigorRuntimeParameterPlanRequest,
}

impl HvigorRuntimeParameterPlanner {
    pub fn new(request: HvigorRuntimeParameterPlanRequest) -> Self {
        Self { request }
    }

    pub fn plan(&self) -> HvigorRuntimeParameterPlan {
        let mut properties = Map::new();
        properties.insert("enableSignTask".to_string(), Value::Bool(true));
        properties.insert("skipNativeIncremental".to_string(), Value::Bool(false));
        properties.insert("hvigor.keepDependency".to_string(), Value::Bool(true));
        self.extend_from_field(&mut properties, &self.request.user_config, "properties");
        self.extend_from_field(&mut properties, &self.request.project_config, "properties");
        self.extend_from_field(
            &mut properties,
            &self.request.runtime_config,
            "hvigorProperties",
        );

        let mut ext_params = Map::new();
        self.extend_from_field(&mut ext_params, &self.request.runtime_config, "extParams");
        let mut undefined_property_keys = BTreeSet::new();
        let mut undefined_ext_param_keys = BTreeSet::new();
        let mut property_override_keys = BTreeSet::new();
        let mut prop_ext_param_keys = BTreeSet::new();
        self.apply_property_arguments(
            &mut properties,
            &mut ext_params,
            &mut undefined_property_keys,
            &mut undefined_ext_param_keys,
            &mut property_override_keys,
            &mut prop_ext_param_keys,
        );
        self.apply_config_overrides(
            &mut properties,
            &mut undefined_property_keys,
            &mut property_override_keys,
        );
        self.apply_command_defaults(&mut ext_params);

        for key in &undefined_property_keys {
            properties.remove(key);
        }
        for key in &undefined_ext_param_keys {
            ext_params.remove(key);
        }

        HvigorRuntimeParameterPlan {
            properties,
            ext_params,
            core_start_params: self.core_start_params(),
            workspace_dir: self.request.workspace_dir.clone(),
            undefined_property_keys: undefined_property_keys.into_iter().collect(),
            undefined_ext_param_keys: undefined_ext_param_keys.into_iter().collect(),
            property_override_keys: property_override_keys.into_iter().collect(),
            prop_ext_param_keys: prop_ext_param_keys.into_iter().collect(),
        }
    }

    fn extend_from_field(&self, target: &mut Map<String, Value>, owner: &Value, field: &str) {
        let Some(value) = owner.get(field) else {
            return;
        };
        match value {
            Value::Object(entries) => target.extend(entries.clone()),
            Value::Array(entries) => target.extend(
                entries
                    .iter()
                    .enumerate()
                    .map(|(index, value)| (index.to_string(), value.clone())),
            ),
            _ => {}
        }
    }

    fn apply_property_arguments(
        &self,
        properties: &mut Map<String, Value>,
        ext_params: &mut Map<String, Value>,
        undefined_property_keys: &mut BTreeSet<String>,
        undefined_ext_param_keys: &mut BTreeSet<String>,
        property_override_keys: &mut BTreeSet<String>,
        prop_ext_param_keys: &mut BTreeSet<String>,
    ) {
        for entry in self.string_arguments("prop") {
            let (raw_key, raw_value) = entry
                .split_once('=')
                .map_or((entry.as_str(), None), |(key, value)| (key, Some(value)));
            let key = if raw_key == "coverage" {
                "ohos-test-coverage"
            } else {
                raw_key
            };
            prop_ext_param_keys.insert(key.to_string());
            match raw_value {
                Some(value) => {
                    ext_params.insert(key.to_string(), Value::String(value.to_string()));
                    undefined_ext_param_keys.remove(key);
                }
                None => {
                    ext_params.insert(key.to_string(), Value::Null);
                    undefined_ext_param_keys.insert(key.to_string());
                }
            }
            if let Some(property) = raw_key.strip_prefix("properties.") {
                property_override_keys.insert(property.to_string());
                match raw_value {
                    Some(value) => {
                        properties.insert(property.to_string(), self.config_property_value(value));
                        undefined_property_keys.remove(property);
                    }
                    None => {
                        properties.insert(property.to_string(), Value::Null);
                        undefined_property_keys.insert(property.to_string());
                    }
                }
            }
        }
    }

    fn apply_config_overrides(
        &self,
        properties: &mut Map<String, Value>,
        undefined_property_keys: &mut BTreeSet<String>,
        property_override_keys: &mut BTreeSet<String>,
    ) {
        for entry in self.string_arguments("configOverrides") {
            let Some((key, value)) = entry.split_once('=') else {
                continue;
            };
            let Some(property) = key.strip_prefix("properties.") else {
                continue;
            };
            property_override_keys.insert(property.to_string());
            properties.insert(property.to_string(), self.config_property_value(value));
            undefined_property_keys.remove(property);
        }
    }

    fn apply_command_defaults(&self, ext_params: &mut Map<String, Value>) {
        let defaults: &[(&str, &str)] = if self
            .request
            .command_entry_tasks
            .iter()
            .any(|task| task == "test")
        {
            &[
                ("pageType", "page"),
                ("product", "default"),
                ("buildRoot", ".test"),
                ("unitTestMode", "true"),
                ("isLocalTest", "true"),
                ("ohos-test-coverage", "true"),
                (
                    "unit.test.replace.page",
                    "../../../.test/testability/pages/Index",
                ),
            ]
        } else if self
            .request
            .command_entry_tasks
            .iter()
            .any(|task| task == "onDeviceTest")
        {
            &[
                ("product", "default"),
                ("buildMode", "test"),
                ("isOhosTest", "true"),
                ("ohos-test-coverage", "true"),
            ]
        } else {
            &[]
        };
        for (key, value) in defaults {
            if !ext_params.contains_key(*key) {
                ext_params.insert((*key).to_string(), Value::String((*value).to_string()));
            }
        }
    }

    fn core_start_params(&self) -> Map<String, Value> {
        let project_execution = self.object_field(&self.request.project_config, "execution");
        let project_debugging = self.object_field(&self.request.project_config, "debugging");
        let project_logging = self.object_field(&self.request.project_config, "logging");
        let runtime = self.request.runtime_config.as_object();

        let incremental = self.first_non_null([
            runtime.and_then(|value| value.get("cache")),
            project_execution.and_then(|value| value.get("incremental")),
        ]);
        let parallel = self.first_non_null([
            runtime.and_then(|value| value.get("parallel")),
            project_execution.and_then(|value| value.get("parallel")),
        ]);
        let type_check = self.first_non_null([
            runtime.and_then(|value| value.get("enableBuildScriptTypeCheck")),
            runtime.and_then(|value| value.get("typeCheck")),
            project_execution.and_then(|value| value.get("typeCheck")),
        ]);
        let stacktrace = self.first_non_null([
            runtime.and_then(|value| value.get("stacktrace")),
            project_debugging.and_then(|value| value.get("stacktrace")),
        ]);
        let daemon = self.first_non_null([
            runtime.and_then(|value| value.get("daemon")),
            project_execution.and_then(|value| value.get("daemon")),
        ]);
        let analyze = self
            .first_non_null([
                runtime.and_then(|value| value.get("analyze")),
                project_execution.and_then(|value| value.get("analyze")),
            ])
            .cloned()
            .unwrap_or_else(|| Value::String("normal".to_string()));
        let optimization_strategy = self
            .first_non_null([
                runtime.and_then(|value| value.get("optimizationStrategy")),
                project_execution.and_then(|value| value.get("optimizationStrategy")),
            ])
            .cloned()
            .unwrap_or_else(|| Value::String("memory".to_string()));
        let log_level = self
            .first_non_null([
                runtime.and_then(|value| value.get("logLevel")),
                project_logging.and_then(|value| value.get("level")),
            ])
            .cloned()
            .unwrap_or_else(|| Value::String("info".to_string()));

        let mut output = Map::new();
        output.insert("analyze".to_string(), self.analyze_mode(analyze));
        output.insert(
            "daemon".to_string(),
            Value::Bool(daemon.map(Self::js_truthy).unwrap_or(true)),
        );
        output.insert(
            "parallelExecution".to_string(),
            Value::Bool(parallel.map(Self::js_truthy).unwrap_or(true)),
        );
        output.insert(
            "incrementalExecution".to_string(),
            Value::Bool(incremental.map(Self::js_truthy).unwrap_or(true)),
        );
        output.insert(
            "hvigorfileTypeCheck".to_string(),
            Value::Bool(type_check.map(Self::js_truthy).unwrap_or(false)),
        );
        output.insert(
            "printStackTrace".to_string(),
            Value::Bool(stacktrace.map(Self::js_truthy).unwrap_or(false)),
        );
        output.insert("logLevel".to_string(), log_level);
        output.insert("optimizationStrategy".to_string(), optimization_strategy);
        output.insert(
            "hotCompile".to_string(),
            Value::Bool(
                runtime
                    .and_then(|value| value.get("hotCompile"))
                    .map(Self::js_truthy)
                    .unwrap_or(false),
            ),
        );
        output.insert(
            "hotReloadBuild".to_string(),
            Value::Bool(
                runtime
                    .and_then(|value| value.get("hotReloadBuild"))
                    .map(Self::js_truthy)
                    .unwrap_or(false),
            ),
        );
        if let Some(value) = runtime
            .and_then(|value| value.get("customizedParam"))
            .filter(|value| matches!(value, Value::Object(_) | Value::Array(_)))
        {
            output.insert("customizedParam".to_string(), value.clone());
        }
        output
    }

    fn string_arguments(&self, field: &str) -> Vec<String> {
        match self.request.runtime_config.get(field) {
            Some(Value::String(value)) => vec![value.clone()],
            Some(Value::Array(values)) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn object_field<'a>(&self, owner: &'a Value, field: &str) -> Option<&'a Map<String, Value>> {
        owner.get(field).and_then(Value::as_object)
    }

    fn first_non_null<'a, const N: usize>(
        &self,
        values: [Option<&'a Value>; N],
    ) -> Option<&'a Value> {
        values.into_iter().flatten().find(|value| !value.is_null())
    }

    fn analyze_mode(&self, value: Value) -> Value {
        match &value {
            Value::String(mode) => match mode.as_str() {
                "default" | "normal" => Value::from(0),
                "verbose" | "advanced" => Value::from(1),
                "false" => Value::from(2),
                "ultrafine" => Value::from(3),
                _ => value,
            },
            Value::Bool(false) => Value::from(2),
            _ => value,
        }
    }

    fn config_property_value(&self, value: &str) -> Value {
        if value == "true" {
            return Value::Bool(true);
        }
        if value == "false" {
            return Value::Bool(false);
        }
        if value.len() <= 16 {
            if let Some(number) = self.javascript_number(value) {
                return Value::Number(number);
            }
        }
        Value::String(value.trim().to_string())
    }

    fn javascript_number(&self, value: &str) -> Option<Number> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Some(Number::from(0));
        }
        let integer = if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            u64::from_str_radix(hex, 16).ok()
        } else if let Some(binary) = trimmed
            .strip_prefix("0b")
            .or_else(|| trimmed.strip_prefix("0B"))
        {
            u64::from_str_radix(binary, 2).ok()
        } else if let Some(octal) = trimmed
            .strip_prefix("0o")
            .or_else(|| trimmed.strip_prefix("0O"))
        {
            u64::from_str_radix(octal, 8).ok()
        } else {
            None
        };
        if let Some(integer) = integer {
            return Some(Number::from(integer));
        }
        trimmed
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .and_then(Number::from_f64)
    }

    fn js_truthy(value: &Value) -> bool {
        match value {
            Value::Null => false,
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
            Value::String(value) => !value.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        }
    }
}

/// Rust owner for OHOS plugin predicates derived from the live Parameter
/// snapshot. The request is produced only after plugin configuration hooks, so
/// callback mutations remain visible without reimplementing the predicates in
/// JavaScript.
pub struct HvigorRuntimePropertyPlanner {
    request: HvigorRuntimePropertyPlanRequest,
}

impl HvigorRuntimePropertyPlanner {
    pub fn new(request: HvigorRuntimePropertyPlanRequest) -> Self {
        Self { request }
    }

    pub fn plan(&self) -> Result<HvigorRuntimePropertyPlan, HvigorRuntimePropertyDiagnostic> {
        let align_target =
            self.optional_string_property("ohos.align.target", "INVALID_ALIGN_TARGET")?;
        let source_map_dir =
            self.optional_string_property(SOURCE_MAP_DIR, "INVALID_SOURCE_MAP_DIR")?;
        let release_cmake_build_type = self.optional_string_property(
            "ohos.defaults.release.cmakebuildtype",
            "INVALID_CMAKE_BUILD_TYPE",
        )?;
        let compile_lib_entry_file = self.optional_boolean_property(
            "ohos.compile.lib.entryfile",
            "INVALID_COMPILE_LIB_ENTRY_FILE",
        )?;
        let fallback_targets = self.fallback_targets()?;
        let previewer_replace_page = self.optional_string_ext_param("previewer.replace.page")?;
        let main_module = self.optional_string_ext_param("mainModule")?;
        let page_type = self.optional_string_ext_param("pageType")?;

        let mut runtime_properties = Map::new();
        runtime_properties.insert(
            "daemon".to_string(),
            Value::Bool(
                self.request
                    .core_start_params
                    .get("daemon")
                    .map(HvigorRuntimeParameterPlanner::js_truthy)
                    .unwrap_or(false),
            ),
        );
        self.insert_optional_string(
            &mut runtime_properties,
            "alignTarget",
            align_target.filter(|value| !value.is_empty()),
        );
        self.insert_optional_string(&mut runtime_properties, "sourceMapDir", source_map_dir);
        runtime_properties.insert(
            "harExcludeHspDependencies".to_string(),
            Value::Bool(self.literal_true("ohos.har.excludeHspDependencies")),
        );
        runtime_properties.insert(
            "dependenciesTypesEnable".to_string(),
            Value::Bool(self.literal_true("ohos.dependencies.types.enable")),
        );
        runtime_properties.insert(
            "singleFileEmit".to_string(),
            Value::Bool(self.truthy_property("ohos.arkCompile.singleFileEmit")),
        );
        runtime_properties.insert(
            "rollupCacheUseSourceHash".to_string(),
            Value::Bool(self.literal_true("ohos.rollupCache.useSourceHash")),
        );
        runtime_properties.insert(
            "processLibOptimization".to_string(),
            Value::Bool(self.truthy_property("ohos.processLib.optimization")),
        );
        if let Some(value) = compile_lib_entry_file {
            runtime_properties.insert("compileLibEntryFile".to_string(), Value::Bool(value));
        }
        self.insert_optional_string(
            &mut runtime_properties,
            "releaseCmakeBuildType",
            release_cmake_build_type,
        );
        runtime_properties.insert(
            "fallbackTargets".to_string(),
            Value::Array(fallback_targets.into_iter().map(Value::String).collect()),
        );
        runtime_properties.insert(
            "obfuscationRulesOptimization".to_string(),
            Value::Bool(self.literal_true("ohos.obfuscationRules.optimization")),
        );
        runtime_properties.insert(
            "signHar".to_string(),
            Value::Bool(self.literal_true("ohos.sign.har")),
        );
        runtime_properties.insert(
            "previewerCompileResIncremental".to_string(),
            Value::Bool(self.ext_param_literal_true("compileResInc")),
        );
        self.insert_optional_string(
            &mut runtime_properties,
            "previewerReplacePage",
            previewer_replace_page,
        );
        self.insert_optional_string(&mut runtime_properties, "mainModule", main_module);
        self.insert_optional_string(&mut runtime_properties, "pageType", page_type);
        let compile_hsp = self.request.ext_params.get("compileHspDependencies");
        runtime_properties.insert(
            "compileHspDependencies".to_string(),
            Value::Bool(compile_hsp.and_then(Value::as_str) == Some("true")),
        );
        runtime_properties.insert(
            "compileHspDependenciesTruthy".to_string(),
            Value::Bool(
                compile_hsp
                    .map(HvigorRuntimeParameterPlanner::js_truthy)
                    .unwrap_or(false),
            ),
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "asanConfig",
            "ohos-debug-asan",
            false,
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "tsanConfig",
            "ohos-enable-tsan",
            false,
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "ubsanConfig",
            "ohos-enable-ubsan",
            false,
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "hwasanConfig",
            "ohos-enable-hwasan",
            true,
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "binxoConfig",
            "ohos-enable-binxo",
            true,
        );
        self.insert_exact_boolean_ext_param(
            &mut runtime_properties,
            "withDeduplicate",
            "withDeduplicate",
            false,
        );

        Ok(HvigorRuntimePropertyPlan {
            runtime_properties,
            native_properties: self.native_properties(),
        })
    }

    fn native_properties(&self) -> HvigorNativePropertyPlan {
        let pack_compress_level = self
            .request
            .properties
            .get("ohos.pack.compressLevel")
            .and_then(Value::as_str)
            .and_then(|value| match value {
                "fast" => Some(1),
                "standard" => Some(5),
                "ultimate" => Some(9),
                _ => None,
            });
        let align_device_types = match self.request.properties.get("ohos.align.deviceTypes") {
            Some(Value::String(value)) => Value::Array(
                value
                    .split(',')
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
            Some(Value::Array(value)) => Value::Array(value.clone()),
            _ => Value::Array(Vec::new()),
        };
        HvigorNativePropertyPlan {
            pack_compress_level,
            align_device_types,
            collect_debug_symbol: self
                .request
                .properties
                .get("ohos.collect.debugSymbol")
                .map(HvigorRuntimeParameterPlanner::js_truthy),
            ui_transform_optimization: self.literal_true("ohos.uiTransform.Optimization"),
            integrated_hsp_use_get_current_bundle_name: self
                .literal_true("ohos.integratedHsp.useGetCurrentBundleName"),
            byte_code_har_integrated_optimization: self
                .literal_true("ohos.byteCodeHar.integratedOptimization"),
            keep_dependency: self.truthy_property("hvigor.keepDependency"),
            enable_sign_task: self.truthy_property("enableSignTask"),
        }
    }

    fn fallback_targets(&self) -> Result<Vec<String>, HvigorRuntimePropertyDiagnostic> {
        match self.request.properties.get("ohos.fallback.target") {
            None => Ok(Vec::new()),
            Some(Value::String(value)) if value.is_empty() => Ok(Vec::new()),
            Some(Value::String(value)) => Ok(value.split(',').map(str::to_string).collect()),
            Some(Value::Array(values)) if values.iter().all(Value::is_string) => Ok(values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()),
            _ => Err(HvigorRuntimePropertyDiagnostic::invalid(
                "INVALID_FALLBACK_TARGET",
                "ohos.fallback.target",
                "a comma-separated string or string array",
            )),
        }
    }

    fn optional_string_property(
        &self,
        property: &str,
        code: &str,
    ) -> Result<Option<String>, HvigorRuntimePropertyDiagnostic> {
        match self.request.properties.get(property) {
            None => Ok(None),
            Some(Value::String(value)) => Ok(Some(value.clone())),
            _ => Err(HvigorRuntimePropertyDiagnostic::invalid(
                code, property, "a string",
            )),
        }
    }

    fn optional_boolean_property(
        &self,
        property: &str,
        code: &str,
    ) -> Result<Option<bool>, HvigorRuntimePropertyDiagnostic> {
        match self.request.properties.get(property) {
            None => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            _ => Err(HvigorRuntimePropertyDiagnostic::invalid(
                code,
                property,
                "a boolean",
            )),
        }
    }

    fn optional_string_ext_param(
        &self,
        property: &str,
    ) -> Result<Option<String>, HvigorRuntimePropertyDiagnostic> {
        match self.request.ext_params.get(property) {
            None => Ok(None),
            Some(Value::String(value)) => Ok(Some(value.clone())),
            _ => Err(HvigorRuntimePropertyDiagnostic::invalid(
                "INVALID_PREVIEW_PROPERTY",
                property,
                "a string",
            )),
        }
    }

    fn insert_optional_string(
        &self,
        output: &mut Map<String, Value>,
        key: &str,
        value: Option<String>,
    ) {
        if let Some(value) = value {
            output.insert(key.to_string(), Value::String(value));
        }
    }

    fn insert_exact_boolean_ext_param(
        &self,
        output: &mut Map<String, Value>,
        output_key: &str,
        input_key: &str,
        truthy_override: bool,
    ) {
        let Some(value) = self.request.ext_params.get(input_key) else {
            return;
        };
        if truthy_override && !HvigorRuntimeParameterPlanner::js_truthy(value) {
            return;
        }
        output.insert(
            output_key.to_string(),
            Value::Bool(value.as_str() == Some("true")),
        );
    }

    fn literal_true(&self, property: &str) -> bool {
        self.request.properties.get(property) == Some(&Value::Bool(true))
    }

    fn truthy_property(&self, property: &str) -> bool {
        self.request
            .properties
            .get(property)
            .map(HvigorRuntimeParameterPlanner::js_truthy)
            .unwrap_or(false)
    }

    fn ext_param_literal_true(&self, property: &str) -> bool {
        self.request
            .ext_params
            .get(property)
            .and_then(Value::as_str)
            == Some("true")
    }
}
