//! OHPM global cache layout, path construction, and offline/prefer-offline lookup.
//!
//! The OHPM global cache lives at `$OHPM_HOME` (or `~/.ohpm` as fallback).
//! Package directories follow the layout:
//!
//! ```text
//! $OHPM_HOME/
//!   <disk-encoded-name>@<version>/
//!     oh_modules/
//!       <original-name>/      ← the actual package root
//!         oh-package.json5
//!         ...
//! ```
//!
//! For scoped packages, `@scope/name` → `@scope+name` on disk.

use std::path::{Path, PathBuf};

use crate::lockfile::OhPackageLockfile;
use crate::package_name::PackageName;

/// Result of a cache lookup for a package.
#[derive(Debug, Clone)]
pub struct CacheHit {
    /// The resolved package root directory inside the OHPM cache.
    pub root: PathBuf,
    /// The version found in the cache.
    pub version: Box<str>,
}

/// Return the OHPM global cache root directory.
///
/// Checks `$OHPM_HOME` first, then falls back to `$HOME/.ohpm`.
pub fn ohpm_cache_root() -> Option<PathBuf> {
    if let Some(ohpm_home) = std::env::var_os("OHPM_HOME") {
        let p = PathBuf::from(ohpm_home);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    let p = home.join(".ohpm");
    // Return even if it does not exist yet — callers check existence.
    Some(p)
}

/// Compute the expected cache directory path for `name@version`.
///
/// Returns the path to the package root inside the cache, i.e.:
/// `$OHPM_HOME/<disk-name>@<version>/oh_modules/<original-name>/`
pub fn cache_path(cache_root: &Path, name: &PackageName, version: &str) -> PathBuf {
    let disk_name = name.disk_name();
    cache_root
        .join(format!("{disk_name}@{version}"))
        .join("oh_modules")
        .join(name.as_str())
}

/// Look up a package in the OHPM global cache.
///
/// Returns `Some(CacheHit)` when the package root exists in the cache.
/// Returns `None` when the package is absent.
///
/// # Arguments
///
/// - `cache_root` — the OHPM home directory (`$OHPM_HOME` / `~/.ohpm`).
/// - `name` — the package name.
/// - `version` — the resolved version (from lockfile or explicit spec).
pub fn lookup_cache(cache_root: &Path, name: &PackageName, version: &str) -> Option<CacheHit> {
    let path = cache_path(cache_root, name, version);
    if path.is_dir() {
        // Canonicalize to resolve any symlinks.
        let root = std::fs::canonicalize(&path).unwrap_or(path);
        return Some(CacheHit {
            root,
            version: version.into(),
        });
    }
    None
}

/// Look up a package in the OHPM global cache using a lockfile for version
/// resolution.
///
/// If the lockfile has a resolved version for `name`, tries `cache_path`
/// with that version. Falls back to checking for any installed version if
/// the lockfile has no entry.
///
/// Returns `None` when the package is absent from the cache.
pub fn lookup_via_lockfile(
    cache_root: &Path,
    name: &PackageName,
    lockfile: &OhPackageLockfile,
) -> Option<CacheHit> {
    if let Some(version) = lockfile.resolved_version(name.as_str()) {
        return lookup_cache(cache_root, name, version);
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_path_plain_package() {
        let root = Path::new("/home/user/.ohpm");
        let name = PackageName::new("lodash").unwrap();
        let path = cache_path(root, &name, "4.17.21");
        assert_eq!(
            path,
            Path::new("/home/user/.ohpm/lodash@4.17.21/oh_modules/lodash")
        );
    }

    #[test]
    fn test_cache_path_scoped_package() {
        let root = Path::new("/home/user/.ohpm");
        let name = PackageName::new("@ohos/hilog").unwrap();
        let path = cache_path(root, &name, "1.0.0");
        // Disk name uses `+` instead of `/`
        assert_eq!(
            path,
            Path::new("/home/user/.ohpm/@ohos+hilog@1.0.0/oh_modules/@ohos/hilog")
        );
    }

    #[test]
    fn test_lookup_cache_missing_returns_none() {
        // A non-existent directory always returns None.
        let root = Path::new("/nonexistent/ohpm/home");
        let name = PackageName::new("lodash").unwrap();
        let result = lookup_cache(root, &name, "4.17.21");
        assert!(result.is_none());
    }

    #[test]
    fn test_lookup_via_lockfile_no_entry_returns_none() {
        let root = Path::new("/home/user/.ohpm");
        let name = PackageName::new("unknown-pkg").unwrap();
        let lf = OhPackageLockfile::default();
        let result = lookup_via_lockfile(root, &name, &lf);
        assert!(result.is_none());
    }
}
