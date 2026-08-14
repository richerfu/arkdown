//! Immutable remote-HSP inventory used by Hvigor plugin facades.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHspObjectRequest {
    pub module_root: PathBuf,
    pub project_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHspObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hsp_name: Option<Box<str>>,
    pub hsp_version: Box<str>,
    pub hsp_path: PathBuf,
    pub hsp_dir_name: Box<str>,
    pub hsp_file_name: Box<str>,
    pub is_integrated_hsp: bool,
    #[serde(skip)]
    version: Version,
}

#[derive(Debug, Error)]
pub enum RemoteHspObjectError {
    #[error("failed to read remote HSP directory {path}: {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("only one HSP is allowed in {path}")]
    AllowOneHsp { path: PathBuf },
    #[error("no oh-package.json5 found for {hsp_file_name:?} in {path}")]
    NoPackageFound {
        path: PathBuf,
        hsp_file_name: Option<Box<str>>,
    },
    #[error("failed to read remote HSP package {path}: {source}")]
    PackageRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse remote HSP package {path}: {source}")]
    PackageParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("remote HSP version in {path} is not valid semver")]
    VersionIllegal {
        path: PathBuf,
        version: Option<Box<str>>,
    },
    #[error("no HSP found in {path}")]
    NoHspFound { path: PathBuf },
}

impl RemoteHspObjectError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AllowOneHsp { .. } => "ALLOW_ONE_HSP",
            Self::NoPackageFound { .. } => "NO_PACKAGE_FOUND",
            Self::VersionIllegal { .. } => "VERSION_ILLEGAL",
            Self::NoHspFound { .. } => "NO_HSP_FOUND",
            Self::ReadDirectory { .. } | Self::PackageRead { .. } => "REMOTE_HSP_IO_FAILED",
            Self::PackageParse { .. } => "INVALID_REMOTE_HSP_PACKAGE",
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::ReadDirectory { path, .. }
            | Self::AllowOneHsp { path }
            | Self::NoPackageFound { path, .. }
            | Self::PackageRead { path, .. }
            | Self::PackageParse { path, .. }
            | Self::VersionIllegal { path, .. }
            | Self::NoHspFound { path } => path,
        }
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            Self::VersionIllegal { version, .. } => version.as_deref(),
            _ => None,
        }
    }
}

/// Scans module-level remote HSPs first, then applies project-level packages
/// by highest semver, matching `remote-hsp-utils.ts::getRemoteHspObjList`.
pub struct RemoteHspObjectInventory {
    request: RemoteHspObjectRequest,
}

impl RemoteHspObjectInventory {
    pub fn new(request: RemoteHspObjectRequest) -> Self {
        Self { request }
    }

    pub fn inspect(self) -> Result<Vec<RemoteHspObject>, RemoteHspObjectError> {
        let mut objects = IndexMap::<Box<str>, RemoteHspObject>::new();
        self.scan_root(
            &self.request.module_root.join("oh_modules").join(".hsp"),
            &mut objects,
        )?;
        self.scan_root(
            &self.request.project_root.join("oh_modules").join(".hsp"),
            &mut objects,
        )?;
        Ok(objects.into_values().collect())
    }

    fn scan_root(
        &self,
        root: &Path,
        objects: &mut IndexMap<Box<str>, RemoteHspObject>,
    ) -> Result<(), RemoteHspObjectError> {
        if !root.exists() {
            return Ok(());
        }
        let mut entries = std::fs::read_dir(root)
            .map_err(|source| RemoteHspObjectError::ReadDirectory {
                path: root.to_path_buf(),
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| RemoteHspObjectError::ReadDirectory {
                path: root.to_path_buf(),
                source,
            })?;
        entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        for entry in entries {
            let candidate = self.inspect_directory(&entry.path(), &entry.file_name())?;
            let replace = objects
                .get(candidate.hsp_file_name.as_ref())
                .is_none_or(|current| candidate.version >= current.version);
            if replace {
                objects.insert(candidate.hsp_file_name.clone(), candidate);
            }
        }
        Ok(())
    }

    fn inspect_directory(
        &self,
        directory: &Path,
        directory_name: &std::ffi::OsStr,
    ) -> Result<RemoteHspObject, RemoteHspObjectError> {
        let mut entries = std::fs::read_dir(directory)
            .map_err(|source| RemoteHspObjectError::ReadDirectory {
                path: directory.to_path_buf(),
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| RemoteHspObjectError::ReadDirectory {
                path: directory.to_path_buf(),
                source,
            })?;
        entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        let mut hsp_file_name = None;
        let mut has_package = false;
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".hsp") {
                if hsp_file_name.is_some() {
                    return Err(RemoteHspObjectError::AllowOneHsp {
                        path: directory.to_path_buf(),
                    });
                }
                hsp_file_name = Some(Box::<str>::from(name.as_str()));
            }
            if name.contains("oh-package.json5") {
                has_package = true;
            }
        }
        if !has_package {
            return Err(RemoteHspObjectError::NoPackageFound {
                path: directory.to_path_buf(),
                hsp_file_name,
            });
        }
        // Inventory the complete directory before parsing metadata. This
        // preserves Hvigor's ALLOW_ONE_HSP contract independently of host
        // readdir order while retaining deterministic sorted traversal.
        let package = self.read_package(&directory.join("oh-package.json5"))?;
        let Some(hsp_file_name) = hsp_file_name else {
            return Err(RemoteHspObjectError::NoHspFound {
                path: directory.to_path_buf(),
            });
        };
        Ok(RemoteHspObject {
            hsp_name: package.name,
            hsp_version: package.version_text,
            hsp_path: directory.join(hsp_file_name.as_ref()),
            hsp_dir_name: Box::<str>::from(directory_name.to_string_lossy().as_ref()),
            hsp_file_name,
            is_integrated_hsp: package.is_integrated_hsp,
            version: package.version,
        })
    }

    fn read_package(&self, path: &Path) -> Result<RemoteHspPackage, RemoteHspObjectError> {
        let source =
            std::fs::read_to_string(path).map_err(|source| RemoteHspObjectError::PackageRead {
                path: path.to_path_buf(),
                source,
            })?;
        let value = serde_json::from_str::<serde_json::Value>(&source).map_err(|source| {
            RemoteHspObjectError::PackageParse {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let version_text = value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(Box::<str>::from);
        let Some(validated_version_text) = version_text.as_deref() else {
            return Err(RemoteHspObjectError::VersionIllegal {
                path: path.to_path_buf(),
                version: None,
            });
        };
        let version = Version::parse(validated_version_text).map_err(|_| {
            RemoteHspObjectError::VersionIllegal {
                path: path.to_path_buf(),
                version: version_text.clone(),
            }
        })?;
        Ok(RemoteHspPackage {
            name: value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(Box::<str>::from),
            version,
            version_text: Box::<str>::from(validated_version_text),
            is_integrated_hsp: value
                .pointer("/metadata/integratedHsp")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        })
    }
}

struct RemoteHspPackage {
    name: Option<Box<str>>,
    version: Version,
    version_text: Box<str>,
    is_integrated_hsp: bool,
}
