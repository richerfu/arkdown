//! npm-compatible OHPM registry client.
//!
//! Fetches package metadata and tarballs from the OHPM registry using the
//! standard npm registry protocol.

use serde::Deserialize;

use crate::error::OhpmError;
use crate::package_name::PackageName;

/// Registry metadata for a single package version.
#[derive(Debug, Clone)]
pub struct RegistryVersionMetadata {
    /// The resolved package version string.
    pub version: Box<str>,
    /// URL to download the tarball.
    pub dist_tarball: Box<str>,
    /// SRI integrity hash for the tarball, if present.
    pub integrity: Option<Box<str>>,
    /// SHA-1 shasum for the tarball (legacy fallback).
    pub shasum: Option<Box<str>>,
}

/// npm-compatible registry metadata response for a package.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPackageMeta {
    #[serde(default)]
    versions: std::collections::BTreeMap<String, RawVersionEntry>,
    #[serde(default)]
    dist_tags: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RawVersionEntry {
    #[serde(default)]
    version: String,
    #[serde(default)]
    dist: RawDist,
}

#[derive(Debug, Default, Deserialize)]
struct RawDist {
    #[serde(default)]
    tarball: String,
    #[serde(default)]
    integrity: Option<String>,
    #[serde(default)]
    shasum: Option<String>,
}

/// A client for the OHPM npm-compatible registry.
#[derive(Debug, Clone)]
pub struct OhpmRegistryClient {
    /// Base URL of the registry, e.g. `"https://registry.ohpm.openharmony.cn"`.
    pub registry_url: Box<str>,
    /// HTTP client (blocking).
    client: reqwest::blocking::Client,
}

impl OhpmRegistryClient {
    /// Construct a new registry client.
    pub fn new(registry_url: impl Into<Box<str>>) -> Self {
        Self {
            registry_url: registry_url.into(),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Fetch version metadata for a package.
    ///
    /// When `version` is `None`, returns metadata for the `"latest"` dist-tag.
    ///
    /// # Errors
    ///
    /// Returns an [`OhpmError`] if the HTTP request fails or the response
    /// cannot be parsed.
    pub fn fetch_version_metadata(
        &self,
        name: &PackageName,
        version: Option<&str>,
    ) -> Result<RegistryVersionMetadata, OhpmError> {
        let url = format!("{}/{}", self.registry_url, name.as_str());
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(OhpmError::RegistryHttp)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(OhpmError::RegistryNotFound {
                name: name.to_string(),
                message: "404 Not Found".into(),
            });
        }

        let meta: RawPackageMeta = response.json().map_err(OhpmError::RegistryHttp)?;

        // Resolve version: use explicit version or resolve `latest` dist-tag.
        let resolved_version = version
            .map(|v| v.to_string())
            .or_else(|| meta.dist_tags.get("latest").cloned())
            .ok_or_else(|| OhpmError::RegistryNotFound {
                name: name.to_string(),
                message: "no version or latest dist-tag found".into(),
            })?;

        let entry =
            meta.versions
                .get(&resolved_version)
                .ok_or_else(|| OhpmError::RegistryNotFound {
                    name: name.to_string(),
                    message: format!("version {resolved_version} not found in registry"),
                })?;

        Ok(RegistryVersionMetadata {
            version: entry.version.clone().into_boxed_str(),
            dist_tarball: entry.dist.tarball.clone().into_boxed_str(),
            integrity: entry.dist.integrity.clone().map(String::into_boxed_str),
            shasum: entry.dist.shasum.clone().map(String::into_boxed_str),
        })
    }

    /// Download a tarball from the given URL.
    ///
    /// # Errors
    ///
    /// Returns an [`OhpmError`] if the HTTP request fails.
    pub fn download_tarball(&self, tarball_url: &str) -> Result<Vec<u8>, OhpmError> {
        let response = self
            .client
            .get(tarball_url)
            .send()
            .map_err(OhpmError::RegistryHttp)?;
        let bytes = response.bytes().map_err(OhpmError::RegistryHttp)?;
        Ok(bytes.to_vec())
    }
}
