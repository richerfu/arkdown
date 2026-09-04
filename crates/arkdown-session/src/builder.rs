//! [`BuildSessionBuilder`] — orchestrates all startup reads and constructs
//! the immutable [`BuildSession`].

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use arkdown_config::build_profile::BuildOptionProfileInventory;
use arkdown_config::ArkdownCustomConfigLoader;
use arkdown_core::{ArkdownConfig, ArktsLanguageVersion};

use crate::dependency_graph::snapshot::DependencyNodeType;
use crate::fingerprint::{FingerprintError, InputFingerprintSet};
use crate::paths::PathResolver;
use crate::profiles::store::{ProfileStore, ProfileStoreError};
use crate::project::snapshot::{ProjectSnapshot, ProjectSnapshotError};
use crate::sdk::snapshot::{SdkSnapshot, SdkSnapshotError};
use crate::session::BuildSession;
use crate::source_store::SourceStore;
use crate::WatchPlanInput;

/// Errors produced during session construction.
#[derive(Debug, Error)]
pub enum BuildSessionError {
    /// The project root directory does not exist.
    #[error("project root not found: {0}")]
    ProjectRootNotFound(PathBuf),

    /// A required config file could not be read.
    #[error("config error: {0}")]
    Config(String),

    /// SDK config could not be converted to a session snapshot.
    #[error("SDK error: {0}")]
    Sdk(#[from] SdkSnapshotError),

    /// Profile store discovery failed.
    #[error("profile store error: {0}")]
    Profile(#[from] ProfileStoreError),

    /// Project/module/package startup snapshot construction failed.
    #[error("project snapshot error: {0}")]
    Project(#[source] Box<ProjectSnapshotError>),

    /// Input fingerprinting failed.
    #[error("fingerprint error: {0}")]
    Fingerprint(#[from] FingerprintError),

    /// An I/O error occurred during startup discovery.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl From<ProjectSnapshotError> for BuildSessionError {
    fn from(value: ProjectSnapshotError) -> Self {
        Self::Project(Box::new(value))
    }
}

/// Builds an immutable [`BuildSession`] from a project root and configuration.
///
/// All startup I/O (reading manifests, packages, profiles, SDK metadata,
/// resource inventories, and fingerprints) happens inside [`build`].
///
/// Later stages consume the resulting snapshots and never re-read startup
/// inputs.
#[derive(Debug)]
pub struct BuildSessionBuilder {
    /// Normalized build config from startup config loading.
    pub config: Arc<ArkdownConfig>,
    /// OHPM resolution mode (offline/prefer-offline/online).
    pub ohpm_mode: crate::package::acquisition::AcquisitionMode,
    /// Hvigor CLI `parameterFile` extra-parameter override.
    pub parameter_file_override: Option<PathBuf>,
    /// Explicit ArkTS language selected by the arkdown invocation.
    pub arkts_language_version: ArktsLanguageVersion,
}

impl BuildSessionBuilder {
    /// Construct a new builder from the normalized build config.
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        Self {
            config,
            ohpm_mode: crate::package::acquisition::AcquisitionMode::Online,
            parameter_file_override: None,
            arkts_language_version: ArktsLanguageVersion::default(),
        }
    }

    /// Set the OHPM resolution mode.
    pub fn with_ohpm_mode(mut self, mode: crate::package::acquisition::AcquisitionMode) -> Self {
        self.ohpm_mode = mode;
        self
    }

    pub fn with_parameter_file_override(mut self, path: PathBuf) -> Self {
        self.parameter_file_override = Some(path);
        self
    }

    /// Select the ArkTS compiler language for the whole build invocation.
    ///
    /// Callers must opt in explicitly; the builder default remains ArkTS 1.1.
    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        self
    }

    /// Build the session, performing all startup I/O.
    ///
    /// This is the only phase that reads project/module/package metadata,
    /// SDK manifests, profile files, and resource trees.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildSessionError`] if any required startup read fails.
    pub fn build(self) -> Result<BuildSession, BuildSessionError> {
        if !self.config.project_root.is_dir() {
            return Err(BuildSessionError::ProjectRootNotFound(
                self.config.project_root.clone(),
            ));
        }

        // Phase 1: Build project and module snapshots.
        let project = ProjectSnapshot::from_config_with_parameter_file_and_acquisition_mode(
            &self.config,
            self.parameter_file_override.clone(),
            Some(self.ohpm_mode),
        )?;

        // Phase 2: Build SDK snapshot.
        let sdk = SdkSnapshot::from_config_for_runtime(
            &self.config.sdk_config,
            &self.config.product.runtime_os,
        )?;

        // Phase 3: Build path resolver.
        let paths = PathResolver::new(self.config.project_root.clone());

        // Phase 4: Build profile store.
        let profiles = ProfileStore::discover_project(&project)?;

        // Phase 5: Load Arkdown-owned project overrides once. Later stages
        // consume this immutable snapshot rather than re-reading the file.
        let custom_config = ArkdownCustomConfigLoader::new(&self.config.project_root)
            .load()
            .map_err(|error| BuildSessionError::Config(error.to_string()))?;

        // Phase 6: Parse project/module target and build-option models once.
        let build_options = BuildOptionProfileInventory::load_with_custom_config(
            &self.config.project_root,
            &self.config.modules,
            &custom_config,
        )
        .map_err(|error| BuildSessionError::Config(error.to_string()))?;

        // Phase 7: Capture source contents once. Transform snapshots and
        // built-in Rolldown adapters borrow this immutable inventory instead
        // of walking or re-reading project/package source trees.
        let source_inventory_roots = self.source_inventory_roots(&project);
        let source_store = SourceStore::discover(&source_inventory_roots).map_err(|source| {
            BuildSessionError::Io {
                path: self.config.project_root.clone(),
                source,
            }
        })?;

        // Phase 8: Build input fingerprints.
        let fingerprints = InputFingerprintSet::build_with_custom_config_and_arkts_version(
            &self.config,
            &project,
            &profiles,
            &sdk,
            &custom_config,
            self.arkts_language_version,
        )?;

        Ok(BuildSession {
            watch_inputs: Arc::new(WatchPlanInput::from_project(
                &project,
                custom_config.source_path(),
            )),
            project: Arc::new(project),
            sdk: Arc::new(sdk),
            paths: Arc::new(paths),
            fingerprints: Arc::new(fingerprints),
            profiles: Arc::new(profiles),
            build_options: Arc::new(build_options),
            custom_config: Arc::new(custom_config),
            source_store: Arc::new(source_store),
            arkts_language_version: self.arkts_language_version,
        })
    }

    fn source_inventory_roots(&self, project: &ProjectSnapshot) -> Vec<PathBuf> {
        let mut roots = BTreeSet::new();
        for module in project.modules.values() {
            // Capture every source set below `src/` once. The active target
            // overlay is selected later without another directory walk.
            roots.insert(module.root.join("src"));
        }
        for graph in project.dependency_graphs.values() {
            for node in graph.nodes.values() {
                if matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar
                        | DependencyNodeType::OhpmSource
                        | DependencyNodeType::IntegratedHsp
                        | DependencyNodeType::NativeSo
                ) {
                    roots.extend(node.package_root.iter().cloned());
                }
            }
        }
        roots.into_iter().collect()
    }
}
