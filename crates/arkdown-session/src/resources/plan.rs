//! [`ResourcePlan`] — resource roots, overlays, and dependency resources.

use std::path::PathBuf;
use std::sync::Arc;

use crate::dependency_graph::snapshot::DependencyGraphSnapshot;
use crate::module::snapshot::ModuleSnapshot;

/// Resource compilation plan for one module.
#[derive(Debug, Clone, Default)]
pub struct ResourcePlan {
    /// Primary resource roots (e.g., `src/main/resources`).
    pub roots: Vec<PathBuf>,
    /// App-scope resource root (from `AppScope/resources`), if included.
    pub app_scope_root: Option<PathBuf>,
    /// Resource roots from HAR/HSP dependencies.
    pub dependency_roots: Vec<PathBuf>,
    /// Whether to include app-scope resources.
    pub include_app_scope: bool,
}

impl ResourcePlan {
    /// Build a resource plan for a module from its snapshot and dependency graph.
    pub fn for_module(
        module: &Arc<ModuleSnapshot>,
        _deps: &Arc<DependencyGraphSnapshot>,
    ) -> Self {
        let roots = module.resource_roots.clone();
        Self {
            roots,
            app_scope_root: None,
            dependency_roots: Vec::new(),
            include_app_scope: true,
        }
    }
}
