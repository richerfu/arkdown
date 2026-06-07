//! [`ModuleSnapshotBuilder`] — reads module-level files once and builds
//! [`ModuleSnapshot`].

use crate::module::snapshot::ModuleSnapshot;

/// Reads module-level files once and builds [`ModuleSnapshot`].
pub struct ModuleSnapshotBuilder {
    root: std::path::PathBuf,
    name: String,
}

impl ModuleSnapshotBuilder {
    pub fn new(root: impl Into<std::path::PathBuf>, name: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            name: name.into(),
        }
    }

    pub fn build(self) -> Result<ModuleSnapshot, std::io::Error> {
        ModuleSnapshot::from_root(&self.root, &self.name)
    }
}
