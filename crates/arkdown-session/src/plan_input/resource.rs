//! Narrow plan input for the resource compilation stage.

use std::path::PathBuf;

use crate::paths::ModulePathLayout;

// ---------------------------------------------------------------------------
// ResourcePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the resource compilation stage.
///
/// Contains the inputs needed by the SDK restool backend.
///
/// # Upstream correspondence
///
/// Corresponds to `CompileResource` task inputs in hvigor.
#[derive(Debug, Clone)]
pub struct ResourcePlanInput {
    /// Absolute path to the module's resource root directory.
    pub resource_root: PathBuf,
    /// Absolute path for the compiled `resources.index` output.
    pub index_output: PathBuf,
    /// Target device types (e.g., `["phone", "tablet"]`).
    pub device_types: Vec<String>,
    /// OHOS module name.
    pub module_name: String,
    /// Bundle name.
    pub bundle_name: String,
    /// Compatible SDK version.
    pub compatible_sdk_version: String,
    /// Whether this is a debug build.
    pub is_debug: bool,
}

impl ResourcePlanInput {
    /// Create from pre-resolved values.
    pub fn from_context_input(
        module_name: impl Into<String>,
        module_root: PathBuf,
        index_output: PathBuf,
        bundle_name: impl Into<String>,
        compatible_sdk_version: impl Into<String>,
        is_debug: bool,
    ) -> Self {
        let resource_root = ModulePathLayout::new(&module_root).resources_root();
        Self {
            resource_root,
            index_output,
            device_types: vec!["phone".to_string()],
            module_name: module_name.into(),
            bundle_name: bundle_name.into(),
            compatible_sdk_version: compatible_sdk_version.into(),
            is_debug,
        }
    }
}
