use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_path::ArtifactPathSerializer;
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::to_absolute_path_string;
use crate::layout::ProfileModuleLayout;

pub(super) struct RuntimeOnlyCollector<'a> {
    config: &'a ArkdownConfig,
    module_cfg: &'a ModuleConfig,
}

impl<'a> RuntimeOnlyCollector<'a> {
    pub(super) fn new(config: &'a ArkdownConfig, module_cfg: &'a ModuleConfig) -> Self {
        Self { config, module_cfg }
    }

    pub(super) fn collect_dynamic_import_dep_names(&self) -> Vec<String> {
        let mut names = self.read_dynamic_dep_names(&self.module_cfg.src_path);
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
        names.extend(self.read_dependency_runtime_only_package_names());
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

        // D3: also collect sources from dependency metadata.runtimeOnly.sources.
        // Matches hvigor's processIndirectDependency() over the dependency graph.
        for pkg_dir in self.collect_declared_dependency_roots() {
            let rt = self.read_pkg_runtime_only(&pkg_dir);
            for source in &rt.sources {
                self.collect_source_config_entry(&pkg_dir, source, &mut files);
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

        // D3: also include runtimeOnly.packages from dependency metadata
        // (matches hvigor's processIndirectDependency() over the dependency graph).
        for pkg_dir in self.collect_declared_dependency_roots() {
            packages.extend(self.read_pkg_runtime_only_packages(&pkg_dir));
        }

        let mut files = Vec::new();
        let mut visited: FxHashSet<String> = FxHashSet::default();
        // BFS over runtimeOnly package list (handles transitive packages).
        let mut queue: VecDeque<String> = packages.into_iter().collect();
        while let Some(package) = queue.pop_front() {
            if !visited.insert(package.clone()) {
                continue;
            }
            let Some(pkg_dir) = self.resolve_oh_module_dir(&package) else {
                continue;
            };
            files.extend(self.collect_package_source_files_from(&pkg_dir));
            // Collect transitive runtimeOnly.packages from this dep.
            queue.extend(self.read_pkg_runtime_only_packages(&pkg_dir));
        }
        self.dedupe_paths(&mut files);
        files
    }

    pub(super) fn dedupe_paths(&self, files: &mut Vec<PathBuf>) {
        let mut seen = FxHashSet::default();
        files.retain(|path| seen.insert(to_absolute_path_string(path)));
    }

    /// Read `dynamicDependencies` package names from `oh-package.json5`.
    ///
    /// Returns an empty vec if the file is absent or unparseable.
    fn read_dynamic_dep_names(&self, module_root: &Path) -> Vec<String> {
        let path = module_root.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return vec![];
        };

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgDynDeps {
            #[serde(default)]
            dynamic_dependencies: FxHashMap<String, String>,
        }

        json5::from_str::<PkgDynDeps>(&content)
            .map(|p| p.dynamic_dependencies.into_keys().collect())
            .unwrap_or_default()
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

    fn read_dependency_runtime_only_package_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for pkg_dir in self.collect_declared_dependency_roots() {
            names.extend(self.read_pkg_runtime_only_packages(&pkg_dir));
        }
        names
    }

    fn read_package_dep_names(&self, module_root: &Path) -> Vec<String> {
        let path = module_root.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return vec![];
        };

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgDeps {
            #[serde(default)]
            dependencies: IndexMap<String, String>,
        }

        json5::from_str::<PkgDeps>(&content)
            .map(|p| p.dependencies.into_keys().collect())
            .unwrap_or_default()
    }

    fn read_pkg_runtime_only(&self, pkg_dir: &Path) -> PkgRuntimeOnly {
        let path = pkg_dir.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return PkgRuntimeOnly {
                packages: vec![],
                sources: vec![],
            };
        };

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgJson {
            #[serde(default)]
            metadata: PkgMetadata,
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgMetadata {
            #[serde(default)]
            runtime_only: PkgRuntimeOnlyConfig,
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgRuntimeOnlyConfig {
            #[serde(default)]
            packages: Vec<String>,
            #[serde(default)]
            sources: Vec<String>,
        }

        json5::from_str::<PkgJson>(&content)
            .map(|p| PkgRuntimeOnly {
                packages: p.metadata.runtime_only.packages,
                sources: p.metadata.runtime_only.sources,
            })
            .unwrap_or(PkgRuntimeOnly {
                packages: vec![],
                sources: vec![],
            })
    }

    fn read_pkg_runtime_only_packages(&self, pkg_dir: &Path) -> Vec<String> {
        self.read_pkg_runtime_only(pkg_dir).packages
    }

    fn resolve_oh_module_dir(&self, dep_name: &str) -> Option<PathBuf> {
        for base in &[
            self.module_cfg.src_path.join("oh_modules"),
            self.config.project_root.join("oh_modules"),
        ] {
            let candidate = base.join(dep_name);
            if candidate.exists() && candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    fn collect_declared_dependency_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut visited: FxHashSet<String> = FxHashSet::default();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        queue.push_back(self.module_cfg.src_path.to_path_buf());
        if self.config.project_root != self.module_cfg.src_path {
            queue.push_back(self.config.project_root.to_path_buf());
        }

        while let Some(pkg_root) = queue.pop_front() {
            for dep_name in self.read_package_dep_names(&pkg_root) {
                let Some(dep_root) = self.resolve_declared_dep_dir(&dep_name, &pkg_root) else {
                    continue;
                };

                if !visited.insert(to_absolute_path_string(&dep_root)) {
                    continue;
                }

                roots.push(dep_root.clone());
                queue.push_back(dep_root);
            }
        }

        roots
    }

    fn resolve_declared_dep_dir(&self, dep_name: &str, issuer_root: &Path) -> Option<PathBuf> {
        for base in [
            issuer_root.join("oh_modules"),
            self.module_cfg.src_path.join("oh_modules"),
            self.config.project_root.join("oh_modules"),
        ] {
            let candidate = base.join(dep_name);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
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
        files.sort_by_key(|path| {
            path.strip_prefix(pkg_dir)
                .map(ArtifactPathSerializer::rel_to_slash)
                .unwrap_or_else(|_| ArtifactPathSerializer::rel_to_slash(path))
        });
        files
    }

    fn collect_dynamic_import_source_files(&self, root: &Path) -> Vec<PathBuf> {
        let mut stack = vec![root.to_path_buf()];
        let mut files = Vec::new();
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if self.is_dynamic_import_source_file(&path) {
                    files.push(path);
                }
            }
        }

        files.sort_by_key(|path| ArtifactPathSerializer::rel_to_slash(path));
        files
    }

    fn collect_source_config_entry(&self, base_dir: &Path, source: &str, files: &mut Vec<PathBuf>) {
        let path = Path::new(source);
        let source_path = if path.is_absolute() {
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

/// Combined runtimeOnly config from a package's `oh-package.json5#metadata.runtimeOnly`.
struct PkgRuntimeOnly {
    packages: Vec<String>,
    sources: Vec<String>,
}
