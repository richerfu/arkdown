//! Narrow plan input for the native C/C++ build stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// NativePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the native C/C++ build stage.
///
/// Contains the inputs needed by the native stage runner to invoke the NDK
/// toolchain or a pre-built SO inclusion plan.
///
/// # Upstream correspondence
///
/// Corresponds to `NativeBuild` task inputs assembled by `TaskCreator`
/// in hvigor.
#[derive(Debug, Clone)]
pub struct NativePlanInput {
    /// Absolute path to the module's `CMakeLists.txt` or build script.
    pub build_script: Option<PathBuf>,
    /// Output directory for compiled `.so` files.
    pub output_dir: PathBuf,
    /// Target ABI triples (e.g., `["arm64-v8a", "x86_64"]`).
    pub target_abis: Vec<String>,
    /// OHOS module name.
    pub module_name: String,
    /// Whether this is a debug build.
    pub is_debug: bool,
}

impl NativePlanInput {
    /// Create from pre-resolved values.
    pub fn from_context_input(
        module_name: impl Into<String>,
        output_dir: PathBuf,
        target_abis: Vec<String>,
        is_debug: bool,
    ) -> Self {
        Self {
            build_script: None,
            output_dir,
            target_abis,
            module_name: module_name.into(),
            is_debug,
        }
    }
}
