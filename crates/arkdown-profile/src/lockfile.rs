//! `oh-package-lock.json5` lockfile reader.
//!
//! Parses the lockfile that `ohpm install` generates and exposes the resolved
//! package versions so that [`LocalDependencyGraph`] can locate packages in the
//! OHPM global cache when they are not materialised under a local `oh_modules/`
//! directory.
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
//! The top-level key under `packages` is `<name>@<version>`.  The `version`
//! field inside the entry is the resolved (pinned) version.
//!
//! ## Integration with `LocalDependencyGraph`
//!
//! Call [`OhPackageLockfile::load`] to parse the lockfile, then use
//! [`OhPackageLockfile::resolved_version`] to look up the pinned version for a
//! package name.  Pass the resolved version to the OHPM global cache path
//! computation in `resolve_dep_dir`.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;

/// Parsed `oh-package-lock.json5` lockfile.
///
/// Provides O(1) lookup of resolved package versions by name.
#[derive(Debug, Default)]
pub struct OhPackageLockfile {
    /// Map from package name (without `@version` suffix) to the resolved
    /// [`LockfileEntry`].  When multiple versions of the same package are
    /// locked (rare but possible in monorepos), only the last entry is stored.
    pub entries: IndexMap<String, LockfileEntry>,
}

/// A single resolved package entry in the lockfile.
#[derive(Debug, Clone)]
pub struct LockfileEntry {
    /// Resolved (pinned) version string, e.g. `"1.0.0"`.
    pub version: String,
    /// Registry URL the package was fetched from.
    pub resolved: Option<String>,
    /// Integrity hash (SHA-512 or SHA-256 in SRI format).
    pub integrity: Option<String>,
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
            // The key is `<name>@<version>`.  Strip the trailing `@<version>`
            // to get the bare package name.  For scoped packages like
            // `@ohos/crypto-js@1.0.0` the first `@` is the scope prefix, so
            // we strip from the LAST `@` that is followed by a digit.
            let name = strip_version_suffix(&key);
            entries.insert(
                name.to_string(),
                LockfileEntry {
                    version: raw_entry.version,
                    resolved: raw_entry.resolved,
                    integrity: raw_entry.integrity,
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

    /// Look up the resolved version for a package by name.
    ///
    /// Returns `None` when the package is not recorded in the lockfile.
    pub fn resolved_version(&self, package_name: &str) -> Option<&str> {
        self.entries.get(package_name).map(|e| e.version.as_str())
    }

    /// Look up a full [`LockfileEntry`] by package name.
    ///
    /// Returns `None` when the package is not in the lockfile.
    pub fn get(&self, package_name: &str) -> Option<&LockfileEntry> {
        self.entries.get(package_name)
    }

    /// Return `true` when the lockfile contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolve the OHPM global-cache path for `package_name` using the
    /// version recorded in this lockfile.
    ///
    /// Returns `None` when:
    /// - The package is not in the lockfile.
    /// - `$OHPM_HOME` / `~/.ohpm` cannot be determined.
    /// - The computed cache path does not exist on disk.
    ///
    /// This mirrors `resolve_dep_dir`'s global-cache fallback logic so that
    /// callers can pass the result directly to dependency graph construction.
    pub fn resolve_from_cache(&self, package_name: &str) -> Option<PathBuf> {
        let version = self.resolved_version(package_name)?;
        let cache_root = ohpm_global_cache_root()?;

        // Standard path: `$OHPM_HOME/<name>@<version>/oh_modules/<name>/`
        let candidate = cache_root
            .join(format!("{}@{}", package_name, version))
            .join("oh_modules")
            .join(package_name);
        if candidate.is_dir() {
            return Some(std::fs::canonicalize(&candidate).unwrap_or(candidate));
        }

        // Scoped package encoding: `@scope/pkg` → `@scope+pkg` on disk.
        if package_name.starts_with('@') {
            let encoded = package_name.replacen('/', "+", 1);
            let scoped = cache_root
                .join(format!("{}@{}", encoded, version))
                .join("oh_modules")
                .join(package_name);
            if scoped.is_dir() {
                return Some(std::fs::canonicalize(&scoped).unwrap_or(scoped));
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Strip the trailing `@<version>` suffix from a lockfile key.
///
/// For a plain package key like `"lodash@4.17.21"` this returns `"lodash"`.
/// For a scoped key like `"@ohos/crypto-js@1.0.0"` this returns
/// `"@ohos/crypto-js"` (the first `@` is the scope prefix, not a version
/// separator).
///
/// The heuristic: find the last `@` that is followed by a digit (version
/// numbers always start with a digit in semver).
fn strip_version_suffix(key: &str) -> &str {
    // Scan from the end for `@` followed by a digit.
    let bytes = key.as_bytes();
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'@' {
            // Check that the character after `@` is a digit.
            if bytes.get(i + 1).map_or(false, |b| b.is_ascii_digit()) {
                return &key[..i];
            }
        }
    }
    key
}

/// Return the OHPM global cache root directory.
///
/// Checks `$OHPM_HOME` first, then falls back to `~/.ohpm`.
fn ohpm_global_cache_root() -> Option<PathBuf> {
    if let Some(ohpm_home) = std::env::var_os("OHPM_HOME") {
        let p = PathBuf::from(ohpm_home);
        if p.is_dir() {
            return Some(p);
        }
    }
    // Try $HOME/.ohpm
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    let p = home.join(".ohpm");
    // Return the path even if it does not yet exist — callers check existence.
    Some(p)
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
                "dependencies": {
                    "some-dep": "^1.0.0"
                }
            },
            "@scope/my-lib@2.3.4": {
                "version": "2.3.4",
                "resolved": "https://registry.ohpm.openharmony.cn/@scope/my-lib",
                "integrity": null,
                "dependencies": {}
            }
        }
    }"#;

    #[test]
    fn test_parse_sample_lockfile() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert!(!lf.is_empty());
        assert_eq!(lf.entries.len(), 3);
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
        assert_eq!(lf.resolved_version("@scope/my-lib"), Some("2.3.4"));
    }

    #[test]
    fn test_resolved_version_missing_package() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        assert_eq!(lf.resolved_version("nonexistent"), None);
    }

    #[test]
    fn test_get_entry_has_resolved_url() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        let entry = lf.get("@ohos/crypto-js").expect("entry present");
        assert_eq!(entry.version, "1.0.0");
        assert_eq!(
            entry.resolved.as_deref(),
            Some("https://registry.ohpm.openharmony.cn/@ohos/crypto-js")
        );
        assert_eq!(entry.integrity.as_deref(), Some("sha512-abc123=="));
    }

    #[test]
    fn test_get_entry_with_dependencies() {
        let lf = OhPackageLockfile::parse(SAMPLE_LOCKFILE);
        let entry = lf.get("lodash").expect("lodash entry present");
        assert_eq!(entry.dependencies, vec!["some-dep".to_string()]);
    }

    #[test]
    fn test_empty_lockfile_on_invalid_json() {
        let lf = OhPackageLockfile::parse("this is not json");
        assert!(lf.is_empty());
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let tmp = std::env::temp_dir().join("arkdown_no_lockfile_dir_xyz");
        std::fs::create_dir_all(&tmp).unwrap();
        let lf = OhPackageLockfile::load(&tmp);
        assert!(lf.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_present_file() {
        use std::io::Write;
        let tmp = std::env::temp_dir().join("arkdown_lockfile_test_xyz");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut f = std::fs::File::create(tmp.join("oh-package-lock.json5")).unwrap();
        f.write_all(SAMPLE_LOCKFILE.as_bytes()).unwrap();

        let lf = OhPackageLockfile::load(&tmp);
        assert_eq!(lf.resolved_version("lodash"), Some("4.17.21"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_strip_version_suffix_plain() {
        assert_eq!(strip_version_suffix("lodash@4.17.21"), "lodash");
        assert_eq!(strip_version_suffix("some-pkg@0.1.2"), "some-pkg");
    }

    #[test]
    fn test_strip_version_suffix_scoped() {
        assert_eq!(
            strip_version_suffix("@ohos/crypto-js@1.0.0"),
            "@ohos/crypto-js"
        );
        assert_eq!(strip_version_suffix("@scope/my-lib@2.3.4"), "@scope/my-lib");
    }

    #[test]
    fn test_strip_version_suffix_no_version() {
        // No version suffix — returns the key unchanged.
        assert_eq!(strip_version_suffix("some-pkg"), "some-pkg");
    }

    #[test]
    fn test_lockfile_missing_meta_field_is_ok() {
        // `meta` is optional — a lockfile without it should still parse.
        let content = r#"{
            "packages": {
                "foo@1.2.3": {
                    "version": "1.2.3",
                    "dependencies": {}
                }
            }
        }"#;
        let lf = OhPackageLockfile::parse(content);
        assert_eq!(lf.resolved_version("foo"), Some("1.2.3"));
    }

    #[test]
    fn test_lockfile_empty_packages_is_ok() {
        let content = r#"{ "meta": {}, "packages": {} }"#;
        let lf = OhPackageLockfile::parse(content);
        assert!(lf.is_empty());
    }
}
