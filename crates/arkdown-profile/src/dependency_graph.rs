//! Basic local dependency graph for OHOS projects.
//!
//! Provides node classification for local (on-disk) dependencies following
//! hvigor's DependencyManager taxonomy. This is a simplified version that
//! handles only local `oh_modules/` packages; remote/cache nodes require
//! the full DependencyGraphService implementation.
//!
//! Node types:
//! - `SourceHar`: source HAR — has `src/main/module.json5` with `type: "har"`
//! - `BytecodeHar`: pre-compiled HAR — has `byteCodeHar: true` in `oh-package.json5`
//! - `SharedHsp`: shared HSP — has `src/main/module.json5` with `type: "shared"`
//! - `NativeSo`: native library package — contains `.so` files
//! - `RemoteOrUnknown`: remote package or type not determinable locally

use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;

/// Classification of a dependency node in the local dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyNodeType {
    /// Source HAR: module.json5 with `type: "har"`.
    SourceHar,
    /// Pre-compiled ByteCode HAR: `byteCodeHar: true` in oh-package.json5.
    BytecodeHar,
    /// Shared HSP: module.json5 with `type: "shared"`.
    SharedHsp,
    /// Native library package: contains `.so` files.
    NativeSo,
    /// Remote / unknown — type cannot be determined from local files.
    RemoteOrUnknown,
}

/// A single node in the local dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// Package name (from `oh-package.json5#name` or directory alias).
    pub package_name: String,
    /// Absolute path to the package root directory.
    pub pkg_root: PathBuf,
    /// Classified dependency type.
    pub node_type: DependencyNodeType,
    /// Names of direct dependencies (from `oh-package.json5#dependencies`).
    pub direct_deps: Vec<String>,
}

/// Minimal local dependency graph built from declared `oh-package.json5` entries.
///
/// Only covers packages installed under `oh_modules/` on disk.  Remote
/// registry packages that are not installed are classified as
/// `RemoteOrUnknown`.
pub struct LocalDependencyGraph {
    /// Map from package name → node, in BFS insertion order.
    pub nodes: indexmap::IndexMap<String, DependencyNode>,
}

impl LocalDependencyGraph {
    /// Build the dependency graph starting from `module_root` and
    /// `project_root` by traversing declared `oh-package.json5` dependencies.
    pub fn build(module_root: &Path, project_root: &Path) -> Self {
        let mut nodes = indexmap::IndexMap::new();
        let mut queue = std::collections::VecDeque::new();

        // Enqueue root-level dependencies.
        for dep_name in declared_dep_names(module_root).into_iter()
            .chain(declared_dep_names(project_root))
        {
            queue.push_back((dep_name, module_root.to_path_buf()));
        }

        while let Some((dep_name, issuer_root)) = queue.pop_front() {
            if nodes.contains_key(&dep_name) {
                continue;
            }

            let Some(pkg_root) = resolve_dep_dir(&dep_name, &issuer_root, module_root, project_root)
            else {
                // Package not found locally — classify as remote/unknown.
                nodes.insert(
                    dep_name.clone(),
                    DependencyNode {
                        package_name: dep_name,
                        pkg_root: PathBuf::new(),
                        node_type: DependencyNodeType::RemoteOrUnknown,
                        direct_deps: vec![],
                    },
                );
                continue;
            };

            let node_type = classify_package(&pkg_root);
            let direct_deps = declared_dep_names(&pkg_root);

            for child in &direct_deps {
                if !nodes.contains_key(child.as_str()) {
                    queue.push_back((child.clone(), pkg_root.clone()));
                }
            }

            nodes.insert(
                dep_name.clone(),
                DependencyNode {
                    package_name: dep_name,
                    pkg_root,
                    node_type,
                    direct_deps,
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

    /// Return ByteCode HAR nodes.
    pub fn bytecode_hars(&self) -> Vec<&DependencyNode> {
        self.nodes_of_type(DependencyNodeType::BytecodeHar)
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
        byte_code_har: bool,
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
        .map(|p| p.byte_code_har || p.metadata.byte_code_har)
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
    for arm_dir in &["libs/arm64-v8a", "libs/armeabi-v7a"] {
        let dir = pkg_root.join(arm_dir);
        if dir.is_dir() {
            if let Ok(mut entries) = std::fs::read_dir(&dir) {
                if entries.any(|e| {
                    e.ok()
                        .and_then(|entry| entry.path().extension().map(|ext| ext == "so"))
                        .unwrap_or(false)
                }) {
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
    }
    json5::from_str::<PkgDeps>(&content)
        .map(|p| p.dependencies.into_keys().collect())
        .unwrap_or_default()
}

fn resolve_dep_dir(
    dep_name: &str,
    issuer_root: &Path,
    module_root: &Path,
    project_root: &Path,
) -> Option<PathBuf> {
    for base in &[
        issuer_root.join("oh_modules"),
        module_root.join("oh_modules"),
        project_root.join("oh_modules"),
    ] {
        let candidate = base.join(dep_name);
        if candidate.is_dir() {
            return Some(
                std::fs::canonicalize(&candidate).unwrap_or(candidate),
            );
        }
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

    #[test]
    fn test_classify_source_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/my-har");
        write_pkg(&pkg, "my-har", "");
        write_module_json(&pkg, "har");
        fs::write(tmp.path().join("oh-package.json5"), r#"{ "dependencies": { "my-har": "1.0.0" } }"#).unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-har").expect("my-har must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SourceHar);
    }

    #[test]
    fn test_classify_bytecode_har() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("oh_modules/bc-har");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("oh-package.json5"),
            r#"{ "name": "bc-har", "byteCodeHar": true, "dependencies": {} }"#,
        ).unwrap();
        fs::write(pkg.join("ets").to_str().unwrap().to_string() + "/modules.abc", b"PANDA\x00\x00\x00").ok();
        fs::create_dir_all(pkg.join("ets")).unwrap();
        fs::write(pkg.join("ets/modules.abc"), b"PANDA\x00\x00\x00").unwrap();
        fs::write(tmp.path().join("oh-package.json5"), r#"{ "dependencies": { "bc-har": "1.0.0" } }"#).unwrap();

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
        fs::write(tmp.path().join("oh-package.json5"), r#"{ "dependencies": { "my-hsp": "1.0.0" } }"#).unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("my-hsp").expect("my-hsp must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::SharedHsp);
    }

    #[test]
    fn test_classify_remote_or_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        // No oh_modules directory — package not found locally.
        fs::write(tmp.path().join("oh-package.json5"), r#"{ "dependencies": { "remote-pkg": "1.0.0" } }"#).unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let node = graph.nodes.get("remote-pkg").expect("remote-pkg must be in graph");
        assert_eq!(node.node_type, DependencyNodeType::RemoteOrUnknown);
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
        ).unwrap();

        let graph = LocalDependencyGraph::build(tmp.path(), tmp.path());
        let names: Vec<_> = graph.nodes.keys().cloned().collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"], "BFS insertion order must be preserved");
    }
}
