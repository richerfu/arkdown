//! Narrow plan input for the filesystem watch stage.

use std::path::{Path, PathBuf};

use crate::dependency_graph::snapshot::DependencyNodeType;
use crate::paths::ProjectPathLayout;
use crate::project::snapshot::ProjectSnapshot;

// ---------------------------------------------------------------------------
// WatchPlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the filesystem watch stage.
///
/// Contains the watch roots and glob patterns needed by
/// `arkdown-watcher::WatchPlanner` to set up incremental rebuild triggers.
///
/// # Upstream correspondence
///
/// Corresponds to the watch configuration assembled from module source roots,
/// resource directories, and config files in hvigor's watch mode.
#[derive(Debug, Clone)]
pub struct WatchPlanInput {
    /// Source root directories to watch for `.ets`/`.ts` changes.
    pub source_roots: Vec<PathBuf>,
    /// Resource directories to watch for resource changes.
    pub resource_roots: Vec<PathBuf>,
    /// Config files to watch for invalidation (e.g., `module.json5`).
    pub config_files: Vec<PathBuf>,
    /// OHOS module name.
    pub module_name: String,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl WatchPlanInput {
    /// Create from pre-resolved values with a default debounce of 300 ms.
    pub fn from_context_input(
        module_name: impl Into<String>,
        source_roots: Vec<PathBuf>,
        resource_roots: Vec<PathBuf>,
        config_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            source_roots,
            resource_roots,
            config_files,
            module_name: module_name.into(),
            debounce_ms: 300,
        }
    }

    /// Build the complete watch inventory from startup-owned project snapshots.
    ///
    /// Source HAR inputs and their resources are included because hvigor's
    /// Rollup watch graph observes source dependencies as compilation inputs.
    /// Configuration files are recorded explicitly instead of recursively
    /// watching the project root and observing generated build output.
    pub fn from_project(project: &ProjectSnapshot, custom_config_path: Option<&Path>) -> Vec<Self> {
        project
            .module_order
            .iter()
            .filter_map(|module_name| {
                let module = project.modules.get(module_name)?;
                let mut source_roots = module.source_roots.clone();
                let mut resource_roots = module.resource_roots.clone();
                let mut config_files = Self::project_config_files(project);
                if let Some(path) = custom_config_path {
                    config_files.push(path.to_path_buf());
                }

                Self::push_file(&mut config_files, module.module_json_path.as_ref());
                Self::push_file(&mut config_files, module.package_path.as_ref());
                Self::push_existing_candidates(
                    &mut config_files,
                    &module.root,
                    &[
                        "build-profile.json5",
                        "hvigorfile.ts",
                        "hvigorfile.js",
                        "hvigorfile.cjs",
                        "hvigorfile.mjs",
                    ],
                );

                if let Some(graph) = project.dependency_graphs.get(module_name) {
                    for node in graph.nodes.values() {
                        let Some(package_root) = node.package_root.as_ref() else {
                            continue;
                        };
                        Self::push_existing_candidates(
                            &mut config_files,
                            package_root,
                            &["oh-package.json5", "src/main/module.json5"],
                        );
                        if matches!(
                            node.node_type,
                            DependencyNodeType::SourceHar | DependencyNodeType::OhpmSource
                        ) {
                            Self::push_dir(
                                &mut source_roots,
                                package_root.join("src").join("main"),
                            );
                        }
                        if matches!(
                            node.node_type,
                            DependencyNodeType::SourceHar
                                | DependencyNodeType::SharedHsp
                                | DependencyNodeType::IntegratedHsp
                        ) {
                            Self::push_dir(
                                &mut resource_roots,
                                package_root.join("src").join("main").join("resources"),
                            );
                        }
                    }
                }

                source_roots.sort();
                source_roots.dedup();
                resource_roots.sort();
                resource_roots.dedup();
                config_files.sort();
                config_files.dedup();

                Some(Self {
                    source_roots,
                    resource_roots,
                    config_files,
                    module_name: module_name.clone(),
                    // hvigor-arkts-compose/arkts-pack.ts::getWatchConfig.
                    debounce_ms: 100,
                })
            })
            .collect()
    }

    fn project_config_files(project: &ProjectSnapshot) -> Vec<PathBuf> {
        let layout = ProjectPathLayout::new(&project.root);
        let mut files = Vec::new();
        Self::push_existing_candidates(
            &mut files,
            &project.root,
            &[
                "build-profile.json5",
                "oh-package.json5",
                "oh-package-lock.json5",
                "hvigorfile.ts",
                "hvigorfile.js",
                "hvigorfile.cjs",
                "hvigorfile.mjs",
                "hvigorconfig.ts",
            ],
        );
        let app_json = layout.app_scope_app_json5();
        if app_json.is_file() {
            files.push(app_json);
        }
        files
    }

    fn push_existing_candidates(output: &mut Vec<PathBuf>, root: &std::path::Path, paths: &[&str]) {
        for path in paths {
            let candidate = root.join(path);
            if candidate.is_file() {
                output.push(candidate);
            }
        }
    }

    fn push_file(output: &mut Vec<PathBuf>, path: Option<&PathBuf>) {
        if let Some(path) = path {
            output.push(path.clone());
        }
    }

    fn push_dir(output: &mut Vec<PathBuf>, path: PathBuf) {
        if path.is_dir() {
            output.push(path);
        }
    }
}
