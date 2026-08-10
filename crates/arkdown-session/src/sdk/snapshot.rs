//! [`SdkSnapshot`] — selected SDK, tool paths, and API version metadata.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::SdkConfig;
use arkdown_path::lexical_normalize;
use thiserror::Error;

use crate::sdk_compile_plugin::{SdkCompilePluginSnapshot, SdkSupportExtensionSet};
use crate::sdk_schemas::{SdkSchemaKind, SdkSchemaPath, SdkSchemaResolver};
use crate::sdk_tools::{SdkToolKind, SdkToolPath, SdkToolResolver};

use super::SdkEs2abcOptionSnapshot;

/// Errors when building the SDK snapshot.
#[derive(Debug, Error)]
pub enum SdkSnapshotError {
    #[error("normalized SDK config has no SDK component root for ets-loader path: {0}")]
    InvalidEtsLoaderPath(PathBuf),

    #[error("SDK component root not found: {0}")]
    SdkRootNotFound(PathBuf),
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
    /// Actual `es2abc --help` option inventory, probed once at startup.
    pub es2abc_options: Arc<SdkEs2abcOptionSnapshot>,
}

impl SdkSnapshot {
    /// Build from the normalized SDK config produced during config loading.
    ///
    /// `arkdown-config` performs SDK discovery and normalizes tool paths once.
    /// The session derives the component root from that config instead of
    /// probing environment variables a second time.
    pub fn from_config(config: &SdkConfig) -> Result<Self, SdkSnapshotError> {
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
        snapshot.es2abc_options = Arc::new(SdkEs2abcOptionSnapshot::inspect(&config.es2abc_path));
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
            es2abc_options: Arc::new(SdkEs2abcOptionSnapshot::default()),
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
