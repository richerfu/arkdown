//! [`DependencyGraphBuilder`] — builds one dependency graph per host module.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::canonicalize_or_owned;

use crate::dependency_graph::snapshot::{
    DependencyEdgeKind, DependencyEdgeSnapshot, DependencyGraphSnapshot, DependencyNodeSnapshot,
    DependencyNodeType,
};
use crate::package::snapshot::PackageSnapshot;
use crate::paths::ModulePathLayout;

/// Builds a dependency graph from package snapshot data.
pub struct DependencyGraphBuilder {
    host_module: String,
    project_root: PathBuf,
    host_root: PathBuf,
}

impl DependencyGraphBuilder {
    pub fn new(
        host_module: impl Into<String>,
        project_root: impl Into<PathBuf>,
        host_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            host_module: host_module.into(),
            project_root: project_root.into(),
            host_root: host_root.into(),
        }
    }

    /// Build the dependency graph snapshot.
    pub fn build(self) -> DependencyGraphSnapshot {
        let mut graph = DependencyGraphSnapshot {
            host_module: self.host_module.clone(),
            ..Default::default()
        };
        self.collect_dependencies(&mut graph);
        graph
    }

    fn collect_dependencies(&self, graph: &mut DependencyGraphSnapshot) {
        let mut queue = VecDeque::new();
        let mut seen_roots = std::collections::BTreeSet::new();
        queue.push_back(self.host_root.clone());
        if self.project_root != self.host_root {
            queue.push_back(self.project_root.clone());
        }

        while let Some(package_root) = queue.pop_front() {
            let Some(package) = self.read_package(&package_root) else {
                continue;
            };
            for (dependencies, edge_kind, is_dynamic) in [
                (
                    &package.package_json.dependencies,
                    DependencyEdgeKind::Dependency,
                    false,
                ),
                (
                    &package.package_json.dev_dependencies,
                    DependencyEdgeKind::DevDependency,
                    false,
                ),
                (
                    &package.package_json.dynamic_dependencies,
                    DependencyEdgeKind::DynamicDependency,
                    true,
                ),
            ] {
                for (alias, spec) in dependencies {
                    let Some(dep_root) = self.resolve_dep_root(&package_root, spec, alias) else {
                        continue;
                    };
                    let dep_package = self.read_package(&dep_root);
                    graph.edges.push(self.edge_snapshot(
                        &package_root,
                        alias,
                        spec,
                        &dep_root,
                        dep_package.as_deref(),
                        edge_kind,
                    ));
                    if !seen_roots.insert(dep_root.clone()) {
                        continue;
                    }
                    let node = self.node_snapshot(
                        alias,
                        spec,
                        &dep_root,
                        dep_package.clone(),
                        false,
                        is_dynamic,
                    );
                    graph.roots_by_alias.insert(alias.clone(), dep_root.clone());
                    graph.nodes.insert(node.package_name.clone(), node);
                    queue.push_back(dep_root);
                }
            }
        }
    }

    fn edge_snapshot(
        &self,
        issuer_root: &Path,
        alias: &str,
        spec: &str,
        root: &Path,
        package: Option<&PackageSnapshot>,
        kind: DependencyEdgeKind,
    ) -> DependencyEdgeSnapshot {
        DependencyEdgeSnapshot {
            issuer_root: issuer_root.to_path_buf(),
            alias: alias.to_string(),
            version_spec: Some(spec.to_string()),
            package_name: self.package_name(alias, package),
            package_root: root.to_path_buf(),
            kind,
        }
    }

    fn read_package(&self, package_root: &Path) -> Option<Arc<PackageSnapshot>> {
        PackageSnapshot::from_root(package_root)
            .ok()
            .flatten()
            .map(Arc::new)
    }

    fn resolve_dep_root(&self, issuer_root: &Path, spec: &str, alias: &str) -> Option<PathBuf> {
        if let Some(file_path) = DependencySpec::new(spec).file_relative_path() {
            let path = issuer_root.join(file_path);
            if path.is_dir() {
                return Some(canonicalize_or_owned(&path));
            }
        }
        let direct_path = issuer_root.join(spec);
        if direct_path.is_dir() {
            return Some(canonicalize_or_owned(&direct_path));
        }
        let mut current = Some(issuer_root);
        while let Some(root) = current {
            let candidate = root.join("oh_modules").join(alias);
            if candidate.is_dir() {
                return Some(canonicalize_or_owned(&candidate));
            }
            if root == self.project_root {
                break;
            }
            current = root.parent();
        }
        let candidate = self.project_root.join("oh_modules").join(alias);
        candidate
            .is_dir()
            .then(|| canonicalize_or_owned(&candidate))
    }

    fn node_snapshot(
        &self,
        alias: &str,
        spec: &str,
        root: &Path,
        package: Option<Arc<PackageSnapshot>>,
        is_transitive: bool,
        is_dynamic: bool,
    ) -> DependencyNodeSnapshot {
        let package_name = self.package_name(alias, package.as_deref());
        let direct_deps = package
            .as_ref()
            .map(|package| self.direct_dependency_aliases(package))
            .unwrap_or_default();
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name,
            version_spec: Some(spec.to_string()),
            package_root: Some(root.to_path_buf()),
            node_type: self.classify_package(root, package.as_deref()),
            package,
            is_runtime_only: false,
            is_dynamic,
            is_transitive,
            direct_deps,
        }
    }

    fn package_name(&self, alias: &str, package: Option<&PackageSnapshot>) -> String {
        package
            .map(|package| package.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| alias.to_string())
    }

    fn classify_package(
        &self,
        root: &Path,
        package: Option<&PackageSnapshot>,
    ) -> DependencyNodeType {
        if package.is_some_and(|package| {
            Path::new(&package.name)
                .extension()
                .is_some_and(|ext| ext == "so")
        }) {
            return DependencyNodeType::NativeSo;
        }
        if package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("isIntegratedHsp"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return DependencyNodeType::IntegratedHsp;
        }
        if ModulePathLayout::new(root)
            .bytecode_har_abc_candidates()
            .iter()
            .any(|path| path.is_file())
        {
            return DependencyNodeType::BytecodeHar;
        }
        match package.and_then(|package| package.package_json.package_type.as_deref()) {
            Some("shared") | Some("hsp") => DependencyNodeType::SharedHsp,
            Some("har") => DependencyNodeType::SourceHar,
            _ => DependencyNodeType::OhpmSource,
        }
    }

    fn direct_dependency_aliases(&self, package: &PackageSnapshot) -> Vec<String> {
        let mut aliases = package
            .package_json
            .dependencies
            .keys()
            .chain(package.package_json.dev_dependencies.keys())
            .chain(package.package_json.dynamic_dependencies.keys())
            .cloned()
            .collect::<Vec<_>>();
        aliases.sort();
        aliases.dedup();
        aliases
    }
}

struct DependencySpec<'a> {
    raw: &'a str,
}

impl<'a> DependencySpec<'a> {
    const FILE_PROTOCOL: &'static str = "file:";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn file_relative_path(&self) -> Option<&'a str> {
        self.raw.strip_prefix(Self::FILE_PROTOCOL)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    struct DependencyGraphFixture {
        temp: tempfile::TempDir,
    }

    impl DependencyGraphFixture {
        fn new() -> Self {
            Self {
                temp: tempfile::tempdir().expect("tempdir"),
            }
        }

        fn root(&self) -> &Path {
            self.temp.path()
        }

        fn write_package(&self, relative: &str, manifest: &str) {
            let root = self.root().join(relative);
            fs::create_dir_all(&root).expect("package root");
            fs::write(root.join("oh-package.json5"), manifest).expect("oh-package");
        }

        fn build_for(&self, host_module: &str) -> DependencyGraphSnapshot {
            DependencyGraphBuilder::new(host_module, self.root(), self.root().join(host_module))
                .build()
        }
    }

    #[test]
    fn collects_project_root_dev_dependencies() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "devDependencies": { "@ohos/hypium": "1.0.11" }
            }"#,
        );
        fixture.write_package("entry", r#"{ "name": "entry", "version": "1.0.0" }"#);
        fixture.write_package(
            "oh_modules/@ohos/hypium",
            r#"{ "name": "@ohos/hypium", "version": "1.0.11", "main": "index.js" }"#,
        );

        let graph = fixture.build_for("entry");

        let node = graph
            .nodes
            .get("@ohos/hypium")
            .expect("root dev dependency node");
        assert_eq!(node.alias, "@ohos/hypium");
        assert_eq!(node.version_spec.as_deref(), Some("1.0.11"));
        assert!(graph.roots_by_alias.contains_key("@ohos/hypium"));
    }

    #[test]
    fn includes_dev_and_dynamic_aliases_in_direct_deps() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "runtime-lib": "1.0.0" },
              "devDependencies": { "dev-lib": "1.0.0" },
              "dynamicDependencies": { "dynamic-lib": "1.0.0" }
            }"#,
        );
        for name in ["runtime-lib", "dev-lib", "dynamic-lib"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }

        let builder =
            DependencyGraphBuilder::new("entry", fixture.root(), fixture.root().join("entry"));
        let package = builder
            .read_package(&fixture.root().join("entry"))
            .expect("entry package");

        assert_eq!(
            builder.direct_dependency_aliases(&package),
            vec!["dev-lib", "dynamic-lib", "runtime-lib"]
        );

        let graph = fixture.build_for("entry");

        assert!(graph.nodes.contains_key("dev-lib"));
        assert!(graph.nodes.contains_key("dynamic-lib"));
        assert!(graph.nodes["dynamic-lib"].is_dynamic);
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| (edge.alias.as_str(), edge.kind))
                .collect::<Vec<_>>(),
            vec![
                ("runtime-lib", DependencyEdgeKind::Dependency),
                ("dev-lib", DependencyEdgeKind::DevDependency),
                ("dynamic-lib", DependencyEdgeKind::DynamicDependency),
            ]
        );
    }

    #[test]
    fn preserves_dependency_edge_declaration_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "dep-b": "1.0.0",
                "dep-a": "1.0.0"
              }
            }"#,
        );
        for name in ["dep-a", "dep-b"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["dep-b", "dep-a"]
        );
    }
}
