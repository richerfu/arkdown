//! [`PackageSnapshot`] — parsed package metadata and package root identity.

use std::path::PathBuf;

use arkdown_ohos_schema::RawOhPackage;
use thiserror::Error;

/// Errors produced while reading package metadata.
#[derive(Debug, Error)]
pub enum PackageSnapshotError {
    /// A package manifest could not be read.
    #[error("failed to read package manifest {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A package manifest could not be parsed.
    #[error("failed to parse package manifest {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

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
    pub fn from_root(root: &std::path::Path) -> Result<Option<Self>, PackageSnapshotError> {
        let path = root.join("oh-package.json5");
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            std::fs::read_to_string(&path).map_err(|source| PackageSnapshotError::Io {
                path: path.clone(),
                source,
            })?;
        let pkg: RawOhPackage =
            json5::from_str(&content).map_err(|error| PackageSnapshotError::Parse {
                path: path.clone(),
                message: error.to_string(),
            })?;
        Ok(Some(Self {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            root: root.to_path_buf(),
            package_json: pkg,
        }))
    }
}
