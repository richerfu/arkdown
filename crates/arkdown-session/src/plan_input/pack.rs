//! Narrow plan input for the HAP/HSP/HAR packaging stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// PackagePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the HAP/HSP/HAR packaging stage.
///
/// Contains the inputs for `arkdown-pack::PackagePlanBuilder`.
///
/// # Upstream correspondence
///
/// Corresponds to `PackHAP`/`PackHSP`/`PackHAR` task inputs in hvigor.
#[derive(Debug, Clone)]
pub struct PackagePlanInput {
    /// Absolute path to the compiled `resources.index`.
    pub resources_index: PathBuf,
    /// Absolute path to the compiled `ets/modules.abc`.
    pub abc_path: PathBuf,
    /// Absolute path to the processed `module.json`.
    pub module_json: PathBuf,
    /// Absolute path to the `pack.info` file.
    pub pack_info: PathBuf,
    /// Output path for the unsigned package.
    pub unsigned_output: PathBuf,
    /// OHOS module type (`"hap"`, `"hsp"`, `"har"`).
    pub artifact_type: String,
    /// Whether this is a debug package.
    pub is_debug: bool,
}

impl PackagePlanInput {
    /// Create from pre-resolved paths.
    pub fn from_context_input(
        resources_index: PathBuf,
        abc_path: PathBuf,
        module_json: PathBuf,
        pack_info: PathBuf,
        unsigned_output: PathBuf,
        artifact_type: impl Into<String>,
        is_debug: bool,
    ) -> Self {
        Self {
            resources_index,
            abc_path,
            module_json,
            pack_info,
            unsigned_output,
            artifact_type: artifact_type.into(),
            is_debug,
        }
    }
}
