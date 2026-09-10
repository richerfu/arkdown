use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_core::OhosModuleType;
use arkdown_path::canonicalize_or_owned;
use arkdown_session::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::PackageSnapshot;
use indexmap::IndexSet;

pub(crate) struct PkgSdkInfoInput<'a> {
    pub dependency_graph: &'a DependencyGraphSnapshot,
    pub project_root: &'a Path,
    pub module_package: &'a PackageSnapshot,
    pub module_type: &'a OhosModuleType,
    pub use_normalized_ohm_url: bool,
    pub output_path: &'a Path,
}

/// Owns the legacy package inventory consumed by the SDK packing tool.
///
/// Current Hvigor `GeneratePkgSdkInfo` emits this file only when normalized
/// OHM URLs are disabled. It traverses the module dependency tree first and
/// then the project tree, stops below HSP nodes, records direct roots, and
/// finally appends the current module package.
pub(crate) struct PkgSdkInfoGenerator<'a> {
    input: PkgSdkInfoInput<'a>,
}

impl<'a> PkgSdkInfoGenerator<'a> {
    pub fn new(input: PkgSdkInfoInput<'a>) -> Self {
        Self { input }
    }

    pub fn write(self) -> Result<Option<PathBuf>> {
        if self.input.output_path.is_file() {
            fs_err::remove_file(self.input.output_path).with_context(|| {
                format!(
                    "remove stale pkgSdkInfo.json {}",
                    self.input.output_path.display()
                )
            })?;
        }
        if self.input.use_normalized_ohm_url {
            return Ok(None);
        }

        let direct_packages = self.direct_package_names();
        let mut seen_aliases = IndexSet::new();
        let mut entries = serde_json::Map::new();
        for root in self.traversal_roots() {
            self.collect_tree(&root, &direct_packages, &mut seen_aliases, &mut entries)?;
        }
        entries.insert(
            self.input.module_package.name.clone(),
            serde_json::json!({
                "version": self.input.module_package.version,
                "type": self.input.module_type.to_string(),
                "isDirectDep": false
            }),
        );

        if let Some(parent) = self.input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(
            self.input.output_path,
            serde_json::to_vec(&serde_json::Value::Object(entries))?,
        )?;
        Ok(Some(self.input.output_path.to_path_buf()))
    }

    fn traversal_roots(&self) -> Vec<PathBuf> {
        let project_root = canonicalize_or_owned(self.input.project_root);
        let mut roots = vec![self.input.dependency_graph.host_root.clone()];
        if project_root != self.input.dependency_graph.host_root {
            roots.push(project_root);
        }
        roots
    }

    fn direct_package_names(&self) -> BTreeSet<String> {
        let project_root = canonicalize_or_owned(self.input.project_root);
        self.input
            .dependency_graph
            .edges
            .iter()
            .filter(|edge| {
                edge.issuer_root == self.input.dependency_graph.host_root
                    || (edge.issuer_root == project_root
                        && edge.package_root != self.input.dependency_graph.host_root)
            })
            .map(|edge| edge.package_name.clone())
            .collect()
    }

    fn collect_tree(
        &self,
        root: &Path,
        direct_packages: &BTreeSet<String>,
        seen_aliases: &mut IndexSet<String>,
        entries: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        let mut queue = VecDeque::from([root.to_path_buf()]);
        let mut visited_issuers = BTreeSet::new();
        while let Some(issuer_root) = queue.pop_front() {
            if !visited_issuers.insert(issuer_root.clone()) {
                continue;
            }
            for edge in self
                .input
                .dependency_graph
                .edges
                .iter()
                .filter(|edge| edge.issuer_root == issuer_root)
            {
                let Some(node) = self.input.dependency_graph.nodes.get(&edge.package_name) else {
                    continue;
                };
                if seen_aliases.insert(edge.alias.clone()) {
                    self.insert_dependency(node, direct_packages, entries)?;
                }
                if !Self::is_hsp(node) {
                    queue.push_back(edge.package_root.clone());
                }
            }
        }
        Ok(())
    }

    fn insert_dependency(
        &self,
        node: &DependencyNodeSnapshot,
        direct_packages: &BTreeSet<String>,
        entries: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        let version = node
            .package
            .as_deref()
            .map(|package| package.version.as_str())
            .unwrap_or("");
        let version = self
            .input
            .dependency_graph
            .dependency_parameters
            .resolve_version(version)?;
        entries.insert(
            node.package_name.clone(),
            serde_json::json!({
                "version": version,
                "type": Self::dependency_type(node.node_type),
                "isDirectDep": direct_packages.contains(&node.package_name)
            }),
        );
        Ok(())
    }

    fn dependency_type(node_type: DependencyNodeType) -> &'static str {
        match node_type {
            DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => "har",
            DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp => "shared",
            DependencyNodeType::NativeSo => "so",
            DependencyNodeType::OhpmSource | DependencyNodeType::Unknown => "other",
        }
    }

    fn is_hsp(node: &DependencyNodeSnapshot) -> bool {
        matches!(
            node.node_type,
            DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
        )
    }
}
