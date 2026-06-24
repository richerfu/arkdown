//! [`ProjectSnapshotBuilder`] — reads project-level files once and builds
//! [`ProjectSnapshot`].

use std::sync::Arc;

use arkdown_core::ArkdownConfig;

use crate::project::snapshot::{ProjectSnapshot, ProjectSnapshotError};

/// Reads project-level files once and builds [`ProjectSnapshot`].
///
/// Parallelizes manifest/package/profile reads where possible.
pub struct ProjectSnapshotBuilder {
    config: Arc<ArkdownConfig>,
}

impl ProjectSnapshotBuilder {
    /// Construct a builder from normalized config.
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        Self { config }
    }

    /// Build the project snapshot.
    pub fn build(self) -> Result<ProjectSnapshot, ProjectSnapshotError> {
        ProjectSnapshot::from_config(&self.config)
    }
}
