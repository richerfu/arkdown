//! Session-built package index and adapters for `arkdown-resolver`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::package::snapshot::PackageSnapshot;

/// An index of package roots built from session discovery.
///
/// Used by `arkdown-resolver` to resolve package imports without performing
/// filesystem scans during hot resolution paths.
#[derive(Debug, Clone, Default)]
pub struct PackageIndex {
    /// Map from package name → resolved root path.
    pub roots: BTreeMap<String, PathBuf>,
}

impl PackageIndex {
    /// Register a package root.
    pub fn insert(&mut self, name: String, root: PathBuf) {
        self.roots.insert(name, root);
    }

    /// Look up a package root by name.
    pub fn get(&self, name: &str) -> Option<&PathBuf> {
        self.roots.get(name)
    }

    /// Build from a list of package snapshots.
    pub fn from_snapshots<'a>(
        snapshots: impl IntoIterator<Item = &'a PackageSnapshot>,
    ) -> Self {
        let mut index = Self::default();
        for snap in snapshots {
            if !snap.name.is_empty() {
                index.insert(snap.name.clone(), snap.root.clone());
            }
        }
        index
    }
}
