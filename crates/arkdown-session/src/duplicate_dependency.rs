//! Project APP strict-mode duplicate HAR dependency validation.
//!
//! This is the snapshot-backed equivalent of hvigor 6.24.3
//! `tasks/duplicate-dependency-check.js#DuplicateDependencyCheck`. It consumes
//! the dependency inventory built at session startup and performs no repeated
//! manifest reads.

use std::path::{Path, PathBuf};

use arkdown_path::canonicalize_or_owned;
use indexmap::IndexMap;
use thiserror::Error;

use crate::module::snapshot::OhosModuleType;
use crate::project::snapshot::ProjectSnapshot;

/// One HAR root shared by more than one local HSP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateHarDependency {
    pub package_name: String,
    pub hsp_modules: Vec<String>,
}

/// Typed equivalent of hvigor's
/// `FOUND_HSPS_THAT_HAVE_THE_SAME_HAR_DEPENDENCIES` diagnostic.
#[derive(Debug, Error)]
#[error(
    "FOUND_HSPS_THAT_HAVE_THE_SAME_HAR_DEPENDENCIES: found duplicate HAR {dependency_word}:\n{details}"
)]
pub struct DuplicateDependencyError {
    dependency_word: &'static str,
    details: String,
    duplicates: Vec<DuplicateHarDependency>,
}

impl DuplicateDependencyError {
    pub fn duplicates(&self) -> &[DuplicateHarDependency] {
        &self.duplicates
    }
}

/// Validates project-local HSP dependency topology before APP packaging.
pub struct DuplicateDependencyValidator<'a> {
    project: &'a ProjectSnapshot,
}

impl<'a> DuplicateDependencyValidator<'a> {
    pub fn new(project: &'a ProjectSnapshot) -> Self {
        Self { project }
    }

    pub fn validate(&self) -> Result<(), DuplicateDependencyError> {
        let local_hars = self.local_project_hars();
        let mut owners = IndexMap::<PathBuf, Vec<String>>::new();
        let mut package_names = IndexMap::<PathBuf, String>::new();

        for module_name in &self.project.module_order {
            let Some(module) = self.project.modules.get(module_name) else {
                continue;
            };
            if !matches!(module.module_type, OhosModuleType::Shared) {
                continue;
            }

            // Hvigor explicitly adds local HAR modules declared by the project
            // to every HSP before it visits npm HAR dependencies.
            for (root, package_name) in &local_hars {
                owners
                    .entry(root.clone())
                    .or_default()
                    .push(module_name.clone());
                package_names.insert(root.clone(), package_name.clone());
            }

            let Some(graph) = self.project.dependency_graph(module_name) else {
                continue;
            };
            for dependency in graph.har_nodes() {
                let Some(root) = dependency.package_root.as_deref() else {
                    continue;
                };
                let root = self.path_identity(root);
                // Project-local HAR modules are the first explicit pass in
                // hvigor's task. Its npm dependency view excludes those
                // project-local nodes for an HSP, so avoid counting one HSP
                // twice when the normalized snapshot also exposes the node.
                if local_hars.contains_key(&root) {
                    continue;
                }
                owners
                    .entry(root.clone())
                    .or_default()
                    .push(module_name.clone());
                package_names.insert(root, dependency.package_name.clone());
            }
        }

        let duplicates = owners
            .into_iter()
            .filter_map(|(root, hsp_modules)| {
                (hsp_modules.len() > 1).then(|| DuplicateHarDependency {
                    package_name: package_names
                        .get(&root)
                        .cloned()
                        .unwrap_or_else(|| "<unknown>".to_string()),
                    hsp_modules,
                })
            })
            .collect::<Vec<_>>();
        if duplicates.is_empty() {
            return Ok(());
        }

        let details = duplicates
            .iter()
            .map(|duplicate| {
                format!(
                    "[{}] depend on the same HAR [{}].",
                    duplicate.hsp_modules.join(", "),
                    duplicate.package_name
                )
            })
            .collect::<Vec<_>>()
            .join(if cfg!(windows) { "\r\n" } else { "\n" });
        Err(DuplicateDependencyError {
            dependency_word: if duplicates.len() > 1 {
                "dependencies"
            } else {
                "dependency"
            },
            details,
            duplicates,
        })
    }

    fn local_project_hars(&self) -> IndexMap<PathBuf, String> {
        let mut declared = IndexMap::new();
        if let Some(root_package) = self.project.root_package.as_ref() {
            for (package_name, specifier) in &root_package.package_json.dependencies {
                let relative = self.local_dependency_path(specifier);
                if relative.as_os_str().is_empty() {
                    continue;
                }
                let root = self.path_identity(&self.project.root.join(relative));
                if root.exists() {
                    declared.insert(root, package_name.clone());
                }
            }
        }

        let mut local_hars = IndexMap::new();
        for module_name in &self.project.module_order {
            let Some(module) = self.project.modules.get(module_name) else {
                continue;
            };
            if !matches!(module.module_type, OhosModuleType::Har) {
                continue;
            }
            let root = self.path_identity(&module.root);
            if let Some(package_name) = declared.get(&root) {
                local_hars.insert(root, package_name.clone());
            }
        }
        local_hars
    }

    fn local_dependency_path<'b>(&self, specifier: &'b str) -> &'b Path {
        let path = specifier
            .get(..5)
            .filter(|prefix| prefix.eq_ignore_ascii_case("file:"))
            .map_or(specifier, |_| &specifier[5..]);
        Path::new(path)
    }

    fn path_identity(&self, path: &Path) -> PathBuf {
        canonicalize_or_owned(path)
    }
}
