//! OHPM package registry client.
//!
//! Implements the remote-fetch side of hvigor's `DependencyManager`: when a
//! package is not found in `oh_modules/` or the OHPM global cache, this module
//! fetches it from the OHPM registry, verifies the tarball integrity, and
//! installs it into the global cache so `LocalDependencyGraph` can discover it.
//!
//! ## Registry API (npm-compatible)
//!
//! The OHPM registry at `https://registry.ohpm.openharmony.cn` speaks the npm
//! registry protocol (CommonJS package format):
//!
//! - `GET /pkg-name` — package metadata (all versions)
//! - `GET /@scope/pkg-name` — scoped package metadata
//! - `GET /@scope/pkg-name/-/@scope/pkg-name-1.0.0.tgz` — tarball download
//!
//! ## Cache layout
//!
//! Downloaded packages are installed to the OHPM global cache (`~/.ohpm/`):
//!
//! ```text
//! ~/.ohpm/
//!   @ohos/crypto-js@1.0.0/
//!     oh_modules/
//!       @ohos/crypto-js/
//!         oh-package.json5
//!         src/
//!         ...
//! ```
//!
//! This mirrors the layout that `ohpm install` produces, making installed
//! packages discoverable by `LocalDependencyGraph::build()`.
//!
//! ## Hvigor alignment
//!
//! hvigor's `DependencyManager` classifies remote packages as `RemoteOrUnknown`
//! until they are downloaded, then reclassifies them by inspecting the installed
//! package structure.  This module implements the download-and-install step so
//! that subsequent `LocalDependencyGraph` builds see the package as a local node.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Default OHPM registry URL.
pub const DEFAULT_REGISTRY: &str = "https://registry.ohpm.openharmony.cn";

// ---------------------------------------------------------------------------
// Offline cache support
// ---------------------------------------------------------------------------

/// Options controlling the install behaviour of [`OhpmRegistryClient::install`].
///
/// These options mirror the `--offline` / `--prefer-offline` flags that `npm`
/// and `ohpm` CLIs expose, allowing builds to succeed in air-gapped
/// environments where network access is unavailable or unreliable.
#[derive(Debug, Clone, Default)]
pub struct OhpmInstallOptions {
    /// When `true`, only the local cache is consulted.  If the requested
    /// package version is not present in the cache the install fails
    /// immediately without making any network requests.
    ///
    /// Equivalent to `ohpm install --offline`.
    pub offline: bool,

    /// When `true`, the cache is checked first.  If the package is already
    /// cached, the cached copy is used without hitting the network.  Only when
    /// the package is absent from the cache is a network download attempted.
    ///
    /// Equivalent to `ohpm install --prefer-offline`.
    ///
    /// If both `offline` and `prefer_offline` are `true`, `offline` takes
    /// precedence (no network requests are ever made).
    pub prefer_offline: bool,

    /// Override the registry URL for this install operation.
    ///
    /// When `None`, the registry URL configured on the [`OhpmRegistryClient`]
    /// is used (which itself falls back to `$OHPM_REGISTRY`).
    pub registry_url: Option<String>,
}

/// Offline OHPM package cache utilities.
///
/// Provides helpers for querying the on-disk OHPM global cache without
/// making any network requests.  Suitable for use in air-gapped build
/// environments or for implementing `--prefer-offline` install strategies.
///
/// ## Cache layout
///
/// The OHPM global cache (`~/.ohpm/` by default, overridden by `$OHPM_HOME`)
/// stores packages as:
///
/// ```text
/// ~/.ohpm/
///   <name>@<version>/
///     oh_modules/
///       <name>/
///         oh-package.json5
///         ...
///   @scope+name@<version>/        ← scoped packages use '+' instead of '/'
///     oh_modules/
///       @scope/name/
///         ...
/// ```
pub struct OfflineOhpmCache;

impl OfflineOhpmCache {
    /// Resolve the on-disk path for `name@version` using the lockfile.
    ///
    /// Looks up `version` from `lockfile` when `version` is empty or
    /// `"*"`; otherwise uses the provided `version` string directly.
    ///
    /// Returns `Some(path)` when the package directory exists in the OHPM
    /// global cache, `None` otherwise.
    pub fn resolve_from_lockfile(
        lockfile: &crate::lockfile::OhPackageLockfile,
        name: &str,
        version: &str,
        cache_root: &Path,
    ) -> Option<PathBuf> {
        // Determine the concrete version: use the lockfile entry when the
        // caller passes an empty/wildcard version.
        let resolved_version: String = if version.is_empty() || version == "*" {
            lockfile.resolved_version(name)?.to_string()
        } else {
            version.to_string()
        };

        // Standard path: `<cache_root>/<name>@<version>/oh_modules/<name>/`
        let candidate = cache_dir_for(name, &resolved_version, cache_root)
            .join("oh_modules")
            .join(name);
        if candidate.is_dir() {
            return Some(std::fs::canonicalize(&candidate).unwrap_or(candidate));
        }

        None
    }

    /// Return all cached version strings for `name` under `cache_root`.
    ///
    /// Scans the cache root directory for entries matching the pattern
    /// `<name>@<version>` (plain packages) or `<scope>+<pkg>@<version>`
    /// (scoped packages) and returns the version portion of each match.
    ///
    /// Returns an empty `Vec` when no cached versions are found or when
    /// `cache_root` does not exist.
    pub fn list_cached_versions(name: &str, cache_root: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(cache_root) else {
            return vec![];
        };

        // Build the prefix we expect to see before the `@<version>` suffix.
        // For `@scope/pkg` the disk prefix is `@scope+pkg`.
        let disk_prefix = if name.starts_with('@') {
            format!("{}@", name.replacen('/', "+", 1))
        } else {
            format!("{}@", name)
        };

        let mut versions = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let dir_name = file_name.to_string_lossy();
            if let Some(ver) = dir_name.strip_prefix(&disk_prefix) {
                // Verify the package directory actually exists inside.
                let pkg_path = entry.path().join("oh_modules").join(name);
                if pkg_path.is_dir() {
                    versions.push(ver.to_string());
                }
            }
        }
        versions.sort();
        versions
    }

    /// Return `true` when `name@version` is present in `cache_root`.
    ///
    /// This is a cheap existence check: it only verifies that the expected
    /// directory path exists without reading any package metadata.
    pub fn is_cached(name: &str, version: &str, cache_root: &Path) -> bool {
        let pkg_path = cache_dir_for(name, version, cache_root)
            .join("oh_modules")
            .join(name);
        pkg_path.is_dir()
    }
}

/// OHPM registry client.
///
/// Uses `reqwest::blocking::Client` for synchronous HTTP requests, matching
/// the synchronous nature of the `LocalDependencyGraph` build phase.
pub struct OhpmRegistryClient {
    registry_url: String,
    client: reqwest::blocking::Client,
}

impl OhpmRegistryClient {
    /// Create a client using the default OHPM registry.
    pub fn new() -> Result<Self, OhpmRegistryError> {
        Self::with_registry(DEFAULT_REGISTRY)
    }

    /// Create a client using a custom registry URL.
    ///
    /// Reads `$OHPM_REGISTRY` env var if no explicit URL is given.
    pub fn with_registry(registry_url: &str) -> Result<Self, OhpmRegistryError> {
        let url = std::env::var("OHPM_REGISTRY").unwrap_or_else(|_| registry_url.to_string());
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("arkdown/0.1 (OHOS bundler; compatible with ohpm)")
            .build()
            .map_err(OhpmRegistryError::Http)?;
        Ok(Self {
            registry_url: url,
            client,
        })
    }

    /// Fetch the package metadata for `name` from the registry.
    ///
    /// Returns the full npm-style package document containing all published
    /// versions and their dist metadata (tarball URL, integrity hash).
    pub fn fetch_metadata(&self, name: &str) -> Result<PackageMetadata, OhpmRegistryError> {
        let url = format!(
            "{}/{}",
            self.registry_url.trim_end_matches('/'),
            encode_package_name(name)
        );
        tracing::debug!(url = %url, "ohpm: fetching package metadata");

        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(OhpmRegistryError::Http)?;

        if !resp.status().is_success() {
            return Err(OhpmRegistryError::NotFound {
                name: name.to_string(),
                status: resp.status().as_u16(),
            });
        }

        resp.json::<PackageMetadata>()
            .map_err(OhpmRegistryError::Http)
    }

    /// Resolve the tarball URL and integrity hash for `name@version`.
    pub fn resolve_version(
        &self,
        name: &str,
        version: &str,
    ) -> Result<VersionInfo, OhpmRegistryError> {
        let meta = self.fetch_metadata(name)?;
        let ver = meta
            .versions
            .get(version)
            .ok_or_else(|| OhpmRegistryError::VersionNotFound {
                name: name.to_string(),
                version: version.to_string(),
            })?;
        Ok(ver.clone())
    }

    /// Download and install `name@version` into the OHPM global cache.
    ///
    /// After this call, `LocalDependencyGraph` can discover the package via
    /// the global cache path (`~/.ohpm/<name>@<version>/oh_modules/<name>/`).
    ///
    /// Returns the path to the installed package root.
    ///
    /// # Options
    ///
    /// Pass [`OhpmInstallOptions`] to control offline behaviour:
    ///
    /// - `offline = true` — check the cache first; return
    ///   [`OhpmRegistryError::PackageNotCached`] without any network request
    ///   when the package is absent.
    /// - `prefer_offline = true` — use the cached copy when available,
    ///   otherwise fall back to a normal network download.
    /// - `registry_url` — override the registry URL for this request.
    pub fn install(
        &self,
        name: &str,
        version: &str,
        cache_root: &Path,
        options: &OhpmInstallOptions,
    ) -> Result<PathBuf, OhpmRegistryError> {
        // Offline mode: check cache only, never hit the network.
        if options.offline {
            if OfflineOhpmCache::is_cached(name, version, cache_root) {
                let install_root = cache_dir_for(name, version, cache_root)
                    .join("oh_modules")
                    .join(name);
                tracing::debug!(
                    name = %name,
                    version = %version,
                    "ohpm: offline mode — using cached package"
                );
                return Ok(install_root);
            }
            return Err(OhpmRegistryError::PackageNotCached {
                name: name.to_string(),
                version: version.to_string(),
                cache_root: cache_root.to_path_buf(),
            });
        }

        // prefer_offline: use cache if available, otherwise download.
        if options.prefer_offline && OfflineOhpmCache::is_cached(name, version, cache_root) {
            let install_root = cache_dir_for(name, version, cache_root)
                .join("oh_modules")
                .join(name);
            tracing::debug!(
                name = %name,
                version = %version,
                "ohpm: prefer-offline — using cached package"
            );
            return Ok(install_root);
        }

        // Determine the effective registry URL.
        let effective_registry = options
            .registry_url
            .as_deref()
            .unwrap_or(&self.registry_url);

        // Resolve version metadata from the registry.
        let ver_info = self.resolve_version_from(name, version, effective_registry)?;
        let tarball_url = &ver_info.dist.tarball;

        tracing::info!(
            name = %name,
            version = %version,
            url = %tarball_url,
            "ohpm: downloading package"
        );

        // Download the tarball.
        let mut resp = self
            .client
            .get(tarball_url)
            .send()
            .map_err(OhpmRegistryError::Http)?;

        if !resp.status().is_success() {
            return Err(OhpmRegistryError::DownloadFailed {
                url: tarball_url.clone(),
                status: resp.status().as_u16(),
            });
        }

        let mut tarball_bytes = Vec::new();
        resp.read_to_end(&mut tarball_bytes)
            .map_err(|e| OhpmRegistryError::Io(std::io::Error::other(e)))?;

        // Verify integrity if provided.
        if let Some(integrity) = &ver_info.dist.integrity {
            verify_integrity(integrity, &tarball_bytes, name, version)?;
        }

        // Install into cache: ~/.ohpm/<name>@<version>/oh_modules/<name>/
        let pkg_dir = cache_dir_for(name, version, cache_root);
        let install_root = pkg_dir.join("oh_modules").join(name);
        fs_err::create_dir_all(&install_root).map_err(|e| OhpmRegistryError::Io(e.into()))?;

        // Extract the tarball (gzip-compressed tar archive).
        extract_tgz(&tarball_bytes, &install_root)?;

        tracing::info!(
            name = %name,
            version = %version,
            path = %install_root.display(),
            "ohpm: package installed to global cache"
        );

        Ok(install_root)
    }

    /// Install `name@version` using an offline-first strategy.
    ///
    /// This is a convenience wrapper that:
    /// 1. Returns the cached path immediately when the package is already
    ///    present in `cache_root`.
    /// 2. Downloads and installs the package when not cached.
    ///
    /// This is equivalent to calling `install` with
    /// `OhpmInstallOptions { prefer_offline: true, .. }`.
    pub fn install_or_use_cache(
        &self,
        name: &str,
        version: &str,
        cache_root: &Path,
    ) -> Result<PathBuf, OhpmRegistryError> {
        self.install(
            name,
            version,
            cache_root,
            &OhpmInstallOptions {
                prefer_offline: true,
                ..Default::default()
            },
        )
    }

    /// Resolve version info from an explicit registry URL.
    ///
    /// Used internally to support the `registry_url` override in
    /// [`OhpmInstallOptions`].
    fn resolve_version_from(
        &self,
        name: &str,
        version: &str,
        registry_url: &str,
    ) -> Result<VersionInfo, OhpmRegistryError> {
        let url = format!(
            "{}/{}",
            registry_url.trim_end_matches('/'),
            encode_package_name(name)
        );
        tracing::debug!(url = %url, "ohpm: fetching package metadata (override registry)");

        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(OhpmRegistryError::Http)?;

        if !resp.status().is_success() {
            return Err(OhpmRegistryError::NotFound {
                name: name.to_string(),
                status: resp.status().as_u16(),
            });
        }

        let meta = resp
            .json::<PackageMetadata>()
            .map_err(OhpmRegistryError::Http)?;

        let ver = meta
            .versions
            .get(version)
            .ok_or_else(|| OhpmRegistryError::VersionNotFound {
                name: name.to_string(),
                version: version.to_string(),
            })?;
        Ok(ver.clone())
    }
}

/// Compute the cache directory path for `name@version`.
///
/// Matches the path layout that `ohpm install` uses, so the result is
/// discoverable by `LocalDependencyGraph` / `OhPackageLockfile`.
pub fn cache_dir_for(name: &str, version: &str, cache_root: &Path) -> PathBuf {
    // Scoped packages (@scope/name) are stored as @scope+name on disk.
    let dir_name = if name.starts_with('@') {
        format!("{}@{}", name.replacen('/', "+", 1), version)
    } else {
        format!("{}@{}", name, version)
    };
    cache_root.join(dir_name)
}

// ---------------------------------------------------------------------------
// Registry response types
// ---------------------------------------------------------------------------

/// npm-registry-compatible package metadata document.
#[derive(Debug, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// All published versions keyed by version string.
    #[serde(default)]
    pub versions: indexmap::IndexMap<String, VersionInfo>,
    /// Distribution tags (`latest`, `next`, etc.).
    #[serde(rename = "dist-tags", default)]
    pub dist_tags: indexmap::IndexMap<String, String>,
}

impl PackageMetadata {
    /// Return the `latest` version string.
    pub fn latest_version(&self) -> Option<&str> {
        self.dist_tags.get("latest").map(|s| s.as_str())
    }
}

/// Metadata for a single package version.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub dist: DistInfo,
}

/// Distribution info for one version.
#[derive(Debug, Clone, Deserialize)]
pub struct DistInfo {
    /// URL of the `.tgz` tarball.
    pub tarball: String,
    /// SRI integrity string (e.g. `sha512-<base64>`).
    pub integrity: Option<String>,
    /// SHA-1 sum (legacy npm field, used as fallback).
    pub shasum: Option<String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from the OHPM registry client.
#[derive(Debug, thiserror::Error)]
pub enum OhpmRegistryError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Package '{name}' not found in registry (HTTP {status})")]
    NotFound { name: String, status: u16 },

    #[error("Version '{version}' of package '{name}' not found in registry")]
    VersionNotFound { name: String, version: String },

    #[error("Download failed for '{url}' (HTTP {status})")]
    DownloadFailed { url: String, status: u16 },

    #[error("Integrity check failed for {name}@{version}: expected {expected}, got {actual}")]
    IntegrityMismatch {
        name: String,
        version: String,
        expected: String,
        actual: String,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tarball extraction error: {0}")]
    TarExtract(String),

    /// Returned when `offline = true` and the package is not present in the
    /// local cache.  No network request was made.
    #[error(
        "Package '{name}@{version}' is not cached in '{cache_root}' and offline mode is enabled"
    )]
    PackageNotCached {
        name: String,
        version: String,
        cache_root: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Encode a package name for use in a registry URL.
/// Scoped names like `@ohos/foo` become `@ohos%2Ffoo` in the path.
fn encode_package_name(name: &str) -> String {
    if let Some(stripped) = name.strip_prefix('@') {
        // "@scope/pkg" → "%40scope%2Fpkg" for the registry path
        // Actually npm convention: "%40scope%2Fpkg" is wrong; the standard is
        // to keep the leading '@' as-is and encode only the '/'.
        let encoded = stripped.replacen('/', "%2F", 1);
        format!("@{}", encoded)
    } else {
        name.to_string()
    }
}

/// Verify tarball integrity using the SRI `sha512-<base64>` or `sha256-<base64>` format.
///
/// This is the same integrity format used by npm/ohpm registries and matches
/// the `integrity` field in `oh-package-lock.json5`.
fn verify_integrity(
    integrity: &str,
    bytes: &[u8],
    name: &str,
    version: &str,
) -> Result<(), OhpmRegistryError> {
    use sha2::Digest;

    let (algo, expected_b64) = match integrity.split_once('-') {
        Some(pair) => pair,
        None => return Ok(()), // unknown format, skip verification
    };

    // Compute the actual digest.
    let actual_bytes: Vec<u8> = match algo {
        "sha512" => {
            let mut hasher = sha2::Sha512::new();
            hasher.update(bytes);
            hasher.finalize().to_vec()
        }
        "sha256" => {
            let mut hasher = sha2::Sha256::new();
            hasher.update(bytes);
            hasher.finalize().to_vec()
        }
        _ => return Ok(()), // unknown algo, skip
    };

    // Encode actual as base64 and compare.
    let actual_b64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &actual_bytes);

    if actual_b64 != expected_b64 {
        return Err(OhpmRegistryError::IntegrityMismatch {
            name: name.to_string(),
            version: version.to_string(),
            expected: expected_b64.to_string(),
            actual: actual_b64,
        });
    }

    Ok(())
}

/// Extract a gzip-compressed tar archive to `dest`.
/// Extract a gzip-compressed tar archive to `dest`, stripping the first path
/// component.
///
/// OHPM (like npm) tarballs contain a top-level `package/` directory:
/// ```text
/// package/oh-package.json5
/// package/src/main/module.json5
/// ```
/// We strip that prefix so that `oh-package.json5` is placed directly in
/// `dest/`, matching the directory layout expected by
/// `LocalDependencyGraph::classify_package()`.
fn extract_tgz(bytes: &[u8], dest: &Path) -> Result<(), OhpmRegistryError> {
    use std::io::Cursor;
    use std::path::PathBuf;

    let cursor = Cursor::new(bytes);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);

    for entry in archive
        .entries()
        .map_err(|e| OhpmRegistryError::TarExtract(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| OhpmRegistryError::TarExtract(e.to_string()))?;
        let raw_path = entry
            .path()
            .map_err(|e| OhpmRegistryError::TarExtract(e.to_string()))?
            .to_path_buf();

        // Strip the leading `package/` component (npm/ohpm tarball convention).
        let stripped: PathBuf = raw_path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() {
            continue; // skip the directory entry itself
        }

        let out_path = dest.join(&stripped);
        if let Some(parent) = out_path.parent() {
            fs_err::create_dir_all(parent).map_err(|e| OhpmRegistryError::Io(e.into()))?;
        }
        entry
            .unpack(&out_path)
            .map_err(|e| OhpmRegistryError::TarExtract(e.to_string()))?;
    }

    Ok(())
}

/// Compute the SHA-256 hex digest of `data` using the `sha2` crate.
#[cfg(test)]
fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in result.iter() {
        use std::fmt::Write as _;
        write!(&mut hex, "{:02x}", b).unwrap();
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_package_name_plain() {
        assert_eq!(encode_package_name("my-package"), "my-package");
    }

    #[test]
    fn test_encode_package_name_scoped() {
        assert_eq!(encode_package_name("@ohos/crypto-js"), "@ohos%2Fcrypto-js");
    }

    #[test]
    fn test_cache_dir_for_plain() {
        let root = Path::new("/home/user/.ohpm");
        let dir = cache_dir_for("my-pkg", "1.0.0", root);
        assert_eq!(dir, root.join("my-pkg@1.0.0"));
    }

    #[test]
    fn test_cache_dir_for_scoped() {
        let root = Path::new("/home/user/.ohpm");
        let dir = cache_dir_for("@ohos/crypto-js", "1.0.0", root);
        assert_eq!(dir, root.join("@ohos+crypto-js@1.0.0"));
    }

    #[test]
    fn test_sha256_produces_64_hex_chars() {
        // SHA-256 always produces 32 bytes = 64 hex chars.
        assert_eq!(sha256_hex(b"").len(), 64);
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }

    #[test]
    fn test_sha256_different_inputs_produce_different_outputs() {
        // Basic collision resistance: different inputs must produce different digests.
        let h1 = sha256_hex(b"");
        let h2 = sha256_hex(b"abc");
        let h3 = sha256_hex(b"abd");
        assert_ne!(h1, h2, "SHA-256('') != SHA-256('abc')");
        assert_ne!(h2, h3, "SHA-256('abc') != SHA-256('abd')");
    }

    #[test]
    fn test_sha256_same_input_stable() {
        // SHA-256 is deterministic: same input always yields same output.
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"hello");
        assert_eq!(a, b, "SHA-256 must be deterministic");
    }

    #[test]
    fn test_sha256_empty_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // This is a universally-known test vector (FIPS 180-4).
        let hex = sha256_hex(b"");
        assert_eq!(
            hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_package_metadata_deserialization() {
        let json = r#"{
            "name": "@ohos/crypto-js",
            "description": "Crypto library",
            "versions": {
                "1.0.0": {
                    "version": "1.0.0",
                    "description": "Crypto library",
                    "dist": {
                        "tarball": "https://registry.ohpm.openharmony.cn/@ohos/crypto-js/-/@ohos/crypto-js-1.0.0.tgz",
                        "integrity": "sha512-abc123",
                        "shasum": "deadbeef"
                    }
                }
            },
            "dist-tags": {
                "latest": "1.0.0"
            }
        }"#;
        let meta: PackageMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.name, "@ohos/crypto-js");
        assert_eq!(meta.latest_version(), Some("1.0.0"));
        let ver = &meta.versions["1.0.0"];
        assert_eq!(
            ver.dist.tarball,
            "https://registry.ohpm.openharmony.cn/@ohos/crypto-js/-/@ohos/crypto-js-1.0.0.tgz"
        );
        assert_eq!(ver.dist.integrity.as_deref(), Some("sha512-abc123"));
    }

    // -------------------------------------------------------------------------
    // OfflineOhpmCache tests
    // -------------------------------------------------------------------------

    /// Helper: create a fake cached package directory under `cache_root`.
    fn make_fake_cached_pkg(cache_root: &Path, name: &str, version: &str) -> PathBuf {
        let pkg_dir = cache_dir_for(name, version, cache_root)
            .join("oh_modules")
            .join(name);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        // Write a minimal oh-package.json5 so the directory looks like a real package.
        std::fs::write(
            pkg_dir.join("oh-package.json5"),
            format!(r#"{{ "name": "{name}", "version": "{version}" }}"#),
        )
        .unwrap();
        pkg_dir
    }

    #[test]
    fn test_is_cached_returns_true_when_package_exists() {
        let tmp = tempfile::tempdir().unwrap();
        make_fake_cached_pkg(tmp.path(), "my-lib", "1.2.3");
        assert!(OfflineOhpmCache::is_cached("my-lib", "1.2.3", tmp.path()));
    }

    #[test]
    fn test_is_cached_returns_false_when_package_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!OfflineOhpmCache::is_cached(
            "nonexistent",
            "1.0.0",
            tmp.path()
        ));
    }

    #[test]
    fn test_is_cached_scoped_package() {
        let tmp = tempfile::tempdir().unwrap();
        make_fake_cached_pkg(tmp.path(), "@ohos/crypto-js", "2.0.0");
        assert!(OfflineOhpmCache::is_cached(
            "@ohos/crypto-js",
            "2.0.0",
            tmp.path()
        ));
        // A different version must not be reported as cached.
        assert!(!OfflineOhpmCache::is_cached(
            "@ohos/crypto-js",
            "1.0.0",
            tmp.path()
        ));
    }

    #[test]
    fn test_list_cached_versions_returns_all_installed_versions() {
        let tmp = tempfile::tempdir().unwrap();
        make_fake_cached_pkg(tmp.path(), "my-pkg", "1.0.0");
        make_fake_cached_pkg(tmp.path(), "my-pkg", "1.1.0");
        make_fake_cached_pkg(tmp.path(), "my-pkg", "2.0.0");
        // Unrelated package — must not appear in results.
        make_fake_cached_pkg(tmp.path(), "other-pkg", "1.0.0");

        let mut versions = OfflineOhpmCache::list_cached_versions("my-pkg", tmp.path());
        versions.sort();
        assert_eq!(versions, vec!["1.0.0", "1.1.0", "2.0.0"]);
    }

    #[test]
    fn test_list_cached_versions_empty_when_none_cached() {
        let tmp = tempfile::tempdir().unwrap();
        let versions = OfflineOhpmCache::list_cached_versions("nonexistent", tmp.path());
        assert!(versions.is_empty());
    }

    #[test]
    fn test_list_cached_versions_scoped_package() {
        let tmp = tempfile::tempdir().unwrap();
        make_fake_cached_pkg(tmp.path(), "@ohos/util", "1.0.0");
        make_fake_cached_pkg(tmp.path(), "@ohos/util", "1.1.0");
        // Different scoped package — must not appear.
        make_fake_cached_pkg(tmp.path(), "@ohos/other", "1.0.0");

        let mut versions = OfflineOhpmCache::list_cached_versions("@ohos/util", tmp.path());
        versions.sort();
        assert_eq!(versions, vec!["1.0.0", "1.1.0"]);
    }

    #[test]
    fn test_resolve_from_lockfile_returns_path_when_cached() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg_path = make_fake_cached_pkg(tmp.path(), "@ohos/crypto-js", "1.0.0");

        let lockfile_content = r#"{
            "packages": {
                "@ohos/crypto-js@1.0.0": {
                    "version": "1.0.0",
                    "resolved": "https://registry.ohpm.openharmony.cn/@ohos/crypto-js",
                    "dependencies": {}
                }
            }
        }"#;
        let lockfile = crate::lockfile::OhPackageLockfile::parse(lockfile_content);

        let result = OfflineOhpmCache::resolve_from_lockfile(
            &lockfile,
            "@ohos/crypto-js",
            "1.0.0",
            tmp.path(),
        );
        assert!(result.is_some(), "should resolve cached package");
        // Resolved path must point to the package directory.
        let resolved = result.unwrap();
        // Canonicalize both for comparison (macOS /tmp → /private/tmp).
        let canon_resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        let canon_expected = std::fs::canonicalize(&pkg_path).unwrap_or(pkg_path);
        assert_eq!(canon_resolved, canon_expected);
    }

    #[test]
    fn test_resolve_from_lockfile_returns_none_when_not_cached() {
        let tmp = tempfile::tempdir().unwrap();
        // No package on disk.
        let lockfile_content = r#"{
            "packages": {
                "some-pkg@1.0.0": {
                    "version": "1.0.0",
                    "dependencies": {}
                }
            }
        }"#;
        let lockfile = crate::lockfile::OhPackageLockfile::parse(lockfile_content);
        let result =
            OfflineOhpmCache::resolve_from_lockfile(&lockfile, "some-pkg", "1.0.0", tmp.path());
        assert!(
            result.is_none(),
            "should return None when package not on disk"
        );
    }

    #[test]
    fn test_resolve_from_lockfile_uses_lockfile_version_when_wildcard() {
        let tmp = tempfile::tempdir().unwrap();
        // Package is cached at 2.3.4 — lockfile records that version.
        make_fake_cached_pkg(tmp.path(), "my-lib", "2.3.4");

        let lockfile_content = r#"{
            "packages": {
                "my-lib@2.3.4": {
                    "version": "2.3.4",
                    "dependencies": {}
                }
            }
        }"#;
        let lockfile = crate::lockfile::OhPackageLockfile::parse(lockfile_content);

        // Pass "*" as version — should resolve via lockfile.
        let result = OfflineOhpmCache::resolve_from_lockfile(&lockfile, "my-lib", "*", tmp.path());
        assert!(
            result.is_some(),
            "wildcard version should resolve via lockfile"
        );
    }

    #[test]
    fn test_offline_install_errors_when_not_cached() {
        let tmp = tempfile::tempdir().unwrap();
        // No network — use a client pointing at a non-existent registry.
        // With offline=true the network must never be reached, so the registry
        // URL is irrelevant; we only care about the error variant.
        let client = OhpmRegistryClient::with_registry("http://127.0.0.1:0").unwrap();
        let opts = OhpmInstallOptions {
            offline: true,
            ..Default::default()
        };
        let result = client.install("missing-pkg", "1.0.0", tmp.path(), &opts);
        assert!(
            matches!(result, Err(OhpmRegistryError::PackageNotCached { .. })),
            "offline install of uncached package must return PackageNotCached, got: {:?}",
            result
        );
    }

    #[test]
    fn test_offline_install_returns_cached_path_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = make_fake_cached_pkg(tmp.path(), "my-pkg", "1.0.0");

        // Client with a bogus registry URL — offline mode must not hit it.
        let client = OhpmRegistryClient::with_registry("http://127.0.0.1:0").unwrap();
        let opts = OhpmInstallOptions {
            offline: true,
            ..Default::default()
        };
        let result = client.install("my-pkg", "1.0.0", tmp.path(), &opts);
        assert!(
            result.is_ok(),
            "offline install of cached package must succeed"
        );
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_prefer_offline_returns_cached_path_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = make_fake_cached_pkg(tmp.path(), "@ohos/util", "3.0.0");

        // Bogus registry — prefer_offline must use the cache and never reach it.
        let client = OhpmRegistryClient::with_registry("http://127.0.0.1:0").unwrap();
        let opts = OhpmInstallOptions {
            prefer_offline: true,
            ..Default::default()
        };
        let result = client.install("@ohos/util", "3.0.0", tmp.path(), &opts);
        assert!(
            result.is_ok(),
            "prefer_offline must succeed when package is cached"
        );
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_install_or_use_cache_returns_cached_path() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = make_fake_cached_pkg(tmp.path(), "fast-lib", "0.9.0");

        let client = OhpmRegistryClient::with_registry("http://127.0.0.1:0").unwrap();
        let result = client.install_or_use_cache("fast-lib", "0.9.0", tmp.path());
        assert!(
            result.is_ok(),
            "install_or_use_cache must succeed for cached package"
        );
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_ohpm_install_options_default() {
        // Default options must have all flags false and no registry override.
        let opts = OhpmInstallOptions::default();
        assert!(!opts.offline);
        assert!(!opts.prefer_offline);
        assert!(opts.registry_url.is_none());
    }

    #[test]
    fn test_offline_flag_takes_precedence_over_prefer_offline() {
        // When both offline and prefer_offline are true, offline wins:
        // if not cached → PackageNotCached (no network).
        let tmp = tempfile::tempdir().unwrap();
        let client = OhpmRegistryClient::with_registry("http://127.0.0.1:0").unwrap();
        let opts = OhpmInstallOptions {
            offline: true,
            prefer_offline: true,
            ..Default::default()
        };
        let result = client.install("absent-pkg", "1.0.0", tmp.path(), &opts);
        assert!(
            matches!(result, Err(OhpmRegistryError::PackageNotCached { .. })),
            "offline=true must take precedence over prefer_offline=true"
        );
    }
}
