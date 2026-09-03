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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use arkdown_path::{canonicalize_or_owned, lexical_normalize};

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
        if let Some(result) = self.resolve_local(req) {
            return Ok(result);
        }
        self.resolve_cached_or_fetch(req, lockfile)
    }

    /// Resolve an issuer-specific local path or `oh_modules` installation.
    ///
    /// This phase never accesses the registry and is separated so session
    /// startup can finish all local lookups before scheduling bounded remote
    /// acquisitions.
    pub fn resolve_local(
        &self,
        req: &PackageAcquisitionRequest,
    ) -> Option<PackageAcquisitionResult> {
        // 1. Resolve local path dependencies before consulting an install
        // tree. The hvigor wrapper runs `ohpm install` first and therefore
        // normally observes a symlink at `oh_modules/<alias>`; resolving the
        // declared path here yields the same canonical package identity
        // without requiring arkdown to mutate the project checkout.
        if let Some(local_path) = req
            .version
            .as_deref()
            .and_then(|specifier| LocalPackageSpecifier::new(specifier).path())
        {
            let candidate = lexical_normalize(&req.issuer_root.join(local_path));
            if candidate.is_dir() {
                return Some(PackageAcquisitionResult {
                    name: req.name.clone(),
                    root: Some(canonicalize_or_owned(&candidate)),
                    source: PackageAcquisitionSource::LocalPath,
                    version: req.version.clone(),
                    integrity: None,
                });
            }
        }

        // 2. Check local oh_modules/ first.
        let local_candidate = req.issuer_root.join("oh_modules").join(req.name.as_str());
        if local_candidate.is_dir() {
            let root = canonicalize_or_owned(&local_candidate);
            return Some(PackageAcquisitionResult {
                name: req.name.clone(),
                root: Some(root),
                source: PackageAcquisitionSource::LocalOhModules,
                version: req.version.clone(),
                integrity: None,
            });
        }

        None
    }

    /// Resolve a package from the global cache or registry after local lookup
    /// has missed. This phase is issuer-independent and may therefore be
    /// deduplicated by package/version by the build-session planner.
    pub fn resolve_cached_or_fetch(
        &self,
        req: &PackageAcquisitionRequest,
        lockfile: &OhPackageLockfile,
    ) -> Result<PackageAcquisitionResult, OhpmError> {
        // 3. Check OHPM global cache. The lockfile is authoritative when it
        // pins this package; an exact manifest version is also a valid cache
        // key for lockfile-free projects.
        if !matches!(self.mode, OhpmResolutionMode::Online) {
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
            if let Some(version) = req
                .version
                .as_deref()
                .filter(|version| semver::Version::parse(version).is_ok())
            {
                if let Some(hit) = crate::cache::lookup_cache(&self.cache_root, &req.name, version)
                {
                    return Ok(PackageAcquisitionResult {
                        name: req.name.clone(),
                        root: Some(hit.root),
                        source: PackageAcquisitionSource::OhpmCache,
                        version: Some(hit.version),
                        integrity: None,
                    });
                }
            }
        }

        // 4. Registry fetch if not offline.
        match self.mode {
            OhpmResolutionMode::Offline => Err(OhpmError::OfflineMiss {
                name: req.name.to_string(),
                version: req.version.as_deref().unwrap_or("latest").to_string(),
            }),
            OhpmResolutionMode::PreferOffline | OhpmResolutionMode::Online => {
                self.fetch_and_install(req, lockfile)
            }
        }
    }

    /// Fetch a package from the registry and install it into the OHPM cache.
    fn fetch_and_install(
        &self,
        req: &PackageAcquisitionRequest,
        lockfile: &OhPackageLockfile,
    ) -> Result<PackageAcquisitionResult, OhpmError> {
        let registry_url = self
            .registry_url
            .as_deref()
            .unwrap_or("https://registry.ohpm.openharmony.cn");
        let client = OhpmRegistryClient::new(registry_url);
        let locked = lockfile.get(req.name.as_str());
        let requested_version = locked
            .map(|entry| entry.version.as_ref())
            .or(req.version.as_deref());
        let meta = client.fetch_version_metadata(&req.name, requested_version)?;

        let expected_integrity = locked
            .and_then(|entry| entry.integrity.as_deref())
            .or(meta.integrity.as_deref());
        let dest = crate::cache::cache_path(&self.cache_root, &req.name, &meta.version);
        if dest.is_dir() {
            return Ok(PackageAcquisitionResult {
                name: req.name.clone(),
                root: Some(canonicalize_or_owned(&dest)),
                source: PackageAcquisitionSource::OhpmCache,
                version: Some(meta.version),
                integrity: expected_integrity
                    .map(str::to_string)
                    .map(String::into_boxed_str),
            });
        }

        let tarball_data = client.download_tarball(&meta.dist_tarball)?;

        // A lockfile integrity value is the reproducibility contract. Registry
        // metadata is used only when no lockfile integrity is available.
        if let Some(int_str) = expected_integrity {
            integrity::verify(&tarball_data, int_str).map_err(|e| {
                OhpmError::IntegrityMismatch {
                    name: req.name.to_string(),
                    version: meta.version.to_string(),
                    expected: int_str.to_string(),
                    actual: e.to_string(),
                }
            })?;
        }

        // Install through a sibling staging directory, then atomically publish
        // the complete package root. Concurrent processes may race, but never
        // observe a partially extracted cache entry.
        let staging = CacheInstallStagingPath::new(&dest).path();
        extract_tgz(&tarball_data, &staging)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|error| OhpmError::io(parent, error))?;
        }
        match std::fs::rename(&staging, &dest) {
            Ok(()) => {}
            Err(_) if dest.is_dir() => {
                std::fs::remove_dir_all(&staging)
                    .map_err(|cleanup| OhpmError::io(&staging, cleanup))?;
            }
            Err(error) => return Err(OhpmError::io(&dest, error)),
        }

        Ok(PackageAcquisitionResult {
            name: req.name.clone(),
            root: dest.is_dir().then(|| canonicalize_or_owned(&dest)),
            source: PackageAcquisitionSource::Registry,
            version: Some(meta.version),
            integrity: expected_integrity
                .map(str::to_string)
                .map(String::into_boxed_str),
        })
    }
}

struct LocalPackageSpecifier<'a> {
    value: &'a str,
}

impl<'a> LocalPackageSpecifier<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn path(self) -> Option<&'a Path> {
        let path = self.value.strip_prefix("file:").unwrap_or(self.value);
        (path.starts_with('.') || Path::new(path).is_absolute()).then(|| Path::new(path))
    }
}

struct CacheInstallStagingPath<'a> {
    destination: &'a Path,
}

impl<'a> CacheInstallStagingPath<'a> {
    fn new(destination: &'a Path) -> Self {
        Self { destination }
    }

    fn path(&self) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let file_name = self
            .destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        self.destination.with_file_name(format!(
            ".{file_name}.arkdown-{}-{id}.tmp",
            std::process::id()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn local_path_dependency_resolves_without_a_materialized_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let issuer = temp.path().join("entry");
        let dependency = temp.path().join("library");
        std::fs::create_dir_all(&issuer).unwrap();
        std::fs::create_dir_all(&dependency).unwrap();
        let resolver =
            OhpmResolver::new(temp.path().join("cache"), None, OhpmResolutionMode::Offline);
        let result = resolver
            .resolve(
                &PackageAcquisitionRequest {
                    name: PackageName::new("library").unwrap(),
                    version: Some("file:../library".into()),
                    issuer_root: issuer,
                },
                &OhPackageLockfile::default(),
            )
            .unwrap();

        assert_eq!(result.source, PackageAcquisitionSource::LocalPath);
        assert_eq!(result.root, Some(canonicalize_or_owned(&dependency)));
    }

    #[test]
    fn offline_cache_lookup_accepts_an_exact_manifest_version_without_lockfile() {
        let temp = tempfile::tempdir().unwrap();
        let name = PackageName::new("library").unwrap();
        let package = crate::cache::cache_path(temp.path(), &name, "1.2.3");
        std::fs::create_dir_all(&package).unwrap();
        let resolver =
            OhpmResolver::new(temp.path().to_path_buf(), None, OhpmResolutionMode::Offline);
        let result = resolver
            .resolve(
                &PackageAcquisitionRequest {
                    name,
                    version: Some("1.2.3".into()),
                    issuer_root: temp.path().join("project"),
                },
                &OhPackageLockfile::default(),
            )
            .unwrap();

        assert_eq!(result.source, PackageAcquisitionSource::OhpmCache);
        assert_eq!(result.root, Some(canonicalize_or_owned(&package)));
    }

    #[test]
    fn prefer_offline_returns_a_cache_hit_without_contacting_the_registry() {
        let temp = tempfile::tempdir().unwrap();
        let name = PackageName::new("library").unwrap();
        let package = crate::cache::cache_path(temp.path(), &name, "1.2.3");
        std::fs::create_dir_all(&package).unwrap();
        let resolver = OhpmResolver::new(
            temp.path().to_path_buf(),
            Some("http://127.0.0.1:1".into()),
            OhpmResolutionMode::PreferOffline,
        );

        let result = resolver
            .resolve(
                &PackageAcquisitionRequest {
                    name,
                    version: Some("1.2.3".into()),
                    issuer_root: temp.path().join("project"),
                },
                &OhPackageLockfile::default(),
            )
            .unwrap();
        assert_eq!(result.source, PackageAcquisitionSource::OhpmCache);
    }

    #[test]
    fn offline_miss_never_contacts_the_configured_registry() {
        let temp = tempfile::tempdir().unwrap();
        let resolver = OhpmResolver::new(
            temp.path().to_path_buf(),
            Some("http://127.0.0.1:1".into()),
            OhpmResolutionMode::Offline,
        );
        let error = resolver
            .resolve(
                &PackageAcquisitionRequest {
                    name: PackageName::new("missing").unwrap(),
                    version: Some("1.0.0".into()),
                    issuer_root: temp.path().join("project"),
                },
                &OhPackageLockfile::default(),
            )
            .unwrap_err();
        assert!(matches!(error, OhpmError::OfflineMiss { .. }));
    }

    #[test]
    fn online_registry_fetch_installs_a_complete_package_in_the_cache() {
        let tarball = {
            let mut compressed = Vec::new();
            {
                let encoder =
                    flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::default());
                let mut archive = tar::Builder::new(encoder);
                let manifest = br#"{ "name": "library", "version": "1.2.3" }"#;
                let mut header = tar::Header::new_gnu();
                header.set_size(manifest.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                archive
                    .append_data(&mut header, "package/oh-package.json5", &manifest[..])
                    .unwrap();
                archive.into_inner().unwrap().finish().unwrap();
            }
            compressed
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for request_index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let _ = stream.read(&mut request).unwrap();
                let body = if request_index == 0 {
                    format!(
                        r#"{{"versions":{{"1.2.3":{{"version":"1.2.3","dist":{{"tarball":"http://{address}/library.tgz"}}}}}},"distTags":{{"latest":"1.2.3"}}}}"#,
                    )
                    .into_bytes()
                } else {
                    tarball.clone()
                };
                let content_type = if request_index == 0 {
                    "application/json"
                } else {
                    "application/octet-stream"
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let resolver = OhpmResolver::new(
            temp.path().join("cache"),
            Some(format!("http://{address}").into_boxed_str()),
            OhpmResolutionMode::Online,
        );
        let result = resolver
            .resolve(
                &PackageAcquisitionRequest {
                    name: PackageName::new("library").unwrap(),
                    version: Some("^1.0.0".into()),
                    issuer_root: temp.path().join("project"),
                },
                &OhPackageLockfile::default(),
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(result.source, PackageAcquisitionSource::Registry);
        assert_eq!(result.version.as_deref(), Some("1.2.3"));
        assert!(result
            .root
            .expect("installed root")
            .join("oh-package.json5")
            .is_file());
    }
}
