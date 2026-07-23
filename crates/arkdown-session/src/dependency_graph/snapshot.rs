//! [`DependencyGraphSnapshot`] — immutable per-host-module dependency graph.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::package::snapshot::PackageSnapshot;

/// Stable module/profile identity loaded from an installed dependency.
///
/// Hvigor exposes these fields through `DependencyInterface.getModuleJsonObj()`
/// to `GeneratePkgContextInfo` and `ProcessProfile`. Keeping them on the
/// session node prevents those stages from re-reading dependency profiles.
#[derive(Debug, Clone, Default)]
pub struct DependencyModuleProfileSnapshot {
    /// `module.name` from `src/main/module.json[5]`.
    pub module_name: Option<String>,
    /// `module.type` (or legacy `module.distro.moduleType`).
    pub module_type: Option<arkdown_core::OhosModuleType>,
    /// `app.bundleName` for common-app remote HSP packages.
    pub bundle_name: Option<String>,
    /// `app.bundleType` for common-app remote HSP packages.
    pub bundle_type: Option<String>,
    /// `app.versionCode` when it is embedded in a packaged HSP profile.
    pub version_code: Option<u64>,
}

/// Classification of a dependency node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyNodeType {
    /// Source HAR (bundled into the output).
    SourceHar,
    /// Bytecode HAR (pre-compiled `.abc` included as-is).
    BytecodeHar,
    /// Integrated HSP (shared package bundled into host).
    IntegratedHsp,
    /// External shared HSP (deployed separately on device).
    SharedHsp,
    /// Native shared library (`.so` file).
    NativeSo,
    /// Regular OHPM source package.
    OhpmSource,
    /// Unknown or unresolved package.
    Unknown,
}

/// Classification of a declared dependency edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyEdgeKind {
    /// `dependencies` entry from `oh-package.json5`.
    Dependency,
    /// `devDependencies` entry from `oh-package.json5`.
    DevDependency,
    /// `dynamicDependencies` entry from `oh-package.json5`.
    DynamicDependency,
}

/// A declared dependency edge in traversal order.
#[derive(Debug, Clone)]
pub struct DependencyEdgeSnapshot {
    /// Package root that declares the dependency.
    pub issuer_root: PathBuf,
    /// Alias (key in `oh-package.json5` dependencies).
    pub alias: String,
    /// Raw dependency specifier from the declaring `oh-package.json5`.
    pub version_spec: Option<String>,
    /// Resolved package name.
    pub package_name: String,
    /// Resolved package root directory.
    pub package_root: PathBuf,
    /// Declared dependency section.
    pub kind: DependencyEdgeKind,
}

/// A node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNodeSnapshot {
    /// Alias (key in `oh-package.json5` dependencies).
    pub alias: String,
    /// Resolved package name.
    pub package_name: String,
    /// Raw dependency specifier from the declaring `oh-package.json5`.
    pub version_spec: Option<String>,
    /// Resolved package root directory.
    pub package_root: Option<PathBuf>,
    /// Parsed package metadata.
    pub package: Option<Arc<PackageSnapshot>>,
    /// Parsed dependency module/profile identity, when the package ships one.
    pub module_profile: Option<DependencyModuleProfileSnapshot>,
    /// Dependency node classification.
    pub node_type: DependencyNodeType,
    /// Whether this is a runtime-only (not bundled) dependency.
    pub is_runtime_only: bool,
    /// Whether this is a dynamic import dependency.
    pub is_dynamic: bool,
    /// Whether this is a transitive (indirect) dependency.
    pub is_transitive: bool,
    /// Direct dependency aliases declared by this package.
    pub direct_deps: Vec<String>,
}

/// One per-host-module immutable dependency graph.
///
/// Consumers:
/// - share config merge
/// - startup config merge
/// - loader JSON `modulePathMap`, `hspNameOhmMap`, `harNameOhmMap`
/// - bytecode HAR scan
/// - integrated HSP packaging
/// - resolver package root lookup
/// - module meta package lookup
/// - CompileArkTS package metadata
#[derive(Debug, Clone, Default)]
pub struct DependencyGraphSnapshot {
    /// The host module name.
    pub host_module: String,
    /// Nodes indexed by package name.
    pub nodes: BTreeMap<String, DependencyNodeSnapshot>,
    /// Declared dependency edges in graph traversal order.
    pub edges: Vec<DependencyEdgeSnapshot>,
    /// Package roots by alias.
    pub roots_by_alias: BTreeMap<String, PathBuf>,
}

impl DependencyGraphSnapshot {
    /// Return an iterator over all HAR nodes (source and bytecode).
    pub fn har_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes.values().filter(|n| {
            matches!(
                n.node_type,
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar
            )
        })
    }

    /// Return an iterator over bytecode HAR nodes.
    pub fn bytecode_har_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, DependencyNodeType::BytecodeHar))
    }

    /// Return ByteCode HAR nodes in declared dependency traversal order.
    ///
    /// Hvigor's `DependencyManager` preserves manifest insertion order when
    /// `GenerateLoaderJson.processByteCodeHar` builds its maps. The node index
    /// is a `BTreeMap` for lookup stability, so artifact writers must recover
    /// semantic order from the edge inventory instead of serializing key order.
    pub fn bytecode_har_nodes_in_traversal_order(&self) -> Vec<&DependencyNodeSnapshot> {
        let mut seen = std::collections::BTreeSet::new();
        self.edges
            .iter()
            .filter_map(|edge| {
                let node = self.nodes.get(&edge.package_name)?;
                (matches!(node.node_type, DependencyNodeType::BytecodeHar)
                    && seen.insert(node.package_name.as_str()))
                .then_some(node)
            })
            .collect()
    }

    /// Return an iterator over integrated HSP nodes.
    pub fn integrated_hsp_nodes(&self) -> impl Iterator<Item = &DependencyNodeSnapshot> {
        self.nodes
            .values()
            .filter(|n| matches!(n.node_type, DependencyNodeType::IntegratedHsp))
    }

    /// Return `true` if a path is inside this graph's `oh_modules` ownership.
    ///
    /// Uses the `roots_by_alias` map; no substring matching.
    pub fn owns_path(&self, path: &std::path::Path) -> bool {
        self.roots_by_alias
            .values()
            .any(|root| path.starts_with(root))
    }
}
