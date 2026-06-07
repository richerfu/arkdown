//! [`ModuleSnapshot`] — immutable per-module stable inputs.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_ohos_schema::{RawModuleJson, RawOhPackage};

use crate::package::snapshot::PackageSnapshot;

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
    pub module_json: Option<RawModuleJson>,
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
    pub fn from_root(root: &std::path::Path, name: &str) -> Result<Self, std::io::Error> {
        let src_main = root.join("src").join("main");

        // Read module.json5.
        let module_json_path = src_main.join("module.json5");
        let (module_json_path_opt, module_json, module_type) =
            if module_json_path.is_file() {
                let content = std::fs::read_to_string(&module_json_path)?;
                let parsed: Option<RawModuleJson> = json5::from_str(&content).ok();
                let mtype = parsed
                    .as_ref()
                    .map(|m| OhosModuleType::parse(&m.module.module_type))
                    .unwrap_or(OhosModuleType::Entry);
                (Some(module_json_path), parsed, mtype)
            } else {
                (None, None, OhosModuleType::Entry)
            };

        // Read oh-package.json5.
        let package_path = root.join("oh-package.json5");
        let (package_path_opt, package) = if package_path.is_file() {
            let content = std::fs::read_to_string(&package_path)?;
            let parsed: Option<RawOhPackage> = json5::from_str(&content).ok();
            let snap = parsed.map(|p| Arc::new(PackageSnapshot {
                name: p.name.clone(),
                version: p.version.clone(),
                root: root.to_path_buf(),
                package_json: p,
            }));
            (Some(package_path), snap)
        } else {
            (None, None)
        };

        // Default source roots: src/main/ets or src/main/js.
        let mut source_roots = Vec::new();
        let ets_root = src_main.join("ets");
        let js_root = src_main.join("js");
        if ets_root.is_dir() {
            source_roots.push(ets_root);
        } else if js_root.is_dir() {
            source_roots.push(js_root);
        }

        // Default resource roots: src/main/resources.
        let mut resource_roots = Vec::new();
        let res_root = src_main.join("resources");
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
}
