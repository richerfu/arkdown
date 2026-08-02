//! [`PackageSnapshot`] — parsed package metadata and package root identity.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_ohos_schema::RawOhPackage;
use arkdown_path::lexical_normalize;
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
    /// Files collected from the declared `oh-exports` entries.
    ///
    /// `None` means the field was absent. `Some([])` means the manifest
    /// explicitly declared an empty array, matching hvigor's truthy-array
    /// semantics.
    pub oh_export_files: Option<Arc<[PathBuf]>>,
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
        Ok(Some(Self::from_parsed(root, pkg)?))
    }

    /// Build from the complete object held by hvigor's in-memory
    /// `OhPackageLoader` after a plugin context mutation.
    pub fn from_runtime_value(
        root: &Path,
        value: &serde_json::Value,
    ) -> Result<Self, PackageSnapshotError> {
        let path = root.join("oh-package.json5");
        let package =
            serde_json::from_value(value.clone()).map_err(|error| PackageSnapshotError::Parse {
                path,
                message: error.to_string(),
            })?;
        Self::from_parsed(root, package)
    }

    pub(crate) fn from_parsed(
        root: &Path,
        pkg: RawOhPackage,
    ) -> Result<Self, PackageSnapshotError> {
        let oh_export_files = OhExportsCollector::new(root).collect(pkg.oh_exports.as_deref())?;
        Ok(Self {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            root: root.to_path_buf(),
            package_json: pkg,
            oh_export_files,
        })
    }
}

/// Snapshot-time implementation of
/// `oh-exports-util.ts#collectOhExportsFilePaths`.
struct OhExportsCollector<'a> {
    root: &'a Path,
}

impl<'a> OhExportsCollector<'a> {
    fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn collect(
        &self,
        declarations: Option<&[String]>,
    ) -> Result<Option<Arc<[PathBuf]>>, PackageSnapshotError> {
        let Some(declarations) = declarations else {
            return Ok(None);
        };
        let mut files = Vec::new();
        for declaration in declarations {
            let resolved = lexical_normalize(&self.root.join(declaration));
            if !resolved.exists() {
                continue;
            }
            let metadata =
                std::fs::metadata(&resolved).map_err(|source| PackageSnapshotError::Io {
                    path: resolved.clone(),
                    source,
                })?;
            if metadata.is_dir() {
                files.extend(self.collect_directory(&resolved)?);
            } else if metadata.is_file() {
                files.push(resolved);
            }
        }
        Ok(Some(Arc::from(files)))
    }

    fn collect_directory(&self, directory: &Path) -> Result<Vec<PathBuf>, PackageSnapshotError> {
        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(directory).follow_links(false) {
            let entry = entry.map_err(|error| PackageSnapshotError::Io {
                path: error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| directory.to_path_buf()),
                source: std::io::Error::other(error.to_string()),
            })?;
            if entry.file_type().is_file()
                && matches!(
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("ets" | "ts" | "js")
                )
            {
                files.push(entry.path().to_path_buf());
            }
        }
        files.sort();
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::PackageSnapshot;

    #[test]
    fn collects_declared_files_and_recursive_code_directories_in_manifest_order() {
        let temp = tempfile::tempdir().unwrap();
        let exports = temp.path().join("exports");
        std::fs::create_dir_all(exports.join("nested")).unwrap();
        std::fs::write(exports.join("a.ets"), "").unwrap();
        std::fs::write(exports.join("nested/b.ts"), "").unwrap();
        std::fs::write(exports.join("ignored.json"), "").unwrap();
        std::fs::write(temp.path().join("direct.js"), "").unwrap();
        std::fs::write(
            temp.path().join("oh-package.json5"),
            r#"{ name: "library", "oh-exports": ["direct.js", "exports"] }"#,
        )
        .unwrap();

        let snapshot = PackageSnapshot::from_root(temp.path())
            .unwrap()
            .expect("package snapshot");
        let files = snapshot.oh_export_files.expect("declared oh-exports");

        assert_eq!(
            files.as_ref(),
            &[
                temp.path().join("direct.js"),
                exports.join("a.ets"),
                exports.join("nested/b.ts"),
            ]
        );
    }

    #[test]
    fn preserves_present_empty_oh_exports_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("oh-package.json5"),
            r#"{ name: "library", "oh-exports": [] }"#,
        )
        .unwrap();

        let snapshot = PackageSnapshot::from_root(temp.path())
            .unwrap()
            .expect("package snapshot");
        assert_eq!(
            snapshot
                .oh_export_files
                .expect("present empty declaration")
                .len(),
            0
        );
    }
}
