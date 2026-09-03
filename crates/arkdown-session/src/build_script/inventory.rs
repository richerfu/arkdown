use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arkdown_config::ProjectHvigorOverrides;
use arkdown_core::ArkdownConfig;
use arkdown_path::AbsPath;

use crate::build_script::{BuildScriptParser, BuildScriptSnapshot, ParseBuildScriptError};

/// Static hvigorfile discovery/evaluation errors for the pure Rust CLI.
#[derive(Debug, thiserror::Error)]
pub enum BuildScriptOverrideError {
    #[error(transparent)]
    Parse(#[from] ParseBuildScriptError),
    #[error("invalid absolute build script path {path:?}: {message}")]
    InvalidPath { path: PathBuf, message: String },
    #[error("invalid project OHOS overrides in {path:?}: {source}")]
    InvalidProjectOverrides {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "module build script {path:?} contains unsupported project override '{field}'; module scripts only support config.ohos.overrides.buildOption"
    )]
    InvalidModuleScope { path: PathBuf, field: String },
}

/// Statically evaluated project/module hvigorfile overrides.
///
/// The Node package evaluates arbitrary JavaScript in `packages/arkdown/js/runtime.js`. The pure
/// Rust CLI accepts the literal-object subset and rejects dynamic values with a
/// typed diagnostic.
#[derive(Debug, Clone, Default)]
pub struct BuildScriptOverrideInventory {
    project: Option<ProjectHvigorOverrides>,
    project_source: Option<PathBuf>,
    module_build_options: BTreeMap<String, serde_json::Value>,
    snapshots: Vec<BuildScriptSnapshot>,
}

impl BuildScriptOverrideInventory {
    pub fn discover(config: &ArkdownConfig) -> Result<Self, BuildScriptOverrideError> {
        let mut inventory = Self::default();
        let mut project_value = serde_json::Value::Object(serde_json::Map::new());

        for file_name in ["hvigorfile.ts", "hvigorconfig.ts"] {
            let path = config.project_root.join(file_name);
            if !path.is_file() {
                continue;
            }
            let path = inventory.absolute_path(path)?;
            let snapshot = BuildScriptParser::parse_project(&path)?;
            if let Some(overrides) = snapshot.ohos_overrides() {
                inventory.merge_value(&mut project_value, overrides);
                inventory.project_source = Some(path.as_path().to_path_buf());
            }
            inventory.snapshots.push(snapshot);
        }

        if inventory.project_source.is_some() {
            inventory.project = Some(serde_json::from_value(project_value).map_err(|source| {
                BuildScriptOverrideError::InvalidProjectOverrides {
                    path: inventory.project_source.clone().unwrap_or_default(),
                    source,
                }
            })?);
        }

        for module in &config.modules {
            let mut module_value = None;
            for file_name in ["hvigorfile.ts", "hvigorconfig.ts"] {
                let path = module.src_path.join(file_name);
                if !path.is_file() {
                    continue;
                }
                let path = inventory.absolute_path(path)?;
                let snapshot = BuildScriptParser::parse_module(&path, &module.name)?;
                if let Some(overrides) = snapshot.ohos_overrides() {
                    let Some(object) = overrides.as_object() else {
                        return Err(BuildScriptOverrideError::InvalidModuleScope {
                            path: path.as_path().to_path_buf(),
                            field: "overrides".to_string(),
                        });
                    };
                    if let Some(field) = object.keys().find(|field| field.as_str() != "buildOption")
                    {
                        return Err(BuildScriptOverrideError::InvalidModuleScope {
                            path: path.as_path().to_path_buf(),
                            field: field.clone(),
                        });
                    }
                    if let Some(build_option) = object.get("buildOption") {
                        if let Some(value) = module_value.as_mut() {
                            inventory.merge_value(value, build_option);
                        } else {
                            module_value = Some(build_option.clone());
                        }
                    }
                }
                inventory.snapshots.push(snapshot);
            }
            if let Some(module_value) = module_value {
                inventory
                    .module_build_options
                    .insert(module.name.clone(), module_value);
            }
        }

        Ok(inventory)
    }

    pub fn project_overrides(&self) -> Option<&ProjectHvigorOverrides> {
        self.project.as_ref()
    }

    pub fn project_source(&self) -> Option<&Path> {
        self.project_source.as_deref()
    }

    pub fn module_build_options(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.module_build_options
    }

    pub fn snapshots(&self) -> &[BuildScriptSnapshot] {
        &self.snapshots
    }

    fn absolute_path(&self, path: PathBuf) -> Result<AbsPath, BuildScriptOverrideError> {
        AbsPath::from_absolute(path.clone()).map_err(|error| {
            BuildScriptOverrideError::InvalidPath {
                path,
                message: error.to_string(),
            }
        })
    }

    fn merge_value(&self, target: &mut serde_json::Value, overlay: &serde_json::Value) {
        if let (Some(target), Some(overlay)) = (target.as_object_mut(), overlay.as_object()) {
            for (key, value) in overlay {
                if let Some(target_value) = target.get_mut(key) {
                    self.merge_value(target_value, value);
                } else {
                    target.insert(key.clone(), value.clone());
                }
            }
        } else {
            target.clone_from(overlay);
        }
    }
}

#[cfg(test)]
mod tests {
    use arkdown_core::{
        AppOptionOverride, BuildMode, BuildOption, ModuleConfig, NativeLibOption, OhosModuleType,
        OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };

    use super::*;

    #[test]
    fn discovers_project_and_module_literal_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let entry = temp.path().join("entry");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(
            temp.path().join("hvigorfile.ts"),
            r#"
                export default {
                    config: {
                        ohos: {
                            overrides: {
                                appOpt: { vendor: "script", versionCode: 200 }
                            }
                        }
                    }
                };
            "#,
        )
        .unwrap();
        std::fs::write(
            entry.join("hvigorfile.ts"),
            r#"
                export default {
                    config: {
                        ohos: {
                            overrides: {
                                buildOption: {
                                    arkOptions: { hostPGO: true },
                                    externalNativeOptions: {
                                        abiFilters: ["arm64-v8a"]
                                    }
                                }
                            }
                        }
                    }
                };
            "#,
        )
        .unwrap();
        let config = ArkdownConfig {
            project_root: temp.path().to_path_buf(),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                artifact_name: None,
                bundle_name: "com.example".to_string(),
                compatible_sdk_version: "12".to_string(),
                compatible_sdk_version_stage: None,
                target_sdk_version: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                        no_external_import_by_path: None,
                        duplicate_dependency_check: false,
                        api_compatibility_check: None,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: Vec::new(),
                    deduplicate_har: false,
                    enable_source_code_check: true,
                    app_with_signed_pkg: false,
                    generate_shared_tgz: None,
                    customized_har_base_package: None,
                    remove_permissions: Vec::new(),
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: Vec::new(),
                    external_native_options: None,
                    har_artifact_type: Default::default(),
                    native_lib: NativeLibOption::default(),
                },
                version_code: None,
                version_name: None,
                build_version: None,
                app_options: AppOptionOverride::default(),
            },
            modules: vec![ModuleConfig {
                name: "entry".to_string(),
                src_path: entry,
                module_type: OhosModuleType::Entry,
            }],
            sdk_config: SdkConfig {
                ets_loader_path: temp.path().join("sdk/ets-loader"),
                es2abc_path: temp.path().join("sdk/es2abc"),
                restool_path: temp.path().join("sdk/restool"),
                compile_api_version: 12,
                sdk_component_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        };

        let inventory = BuildScriptOverrideInventory::discover(&config).unwrap();
        assert_eq!(
            inventory
                .project_overrides()
                .unwrap()
                .app_opt
                .as_ref()
                .unwrap()
                .version_code,
            Some(200)
        );
        assert_eq!(
            inventory.module_build_options()["entry"]["arkOptions"]["hostPGO"],
            true
        );
        assert_eq!(inventory.snapshots().len(), 2);
    }
}
