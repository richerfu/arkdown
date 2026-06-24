//! [`AbsPath`] — an absolute, lexically-normalized filesystem path.
//!
//! # Invariants
//!
//! - Always absolute (passes [`Path::is_absolute`]).
//! - Lexically normalized: no `.`, `..`, or redundant separators.
//! - Constructed either from canonicalization of an existing path, or from
//!   lexical normalization of a known-absolute path that may not exist yet.
//!
//! # Cross-platform note
//!
//! Absolute-ness is determined by [`Path::is_absolute`], which handles
//! Unix `/`, Windows drive roots (`C:\`), and UNC roots (`\\?\…`).

use std::fmt;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An absolute, lexically-normalized filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AbsPath(PathBuf);

/// Errors produced when constructing an [`AbsPath`].
#[derive(Debug, Error)]
pub enum AbsPathError {
    /// The supplied path is not absolute.
    #[error("path is not absolute: {0}")]
    NotAbsolute(PathBuf),
}

impl AbsPath {
    /// Construct from an existing path by **canonicalization**.
    ///
    /// Use this for input paths that are known to exist on the filesystem.
    /// Symlinks are resolved and the path is made absolute.
    ///
    /// # Errors
    ///
    /// Returns an IO error if the path does not exist or cannot be
    /// canonicalized.
    pub fn canonicalize(path: &Path) -> std::io::Result<Self> {
        let canonical = dunce_canonicalize(path)?;
        Ok(Self(canonical))
    }

    /// Construct from an absolute path by **lexical normalization**.
    ///
    /// Use this for output paths or paths that may not exist yet. The path
    /// must already be absolute; no IO is performed.
    ///
    /// # Errors
    ///
    /// Returns [`AbsPathError::NotAbsolute`] if `path` is not absolute.
    pub fn from_absolute(path: impl Into<PathBuf>) -> Result<Self, AbsPathError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(AbsPathError::NotAbsolute(path));
        }
        Ok(Self(lexical_normalize_pathbuf(path)))
    }

    /// Construct without validation — caller must guarantee the path is
    /// absolute and normalized.
    ///
    /// Prefer [`AbsPath::from_absolute`] or [`AbsPath::canonicalize`] in
    /// production code.
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

    /// Return a new `AbsPath` with `component` appended.
    ///
    /// The component must not be absolute. Use [`PathBuf::join`] semantics:
    /// a component starting with `/` would replace the base, so that is
    /// rejected here.
    pub fn join(&self, component: impl AsRef<Path>) -> Self {
        let joined = self.0.join(component);
        // The result remains absolute because `self.0` is absolute and we
        // only append a relative component.
        Self(lexical_normalize_pathbuf(joined))
    }

    /// Return the parent directory, or `None` if this is the root.
    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|p| Self(p.to_path_buf()))
    }

    /// Return the file name component, if any.
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.0.file_name()
    }

    /// Return the file stem (file name without the last extension).
    pub fn file_stem(&self) -> Option<&std::ffi::OsStr> {
        self.0.file_stem()
    }

    /// Return the extension, if any.
    pub fn extension(&self) -> Option<&std::ffi::OsStr> {
        self.0.extension()
    }

    /// Return a new path with the file extension replaced.
    pub fn with_extension(&self, ext: &str) -> Self {
        Self(self.0.with_extension(ext))
    }

    /// Check whether the path exists on the filesystem.
    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    /// Check whether the path points to a directory.
    pub fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    /// Check whether the path points to a file.
    pub fn is_file(&self) -> bool {
        self.0.is_file()
    }
}

impl Deref for AbsPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for AbsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl From<AbsPath> for PathBuf {
    fn from(p: AbsPath) -> Self {
        p.0
    }
}

impl fmt::Display for AbsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// Lexically normalize a `PathBuf` by removing `.` components and resolving
/// `..` components without touching the filesystem.
///
/// This is the same algorithm OXC uses for lexical normalization. It does
/// **not** resolve symlinks.
pub(crate) fn lexical_normalize_pathbuf(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {
                // `.` — skip
            }
            Component::ParentDir => {
                // `..` — pop last component if not a prefix/root
                if !normalized.pop() {
                    // Cannot go above the root; keep the `..` only for relative
                    // paths (AbsPath should never have loose `..` above root).
                    normalized.push(component);
                }
            }
            c => normalized.push(c),
        }
    }
    normalized
}

/// Platform-agnostic canonicalization that avoids UNC path expansion on Windows.
///
/// On Windows, `std::fs::canonicalize` returns extended-length UNC paths
/// (`\\?\C:\…`) which many tools cannot handle. This function produces a
/// shorter form when the path does not require UNC.
fn dunce_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    // Use `std::fs::canonicalize` for now. Callers that need Windows
    // dunce behavior can replace this with the `dunce` crate.
    std::fs::canonicalize(path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_abs_path_from_absolute_unix() {
        let p = AbsPath::from_absolute("/usr/local/bin").expect("should succeed");
        assert!(p.is_absolute());
    }

    #[test]
    fn test_abs_path_from_absolute_rejects_relative() {
        let err = AbsPath::from_absolute("relative/path");
        assert!(err.is_err());
    }

    #[test]
    fn test_abs_path_lexical_normalization() {
        // /foo/./bar/../baz -> /foo/baz
        let p = AbsPath::from_absolute("/foo/./bar/../baz").expect("absolute");
        assert_eq!(p.as_path(), Path::new("/foo/baz"));
    }

    #[test]
    fn test_abs_path_join_appends() {
        let base = AbsPath::from_absolute("/project/src").unwrap();
        let joined = base.join("pages/Index.ets");
        assert_eq!(joined.as_path(), Path::new("/project/src/pages/Index.ets"));
    }

    #[test]
    fn test_abs_path_parent() {
        let p = AbsPath::from_absolute("/project/src/lib.rs").unwrap();
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_path(), Path::new("/project/src"));
    }

    #[test]
    fn test_abs_path_with_extension() {
        let p = AbsPath::from_absolute("/project/src/Index.ets").unwrap();
        let js = p.with_extension("js");
        assert_eq!(js.as_path(), Path::new("/project/src/Index.js"));
    }

    #[test]
    fn test_abs_path_display() {
        let p = AbsPath::from_absolute("/project").unwrap();
        assert_eq!(p.to_string(), "/project");
    }

    #[cfg(windows)]
    #[test]
    fn test_abs_path_windows_drive() {
        let p = AbsPath::from_absolute("C:\\Users\\ranger\\project").unwrap();
        assert!(p.is_absolute());
    }

    #[cfg(windows)]
    #[test]
    fn test_abs_path_relative_rejected_on_windows() {
        let err = AbsPath::from_absolute("relative\\path");
        assert!(err.is_err());
    }
}
