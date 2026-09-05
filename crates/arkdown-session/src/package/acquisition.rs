//! OHPM dependency acquisition and immutable package inventory construction.
//!
//! The planner runs exactly once while [`crate::BuildSession`] is created. It
//! resolves issuer-local packages first, batches remaining package/version
//! identities through the global cache or registry, and parses every acquired
//! manifest before later dependency, resolver, transform, or plugin stages run.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_ohpm::{
    OhPackageLockfile, OhpmError, OhpmResolutionMode, OhpmResolver, PackageAcquisitionRequest,
    PackageName,
};
use arkdown_path::canonicalize_or_owned;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use thiserror::Error;

use super::snapshot::{PackageSnapshot, PackageSnapshotError};

/// OHPM acquisition mode selected for one build session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcquisitionMode {
    /// Only use issuer-local packages and the OHPM cache.
    Offline,
    /// Prefer issuer-local packages and cache; use the registry on a miss.
    PreferOffline,
    /// Use local installations first and allow registry resolution.
    #[default]
    Online,
}

impl From<AcquisitionMode> for OhpmResolutionMode {
    fn from(value: AcquisitionMode) -> Self {
        match value {
            AcquisitionMode::Offline => Self::Offline,
            AcquisitionMode::PreferOffline => Self::PreferOffline,
            AcquisitionMode::Online => Self::Online,
        }
    }
}

/// One dependency declaration scheduled by the acquisition planner.
#[derive(Debug, Clone)]
pub struct PackageAcquisitionTask {
    /// Dependency alias used by the declaring manifest.
    pub alias: String,
    /// Registry package name after unwrapping an optional `npm:` alias.
    pub package_name: String,
    /// Requested version constraint, tag, exact version, or local path.
    pub version_spec: Option<String>,
    /// Directory of the declaring package.
    pub issuer_root: PathBuf,
    /// Whether this is a dev-only dependency.
    pub is_dev: bool,
    /// Whether this is a dynamic dependency.
    pub is_dynamic: bool,
}

/// Acquisition failures reported before an immutable build session exists.
#[derive(Debug, Error)]
pub enum PackageAcquisitionError {
    /// The project lockfile could not be read.
    #[error("OHPM lockfile error: {0}")]
    Lockfile(#[source] Box<OhpmError>),
    /// No OHPM cache root can be selected for this process.
    #[error("OHPM resolver initialization failed: {0}")]
    Resolver(#[source] Box<OhpmError>),
    /// A dependency declaration is not a valid OHPM package name.
    #[error("invalid OHPM dependency name '{name}' declared by {}: {message}", issuer.display())]
    InvalidPackageName {
        name: String,
        issuer: PathBuf,
        message: String,
    },
    /// A local/cache/registry acquisition failed.
    #[error("failed to acquire OHPM dependency '{alias}' declared by {}: {source}", issuer.display())]
    Acquire {
        alias: String,
        issuer: PathBuf,
        #[source]
        source: Box<OhpmError>,
    },
    /// An acquired package manifest could not be read or parsed.
    #[error("acquired package metadata error: {0}")]
    Package(#[source] Box<PackageSnapshotError>),
}

impl From<PackageSnapshotError> for PackageAcquisitionError {
    fn from(value: PackageSnapshotError) -> Self {
        Self::Package(Box::new(value))
    }
}

/// Frozen package roots and parsed manifests produced by startup acquisition.
#[derive(Debug, Default)]
pub(crate) struct PackageAcquisitionInventory {
    pub(crate) packages: FxHashMap<PathBuf, Option<Arc<PackageSnapshot>>>,
    pub(crate) resolved_roots: FxHashMap<(PathBuf, String), PathBuf>,
}

impl PackageAcquisitionInventory {
    pub(crate) fn package(&self, root: &Path) -> Option<Arc<PackageSnapshot>> {
        self.packages.get(root).cloned().flatten().or_else(|| {
            self.packages
                .get(&canonicalize_or_owned(root))
                .cloned()
                .flatten()
        })
    }

    fn insert_package(&mut self, root: PathBuf, package: Option<Arc<PackageSnapshot>>) {
        self.packages.insert(root.clone(), package.clone());
        let canonical = canonicalize_or_owned(&root);
        self.packages.entry(canonical).or_insert(package);
    }

    fn contains_resolution(&self, issuer: &Path, alias: &str) -> bool {
        self.resolved_roots
            .contains_key(&(canonicalize_or_owned(issuer), alias.to_string()))
    }

    fn insert_resolution(&mut self, task: &PackageAcquisitionTask, root: PathBuf) {
        self.resolved_roots.insert(
            (canonicalize_or_owned(&task.issuer_root), task.alias.clone()),
            root,
        );
    }
}

/// Builds the complete OHPM package inventory for module dependency roots.
///
/// The project-root manifest is captured as metadata, but its dependency list
/// is not an install request. Hvigor resolves compilation dependencies from
/// each module and merely uses the project installation tree as a lookup
/// fallback; its assemble task does not fetch packages declared only by the
/// root manifest.
pub(crate) struct PackageAcquisitionPlanner<'a> {
    project_root: &'a Path,
    package_roots: Vec<PathBuf>,
    mode: AcquisitionMode,
    acquire_remote_dev_dependencies: bool,
}

impl<'a> PackageAcquisitionPlanner<'a> {
    pub(crate) fn new(
        project_root: &'a Path,
        package_roots: impl IntoIterator<Item = PathBuf>,
        mode: AcquisitionMode,
    ) -> Self {
        let mut package_roots = package_roots.into_iter().collect::<Vec<_>>();
        package_roots.sort();
        package_roots.dedup();
        Self {
            project_root,
            package_roots,
            mode,
            acquire_remote_dev_dependencies: false,
        }
    }

    /// Allow a test build to acquire dev-only packages from the global cache
    /// or registry. Ordinary assemble builds only consume dev dependencies
    /// already materialized in an issuer's `oh_modules`, matching hvigor's
    /// separation between its install wrapper and production task graph.
    pub(crate) fn with_remote_dev_dependencies(mut self, enabled: bool) -> Self {
        self.acquire_remote_dev_dependencies = enabled;
        self
    }

    pub(crate) fn acquire(self) -> Result<PackageAcquisitionInventory, PackageAcquisitionError> {
        let lockfile = OhPackageLockfile::load_or_error(self.project_root)
            .map_err(|error| PackageAcquisitionError::Lockfile(Box::new(error)))?;
        let resolver = OhpmResolver::from_env(None, self.mode.into())
            .map_err(|error| PackageAcquisitionError::Resolver(Box::new(error)))?;
        let mut inventory = PackageAcquisitionInventory::default();
        let mut pending = VecDeque::new();

        let project_package = PackageSnapshot::from_root(self.project_root)?.map(Arc::new);
        inventory.insert_package(self.project_root.to_path_buf(), project_package);

        for root in &self.package_roots {
            let package = PackageSnapshot::from_root(root)?.map(Arc::new);
            inventory.insert_package(root.clone(), package.clone());
            if let Some(package) = package {
                pending.push_back((root.clone(), package));
            }
        }

        let mut visited_issuers = BTreeSet::new();
        while !pending.is_empty() {
            let mut wave = Vec::new();
            while let Some((issuer_root, package)) = pending.pop_front() {
                let canonical_issuer = canonicalize_or_owned(&issuer_root);
                if !visited_issuers.insert(canonical_issuer) {
                    continue;
                }
                wave.extend(self.tasks_for_package(&issuer_root, &package));
            }
            wave.retain(|task| !inventory.contains_resolution(&task.issuer_root, &task.alias));
            if wave.is_empty() {
                continue;
            }
            wave.sort_by(|left, right| {
                (&left.issuer_root, &left.alias).cmp(&(&right.issuer_root, &right.alias))
            });

            let local_results = wave
                .par_iter()
                .map(|task| {
                    self.request_for_task(task)
                        .map(|request| resolver.resolve_local(&request))
                })
                .collect::<Vec<_>>();
            let mut resolved = vec![None; wave.len()];
            let mut remote_groups = BTreeMap::<(String, String), Vec<usize>>::new();
            for (index, local) in local_results.into_iter().enumerate() {
                match local? {
                    Some(result) => resolved[index] = Some(result),
                    None => {
                        let task = &wave[index];
                        let version = lockfile
                            .resolved_version(&task.package_name)
                            .or(task.version_spec.as_deref())
                            .unwrap_or("latest");
                        remote_groups
                            .entry((task.package_name.clone(), version.to_string()))
                            .or_default()
                            .push(index);
                    }
                }
            }

            let groups = remote_groups
                .into_values()
                .filter(|indices| {
                    self.acquire_remote_dev_dependencies
                        || indices.iter().any(|index| !wave[*index].is_dev)
                })
                .collect::<Vec<_>>();
            let remote_results = groups
                .par_iter()
                .map(|indices| {
                    let task = &wave[indices[0]];
                    let request = self.request_for_task(task)?;
                    resolver
                        .resolve_cached_or_fetch(&request, &lockfile)
                        .map_err(|source| PackageAcquisitionError::Acquire {
                            alias: task.alias.clone(),
                            issuer: task.issuer_root.clone(),
                            source: Box::new(source),
                        })
                })
                .collect::<Vec<_>>();
            for (indices, result) in groups.into_iter().zip(remote_results) {
                let result = result?;
                for index in indices {
                    resolved[index] = Some(result.clone());
                }
            }

            for (task, result) in wave.into_iter().zip(resolved) {
                let Some(result) = result else {
                    continue;
                };
                let Some(root) = result.root else {
                    continue;
                };
                let package = match inventory.packages.get(&root) {
                    Some(package) => package.clone(),
                    None => {
                        let package = PackageSnapshot::from_root(&root)?.map(Arc::new);
                        inventory.insert_package(root.clone(), package.clone());
                        package
                    }
                };
                inventory.insert_resolution(&task, root.clone());
                if let Some(package) = package {
                    pending.push_back((root, package));
                }
            }
        }

        Ok(inventory)
    }

    fn tasks_for_package(
        &self,
        issuer_root: &Path,
        package: &PackageSnapshot,
    ) -> Vec<PackageAcquisitionTask> {
        let mut tasks = Vec::new();
        for (dependencies, is_dev, is_dynamic) in [
            (&package.package_json.dependencies, false, false),
            (&package.package_json.dynamic_dependencies, false, true),
            (&package.package_json.dev_dependencies, true, false),
        ] {
            tasks.extend(dependencies.iter().map(|(alias, specifier)| {
                let identity = DependencyPackageIdentity::new(alias, specifier);
                PackageAcquisitionTask {
                    alias: alias.clone(),
                    package_name: identity.package_name,
                    version_spec: Some(identity.version_spec),
                    issuer_root: issuer_root.to_path_buf(),
                    is_dev,
                    is_dynamic,
                }
            }));
        }
        tasks
    }

    fn request_for_task(
        &self,
        task: &PackageAcquisitionTask,
    ) -> Result<PackageAcquisitionRequest, PackageAcquisitionError> {
        let name = PackageName::new(task.package_name.clone()).map_err(|error| {
            PackageAcquisitionError::InvalidPackageName {
                name: task.package_name.clone(),
                issuer: task.issuer_root.clone(),
                message: error.to_string(),
            }
        })?;
        Ok(PackageAcquisitionRequest {
            name,
            version: task.version_spec.clone().map(String::into_boxed_str),
            issuer_root: task.issuer_root.clone(),
        })
    }
}

struct DependencyPackageIdentity {
    package_name: String,
    version_spec: String,
}

impl DependencyPackageIdentity {
    fn new(alias: &str, specifier: &str) -> Self {
        let Some(npm_specifier) = specifier.strip_prefix("npm:") else {
            return Self {
                package_name: alias.to_string(),
                version_spec: specifier.to_string(),
            };
        };
        let split = if npm_specifier.starts_with('@') {
            npm_specifier
                .rfind('@')
                .filter(|index| *index > npm_specifier.find('/').unwrap_or(usize::MAX))
        } else {
            npm_specifier.rfind('@')
        };
        match split {
            Some(index) => Self {
                package_name: npm_specifier[..index].to_string(),
                version_spec: npm_specifier[index + 1..].to_string(),
            },
            None => Self {
                package_name: npm_specifier.to_string(),
                version_spec: "latest".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_alias_separates_scoped_package_name_and_version() {
        let identity = DependencyPackageIdentity::new("alias", "npm:@scope/library@^1.2.0");
        assert_eq!(identity.package_name, "@scope/library");
        assert_eq!(identity.version_spec, "^1.2.0");
    }

    #[test]
    fn offline_planner_resolves_local_and_transitive_path_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        let entry = project.join("entry");
        let library = project.join("library");
        let nested = project.join("nested");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            entry.join("oh-package.json5"),
            r#"{ name: "entry", dependencies: { library: "file:../library" } }"#,
        )
        .unwrap();
        std::fs::write(
            library.join("oh-package.json5"),
            r#"{ name: "library", dependencies: { nested: "file:../nested" } }"#,
        )
        .unwrap();
        std::fs::write(nested.join("oh-package.json5"), r#"{ name: "nested" }"#).unwrap();

        let inventory =
            PackageAcquisitionPlanner::new(project, [entry.clone()], AcquisitionMode::Offline)
                .acquire()
                .unwrap();

        assert_eq!(
            inventory
                .resolved_roots
                .get(&(canonicalize_or_owned(&entry), "library".to_string())),
            Some(&canonicalize_or_owned(&library))
        );
        assert_eq!(
            inventory
                .resolved_roots
                .get(&(canonicalize_or_owned(&library), "nested".to_string())),
            Some(&canonicalize_or_owned(&nested))
        );
        assert_eq!(
            inventory.package(&nested).expect("nested manifest").name,
            "nested"
        );
    }

    #[test]
    fn ordinary_build_does_not_acquire_uninstalled_dev_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        let entry = project.join("entry");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(
            entry.join("oh-package.json5"),
            r#"{ name: "entry", devDependencies: { "@ohos/hypium": "1.0.24" } }"#,
        )
        .unwrap();

        let inventory = PackageAcquisitionPlanner::new(project, [entry], AcquisitionMode::Offline)
            .acquire()
            .expect("ordinary assemble must not require a remote test dependency");

        assert!(inventory.resolved_roots.is_empty());
    }
}
