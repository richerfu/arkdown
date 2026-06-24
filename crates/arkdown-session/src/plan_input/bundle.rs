//! Narrow plan input for the rolldown bundle stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// BundlePlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the rolldown bundle stage.
///
/// Contains the inputs for configuring the rolldown bundler.
///
/// # Upstream correspondence
///
/// Corresponds to `ArkCompile` rolldown config inputs in hvigor.
#[derive(Debug, Clone)]
pub struct BundlePlanInput {
    /// Absolute path to the `loader.json` entry manifest.
    pub loader_json_path: PathBuf,
    /// Output directory for JS chunks.
    pub output_dir: PathBuf,
    /// OHOS module name.
    pub module_name: String,
    /// Bundle name.
    pub bundle_name: String,
    /// Whether `useNormalizedOHMUrl` is active.
    pub use_normalized_ohm_url: bool,
    /// Whether this is a debug build (affects source maps, opt level).
    pub is_debug: bool,
}

impl BundlePlanInput {
    /// Create from pre-resolved values.
    pub fn from_context_input(
        module_name: impl Into<String>,
        loader_json_path: PathBuf,
        output_dir: PathBuf,
        bundle_name: impl Into<String>,
        is_debug: bool,
    ) -> Self {
        Self {
            loader_json_path,
            output_dir,
            module_name: module_name.into(),
            bundle_name: bundle_name.into(),
            use_normalized_ohm_url: true, // default for API >= 9
            is_debug,
        }
    }
}
