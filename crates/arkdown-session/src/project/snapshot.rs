//! [`ProjectSnapshot`] — immutable project-level build inputs.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_config::RuntimeContextOverrides;
use arkdown_core::ArkdownConfig;
use indexmap::IndexMap;
use thiserror::Error;

use crate::dependency_graph::builder::{DependencyGraphBuilder, DependencySnapshotInventory};
use crate::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyParameterError, DependencyParameterSnapshot,
};
use crate::module::snapshot::{ModuleSnapshot, ModuleSnapshotError};
use crate::package::snapshot::{PackageSnapshot, PackageSnapshotError};
use crate::paths::ProjectPathLayout;

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
    /// Module snapshot error.
    #[error("module snapshot error: {0}")]
    Module(#[from] ModuleSnapshotError),
    /// Package snapshot error.
    #[error("package snapshot error: {0}")]
    Package(#[from] PackageSnapshotError),
    /// Project dependency parameter-file error.
    #[error("dependency parameter error: {0}")]
    DependencyParameter(#[from] DependencyParameterError),
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
    /// Effective build version using hvigor's invocation → product → app precedence.
    pub build_version: Option<String>,
    /// Bundle type from `AppScope/app.json5`.
    pub bundle_type: String,
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
    /// Build a snapshot from the normalized build config.
    pub fn from_config(config: &ArkdownConfig) -> Result<Self, ProjectSnapshotError> {
        Self::from_config_with_runtime_overrides(config, None, None)
    }

    /// Build the immutable project model from disk plus the complete loader
    /// objects replaced by hvigor plugin context setters.
    pub fn from_config_with_runtime_overrides(
        config: &ArkdownConfig,
        runtime_overrides: Option<Arc<RuntimeContextOverrides>>,
        parameter_file_override: Option<PathBuf>,
    ) -> Result<Self, ProjectSnapshotError> {
        let root = &config.project_root;
        let app_json_path = ProjectPathLayout::new(root).app_scope_app_json5();
        let (
            app_bundle_name,
            app_version_code,
            app_version_name,
            app_build_version,
            app_bundle_type,
        ) = if let Some(value) = runtime_overrides
            .as_ref()
            .and_then(|overrides| overrides.project.app_json.as_ref())
        {
            let app: arkdown_ohos_schema::RawAppJson5 = serde_json::from_value(value.clone())
                .map_err(|error| ProjectSnapshotError::Parse {
                    file: app_json_path.clone(),
                    message: error.to_string(),
                })?;
            (
                app.app.bundle_name,
                app.app.version_code,
                app.app.version_name,
                app.app.build_version,
                app.app.bundle_type.unwrap_or_else(|| "app".to_string()),
            )
        } else if app_json_path.is_file() {
            let content = std::fs::read_to_string(&app_json_path)?;
            let app: arkdown_ohos_schema::RawAppJson5 =
                json5::from_str(&content).map_err(|e| ProjectSnapshotError::Parse {
                    file: app_json_path.clone(),
                    message: e.to_string(),
                })?;
            (
                app.app.bundle_name,
                app.app.version_code,
                app.app.version_name,
                app.app.build_version,
                app.app.bundle_type.unwrap_or_else(|| "app".to_string()),
            )
        } else {
            (String::new(), 0, String::new(), None, "app".to_string())
        };
        // Hvigor MergeProfile.getMergedAppOptObj applies product/build-script
        // values after AppScope/app.json5. `load_config` has already folded
        // hvigorfile appOpt into ProductConfig at the highest precedence.
        let bundle_name = if config.product.bundle_name.is_empty() {
            app_bundle_name
        } else {
            config.product.bundle_name.clone()
        };
        let version_code = config.product.version_code.unwrap_or(app_version_code);
        let version_name = config
            .product
            .version_name
            .clone()
            .unwrap_or(app_version_name);
        let build_version = config.product.build_version.clone().or(app_build_version);
        let bundle_type = config
            .product
            .app_options
            .bundle_type
            .clone()
            .unwrap_or(app_bundle_type);

        let mut module_order = Vec::with_capacity(config.modules.len());
        let mut modules = IndexMap::with_capacity(config.modules.len());
        for module in &config.modules {
            let snap = ModuleSnapshot::from_root_with_runtime_override(
                &module.src_path,
                &module.name,
                runtime_overrides
                    .as_ref()
                    .and_then(|overrides| overrides.module_for_root(&module.src_path)),
            )?;
            module_order.push(module.name.clone());
            modules.insert(module.name.clone(), Arc::new(snap));
        }

        let root_package = if let Some(value) = runtime_overrides
            .as_ref()
            .and_then(|overrides| overrides.project.package.as_ref())
        {
            Some(Arc::new(PackageSnapshot::from_runtime_value(root, value)?))
        } else {
            PackageSnapshot::from_root(root)?.map(Arc::new)
        };
        let dependency_parameters = DependencyParameterSnapshot::from_project(
            root,
            root_package.as_deref(),
            parameter_file_override.as_deref(),
        )?;

        let mut dependency_inventory = DependencySnapshotInventory::default();
        dependency_inventory.insert_package(root, root_package.clone());
        for module in modules.values() {
            dependency_inventory.insert_package(&module.root, module.package.clone());
        }

        let mut dependency_graphs = IndexMap::with_capacity(module_order.len());
        for module_name in &module_order {
            dependency_graphs.insert(
                module_name.clone(),
                Arc::new({
                    let mut builder = DependencyGraphBuilder::new(
                        module_name.clone(),
                        root.clone(),
                        modules
                            .get(module_name)
                            .map(|module| module.root.clone())
                            .unwrap_or_else(|| root.clone()),
                    );
                    if let Some(runtime_overrides) = runtime_overrides.as_ref() {
                        builder = builder.with_runtime_overrides(Arc::clone(runtime_overrides));
                    }
                    builder
                        .with_dependency_parameters(dependency_parameters.clone())
                        .build_with_inventory(&mut dependency_inventory)
                }),
            );
        }

        Ok(ProjectSnapshot {
            root: root.clone(),
            bundle_name,
            version_code,
            version_name,
            build_version,
            bundle_type,
            product_name: config.product.name.clone(),
            module_order,
            modules,
            root_package,
            dependency_graphs,
        })
    }

    /// Return the dependency graph for a host module.
    pub fn dependency_graph(&self, module_name: &str) -> Option<Arc<DependencyGraphSnapshot>> {
        self.dependency_graphs.get(module_name).cloned()
    }
}
