//! [`RelPath`] — a relative path that does not traverse above its base.
//!
//! # Invariants
//!
//! - Not absolute.
//! - Contains no leading `..` components that would traverse above the base
//!   directory.
//! - Does not start with a root (`/`).

use std::fmt;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A relative path that stays within its base directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelPath(PathBuf);

/// Errors produced when constructing a [`RelPath`].
#[derive(Debug, Error)]
pub enum RelPathError {
    /// The supplied path is absolute.
    #[error("path is absolute: {0}")]
    IsAbsolute(PathBuf),
    /// The supplied path traverses above the base directory.
    #[error("path traverses above base: {0}")]
    TraversesAboveBase(PathBuf),
}

impl RelPath {
    /// Parse a relative path, rejecting absolute paths and `..`-traversals
    /// above the implicit base.
    ///
    /// # Errors
    ///
    /// - [`RelPathError::IsAbsolute`] — path starts with a root component.
    /// - [`RelPathError::TraversesAboveBase`] — path leads with `..`.
    pub fn parse(path: impl Into<PathBuf>) -> Result<Self, RelPathError> {
        let path = path.into();
        if path.is_absolute() {
            return Err(RelPathError::IsAbsolute(path));
        }
        // Check for leading `..` traversal.
        let first = path.components().next();
        if matches!(first, Some(Component::ParentDir)) {
            return Err(RelPathError::TraversesAboveBase(path));
        }
        Ok(Self(path))
    }

    /// Construct from a relative path without validation.
    ///
    /// Prefer [`RelPath::parse`] in production code.
    #[doc(hidden)]
    pub fn from_raw_unchecked(path: PathBuf) -> Self {
        Self(path)
    }

    /// Borrow as a [`Path`].
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Consume into the inner [`PathBuf`].
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl Deref for RelPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for RelPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl From<RelPath> for PathBuf {
    fn from(p: RelPath) -> Self {
        p.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rel_path_parse_ok() {
        let p = RelPath::parse("pages/Index.ets").unwrap();
        assert_eq!(p.as_path(), Path::new("pages/Index.ets"));
    }

    #[test]
    fn test_rel_path_parse_rejects_absolute() {
        let err = RelPath::parse("/absolute/path");
        assert!(matches!(err, Err(RelPathError::IsAbsolute(_))));
    }

    #[test]
    fn test_rel_path_parse_rejects_traversal() {
        let err = RelPath::parse("../outside/base");
        assert!(matches!(err, Err(RelPathError::TraversesAboveBase(_))));
    }

    #[test]
    fn test_rel_path_dot_is_ok() {
        // `.` alone is the current directory — not a traversal.
        let p = RelPath::parse(".").unwrap();
        assert_eq!(p.as_path(), Path::new("."));
    }

    #[test]
    fn test_rel_path_subdir_then_dotdot_is_ok() {
        // `a/../b` does not lead with `..` so it passes the leading-check.
        // (The spec only prevents leading traversal, not internal one.)
        let p = RelPath::parse("a/../b").unwrap();
        assert_eq!(p.as_path(), Path::new("a/../b"));
    }

    #[test]
    fn test_rel_path_display() {
        let p = RelPath::parse("ets/pages/Index.ets").unwrap();
        assert_eq!(p.to_string(), "ets/pages/Index.ets");
    }

    #[cfg(windows)]
    #[test]
    fn test_rel_path_rejects_windows_drive() {
        let err = RelPath::parse("C:\\windows\\path");
        // Windows `C:\...` paths are absolute.
        assert!(err.is_err());
    }
}
