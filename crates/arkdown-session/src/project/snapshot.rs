//! [`ProjectSnapshot`] — immutable project-level build inputs.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ArkdownConfig;
use indexmap::IndexMap;
use thiserror::Error;

use crate::dependency_graph::builder::{DependencyGraphBuilder, DependencySnapshotInventory};
use crate::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyParameterError, DependencyParameterSnapshot,
};
use crate::module::snapshot::{ModuleSnapshot, ModuleSnapshotError};
use crate::package::acquisition::{
    AcquisitionMode, PackageAcquisitionError, PackageAcquisitionPlanner,
};
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
    /// OHPM dependency acquisition failed before graph construction.
    #[error("package acquisition error: {0}")]
    PackageAcquisition(#[source] Box<PackageAcquisitionError>),
}

impl From<PackageAcquisitionError> for ProjectSnapshotError {
    fn from(value: PackageAcquisitionError) -> Self {
        Self::PackageAcquisition(Box::new(value))
    }
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
    /// Minimum compatible version code from `AppScope/app.json5`.
    ///
    /// Hvigor's `MakePackInfo` reads this field directly from the app resource
    /// model; product and module profiles do not provide a fallback.
    pub min_compatible_version_code: Option<u64>,
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
        Self::from_config_with_parameter_file(config, None)
    }

    /// Build the immutable project model from disk with an optional explicit
    /// OHPM parameter-file path supplied by the command line.
    pub fn from_config_with_parameter_file(
        config: &ArkdownConfig,
        parameter_file_override: Option<PathBuf>,
    ) -> Result<Self, ProjectSnapshotError> {
        Self::from_config_with_parameter_file_and_acquisition_mode(
            config,
            parameter_file_override,
            None,
        )
    }

    pub(crate) fn from_config_with_parameter_file_and_acquisition_mode(
        config: &ArkdownConfig,
        parameter_file_override: Option<PathBuf>,
        acquisition_mode: Option<AcquisitionMode>,
    ) -> Result<Self, ProjectSnapshotError> {
        let root = &config.project_root;
        let app_json_path = ProjectPathLayout::new(root).app_scope_app_json5();
        let (
            app_bundle_name,
            app_version_code,
            app_version_name,
            app_build_version,
            app_min_compatible_version_code,
            app_bundle_type,
        ) = if app_json_path.is_file() {
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
                app.app.min_compatible_version_code,
                app.app.bundle_type.unwrap_or_else(|| "app".to_string()),
            )
        } else {
            (
                String::new(),
                0,
                String::new(),
                None,
                None,
                "app".to_string(),
            )
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

        // `CoreProjectModelImpl.isOhpmProject()` is project-wide: API 9+ uses
        // OHPM when the root oh-package exists or the legacy package.json is
        // absent; otherwise every module is loaded through npm metadata.
        let use_ohpm = config.sdk_config.compile_api_version >= 9
            && (root.join("oh-package.json5").is_file() || !root.join("package.json").is_file());
        let package_file_name = if use_ohpm {
            "oh-package.json5"
        } else {
            "package.json"
        };

        let acquisitions = if use_ohpm {
            acquisition_mode
                .map(|mode| {
                    PackageAcquisitionPlanner::new(
                        root,
                        std::iter::once(root.clone())
                            .chain(config.modules.iter().map(|module| module.src_path.clone())),
                        mode,
                    )
                    .acquire()
                })
                .transpose()?
        } else {
            None
        };

        let mut module_order = Vec::with_capacity(config.modules.len());
        let mut modules = IndexMap::with_capacity(config.modules.len());
        for module in &config.modules {
            let package_path = module.src_path.join(package_file_name);
            let snap = if let Some(acquisitions) = acquisitions.as_ref() {
                ModuleSnapshot::from_root_with_package_snapshot(
                    &module.src_path,
                    &module.name,
                    package_path.is_file().then_some(package_path),
                    acquisitions.package(&module.src_path),
                )?
            } else {
                ModuleSnapshot::from_root_with_package_manifest(
                    &module.src_path,
                    &module.name,
                    &package_path,
                )?
            };
            module_order.push(module.name.clone());
            modules.insert(module.name.clone(), Arc::new(snap));
        }

        let root_package = if let Some(acquisitions) = acquisitions.as_ref() {
            acquisitions.package(root)
        } else {
            PackageSnapshot::from_manifest(root, &root.join(package_file_name))?.map(Arc::new)
        };
        let dependency_parameters = if use_ohpm {
            DependencyParameterSnapshot::from_project(
                root,
                root_package.as_deref(),
                parameter_file_override.as_deref(),
            )?
        } else {
            DependencyParameterSnapshot::default()
        };

        let mut dependency_inventory = acquisitions
            .as_ref()
            .map(DependencySnapshotInventory::with_acquisitions)
            .unwrap_or_default();
        dependency_inventory.insert_package(root, root_package.clone());
        for module in modules.values() {
            dependency_inventory.insert_package(&module.root, module.package.clone());
        }

        let mut dependency_graphs = IndexMap::with_capacity(module_order.len());
        for module_name in &module_order {
            dependency_graphs.insert(
                module_name.clone(),
                Arc::new({
                    DependencyGraphBuilder::new(
                        module_name.clone(),
                        root.clone(),
                        modules
                            .get(module_name)
                            .map(|module| module.root.clone())
                            .unwrap_or_else(|| root.clone()),
                    )
                    .with_package_manager(use_ohpm)
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
            min_compatible_version_code: app_min_compatible_version_code,
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
