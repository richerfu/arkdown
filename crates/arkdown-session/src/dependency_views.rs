//! Specialized dependency graph views for domain crates.
//!
//! Each view is a typed projection over [`DependencyGraphSnapshot`] that
//! mirrors one of the `TaskService.get*Dependencies` families in hvigor's
//! `src/project/dependency/module-dependency-info.js`.
//!
//! # Upstream correspondence
//!
//! | Rust type                  | hvigor method                                  |
//! |----------------------------|------------------------------------------------|
//! | [`HarDependencyView`]      | `TaskService.getRemoteHarDependencies`          |
//! | [`HspDependencyView`]      | `TaskService.getOhpmRemoteHspDependencies`      |
//! | [`ByteCodeHarDependencyView`] | `TaskService.getHspOrByteCodeHarDependencies` |
//! | [`RemoteHspDependencyView`]| `TaskService.getHspRecursion`                  |
//! | [`NativeSoDependencyView`] | `TaskService.getSODependencies`                |
//! | [`DynamicDependencyView`]  | `TaskService.getDynamicDependencies`            |

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};

// ---------------------------------------------------------------------------
// HarDependencyView
// ---------------------------------------------------------------------------

/// Typed view over HAR (source and bytecode) dependencies for a host module.
///
/// Upstream: `TaskService.getRemoteHarDependencies` and
/// `getDependencyName2RootPath` / `getDependencyName2DepInfo` in
/// `module-dependency-info.js`.
#[derive(Debug, Clone)]
pub struct HarDependencyView {
    /// All HAR nodes (source + bytecode), keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl HarDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: `TaskService.getRemoteHarDependencies`.
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| {
                matches!(
                    n.node_type,
                    DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
                )
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over remote (OHPM-sourced) HAR dependency nodes.
    ///
    /// Upstream: `TaskService.getRemoteHarDependencies`.
    pub fn remote_hars(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }

    /// Map from dependency alias → resolved package root path.
    ///
    /// Upstream: `TaskService.getDependencyName2RootPath`.
    pub fn dependency_name_to_root_path(&self) -> BTreeMap<&str, &PathBuf> {
        self.nodes
            .values()
            .filter_map(|n| n.package_root.as_ref().map(|root| (n.alias.as_str(), root)))
            .collect()
    }

    /// Map from dependency alias → full node info.
    ///
    /// Upstream: `TaskService.getDependencyName2DepInfo`.
    pub fn dependency_name_to_dep_info(&self) -> &BTreeMap<String, DependencyNodeSnapshot> {
        &self.nodes
    }
}

// ---------------------------------------------------------------------------
// HspDependencyView
// ---------------------------------------------------------------------------

/// Typed view over HSP (integrated and shared) dependencies for a host module.
///
/// Upstream: `TaskService.getOhpmRemoteHspDependencies` and
/// `getHspRecursion` in `module-dependency-info.js`.
#[derive(Debug, Clone)]
pub struct HspDependencyView {
    /// All HSP nodes, keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl HspDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: `TaskService.getOhpmRemoteHspDependencies`.
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| {
                matches!(
                    n.node_type,
                    DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
                )
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over remote (OHPM-sourced) HSP dependency nodes.
    ///
    /// Upstream: `TaskService.getOhpmRemoteHspDependencies`.
    pub fn remote_hsps(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }

    /// Return the transitive HSP recursion set (all transitive HSP packages).
    ///
    /// Upstream: `TaskService.getHspRecursion`.
    pub fn hsp_recursion(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values().filter(|n| n.is_transitive)
    }

    /// Iterate over HSP or bytecode-HAR dependencies combined.
    ///
    /// Upstream: `TaskService.getHspOrByteCodeHarDependencies`.
    pub fn hsp_or_bytecode_har_dependencies<'a>(
        &'a self,
        bytecode_view: &'a ByteCodeHarDependencyView,
    ) -> impl Iterator<Item = &'a DependencyNodeSnapshot> {
        self.nodes.values().chain(bytecode_view.nodes.values())
    }

    /// Map from dependency alias → resolved package root path.
    ///
    /// Upstream: `TaskService.getDependencyName2RootPath`.
    pub fn dependency_name_to_root_path(&self) -> BTreeMap<&str, &PathBuf> {
        self.nodes
            .values()
            .filter_map(|n| n.package_root.as_ref().map(|root| (n.alias.as_str(), root)))
            .collect()
    }

    /// Map from dependency alias → full node info.
    ///
    /// Upstream: `TaskService.getDependencyName2DepInfo`.
    pub fn dependency_name_to_dep_info(&self) -> &BTreeMap<String, DependencyNodeSnapshot> {
        &self.nodes
    }
}

// ---------------------------------------------------------------------------
// ByteCodeHarDependencyView
// ---------------------------------------------------------------------------

/// Typed view over bytecode-only HAR dependencies.
///
/// Bytecode HARs contain pre-compiled `.abc` files and are included as-is
/// rather than being re-compiled.
///
/// Upstream: `TaskService.getHspOrByteCodeHarDependencies` (bytecode half).
#[derive(Debug, Clone)]
pub struct ByteCodeHarDependencyView {
    /// Bytecode HAR nodes, keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl ByteCodeHarDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: bytecode-HAR subset of `getHspOrByteCodeHarDependencies`.
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n.node_type, DependencyNodeType::BytecodeHar))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over all bytecode HAR dependency nodes.
    pub fn blocked_dependencies(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }
}

// ---------------------------------------------------------------------------
// RemoteHspDependencyView
// ---------------------------------------------------------------------------

/// Typed view over remote (OHPM-published) shared HSP packages.
///
/// Remote HSPs are deployed separately on device; they are not bundled into
/// the host HAP/HSP artifact.
///
/// Upstream: `TaskService.getHspRecursion` (shared HSP subset).
#[derive(Debug, Clone)]
pub struct RemoteHspDependencyView {
    /// Shared HSP nodes, keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl RemoteHspDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: `TaskService.getOhpmRemoteHspDependencies` (shared subset).
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n.node_type, DependencyNodeType::SharedHsp))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over all remote shared HSP nodes.
    pub fn remote_hsps(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }

    /// Return the transitive recursion set for remote HSPs.
    ///
    /// Upstream: `TaskService.getHspRecursion`.
    pub fn hsp_recursion(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values().filter(|n| n.is_transitive)
    }

    /// Map from dependency alias → resolved package root path.
    ///
    /// Upstream: `TaskService.getDependencyName2RootPath`.
    pub fn dependency_name_to_root_path(&self) -> BTreeMap<&str, &PathBuf> {
        self.nodes
            .values()
            .filter_map(|n| n.package_root.as_ref().map(|root| (n.alias.as_str(), root)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// NativeSoDependencyView
// ---------------------------------------------------------------------------

/// Typed view over native shared library (`.so`) dependencies.
///
/// Upstream: `TaskService.getSODependencies` in
/// `module-dependency-info.js`.
#[derive(Debug, Clone)]
pub struct NativeSoDependencyView {
    /// Native SO nodes, keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl NativeSoDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: `TaskService.getSODependencies`.
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n.node_type, DependencyNodeType::NativeSo))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over all native SO dependency nodes.
    ///
    /// Upstream: `TaskService.getSODependencies`.
    pub fn so_dependencies(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }

    /// Map from dependency alias → resolved package root path.
    ///
    /// Upstream: `TaskService.getDependencyName2RootPath`.
    pub fn dependency_name_to_root_path(&self) -> BTreeMap<&str, &PathBuf> {
        self.nodes
            .values()
            .filter_map(|n| n.package_root.as_ref().map(|root| (n.alias.as_str(), root)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DynamicDependencyView
// ---------------------------------------------------------------------------

/// Typed view over dynamic import dependencies.
///
/// Dynamic dependencies are loaded at runtime via `import()` expressions and
/// may be HSP or HAR packages.
///
/// Upstream: `TaskService.getDynamicDependencies` in
/// `module-dependency-info.js`.
#[derive(Debug, Clone)]
pub struct DynamicDependencyView {
    /// Dynamic dependency nodes, keyed by package name.
    nodes: BTreeMap<String, DependencyNodeSnapshot>,
}

impl DynamicDependencyView {
    /// Build the view from a dependency graph snapshot.
    ///
    /// Upstream: `TaskService.getDynamicDependencies`.
    pub fn from_graph(graph: &DependencyGraphSnapshot) -> Self {
        let nodes = graph
            .nodes
            .iter()
            .filter(|(_, n)| n.is_dynamic)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { nodes }
    }

    /// Iterate over all dynamic dependency nodes.
    ///
    /// Upstream: `TaskService.getDynamicDependencies`.
    pub fn dynamic_dependencies(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values()
    }

    /// Map from dependency alias → resolved package root path.
    ///
    /// Upstream: `TaskService.getDependencyName2RootPath`.
    pub fn dependency_name_to_root_path(&self) -> BTreeMap<&str, &PathBuf> {
        self.nodes
            .values()
            .filter_map(|n| n.package_root.as_ref().map(|root| (n.alias.as_str(), root)))
            .collect()
    }

    /// Map from dependency alias → full node info.
    ///
    /// Upstream: `TaskService.getDependencyName2DepInfo`.
    pub fn dependency_name_to_dep_info(&self) -> &BTreeMap<String, DependencyNodeSnapshot> {
        &self.nodes
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::dependency_graph::snapshot::{
        DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
    };

    fn make_node(
        alias: &str,
        node_type: DependencyNodeType,
        is_dynamic: bool,
        is_transitive: bool,
    ) -> DependencyNodeSnapshot {
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name: alias.to_string(),
            version_spec: None,
            package_root: Some(PathBuf::from(format!("/oh_modules/{alias}"))),
            package: None,
            node_type,
            is_runtime_only: false,
            is_dynamic,
            is_transitive,
            direct_deps: vec![],
        }
    }

    fn make_graph() -> DependencyGraphSnapshot {
        let mut g = DependencyGraphSnapshot::default();
        g.nodes.insert(
            "har_a".to_string(),
            make_node("har_a", DependencyNodeType::SourceHar, false, false),
        );
        g.nodes.insert(
            "bhar_b".to_string(),
            make_node("bhar_b", DependencyNodeType::BytecodeHar, false, false),
        );
        g.nodes.insert(
            "hsp_c".to_string(),
            make_node("hsp_c", DependencyNodeType::SharedHsp, false, true),
        );
        g.nodes.insert(
            "ihsp_d".to_string(),
            make_node("ihsp_d", DependencyNodeType::IntegratedHsp, false, false),
        );
        g.nodes.insert(
            "so_e".to_string(),
            make_node("so_e", DependencyNodeType::NativeSo, false, false),
        );
        g.nodes.insert(
            "dyn_f".to_string(),
            make_node("dyn_f", DependencyNodeType::SourceHar, true, false),
        );
        g
    }

    #[test]
    fn har_view_contains_source_and_bytecode() {
        let g = make_graph();
        let view = HarDependencyView::from_graph(&g);
        let names: Vec<_> = view.remote_hars().map(|n| n.alias.as_str()).collect();
        assert!(names.contains(&"har_a"), "expected har_a");
        assert!(names.contains(&"bhar_b"), "expected bhar_b");
        assert!(!names.contains(&"hsp_c"), "should not include hsp");
    }

    #[test]
    fn hsp_view_contains_integrated_and_shared() {
        let g = make_graph();
        let view = HspDependencyView::from_graph(&g);
        let names: Vec<_> = view.remote_hsps().map(|n| n.alias.as_str()).collect();
        assert!(names.contains(&"hsp_c"));
        assert!(names.contains(&"ihsp_d"));
        assert!(!names.contains(&"har_a"));
    }

    #[test]
    fn hsp_recursion_filters_transitive() {
        let g = make_graph();
        let view = HspDependencyView::from_graph(&g);
        let transitive: Vec<_> = view.hsp_recursion().map(|n| n.alias.as_str()).collect();
        assert_eq!(transitive, vec!["hsp_c"]);
    }

    #[test]
    fn hsp_or_bytecode_har_combines() {
        let g = make_graph();
        let hsp_view = HspDependencyView::from_graph(&g);
        let bc_view = ByteCodeHarDependencyView::from_graph(&g);
        let combined: Vec<_> = hsp_view
            .hsp_or_bytecode_har_dependencies(&bc_view)
            .map(|n| n.alias.as_str())
            .collect();
        assert!(combined.contains(&"bhar_b"));
        assert!(combined.contains(&"hsp_c"));
        assert!(combined.contains(&"ihsp_d"));
    }

    #[test]
    fn bytecode_har_view_only_bytecode() {
        let g = make_graph();
        let view = ByteCodeHarDependencyView::from_graph(&g);
        let names: Vec<_> = view
            .blocked_dependencies()
            .map(|n| n.alias.as_str())
            .collect();
        assert_eq!(names, vec!["bhar_b"]);
    }

    #[test]
    fn native_so_view() {
        let g = make_graph();
        let view = NativeSoDependencyView::from_graph(&g);
        let names: Vec<_> = view.so_dependencies().map(|n| n.alias.as_str()).collect();
        assert_eq!(names, vec!["so_e"]);
    }

    #[test]
    fn dynamic_view() {
        let g = make_graph();
        let view = DynamicDependencyView::from_graph(&g);
        let names: Vec<_> = view
            .dynamic_dependencies()
            .map(|n| n.alias.as_str())
            .collect();
        assert_eq!(names, vec!["dyn_f"]);
    }

    #[test]
    fn remote_hsp_view_only_shared() {
        let g = make_graph();
        let view = RemoteHspDependencyView::from_graph(&g);
        let names: Vec<_> = view.remote_hsps().map(|n| n.alias.as_str()).collect();
        assert_eq!(names, vec!["hsp_c"]);
    }

    #[test]
    fn dependency_name_to_root_path_works() {
        let g = make_graph();
        let view = HarDependencyView::from_graph(&g);
        let map = view.dependency_name_to_root_path();
        assert_eq!(map.get("har_a"), Some(&&PathBuf::from("/oh_modules/har_a")));
    }
}
