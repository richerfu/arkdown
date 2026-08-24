use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;

use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::build_profile::{
    pick_effective_raw_target, BuildOptionProfileInventory, BuildOptionProfileModuleValue,
    EffectiveModuleBuildOptionResolver, RawTarget,
};

const COMPILE_PLUGIN_PREFIX: &str = "./build/hvigor_compile/";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimePlanRequest {
    pub project_root: PathBuf,
    #[serde(default)]
    pub project_profile: serde_json::Value,
    #[serde(default)]
    pub modules: Vec<BuildScriptRuntimeModulePlanRequest>,
    pub product_name: String,
    pub build_mode_name: String,
    #[serde(default)]
    pub module_selection: Option<String>,
    #[serde(default)]
    pub build_target_name: Option<String>,
    #[serde(default)]
    pub device_test: bool,
    #[serde(default)]
    pub project_oh_package_exists: bool,
    #[serde(default)]
    pub project_npm_package_exists: bool,
    #[serde(default)]
    pub project_kind: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub compile_api_version: Option<Value>,
    #[serde(default)]
    pub compile_sdk_version: Option<Value>,
    #[serde(default)]
    pub compatible_api_version: Option<Value>,
    #[serde(default)]
    pub compatible_sdk_version: Option<Value>,
    #[serde(default)]
    pub command_entry_tasks: Vec<String>,
    #[serde(default)]
    pub properties: Map<String, Value>,
    #[serde(default)]
    pub ext_params: Map<String, Value>,
    #[serde(default)]
    pub start_params: Map<String, Value>,
    #[serde(default)]
    pub ets_loader_path: Option<PathBuf>,
    #[serde(default)]
    pub js_loader_path: Option<PathBuf>,
    #[serde(default)]
    pub has_rollup_plugin_in_ets_loader: Option<bool>,
    #[serde(default)]
    pub has_rollup_plugin_in_js_loader: Option<bool>,
    #[serde(default)]
    pub sync_version_changed: bool,
}

impl BuildScriptRuntimePlanRequest {
    pub fn effective_compile_api_version(&self) -> u32 {
        let product = self.selected_product();
        [
            product.and_then(|value| value.get("compileSdkVersion")),
            self.compile_api_version.as_ref(),
            self.compile_sdk_version.as_ref(),
            product.and_then(|value| value.get("targetSdkVersion")),
            product.and_then(|value| value.get("compatibleSdkVersion")),
        ]
        .into_iter()
        .flatten()
        .find_map(Self::api_version)
        .unwrap_or(10)
    }

    pub fn explicitly_configured_compile_api_version(&self) -> Option<u32> {
        let product = self.selected_product();
        [
            product.and_then(|value| value.get("compileSdkVersion")),
            self.compile_api_version.as_ref(),
            self.compile_sdk_version.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find_map(Self::api_version)
    }

    pub fn effective_compatible_api_version(&self) -> u32 {
        let compile = self.effective_compile_api_version();
        let product = self.selected_product();
        [
            product.and_then(|value| value.get("compatibleSdkVersion")),
            self.compatible_api_version.as_ref(),
            self.compatible_sdk_version.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find_map(Self::api_version)
        .unwrap_or(compile)
    }

    pub fn uses_ohpm(&self) -> bool {
        self.effective_compile_api_version() >= 9
            && (self.project_oh_package_exists || !self.project_npm_package_exists)
    }

    pub fn selected_product_value(&self) -> Value {
        self.selected_product()
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "name": self.product_name }))
    }

    fn selected_product(&self) -> Option<&Value> {
        self.project_profile
            .pointer("/app/products")
            .and_then(Value::as_array)
            .and_then(|products| {
                products.iter().find(|product| {
                    product.get("name").and_then(Value::as_str) == Some(&self.product_name)
                })
            })
    }

    fn api_version(value: &Value) -> Option<u32> {
        if let Some(value) = value.as_u64() {
            return u32::try_from(value).ok();
        }
        let value = value.as_str()?.trim();
        if let Ok(version) = value.parse() {
            return Some(version);
        }
        value
            .strip_suffix(')')
            .and_then(|value| value.rsplit_once('('))
            .and_then(|(_, api)| api.parse().ok())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeModulePlanRequest {
    pub name: String,
    pub root: PathBuf,
    #[serde(default)]
    pub descriptor: serde_json::Value,
    #[serde(default)]
    pub build_profile: serde_json::Value,
    #[serde(default)]
    pub build_script_override: Option<serde_json::Value>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub package: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptModuleSelection {
    pub module_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimePlan {
    pub module_selections: Vec<BuildScriptModuleSelection>,
    pub modules: Vec<BuildScriptRuntimeModulePlan>,
    pub compile_plugin_records: Vec<BuildScriptCompilePluginRecord>,
    pub task_catalogs: Vec<BuildScriptTaskCatalogPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported_build_diagnostic: Option<BuildScriptRuntimePlanDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeModulePlan {
    pub name: String,
    pub root: PathBuf,
    pub selected_targets: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_target: Option<serde_json::Value>,
    pub build_options: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptTaskCatalogPlan {
    pub node_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub project_task: bool,
    pub project_root: PathBuf,
    pub node_root: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ets_loader_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub js_loader_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_rollup_plugin_in_ets_loader: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_rollup_plugin_in_js_loader: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_target_name: Option<String>,
    pub target_names: Vec<String>,
    pub target_options: Vec<BuildScriptTaskTargetPlan>,
    pub child_modules: Vec<BuildScriptTaskChildModulePlan>,
    pub sync_version_changed: bool,
    pub fast_build_app: bool,
    pub build_app_skip_sign_hap: bool,
    pub command_entry_test: bool,
    pub unit_test_process: bool,
    pub ohos_test_coverage: bool,
    pub preview_process: bool,
    pub preview_compile_resource_incremental: bool,
    pub in_test: bool,
    pub device_test: bool,
    pub hot_reload: bool,
    pub cold_reload: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptTaskTargetPlan {
    pub name: String,
    pub deduplicate_har: bool,
    pub binxo_enabled: bool,
    pub ohpm_project: bool,
    pub compile_api_version: u32,
    pub compatible_api_version: u32,
    pub open_source_har: bool,
    pub external_native_options: bool,
    pub use_normalized_ohm_url: bool,
    pub hsp_dependencies: Vec<BuildScriptTaskDependencyPlan>,
    pub har_dependencies: Vec<BuildScriptTaskDependencyPlan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptTaskDependencyPlan {
    pub module_name: String,
    pub module_root: PathBuf,
    pub target_name: String,
    pub external_native_options: bool,
    pub use_normalized_ohm_url: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptTaskChildModulePlan {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptCompilePluginRecord {
    pub file: PathBuf,
    pub module_root: PathBuf,
    pub target_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimePlanDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(flatten)]
    context: Box<BuildScriptRuntimePlanDiagnosticContext>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildScriptRuntimePlanDiagnosticContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    module_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feature: Option<String>,
}

impl BuildScriptRuntimePlanDiagnostic {
    pub fn module_name(&self) -> Option<&str> {
        self.context.module_name.as_deref()
    }

    pub fn target_name(&self) -> Option<&str> {
        self.context.target_name.as_deref()
    }

    pub fn module(&self) -> Option<&str> {
        self.context.module.as_deref()
    }

    pub fn target(&self) -> Option<&str> {
        self.context.target.as_deref()
    }

    pub fn feature(&self) -> Option<&str> {
        self.context.feature.as_deref()
    }

    fn model(module_name: &str, message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_HVIGOR_BUILD_OPTION_PLAN".to_string(),
            message: message.into(),
            context: Box::new(BuildScriptRuntimePlanDiagnosticContext {
                module_name: Some(module_name.to_string()),
                ..Default::default()
            }),
        }
    }

    fn build_option(module_name: &str, error: crate::ConfigError) -> Self {
        let message = error.to_string();
        let code = if message.contains("circular buildOptionSet copyFrom chain") {
            "CYCLIC_HVIGOR_BUILD_OPTION_SET"
        } else {
            "INVALID_HVIGOR_BUILD_OPTION_PLAN"
        };
        Self {
            code: code.to_string(),
            message,
            context: Box::new(BuildScriptRuntimePlanDiagnosticContext {
                module_name: Some(module_name.to_string()),
                ..Default::default()
            }),
        }
    }

    fn unknown_target(module_name: &str, target_name: &str) -> Self {
        Self {
            code: "UNKNOWN_MODULE_TARGET_IN_COMMAND_LINE".to_string(),
            message: format!(
                "target '{target_name}' was not found for module '{module_name}' in build-profile.json5"
            ),
            context: Box::new(BuildScriptRuntimePlanDiagnosticContext {
                module_name: Some(module_name.to_string()),
                target_name: Some(target_name.to_string()),
                ..Default::default()
            }),
        }
    }

    fn compile_plugin(
        code: &str,
        module_name: &str,
        message: impl Into<String>,
        plugin_path: Option<PathBuf>,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            context: Box::new(BuildScriptRuntimePlanDiagnosticContext {
                module_name: Some(module_name.to_string()),
                plugin_path,
                ..Default::default()
            }),
        }
    }

    fn unsupported_cangjie(module_name: &str, target_name: &str) -> Self {
        Self {
            code: "ARKDOWN_UNSUPPORTED_CANGJIE_BUILD".to_string(),
            message: format!(
                "Cangjie compilation is configured for {module_name}:{target_name}, but Arkdown cannot execute the external @ohos/cangjie-build-support task pipeline yet"
            ),
            context: Box::new(BuildScriptRuntimePlanDiagnosticContext {
                module: Some(module_name.to_string()),
                target: Some(target_name.to_string()),
                feature: Some("cangjieOptions".to_string()),
                ..Default::default()
            }),
        }
    }
}

/// Rust owner for plugin-visible target selection and effective build-option
/// merging. JavaScript supplies values changed by user callbacks and receives
/// an immutable plan for facade construction.
pub struct BuildScriptRuntimePlanner {
    request: BuildScriptRuntimePlanRequest,
}

impl BuildScriptRuntimePlanner {
    pub fn new(request: BuildScriptRuntimePlanRequest) -> Self {
        Self { request }
    }

    pub fn plan(&self) -> Result<BuildScriptRuntimePlan, BuildScriptRuntimePlanDiagnostic> {
        let module_selections =
            BuildScriptModuleSelectionParser::new(self.request.module_selection.as_deref()).parse();
        let profile_modules = self
            .request
            .modules
            .iter()
            .map(|module| BuildOptionProfileModuleValue {
                name: module.name.clone(),
                root: module.root.clone(),
                profile: module.build_profile.clone(),
            })
            .collect::<Vec<_>>();
        let inventory = BuildOptionProfileInventory::from_values(
            &self.request.project_root,
            self.request.project_profile.clone(),
            &profile_modules,
        )
        .map_err(|error| BuildScriptRuntimePlanDiagnostic::model("project", error.to_string()))?;
        let mut modules = Vec::with_capacity(self.request.modules.len());
        let mut compile_plugin_records = Vec::new();
        let mut unsupported_build_diagnostic = None;
        for module in &self.request.modules {
            let targets = BuildScriptTargetSet::from_module(module)?;
            let selected_targets = targets.select(
                &module.name,
                &module_selections,
                &self.request.product_name,
                &self.request.build_mode_name,
                self.request.build_target_name.as_deref(),
                self.request.device_test,
            )?;
            let effective_target =
                targets.effective(&self.request.product_name, &self.request.build_mode_name);
            let target_names = targets.target_names(&selected_targets, effective_target.as_ref());
            let mut build_options = BTreeMap::new();
            for target_name in target_names {
                let mut resolver = EffectiveModuleBuildOptionResolver::new(
                    &inventory,
                    &module.name,
                    &self.request.product_name,
                    &self.request.build_mode_name,
                    &target_name,
                );
                if let Some(value) = module.build_script_override.clone() {
                    resolver = resolver.with_build_script_override(value);
                }
                let option = resolver.resolve_value().map_err(|error| {
                    BuildScriptRuntimePlanDiagnostic::build_option(&module.name, error)
                })?;
                if option
                    .get("cangjieOptions")
                    .and_then(Value::as_object)
                    .is_some_and(|options| options.contains_key("path"))
                {
                    unsupported_build_diagnostic.get_or_insert_with(|| {
                        BuildScriptRuntimePlanDiagnostic::unsupported_cangjie(
                            &module.name,
                            &target_name,
                        )
                    });
                }
                build_options.insert(target_name, option);
            }
            let compile_targets = targets.compile_targets(
                &selected_targets,
                effective_target.as_ref(),
                self.request.build_target_name.is_some(),
            );
            for target in compile_targets {
                let target_name = target
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("default");
                let Some(configured) = build_options
                    .get(target_name)
                    .and_then(|option| option.pointer("/arkOptions/compilePluginFile"))
                else {
                    continue;
                };
                compile_plugin_records.push(
                    BuildScriptCompilePluginPathResolver::new(module, configured)
                        .resolve(target_name)?,
                );
            }
            modules.push(BuildScriptRuntimeModulePlan {
                name: module.name.clone(),
                root: module.root.clone(),
                selected_targets,
                effective_target,
                build_options,
            });
        }
        let task_catalogs =
            BuildScriptTaskCatalogPlanner::new(&self.request, &modules, &module_selections).plan();
        Ok(BuildScriptRuntimePlan {
            module_selections,
            modules,
            compile_plugin_records,
            task_catalogs,
            unsupported_build_diagnostic,
        })
    }
}

/// Derives the complete Rust task-catalog input from the same immutable
/// runtime plan used by build-option resolution. The upstream evidence is
/// `hvigor-ohos-plugin` 6.24.2's task creators and `InjectUtil`: JavaScript
/// exposes task objects to plugins, but it does not own target selection,
/// local HSP/HAR dependency alignment, API gates, or feature predicates.
struct BuildScriptTaskCatalogPlanner<'a> {
    request: &'a BuildScriptRuntimePlanRequest,
    modules: &'a [BuildScriptRuntimeModulePlan],
    selections: &'a [BuildScriptModuleSelection],
    compile_api_version: u32,
    compatible_api_version: u32,
}

impl<'a> BuildScriptTaskCatalogPlanner<'a> {
    fn new(
        request: &'a BuildScriptRuntimePlanRequest,
        modules: &'a [BuildScriptRuntimeModulePlan],
        selections: &'a [BuildScriptModuleSelection],
    ) -> Self {
        let compile_api_version = request.effective_compile_api_version();
        let compatible_api_version = request.effective_compatible_api_version();
        Self {
            request,
            modules,
            selections,
            compile_api_version,
            compatible_api_version,
        }
    }

    fn plan(&self) -> Vec<BuildScriptTaskCatalogPlan> {
        let mut catalogs = Vec::with_capacity(self.modules.len() + 1);
        catalogs.push(self.project_catalog());
        catalogs.extend(
            self.request
                .modules
                .iter()
                .zip(self.modules)
                .map(|(request, plan)| self.module_catalog(request, plan)),
        );
        catalogs
    }

    fn project_catalog(&self) -> BuildScriptTaskCatalogPlan {
        BuildScriptTaskCatalogPlan {
            node_name: self
                .request
                .project_root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".to_string()),
            kind: self.request.project_kind.clone(),
            project_task: true,
            project_root: self.request.project_root.clone(),
            node_root: self.request.project_root.clone(),
            ets_loader_path: self.request.ets_loader_path.clone(),
            js_loader_path: self.request.js_loader_path.clone(),
            has_rollup_plugin_in_ets_loader: self.request.has_rollup_plugin_in_ets_loader,
            has_rollup_plugin_in_js_loader: self.request.has_rollup_plugin_in_js_loader,
            build_target_name: self.request.build_target_name.clone(),
            target_names: vec!["default".to_string()],
            target_options: Vec::new(),
            child_modules: self
                .request
                .modules
                .iter()
                .map(|module| BuildScriptTaskChildModulePlan {
                    name: module.name.clone(),
                    kind: module.kind.clone(),
                })
                .collect(),
            sync_version_changed: self.request.sync_version_changed,
            fast_build_app: self.fast_build_app(),
            build_app_skip_sign_hap: self.build_app_skip_sign_hap(),
            command_entry_test: self.has_command("test"),
            unit_test_process: self.ext_param_is("unitTestMode", "true"),
            ohos_test_coverage: self.ext_param_is("ohos-test-coverage", "true"),
            preview_process: self.preview_process(),
            preview_compile_resource_incremental: self.ext_param_is("compileResInc", "true"),
            in_test: self.ext_param_is("isLocalTest", "true")
                || self.ext_param_is("isOhosTest", "true"),
            device_test: self.has_command("onDeviceTest"),
            hot_reload: self.ext_param_is("hotReload", "true")
                && self
                    .request
                    .start_params
                    .get("daemon")
                    .is_some_and(Self::js_truthy),
            cold_reload: self.has_command("assembleDevHqf"),
        }
    }

    fn module_catalog(
        &self,
        request: &BuildScriptRuntimeModulePlanRequest,
        plan: &BuildScriptRuntimeModulePlan,
    ) -> BuildScriptTaskCatalogPlan {
        let target_names = plan
            .selected_targets
            .iter()
            .filter_map(|target| {
                target
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        let target_options = target_names
            .iter()
            .map(|target_name| self.target_plan(request, plan, target_name))
            .collect();
        let mut catalog = self.project_catalog();
        catalog.node_name = request.name.clone();
        catalog.kind = request.kind.clone();
        catalog.project_task = false;
        catalog.node_root = request.root.clone();
        catalog.target_names = target_names;
        catalog.target_options = target_options;
        catalog.child_modules.clear();
        catalog
    }

    fn target_plan(
        &self,
        request: &BuildScriptRuntimeModulePlanRequest,
        plan: &BuildScriptRuntimeModulePlan,
        target_name: &str,
    ) -> BuildScriptTaskTargetPlan {
        let build_option = Self::build_option(plan, target_name);
        BuildScriptTaskTargetPlan {
            name: target_name.to_string(),
            deduplicate_har: build_option
                .pointer("/packOptions/deduplicateHar")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            binxo_enabled: self.binxo_enabled(build_option),
            ohpm_project: self.compile_api_version >= 9
                && (self.request.project_oh_package_exists
                    || !self.request.project_npm_package_exists),
            compile_api_version: self.compile_api_version,
            compatible_api_version: self.compatible_api_version,
            open_source_har: Self::open_source_har(build_option),
            external_native_options: build_option
                .get("externalNativeOptions")
                .is_some_and(Self::js_truthy),
            use_normalized_ohm_url: build_option
                .pointer("/strictMode/useNormalizedOHMUrl")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            hsp_dependencies: self.dependencies(request, target_name, "hsp"),
            har_dependencies: self.dependencies(request, target_name, "har"),
        }
    }

    fn dependencies(
        &self,
        owner: &BuildScriptRuntimeModulePlanRequest,
        owner_target_name: &str,
        kind: &str,
    ) -> Vec<BuildScriptTaskDependencyPlan> {
        let declarations = ["dependencies", "dynamicDependencies"]
            .into_iter()
            .filter_map(|key| owner.package.get(key).and_then(Value::as_object))
            .flat_map(|entries| entries.iter());
        let mut seen = BTreeSet::new();
        declarations
            .filter_map(|(alias, specifier)| self.local_dependency(owner, alias, specifier))
            .filter(|(request, _)| request.kind.as_deref() == Some(kind))
            .filter(|(request, _)| seen.insert(request.name.clone()))
            .map(|(request, plan)| {
                let target_name =
                    self.dependency_target_name(owner_target_name, request, plan, kind);
                let build_option = Self::build_option(plan, &target_name);
                BuildScriptTaskDependencyPlan {
                    module_name: request.name.clone(),
                    module_root: request.root.clone(),
                    target_name,
                    external_native_options: build_option
                        .get("externalNativeOptions")
                        .is_some_and(Self::js_truthy),
                    use_normalized_ohm_url: build_option
                        .pointer("/strictMode/useNormalizedOHMUrl")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                }
            })
            .collect()
    }

    fn local_dependency(
        &self,
        owner: &BuildScriptRuntimeModulePlanRequest,
        alias: &str,
        specifier: &Value,
    ) -> Option<(
        &'a BuildScriptRuntimeModulePlanRequest,
        &'a BuildScriptRuntimeModulePlan,
    )> {
        self.request
            .modules
            .iter()
            .zip(self.modules)
            .find(|(candidate, _)| {
                if candidate.name == owner.name {
                    return false;
                }
                let package_name = candidate.package.get("name").and_then(Value::as_str);
                if candidate.name == alias || package_name == Some(alias) {
                    return true;
                }
                let Some(relative) = specifier
                    .as_str()
                    .and_then(|value| value.strip_prefix("file:"))
                else {
                    return false;
                };
                lexical_normalize(&owner.root.join(relative)) == lexical_normalize(&candidate.root)
            })
    }

    fn dependency_target_name(
        &self,
        owner_target_name: &str,
        dependency: &BuildScriptRuntimeModulePlanRequest,
        plan: &BuildScriptRuntimeModulePlan,
        kind: &str,
    ) -> String {
        let target_names = plan
            .selected_targets
            .iter()
            .filter_map(|target| target.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let explicit = self
            .selections
            .iter()
            .find(|selection| selection.module_name == dependency.name)
            .and_then(|selection| selection.target_name.as_deref());
        if let Some(target) = explicit.filter(|target| target_names.contains(target)) {
            return target.to_string();
        }
        if kind == "har" && owner_target_name == "ohosTest" && target_names.contains(&"default") {
            return "default".to_string();
        }
        let aligned = self
            .request
            .properties
            .get("ohos.align.target")
            .and_then(Value::as_str);
        for target in [aligned, Some(owner_target_name), Some("default")]
            .into_iter()
            .flatten()
        {
            if target_names.contains(&target) {
                return target.to_string();
            }
        }
        target_names
            .first()
            .copied()
            .unwrap_or("default")
            .to_string()
    }

    fn build_option<'b>(plan: &'b BuildScriptRuntimeModulePlan, target_name: &str) -> &'b Value {
        plan.build_options
            .get(target_name)
            .or_else(|| plan.build_options.get("default"))
            .unwrap_or(&Value::Null)
    }

    fn binxo_enabled(&self, build_option: &Value) -> bool {
        if let Some(configured) = self.request.ext_params.get("ohos-enable-binxo") {
            if Self::js_truthy(configured) {
                return configured.as_str() == Some("true");
            }
        }
        let Some(arguments) = build_option.pointer("/externalNativeOptions/arguments") else {
            return false;
        };
        let values = arguments
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_else(|| std::slice::from_ref(arguments));
        values.iter().filter_map(Value::as_str).any(|value| {
            value
                .split_whitespace()
                .any(|argument| argument == "-DOHOS_ENABLE_BINXO=ON")
        })
    }

    fn open_source_har(build_option: &Value) -> bool {
        let debug = build_option
            .get("debuggable")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        debug
            || build_option
                .pointer("/arkOptions/obfuscation/ruleOptions/enable")
                .and_then(Value::as_bool)
                != Some(true)
    }

    fn fast_build_app(&self) -> bool {
        let enabled = self
            .request
            .selected_product()
            .and_then(|value| value.pointer("/buildOption/packOptions/fastBuildApp"))
            .or_else(|| {
                self.request
                    .project_profile
                    .pointer("/app/buildOption/packOptions/fastBuildApp")
            })
            .and_then(Value::as_bool)
            .unwrap_or(false);
        enabled
            && (self.request.target.as_deref() == Some("app")
                || self
                    .request
                    .command_entry_tasks
                    .iter()
                    .any(|task| task == "assembleApp" || task.ends_with(":assembleApp")))
    }

    fn build_app_skip_sign_hap(&self) -> bool {
        self.request
            .selected_product()
            .and_then(|value| value.pointer("/buildOption/packOptions/buildAppSkipSignHap"))
            .or_else(|| {
                self.request
                    .project_profile
                    .pointer("/app/buildOption/packOptions/buildAppSkipSignHap")
            })
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && self.has_command("assembleApp")
    }

    fn preview_process(&self) -> bool {
        self.ext_param_is("previewMode", "true") || self.has_command("PreviewBuild")
    }

    fn has_command(&self, name: &str) -> bool {
        self.request
            .command_entry_tasks
            .iter()
            .any(|task| task == name)
    }

    fn ext_param_is(&self, name: &str, expected: &str) -> bool {
        self.request.ext_params.get(name).and_then(Value::as_str) == Some(expected)
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

struct BuildScriptModuleSelectionParser<'a> {
    raw: Option<&'a str>,
}

impl<'a> BuildScriptModuleSelectionParser<'a> {
    fn new(raw: Option<&'a str>) -> Self {
        Self { raw }
    }

    fn parse(&self) -> Vec<BuildScriptModuleSelection> {
        let Some(raw) = self.raw.filter(|raw| !raw.is_empty()) else {
            return Vec::new();
        };
        raw.split(',')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let (module_name, target_name) = entry.split_once('@').map_or_else(
                    || (entry, None),
                    |(module_name, target_name)| {
                        (
                            module_name,
                            (!target_name.is_empty()).then_some(target_name),
                        )
                    },
                );
                BuildScriptModuleSelection {
                    module_name: module_name.to_string(),
                    target_name: target_name.map(str::to_string),
                }
            })
            .collect()
    }
}

struct BuildScriptTargetSet {
    values: Vec<serde_json::Value>,
    typed: Vec<RawTarget>,
}

impl BuildScriptTargetSet {
    fn from_module(
        module: &BuildScriptRuntimeModulePlanRequest,
    ) -> Result<Self, BuildScriptRuntimePlanDiagnostic> {
        let descriptor_targets = module
            .descriptor
            .get("targets")
            .and_then(serde_json::Value::as_array);
        // `CoreModuleModelImpl.getOrDefaultTargets()` owns the module target
        // list and reads it from the module-level build profile. Project-level
        // `modules[].targets` only supplies product applicability through
        // `CoreProjectModelImpl.getTargetApplyProducts()`; it must not hide an
        // `ohosTest` (or any other target) declared by the module.
        let mut values = module
            .build_profile
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !values
            .iter()
            .any(|target| target.get("name").and_then(serde_json::Value::as_str) == Some("default"))
        {
            values.push(serde_json::json!({ "name": "default" }));
        }
        for target in &mut values {
            let Some(name) = target
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            let descriptor_target = descriptor_targets.and_then(|targets| {
                targets.iter().find(|candidate| {
                    candidate.get("name").and_then(serde_json::Value::as_str) == Some(name.as_str())
                })
            });
            if let (Some(target), Some(descriptor_target)) = (
                target.as_object_mut(),
                descriptor_target.and_then(serde_json::Value::as_object),
            ) {
                // Project target metadata supplies product/build-mode/output
                // defaults. Module target fields remain authoritative when
                // both profiles declare the same key.
                for (key, value) in descriptor_target {
                    target.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }
        let typed = values
            .iter()
            .enumerate()
            .map(|(index, target)| {
                serde_json::from_value(target.clone()).map_err(|error| {
                    BuildScriptRuntimePlanDiagnostic::model(
                        &module.name,
                        format!("invalid targets[{index}] in build-profile.json5: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { values, typed })
    }

    fn selected_module_entries<'a>(
        module_name: &str,
        selections: &'a [BuildScriptModuleSelection],
    ) -> Vec<&'a BuildScriptModuleSelection> {
        selections
            .iter()
            .filter(|selection| selection.module_name == module_name)
            .collect()
    }

    fn applies(target: &RawTarget, product_name: &str, build_mode_name: &str) -> bool {
        let product_matches = if target.apply_to_products.is_empty() {
            product_name == "default"
        } else {
            target
                .apply_to_products
                .iter()
                .any(|product| product == product_name)
        };
        product_matches
            && (target.build_mode_set.is_empty()
                || target
                    .build_mode_set
                    .iter()
                    .any(|mode| mode.name == build_mode_name))
    }

    fn applicable_indices(&self, product_name: &str, build_mode_name: &str) -> Vec<usize> {
        self.typed
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                Self::applies(target, product_name, build_mode_name).then_some(index)
            })
            .collect()
    }

    fn effective_index(&self, product_name: &str, build_mode_name: &str) -> Option<usize> {
        let selected = pick_effective_raw_target(&self.typed, product_name, build_mode_name)?;
        self.typed
            .iter()
            .position(|target| std::ptr::eq(target, selected))
    }

    fn effective(&self, product_name: &str, build_mode_name: &str) -> Option<serde_json::Value> {
        self.effective_index(product_name, build_mode_name)
            .map(|index| self.values[index].clone())
    }

    fn select(
        &self,
        module_name: &str,
        selections: &[BuildScriptModuleSelection],
        product_name: &str,
        build_mode_name: &str,
        build_target_name: Option<&str>,
        device_test: bool,
    ) -> Result<Vec<serde_json::Value>, BuildScriptRuntimePlanDiagnostic> {
        let selected_entries = Self::selected_module_entries(module_name, selections);
        let applicable = self.applicable_indices(product_name, build_mode_name);
        // `ModuleTaskService.targetNeedPack()` overwrites the normal
        // computeTargets result for `onDeviceTest`: only `default` and
        // `ohosTest` remain selected, while module filtering and product
        // applicability still apply.
        if device_test {
            return Ok(applicable
                .into_iter()
                .filter(|index| matches!(self.typed[*index].name.as_str(), "default" | "ohosTest"))
                .map(|index| self.values[index].clone())
                .collect());
        }
        let mut selected_target_names = Vec::new();
        let mut seen_target_names = BTreeSet::new();
        for target_name in selected_entries
            .into_iter()
            .filter_map(|selection| selection.target_name.as_deref())
        {
            if seen_target_names.insert(target_name) {
                selected_target_names.push(target_name);
            }
        }
        if !selected_target_names.is_empty() {
            let mut selected = Vec::with_capacity(selected_target_names.len());
            for target_name in selected_target_names {
                let Some(index) = applicable
                    .iter()
                    .copied()
                    .find(|index| self.typed[*index].name == target_name)
                else {
                    return Err(BuildScriptRuntimePlanDiagnostic::unknown_target(
                        module_name,
                        target_name,
                    ));
                };
                selected.push(self.values[index].clone());
            }
            return Ok(selected);
        }
        if let Some(target_name) = build_target_name {
            let selected = applicable
                .iter()
                .copied()
                .find(|index| self.typed[*index].name == target_name)
                .or_else(|| self.effective_index(product_name, build_mode_name));
            return Ok(vec![selected.map_or_else(
                || serde_json::json!({ "name": "default" }),
                |index| self.values[index].clone(),
            )]);
        }
        // Hvigor 6.24.3 `TargetUtils.computeTargets()` treats a module with no
        // module-specific CLI entry as `all`, even when another module was
        // explicitly selected. Every module therefore still registers its
        // applicable target tasks; command roots decide which outputs execute.
        // This keeps local HSP/HAR dependency targets addressable from a
        // selected HAP module.
        Ok(applicable
            .into_iter()
            .filter(|index| self.typed[*index].name != "ohosTest")
            .map(|index| self.values[index].clone())
            .collect())
    }

    fn target_names(
        &self,
        selected_targets: &[serde_json::Value],
        effective_target: Option<&serde_json::Value>,
    ) -> BTreeSet<String> {
        let mut names = self
            .values
            .iter()
            .chain(selected_targets)
            .filter_map(|target| target.get("name").and_then(serde_json::Value::as_str))
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        if let Some(name) = effective_target
            .and_then(|target| target.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            names.insert(name.to_string());
        }
        names.insert("default".to_string());
        names
    }

    fn compile_targets(
        &self,
        selected_targets: &[serde_json::Value],
        effective_target: Option<&serde_json::Value>,
        has_explicit_build_target: bool,
    ) -> Vec<serde_json::Value> {
        let mut targets = selected_targets.to_vec();
        if !has_explicit_build_target {
            if let Some(effective) = effective_target {
                let effective_name = effective.get("name").and_then(serde_json::Value::as_str);
                if !targets.iter().any(|target| {
                    target.get("name").and_then(serde_json::Value::as_str) == effective_name
                }) {
                    targets.push(effective.clone());
                }
            }
        }
        if targets.is_empty() {
            targets.push(serde_json::json!({ "name": "default" }));
        }
        targets
    }
}

struct BuildScriptCompilePluginPathResolver<'a> {
    module: &'a BuildScriptRuntimeModulePlanRequest,
    configured: &'a serde_json::Value,
}

impl<'a> BuildScriptCompilePluginPathResolver<'a> {
    fn new(
        module: &'a BuildScriptRuntimeModulePlanRequest,
        configured: &'a serde_json::Value,
    ) -> Self {
        Self { module, configured }
    }

    fn resolve(
        &self,
        target_name: &str,
    ) -> Result<BuildScriptCompilePluginRecord, BuildScriptRuntimePlanDiagnostic> {
        let Some(configured) = self
            .configured
            .as_str()
            .filter(|configured| !configured.is_empty())
        else {
            return Err(BuildScriptRuntimePlanDiagnostic::compile_plugin(
                "INVALID_COMPILE_PLUGIN_FILE",
                &self.module.name,
                format!(
                    "'compilePluginFile' must be a non-empty string in {}.",
                    self.module.root.join("build-profile.json5").display()
                ),
                None,
            ));
        };
        if !configured.starts_with(COMPILE_PLUGIN_PREFIX) {
            return Err(BuildScriptRuntimePlanDiagnostic::compile_plugin(
                "INVALID_COMPILE_PLUGIN_PREFIX",
                &self.module.name,
                format!("'compilePluginFile' must start with '{COMPILE_PLUGIN_PREFIX}'."),
                Some(PathBuf::from(configured)),
            ));
        }
        let compile_root =
            lexical_normalize(&self.module.root.join("build").join("hvigor_compile"));
        let mut resolved = lexical_normalize(&self.module.root.join(configured));
        let inside = resolved
            .strip_prefix(&compile_root)
            .ok()
            .is_some_and(|relative| !relative.as_os_str().is_empty());
        if !inside {
            return Err(BuildScriptRuntimePlanDiagnostic::compile_plugin(
                "COMPILE_PLUGIN_OUTSIDE_HVIGOR_DIRECTORY",
                &self.module.name,
                format!(
                    "'compilePluginFile' is not under directory: '{}'.",
                    compile_root.display()
                ),
                Some(resolved),
            ));
        }
        let file_name = resolved
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        if let Some(index) = file_name.rfind('.') {
            if index > 0 && &file_name[index..] != ".ts" {
                return Err(BuildScriptRuntimePlanDiagnostic::compile_plugin(
                    "PLUGIN_FILE_MUST_END_WITH_TS",
                    &self.module.name,
                    "'compilePluginFile' must end with .ts.",
                    Some(resolved),
                ));
            }
        }
        if !resolved.exists() {
            let mut suffixed = OsString::from(resolved.as_os_str());
            suffixed.push(".ts");
            resolved = PathBuf::from(suffixed);
        }
        if !resolved.exists() {
            return Err(BuildScriptRuntimePlanDiagnostic::compile_plugin(
                "PLUGIN_PATH_NOT_FOUND",
                &self.module.name,
                "'compilePluginFile' does not exist.",
                Some(resolved),
            ));
        }
        Ok(BuildScriptCompilePluginRecord {
            file: resolved,
            module_root: self.module.root.clone(),
            target_name: target_name.to_string(),
        })
    }
}
