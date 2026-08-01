//! `OhpmResolver`, `PackageAcquisitionRequest`, and `PackageAcquisitionResult`.
//!
//! This module is the primary entry point for `arkdown-session` to acquire
//! packages. The resolver orchestrates cache lookup, lockfile resolution, and
//! registry fetch/install according to the configured [`OhpmResolutionMode`].
//!
//! # Resolution policy
//!
//! | Mode                            | Policy                                                    |
//! |---------------------------------|-----------------------------------------------------------|
//! | [`OhpmResolutionMode::Offline`] | Only look in local `oh_modules/` and OHPM cache. Never fetch. |
//! | [`OhpmResolutionMode::PreferOffline`] | Cache before network; only fetch if cache misses. |
//! | [`OhpmResolutionMode::Online`]  | Always contact registry for version resolution.           |

use std::path::PathBuf;

use arkdown_path::canonicalize_or_owned;

use crate::cache::{lookup_via_lockfile, ohpm_cache_root};
use crate::error::OhpmError;
use crate::integrity;
use crate::lockfile::OhPackageLockfile;
use crate::package_name::PackageName;
use crate::registry::OhpmRegistryClient;
use crate::tarball::extract_tgz;

/// OHPM resolution mode, controlling offline/online behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OhpmResolutionMode {
    /// Only read from `oh_modules/` and the OHPM cache. Never make HTTP requests.
    Offline,
    /// Use OHPM cache before attempting network fetch.
    PreferOffline,
    /// Always contact the registry for fresh version resolution.
    Online,
}

/// A request to acquire one package.
#[derive(Debug, Clone)]
pub struct PackageAcquisitionRequest {
    /// The package name to acquire.
    pub name: PackageName,
    /// The requested version constraint or pinned version (may be `None` for
    /// `latest`).
    pub version: Option<Box<str>>,
    /// The directory of the package that declares this dependency.
    pub issuer_root: PathBuf,
}

/// The source of an acquired package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageAcquisitionSource {
    /// Found in a local `oh_modules/` directory adjacent to the project.
    LocalOhModules,
    /// Found in the OHPM global cache (offline).
    OhpmCache,
    /// Downloaded from the OHPM registry and installed into the cache.
    Registry,
    /// A local path dependency (`"file:../relative"`).
    LocalPath,
    /// The package could not be located.
    Unknown,
}

/// The result of an acquisition request.
#[derive(Debug, Clone)]
pub struct PackageAcquisitionResult {
    /// The package name.
    pub name: PackageName,
    /// The resolved package root directory, or `None` when not found.
    pub root: Option<PathBuf>,
    /// Where the package came from.
    pub source: PackageAcquisitionSource,
    /// The resolved version string.
    pub version: Option<Box<str>>,
    /// The integrity hash if verified.
    pub integrity: Option<Box<str>>,
}

/// The OHPM package resolver.
///
/// Accepts acquisition requests and returns typed results. Does not classify
/// the dependency graph — that belongs to `arkdown-session`.
#[derive(Debug, Clone)]
pub struct OhpmResolver {
    /// OHPM global cache root (`$OHPM_HOME` or `~/.ohpm`).
    pub cache_root: PathBuf,
    /// Registry URL for online fetches.
    pub registry_url: Option<Box<str>>,
    /// Resolution mode.
    pub mode: OhpmResolutionMode,
}

impl OhpmResolver {
    /// Construct with an explicit cache root and mode.
    pub fn new(
        cache_root: PathBuf,
        registry_url: Option<Box<str>>,
        mode: OhpmResolutionMode,
    ) -> Self {
        Self {
            cache_root,
            registry_url,
            mode,
        }
    }

    /// Construct using the system OHPM home directory.
    ///
    /// # Errors
    ///
    /// Returns [`OhpmError::NoCacheRoot`] if the cache root cannot be
    /// determined.
    pub fn from_env(
        registry_url: Option<Box<str>>,
        mode: OhpmResolutionMode,
    ) -> Result<Self, OhpmError> {
        let cache_root = ohpm_cache_root().ok_or(OhpmError::NoCacheRoot)?;
        Ok(Self::new(cache_root, registry_url, mode))
    }

    /// Resolve a single package acquisition request.
    ///
    /// Resolution order:
    ///
    /// 1. Local `oh_modules/` directory relative to `issuer_root`.
    /// 2. OHPM global cache (using lockfile version when available).
    /// 3. Registry fetch (only if mode is not `Offline`).
    ///
    /// # Errors
    ///
    /// Returns [`OhpmError::OfflineMiss`] in offline mode when the package
    /// is not in cache.
    pub fn resolve(
        &self,
        req: &PackageAcquisitionRequest,
        lockfile: &OhPackageLockfile,
    ) -> Result<PackageAcquisitionResult, OhpmError> {
        // 1. Check local oh_modules/ first.
        let local_candidate = req.issuer_root.join("oh_modules").join(req.name.as_str());
        if local_candidate.is_dir() {
            let root = canonicalize_or_owned(&local_candidate);
            return Ok(PackageAcquisitionResult {
                name: req.name.clone(),
                root: Some(root),
                source: PackageAcquisitionSource::LocalOhModules,
                version: req.version.clone(),
                integrity: None,
            });
        }

        // 2. Check OHPM global cache.
        if let Some(hit) = lookup_via_lockfile(&self.cache_root, &req.name, lockfile) {
            return Ok(PackageAcquisitionResult {
                name: req.name.clone(),
                root: Some(hit.root),
                source: PackageAcquisitionSource::OhpmCache,
                version: Some(hit.version),
                integrity: lockfile
                    .get(req.name.as_str())
                    .and_then(|e| e.integrity.clone()),
            });
        }

        // 3. Registry fetch if not offline.
        match self.mode {
            OhpmResolutionMode::Offline => Err(OhpmError::OfflineMiss {
                name: req.name.to_string(),
                version: req.version.as_deref().unwrap_or("latest").to_string(),
            }),
            OhpmResolutionMode::PreferOffline | OhpmResolutionMode::Online => {
                self.fetch_and_install(req)
            }
        }
    }

    /// Fetch a package from the registry and install it into the OHPM cache.
    fn fetch_and_install(
        &self,
        req: &PackageAcquisitionRequest,
    ) -> Result<PackageAcquisitionResult, OhpmError> {
        let registry_url = self
            .registry_url
            .as_deref()
            .unwrap_or("https://registry.ohpm.openharmony.cn");
        let client = OhpmRegistryClient::new(registry_url);
        let meta = client.fetch_version_metadata(&req.name, req.version.as_deref())?;

        let tarball_data = client.download_tarball(&meta.dist_tarball)?;

        // Verify integrity if provided.
        if let Some(ref int_str) = meta.integrity {
            integrity::verify(&tarball_data, int_str).map_err(|e| {
                OhpmError::IntegrityMismatch {
                    name: req.name.to_string(),
                    version: meta.version.to_string(),
                    expected: int_str.to_string(),
                    actual: e.to_string(),
                }
            })?;
        }

        // Install into cache.
        let dest = crate::cache::cache_path(&self.cache_root, &req.name, &meta.version);
        extract_tgz(&tarball_data, &dest)?;

        Ok(PackageAcquisitionResult {
            name: req.name.clone(),
            root: dest.is_dir().then_some(dest),
            source: PackageAcquisitionSource::Registry,
            version: Some(meta.version),
            integrity: meta.integrity,
        })
    }
}
