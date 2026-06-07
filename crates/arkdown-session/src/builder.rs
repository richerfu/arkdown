//! [`BuildSessionBuilder`] — orchestrates all startup reads and constructs
//! the immutable [`BuildSession`].

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::fingerprint::InputFingerprintSet;
use crate::paths::PathResolver;
use crate::profiles::store::ProfileStore;
use crate::project::snapshot::ProjectSnapshot;
use crate::sdk::snapshot::SdkSnapshot;
use crate::session::BuildSession;

/// Errors produced during session construction.
#[derive(Debug, Error)]
pub enum BuildSessionError {
    /// The project root directory does not exist.
    #[error("project root not found: {0}")]
    ProjectRootNotFound(PathBuf),

    /// A required config file could not be read.
    #[error("config error: {0}")]
    Config(String),

    /// An I/O error occurred during startup discovery.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
    /// Absolute project root path.
    pub project_root: PathBuf,
    /// Selected product name.
    pub product_name: String,
    /// OHPM resolution mode (offline/prefer-offline/online).
    pub ohpm_mode: crate::package::acquisition::AcquisitionMode,
}

impl BuildSessionBuilder {
    /// Construct a new builder for the given project root.
    pub fn new(project_root: PathBuf, product_name: String) -> Self {
        Self {
            project_root,
            product_name,
            ohpm_mode: crate::package::acquisition::AcquisitionMode::Online,
        }
    }

    /// Set the OHPM resolution mode.
    pub fn with_ohpm_mode(
        mut self,
        mode: crate::package::acquisition::AcquisitionMode,
    ) -> Self {
        self.ohpm_mode = mode;
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
        if !self.project_root.is_dir() {
            return Err(BuildSessionError::ProjectRootNotFound(self.project_root));
        }

        // Phase 1: Build project and module snapshots.
        let project = ProjectSnapshot::from_root(&self.project_root, &self.product_name)
            .map_err(|e| BuildSessionError::Config(e.to_string()))?;

        // Phase 2: Build SDK snapshot.
        let sdk = SdkSnapshot::from_env()
            .unwrap_or_else(|_| SdkSnapshot::placeholder());

        // Phase 3: Build path resolver.
        let paths = PathResolver::new(self.project_root.clone());

        // Phase 4: Build profile store.
        let profiles = ProfileStore::new(&project);

        // Phase 5: Build input fingerprints.
        let fingerprints = InputFingerprintSet::build_from_project(&project);

        Ok(BuildSession {
            project: Arc::new(project),
            sdk: Arc::new(sdk),
            paths: Arc::new(paths),
            fingerprints: Arc::new(fingerprints),
            profiles: Arc::new(profiles),
        })
    }
}
