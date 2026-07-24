//! [`ModuleSnapshot`] — immutable per-module stable inputs.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ModuleJson5;
use thiserror::Error;

use crate::package::snapshot::{PackageSnapshot, PackageSnapshotError};
use crate::paths::ModulePathLayout;

/// Errors produced while reading a module snapshot.
#[derive(Debug, Error)]
pub enum ModuleSnapshotError {
    /// A module file could not be read.
    #[error("failed to read module input {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// `module.json5` could not be parsed.
    #[error("failed to parse module manifest {path}: {message}")]
    ModuleJsonParse { path: PathBuf, message: String },

    /// `oh-package.json5` could not be parsed.
    #[error("failed to parse package manifest {path}: {message}")]
    PackageJsonParse { path: PathBuf, message: String },
}

/// OHOS module type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OhosModuleType {
    Entry,
    Feature,
    Shared,
    Har,
    Unknown(String),
}

impl OhosModuleType {
    pub fn parse(s: &str) -> Self {
        match s {
            "entry" => Self::Entry,
            "feature" => Self::Feature,
            "shared" => Self::Shared,
            "har" => Self::Har,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Entry => "entry",
            Self::Feature => "feature",
            Self::Shared => "shared",
            Self::Har => "har",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

/// Immutable per-module snapshot.
///
/// Replaces repeated `read_module_json5`, `read_module_router_map_ref`,
/// `module_resource_dirs`, and module-package lookups in CLI/profile/loader code.
#[derive(Debug, Clone)]
pub struct ModuleSnapshot {
    /// Module name (from `module.json5` `module.name`).
    pub name: String,
    /// Absolute path to the module root directory.
    pub root: PathBuf,
    /// Absolute path to `src/main/`.
    pub src_main: PathBuf,
    /// Absolute path to `src/main/module.json5`.
    pub module_json_path: Option<PathBuf>,
    /// Parsed `module.json5` content.
    pub module_json: Option<ModuleJson5>,
    /// Module type.
    pub module_type: OhosModuleType,
    /// Absolute path to `oh-package.json5`.
    pub package_path: Option<PathBuf>,
    /// Parsed `oh-package.json5`.
    pub package: Option<Arc<PackageSnapshot>>,
    /// Source root directories (resolved from source-set overlays).
    pub source_roots: Vec<PathBuf>,
    /// Resource root directories.
    pub resource_roots: Vec<PathBuf>,
}

impl ModuleSnapshot {
    /// Build a module snapshot from its root directory.
    pub fn from_root(root: &std::path::Path, name: &str) -> Result<Self, ModuleSnapshotError> {
        let layout = ModulePathLayout::new(root);
        let src_main = layout.src_main();

        // Read module.json5.
        let module_json_path = layout.module_json5();
        let (module_json_path_opt, module_json, module_type) = if module_json_path.is_file() {
            let content = std::fs::read_to_string(&module_json_path).map_err(|source| {
                ModuleSnapshotError::Io {
                    path: module_json_path.clone(),
                    source,
                }
            })?;
            let parsed: ModuleJson5 = json5::from_str(&content).map_err(|error| {
                ModuleSnapshotError::ModuleJsonParse {
                    path: module_json_path.clone(),
                    message: error.to_string(),
                }
            })?;
            let mtype = OhosModuleType::parse(&parsed.module.type_);
            (Some(module_json_path), Some(parsed), mtype)
        } else {
            (None, None, OhosModuleType::Entry)
        };

        // Read oh-package.json5.
        let package_path = layout.oh_package_json5();
        let (package_path_opt, package) = if package_path.is_file() {
            let snapshot = PackageSnapshot::from_root(root)
                .map_err(Self::package_snapshot_error)?
                .map(Arc::new);
            (Some(package_path), snapshot)
        } else {
            (None, None)
        };

        // Default source roots: src/main/ets or src/main/js.
        let mut source_roots = Vec::new();
        let ets_root = layout.ets_source_root();
        let js_root = layout.js_source_root();
        if ets_root.is_dir() {
            source_roots.push(ets_root);
        } else if js_root.is_dir() {
            source_roots.push(js_root);
        }

        // Default resource roots: src/main/resources.
        let mut resource_roots = Vec::new();
        let res_root = layout.resources_root();
        if res_root.is_dir() {
            resource_roots.push(res_root);
        }

        Ok(ModuleSnapshot {
            name: name.to_string(),
            root: root.to_path_buf(),
            src_main,
            module_json_path: module_json_path_opt,
            module_json,
            module_type,
            package_path: package_path_opt,
            package,
            source_roots,
            resource_roots,
        })
    }

    fn package_snapshot_error(error: PackageSnapshotError) -> ModuleSnapshotError {
        match error {
            PackageSnapshotError::Io { path, source } => ModuleSnapshotError::Io { path, source },
            PackageSnapshotError::Parse { path, message } => {
                ModuleSnapshotError::PackageJsonParse { path, message }
            }
        }
    }
}
