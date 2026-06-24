//! [`ArtifactLayout`] — per-module output directories and artifact paths.

use std::path::PathBuf;
use std::sync::Arc;

use crate::module::snapshot::ModuleSnapshot;
use crate::paths::PathResolver;

/// Per-module artifact output layout.
///
/// Provides typed accessors for all output paths rather than constructing
/// path strings inline throughout the CLI.
#[derive(Debug, Clone)]
pub struct ArtifactLayout {
    /// Root output directory for this module (`.hvigor/default/cache/entry/`).
    pub output_root: PathBuf,
    /// JS chunk output directory.
    pub js_output: PathBuf,
    /// Source map output directory.
    pub source_map_output: PathBuf,
    /// ABC (bytecode) output directory.
    pub abc_output: PathBuf,
    /// Resource output directory.
    pub resource_output: PathBuf,
    /// Pack (HAP/HSP/HAR) output directory.
    pub pack_output: PathBuf,
}

impl ArtifactLayout {
    /// Build the artifact layout for a module.
    pub fn for_module(module: &Arc<ModuleSnapshot>, paths: &Arc<PathResolver>) -> Self {
        let output_root = paths
            .project_root
            .join(".hvigor")
            .join("build")
            .join(&module.name);
        Self {
            js_output: output_root.join("js"),
            source_map_output: output_root.join("sourceMaps"),
            abc_output: output_root.join("ets"),
            resource_output: output_root.join("resources"),
            pack_output: output_root.join("pack"),
            output_root,
        }
    }

    /// Path to the final `module.json` artifact.
    pub fn module_json(&self) -> PathBuf {
        self.pack_output.join("module.json")
    }

    /// Path to the `ets/modules.abc` artifact.
    pub fn modules_abc(&self) -> PathBuf {
        self.abc_output.join("modules.abc")
    }

    /// Path to the `loader.json` artifact.
    pub fn loader_json(&self) -> PathBuf {
        self.output_root.join("loader.json")
    }
}
