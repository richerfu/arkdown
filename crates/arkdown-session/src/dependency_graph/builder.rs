//! [`DependencyGraphBuilder`] — builds one dependency graph per host module.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::OhosModuleType;
use arkdown_path::canonicalize_or_owned;
use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::dependency_graph::snapshot::{
    DependencyEdgeKind, DependencyEdgeSnapshot, DependencyGraphSnapshot,
    DependencyModuleProfileSnapshot, DependencyNodeSnapshot, DependencyNodeType,
    DependencyParameterSnapshot,
};
use crate::package::acquisition::PackageAcquisitionInventory;
use crate::package::snapshot::PackageSnapshot;
use crate::paths::ModulePathLayout;

/// Build-session inventory for dependency package and module-profile snapshots.
///
/// Latest hvigor keeps dependency nodes in its process-level `DEP_NODE` cache
/// and reuses their parsed package/profile objects across module dependency
/// managers. Arkdown keeps the equivalent cache scoped to one immutable build
/// session so later module graphs observe the same startup inputs without
/// re-reading manifests.
#[derive(Debug, Default)]
pub(crate) struct DependencySnapshotInventory {
    packages: FxHashMap<PathBuf, Option<Arc<PackageSnapshot>>>,
    module_profiles: FxHashMap<PathBuf, Option<DependencyModuleProfileSnapshot>>,
    resolved_roots: FxHashMap<(PathBuf, PathBuf, String), Option<PathBuf>>,
    acquired_roots: FxHashMap<(PathBuf, String), PathBuf>,
    canonical_roots: FxHashMap<PathBuf, PathBuf>,
}

#[derive(Clone, Copy)]
struct DependencyTraversalState {
    kind: DependencyEdgeKind,
    is_transitive: bool,
    is_dynamic: bool,
}

#[derive(Clone, Copy)]
enum DependencyPackageManager {
    Ohpm,
    Npm,
}

impl DependencyPackageManager {
    fn from_ohpm_project(use_ohpm: bool) -> Self {
        if use_ohpm {
            Self::Ohpm
        } else {
            Self::Npm
        }
    }

    fn modules_directory(self) -> &'static str {
        match self {
            Self::Ohpm => "oh_modules",
            Self::Npm => "node_modules",
        }
    }

    fn manifest_name(self) -> &'static str {
        match self {
            Self::Ohpm => "oh-package.json5",
            Self::Npm => "package.json",
        }
    }
}

impl DependencySnapshotInventory {
    pub(crate) fn with_acquisitions(acquisitions: &PackageAcquisitionInventory) -> Self {
        Self {
            packages: acquisitions.packages.clone(),
            acquired_roots: acquisitions.resolved_roots.clone(),
            ..Self::default()
        }
    }

    pub(crate) fn insert_package(
        &mut self,
        package_root: &Path,
        package: Option<Arc<PackageSnapshot>>,
    ) {
        self.packages.insert(package_root.to_path_buf(), package);
    }

    fn canonical_root(&mut self, root: &Path) -> PathBuf {
        if let Some(canonical) = self.canonical_roots.get(root) {
            return canonical.clone();
        }
        let canonical = canonicalize_or_owned(root);
        self.canonical_roots
            .insert(root.to_path_buf(), canonical.clone());
        canonical
    }

    fn resolve_package_root(
        &mut self,
        issuer_root: &Path,
        project_root: &Path,
        alias: &str,
        package_manager: DependencyPackageManager,
    ) -> Option<PathBuf> {
        let key = (
            issuer_root.to_path_buf(),
            project_root.to_path_buf(),
            alias.to_string(),
        );
        if let Some(root) = self.resolved_roots.get(&key) {
            return root.clone();
        }

        let canonical_issuer = self.canonical_root(issuer_root);
        if let Some(root) = self
            .acquired_roots
            .get(&(canonical_issuer, alias.to_string()))
            .cloned()
        {
            self.resolved_roots.insert(key, Some(root.clone()));
            return Some(root);
        }

        // Hvigor's `BasePackageResolver.resolvePackagePath()` walks from the
        // issuer to the filesystem root through the package-manager-specific
        // installation directory. The raw version specifier is not a
        // fallback package root.
        let modules_directory = package_manager.modules_directory();
        let mut current = Some(issuer_root);
        let mut resolved = None;
        while let Some(root) = current {
            let search_root = root
                .file_name()
                .filter(|name| {
                    name.to_string_lossy()
                        .eq_ignore_ascii_case(modules_directory)
                })
                .and_then(|_| root.parent())
                .unwrap_or(root);
            let candidate = search_root.join(modules_directory).join(alias);
            if candidate.is_dir() {
                resolved = Some(self.canonical_root(&candidate));
                break;
            }
            current = search_root.parent();
        }
        self.resolved_roots.insert(key, resolved.clone());
        resolved
    }

    fn read_package(
        &mut self,
        package_root: &Path,
        package_manager: DependencyPackageManager,
    ) -> Option<Arc<PackageSnapshot>> {
        if let Some(package) = self.packages.get(package_root) {
            return package.clone();
        }

        let package = PackageSnapshot::from_manifest(
            package_root,
            &package_root.join(package_manager.manifest_name()),
        )
        .ok()
        .flatten()
        .map(Arc::new);
        self.packages
            .insert(package_root.to_path_buf(), package.clone());
        package
    }

    fn read_module_profile(
        &mut self,
        package_root: &Path,
    ) -> Option<DependencyModuleProfileSnapshot> {
        if let Some(profile) = self.module_profiles.get(package_root) {
            return profile.clone();
        }

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

        let profile = ModulePathLayout::new(package_root)
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
            });
        self.module_profiles
            .insert(package_root.to_path_buf(), profile.clone());
        profile
    }
}

/// Builds a dependency graph from package snapshot data.
pub struct DependencyGraphBuilder {
    host_module: String,
    project_root: PathBuf,
    host_root: PathBuf,
    parameter_file_override: Option<PathBuf>,
    dependency_parameters: Option<DependencyParameterSnapshot>,
    package_manager: DependencyPackageManager,
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
            parameter_file_override: None,
            dependency_parameters: None,
            package_manager: DependencyPackageManager::Ohpm,
        }
    }

    pub(crate) fn with_package_manager(mut self, use_ohpm: bool) -> Self {
        self.package_manager = DependencyPackageManager::from_ohpm_project(use_ohpm);
        self
    }

    /// Override the project `parameterFile` with hvigor's `-p parameterFile=`
    /// extra parameter.
    pub fn with_parameter_file_override(mut self, path: PathBuf) -> Self {
        self.parameter_file_override = Some(path);
        self
    }

    /// Reuse the project-level parameter snapshot loaded by
    /// [`crate::ProjectSnapshot`]. This prevents one parameter-file read per
    /// module while retaining the standalone graph-builder API.
    pub fn with_dependency_parameters(
        mut self,
        dependency_parameters: DependencyParameterSnapshot,
    ) -> Self {
        self.dependency_parameters = Some(dependency_parameters);
        self
    }

    /// Build the dependency graph snapshot.
    pub fn build(self) -> DependencyGraphSnapshot {
        let mut inventory = DependencySnapshotInventory::default();
        self.build_with_inventory(&mut inventory)
    }

    /// Build while reusing package/profile snapshots from the current session.
    pub(crate) fn build_with_inventory(
        self,
        inventory: &mut DependencySnapshotInventory,
    ) -> DependencyGraphSnapshot {
        let dependency_parameters = self.dependency_parameters.clone().unwrap_or_else(|| {
            let project_package = inventory.read_package(&self.project_root, self.package_manager);
            match self.package_manager {
                DependencyPackageManager::Ohpm => {
                    DependencyParameterSnapshot::from_project_deferred(
                        &self.project_root,
                        project_package.as_deref(),
                        self.parameter_file_override.as_deref(),
                    )
                }
                DependencyPackageManager::Npm => DependencyParameterSnapshot::default(),
            }
        });
        let mut graph = DependencyGraphSnapshot {
            host_module: self.host_module.clone(),
            host_root: inventory.canonical_root(&self.host_root),
            dependency_parameters,
            ..Default::default()
        };
        self.collect_dependencies(&mut graph, inventory);
        graph
    }

    fn collect_dependencies(
        &self,
        graph: &mut DependencyGraphSnapshot,
        inventory: &mut DependencySnapshotInventory,
    ) {
        let mut queue = VecDeque::new();
        let mut seen_roots = std::collections::BTreeSet::new();
        queue.push_back((self.host_root.clone(), 0usize));
        if self.project_root != self.host_root {
            queue.push_back((self.project_root.clone(), 0usize));
        }

        while let Some((package_root, depth)) = queue.pop_front() {
            let Some(package) = inventory.read_package(&package_root, self.package_manager) else {
                continue;
            };
            for (dependencies, edge_kind, is_dynamic) in [
                (
                    &package.package_json.dependencies,
                    DependencyEdgeKind::Dependency,
                    false,
                ),
                (
                    &package.package_json.dynamic_dependencies,
                    DependencyEdgeKind::DynamicDependency,
                    true,
                ),
                (
                    &package.package_json.dev_dependencies,
                    DependencyEdgeKind::DevDependency,
                    false,
                ),
            ] {
                for (alias, spec) in dependencies {
                    let Some(dep_root) = inventory.resolve_package_root(
                        &package_root,
                        &self.project_root,
                        alias,
                        self.package_manager,
                    ) else {
                        continue;
                    };
                    let dep_package = inventory.read_package(&dep_root, self.package_manager);
                    let traversal = DependencyTraversalState {
                        kind: edge_kind,
                        is_transitive: depth > 0,
                        is_dynamic,
                    };
                    let canonical_issuer_root = inventory.canonical_root(&package_root);
                    let edge = self.edge_snapshot(
                        &canonical_issuer_root,
                        alias,
                        spec,
                        &dep_root,
                        dep_package.as_deref(),
                        traversal,
                    );
                    if !seen_roots.insert(dep_root.clone()) {
                        graph.edges.push(edge);
                        continue;
                    }
                    let node = self.node_snapshot(&edge, dep_package.clone(), traversal, inventory);
                    graph.roots_by_alias.insert(alias.clone(), dep_root.clone());
                    graph.nodes.insert(node.package_name.clone(), node);
                    queue.push_back((dep_root, depth + 1));
                    graph.edges.push(edge);
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
        traversal: DependencyTraversalState,
    ) -> DependencyEdgeSnapshot {
        DependencyEdgeSnapshot {
            // Keep issuer identity in the same canonical domain as host_root
            // and resolved package roots. This is observable on macOS where
            // `/tmp` aliases `/private/tmp`, and avoids equivalent Windows
            // drive/case spellings being misclassified as project globals.
            issuer_root: issuer_root.to_path_buf(),
            alias: alias.to_string(),
            version_spec: Some(spec.to_string()),
            package_name: self.package_name(alias, package),
            package_root: root.to_path_buf(),
            installed_alias_path: self.installed_alias_path(issuer_root, alias, root),
            kind: traversal.kind,
            is_transitive: traversal.is_transitive,
        }
    }

    fn installed_alias_path(
        &self,
        issuer_root: &Path,
        alias: &str,
        root: &Path,
    ) -> Option<PathBuf> {
        let modules_directory = self.package_manager.modules_directory();
        let mut current = Some(issuer_root);
        while let Some(issuer) = current {
            let search_root = issuer
                .file_name()
                .filter(|name| {
                    name.to_string_lossy()
                        .eq_ignore_ascii_case(modules_directory)
                })
                .and_then(|_| issuer.parent())
                .unwrap_or(issuer);
            let candidate = search_root.join(modules_directory).join(alias);
            if std::fs::symlink_metadata(&candidate)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
                && canonicalize_or_owned(&candidate) == canonicalize_or_owned(root)
            {
                return Some(candidate);
            }
            current = search_root.parent();
        }
        None
    }

    fn node_snapshot(
        &self,
        edge: &DependencyEdgeSnapshot,
        package: Option<Arc<PackageSnapshot>>,
        traversal: DependencyTraversalState,
        inventory: &mut DependencySnapshotInventory,
    ) -> DependencyNodeSnapshot {
        let alias = edge.alias.as_str();
        let spec = edge.version_spec.as_deref().unwrap_or_default();
        let root = &edge.package_root;
        let package_name = self.package_name(alias, package.as_deref());
        let module_profile = inventory.read_module_profile(root);
        let direct_deps = package
            .as_ref()
            .map(|package| self.direct_dependency_aliases(package))
            .unwrap_or_default();
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name,
            version_spec: Some(spec.to_string()),
            package_root: Some(root.to_path_buf()),
            node_type: self.classify_package(
                package.as_deref(),
                module_profile.as_ref(),
                self.package_manager,
            ),
            package,
            module_profile,
            is_runtime_only: false,
            is_dynamic: traversal.is_dynamic,
            is_transitive: traversal.is_transitive,
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
        package_manager: DependencyPackageManager,
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
            .and_then(|metadata| metadata.get("integratedHsp"))
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
        // npm packages are Harmony dependencies only when their package.json
        // carries the `ohos` marker. Without it Hvigor keeps the dependency as
        // an ordinary npm node even if a coincidental module profile exists.
        if matches!(package_manager, DependencyPackageManager::Npm)
            && !package.is_some_and(|package| package.package_json.extra.contains_key("ohos"))
        {
            return DependencyNodeType::OhpmSource;
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

    fn direct_dependency_aliases(&self, package: &PackageSnapshot) -> Vec<String> {
        let mut aliases = package
            .package_json
            .dependencies
            .keys()
            .chain(package.package_json.dynamic_dependencies.keys())
            .chain(package.package_json.dev_dependencies.keys())
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
    use crate::package::acquisition::{AcquisitionMode, PackageAcquisitionPlanner};

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
    fn transitive_package_resolves_without_redundant_project_declaration() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "example",
            r#"{
              "name": "example",
              "version": "1.0.0",
              "dependencies": { "@msi/jsi": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "example/oh_modules/@msi/jsi",
            r#"{
              "name": "@msi/jsi",
              "version": "1.0.0",
              "dependencies": { "@msi/jsi_interface": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "example/oh_modules/@msi/jsi/oh_modules/@msi/jsi_interface",
            r#"{
              "name": "@msi/jsi_interface",
              "version": "1.0.0",
              "main": "index.ets"
            }"#,
        );

        let graph = fixture.build_for("example");
        let interface = graph
            .nodes
            .get("@msi/jsi_interface")
            .expect("transitive interface package");
        let expected_root = canonicalize_or_owned(
            &fixture
                .root()
                .join("example/oh_modules/@msi/jsi/oh_modules/@msi/jsi_interface"),
        );

        assert!(interface.is_transitive);
        assert_eq!(
            interface.package_root.as_deref(),
            Some(expected_root.as_path())
        );
        assert_eq!(
            graph.roots_by_alias.get("@msi/jsi_interface"),
            Some(&expected_root)
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.alias == "@msi/jsi_interface"
                && edge.issuer_root
                    == canonicalize_or_owned(&fixture.root().join("example/oh_modules/@msi/jsi"))
        }));
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
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
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
                ("dynamic-lib", DependencyEdgeKind::DynamicDependency),
                ("dev-lib", DependencyEdgeKind::DevDependency),
            ]
        );
    }

    #[test]
    fn shared_inventory_reuses_hvigor_dep_node_package_and_profile_snapshots() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        for module in ["first", "second"] {
            fixture.write_package(
                module,
                r#"{
                  "name": "host",
                  "version": "1.0.0",
                  "dependencies": { "shared-har": "1.0.0" }
                }"#,
            );
        }
        fixture.write_package(
            "oh_modules/shared-har",
            r#"{ "name": "shared-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "oh_modules/shared-har",
            "module.json5",
            r#"{ "module": { "name": "shared", "type": "har" } }"#,
        );

        let mut inventory = DependencySnapshotInventory::default();
        let first =
            DependencyGraphBuilder::new("first", fixture.root(), fixture.root().join("first"))
                .build_with_inventory(&mut inventory);

        fixture.write_package(
            "oh_modules/shared-har",
            r#"{ "name": "changed-after-snapshot", "version": "2.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "oh_modules/shared-har",
            "module.json5",
            r#"{ "module": { "name": "changed", "type": "shared" } }"#,
        );

        let second =
            DependencyGraphBuilder::new("second", fixture.root(), fixture.root().join("second"))
                .build_with_inventory(&mut inventory);

        for graph in [&first, &second] {
            let node = graph.nodes.get("shared-har").expect("cached dependency");
            assert_eq!(node.package.as_ref().unwrap().version, "1.0.0");
            assert_eq!(node.node_type, DependencyNodeType::SourceHar);
            assert_eq!(
                node.module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.as_deref()),
                Some("shared")
            );
        }
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

    #[cfg(unix)]
    #[test]
    fn captures_installed_symlink_alias_once_in_the_session_graph() {
        use std::os::unix::fs::symlink;

        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "linked": "file:../packages/linked" }
            }"#,
        );
        fixture.write_package(
            "packages/linked",
            r#"{ "name": "linked", "version": "1.0.0" }"#,
        );
        let modules = fixture.root().join("entry/oh_modules");
        fs::create_dir_all(&modules).expect("oh_modules");
        symlink("../../packages/linked", modules.join("linked")).expect("package symlink");

        let graph = fixture.build_for("entry");
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.alias == "linked")
            .expect("linked dependency edge");

        assert_eq!(
            edge.installed_alias_path.as_deref(),
            Some(graph.host_root.join("oh_modules/linked").as_path())
        );
        assert_eq!(
            edge.package_root,
            canonicalize_or_owned(&fixture.root().join("packages/linked"))
        );
        let linked_change = graph.host_root.join("oh_modules/linked/src/Index.ets");
        assert_eq!(
            graph.changed_package_names_for_paths([linked_change.as_path()]),
            Some(vec!["linked".to_string()])
        );
        let project_change = fixture.root().join("entry/src/main/ets/Index.ets");
        assert_eq!(
            graph.changed_package_names_for_paths([project_change.as_path()]),
            None
        );
    }

    #[test]
    fn startup_acquisition_feeds_local_path_roots_into_the_frozen_graph() {
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

        let acquisitions = PackageAcquisitionPlanner::new(
            fixture.root(),
            [fixture.root().join("entry")],
            AcquisitionMode::Offline,
        )
        .acquire()
        .expect("resolve startup packages");
        let mut inventory = DependencySnapshotInventory::with_acquisitions(&acquisitions);
        let graph =
            DependencyGraphBuilder::new("entry", fixture.root(), fixture.root().join("entry"))
                .build_with_inventory(&mut inventory);

        let dependency = graph
            .nodes
            .get("native-types.so")
            .expect("acquired package must enter dependency graph");
        assert_eq!(dependency.node_type, DependencyNodeType::NativeSo);
        assert_eq!(
            dependency.package_root,
            Some(canonicalize_or_owned(
                &fixture.root().join("entry/types/native")
            ))
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
    fn bytecode_har_ohm_view_matches_latest_hvigor_tree_barriers_and_section_order() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "source-har": "1.0.0",
                "direct-byte": "1.0.0",
                "direct-other": "1.0.0",
                "direct-hsp": "1.0.0"
              },
              "dynamicDependencies": { "dynamic-other": "1.0.0" },
              "devDependencies": { "dev-other": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "dependencies": { "nested-byte": "1.0.0" }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "source", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-byte",
            r#"{ "name": "direct-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other",
            r#"{
              "name": "direct-other",
              "version": "1.0.0",
              "dependencies": { "hidden-byte": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-hsp",
            r#"{ "name": "direct-hsp", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/direct-hsp",
            "module.json",
            r#"{ "module": { "name": "shared", "type": "shared" } }"#,
        );
        for name in ["dynamic-other", "dev-other"] {
            fixture.write_package(
                &format!("entry/oh_modules/{name}"),
                &format!(r#"{{ "name": "{name}", "version": "1.0.0" }}"#),
            );
        }
        fixture.write_package(
            "entry/oh_modules/source-har/oh_modules/nested-byte",
            r#"{ "name": "nested-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other/oh_modules/hidden-byte",
            r#"{ "name": "hidden-byte", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );

        let graph = fixture.build_for("entry");
        let names = |bundled, optimized| {
            graph
                .bytecode_har_ohm_nodes_in_traversal_order(bundled, optimized)
                .into_iter()
                .map(|node| node.package_name.as_str())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            names(true, true),
            [
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
                "nested-byte",
            ]
        );
        assert_eq!(
            names(true, false),
            [
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
                "nested-byte",
                "hidden-byte",
            ]
        );
        assert_eq!(
            names(false, true),
            [
                "source-har",
                "direct-byte",
                "direct-other",
                "dynamic-other",
                "dev-other",
            ]
        );
        assert_eq!(
            names(false, false),
            ["source-har", "direct-byte", "direct-other", "dynamic-other",]
        );
    }

    #[test]
    fn bytecode_har_dependency_versions_match_direct_and_bundled_hvigor_views() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "direct-other": "2.0.0",
                "direct-har": "1.0.0",
                "shared": "3.0.0"
              }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other",
            r#"{
              "name": "direct-other",
              "version": "2.0.0",
              "dependencies": { "nested-other": "4.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-other/oh_modules/nested-other",
            r#"{ "name": "nested-other", "version": "4.0.0" }"#,
        );
        fixture.write_package(
            "entry/oh_modules/direct-har",
            r#"{ "name": "direct-har", "version": "1.0.0" }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/direct-har",
            "module.json",
            r#"{ "module": { "name": "directhar", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/shared",
            r#"{ "name": "shared", "version": "3.0.0", "metadata": { "integratedHsp": true } }"#,
        );

        let graph = fixture.build_for("entry");
        assert_eq!(
            graph
                .bytecode_har_dependency_versions(
                    false,
                    false,
                    &Default::default(),
                    &Default::default(),
                )
                .expect("direct dependency versions")
                .into_iter()
                .collect::<Vec<_>>(),
            [
                ("direct-other".to_string(), "2.0.0".to_string()),
                ("direct-har".to_string(), "1.0.0".to_string()),
            ]
        );

        let compiled = ["direct-har".to_string()].into_iter().collect();
        assert_eq!(
            graph
                .bytecode_har_dependency_versions(true, false, &compiled, &Default::default(),)
                .expect("bundled dependency versions")
                .into_iter()
                .collect::<Vec<_>>(),
            [
                ("direct-other".to_string(), "2.0.0".to_string()),
                ("nested-other".to_string(), "4.0.0".to_string()),
            ]
        );
    }

    #[test]
    fn classifies_latest_metadata_integrated_hsp_key() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "shared": "1.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/shared",
            r#"{
              "name": "shared",
              "version": "1.0.0",
              "metadata": { "integratedHsp": true }
            }"#,
        );

        let graph = fixture.build_for("entry");

        assert_eq!(
            graph.nodes["shared"].node_type,
            DependencyNodeType::IntegratedHsp
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

    #[test]
    fn bundled_har_importees_and_resources_follow_process_har_artifacts() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "source-har": "1.0.0", "plain-pkg": "2.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "metadata": { "resource": { "directories": ["src/main/custom"] } }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "sourcehar", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/plain-pkg",
            r#"{ "name": "plain-pkg", "version": "2.0.0" }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["source-har".into(), "plain-pkg/feature".into()],
            &["source-har".into(), "plain-pkg/feature".into()],
            true,
            true,
            &[],
        );

        assert_eq!(metadata.compiled_package_names, ["source-har"]);
        assert_eq!(metadata.extra_importees, ["plain-pkg/feature"]);
        assert!(metadata
            .affects_compilation_dependency_keys
            .contains(&"source-har".to_string()));
        let resources = graph.bundled_har_resources();
        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0].metadata_directory.as_str(),
            "src/main/custom-source-har"
        );
        assert_eq!(
            resources[0].archive_directory.as_str(),
            "src/main/custom-source-har"
        );
    }

    #[test]
    fn optimized_importees_mark_project_root_direct_dependencies_global() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "dependencies": { "global-lib": "1.0.0" }
            }"#,
        );
        fixture.write_package("entry", r#"{ "name": "entry", "version": "1.0.0" }"#);
        fixture.write_package(
            "oh_modules/global-lib",
            r#"{ "name": "global-lib", "version": "1.0.0" }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["global-lib".into()],
            &["global-lib".into()],
            true,
            true,
            &[],
        );

        assert_eq!(metadata.global_dependencies, ["global-lib"]);
    }

    #[test]
    fn bundled_bytecode_har_keep_dependency_false_merges_then_filters_like_hvigor() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package("", r#"{ "name": "app", "version": "1.0.0" }"#);
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": {
                "source-har": "1.0.0",
                "byte-har": "1.0.0"
              },
              "devDependencies": { "dev-source": "1.0.0" },
              "dynamicDependencies": {
                "dyn-source": "1.0.0",
                "dyn-byte": "1.0.0"
              }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/source-har",
            r#"{
              "name": "source-har",
              "version": "1.0.0",
              "dependencies": { "byte-har": "2.0.0" }
            }"#,
        );
        fixture.write_dependency_profile(
            "entry/oh_modules/source-har",
            "module.json",
            r#"{ "module": { "name": "source-har", "type": "har" } }"#,
        );
        fixture.write_package(
            "entry/oh_modules/byte-har",
            r#"{
              "name": "byte-har",
              "version": "1.0.0",
              "metadata": { "byteCodeHar": true }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dev-source",
            r#"{ "name": "dev-source", "version": "1.0.0" }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dyn-source",
            r#"{
              "name": "dyn-source",
              "version": "1.0.0",
              "dynamicDependencies": { "dyn-byte": "2.0.0" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/dyn-byte",
            r#"{
              "name": "dyn-byte",
              "version": "1.0.0",
              "metadata": { "byteCodeHar": true }
            }"#,
        );

        let graph = fixture.build_for("entry");
        let metadata = graph.har_importee_metadata(
            &[],
            &["source-har".into()],
            &["source-har".into()],
            true,
            true,
            &[],
        );
        assert_eq!(metadata.compiled_package_names, ["source-har"]);
        assert!(metadata.global_dependencies.is_empty());
        let versions = graph
            .bytecode_har_dependency_versions(
                true,
                true,
                &metadata.compiled_package_names.iter().cloned().collect(),
                &metadata
                    .affects_compilation_dependency_keys
                    .iter()
                    .cloned()
                    .collect(),
            )
            .expect("collect optimized bundled dependency versions");
        assert!(versions.is_empty());
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
            .expect("entry package");
        let retained = graph
            .bundled_har_retained_dependencies(&package.package_json)
            .expect("rewrite bundled dependencies");

        assert_eq!(
            retained.dependencies.into_iter().collect::<Vec<_>>(),
            vec![("byte-har".to_string(), "2.0.0".to_string())]
        );
        assert_eq!(
            retained
                .dynamic_dependencies
                .into_iter()
                .collect::<Vec<_>>(),
            vec![("dyn-byte".to_string(), "2.0.0".to_string())]
        );
    }

    #[test]
    fn bytecode_har_dependency_versions_resolve_project_parameter_file() {
        let fixture = DependencyGraphFixture::new();
        fixture.write_package(
            "",
            r#"{
              "name": "app",
              "version": "1.0.0",
              "parameterFile": "${paths.parameters}",
              "properties": { "paths": { "parameters": "ohpm-params.json5" } }
            }"#,
        );
        fs::write(
            fixture.root().join("ohpm-params.json5"),
            r#"{ "versions.lib": "^1.2.3" }"#,
        )
        .expect("parameter file");
        fixture.write_package(
            "entry",
            r#"{
              "name": "entry",
              "version": "1.0.0",
              "dependencies": { "lib": "@param:versions.lib" }
            }"#,
        );
        fixture.write_package(
            "entry/oh_modules/lib",
            r#"{ "name": "lib", "version": "1.2.3" }"#,
        );

        let graph = fixture.build_for("entry");
        let versions = graph
            .bytecode_har_dependency_versions(
                false,
                false,
                &Default::default(),
                &Default::default(),
            )
            .expect("resolve parameterized dependency version");

        assert_eq!(versions["lib"], "^1.2.3");
        let package = PackageSnapshot::from_root(&fixture.root().join("entry"))
            .expect("read entry package")
            .expect("entry package");
        let resolved = graph
            .dependency_parameters
            .resolve_package_parameters(&package.package_json)
            .expect("resolve generated package parameters");
        assert_eq!(resolved.dependencies["lib"], "^1.2.3");
    }
}
