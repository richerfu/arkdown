use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arkdown_config::{
    BuildScriptCompilePluginRecord, BuildScriptModuleSelection, BuildScriptRuntimeModulePlan,
    BuildScriptRuntimeModulePlanRequest, BuildScriptRuntimePlan, BuildScriptRuntimePlanDiagnostic,
    BuildScriptRuntimePlanRequest, BuildScriptRuntimePlanner, BuildScriptTaskCatalogPlan,
    HvigorRuntimeParameterPlan, HvigorRuntimeParameterPlanRequest, HvigorRuntimeParameterPlanner,
    HvigorRuntimePropertyDiagnostic, HvigorRuntimePropertyPlan, HvigorRuntimePropertyPlanRequest,
    HvigorRuntimePropertyPlanner,
};
use arkdown_path::{canonicalize_or_owned, lexical_normalize};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    HvigorContextInspectError, HvigorContextModuleState, HvigorContextMutation,
    HvigorContextMutationDiagnostic, HvigorContextPathOptions, HvigorContextSession,
    HvigorDependencyContextSnapshot, HvigorModuleTargetContextPaths, HvigorProjectContextPaths,
    HvigorSdkDetailsError, HvigorSdkDetailsInventory, HvigorSdkDetailsRequest,
};

use super::runtime_model::{
    BuildScriptModuleModelLoader, BuildScriptModuleModelRequest, BuildScriptModuleModelSnapshot,
    BuildScriptRuntimeModelError, BuildScriptRuntimeModelSnapshot,
};

/// Values that can change while official Hvigor plugins are evaluating.
/// Static project and CLI inputs stay in the session-owned plan request.
pub struct BuildScriptRuntimePlanUpdate {
    pub product_name: String,
    pub build_mode_name: String,
    pub module_selection: Option<String>,
    pub properties: Map<String, Value>,
    pub ext_params: Map<String, Value>,
    pub start_params: Map<String, Value>,
    pub sync_version_changed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BuildScriptRuntimeSdkConfig {
    pub sdk_path: Option<PathBuf>,
    pub ets_loader_path: Option<PathBuf>,
    pub runtime_os: Option<String>,
    pub ets_component_version: Option<String>,
    pub ets_component_release_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeSdkDetails {
    pub is_ohos: bool,
    pub sdk_dir: PathBuf,
    pub sdk_version: u32,
    pub ets_component_version: String,
    pub ets_component_release_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeSyncSnapshot {
    pub project_name: String,
    pub selected_product_name: String,
    pub mode: String,
    pub project_profile: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_json: Option<Value>,
    pub properties: Map<String, Value>,
    pub modules: Vec<BuildScriptRuntimeSyncModule>,
    pub sync_version_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeSyncModule {
    pub name: String,
    pub root: PathBuf,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belong_project_path: Option<PathBuf>,
    pub module_profile: Value,
    pub selected_targets: Vec<BuildScriptRuntimeSyncTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeSyncTarget {
    pub name: String,
    pub build_option: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeCommandSnapshot {
    pub project_name: String,
    pub sync_version_changed: bool,
    pub task_names: Vec<String>,
    pub module_selections: Vec<BuildScriptModuleSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_target_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeCleanSnapshot {
    pub project_root: PathBuf,
    pub node_root: PathBuf,
    pub project_task: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_build_dir: Option<String>,
    pub project_profile: Value,
    pub target_names: Vec<String>,
    pub build_mode: String,
}

#[derive(Debug)]
pub enum BuildScriptRuntimeExecutionSnapshotError {
    PlanUnavailable { operation: &'static str },
    ModulePlanMissing { root: PathBuf },
    TargetNameMissing { root: PathBuf, target: Value },
}

impl std::fmt::Display for BuildScriptRuntimeExecutionSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlanUnavailable { operation } => write!(
                formatter,
                "ARKDOWN_BUILD_SCRIPT_RUNTIME_PLAN_UNAVAILABLE: runtime plan must be prepared before {operation} projection",
            ),
            Self::ModulePlanMissing { root } => write!(
                formatter,
                "ARKDOWN_BUILD_SCRIPT_RUNTIME_MODULE_NOT_FOUND: runtime plan does not contain module at {}",
                root.display(),
            ),
            Self::TargetNameMissing { root, target } => write!(
                formatter,
                "ARKDOWN_INVALID_HVIGOR_CLEAN_TARGET: module at {} contains a selected target without a name: {target}",
                root.display(),
            ),
        }
    }
}

impl std::error::Error for BuildScriptRuntimeExecutionSnapshotError {}

#[derive(Debug)]
pub enum BuildScriptRuntimeSyncSnapshotError {
    PlanUnavailable,
    ModulePlanMissing { name: String, root: PathBuf },
    TargetNameMissing { name: String, target: Value },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildScriptCompilePluginPathError {
    Empty,
    NotFound { path: PathBuf },
    InvalidExtension { path: PathBuf },
}

impl BuildScriptCompilePluginPathError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "EMPTY_COMPILE_PLUGIN_PATH",
            Self::NotFound { .. } => "PLUGIN_PATH_NOT_FOUND",
            Self::InvalidExtension { .. } => "PLUGIN_FILE_MUST_END_WITH_TS",
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Empty => None,
            Self::NotFound { path } | Self::InvalidExtension { path } => Some(path),
        }
    }
}

impl std::fmt::Display for BuildScriptCompilePluginPathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("compile plugin path must not be empty"),
            Self::NotFound { path } => {
                write!(
                    formatter,
                    "compile plugin path does not exist: {}",
                    path.display()
                )
            }
            Self::InvalidExtension { path } => {
                write!(
                    formatter,
                    "compile plugin file must end with .ts: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for BuildScriptCompilePluginPathError {}

impl std::fmt::Display for BuildScriptRuntimeSyncSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlanUnavailable => formatter.write_str(
                "ARKDOWN_BUILD_SCRIPT_RUNTIME_PLAN_UNAVAILABLE: runtime plan must be prepared before sync projection",
            ),
            Self::ModulePlanMissing { name, root } => write!(
                formatter,
                "ARKDOWN_BUILD_SCRIPT_RUNTIME_MODULE_NOT_FOUND: runtime plan does not contain module {name:?} at {}",
                root.display()
            ),
            Self::TargetNameMissing { name, target } => write!(
                formatter,
                "ARKDOWN_INVALID_HVIGOR_SYNC_TARGET: module {name:?} contains a selected target without a name: {target}",
            ),
        }
    }
}

impl std::error::Error for BuildScriptRuntimeSyncSnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimePathPlan {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_file_override: Option<String>,
    pub hvigor_cache_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hvigor_cache_warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_build_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_cache_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_cache_dir: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_dir: Option<Value>,
    pub preview_mode: bool,
    pub local_test: bool,
    pub ohos_test: bool,
}

/// Build-script model and runtime-plan owner shared by the N-API facade.
///
/// The session reads module files only through the startup model loaders. Live
/// JavaScript nodes submit typed snapshots after plugin callbacks mutate them;
/// target selection, build-option merging, path identity, and plan caching do
/// not return to JavaScript.
pub struct BuildScriptRuntimeSession {
    model: BuildScriptRuntimeModelSnapshot,
    module_models: BTreeMap<PathBuf, BuildScriptModuleModelSnapshot>,
    descriptors: BTreeMap<PathBuf, Value>,
    descriptor_order: Vec<PathBuf>,
    project_hvigor_overrides: Value,
    module_overrides: BTreeMap<PathBuf, Value>,
    module_override_roots: BTreeMap<PathBuf, PathBuf>,
    request: BuildScriptRuntimePlanRequest,
    parameter_plan: HvigorRuntimeParameterPlan,
    context: HvigorContextSession,
    plan: Option<BuildScriptRuntimePlan>,
    sdk_details: Option<BuildScriptRuntimeSdkDetails>,
    package_model_updated: bool,
}

impl BuildScriptRuntimeSession {
    pub fn new(
        model: BuildScriptRuntimeModelSnapshot,
        mut request: BuildScriptRuntimePlanRequest,
        parameter_request: HvigorRuntimeParameterPlanRequest,
        product_is_explicit: bool,
    ) -> Self {
        request.project_root = model.project_root.clone();
        request.project_profile = model.project_profile.clone();
        request.project_oh_package_exists = model.project_oh_package_exists;
        request.project_npm_package_exists = model.project_npm_package_exists;

        let uses_ohpm = request.uses_ohpm();
        let mut module_models = BTreeMap::new();
        let mut descriptors = BTreeMap::new();
        let mut descriptor_order = Vec::with_capacity(model.modules.len());
        let mut modules = Vec::with_capacity(model.modules.len());
        for module in &model.modules {
            let key = Self::root_key(&module.root);
            let descriptor = Self::prepared_descriptor(module, &Value::Null, uses_ohpm);
            modules.push(Self::module_request(&descriptor));
            descriptor_order.push(key.clone());
            descriptors.insert(key.clone(), descriptor);
            module_models.insert(key, module.clone());
        }
        request.modules = modules;
        let project_package = if uses_ohpm {
            model.project_package.clone()
        } else {
            model.project_npm_package.clone()
        };
        let context_modules = request
            .modules
            .iter()
            .map(|module| HvigorContextModuleState {
                root: module.root.clone(),
                package: module.package.clone(),
            })
            .collect();
        let context = HvigorContextSession::new(
            model.project_root.clone(),
            request.product_name.clone(),
            product_is_explicit,
            model.project_profile.clone(),
            project_package,
            context_modules,
        );

        Self {
            model,
            module_models,
            descriptors,
            descriptor_order,
            project_hvigor_overrides: Value::Object(Map::new()),
            module_overrides: BTreeMap::new(),
            module_override_roots: BTreeMap::new(),
            request,
            parameter_plan: HvigorRuntimeParameterPlanner::new(parameter_request).plan(),
            context,
            plan: None,
            sdk_details: None,
            package_model_updated: false,
        }
    }

    pub fn project_model(&self) -> &BuildScriptRuntimeModelSnapshot {
        &self.model
    }

    pub fn uses_ohpm(&self) -> bool {
        self.request.uses_ohpm()
    }

    pub fn effective_compile_api_version(&self) -> u32 {
        self.request.effective_compile_api_version()
    }

    pub fn build_target_name(&self) -> Option<&str> {
        self.request.build_target_name.as_deref()
    }

    pub fn output_target(&self) -> &str {
        self.request
            .target
            .as_deref()
            .filter(|target| !target.is_empty())
            .unwrap_or("hap")
    }

    pub fn compile_plugin_path(
        &self,
        module_root: &Path,
        plugin_path: &str,
    ) -> Result<PathBuf, BuildScriptCompilePluginPathError> {
        if plugin_path.is_empty() {
            return Err(BuildScriptCompilePluginPathError::Empty);
        }
        let configured = PathBuf::from(plugin_path);
        let configured = if configured.is_absolute() {
            configured
        } else {
            module_root.join(configured)
        };
        let resolved = lexical_normalize(&configured);
        if !resolved.exists() {
            return Err(BuildScriptCompilePluginPathError::NotFound { path: resolved });
        }
        if !resolved.to_string_lossy().ends_with(".ts") {
            return Err(BuildScriptCompilePluginPathError::InvalidExtension { path: resolved });
        }
        Ok(resolved)
    }

    pub fn command_is_clean_only(&self) -> bool {
        self.request.command_entry_tasks.as_slice() == ["clean"]
    }

    pub fn fast_build_app(&self) -> Result<bool, BuildScriptRuntimeExecutionSnapshotError> {
        let plan = self.plan.as_ref().ok_or(
            BuildScriptRuntimeExecutionSnapshotError::PlanUnavailable {
                operation: "native build configuration",
            },
        )?;
        Ok(plan
            .task_catalogs
            .iter()
            .find(|catalog| catalog.project_task)
            .is_some_and(|catalog| catalog.fast_build_app))
    }

    pub fn task_catalog(
        &self,
        node_name: &str,
        project_task: bool,
    ) -> Option<&BuildScriptTaskCatalogPlan> {
        self.plan
            .as_ref()?
            .task_catalogs
            .iter()
            .find(|catalog| catalog.project_task == project_task && catalog.node_name == node_name)
    }

    pub fn compile_plugin_records(&self) -> Option<&[BuildScriptCompilePluginRecord]> {
        Some(&self.plan.as_ref()?.compile_plugin_records)
    }

    pub fn explicitly_configured_compile_api_version(&self) -> Option<u32> {
        self.request.explicitly_configured_compile_api_version()
    }

    pub fn selected_product(&self) -> Value {
        self.request.selected_product_value()
    }

    pub fn project_build_profile(&self) -> Value {
        self.model.project_profile.clone()
    }

    pub fn project_app_json(&self) -> Value {
        self.model.app_json.clone().unwrap_or(Value::Null)
    }

    pub fn project_package(&self) -> Value {
        self.context.project_package().clone()
    }

    pub fn module_build_profile(&self, root: &Path) -> Option<Value> {
        self.module_descriptor(root)
            .and_then(|descriptor| descriptor.get("moduleProfile").cloned())
    }

    pub fn module_json(&self, root: &Path) -> Option<Value> {
        self.module_descriptor(root)
            .and_then(|descriptor| descriptor.get("moduleJson").cloned())
    }

    pub fn ohpm_module_packages(&self) -> Vec<Value> {
        self.descriptor_order
            .iter()
            .filter_map(|key| self.descriptors.get(key))
            .filter_map(|descriptor| {
                let name = descriptor.get("name")?.clone();
                let root = descriptor.get("root")?.clone();
                let root_path = root.as_str().map(PathBuf::from)?;
                let package = self
                    .context
                    .module_package_snapshot(&root_path)
                    .unwrap_or(Value::Null);
                Some(serde_json::json!({ "name": name, "root": root, "package": package }))
            })
            .collect()
    }

    pub fn package_model_updated(&self) -> bool {
        self.package_model_updated
    }

    pub fn command_snapshot(
        &self,
    ) -> Result<BuildScriptRuntimeCommandSnapshot, BuildScriptRuntimeExecutionSnapshotError> {
        let plan = self.plan.as_ref().ok_or(
            BuildScriptRuntimeExecutionSnapshotError::PlanUnavailable {
                operation: "command",
            },
        )?;
        Ok(BuildScriptRuntimeCommandSnapshot {
            project_name: self.project_name(),
            sync_version_changed: self.request.sync_version_changed,
            task_names: self.request.command_entry_tasks.clone(),
            module_selections: plan.module_selections.clone(),
            build_target_name: self.request.build_target_name.clone(),
        })
    }

    pub fn clean_snapshot(
        &self,
        node_root: &Path,
        project_task: bool,
    ) -> Result<BuildScriptRuntimeCleanSnapshot, BuildScriptRuntimeExecutionSnapshotError> {
        let plan = self.plan.as_ref().ok_or(
            BuildScriptRuntimeExecutionSnapshotError::PlanUnavailable { operation: "clean" },
        )?;
        let target_names = if project_task {
            Vec::new()
        } else {
            let root_key = Self::root_key(node_root);
            let module = plan
                .modules
                .iter()
                .find(|module| Self::root_key(&module.root) == root_key)
                .ok_or_else(
                    || BuildScriptRuntimeExecutionSnapshotError::ModulePlanMissing {
                        root: node_root.to_path_buf(),
                    },
                )?;
            module
                .selected_targets
                .iter()
                .map(|target| {
                    target
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            BuildScriptRuntimeExecutionSnapshotError::TargetNameMissing {
                                root: node_root.to_path_buf(),
                                target: target.clone(),
                            }
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let path_plan = self.runtime_path_plan(&self.request.properties, &self.request.ext_params);
        Ok(BuildScriptRuntimeCleanSnapshot {
            project_root: self.model.project_root.clone(),
            node_root: node_root.to_path_buf(),
            project_task,
            configured_build_dir: path_plan.configured_build_dir,
            project_profile: self.model.project_profile.clone(),
            target_names,
            build_mode: self.request.build_mode_name.clone(),
        })
    }

    pub fn sync_snapshot(
        &self,
    ) -> Result<BuildScriptRuntimeSyncSnapshot, BuildScriptRuntimeSyncSnapshotError> {
        let plan = self
            .plan
            .as_ref()
            .ok_or(BuildScriptRuntimeSyncSnapshotError::PlanUnavailable)?;
        let mut modules = Vec::new();
        for module in &self.request.modules {
            let Some(kind) = module
                .kind
                .as_deref()
                .filter(|kind| matches!(*kind, "hap" | "hsp" | "har"))
            else {
                continue;
            };
            let planned = plan
                .modules
                .iter()
                .find(|candidate| Self::root_key(&candidate.root) == Self::root_key(&module.root))
                .ok_or_else(|| BuildScriptRuntimeSyncSnapshotError::ModulePlanMissing {
                    name: module.name.clone(),
                    root: module.root.clone(),
                })?;
            let selected_targets = if planned.selected_targets.is_empty() {
                vec![serde_json::json!({ "name": "default" })]
            } else {
                planned.selected_targets.clone()
            };
            let selected_targets = selected_targets
                .into_iter()
                .map(|target| {
                    let name = target.get("name").and_then(Value::as_str).ok_or_else(|| {
                        BuildScriptRuntimeSyncSnapshotError::TargetNameMissing {
                            name: module.name.clone(),
                            target: target.clone(),
                        }
                    })?;
                    let build_option = planned
                        .build_options
                        .get(name)
                        .or_else(|| planned.build_options.get("default"))
                        .cloned()
                        .unwrap_or_else(|| Value::Object(Map::new()));
                    Ok(BuildScriptRuntimeSyncTarget {
                        name: name.to_string(),
                        build_option,
                    })
                })
                .collect::<Result<Vec<_>, BuildScriptRuntimeSyncSnapshotError>>()?;
            modules.push(BuildScriptRuntimeSyncModule {
                name: module.name.clone(),
                root: module.root.clone(),
                kind: kind.to_string(),
                belong_project_path: module
                    .descriptor
                    .get("belongProjectPath")
                    .and_then(Value::as_str)
                    .map(PathBuf::from),
                module_profile: module.build_profile.clone(),
                selected_targets,
            });
        }
        Ok(BuildScriptRuntimeSyncSnapshot {
            project_name: self.project_name(),
            selected_product_name: self.request.product_name.clone(),
            mode: self.request.build_mode_name.clone(),
            project_profile: self.model.project_profile.clone(),
            app_json: self.model.app_json.clone(),
            properties: self.request.properties.clone(),
            modules,
            sync_version_changed: self.request.sync_version_changed,
        })
    }

    pub fn parameter_plan(&self) -> &HvigorRuntimeParameterPlan {
        &self.parameter_plan
    }

    pub fn runtime_context_overrides(&self) -> Option<arkdown_config::RuntimeContextOverrides> {
        self.context.runtime_overrides()
    }

    pub fn project_hvigor_overrides(&self) -> &Value {
        &self.project_hvigor_overrides
    }

    pub fn module_hvigor_overrides(&self) -> BTreeMap<&Path, &Value> {
        self.module_overrides
            .iter()
            .map(|(key, value)| {
                let root = self
                    .module_override_roots
                    .get(key)
                    .map(PathBuf::as_path)
                    .unwrap_or(key.as_path());
                (root, value)
            })
            .collect()
    }

    pub fn merge_project_hvigor_override(&mut self, field: &str, value: &Value) {
        let overrides = self
            .project_hvigor_overrides
            .as_object_mut()
            .expect("project Hvigor overrides are initialized as an object");
        let merged = overrides
            .get(field)
            .map(|current| Self::merge_json_values(current, value))
            .unwrap_or_else(|| value.clone());
        overrides.insert(field.to_string(), merged);
    }

    pub fn module_hvigor_override(&self, root: &Path) -> Option<&Value> {
        self.module_overrides.get(&Self::root_key(root))
    }

    pub fn merge_json_values(base: &Value, overlay: &Value) -> Value {
        let (Some(base), Some(overlay)) = (base.as_object(), overlay.as_object()) else {
            return overlay.clone();
        };
        let mut merged = base.clone();
        for (key, value) in overlay {
            let value = merged
                .get(key)
                .map(|base| Self::merge_json_values(base, value))
                .unwrap_or_else(|| value.clone());
            merged.insert(key.clone(), value);
        }
        Value::Object(merged)
    }

    pub fn selected_product_bundle_name(&self) -> Option<String> {
        if let Some(bundle_name) = self
            .project_hvigor_overrides
            .pointer("/appOpt/bundleName")
            .and_then(Value::as_str)
        {
            return Some(bundle_name.to_string());
        }
        self.selected_product()
            .get("bundleName")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn set_context_project_build_profile(
        &mut self,
        profile: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        let mutation = self.context.set_project_build_profile(profile)?;
        self.request.project_profile = mutation.value.clone();
        self.model.project_profile = mutation.value.clone();
        if let Some(product_name) = &mutation.selected_product_name {
            self.request.product_name = product_name.clone();
        }
        self.sdk_details = None;
        self.invalidate();
        Ok(mutation)
    }

    pub fn set_context_project_app_json(&mut self, app_json: Value) -> HvigorContextMutation {
        let mutation = self.context.set_project_app_json(app_json);
        self.model.app_json = Some(mutation.value.clone());
        mutation
    }

    pub fn set_context_module_build_profile(
        &mut self,
        root: &Path,
        profile: Value,
    ) -> HvigorContextMutation {
        let mutation = self.context.set_module_build_profile(root, profile);
        let key = Self::root_key(root);
        if let Some(module) = self
            .request
            .modules
            .iter_mut()
            .find(|module| Self::root_key(&module.root) == key)
        {
            module.build_profile = mutation.value.clone();
        }
        if let Some(descriptor) = self.descriptors.get_mut(&key) {
            if let Some(descriptor) = descriptor.as_object_mut() {
                descriptor.insert("moduleProfile".to_string(), mutation.value.clone());
            }
        }
        self.invalidate();
        mutation
    }

    pub fn set_context_module_json(
        &mut self,
        root: &Path,
        module_json: Value,
    ) -> HvigorContextMutation {
        let mutation = self.context.set_module_json(root, module_json);
        let key = Self::root_key(root);
        if let Some(descriptor) = self.descriptors.get_mut(&key) {
            if let Some(descriptor) = descriptor.as_object_mut() {
                descriptor.insert("moduleJson".to_string(), mutation.value.clone());
            }
        }
        mutation
    }

    pub fn project_package_field(&self, field: &str) -> Option<Value> {
        self.context.project_package_field(field)
    }

    pub fn module_package_field(&self, root: &Path, field: &str) -> Option<Value> {
        self.context.module_package_field(root, field)
    }

    pub fn set_context_project_package_field(
        &mut self,
        field: &str,
        value: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        let mutation = self.context.set_project_package_field(field, value)?;
        self.package_model_updated = true;
        Ok(mutation)
    }

    pub fn set_context_module_package_field(
        &mut self,
        root: &Path,
        field: &str,
        value: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        let mutation = self.context.set_module_package_field(root, field, value)?;
        self.update_module_package(root, &mutation.value);
        self.package_model_updated = true;
        Ok(mutation)
    }

    pub fn set_context_module_version(
        &mut self,
        root: &Path,
        version: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        let mutation = self.context.set_module_version(root, version)?;
        self.update_module_package(root, &mutation.value);
        self.package_model_updated = true;
        Ok(mutation)
    }

    pub fn dependency_context(
        &mut self,
        root: Option<&Path>,
    ) -> Result<HvigorDependencyContextSnapshot, HvigorContextInspectError> {
        self.context.dependency_context(root)
    }

    pub fn all_module_name_hash(&mut self) -> Result<String, HvigorContextInspectError> {
        self.context.all_module_name_hash()
    }

    pub fn project_context_paths(
        &self,
        options: &HvigorContextPathOptions,
    ) -> Result<HvigorProjectContextPaths, HvigorContextInspectError> {
        self.context.project_paths(options)
    }

    pub fn module_target_context_paths(
        &self,
        root: &Path,
        target_name: &str,
        options: &HvigorContextPathOptions,
    ) -> Result<HvigorModuleTargetContextPaths, HvigorContextInspectError> {
        self.context.module_target_paths(root, target_name, options)
    }

    pub fn runtime_property_plan(
        &self,
        properties: Map<String, Value>,
        ext_params: Map<String, Value>,
        core_start_params: Map<String, Value>,
    ) -> Result<HvigorRuntimePropertyPlan, HvigorRuntimePropertyDiagnostic> {
        HvigorRuntimePropertyPlanner::new(HvigorRuntimePropertyPlanRequest {
            properties,
            ext_params,
            core_start_params,
        })
        .plan()
    }

    pub fn runtime_path_plan(
        &self,
        properties: &Map<String, Value>,
        ext_params: &Map<String, Value>,
    ) -> BuildScriptRuntimePathPlan {
        let custom_cache = Self::nullish_value(ext_params.get("build-cache-dir"))
            .or_else(|| Self::nullish_value(properties.get("hvigor.cacheDir")));
        let (hvigor_cache_dir, hvigor_cache_warning) = self.resolve_hvigor_cache_dir(custom_cache);
        BuildScriptRuntimePathPlan {
            parameter_file_override: self.parameter_file_override(ext_params),
            hvigor_cache_dir,
            hvigor_cache_warning,
            configured_build_dir: Self::nullish_value(ext_params.get("build-cache-dir"))
                .or_else(|| Self::nullish_value(properties.get("ohos.buildDir")))
                .and_then(Value::as_str)
                .map(str::to_string),
            configured_cache_dir: custom_cache.and_then(Value::as_str).map(str::to_string),
            build_cache_dir: ext_params.get("build-cache-dir").cloned(),
            build_dir: properties.get("ohos.buildDir").cloned(),
            preview_mode: ext_params.get("previewMode").and_then(Value::as_str) == Some("true"),
            local_test: ext_params.get("isLocalTest").and_then(Value::as_str) == Some("true"),
            ohos_test: ext_params.get("isOhosTest").and_then(Value::as_str) == Some("true"),
        }
    }

    pub fn module_descriptors(&self) -> Vec<Value> {
        self.descriptor_order
            .iter()
            .filter_map(|key| self.descriptors.get(key).cloned())
            .collect()
    }

    pub fn module_descriptor(&self, root: &Path) -> Option<Value> {
        self.descriptors.get(&Self::root_key(root)).cloned()
    }

    pub fn load_module_descriptor(
        &mut self,
        overlay: Value,
    ) -> Result<Value, BuildScriptRuntimeModelError> {
        let root = Self::descriptor_root(&self.model.project_root, &overlay)?;
        let key = Self::root_key(&root);
        let module = match self.module_models.get(&key) {
            Some(module) => module.clone(),
            None => {
                let module = BuildScriptModuleModelLoader::new(BuildScriptModuleModelRequest {
                    project_root: self.model.project_root.clone(),
                    descriptor: Self::descriptor_with_root(overlay.clone(), &root),
                })
                .load()?;
                self.module_models.insert(key.clone(), module.clone());
                module
            }
        };
        let descriptor = Self::prepared_descriptor(&module, &overlay, self.uses_ohpm());
        if !self.descriptors.contains_key(&key) {
            self.descriptor_order.push(key.clone());
        }
        self.descriptors.insert(key, descriptor.clone());
        self.context.register_module(
            root,
            descriptor
                .get("modulePackage")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
        );
        Ok(descriptor)
    }

    pub fn replace_module_descriptors(
        &mut self,
        overlays: Vec<Value>,
    ) -> Result<Vec<Value>, BuildScriptRuntimeModelError> {
        let mut descriptors = Vec::with_capacity(overlays.len());
        let mut descriptor_order = Vec::with_capacity(overlays.len());
        let mut plan_modules = Vec::with_capacity(overlays.len());
        for overlay in overlays {
            let descriptor = self.load_module_descriptor(overlay)?;
            let root = descriptor
                .get("root")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .ok_or_else(|| {
                    BuildScriptRuntimeModelError::InvalidModuleDescriptor(descriptor.clone())
                })?;
            let key = Self::root_key(&root);
            descriptor_order.push(key);
            plan_modules.push(Self::module_request(&descriptor));
            descriptors.push(descriptor);
        }
        self.descriptor_order = descriptor_order;
        self.request.modules = plan_modules;
        self.invalidate();
        Ok(descriptors)
    }

    pub fn upsert_module(&mut self, mut module: BuildScriptRuntimeModulePlanRequest) {
        module.root = lexical_normalize(&module.root);
        self.context
            .register_module(module.root.clone(), module.package.clone());
        let key = Self::root_key(&module.root);
        if let Some(override_value) = self.module_overrides.get(&key) {
            module.build_script_override = Some(override_value.clone());
        }
        if let Some(existing) = self
            .request
            .modules
            .iter_mut()
            .find(|candidate| Self::root_key(&candidate.root) == key)
        {
            *existing = module;
        } else {
            self.request.modules.push(module);
        }
        self.invalidate();
    }

    pub fn set_module_override(&mut self, root: &Path, value: Option<Value>) {
        let key = Self::root_key(root);
        if let Some(value) = &value {
            self.module_overrides.insert(key.clone(), value.clone());
            // Hvigor exposes the resolved build-script root as the override-map
            // key. Keep that lexical spelling for serialization while using a
            // canonical key only for internal module identity. On macOS these
            // differ for `/var` and `/private/var`.
            self.module_override_roots
                .insert(key.clone(), lexical_normalize(root));
        } else {
            self.module_overrides.remove(&key);
            self.module_override_roots.remove(&key);
        }
        if let Some(module) = self
            .request
            .modules
            .iter_mut()
            .find(|candidate| Self::root_key(&candidate.root) == key)
        {
            module.build_script_override = value;
        }
        self.invalidate();
    }

    pub fn set_project_profile(&mut self, profile: Value, product_name: String) {
        self.request.project_profile = profile;
        self.request.product_name = product_name;
        self.sdk_details = None;
        self.invalidate();
    }

    pub fn set_project_kind(&mut self, kind: Option<String>) {
        self.request.project_kind = kind;
        self.invalidate();
    }

    pub fn invalidate(&mut self) {
        self.plan = None;
    }

    pub fn plan(
        &mut self,
        update: BuildScriptRuntimePlanUpdate,
    ) -> Result<&BuildScriptRuntimePlan, BuildScriptRuntimePlanDiagnostic> {
        if self.plan.is_none() {
            self.request.product_name = update.product_name;
            self.request.build_mode_name = update.build_mode_name;
            self.request.module_selection = update.module_selection;
            self.request.properties = update.properties;
            self.request.ext_params = update.ext_params;
            self.request.start_params = update.start_params;
            self.request.sync_version_changed = update.sync_version_changed;
            self.plan = Some(BuildScriptRuntimePlanner::new(self.request.clone()).plan()?);
        }
        Ok(self
            .plan
            .as_ref()
            .expect("runtime plan must be initialized"))
    }

    pub fn module_plan(&self, root: &Path) -> Option<&BuildScriptRuntimeModulePlan> {
        let key = Self::root_key(root);
        self.plan
            .as_ref()?
            .modules
            .iter()
            .find(|module| Self::root_key(&module.root) == key)
    }

    pub fn unsupported_build_diagnostic(&self) -> Option<&BuildScriptRuntimePlanDiagnostic> {
        self.plan.as_ref()?.unsupported_build_diagnostic.as_ref()
    }

    pub fn sdk_details(
        &mut self,
        config: &BuildScriptRuntimeSdkConfig,
    ) -> Result<&BuildScriptRuntimeSdkDetails, HvigorSdkDetailsError> {
        if self.sdk_details.is_none() {
            let selected_product = self.request.selected_product_value();
            let runtime_os = selected_product
                .get("runtimeOS")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .or(config.runtime_os.as_deref());
            let is_ohos = runtime_os != Some("HarmonyOS");
            let requested_api = self.request.effective_compile_api_version();
            let inventory = HvigorSdkDetailsInventory::new(HvigorSdkDetailsRequest {
                project_root: self.model.project_root.clone(),
                sdk_path: config.sdk_path.clone(),
                ets_loader_path: config.ets_loader_path.clone(),
                is_ohos,
                requested_api,
            })
            .inspect()?;
            let metadata = inventory.metadata.as_ref();
            let sdk_version = self
                .request
                .explicitly_configured_compile_api_version()
                .or_else(|| metadata.and_then(|value| value.api_version))
                .unwrap_or(requested_api);
            let ets_component_version = config
                .ets_component_version
                .clone()
                .or_else(|| metadata.and_then(|value| value.version.as_deref().map(str::to_string)))
                .unwrap_or_default();
            let ets_component_release_type = config
                .ets_component_release_type
                .clone()
                .or_else(|| {
                    metadata.and_then(|value| value.release_type.as_deref().map(str::to_string))
                })
                .unwrap_or_else(|| "Release".to_string());
            self.sdk_details = Some(BuildScriptRuntimeSdkDetails {
                is_ohos,
                sdk_dir: inventory.sdk_dir,
                sdk_version,
                ets_component_version,
                ets_component_release_type,
            });
        }
        Ok(self
            .sdk_details
            .as_ref()
            .expect("SDK details must be initialized"))
    }

    fn module_request(descriptor: &Value) -> BuildScriptRuntimeModulePlanRequest {
        BuildScriptRuntimeModulePlanRequest {
            name: descriptor
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            root: descriptor
                .get("root")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .unwrap_or_default(),
            descriptor: descriptor.clone(),
            build_profile: descriptor
                .get("moduleProfile")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
            build_script_override: None,
            kind: descriptor
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_string),
            package: descriptor
                .get("modulePackage")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
        }
    }

    fn parameter_file_override(&self, ext_params: &Map<String, Value>) -> Option<String> {
        if !self.uses_ohpm() {
            return None;
        }
        let package = &self.model.project_package;
        let configured = ext_params
            .get("parameterFile")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .or_else(|| package.get("parameterFile").and_then(Value::as_str))?;
        Some(Self::replace_string_parameters(
            configured,
            package.get("properties"),
        ))
    }

    fn update_module_package(&mut self, root: &Path, package: &Value) {
        let key = Self::root_key(root);
        if let Some(module) = self
            .request
            .modules
            .iter_mut()
            .find(|module| Self::root_key(&module.root) == key)
        {
            module.package = package.clone();
        }
        if let Some(descriptor) = self.descriptors.get_mut(&key) {
            if let Some(descriptor) = descriptor.as_object_mut() {
                descriptor.insert("modulePackage".to_string(), package.clone());
            }
        }
        self.invalidate();
    }

    fn replace_string_parameters(value: &str, properties: Option<&Value>) -> String {
        let Some(properties) = properties else {
            return value.to_string();
        };
        let mut output = String::with_capacity(value.len());
        let mut remainder = value;
        while let Some(start) = remainder.find("${") {
            output.push_str(&remainder[..start]);
            let expression = &remainder[start + 2..];
            let Some(end) = expression.find('}') else {
                output.push_str(&remainder[start..]);
                remainder = "";
                break;
            };
            let key = expression[..end].trim();
            let replacement = key
                .split('.')
                .try_fold(properties, |current, component| match current {
                    Value::Object(entries) => entries.get(component),
                    Value::Array(entries) => component
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| entries.get(index)),
                    _ => None,
                })
                .and_then(Self::parameter_scalar);
            if let Some(replacement) = replacement {
                output.push_str(&replacement);
            } else {
                output.push_str(&remainder[start..start + end + 3]);
            }
            remainder = &expression[end + 1..];
        }
        output.push_str(remainder);
        output
    }

    fn parameter_scalar(value: &Value) -> Option<String> {
        match value {
            Value::Bool(value) => Some(value.to_string()),
            Value::Number(value) => Some(value.to_string()),
            Value::String(value) => Some(value.clone()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        }
    }

    fn resolve_hvigor_cache_dir(&self, configured: Option<&Value>) -> (PathBuf, Option<String>) {
        let fallback = self.model.project_root.join(".hvigor");
        let Some(configured) = configured else {
            return (fallback, None);
        };
        let Some(configured_path) = configured.as_str().map(PathBuf::from) else {
            let warning = Self::js_truthy(configured)
                .then(|| format!("Invalid custom hvigor data dir:{configured}"));
            return (fallback, warning);
        };
        if !configured_path.is_absolute() {
            let warning = (!configured_path.as_os_str().is_empty()).then(|| {
                format!(
                    "Invalid custom hvigor data dir:{}",
                    configured_path.to_string_lossy()
                )
            });
            return (fallback, warning);
        }
        let configured_path = canonicalize_or_owned(&configured_path);
        let project_name = self
            .model
            .project_root
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new(""));
        (
            lexical_normalize(&configured_path.join(project_name).join(".hvigor")),
            None,
        )
    }

    fn nullish_value(value: Option<&Value>) -> Option<&Value> {
        value.filter(|value| !value.is_null())
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

    fn prepared_descriptor(
        model: &BuildScriptModuleModelSnapshot,
        overlay: &Value,
        uses_ohpm: bool,
    ) -> Value {
        let mut descriptor = model.descriptor.as_object().cloned().unwrap_or_default();
        if let Some(overlay) = overlay.as_object() {
            descriptor.extend(overlay.clone());
        }
        descriptor.insert("name".to_string(), Value::String(model.name.clone()));
        descriptor.insert("srcPath".to_string(), Value::String(model.src_path.clone()));
        descriptor.insert(
            "root".to_string(),
            Value::String(model.root.to_string_lossy().into_owned()),
        );
        descriptor.insert("moduleProfile".to_string(), model.build_profile.clone());
        if let Some(module_json) = &model.module_json {
            descriptor.insert("moduleJson".to_string(), module_json.clone());
        }
        descriptor.insert(
            "modulePackage".to_string(),
            if uses_ohpm {
                model.package.clone()
            } else {
                model.npm_package.clone()
            },
        );
        Value::Object(descriptor)
    }

    fn descriptor_root(
        project_root: &Path,
        descriptor: &Value,
    ) -> Result<PathBuf, BuildScriptRuntimeModelError> {
        let root = descriptor
            .get("root")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                descriptor
                    .get("srcPath")
                    .or_else(|| descriptor.get("name"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(|value| project_root.join(value))
            })
            .ok_or_else(|| {
                BuildScriptRuntimeModelError::InvalidModuleDescriptor(descriptor.clone())
            })?;
        Ok(if root.is_absolute() {
            lexical_normalize(&root)
        } else {
            lexical_normalize(&project_root.join(root))
        })
    }

    fn descriptor_with_root(mut descriptor: Value, root: &Path) -> Value {
        if !descriptor.is_object() {
            descriptor = Value::Object(Map::new());
        }
        let object = descriptor
            .as_object_mut()
            .expect("descriptor was normalized to an object");
        object.insert(
            "root".to_string(),
            Value::String(root.to_string_lossy().into_owned()),
        );
        descriptor
    }

    fn root_key(root: &Path) -> PathBuf {
        canonicalize_or_owned(&lexical_normalize(root))
    }

    fn project_name(&self) -> String {
        self.model
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}
