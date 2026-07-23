//! [`DependencyGraphBuilder`] — builds one dependency graph per host module.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::OhosModuleType;
use arkdown_path::canonicalize_or_owned;
use serde::Deserialize;

use crate::dependency_graph::snapshot::{
    DependencyEdgeKind, DependencyEdgeSnapshot, DependencyGraphSnapshot,
    DependencyModuleProfileSnapshot, DependencyNodeSnapshot, DependencyNodeType,
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
        queue.push_back((self.host_root.clone(), 0usize));
        if self.project_root != self.host_root {
            queue.push_back((self.project_root.clone(), 0usize));
        }

        while let Some((package_root, depth)) = queue.pop_front() {
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
                        depth > 0,
                        is_dynamic,
                    );
                    graph.roots_by_alias.insert(alias.clone(), dep_root.clone());
                    graph.nodes.insert(node.package_name.clone(), node);
                    queue.push_back((dep_root, depth + 1));
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

    fn resolve_dep_root(&self, issuer_root: &Path, _spec: &str, alias: &str) -> Option<PathBuf> {
        // Hvigor's `BasePackageResolver.resolvePackagePath()` deliberately
        // resolves the dependency alias through the installed `oh_modules`
        // hierarchy. The raw version specifier (including `file:`) is not a
        // fallback package root: an absent alias link means the dependency is
        // not installed and must not enter the dependency graph.
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
        let module_profile = self.read_dependency_module_profile(root);
        let direct_deps = package
            .as_ref()
            .map(|package| self.direct_dependency_aliases(package))
            .unwrap_or_default();
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name,
            version_spec: Some(spec.to_string()),
            package_root: Some(root.to_path_buf()),
            node_type: self.classify_package(package.as_deref(), module_profile.as_ref()),
            package,
            module_profile,
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
        package: Option<&PackageSnapshot>,
        module_profile: Option<&DependencyModuleProfileSnapshot>,
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
        if package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("byteCodeHar"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return DependencyNodeType::BytecodeHar;
        }
        // Mirrors hvigor-ohos-plugin's
        // `DependencyManager.getDependencyTypeByProfile`: packaged source HARs
        // normally declare their kind in src/main/module.json(.5), not in
        // oh-package.json5. Legacy profiles carry the same value under
        // module.distro.moduleType.
        if let Some(module_type) = module_profile.and_then(|profile| profile.module_type.as_ref()) {
            return match module_type {
                OhosModuleType::Har => DependencyNodeType::SourceHar,
                OhosModuleType::Shared => DependencyNodeType::SharedHsp,
                OhosModuleType::Entry | OhosModuleType::Feature => DependencyNodeType::OhpmSource,
            };
        }
        match package.and_then(|package| package.package_json.package_type.as_deref()) {
            Some("shared") | Some("hsp") => DependencyNodeType::SharedHsp,
            Some("har") => DependencyNodeType::SourceHar,
            _ => DependencyNodeType::OhpmSource,
        }
    }

    fn read_dependency_module_profile(
        &self,
        root: &Path,
    ) -> Option<DependencyModuleProfileSnapshot> {
        #[derive(Deserialize)]
        struct DependencyProfile {
            #[serde(default)]
            app: Option<DependencyProfileApp>,
            module: DependencyProfileModule,
        }

        #[derive(Deserialize)]
        struct DependencyProfileModule {
            #[serde(default)]
            name: Option<String>,
            #[serde(rename = "type", default)]
            module_type: Option<String>,
            #[serde(default)]
            distro: Option<DependencyProfileDistro>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileDistro {
            #[serde(default)]
            module_type: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DependencyProfileApp {
            #[serde(default)]
            bundle_name: Option<String>,
            #[serde(default)]
            bundle_type: Option<String>,
            #[serde(default)]
            version_code: Option<u64>,
        }

        ModulePathLayout::new(root)
            .dependency_profile_candidates()
            .into_iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .filter_map(|content| json5::from_str::<DependencyProfile>(&content).ok())
            .next()
            .map(|profile| {
                let app = profile.app;
                DependencyModuleProfileSnapshot {
                    module_name: profile.module.name,
                    module_type: profile
                        .module
                        .module_type
                        .or_else(|| profile.module.distro.and_then(|distro| distro.module_type))
                        .map(|module_type| OhosModuleType::from_type_str(&module_type)),
                    bundle_name: app.as_ref().and_then(|app| app.bundle_name.clone()),
                    bundle_type: app.as_ref().and_then(|app| app.bundle_type.clone()),
                    version_code: app.and_then(|app| app.version_code),
                }
            })
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

        fn write_dependency_profile(&self, relative: &str, file_name: &str, profile: &str) {
            let src_main = self.root().join(relative).join("src/main");
            fs::create_dir_all(&src_main).expect("dependency src/main");
            fs::write(src_main.join(file_name), profile).expect("dependency profile");
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
    fn marks_only_nested_dependencies_as_transitive() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "direct-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har",
            r#"{
              "name": "direct-har",
              "version": "1.0.0",
              "dependencies": { "nested-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har/oh_modules/nested-har",
            r#"{ "name": "nested-har", "version": "1.0.0" }"#,
        );

        let graph = fixture.build_for("entry");

        assert!(!graph.nodes["direct-har"].is_transitive);
        assert!(graph.nodes["nested-har"].is_transitive);
    }

    #[test]
    fn file_dependency_requires_installed_oh_modules_alias_like_hvigor() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "native-types.so": "file:./types/native" }
            }"#,
        );
        fixture.write_package(
            "entry/types/native",
            r#"{ "name": "native-types.so", "version": "1.0.0", "types": "Index.d.ts" }"#,
        );

        let graph_without_install = fixture.build_for("entry");
        assert!(
            !graph_without_install.nodes.contains_key("native-types.so"),
            "a declared file path is not an installed OHPM package alias"
        );

        fixture.write_package(
            "entry/oh_modules/native-types.so",
            r#"{ "name": "native-types.so", "version": "1.0.0", "types": "Index.d.ts" }"#,
        );
        let graph_with_install = fixture.build_for("entry");
        assert!(graph_with_install.nodes.contains_key("native-types.so"));
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

    #[test]
    fn bytecode_har_artifact_view_preserves_manifest_traversal_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "z-bytecode": "1.0.0",
                "a-bytecode": "1.0.0"
              }
            }"#,
        );
        for name in ["a-bytecode", "z-bytecode"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(
                    r#"{{ "name": "{name}", "version": "1.0.0", "metadata": {{ "byteCodeHar": true }} }}"#
                ),
            );
        }

        let graph = fixture.build_for("entry");
        assert_eq!(
            graph
                .bytecode_har_nodes_in_traversal_order()
                .into_iter()
                .map(|node| node.package_name.as_str())
                .collect::<Vec<_>>(),
            ["z-bytecode", "a-bytecode"]
        );
    }

    #[test]
    fn classifies_source_har_from_packaged_module_json() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{ "name": "source-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "sourcehar", "type": "har" } }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["source-har"].node_type,
            DependencyNodeType::SourceHar
        );
    }

    #[test]
    fn classifies_legacy_har_from_distro_module_type() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "legacy-har": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/legacy-har",
            r#"{ "name": "legacy-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/legacy-har",
            "config.json",
            r#"{ "module": { "distro": { "moduleType": "har" } } }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["legacy-har"].node_type,
            DependencyNodeType::SourceHar
        );
    }
}
