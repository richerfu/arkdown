//! Narrow plan input for the filesystem watch stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// WatchPlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the filesystem watch stage.
///
/// Contains the watch roots and glob patterns needed by
/// `arkdown-watcher::WatchPlanner` to set up incremental rebuild triggers.
///
/// # Upstream correspondence
///
/// Corresponds to the watch configuration assembled from module source roots,
/// resource directories, and config files in hvigor's watch mode.
#[derive(Debug, Clone)]
pub struct WatchPlanInput {
    /// Source root directories to watch for `.ets`/`.ts` changes.
    pub source_roots: Vec<PathBuf>,
    /// Resource directories to watch for resource changes.
    pub resource_roots: Vec<PathBuf>,
    /// Config files to watch for invalidation (e.g., `module.json5`).
    pub config_files: Vec<PathBuf>,
    /// OHOS module name.
    pub module_name: String,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl WatchPlanInput {
    /// Create from pre-resolved values with a default debounce of 300 ms.
    pub fn from_context_input(
        module_name: impl Into<String>,
        source_roots: Vec<PathBuf>,
        resource_roots: Vec<PathBuf>,
        config_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            source_roots,
            resource_roots,
            config_files,
            module_name: module_name.into(),
            debounce_ms: 300,
        }
    }
}
