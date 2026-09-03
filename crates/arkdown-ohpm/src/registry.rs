//! npm-compatible OHPM registry client.
//!
//! Fetches package metadata and tarballs from the OHPM registry using the
//! standard npm registry protocol.

use semver::{Version, VersionReq};
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
        let url = OhpmRegistryPackageUrl::new(&self.registry_url, name).string();
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

        let resolved_version = RegistryVersionSelector::new(&meta, version)
            .resolve()
            .ok_or_else(|| OhpmError::RegistryNotFound {
                name: name.to_string(),
                message: format!(
                    "no registry version satisfies {}",
                    version.unwrap_or("latest")
                ),
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

struct OhpmRegistryPackageUrl<'a> {
    registry_url: &'a str,
    package_name: &'a PackageName,
}

impl<'a> OhpmRegistryPackageUrl<'a> {
    fn new(registry_url: &'a str, package_name: &'a PackageName) -> Self {
        Self {
            registry_url,
            package_name,
        }
    }

    fn string(&self) -> String {
        let package_name = self.package_name.as_str();
        let mut url = String::with_capacity(self.registry_url.len() + 1 + package_name.len());
        url.push_str(self.registry_url);
        url.push('/');
        // npm-compatible registries address scoped packages with the slash in
        // the package identifier percent-encoded. Plain package identifiers
        // contain no slash and remain byte-for-byte unchanged.
        url.push_str(&package_name.replace('/', "%2F"));
        url
    }
}

struct RegistryVersionSelector<'a> {
    metadata: &'a RawPackageMeta,
    requested: Option<&'a str>,
}

impl<'a> RegistryVersionSelector<'a> {
    fn new(metadata: &'a RawPackageMeta, requested: Option<&'a str>) -> Self {
        Self {
            metadata,
            requested,
        }
    }

    fn resolve(&self) -> Option<String> {
        let requested = self.requested.unwrap_or("latest");
        if let Some(tagged) = self.metadata.dist_tags.get(requested) {
            return Some(tagged.clone());
        }
        if self.metadata.versions.contains_key(requested) {
            return Some(requested.to_string());
        }
        let requirement = VersionReq::parse(requested).ok()?;
        self.metadata
            .versions
            .keys()
            .filter_map(|candidate| {
                let version = Version::parse(candidate).ok()?;
                requirement
                    .matches(&version)
                    .then_some((version, candidate))
            })
            .max_by(|(left, _), (right, _)| left.cmp(right))
            .map(|(_, candidate)| candidate.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> RawPackageMeta {
        RawPackageMeta {
            versions: ["1.0.0", "1.5.0", "2.0.0"]
                .into_iter()
                .map(|version| {
                    (
                        version.to_string(),
                        RawVersionEntry {
                            version: version.to_string(),
                            dist: RawDist::default(),
                        },
                    )
                })
                .collect(),
            dist_tags: [("latest".to_string(), "2.0.0".to_string())]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn registry_selector_supports_tags_exact_versions_and_semver_ranges() {
        let metadata = metadata();
        assert_eq!(
            RegistryVersionSelector::new(&metadata, None)
                .resolve()
                .as_deref(),
            Some("2.0.0")
        );
        assert_eq!(
            RegistryVersionSelector::new(&metadata, Some("1.0.0"))
                .resolve()
                .as_deref(),
            Some("1.0.0")
        );
        assert_eq!(
            RegistryVersionSelector::new(&metadata, Some("^1.0.0"))
                .resolve()
                .as_deref(),
            Some("1.5.0")
        );
    }

    #[test]
    fn scoped_registry_url_encodes_the_package_separator() {
        let name = PackageName::new("@ohos/hypium").unwrap();
        assert_eq!(
            OhpmRegistryPackageUrl::new("https://registry.example", &name).string(),
            "https://registry.example/@ohos%2Fhypium"
        );
    }
}
