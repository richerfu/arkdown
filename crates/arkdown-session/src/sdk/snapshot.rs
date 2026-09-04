//! [`SdkSnapshot`] — selected SDK, tool paths, and API version metadata.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::SdkConfig;
use arkdown_path::lexical_normalize;
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
    /// Actual `es2abc --help` option inventory, probed once at startup.
    pub es2abc_options: Arc<SdkEs2abcOptionSnapshot>,
    /// Device system-capability declarations parsed once at session startup.
    pub system_capabilities: Arc<SdkSystemCapabilityInventory>,
}

impl SdkSnapshot {
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
        let sdk_root = config
            .ets_loader_path
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.parent())
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
        let global_module_paths = config.arkguard_api_dirs(runtime_os);
        snapshot.external_api_paths = Self::external_api_paths(
            config.ets_loader_path.parent().and_then(Path::parent),
            &global_module_paths,
        )
        .into();
        snapshot.global_module_paths = global_module_paths.clone().into();
        snapshot.es2abc_options = Arc::new(SdkEs2abcOptionSnapshot::inspect(&config.es2abc_path));
        let mut system_capability_roots = config.ets_api_dirs();
        system_capability_roots.extend(global_module_paths);
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
            es2abc_options: Arc::new(SdkEs2abcOptionSnapshot::default()),
            system_capabilities: Arc::new(SdkSystemCapabilityInventory::default()),
        }
    }

    fn external_api_paths(
        primary_ets_root: Option<&Path>,
        global_module_paths: &[PathBuf],
    ) -> Vec<PathBuf> {
        let mut roots = global_module_paths
            .iter()
            .filter_map(|declaration_root| {
                declaration_root
                    .ancestors()
                    .find(|ancestor| ancestor.join("sdkConfig.json").is_file())
                    .map(Path::to_path_buf)
            })
            .filter(|root| Some(root.as_path()) != primary_ets_root)
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        roots
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use arkdown_core::SdkConfig;

    use super::SdkSnapshot;

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
            r#"{"apiPath":["./api","./kits"],"prefix":"@vendor"}"#,
        )
        .expect("external SDK metadata");
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
            &[external_ets.clone()]
        );
        assert!(snapshot
            .global_module_paths
            .contains(&external_ets.join("api")));
        assert!(snapshot
            .global_module_paths
            .contains(&external_ets.join("kits")));
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
