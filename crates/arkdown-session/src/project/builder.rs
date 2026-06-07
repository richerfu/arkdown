//! [`ProjectSnapshotBuilder`] — reads project-level files once and builds
//! [`ProjectSnapshot`].

use crate::project::snapshot::{ProjectSnapshot, ProjectSnapshotError};

/// Reads project-level files once and builds [`ProjectSnapshot`].
///
/// Parallelizes manifest/package/profile reads where possible.
pub struct ProjectSnapshotBuilder {
    root: std::path::PathBuf,
    product_name: String,
}

impl ProjectSnapshotBuilder {
    /// Construct a builder for the given project root.
    pub fn new(root: impl Into<std::path::PathBuf>, product_name: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            product_name: product_name.into(),
        }
    }

    /// Build the project snapshot.
    pub fn build(self) -> Result<ProjectSnapshot, ProjectSnapshotError> {
        ProjectSnapshot::from_root(&self.root, &self.product_name)
    }
}
