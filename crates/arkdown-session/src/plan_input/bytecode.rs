//! Narrow plan input for the bytecode (ABC) compilation stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// BytecodePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the bytecode (ABC) compilation stage.
///
/// Contains the inputs needed by `packages/arkdown::cmd::build::bytecode` to invoke SDK `es2abc`.
///
/// # Upstream correspondence
///
/// Corresponds to `CompileArkTS` / `es2abc` invocation inputs assembled by
/// `TaskCreator` in hvigor.
#[derive(Debug, Clone)]
pub struct BytecodePlanInput {
    /// Absolute path to the bundled JS/TS entry file.
    pub entry_path: PathBuf,
    /// Output path for the compiled `modules.abc` file.
    pub abc_output: PathBuf,
    /// OHOS module name.
    pub module_name: String,
    /// Bundle name.
    pub bundle_name: String,
    /// Target API version (determines bytecode version).
    pub api_version: u32,
    /// Whether to produce a debug build (preserves source maps, no opt).
    pub is_debug: bool,
    /// Whether to enable merge-abc mode for multi-module builds.
    pub merge_abc: bool,
}

impl BytecodePlanInput {
    /// Create from pre-resolved values.
    pub fn from_context_input(
        module_name: impl Into<String>,
        entry_path: PathBuf,
        abc_output: PathBuf,
        bundle_name: impl Into<String>,
        api_version: u32,
        is_debug: bool,
    ) -> Self {
        Self {
            entry_path,
            abc_output,
            module_name: module_name.into(),
            bundle_name: bundle_name.into(),
            api_version,
            is_debug,
            merge_abc: false,
        }
    }
}
