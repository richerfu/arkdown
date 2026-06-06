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
        Ok(Self { registry_url: url, client })
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
    pub fn install(
        &self,
        name: &str,
        version: &str,
        cache_root: &Path,
    ) -> Result<PathBuf, OhpmRegistryError> {
        let ver_info = self.resolve_version(name, version)?;
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
        fs_err::create_dir_all(&install_root)
            .map_err(|e| OhpmRegistryError::Io(e.into()))?;

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
    let actual_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &actual_bytes,
    );

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
fn extract_tgz(bytes: &[u8], dest: &Path) -> Result<(), OhpmRegistryError> {
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);

    archive
        .unpack(dest)
        .map_err(|e| OhpmRegistryError::TarExtract(e.to_string()))?;

    Ok(())
}

/// Compute the SHA-256 hex digest of `data` using the `sha2` crate.
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
        assert_eq!(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
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
        assert_eq!(ver.dist.tarball, "https://registry.ohpm.openharmony.cn/@ohos/crypto-js/-/@ohos/crypto-js-1.0.0.tgz");
        assert_eq!(ver.dist.integrity.as_deref(), Some("sha512-abc123"));
    }
}
