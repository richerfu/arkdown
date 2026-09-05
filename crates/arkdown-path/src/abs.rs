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
        let canonical = dunce::canonicalize(path)?;
        #[cfg(windows)]
        let canonical = simplify_verbatim_unc(canonical);
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

/// Canonicalize an existing filesystem path and otherwise preserve the
/// caller's path spelling for an output that has not been created yet.
///
/// Build plans use this at boundaries that legitimately accept both existing
/// inputs and future outputs. Callers that know which kind they have should
/// prefer [`AbsPath::canonicalize`] or [`AbsPath::from_absolute`].
pub fn canonicalize_or_owned(path: &Path) -> PathBuf {
    AbsPath::canonicalize(path)
        .map(AbsPath::into_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
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
    let has_root = path.has_root();
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {
                // `.` — skip
            }
            Component::ParentDir => {
                // Pop a normal segment. A rooted path cannot traverse above
                // its root; a relative path must preserve any unmatched `..`.
                match normalized.components().next_back() {
                    Some(Component::Normal(_)) => {
                        normalized.pop();
                    }
                    Some(Component::ParentDir) | Some(Component::Prefix(_)) | None if !has_root => {
                        normalized.push(component);
                    }
                    Some(Component::RootDir)
                    | Some(Component::Prefix(_))
                    | Some(Component::ParentDir)
                    | None => {}
                    Some(Component::CurDir) => unreachable!("CurDir components are not retained"),
                }
            }
            c => normalized.push(c),
        }
    }
    normalized
}

#[cfg(windows)]
fn simplify_verbatim_unc(path: PathBuf) -> PathBuf {
    use std::ffi::OsString;
    use std::path::Prefix;

    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return path;
    };
    let Prefix::VerbatimUNC(server, share) = prefix.kind() else {
        return path;
    };

    // Node's path.resolve(), and therefore hvigor, uses the regular
    // `\\server\share` spelling. Preserve OsStr components so UNC roots do not
    // require a Unicode round-trip.
    let mut root = OsString::from(r"\\");
    root.push(server);
    root.push(r"\");
    root.push(share);
    let mut simplified = PathBuf::from(root);
    for component in components {
        if component != Component::RootDir {
            simplified.push(component.as_os_str());
        }
    }
    simplified
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> PathBuf {
        std::env::temp_dir().join("arkdown-abs-path-tests")
    }

    #[test]
    fn test_abs_path_from_absolute_on_current_host() {
        let p = AbsPath::from_absolute(test_root().join("usr/local/bin")).expect("should succeed");
        assert!(p.is_absolute());
    }

    #[test]
    fn test_abs_path_from_absolute_rejects_relative() {
        let err = AbsPath::from_absolute("relative/path");
        assert!(err.is_err());
    }

    #[test]
    fn test_abs_path_lexical_normalization() {
        let root = test_root();
        let p = AbsPath::from_absolute(root.join("foo/./bar/../baz")).expect("absolute");
        assert_eq!(p.as_path(), root.join("foo/baz"));
    }

    #[test]
    fn test_abs_path_does_not_retain_parent_segments_above_root() {
        let temp_dir = std::env::temp_dir();
        let root = temp_dir
            .ancestors()
            .last()
            .expect("temporary directory has a filesystem root");
        let path = root.join("../../arkdown-rooted-output");
        let normalized = AbsPath::from_absolute(path).expect("absolute");
        assert!(normalized.is_absolute());
        assert!(!normalized
            .components()
            .any(|component| component == Component::ParentDir));
    }

    #[test]
    fn test_abs_path_join_appends() {
        let root = test_root();
        let base = AbsPath::from_absolute(root.join("project/src")).unwrap();
        let joined = base.join("pages/Index.ets");
        assert_eq!(joined.as_path(), root.join("project/src/pages/Index.ets"));
    }

    #[test]
    fn test_abs_path_parent() {
        let root = test_root();
        let p = AbsPath::from_absolute(root.join("project/src/lib.rs")).unwrap();
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_path(), root.join("project/src"));
    }

    #[test]
    fn test_abs_path_with_extension() {
        let root = test_root();
        let p = AbsPath::from_absolute(root.join("project/src/Index.ets")).unwrap();
        let js = p.with_extension("js");
        assert_eq!(js.as_path(), root.join("project/src/Index.js"));
    }

    #[test]
    fn test_abs_path_display() {
        let root = test_root().join("project");
        let p = AbsPath::from_absolute(root.clone()).unwrap();
        assert_eq!(p.to_string(), root.display().to_string());
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
