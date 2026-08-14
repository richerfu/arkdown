//! Plugin-facing SDK inventory matching hvigor's `SdkDetails` discovery.
//!
//! Arbitrary JavaScript plugins need a live facade, but SDK path discovery and
//! component metadata IO are immutable startup facts and belong in the Rust
//! session layer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorSdkDetailsRequest {
    pub project_root: PathBuf,
    #[serde(default)]
    pub sdk_path: Option<PathBuf>,
    #[serde(default)]
    pub ets_loader_path: Option<PathBuf>,
    pub is_ohos: bool,
    pub requested_api: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorSdkComponentMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_type: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorSdkDetailsSnapshot {
    pub sdk_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HvigorSdkComponentMetadata>,
}

#[derive(Debug, Error)]
pub enum HvigorSdkDetailsError {
    #[error("failed to resolve the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("failed to read OpenHarmony SDK location from {path}: {source}")]
    LocalPropertiesRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read ETS SDK component metadata {path}: {source}")]
    MetadataRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse ETS SDK component metadata {path}: {source}")]
    MetadataParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Resolves one immutable SDK details snapshot for the plugin compatibility
/// facade. Component traversal is sorted so selection is deterministic across
/// filesystems and thread counts.
pub struct HvigorSdkDetailsInventory {
    request: HvigorSdkDetailsRequest,
}

impl HvigorSdkDetailsInventory {
    pub fn new(request: HvigorSdkDetailsRequest) -> Self {
        Self { request }
    }

    pub fn inspect(self) -> Result<HvigorSdkDetailsSnapshot, HvigorSdkDetailsError> {
        let sdk_dir = self.sdk_dir()?;
        let metadata = self.component_metadata(&sdk_dir)?;
        Ok(HvigorSdkDetailsSnapshot { sdk_dir, metadata })
    }

    fn sdk_dir(&self) -> Result<PathBuf, HvigorSdkDetailsError> {
        if let Some(path) = self
            .request
            .sdk_path
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
        {
            return Ok(path.clone());
        }
        if let Some(path) = self.sdk_root_from_ets_loader()? {
            return Ok(path);
        }
        if self.request.is_ohos {
            if let Some(path) = self.open_harmony_local_sdk_root()? {
                return Ok(path);
            }
            if let Some(path) = std::env::var_os("OHOS_BASE_SDK_HOME") {
                return Ok(PathBuf::from(path));
            }
        } else if let Some(path) = std::env::var_os("DEVECO_SDK_HOME") {
            return Ok(PathBuf::from(path));
        }
        Ok(PathBuf::new())
    }

    fn sdk_root_from_ets_loader(&self) -> Result<Option<PathBuf>, HvigorSdkDetailsError> {
        let Some(loader_path) = self
            .request
            .ets_loader_path
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
        else {
            return Ok(None);
        };
        let absolute = if loader_path.is_absolute() {
            loader_path.clone()
        } else {
            std::env::current_dir()
                .map_err(HvigorSdkDetailsError::CurrentDirectory)?
                .join(loader_path)
        };
        for ancestor in absolute.ancestors() {
            if ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("ets"))
            {
                return Ok(ancestor.parent().map(Path::to_path_buf));
            }
        }
        Ok(None)
    }

    fn open_harmony_local_sdk_root(&self) -> Result<Option<PathBuf>, HvigorSdkDetailsError> {
        let path = self.request.project_root.join("local.properties");
        if !path.is_file() {
            return Ok(None);
        }
        let source = std::fs::read_to_string(&path).map_err(|source| {
            HvigorSdkDetailsError::LocalPropertiesRead {
                path: path.clone(),
                source,
            }
        })?;
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() != "sdk.dir" {
                continue;
            }
            let value = value.trim().replace("\\:", ":").replace("\\\\", "\\");
            if !value.is_empty() {
                return Ok(Some(PathBuf::from(value)));
            }
        }
        Ok(None)
    }

    fn component_metadata(
        &self,
        sdk_dir: &Path,
    ) -> Result<Option<HvigorSdkComponentMetadata>, HvigorSdkDetailsError> {
        if sdk_dir.as_os_str().is_empty() {
            return Ok(None);
        }
        let mut fallback = None;
        for component_root in self.component_roots(sdk_dir) {
            let metadata_path = component_root.join("ets").join("oh-uni-package.json");
            if !metadata_path.is_file() {
                continue;
            }
            let source = std::fs::read_to_string(&metadata_path).map_err(|source| {
                HvigorSdkDetailsError::MetadataRead {
                    path: metadata_path.clone(),
                    source,
                }
            })?;
            let raw = serde_json::from_str::<serde_json::Value>(&source).map_err(|source| {
                HvigorSdkDetailsError::MetadataParse {
                    path: metadata_path.clone(),
                    source,
                }
            })?;
            let metadata = raw
                .get("data")
                .and_then(serde_json::Value::as_object)
                .or_else(|| raw.as_object());
            let candidate = HvigorSdkComponentMetadata {
                api_version: metadata
                    .and_then(|value| value.get("apiVersion"))
                    .and_then(Self::api_version),
                version: metadata
                    .and_then(|value| value.get("version"))
                    .and_then(serde_json::Value::as_str)
                    .map(Box::<str>::from),
                release_type: metadata
                    .and_then(|value| value.get("releaseType"))
                    .and_then(serde_json::Value::as_str)
                    .map(Box::<str>::from),
            };
            if fallback.is_none() {
                fallback = Some(candidate.clone());
            }
            if candidate.api_version == Some(self.request.requested_api) {
                return Ok(Some(candidate));
            }
        }
        Ok(fallback)
    }

    fn component_roots(&self, sdk_dir: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        Self::push_unique(&mut roots, sdk_dir.to_path_buf());
        Self::push_unique(&mut roots, sdk_dir.join("openharmony"));
        Self::push_unique(&mut roots, sdk_dir.join("default"));
        Self::push_unique(&mut roots, sdk_dir.join("default").join("openharmony"));
        let Ok(entries) = std::fs::read_dir(sdk_dir) else {
            return roots;
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        for entry in entries {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }
            let child = entry.path();
            Self::push_unique(&mut roots, child.clone());
            Self::push_unique(&mut roots, child.join("default"));
            Self::push_unique(&mut roots, child.join("openharmony"));
        }
        roots
    }

    fn push_unique(roots: &mut Vec<PathBuf>, path: PathBuf) {
        if !roots.contains(&path) {
            roots.push(path);
        }
    }

    fn api_version(value: &serde_json::Value) -> Option<u32> {
        if let Some(value) = value.as_u64() {
            return u32::try_from(value).ok();
        }
        let value = value.as_str()?.trim();
        if let Ok(value) = value.parse::<u32>() {
            return Some(value);
        }
        let inner = value.strip_suffix(')')?.rsplit_once('(')?.1;
        inner.parse::<u32>().ok()
    }
}
