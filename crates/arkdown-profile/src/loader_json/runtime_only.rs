use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
use rustc_hash::FxHashSet;

use super::to_absolute_path_string;
use crate::layout::ProfileModuleLayout;

pub(super) struct RuntimeOnlyCollector<'a> {
    config: &'a ArkdownConfig,
    module_cfg: &'a ModuleConfig,
    graph: &'a arkdown_session::DependencyGraphSnapshot,
}

impl<'a> RuntimeOnlyCollector<'a> {
    pub(super) fn new(
        config: &'a ArkdownConfig,
        module_cfg: &'a ModuleConfig,
        graph: &'a arkdown_session::DependencyGraphSnapshot,
    ) -> Self {
        Self {
            config,
            module_cfg,
            graph,
        }
    }

    pub(super) fn collect_dynamic_import_dep_names(&self) -> Vec<String> {
        let mut names = self
            .graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind
                    == arkdown_session::dependency_graph::snapshot::DependencyEdgeKind::DynamicDependency
                    && edge.issuer_root == self.graph.host_root
            })
            .map(|edge| edge.alias.clone())
            .collect::<Vec<_>>();
        names.extend(
            self.read_module_runtime_only(&self.module_cfg.src_path)
                .packages,
        );
        if let Some(runtime_only) = self
            .config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|ark_options| ark_options.runtime_only.as_ref())
        {
            names.extend(runtime_only.packages.iter().cloned());
        }
        names.extend(self.graph.edges.iter().filter_map(|edge| {
            self.graph
                .nodes
                .get(&edge.package_name)
                .filter(|node| node.is_runtime_only)
                .map(|_| edge.alias.clone())
        }));
        let mut seen = FxHashSet::default();
        names.retain(|name| seen.insert(name.clone()));
        names
    }

    pub(super) fn discover_worker_files(&self) -> Vec<PathBuf> {
        let workers_dir = ProfileModuleLayout::new(&self.module_cfg.src_path).workers_source_root();
        if !workers_dir.is_dir() {
            return vec![];
        }

        self.collect_dynamic_import_source_files(&workers_dir)
    }

    pub(super) fn collect_source_files(&self) -> Vec<PathBuf> {
        let module_runtime_only = self.read_module_runtime_only(&self.module_cfg.src_path);
        let mut sources: Vec<String> = module_runtime_only.sources;
        sources.extend(
            self.config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|ark_options| ark_options.runtime_only.as_ref())
                .map(|runtime_only| runtime_only.sources.clone())
                .unwrap_or_default(),
        );

        let mut files = Vec::new();
        for source in &sources {
            self.collect_source_config_entry(&self.module_cfg.src_path, source, &mut files);
        }

        // D3: dependency runtime-only metadata follows the immutable graph's
        // declared BFS order. Package manifests were parsed at session startup.
        let mut seen_roots = FxHashSet::default();
        for edge in &self.graph.edges {
            if edge.kind
                != arkdown_session::dependency_graph::snapshot::DependencyEdgeKind::Dependency
                || edge.version_spec.is_none()
                || !seen_roots.insert(edge.package_root.clone())
            {
                continue;
            }
            let Some(node) = self.graph.nodes.get(&edge.package_name) else {
                continue;
            };
            for source in Self::dependency_runtime_only_sources(node) {
                self.collect_source_config_entry(&edge.package_root, &source, &mut files);
            }
        }

        let mut seen = FxHashSet::default();
        files.retain(|path| seen.insert(ArtifactPathSerializer::rel_to_slash(path)));
        files
    }

    pub(super) fn collect_package_source_files(&self) -> Vec<PathBuf> {
        let module_runtime_only = self.read_module_runtime_only(&self.module_cfg.src_path);
        let mut packages: Vec<String> = module_runtime_only.packages;
        packages.extend(
            self.config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|ark_options| ark_options.runtime_only.as_ref())
                .map(|runtime_only| runtime_only.packages.clone())
                .unwrap_or_default(),
        );

        packages.extend(self.graph.edges.iter().filter_map(|edge| {
            self.graph
                .nodes
                .get(&edge.package_name)
                .filter(|node| node.is_runtime_only)
                .map(|_| edge.alias.clone())
        }));

        let mut files = Vec::new();
        let mut visited: FxHashSet<String> = FxHashSet::default();
        for package in packages {
            if !visited.insert(package.clone()) {
                continue;
            }
            let Some(node) = self.dependency_node(&package) else {
                continue;
            };
            let Some(package_root) = node.package_root.as_ref() else {
                continue;
            };
            if !self.should_exclude_remote_har(&package, node) {
                files.extend(self.collect_package_source_files_from(package_root));
            }
        }
        self.dedupe_paths(&mut files);
        files
    }

    pub(super) fn dedupe_paths(&self, files: &mut Vec<PathBuf>) {
        let mut seen = FxHashSet::default();
        files.retain(|path| seen.insert(to_absolute_path_string(path)));
    }

    fn read_module_runtime_only(&self, module_root: &Path) -> RuntimeOnlyLists {
        let path = module_root.join("build-profile.json5");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return RuntimeOnlyLists::default();
        };

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleBuildProfile {
            #[serde(default)]
            build_option: ModuleBuildOption,
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleBuildOption {
            #[serde(default)]
            ark_options: ModuleArkOptions,
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleArkOptions {
            #[serde(default)]
            runtime_only: ModuleRuntimeOnly,
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleRuntimeOnly {
            #[serde(default)]
            packages: Vec<String>,
            #[serde(default)]
            sources: Vec<String>,
        }

        json5::from_str::<ModuleBuildProfile>(&content)
            .map(|profile| RuntimeOnlyLists {
                packages: profile.build_option.ark_options.runtime_only.packages,
                sources: profile.build_option.ark_options.runtime_only.sources,
            })
            .unwrap_or_default()
    }

    fn collect_package_source_files_from(&self, pkg_dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for stem in ["Index", "index"] {
            for ext in ["ets", "ts", "js"] {
                let candidate = pkg_dir.join(Path::new(stem).with_extension(ext));
                if candidate.is_file() {
                    files.push(candidate);
                }
            }
        }

        let ets_root = ProfileModuleLayout::new(pkg_dir).ets_source_root();
        if ets_root.is_dir() {
            files.extend(self.collect_dynamic_import_source_files(&ets_root));
        }
        files
    }

    fn dependency_node(
        &self,
        package_name_or_alias: &str,
    ) -> Option<&arkdown_session::dependency_graph::snapshot::DependencyNodeSnapshot> {
        self.graph
            .edges
            .iter()
            .find(|edge| {
                edge.alias == package_name_or_alias || edge.package_name == package_name_or_alias
            })
            .and_then(|edge| self.graph.nodes.get(&edge.package_name))
    }

    fn dependency_runtime_only_sources(
        node: &arkdown_session::dependency_graph::snapshot::DependencyNodeSnapshot,
    ) -> Vec<String> {
        node.package
            .as_ref()
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("runtimeOnly"))
            .and_then(|runtime_only| runtime_only.get("sources"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect()
    }

    /// Mirrors
    /// `exclude-remote-dynamic-import.ts#isNeedToExcludeHar`: an
    /// `excludePackages` entry suppresses source collection only for remote
    /// source HARs. Local project modules and non-HAR packages remain compile
    /// inputs.
    fn should_exclude_remote_har(
        &self,
        package_name: &str,
        node: &arkdown_session::dependency_graph::snapshot::DependencyNodeSnapshot,
    ) -> bool {
        let excluded = self
            .config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.runtime_only.as_ref())
            .is_some_and(|runtime_only| {
                runtime_only
                    .exclude_packages
                    .iter()
                    .any(|excluded| excluded == package_name)
            });
        excluded
            && node.node_type
                == arkdown_session::dependency_graph::snapshot::DependencyNodeType::SourceHar
            && !self.is_local_dependency(package_name, node)
    }

    fn is_local_dependency(
        &self,
        package_name: &str,
        node: &arkdown_session::dependency_graph::snapshot::DependencyNodeSnapshot,
    ) -> bool {
        let Some(package_root) = node.package_root.as_ref() else {
            return false;
        };
        let canonical_root = canonicalize_or_owned(package_root);
        if self
            .config
            .modules
            .iter()
            .any(|module| canonicalize_or_owned(&module.src_path) == canonical_root)
        {
            return true;
        }
        self.graph.edges.iter().any(|edge| {
            (edge.alias == package_name || edge.package_root == *package_root)
                && edge
                    .version_spec
                    .as_deref()
                    .is_some_and(|specifier| specifier.starts_with("file:"))
        })
    }

    fn collect_dynamic_import_source_files(&self, root: &Path) -> Vec<PathBuf> {
        let mut directories = VecDeque::from([root.to_path_buf()]);
        let mut files = Vec::new();
        while let Some(dir) = directories.pop_front() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entries = entries.flatten().collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    directories.push_back(path);
                    continue;
                }
                if self.is_dynamic_import_source_file(&path) {
                    files.push(path);
                }
            }
        }
        files
    }

    fn collect_source_config_entry(&self, base_dir: &Path, source: &str, files: &mut Vec<PathBuf>) {
        let path = Path::new(source);
        let source_path = if path.has_root() {
            path.to_path_buf()
        } else {
            base_dir.join(path)
        };

        if source_path.is_file() {
            if self.is_dynamic_import_source_file(&source_path) {
                files.push(source_path);
            }
            return;
        }

        if source_path.is_dir() {
            files.extend(self.collect_dynamic_import_source_files(&source_path));
        }
    }

    fn is_dynamic_import_source_file(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ets" | "ts" | "js")
        )
    }
}

#[derive(Default)]
struct RuntimeOnlyLists {
    packages: Vec<String>,
    sources: Vec<String>,
}
