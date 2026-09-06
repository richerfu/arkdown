//! [`SdkSnapshot`] — selected SDK, tool paths, and API version metadata.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{is_harmony_os, ArktsLanguageVersion, SdkConfig};
use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sdk_compile_plugin::{SdkCompilePluginSnapshot, SdkSupportExtensionSet};
use crate::sdk_schemas::{SdkSchemaKind, SdkSchemaPath, SdkSchemaResolver};
use crate::sdk_tools::{SdkToolKind, SdkToolPath, SdkToolResolver};

use super::SdkEs2abcOptionSnapshot;
use super::{SdkSystemCapabilityError, SdkSystemCapabilityInventory};

/// Errors when building the SDK snapshot.
#[derive(Debug, Error)]
pub enum SdkSnapshotError {
    #[error("normalized SDK config has no SDK component root for ets-loader path: {0}")]
    InvalidEtsLoaderPath(PathBuf),

    #[error("SDK component root not found: {0}")]
    SdkRootNotFound(PathBuf),

    #[error("SDK system-capability inventory error: {0}")]
    SystemCapability(#[from] SdkSystemCapabilityError),

    #[error("read SDK component configuration {path}: {source}")]
    ReadComponentConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("parse SDK component configuration {path}: {source}")]
    ParseComponentConfig {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("invalid SDK component configuration {path}: {message}")]
    InvalidComponentConfig { path: PathBuf, message: String },
}

/// One `depsModuleConfig` rule owned by an ArkUI-X SDK component.
///
/// The source compiler keys the outer configuration by declaration filename
/// and the inner rule by the `#`-joined declaration path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SdkCrossplatformDependency {
    pub api_module: String,
    pub function: String,
    pub modules: Vec<String>,
    pub components: Vec<String>,
}

/// SDK metadata loaded once at session startup.
#[derive(Debug, Clone)]
pub struct SdkSnapshot {
    /// Root SDK directory.
    pub sdk_root: PathBuf,
    /// Project target API version passed to compilers and SDK tools.
    pub api_version: u32,
    /// API version of the selected SDK component.
    pub component_api_version: u32,
    /// Compile SDK version string (e.g., `"5.0.0.47"`).
    pub compile_sdk_version: String,
    /// SDK compile-plugin metadata resolved during session startup.
    pub compile_plugin: SdkCompilePluginSnapshot,
    /// Compiler declaration roots matching ets-loader `globalModulePaths`.
    pub global_module_paths: Arc<[PathBuf]>,
    /// External ETS component roots matching ets-loader `externalApiPaths`.
    pub external_api_paths: Arc<[PathBuf]>,
    /// Module prefixes accepted by the SDK system-API transform. Prefixes do
    /// not include the leading `@` (for example `ohos` and `arkui-x`).
    pub module_prefixes: Arc<[String]>,
    /// ArkUI-X API dependency rules loaded from `depsModuleConfig` once at
    /// session startup.
    pub crossplatform_dependencies: Arc<[SdkCrossplatformDependency]>,
    /// Actual `es2abc --help` option inventory, probed once at startup.
    pub es2abc_options: Arc<SdkEs2abcOptionSnapshot>,
    /// Device system-capability declarations parsed once at session startup.
    pub system_capabilities: Arc<SdkSystemCapabilityInventory>,
}

impl SdkSnapshot {
    /// Source roots and files consumed by ArkGuard's system-API whitelist pass.
    ///
    /// Upstream correspondence:
    /// `ConfigResolver.collectSystemApiWhitelist` scans every `sdkApis` entry,
    /// the sibling `component` directory, and the ETS loader's
    /// `lib/pre_define.js` plus `obfuscateWhiteList.json5`. Returning the
    /// paths from the SDK snapshot lets [`crate::SourceStore`] capture their
    /// contents once during session startup.
    pub fn arkguard_source_roots(&self) -> Vec<PathBuf> {
        let mut roots = std::collections::BTreeSet::new();
        for api_root in self.global_module_paths.iter() {
            roots.insert(api_root.clone());
            let Some(ets_root) = api_root.parent() else {
                continue;
            };
            roots.insert(ets_root.join("component"));
            let ets_loader = ets_root.join("build-tools").join("ets-loader");
            roots.insert(ets_loader.join("lib").join("pre_define.js"));
            roots.insert(ets_loader.join("obfuscateWhiteList.json5"));
        }
        roots.into_iter().collect()
    }

    /// Build from the normalized SDK config produced during config loading.
    ///
    /// `arkdown-config` performs SDK discovery and normalizes tool paths once.
    /// The session derives the component root from that config instead of
    /// probing environment variables a second time.
    pub fn from_config(config: &SdkConfig) -> Result<Self, SdkSnapshotError> {
        Self::from_config_for_runtime(config, "OpenHarmony")
    }

    /// Build from normalized SDK config for the selected runtime OS.
    ///
    /// HarmonyOS compilation discovers sibling ETS components once here. The
    /// resulting immutable paths are reused by component metadata, system
    /// resources, semantic checking, and ArkGuard instead of re-scanning the
    /// SDK tree or relying on process-global `externalApiPaths` state.
    pub fn from_config_for_runtime(
        config: &SdkConfig,
        runtime_os: &str,
    ) -> Result<Self, SdkSnapshotError> {
        Self::from_config_for_runtime_with_arkts(
            config,
            runtime_os,
            ArktsLanguageVersion::default(),
        )
    }

    /// Build from normalized SDK config for one ArkTS language generation.
    ///
    /// The split SDK keeps ArkTS 1.1 declarations under `ets/dynamic` and
    /// ArkTS 1.2 declarations under `ets/static`. Selecting these roots while
    /// the immutable session snapshot is built prevents later checking and
    /// transform stages from observing declarations for the wrong language.
    pub fn from_config_for_runtime_with_arkts(
        config: &SdkConfig,
        runtime_os: &str,
        arkts_language_version: ArktsLanguageVersion,
    ) -> Result<Self, SdkSnapshotError> {
        let sdk_root = config
            .component_root()
            .map(lexical_normalize)
            .ok_or_else(|| {
                SdkSnapshotError::InvalidEtsLoaderPath(config.ets_loader_path.clone())
            })?;

        if !sdk_root.is_dir() {
            return Err(SdkSnapshotError::SdkRootNotFound(sdk_root));
        }

        let mut snapshot = Self::from_parts(
            sdk_root,
            config.compile_api_version,
            config.sdk_component_api_version,
            config.compile_sdk_version.clone(),
        );
        let component_inventory = SdkComponentInventory::load(
            config,
            runtime_os,
            &snapshot.sdk_root,
            arkts_language_version,
        )?;
        snapshot.external_api_paths = component_inventory.external_api_paths.into();
        snapshot.global_module_paths = component_inventory.global_module_paths.clone().into();
        snapshot.module_prefixes = component_inventory.module_prefixes.into();
        snapshot.crossplatform_dependencies = component_inventory.crossplatform_dependencies.into();
        snapshot.es2abc_options = Arc::new(SdkEs2abcOptionSnapshot::inspect(&config.es2abc_path));
        let mut system_capability_roots = config.ets_api_dirs_for(arkts_language_version);
        system_capability_roots.extend(component_inventory.global_module_paths);
        system_capability_roots.sort();
        system_capability_roots.dedup();
        snapshot.system_capabilities = Arc::new(SdkSystemCapabilityInventory::from_api_roots(
            &system_capability_roots,
        )?);
        Ok(snapshot)
    }

    /// Build from already resolved SDK parts.
    pub fn from_parts(
        sdk_root: PathBuf,
        api_version: u32,
        component_api_version: u32,
        compile_sdk_version: String,
    ) -> Self {
        let compile_plugin = SdkCompilePluginSnapshot::from_sdk_parts(
            &sdk_root,
            component_api_version,
            &compile_sdk_version,
        );

        Self {
            sdk_root,
            api_version,
            component_api_version,
            compile_sdk_version,
            compile_plugin,
            global_module_paths: Arc::default(),
            external_api_paths: Arc::default(),
            module_prefixes: ["ohos", "system", "kit", "arkts"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
                .into(),
            crossplatform_dependencies: Arc::default(),
            es2abc_options: Arc::new(SdkEs2abcOptionSnapshot::default()),
            system_capabilities: Arc::new(SdkSystemCapabilityInventory::default()),
        }
    }

    /// Return the SDK tool resolver.
    pub fn tools(&self) -> SdkToolResolver {
        SdkToolResolver::new(self)
    }

    /// Return a single SDK tool path by kind.
    pub fn tool_path(&self, kind: SdkToolKind) -> Option<SdkToolPath> {
        self.tools().tool_path(kind)
    }

    /// Return the SDK schema resolver.
    pub fn schemas(&self) -> SdkSchemaResolver {
        SdkSchemaResolver::new(self)
    }

    /// Return a single SDK schema path by kind.
    pub fn schema_path(&self, kind: SdkSchemaKind) -> Option<SdkSchemaPath> {
        self.schemas().schema_path(kind)
    }

    /// Return the SDK compile plugin snapshot.
    pub fn compile_plugin(&self) -> &SdkCompilePluginSnapshot {
        &self.compile_plugin
    }

    /// Return the file extensions supported by the SDK compile pipeline.
    pub fn support_extensions(&self) -> SdkSupportExtensionSet {
        self.compile_plugin.support_extensions().clone()
    }
}

#[derive(Debug)]
struct SdkComponentInventory {
    global_module_paths: Vec<PathBuf>,
    external_api_paths: Vec<PathBuf>,
    module_prefixes: Vec<String>,
    crossplatform_dependencies: Vec<SdkCrossplatformDependency>,
}

impl SdkComponentInventory {
    fn load(
        config: &SdkConfig,
        runtime_os: &str,
        primary_component_root: &Path,
        arkts_language_version: ArktsLanguageVersion,
    ) -> Result<Self, SdkSnapshotError> {
        let primary_ets_root = config
            .ets_loader_path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                SdkSnapshotError::InvalidEtsLoaderPath(config.ets_loader_path.clone())
            })?;
        let mut global_module_paths = config.ets_api_dirs_for(arkts_language_version);
        global_module_paths.extend(config.ets_arkts_dirs_for(arkts_language_version));
        global_module_paths.extend(config.ets_kits_dirs_for(arkts_language_version));
        global_module_paths.retain(|path| path.is_dir());
        let mut inventory = Self {
            global_module_paths,
            external_api_paths: Vec::new(),
            module_prefixes: ["ohos", "system", "kit", "arkts"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            crossplatform_dependencies: Vec::new(),
        };
        if config.compile_api_version <= 9 || !is_harmony_os(runtime_os) {
            return Ok(inventory);
        }
        let Some(component_parent) = primary_component_root.parent() else {
            return Ok(inventory);
        };
        let entries = std::fs::read_dir(component_parent).map_err(|source| {
            SdkSnapshotError::ReadComponentConfig {
                path: component_parent.to_path_buf(),
                source,
            }
        })?;
        let mut external_roots = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("ets"))
            .filter(|root| root != primary_ets_root && root.join("sdkConfig.json").is_file())
            .collect::<Vec<_>>();
        external_roots.sort();

        for root in external_roots {
            let config_path = root.join("sdkConfig.json");
            let content = std::fs::read(&config_path).map_err(|source| {
                SdkSnapshotError::ReadComponentConfig {
                    path: config_path.clone(),
                    source,
                }
            })?;
            let component: SdkExternalComponentConfig =
                serde_json::from_slice(&content).map_err(|source| {
                    SdkSnapshotError::ParseComponentConfig {
                        path: config_path.clone(),
                        source,
                    }
                })?;
            let Some(api_path) = component.api_path else {
                continue;
            };
            let api_paths = api_path.into_paths();
            if api_paths.is_empty() {
                continue;
            }
            inventory.external_api_paths.push(root.clone());
            inventory.global_module_paths.extend(
                api_paths
                    .into_iter()
                    .map(|path| lexical_normalize(&root.join(path)))
                    .filter(|path| path.is_dir()),
            );
            let prefix =
                component
                    .prefix
                    .ok_or_else(|| SdkSnapshotError::InvalidComponentConfig {
                        path: config_path.clone(),
                        message: "apiPath requires prefix".to_string(),
                    })?;
            let normalized_prefix = prefix.trim_start_matches('@').to_string();
            if !normalized_prefix.is_empty()
                && !inventory.module_prefixes.contains(&normalized_prefix)
            {
                inventory.module_prefixes.push(normalized_prefix);
            }
            if prefix == "@arkui-x" {
                if let Some(relative) = component.deps_module_config {
                    if let Some(dependencies) =
                        Self::load_crossplatform_dependencies(&root.join(relative))
                    {
                        inventory.crossplatform_dependencies = dependencies;
                    }
                }
            }
        }
        let mut seen_global_module_paths = std::collections::HashSet::new();
        inventory
            .global_module_paths
            .retain(|path| seen_global_module_paths.insert(path.clone()));
        inventory.external_api_paths.sort();
        inventory.external_api_paths.dedup();
        inventory.crossplatform_dependencies.sort_by(|left, right| {
            left.api_module
                .cmp(&right.api_module)
                .then_with(|| left.function.cmp(&right.function))
        });
        Ok(inventory)
    }

    fn load_crossplatform_dependencies(path: &Path) -> Option<Vec<SdkCrossplatformDependency>> {
        let Ok(content) = std::fs::read(path) else {
            return None;
        };
        let Ok(config) = serde_json::from_slice::<Vec<SdkCrossplatformModuleConfig>>(&content)
        else {
            tracing::warn!(path = %path.display(), "ignoring malformed ArkUI-X depsModuleConfig");
            return Some(Vec::new());
        };
        let mut dependencies = Vec::<SdkCrossplatformDependency>::new();
        for module in config {
            let api_module = format!("@{}", module.module);
            dependencies.retain(|dependency| dependency.api_module != api_module);
            for dependency in module.dependencies {
                dependencies.push(SdkCrossplatformDependency {
                    api_module: api_module.clone(),
                    function: dependency.function,
                    modules: dependency.modules,
                    components: dependency.components,
                });
            }
        }
        Some(dependencies)
    }
}

#[derive(Debug, Deserialize)]
struct SdkExternalComponentConfig {
    #[serde(rename = "apiPath")]
    api_path: Option<SdkApiPath>,
    prefix: Option<String>,
    #[serde(rename = "depsModuleConfig")]
    deps_module_config: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SdkApiPath {
    One(PathBuf),
    Many(Vec<PathBuf>),
}

impl SdkApiPath {
    fn into_paths(self) -> Vec<PathBuf> {
        match self {
            Self::One(path) => vec![path],
            Self::Many(paths) => paths,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SdkCrossplatformModuleConfig {
    module: String,
    #[serde(rename = "deps")]
    dependencies: Vec<SdkCrossplatformDependencyConfig>,
}

#[derive(Debug, Deserialize)]
struct SdkCrossplatformDependencyConfig {
    function: String,
    #[serde(rename = "module", default)]
    modules: Vec<String>,
    #[serde(rename = "component", default)]
    components: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use arkdown_core::{ArktsLanguageVersion, SdkConfig};

    use super::SdkSnapshot;

    #[test]
    fn split_arkts_sdk_snapshot_keeps_component_root_and_dynamic_declarations() {
        let temp = tempfile::tempdir().expect("temporary split SDK");
        let component_root = temp.path().join("default/openharmony");
        let dynamic_ets = component_root.join("ets/dynamic");
        let static_ets = component_root.join("ets/static");
        let loader = dynamic_ets.join("build-tools/ets-loader");
        for path in [
            loader.clone(),
            dynamic_ets.join("api"),
            dynamic_ets.join("arkts"),
            dynamic_ets.join("kits"),
            static_ets.join("api"),
            static_ets.join("arkts"),
            static_ets.join("kits"),
        ] {
            fs::create_dir_all(path).expect("split SDK directory");
        }
        let config = SdkConfig {
            ets_loader_path: loader,
            es2abc_path: PathBuf::new(),
            restool_path: PathBuf::new(),
            compile_api_version: 26,
            sdk_component_api_version: 26,
            has_rollup_plugin: true,
            compile_sdk_version: "7.0.0.1".to_string(),
        };

        let snapshot = SdkSnapshot::from_config(&config).expect("split SDK snapshot");

        assert_eq!(snapshot.sdk_root, component_root);
        assert_eq!(
            snapshot.global_module_paths.as_ref(),
            &[
                dynamic_ets.join("api"),
                dynamic_ets.join("arkts"),
                dynamic_ets.join("kits"),
            ]
        );

        let static_snapshot = SdkSnapshot::from_config_for_runtime_with_arkts(
            &config,
            "OpenHarmony",
            ArktsLanguageVersion::V1_2,
        )
        .expect("static split SDK snapshot");
        assert_eq!(
            static_snapshot.global_module_paths.as_ref(),
            &[
                static_ets.join("api"),
                static_ets.join("arkts"),
                static_ets.join("kits"),
            ]
        );
    }

    #[test]
    fn harmonyos_snapshot_freezes_external_sdk_paths_and_system_capabilities() {
        let temp = tempfile::tempdir().expect("temporary SDK");
        let default_root = temp.path().join("default");
        let primary_ets = default_root.join("openharmony/ets");
        let external_ets = default_root.join("vendor/ets");
        let loader = primary_ets.join("build-tools/ets-loader");
        for path in [
            primary_ets.join("api/device-define"),
            primary_ets.join("arkts"),
            primary_ets.join("kits"),
            external_ets.join("api/device-define"),
            external_ets.join("kits"),
            loader.clone(),
        ] {
            fs::create_dir_all(path).expect("SDK directory");
        }
        fs::write(
            external_ets.join("sdkConfig.json"),
            r#"{"apiPath":["./api","./kits"],"prefix":"@arkui-x","depsModuleConfig":"crossplatform-deps.json"}"#,
        )
        .expect("external SDK metadata");
        fs::write(
            external_ets.join("crossplatform-deps.json"),
            r#"[
              {
                "module": "arkui-x.telephony",
                "deps": [
                  {
                    "function": "CallManager#dial",
                    "module": ["ohos.telephony.call"],
                    "component": ["Button", "Text"]
                  }
                ]
              }
            ]"#,
        )
        .expect("crossplatform dependency metadata");
        fs::write(
            primary_ets.join("api/device-define/default.json"),
            r#"{"SysCaps":["SystemCapability.Primary"]}"#,
        )
        .expect("primary system capability");
        fs::write(
            external_ets.join("api/device-define/phone.json"),
            r#"{"SysCaps":["SystemCapability.External"]}"#,
        )
        .expect("external system capability");
        fs::write(
            external_ets.join("api/device-define/syscap-version.json"),
            r#"{"versions":{"1":"HarmonyOS"}}"#,
        )
        .expect("external system capability metadata");
        fs::write(
            external_ets.join("api/device-define/api-white-list.json"),
            "[]",
        )
        .expect("external API white-list metadata");

        let config = SdkConfig {
            ets_loader_path: loader,
            es2abc_path: PathBuf::new(),
            restool_path: PathBuf::new(),
            compile_api_version: 17,
            sdk_component_api_version: 24,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.1.112".to_string(),
        };
        let snapshot = SdkSnapshot::from_config_for_runtime(&config, "HarmonyOS")
            .expect("HarmonyOS SDK snapshot");

        assert_eq!(
            snapshot.external_api_paths.as_ref(),
            std::slice::from_ref(&external_ets)
        );
        assert!(snapshot
            .global_module_paths
            .contains(&external_ets.join("api")));
        assert!(snapshot
            .global_module_paths
            .contains(&external_ets.join("kits")));
        assert!(snapshot.module_prefixes.contains(&"arkui-x".to_string()));
        assert_eq!(
            snapshot.crossplatform_dependencies.as_ref(),
            &[super::SdkCrossplatformDependency {
                api_module: "@arkui-x.telephony".to_string(),
                function: "CallManager#dial".to_string(),
                modules: vec!["ohos.telephony.call".to_string()],
                components: vec!["Button".to_string(), "Text".to_string()],
            }]
        );
        let capabilities = snapshot
            .system_capabilities
            .select(&["phone".to_string()])
            .expect("phone capabilities");
        assert!(capabilities
            .intersection
            .contains("SystemCapability.Primary"));
        assert!(capabilities
            .intersection
            .contains("SystemCapability.External"));
    }
}
