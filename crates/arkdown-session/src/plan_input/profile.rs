//! Narrow plan input for the profile processing stage.

use std::path::PathBuf;

use crate::paths::ModulePathLayout;

// ---------------------------------------------------------------------------
// ProfilePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the profile processing stage.
///
/// Contains the inputs needed by `arkdown-profile::ProfileProcessor` without
/// exposing the full session context.
///
/// # Upstream correspondence
///
/// Corresponds to `ProcessProfile` task inputs assembled by `TaskCreator`
/// in hvigor.
#[derive(Debug, Clone)]
pub struct ProfilePlanInput {
    /// Absolute path to the module's `module.json5`.
    pub module_json5_path: PathBuf,
    /// Absolute path to the module root directory.
    pub module_root: PathBuf,
    /// The output directory for processed profile artifacts.
    pub output_dir: PathBuf,
    /// OHOS module name (e.g., `"entry"`, `"feature"`).
    pub module_name: String,
    /// Bundle name (from `AppScope/app.json5`).
    pub bundle_name: String,
    /// Whether this is a debug build.
    pub is_debug: bool,
}

impl ProfilePlanInput {
    /// Create from a module snapshot and artifact layout.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to assembling `ProcessProfile` task inputs from
    /// `TaskCreator` in hvigor.
    pub fn from_context_input(
        module_name: impl Into<String>,
        module_root: PathBuf,
        output_dir: PathBuf,
        bundle_name: impl Into<String>,
        is_debug: bool,
    ) -> Self {
        let module_json5_path = ModulePathLayout::new(&module_root).module_json5();
        Self {
            module_json5_path,
            module_root,
            output_dir,
            module_name: module_name.into(),
            bundle_name: bundle_name.into(),
            is_debug,
        }
    }

    /// Validate that the required fields are present.
    pub fn validate(&self) -> Result<(), String> {
        if self.module_name.is_empty() {
            return Err("module_name is required".to_string());
        }
        Ok(())
    }

    /// Return the `module.json5` input path.
    pub fn module_json_input(&self) -> &std::path::Path {
        &self.module_json5_path
    }

    /// Return the `loader.json` output path within the output directory.
    pub fn loader_input(&self) -> PathBuf {
        self.output_dir.join("loader.json")
    }
}
