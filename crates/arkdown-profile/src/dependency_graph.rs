//! Dependency graph for OHOS projects.
//!
//! Implements the `DependencyGraphService` taxonomy from hvigor's
//! `DependencyManager`, which classifies dependency nodes into:
//!
//! - `SourceHar` — source HAR (`type: "har"` in `module.json5`)
//! - `BytecodeHar` — pre-compiled HAR (`byteCodeHar: true` in `oh-package.json5`)
//! - `SharedHsp` — shared HSP (`type: "shared"` in `module.json5`)
//! - `NativeSo` — native library package (contains `.so` files)
//! - `RuntimeOnly` — package listed under `oh-package.json5#metadata.runtimeOnly.packages`
//! - `Dynamic` — package imported via dynamic `import()` expression
//! - `Bundled` — package bundled inside another HAR's `oh_modules/`
//! - `RemoteOrUnknown` — package not found on disk (remote registry, network-only)
//!
//! ## Node Resolution Search Order
//!
//! For each declared dependency name, the graph searches for a physical
//! directory in this order (mirrors `ohpm` install / `DependencyManager`):
//!
//! 1. `<issuer_store_root>/oh_modules/<name>` — package-local OHPM symlink farm
//! 2. `<issuer_root>/oh_modules/<name>` — package-local install
//! 3. `<module_root>/oh_modules/<name>` — module-local install
//! 4. `<project_root>/oh_modules/<name>` — project-level install
//!
//! OHPM store directory names are intentionally not inferred here. Installed
//! packages are reached through the `oh_modules` symlink farm that OHPM creates.

use std::path::{Path, PathBuf};

use arkdown_path::HvigorNativeLibraryPattern;

/// Classification of a dependency node in the dependency graph.
///
/// Mirrors hvigor's `DependencyManager.NodeType` taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyNodeType {
    /// Source HAR: `module.json5` with `type: "har"`.
    SourceHar,
    /// Pre-compiled ByteCode HAR: `byteCodeHar: true` in `oh-package.json5`.
    BytecodeHar,
    /// Shared HSP: `module.json5` with `type: "shared"`.
    SharedHsp,
    /// Native library package: contains `.so` files under `libs/`.
    NativeSo,
    /// Runtime-only dependency listed in `oh-package.json5#metadata.runtimeOnly.packages`.
    ///
    /// These packages are NOT bundled into the output artifact — they are
    /// expected to be present on the device at runtime.  Hvigor's
    /// `DependencyManager` tracks them separately so they do not appear in
    /// `modulePathMap` but ARE listed in `byteCodeHarInfo` when they are
    /// ByteCode HARs.
    RuntimeOnly,
    /// Remote / unknown — not found on any local search path.
    ///
    /// This covers packages that require a network fetch (not yet in any
    /// local cache), corrupt installs, or package names that cannot be matched
    /// to a directory by the path-resolution rules.
    RemoteOrUnknown,
}

/// A single node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// Package name (from `oh-package.json5#name` or directory alias).
    pub package_name: String,
    /// Absolute path to the package root directory.
    ///
    /// Empty `PathBuf` when `node_type == RemoteOrUnknown` and the package
    /// was not found on disk.
    pub pkg_root: PathBuf,
    /// Classified dependency type.
    pub node_type: DependencyNodeType,
}

/// Dependency graph service for OHOS projects.
///
/// Builds the complete dependency graph from declared `oh-package.json5`
/// entries using BFS traversal.  Covers:
/// - local `oh_modules/` packages at issuer, module, and project levels
/// - OHPM global cache packages (fallback for remote-fetched dependencies)
/// - `runtimeOnly.packages` entries classified as `RuntimeOnly` nodes
/// - `Bundled` classification for packages found inside another package's
///   local `oh_modules/`
///
/// Remote/cache packages that cannot be located on any search path are
/// retained as `RemoteOrUnknown` nodes so that callers can detect missing
/// installations.
///
/// This implements the local portion of hvigor's `DependencyGraphService`.
/// Full remote metadata (registry version manifests, signing status,
/// integrated HSP index) requires network access and is out of scope.
pub struct LocalDependencyGraph {
    /// Map from package name → node, in BFS insertion order.
    pub nodes: indexmap::IndexMap<String, DependencyNode>,
}

impl LocalDependencyGraph {
    /// Build the dependency graph starting from `module_root` and
    /// `project_root` by traversing declared `oh-package.json5` dependencies.
    ///
    /// Also scans `oh-package.json5#metadata.runtimeOnly.packages` at both
    /// roots so that runtime-only ByteCode HARs are classified as
    /// `RuntimeOnly` rather than `RemoteOrUnknown`.
    pub fn build(module_root: &Path, project_root: &Path) -> Self {
        Self::build_with_config(module_root, project_root, None)
    }

    /// Build the dependency graph with explicit resolution configuration.
    ///
    /// Mirrors hvigor's `DependencyManager` construction API — the config
    /// controls offline mode, registry URL, and OHPM cache root.  When
    /// `config.offline` is `false` (the default), packages not found locally
    /// will be fetched from the OHPM registry (step 6 of resolution order).
    ///
    /// Section 3.4: "从 dependency graph 生成所有 package context" — this
    /// is the entry point that drives all remote/cache package resolution.
    pub fn build_with_config(
        module_root: &Path,
        project_root: &Path,
        config: Option<&DependencyResolutionConfig>,
    ) -> Self {
        let mut nodes = indexmap::IndexMap::new();
        let mut queue = std::collections::VecDeque::new();

        // Enqueue direct (non-transitive) dependencies from both roots.
        for dep_name in declared_dep_names(module_root)
            .into_iter()
            .chain(declared_dep_names(project_root))
        {
            queue.push_back((dep_name, module_root.to_path_buf(), false));
        }

        // Enqueue runtimeOnly packages as RuntimeOnly nodes (also non-transitive
        // from the perspective of the module root).
        for pkg_name in runtime_only_package_names(module_root)
            .into_iter()
            .chain(runtime_only_package_names(project_root))
        {
            queue.push_back((pkg_name, module_root.to_path_buf(), false));
        }

        while let Some((dep_name, issuer_root, _is_transitive)) = queue.pop_front() {
            if nodes.contains_key(&dep_name) {
                continue;
            }

            // Is this package listed as a runtimeOnly package?
            let is_runtime_only = is_runtime_only_package(&dep_name, module_root, project_root);

            let Some(pkg_root) = resolve_dep_dir_with_config(
                &dep_name,
                &issuer_root,
                module_root,
                project_root,
                config,
            ) else {
                // Package not found locally on any path — mark remote/unknown.
                nodes.insert(
                    dep_name.clone(),
                    DependencyNode {
                        package_name: dep_name,
                        pkg_root: PathBuf::new(),
                        node_type: if is_runtime_only {
                            DependencyNodeType::RuntimeOnly
                        } else {
                            DependencyNodeType::RemoteOrUnknown
                        },
                    },
                );
                continue;
            };

            // Classify by artifact type.
            // RuntimeOnly overrides other types since it affects packing rules.
            let node_type = if is_runtime_only {
                DependencyNodeType::RuntimeOnly
            } else {
                classify_package(&pkg_root)
            };

            let direct_deps = declared_dep_names(&pkg_root);

            for child in &direct_deps {
                if !nodes.contains_key(child.as_str()) {
                    // Child deps of a resolved package are transitive from
                    // the module root's point of view (they are not direct
                    // deps of the module itself).
                    queue.push_back((child.clone(), pkg_root.clone(), true));
                }
            }

            nodes.insert(
                dep_name.clone(),
                DependencyNode {
                    package_name: dep_name,
                    pkg_root,
                    node_type,
                },
            );
        }

        LocalDependencyGraph { nodes }
    }

    /// Return all nodes classified as a specific type, in BFS order.
    pub fn nodes_of_type(&self, ty: DependencyNodeType) -> Vec<&DependencyNode> {
        self.nodes.values().filter(|n| n.node_type == ty).collect()
    }

    /// Return source HAR nodes.
    pub fn source_hars(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::SourceHar)
    }

    /// Return shared HSP nodes.
    pub fn shared_hsps(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::SharedHsp)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn classify_package(pkg_root: &Path) -> DependencyNodeType {
    // ByteCode HAR: byteCodeHar flag in oh-package.json5.
    if is_bytecode_har(pkg_root) {
        return DependencyNodeType::BytecodeHar;
    }
    // Source/Shared: check module.json5 type field.
    if let Some(module_type) = read_module_type(pkg_root) {
        if module_type == "shared" {
            return DependencyNodeType::SharedHsp;
        }
        if module_type == "har" {
            return DependencyNodeType::SourceHar;
        }
    }
    // Native SO package: contains .so files.
    if has_native_so(pkg_root) {
        return DependencyNodeType::NativeSo;
    }
    DependencyNodeType::RemoteOrUnknown
}

fn is_bytecode_har(pkg_root: &Path) -> bool {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return false;
    };
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMinimal {
        #[serde(default)]
        metadata: PkgMeta,
    }
    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMeta {
        #[serde(default)]
        byte_code_har: bool,
    }
    json5::from_str::<PkgMinimal>(&content)
        .map(|p| p.metadata.byte_code_har)
        .unwrap_or(false)
}

fn read_module_type(pkg_root: &Path) -> Option<String> {
    for path in &["src/main/module.json5", "src/main/module.json"] {
        let file = pkg_root.join(path);
        if !file.is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleSection,
        }
        #[derive(serde::Deserialize)]
        struct ModuleSection {
            #[serde(rename = "type", default)]
            type_: String,
        }
        if let Ok(m) = json5::from_str::<ModuleJson>(&content) {
            return Some(m.module.type_);
        }
    }
    None
}

fn has_native_so(pkg_root: &Path) -> bool {
    for arm_dir in &[
        "libs/arm64-v8a",
        "libs/armeabi-v7a",
        "libs/x86_64",
        "libs/x86",
        "libs/armeabi",
    ] {
        let dir = pkg_root.join(arm_dir);
        if dir.is_dir() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
            {
                if HvigorNativeLibraryPattern::matches_path_under(&dir, entry.path()) {
                    return true;
                }
            }
        }
    }
    false
}

fn declared_dep_names(pkg_root: &Path) -> Vec<String> {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return vec![];
    };
    #[derive(serde::Deserialize)]
    struct PkgDeps {
        #[serde(default)]
        dependencies: indexmap::IndexMap<String, String>,
        #[serde(default, rename = "devDependencies")]
        dev_dependencies: indexmap::IndexMap<String, String>,
        #[serde(default, rename = "dynamicDependencies")]
        dynamic_dependencies: indexmap::IndexMap<String, String>,
    }
    json5::from_str::<PkgDeps>(&content)
        .map(|p| {
            let mut seen = std::collections::BTreeSet::new();
            p.dependencies
                .into_keys()
                .chain(p.dev_dependencies.into_keys())
                .chain(p.dynamic_dependencies.into_keys())
                .filter(|name| seen.insert(name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract package names listed in `oh-package.json5#metadata.runtimeOnly.packages`.
///
/// hvigor's `DependencyManager` treats these as `RuntimeOnly` nodes — they are
/// not bundled into the artifact but may still contribute ByteCode HAR info.
fn runtime_only_package_names(pkg_root: &Path) -> Vec<String> {
    let pkg_json = pkg_root.join("oh-package.json5");
    let Ok(content) = std::fs::read_to_string(&pkg_json) else {
        return vec![];
    };
    #[derive(serde::Deserialize)]
    struct Pkg {
        #[serde(default)]
        metadata: PkgMetaRuntime,
    }
    #[derive(Default, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PkgMetaRuntime {
        #[serde(default)]
        runtime_only: RuntimeOnlySection,
    }
    #[derive(Default, serde::Deserialize)]
    struct RuntimeOnlySection {
        #[serde(default)]
        packages: Vec<String>,
    }
    json5::from_str::<Pkg>(&content)
        .map(|p| p.metadata.runtime_only.packages)
        .unwrap_or_default()
}

/// Check whether `dep_name` appears in any `runtimeOnly.packages` list
/// reachable from `module_root` or `project_root`.
fn is_runtime_only_package(dep_name: &str, module_root: &Path, project_root: &Path) -> bool {
    runtime_only_package_names(module_root)
        .iter()
        .chain(runtime_only_package_names(project_root).iter())
        .any(|n| n == dep_name)
}

/// Resolve a dependency name to its physical directory.
///
/// Search order (mirrors ohpm / DependencyManager):
/// 1. `<issuer_store_root>/oh_modules/<dep_name>`
/// 2. `<issuer_root>/oh_modules/<dep_name>`
/// 3. `<module_root>/oh_modules/<dep_name>`
/// 4. `<project_root>/oh_modules/<dep_name>`
///
/// Per SPEC-048: registry HTTP fetch has been removed; it is now orchestrated
/// by `arkdown-session::PackageAcquisitionPlanner` via `arkdown-ohpm`.
/// Configuration for dependency resolution in `DependencyGraphService`.
///
/// Per SPEC-048: OHPM registry network options have been removed. Registry
/// access is now orchestrated by `arkdown-session` via `arkdown-ohpm`.
/// This struct is retained for API compatibility; local graph resolution does
/// not infer OHPM store directory names.
#[derive(Debug, Clone, Default)]
pub struct DependencyResolutionConfig {}

/// Dependency directory resolution: local oh_modules/ and OHPM global cache.
///
/// Implements hvigor's `DependencyManager` resolution order:
/// 1. issuer store oh_modules/ — package-local OHPM symlink farm
/// 2. issuer oh_modules/ — package-local install
/// 3. module oh_modules/ — module-level install
/// 4. project oh_modules/ — project-level install
///
/// OHPM store directories are opaque. This code follows the `oh_modules`
/// symlinks that OHPM materializes instead of deriving store folder names.
fn resolve_dep_dir_with_config(
    dep_name: &str,
    issuer_root: &Path,
    module_root: &Path,
    project_root: &Path,
    _config: Option<&DependencyResolutionConfig>,
) -> Option<PathBuf> {
    let mut local_roots = Vec::with_capacity(4);
    if let Some(root) = issuer_store_oh_modules_root(issuer_root) {
        local_roots.push(root);
    }
    local_roots.extend([
        issuer_root.join("oh_modules"),
        module_root.join("oh_modules"),
        project_root.join("oh_modules"),
    ]);

    for base in local_roots {
        let candidate = base.join(dep_name);
        if candidate.is_dir() {
            return Some(std::fs::canonicalize(&candidate).unwrap_or(candidate));
        }
    }

    None
}

fn issuer_store_oh_modules_root(issuer_root: &Path) -> Option<PathBuf> {
    let mut current = issuer_root.parent();
    while let Some(path) = current {
        let Some(file_name) = path.file_name() else {
            current = path.parent();
            continue;
        };
        if file_name == "oh_modules" {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_pkg(dir: &Path, name: &str, extra: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("oh-package.json5"),
            format!(r#"{{ "name": "{name}", "dependencies": {{}} {extra} }}"#),
        )
        .unwrap();
    }

    fn write_module_json(pkg: &Path, ty: &str) {
        let src_main = pkg.join("src/main");
        fs::create_dir_all(&src_main).unwrap();
        fs::write(
            src_main.join("module.json5"),
            format!(r#"{{ "module": {{ "name": "mod", "type": "{ty}" }} }}"#),
        )
        .unwrap();
    }

    fn symlink_dir(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target, link).unwrap();
    }

    #[test]
    fn test_classify_source_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/my-har");
        write_pkg(&pkg, "my-har", "");
        write_module_json(&pkg, "har");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "my-har": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-har").expect("my-har must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SourceHar);
    }

    #[test]
    fn test_collects_root_dev_dependency() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/@ohos/hypium");
        write_pkg(&pkg, "@ohos/hypium", r#", "main": "index.js""#);
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "devDependencies": { "@ohos/hypium": "1.0.11" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("@ohos/hypium")
            .expect("dev dependency must be in graph");

        assert_eq!(node.pkg_root, std::fs::canonicalize(pkg).unwrap());
    }

    #[test]
    fn test_classify_bytecode_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/bc-har");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("oh-package.json5"),
            r#"{
              "name": "bc-har",
              "metadata": { "byteCodeHar": true },
              "dependencies": {}
            }"#,
        )
        .unwrap();
        fs::create_dir_all(pkg.join("ets")).unwrap();
        fs::write(pkg.join("ets/modules.abc"), b"PANDA\x00\x00\x00").unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "bc-har": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("bc-har").expect("bc-har must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::BytecodeHar);
    }

    #[test]
    fn test_classify_shared_hsp() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/my-hsp");
        write_pkg(&pkg, "my-hsp", "");
        write_module_json(&pkg, "shared");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "my-hsp": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-hsp").expect("my-hsp must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SharedHsp);
    }

    #[test]
    fn test_classify_remote_or_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        // No oh_modules directory — package not found locally.
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "remote-pkg": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("remote-pkg")
            .expect("remote-pkg must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::RemoteOrUnknown);
    }

    /// runtimeOnly packages must be classified as RuntimeOnly, not RemoteOrUnknown,
    /// even when they cannot be found on disk (they live on the device at runtime).
    #[test]
    fn test_runtime_only_package_classified_as_runtime_only() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": {},
                "metadata": {
                    "runtimeOnly": {
                        "packages": ["@runtime/deviceLib"]
                    }
                }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("@runtime/deviceLib")
            .expect("runtimeOnly pkg must be in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RuntimeOnly,
            "runtimeOnly packages must be classified as RuntimeOnly"
        );
    }

    /// A package that is installed locally AND listed in runtimeOnly should
    /// be classified as RuntimeOnly (RuntimeOnly overrides other classifications).
    #[test]
    fn test_runtime_only_overrides_local_classification() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/rt-har");
        write_pkg(&pkg, "rt-har", "");
        write_module_json(&pkg, "har");
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": { "rt-har": "1.0.0" },
                "metadata": { "runtimeOnly": { "packages": ["rt-har"] } }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("rt-har").expect("rt-har must be in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RuntimeOnly,
            "RuntimeOnly must override SourceHar classification"
        );
    }

    #[test]
    fn test_bfs_insertion_order() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");

        for name in &["alpha", "beta", "gamma"] {
            let pkg = oh_modules.join(name);
            write_pkg(&pkg, name, "");
            write_module_json(&pkg, "har");
        }
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "alpha": "1.0.0", "beta": "1.0.0", "gamma": "1.0.0" } }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let names: Vec<_> = graph.nodes.keys().cloned().collect();
        assert_eq!(
            names,
            vec!["alpha", "beta", "gamma"],
            "BFS insertion order must be preserved"
        );
    }

    /// helper_methods: source_hars(), shared_hsps()
    #[test]
    fn test_helper_methods_by_node_type() {
        let tmp = tempfile::tempdir().unwrap();
        let oh = tmp.path().join("oh_modules");

        let src_pkg = oh.join("src-har");
        write_pkg(&src_pkg, "src-har", "");
        write_module_json(&src_pkg, "har");

        let hsp_pkg = oh.join("my-hsp");
        write_pkg(&hsp_pkg, "my-hsp", "");
        write_module_json(&hsp_pkg, "shared");

        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{
                "dependencies": { "src-har": "1.0.0", "my-hsp": "1.0.0" }
            }"#,
        )
        .unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        assert_eq!(graph.source_hars().len(), 1);
        assert_eq!(graph.shared_hsps().len(), 1);
    }

    /// Packages not found locally must be classified as `RemoteOrUnknown`.
    ///
    /// Per SPEC-048, registry fetch and OHPM cache acquisition are orchestrated
    /// by `arkdown-session`. This test verifies that packages absent from the
    /// local symlink graph are still classified as `RemoteOrUnknown`.
    #[test]
    fn test_dependency_resolution_config_unfound_is_remote_or_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "not-installed": "1.0.0" } }"#,
        )
        .unwrap();

        let config = DependencyResolutionConfig {};
        let graph = LocalDependencyGraph::build_with_config(tmp.path(), tmp.path(), Some(&config));
        let node = graph.nodes.get("not-installed").expect("node in graph");
        assert_eq!(
            node.node_type,
            DependencyNodeType::RemoteOrUnknown,
            "packages not found locally must be classified as RemoteOrUnknown"
        );
    }

    #[test]
    fn test_transitive_dependency_resolves_from_issuer_ohpm_symlink_farm() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let cloud_store_root = oh_modules
            .join(".ohpm")
            .join("@hw-agconnect+cloud@yzqapoubp9vkiyjnrivxkgyixmfolam5fmb1z55bw2a=")
            .join("oh_modules")
            .join("@hw-agconnect")
            .join("cloud");
        let base64_store_root = oh_modules
            .join(".ohpm")
            .join("@protobufjs+base64@clcadirmo9vkiyjnrivxkgyixmfolam5fmb1z55bw2a=")
            .join("oh_modules")
            .join("@protobufjs")
            .join("base64");
        fs::create_dir_all(&cloud_store_root).unwrap();
        fs::create_dir_all(&base64_store_root).unwrap();
        fs::write(
            tmp.path().join("oh-package.json5"),
            r#"{ "dependencies": { "@hw-agconnect/cloud": "1.0.2" } }"#,
        )
        .unwrap();
        fs::write(
            cloud_store_root.join("oh-package.json5"),
            r#"{
                "name": "@hw-agconnect/cloud",
                "version": "1.0.2",
                "dependencies": { "@protobufjs/base64": "1.1.2" }
            }"#,
        )
        .unwrap();
        fs::write(
            base64_store_root.join("oh-package.json5"),
            r#"{ "name": "@protobufjs/base64", "version": "1.1.2" }"#,
        )
        .unwrap();

        let top_scope = oh_modules.join("@hw-agconnect");
        fs::create_dir_all(&top_scope).unwrap();
        symlink_dir(&cloud_store_root, &top_scope.join("cloud"));

        let cloud_scope = cloud_store_root
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("@protobufjs");
        fs::create_dir_all(&cloud_scope).unwrap();
        symlink_dir(&base64_store_root, &cloud_scope.join("base64"));

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph
            .nodes
            .get("@protobufjs/base64")
            .expect("transitive symlink dependency must be in graph");

        assert_eq!(
            node.pkg_root,
            fs::canonicalize(&base64_store_root).unwrap(),
            "dependency graph must follow the issuer package's OHPM symlink farm"
        );
    }
}
