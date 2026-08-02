use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;

use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};

use crate::build_profile::{
    pick_effective_raw_target, BuildOptionProfileInventory, BuildOptionProfileModuleValue,
    EffectiveModuleBuildOptionResolver, RawTarget,
};

const COMPILE_PLUGIN_PREFIX: &str = "./build/hvigor_compile/";

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeModulePlan {
    pub name: String,
    pub root: PathBuf,
    pub selected_targets: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_target: Option<serde_json::Value>,
    pub build_options: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_path: Option<PathBuf>,
}

impl BuildScriptRuntimePlanDiagnostic {
    fn model(module_name: &str, message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_HVIGOR_BUILD_OPTION_PLAN".to_string(),
            message: message.into(),
            module_name: Some(module_name.to_string()),
            target_name: None,
            plugin_path: None,
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
            module_name: Some(module_name.to_string()),
            target_name: None,
            plugin_path: None,
        }
    }

    fn unknown_target(module_name: &str, target_name: &str) -> Self {
        Self {
            code: "UNKNOWN_MODULE_TARGET_IN_COMMAND_LINE".to_string(),
            message: format!(
                "target '{target_name}' was not found for module '{module_name}' in build-profile.json5"
            ),
            module_name: Some(module_name.to_string()),
            target_name: Some(target_name.to_string()),
            plugin_path: None,
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
            module_name: Some(module_name.to_string()),
            target_name: None,
            plugin_path,
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
        for module in &self.request.modules {
            let targets = BuildScriptTargetSet::from_module(module)?;
            let selected_targets = targets.select(
                &module.name,
                &module_selections,
                &self.request.product_name,
                &self.request.build_mode_name,
                self.request.build_target_name.as_deref(),
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
        Ok(BuildScriptRuntimePlan {
            module_selections,
            modules,
            compile_plugin_records,
        })
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
        let values = descriptor_targets
            .filter(|targets| !targets.is_empty())
            .or_else(|| {
                module
                    .build_profile
                    .get("targets")
                    .and_then(serde_json::Value::as_array)
            })
            .cloned()
            .unwrap_or_default();
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
    ) -> Result<Vec<serde_json::Value>, BuildScriptRuntimePlanDiagnostic> {
        let selected_entries = Self::selected_module_entries(module_name, selections);
        if !selections.is_empty() && selected_entries.is_empty() {
            return Ok(Vec::new());
        }
        let applicable = self.applicable_indices(product_name, build_mode_name);
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
        if self.values.is_empty() {
            return Ok(vec![serde_json::json!({ "name": "default" })]);
        }
        Ok(applicable
            .into_iter()
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
