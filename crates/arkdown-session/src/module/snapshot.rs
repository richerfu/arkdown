//! [`ModuleSnapshot`] — immutable per-module stable inputs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_config::RuntimeModuleContextOverrides;
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

    /// Module `build-profile.json5` could not be parsed.
    #[error("failed to parse module build profile {path}: {message}")]
    BuildProfileParse { path: PathBuf, message: String },
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
    /// Complete `module.json5` value from the same immutable snapshot read.
    ///
    /// The typed model intentionally exposes only build fields owned by core,
    /// while Hvigor consumers such as `MakePackInfo` also project schema fields
    /// including ability `visible`/`forms`. Keeping the raw value prevents a
    /// later filesystem re-read and preserves values replaced through the JS
    /// `OhosModuleContext` setter.
    pub module_json_value: Option<Arc<serde_json::Value>>,
    /// Optional `src/ohosTest/module.json5` source-set value captured with the
    /// main module snapshot. Hvigor switches the complete source-set model for
    /// this target instead of overlaying it at packaging time.
    pub ohos_test_module_json_value: Option<Arc<serde_json::Value>>,
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
    /// Feature-module `entryModules` from the module build profile.
    ///
    /// `None` preserves the distinction between an absent field and an
    /// explicitly empty list used by Hvigor's `checkEntryModules` predicate.
    pub related_entry_modules: Option<Vec<String>>,
}

impl ModuleSnapshot {
    /// Build a module snapshot from its root directory.
    pub fn from_root(root: &std::path::Path, name: &str) -> Result<Self, ModuleSnapshotError> {
        Self::from_root_with_runtime_override(root, name, None)
    }

    /// Build a module snapshot using complete loader objects replaced through
    /// `OhosModuleContext` setters.
    pub fn from_root_with_runtime_override(
        root: &std::path::Path,
        name: &str,
        runtime_override: Option<&RuntimeModuleContextOverrides>,
    ) -> Result<Self, ModuleSnapshotError> {
        Self::from_root_with_runtime_override_and_package_manifest(
            root,
            name,
            runtime_override,
            &ModulePathLayout::new(root).oh_package_json5(),
        )
    }

    pub(crate) fn from_root_with_runtime_override_and_package_manifest(
        root: &std::path::Path,
        name: &str,
        runtime_override: Option<&RuntimeModuleContextOverrides>,
        package_path: &std::path::Path,
    ) -> Result<Self, ModuleSnapshotError> {
        let layout = ModulePathLayout::new(root);
        let src_main = layout.src_main();

        // Read module.json5.
        let module_json_path = layout.module_json5();
        let (module_json_path_opt, module_json, module_json_value, module_type) =
            if let Some(value) =
                runtime_override.and_then(|overrides| overrides.module_json.as_ref())
            {
                let parsed: ModuleJson5 =
                    serde_json::from_value(value.clone()).map_err(|error| {
                        ModuleSnapshotError::ModuleJsonParse {
                            path: module_json_path.clone(),
                            message: error.to_string(),
                        }
                    })?;
                let module_type = OhosModuleType::parse(&parsed.module.type_);
                (
                    Some(module_json_path),
                    Some(parsed),
                    Some(Arc::new(value.clone())),
                    module_type,
                )
            } else if module_json_path.is_file() {
                let content = std::fs::read_to_string(&module_json_path).map_err(|source| {
                    ModuleSnapshotError::Io {
                        path: module_json_path.clone(),
                        source,
                    }
                })?;
                let value: serde_json::Value = json5::from_str(&content).map_err(|error| {
                    ModuleSnapshotError::ModuleJsonParse {
                        path: module_json_path.clone(),
                        message: error.to_string(),
                    }
                })?;
                let parsed: ModuleJson5 =
                    serde_json::from_value(value.clone()).map_err(|error| {
                        ModuleSnapshotError::ModuleJsonParse {
                            path: module_json_path.clone(),
                            message: error.to_string(),
                        }
                    })?;
                let mtype = OhosModuleType::parse(&parsed.module.type_);
                (
                    Some(module_json_path),
                    Some(parsed),
                    Some(Arc::new(value)),
                    mtype,
                )
            } else {
                (None, None, None, OhosModuleType::Entry)
            };
        let ohos_test_module_json_path = layout.ohos_test_module_json5();
        let ohos_test_module_json_value = if ohos_test_module_json_path.is_file() {
            let content =
                std::fs::read_to_string(&ohos_test_module_json_path).map_err(|source| {
                    ModuleSnapshotError::Io {
                        path: ohos_test_module_json_path.clone(),
                        source,
                    }
                })?;
            let value = json5::from_str::<serde_json::Value>(&content).map_err(|error| {
                ModuleSnapshotError::ModuleJsonParse {
                    path: ohos_test_module_json_path,
                    message: error.to_string(),
                }
            })?;
            Some(Arc::new(value))
        } else {
            None
        };

        // Read the project-selected OHPM/npm manifest. Hvigor makes this
        // decision once at project scope and applies it to every module.
        let package_path = package_path.to_path_buf();
        let (package_path_opt, package) = if let Some(value) =
            runtime_override.and_then(|overrides| overrides.package.as_ref())
        {
            let snapshot = PackageSnapshot::from_runtime_manifest_value(root, &package_path, value)
                .map_err(Self::package_snapshot_error)?;
            (Some(package_path), Some(Arc::new(snapshot)))
        } else if package_path.is_file() {
            let snapshot = PackageSnapshot::from_manifest(root, &package_path)
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

        let related_entry_modules = Self::read_related_entry_modules(root)?;

        Ok(ModuleSnapshot {
            name: name.to_string(),
            root: root.to_path_buf(),
            src_main,
            module_json_path: module_json_path_opt,
            module_json,
            module_json_value,
            ohos_test_module_json_value,
            module_type,
            package_path: package_path_opt,
            package,
            source_roots,
            resource_roots,
            related_entry_modules,
        })
    }

    fn read_related_entry_modules(
        module_root: &Path,
    ) -> Result<Option<Vec<String>>, ModuleSnapshotError> {
        let path = module_root.join("build-profile.json5");
        if !path.is_file() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|source| ModuleSnapshotError::Io {
            path: path.clone(),
            source,
        })?;
        let value: serde_json::Value =
            json5::from_str(&content).map_err(|error| ModuleSnapshotError::BuildProfileParse {
                path,
                message: error.to_string(),
            })?;
        Ok(value.get("entryModules").map(|entries| {
            entries
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        }))
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
