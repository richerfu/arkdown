//! [`PathResolver`] — project/module/package-relative paths and artifact
//! serialization.

use std::path::{Path, PathBuf};

use arkdown_path::{AbsPath, ArtifactPathSerializer, RelPath};

/// Session path resolver for converting between absolute and relative paths
/// and for producing artifact path strings.
#[derive(Debug, Clone)]
pub struct PathResolver {
    /// Absolute path to the project root.
    pub project_root: PathBuf,
}

impl PathResolver {
    /// Construct a new path resolver for the given project root.
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Return the given path as an `AbsPath`, canonicalizing if it exists or
    /// normalizing lexically if it does not.
    pub fn abs_existing_or_lexical(&self, path: &Path) -> AbsPath {
        if path.exists() {
            AbsPath::canonicalize(path).unwrap_or_else(|_| AbsPath::from_absolute(path).unwrap())
        } else if path.is_absolute() {
            AbsPath::from_absolute(path).unwrap()
        } else {
            let joined = self.project_root.join(path);
            AbsPath::from_absolute(joined.clone())
                .unwrap_or_else(|_| AbsPath::from_raw_unchecked(joined))
        }
    }

    /// Return the path relative to the project root.
    ///
    /// Returns `None` if `path` is not inside the project root.
    pub fn project_relative(&self, path: &Path) -> Option<RelPath> {
        let rel = path.strip_prefix(&self.project_root).ok()?;
        RelPath::parse(rel).ok()
    }

    /// Build an artifact path (slash-separated) from a project-relative path.
    pub fn artifact_path_from_relative(&self, rel: &RelPath) -> String {
        ArtifactPathSerializer::rel_to_slash(rel.as_path())
    }
}
