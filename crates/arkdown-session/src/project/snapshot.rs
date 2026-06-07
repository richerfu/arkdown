//! [`ProjectSnapshot`] — immutable project-level build inputs.

use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use thiserror::Error;

use crate::dependency_graph::snapshot::DependencyGraphSnapshot;
use crate::module::snapshot::ModuleSnapshot;
use crate::package::snapshot::PackageSnapshot;

/// Project-level errors.
#[derive(Debug, Error)]
pub enum ProjectSnapshotError {
    /// A required project file was not found.
    #[error("required file not found: {0}")]
    FileNotFound(PathBuf),
    /// A file could not be parsed.
    #[error("parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Immutable project-level snapshot.
///
/// Constructed once by [`ProjectSnapshotBuilder`](super::builder::ProjectSnapshotBuilder)
/// and shared across all stages via `Arc`.
#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    /// Absolute path to the project root.
    pub root: PathBuf,
    /// Application bundle name from `AppScope/app.json5`.
    pub bundle_name: String,
    /// Version code from `AppScope/app.json5`.
    pub version_code: u64,
    /// Version name from `AppScope/app.json5`.
    pub version_name: String,
    /// Selected product name.
    pub product_name: String,
    /// Module name order (from `build-profile.json5` modules array).
    pub module_order: Vec<String>,
    /// Per-module snapshots.
    pub modules: IndexMap<String, Arc<ModuleSnapshot>>,
    /// Root package (`oh-package.json5` at the project root), if present.
    pub root_package: Option<Arc<PackageSnapshot>>,
    /// Dependency graphs indexed by host module name.
    pub dependency_graphs: IndexMap<String, Arc<DependencyGraphSnapshot>>,
}

impl ProjectSnapshot {
    /// Build a snapshot from the project root directory.
    ///
    /// This is a minimal implementation; the full implementation in
    /// `ProjectSnapshotBuilder` will parallel-scan all manifests.
    pub fn from_root(
        root: &std::path::Path,
        product_name: &str,
    ) -> Result<Self, ProjectSnapshotError> {
        use arkdown_ohos_schema::RawAppJson5;

        // Read AppScope/app.json5 if present.
        let app_json_path = root.join("AppScope/app.json5");
        let (bundle_name, version_code, version_name) = if app_json_path.is_file() {
            let content = std::fs::read_to_string(&app_json_path)?;
            let app: RawAppJson5 =
                json5::from_str(&content).map_err(|e| ProjectSnapshotError::Parse {
                    file: app_json_path.clone(),
                    message: e.to_string(),
                })?;
            (
                app.app.bundle_name,
                app.app.version_code,
                app.app.version_name,
            )
        } else {
            (String::new(), 0, String::new())
        };

        // Read build-profile.json5 for module list.
        use arkdown_ohos_schema::RawBuildProfile;
        let bp_path = root.join("build-profile.json5");
        let (module_order, modules_map) = if bp_path.is_file() {
            let content = std::fs::read_to_string(&bp_path)?;
            let bp: RawBuildProfile =
                json5::from_str(&content).map_err(|e| ProjectSnapshotError::Parse {
                    file: bp_path.clone(),
                    message: e.to_string(),
                })?;
            let mut order = Vec::new();
            let mut modules = IndexMap::new();
            for m in &bp.modules {
                let module_root = root.join(m.src_path.trim_start_matches("./"));
                if let Ok(snap) = ModuleSnapshot::from_root(&module_root, &m.name) {
                    order.push(m.name.clone());
                    modules.insert(m.name.clone(), Arc::new(snap));
                }
            }
            (order, modules)
        } else {
            (Vec::new(), IndexMap::new())
        };

        Ok(ProjectSnapshot {
            root: root.to_path_buf(),
            bundle_name,
            version_code,
            version_name,
            product_name: product_name.to_string(),
            module_order,
            modules: modules_map,
            root_package: None,
            dependency_graphs: IndexMap::new(),
        })
    }

    /// Return the dependency graph for a host module.
    pub fn dependency_graph(&self, module_name: &str) -> Option<Arc<DependencyGraphSnapshot>> {
        self.dependency_graphs.get(module_name).cloned()
    }
}
