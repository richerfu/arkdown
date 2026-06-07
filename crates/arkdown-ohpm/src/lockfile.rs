//! `oh-package-lock.json5` lockfile reader.
//!
//! Parses the lockfile that `ohpm install` generates and exposes the resolved
//! package versions so that the dependency graph builder can locate packages
//! in the OHPM global cache when they are not materialized under a local
//! `oh_modules/` directory.
//!
//! ## Lockfile format
//!
//! ```json5
//! {
//!   "meta": { "generated": "2024-01-01T00:00:00.000Z" },
//!   "packages": {
//!     "@ohos/crypto-js@1.0.0": {
//!       "version": "1.0.0",
//!       "resolved": "https://registry.ohpm.openharmony.cn/@ohos/crypto-js",
//!       "integrity": "sha512-...",
//!       "dependencies": {}
//!     }
//!   }
//! }
//! ```
//!
//! The top-level key under `packages` is `<name>@<version>`. The `version`
//! field inside the entry is the resolved (pinned) version.

use std::path::Path;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::error::OhpmError;

/// Parsed `oh-package-lock.json5` lockfile.
///
/// Provides O(1) lookup of resolved package versions by name.
#[derive(Debug, Default)]
pub struct OhPackageLockfile {
    /// Map from package name (without `@version` suffix) to the resolved
    /// [`LockfileEntry`]. When multiple versions of the same package are
    /// locked, only the last entry is stored.
    pub entries: IndexMap<String, LockfileEntry>,
}

/// A single resolved package entry in the lockfile.
#[derive(Debug, Clone)]
pub struct LockfileEntry {
    /// Resolved (pinned) version string, e.g. `"1.0.0"`.
    pub version: Box<str>,
    /// Registry URL the package was fetched from.
    pub resolved: Option<Box<str>>,
    /// Integrity hash (SHA-512 or SHA-256 in SRI format).
    pub integrity: Option<Box<str>>,
    /// Direct dependency names declared by this package.
    pub dependencies: Vec<String>,
}

// ---------------------------------------------------------------------------
// Serde deserialization helpers (private)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawLockfile {
    #[serde(default)]
    packages: IndexMap<String, RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    version: String,
    #[serde(default)]
    resolved: Option<String>,
    #[serde(default)]
    integrity: Option<String>,
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl OhPackageLockfile {
    /// Parse `oh-package-lock.json5` content from a string.
    ///
    /// Returns an empty lockfile on parse error rather than propagating the
    /// error, so callers can treat a missing or corrupt lockfile as "no lock
    /// information available" without failing the build.
    pub fn parse(content: &str) -> Self {
        let raw: RawLockfile = match json5::from_str(content) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };

        let mut entries = IndexMap::new();
        for (key, raw_entry) in raw.packages {
            let name = strip_version_suffix(&key).to_string();
            entries.insert(
                name,
                LockfileEntry {
                    version: raw_entry.version.into_boxed_str(),
                    resolved: raw_entry.resolved.map(String::into_boxed_str),
                    integrity: raw_entry.integrity.map(String::into_boxed_str),
                    dependencies: raw_entry.dependencies.into_keys().collect(),
                },
            );
        }

        OhPackageLockfile { entries }
    }

    /// Load and parse `oh-package-lock.json5` from `root`.
    ///
    /// Returns an empty lockfile when the file does not exist or cannot be
    /// read, so callers can treat a missing lockfile gracefully.
    pub fn load(root: &Path) -> Self {
        let path = root.join("oh-package-lock.json5");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        Self::parse(&content)
    }

    /// Load and parse, returning a typed error on I/O failure.
    ///
    /// Unlike [`OhPackageLockfile::load`], this variant reports I/O errors
    /// to the caller. Parse errors on a present file still silently return an
    /// empty lockfile.
    pub fn load_or_error(root: &Path) -> Result<Self, OhpmError> {
        let path = root.join("oh-package-lock.json5");
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Self::parse(&content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(OhpmError::io(path, e)),
        }
    }

    /// Look up the resolved version for a package by name.
    ///
    /// Returns `None` when the package is not recorded in the lockfile.
    pub fn resolved_version(&self, package_name: &str) -> Option<&str> {
        self.entries.get(package_name).map(|e| e.version.as_ref())
    }

    /// Look up a full [`LockfileEntry`] by package name.
    pub fn get(&self, package_name: &str) -> Option<&LockfileEntry> {
        self.entries.get(package_name)
    }

    /// Return `true` when the lockfile contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Strip the trailing `@<version>` suffix from a lockfile key.
///
/// For `"lodash@4.17.21"` returns `"lodash"`.
/// For `"@ohos/crypto-js@1.0.0"` returns `"@ohos/crypto-js"`.
///
/// Heuristic: find the last `@` that is followed by a digit (semver versions
/// always start with a digit).
pub(crate) fn strip_version_suffix(key: &str) -> &str {
    let bytes = key.as_bytes();
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'@' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit()) {
            return &key[..i];
        }
    }
    key
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LOCKFILE: &str = r#"{
        "meta": { "generated": "2024-01-01T00:00:00.000Z" },
        "packages": {
            "@ohos/crypto-js@1.0.0": {
                "version": "1.0.0",
                "resolved": "https://registry.ohpm.openharmony.cn/@ohos/crypto-js",
                "integrity": "sha512-abc123==",
                "dependencies": {}
            },
            "lodash@4.17.21": {
                "version": "4.17.21",
                "resolved": "https://registry.ohpm.openharmony.cn/lodash",
                "integrity": "sha512-xyz==",
                "dependencies": { "some-dep": "^1.0.0" }
            }
        }
    }"#;

    #[test]
    fn test_parse_sample_lockfile() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert!(!lf.is_empty());
        assert_eq!(lf.entries.len(), 2);
    }

    #[test]
    fn test_resolved_version_plain_package() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert_eq!(lf.resolved_version("lodash"), Some("4.17.21"));
    }

    #[test]
    fn test_resolved_version_scoped_package() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert_eq!(lf.resolved_version("@ohos/crypto-js"), Some("1.0.0"));
    }

    #[test]
    fn test_resolved_version_missing() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert_eq!(lf.resolved_version("nonexistent"), None);
    }

    #[test]
    fn test_get_entry_integrity() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        let entry = lf.get("@ohos/crypto-js").expect("present");
        assert_eq!(entry.integrity.as_deref(), Some("sha512-abc123=="));
    }

    #[test]
    fn test_empty_lockfile_on_invalid_json() {
        let lf = OhPackageLockfile::parse("this is not json");
        assert!(lf.is_empty());
    }

    #[test]
    fn test_strip_version_suffix_plain() {
        assert_eq!(strip_version_suffix("lodash@4.17.21"), "lodash");
    }

    #[test]
    fn test_strip_version_suffix_scoped() {
        assert_eq!(
            strip_version_suffix("@ohos/crypto-js@1.0.0"),
            "@ohos/crypto-js"
        );
    }

    #[test]
    fn test_strip_version_suffix_no_version() {
        assert_eq!(strip_version_suffix("some-pkg"), "some-pkg");
    }
}
