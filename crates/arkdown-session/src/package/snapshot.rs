//! [`PackageSnapshot`] — parsed package metadata and package root identity.

use std::path::PathBuf;

use arkdown_ohos_schema::RawOhPackage;

/// Parsed package metadata from `oh-package.json5`.
#[derive(Debug, Clone)]
pub struct PackageSnapshot {
    /// Package name from `oh-package.json5`.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Absolute path to the package root directory.
    pub root: PathBuf,
    /// Raw parsed package JSON.
    pub package_json: RawOhPackage,
}

impl PackageSnapshot {
    /// Load from a directory that contains `oh-package.json5`.
    pub fn from_root(root: &std::path::Path) -> Option<Self> {
        let path = root.join("oh-package.json5");
        let content = std::fs::read_to_string(&path).ok()?;
        let pkg: RawOhPackage = json5::from_str(&content).ok()?;
        Some(Self {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            root: root.to_path_buf(),
            package_json: pkg,
        })
    }
}
