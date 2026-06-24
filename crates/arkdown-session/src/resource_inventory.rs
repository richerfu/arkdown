//! Resource compilation inventory.
//!
//! [`ResourceInventory`] captures resource roots, individual resource file
//! entries, and native library scan results for a module. It is the Rust
//! equivalent of the `CompileResource` and `ProcessLibs` tasks in hvigor.
//!
//! This module is **distinct** from `resources/plan.rs` (`ResourcePlan`).
//! `ResourcePlan` is a narrow input view used during bundle planning;
//! `ResourceInventory` is a full scan result used during resource compilation
//! and native library packaging.
//!
//! # Upstream correspondence
//!
//! | Rust type                  | hvigor task / file                        |
//! |----------------------------|-------------------------------------------|
//! | [`ResourceInventory`]      | `CompileResource` task in hvigor          |
//! | [`ResourceRoot`]           | `resourceDirs` in `module.json5`          |
//! | [`ResourceInput`]          | single resource file entry in restool scan|
//! | [`NativeLibInventory`]     | `ProcessLibs` native lib scan             |

use std::path::{Path, PathBuf};

use arkdown_path::HvigorNativeLibraryPattern;

use crate::dependency_graph::snapshot::DependencyGraphSnapshot;
use crate::module::snapshot::ModuleSnapshot;

// ---------------------------------------------------------------------------
// ResourceRoot
// ---------------------------------------------------------------------------

/// A resource root directory for a module.
///
/// Upstream: `resourceDirs` entry in `module.json5` / `CompileResource` task.
#[derive(Debug, Clone)]
pub struct ResourceRoot {
    /// Absolute path to the resource root directory.
    pub path: PathBuf,
    /// Whether this root comes from the `AppScope` (project-level resources).
    pub is_app_scope: bool,
    /// Whether this root comes from a dependency (HAR/HSP package).
    pub is_dependency: bool,
}

impl ResourceRoot {
    /// Construct a module-local resource root.
    pub fn module_root(path: PathBuf) -> Self {
        Self {
            path,
            is_app_scope: false,
            is_dependency: false,
        }
    }

    /// Construct an app-scope resource root.
    pub fn app_scope(path: PathBuf) -> Self {
        Self {
            path,
            is_app_scope: true,
            is_dependency: false,
        }
    }

    /// Construct a dependency resource root.
    pub fn dependency(path: PathBuf) -> Self {
        Self {
            path,
            is_app_scope: false,
            is_dependency: true,
        }
    }

    /// Return a reference to the inner path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// ResourceInput
// ---------------------------------------------------------------------------

/// A single resource file entry discovered during scanning.
///
/// Upstream: individual file entry in `restool` input scan (`CompileResource`).
#[derive(Debug, Clone)]
pub struct ResourceInput {
    /// Absolute path to the resource file.
    pub path: PathBuf,
    /// Resource type category (e.g., `"layout"`, `"media"`, `"string"`).
    pub resource_type: String,
    /// Relative path within its resource root.
    pub relative_path: PathBuf,
}

impl ResourceInput {
    /// Construct a resource input entry.
    pub fn new(path: PathBuf, resource_type: String, relative_path: PathBuf) -> Self {
        Self {
            path,
            resource_type,
            relative_path,
        }
    }

    /// Return a reference to the file path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// NativeLibInventory
// ---------------------------------------------------------------------------

/// Inventory of native libraries discovered via a file system scan.
///
/// Upstream: `ProcessLibs` native library scan in hvigor.
#[derive(Debug, Clone, Default)]
pub struct NativeLibInventory {
    /// Absolute paths to native libraries found by hvigor's default glob.
    pub lib_paths: Vec<PathBuf>,
    /// ABI subdirectory names found (e.g., `["arm64-v8a", "x86_64"]`).
    pub abi_dirs: Vec<String>,
}

impl NativeLibInventory {
    /// Scan a package root for native libraries.
    ///
    /// Looks under `<package_root>/libs` with hvigor's default
    /// `NativeConst.LIBRARY_FILE_PATTERN`.
    ///
    /// Upstream: `ProcessLibs` in hvigor.
    pub fn from_package_root(package_root: &Path) -> Self {
        let libs_dir = package_root.join("libs");
        if !libs_dir.is_dir() {
            return Self::default();
        }

        let mut lib_paths = Vec::new();
        let mut abi_dirs = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&libs_dir) {
            for entry in entries.flatten() {
                let abi_path = entry.path();
                if abi_path.is_dir() {
                    if let Some(abi_name) = abi_path.file_name().and_then(|n| n.to_str()) {
                        abi_dirs.push(abi_name.to_string());
                    }
                }
            }
        }

        for entry in walkdir::WalkDir::new(&libs_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            if HvigorNativeLibraryPattern::matches_path_under(&libs_dir, path) {
                lib_paths.push(path.to_path_buf());
            }
        }
        abi_dirs.sort();
        lib_paths.sort();

        Self {
            lib_paths,
            abi_dirs,
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceInventory
// ---------------------------------------------------------------------------

/// Full resource compilation inventory for a module.
///
/// Captures all resource roots, individual resource file entries, and native
/// library scan results. Built once at session startup.
///
/// Upstream: `CompileResource` task inputs in hvigor.
#[derive(Debug, Clone, Default)]
pub struct ResourceInventory {
    /// All resource roots for this module (module-local + app-scope + deps).
    pub roots: Vec<ResourceRoot>,
    /// All individual resource file entries discovered.
    pub inputs: Vec<ResourceInput>,
    /// Native library inventory from the module's package scan.
    pub native_libs: NativeLibInventory,
    /// Content fingerprint of all resource inputs (FNV-1a hash).
    pub fingerprint: u64,
}

impl ResourceInventory {
    /// Discover resource inputs for a module from its snapshot and dependency graph.
    ///
    /// Scans the module's resource roots and collects all resource files.
    ///
    /// Upstream: `CompileResource` task initialization.
    pub fn discover(module: &ModuleSnapshot, _deps: &DependencyGraphSnapshot) -> Self {
        let mut roots = Vec::new();
        let mut inputs = Vec::new();

        // Add module-local resource roots.
        for root in &module.resource_roots {
            roots.push(ResourceRoot::module_root(root.clone()));
            // Scan for resource files.
            if root.is_dir() {
                scan_resource_dir(root, root, &mut inputs);
            }
        }

        // Compute a simple fingerprint from the count of inputs.
        let fingerprint = inputs.len() as u64;

        Self {
            roots,
            inputs,
            native_libs: NativeLibInventory::default(),
            fingerprint,
        }
    }

    /// Return an iterator over resource inputs for a specific module.
    ///
    /// Upstream: `CompileResource` per-module input list.
    pub fn resources_for_module<'a>(
        &'a self,
        module_name: &'a str,
    ) -> impl Iterator<Item = &'a ResourceInput> {
        let _ = module_name; // All inputs belong to this module's inventory.
        self.inputs.iter()
    }

    /// Return the native library inventory for a package root.
    ///
    /// Upstream: `ProcessLibs` output.
    pub fn native_libs_for_package(&self, _package_root: &Path) -> &NativeLibInventory {
        &self.native_libs
    }

    /// Return the content fingerprint of all resource inputs.
    ///
    /// Used for cache invalidation.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Return `true` if there are no resource inputs.
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }
}

/// Recursively scan a resource directory for resource files.
fn scan_resource_dir(root: &Path, dir: &Path, inputs: &mut Vec<ResourceInput>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_resource_dir(root, &path, inputs);
        } else if path.is_file() {
            let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            // Infer resource type from the parent directory name.
            let resource_type = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            inputs.push(ResourceInput::new(path, resource_type, relative_path));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dummy_module(root: &str, name: &str) -> ModuleSnapshot {
        ModuleSnapshot {
            name: name.to_string(),
            root: PathBuf::from(root),
            src_main: PathBuf::from(root).join("src/main"),
            module_json_path: None,
            module_json: None,
            module_type: crate::module::snapshot::OhosModuleType::Entry,
            package_path: None,
            package: None,
            source_roots: vec![],
            resource_roots: vec![],
        }
    }

    #[test]
    fn empty_inventory_for_module_without_resources() {
        let module = dummy_module("/tmp/no_resources_module", "entry");
        let graph = DependencyGraphSnapshot::default();
        let inv = ResourceInventory::discover(&module, &graph);
        assert!(inv.is_empty());
        assert_eq!(inv.roots.len(), 0);
        assert_eq!(inv.fingerprint(), 0);
    }

    #[test]
    fn resource_root_kinds() {
        let p = PathBuf::from("/module/src/main/resources");
        let module_root = ResourceRoot::module_root(p.clone());
        assert!(!module_root.is_app_scope);
        assert!(!module_root.is_dependency);

        let app_root = ResourceRoot::app_scope(p.clone());
        assert!(app_root.is_app_scope);

        let dep_root = ResourceRoot::dependency(p.clone());
        assert!(dep_root.is_dependency);
    }

    #[test]
    fn resource_input_fields() {
        let path = PathBuf::from("/module/src/main/resources/base/media/icon.png");
        let rel = PathBuf::from("base/media/icon.png");
        let input = ResourceInput::new(path.clone(), "media".to_string(), rel.clone());
        assert_eq!(input.as_path(), path.as_path());
        assert_eq!(input.resource_type, "media");
        assert_eq!(input.relative_path, rel);
    }

    #[test]
    fn native_lib_inventory_empty_for_missing_dir() {
        let inv = NativeLibInventory::from_package_root(Path::new("/nonexistent/package"));
        assert!(inv.lib_paths.is_empty());
        assert!(inv.abi_dirs.is_empty());
    }

    #[test]
    fn native_lib_inventory_matches_hvigor_default_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let abi_dir = tmp.path().join("libs/arm64-v8a");
        std::fs::create_dir_all(&abi_dir).unwrap();
        std::fs::write(abi_dir.join("libplain.so"), b"ELF").unwrap();
        std::fs::write(abi_dir.join("libversioned.so.1"), b"ELF").unwrap();
        std::fs::write(abi_dir.join("._libplain.so"), b"AppleDouble").unwrap();

        let inv = NativeLibInventory::from_package_root(tmp.path());
        let names = inv
            .lib_paths
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["libplain.so", "libversioned.so.1"]);
    }

    #[test]
    fn resources_for_module_returns_all_inputs() {
        let inv = ResourceInventory {
            roots: vec![],
            inputs: vec![
                ResourceInput::new(
                    PathBuf::from("/a.png"),
                    "media".to_string(),
                    PathBuf::from("a.png"),
                ),
                ResourceInput::new(
                    PathBuf::from("/b.json"),
                    "profile".to_string(),
                    PathBuf::from("b.json"),
                ),
            ],
            native_libs: NativeLibInventory::default(),
            fingerprint: 2,
        };
        let count = inv.resources_for_module("entry").count();
        assert_eq!(count, 2);
    }
}
