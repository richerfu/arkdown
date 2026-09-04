//! SDK system-capability declarations loaded once for source diagnostics.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rustc_hash::{FxHashMap, FxHashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkSystemCapabilityError {
    #[error("read SDK system-capability file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse SDK system-capability file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Clone, Debug, Default)]
pub struct SdkSystemCapabilityInventory {
    by_device_type: Arc<FxHashMap<Box<str>, FxHashSet<Box<str>>>>,
}

impl SdkSystemCapabilityInventory {
    pub fn from_api_roots(api_roots: &[PathBuf]) -> Result<Self, SdkSystemCapabilityError> {
        let mut files = Vec::new();
        for root in api_roots {
            let directory = root.join("device-define");
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(SdkSystemCapabilityError::Read {
                        path: directory,
                        source,
                    });
                }
            };
            for entry in entries {
                let entry = entry.map_err(|source| SdkSystemCapabilityError::Read {
                    path: directory.clone(),
                    source,
                })?;
                let path = entry.path();
                if entry
                    .file_type()
                    .map_err(|source| SdkSystemCapabilityError::Read {
                        path: path.clone(),
                        source,
                    })?
                    .is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "json")
                {
                    files.push(path);
                }
            }
        }
        files.sort();

        let mut by_device_type = FxHashMap::<Box<str>, FxHashSet<Box<str>>>::default();
        for path in files {
            let content = std::fs::read_to_string(&path).map_err(|source| {
                SdkSystemCapabilityError::Read {
                    path: path.clone(),
                    source,
                }
            })?;
            let mut value: serde_json::Value =
                serde_json::from_str(&content).map_err(|source| {
                    SdkSystemCapabilityError::Parse {
                        path: path.clone(),
                        source,
                    }
                })?;
            let Some(sys_caps) = value
                .as_object_mut()
                .and_then(|object| object.remove("SysCaps"))
            else {
                // External SDKs colocate metadata such as
                // `syscap-version.json` and `api-white-list.json` with device
                // declarations. ets-loader selects only device-name files;
                // the startup inventory equivalently ignores JSON values that
                // do not declare `SysCaps`.
                continue;
            };
            let sys_caps: Vec<String> = serde_json::from_value(sys_caps).map_err(|source| {
                SdkSystemCapabilityError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
            for device_type in SdkSystemCapabilityDeviceFile::new(&path).device_types() {
                by_device_type
                    .entry(device_type)
                    .or_default()
                    .extend(sys_caps.iter().map(|capability| capability.clone().into()));
            }
        }
        Ok(Self {
            by_device_type: Arc::new(by_device_type),
        })
    }

    pub fn select(&self, device_types: &[String]) -> Option<SdkSystemCapabilitySelection> {
        if device_types.is_empty() {
            return None;
        }
        let sets = device_types
            .iter()
            .map(|device_type| {
                self.by_device_type
                    .iter()
                    .filter(|(file_device_type, _)| {
                        file_device_type.as_ref() == device_type
                            || file_device_type.starts_with(device_type.as_str())
                    })
                    .flat_map(|(_, capabilities)| capabilities.iter().cloned())
                    .collect::<FxHashSet<_>>()
            })
            .collect::<Vec<_>>();
        let mut union = FxHashSet::default();
        for capabilities in &sets {
            union.extend(capabilities.iter().cloned());
        }
        let mut sets = sets.into_iter();
        let mut intersection = sets.next().unwrap_or_default();
        for capabilities in sets {
            intersection.retain(|capability| capabilities.contains(capability));
        }
        Some(SdkSystemCapabilitySelection {
            intersection,
            union,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SdkSystemCapabilitySelection {
    pub intersection: FxHashSet<Box<str>>,
    pub union: FxHashSet<Box<str>>,
}

struct SdkSystemCapabilityDeviceFile<'a> {
    path: &'a Path,
}

impl<'a> SdkSystemCapabilityDeviceFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn device_types(&self) -> Vec<Box<str>> {
        let Some(stem) = self.path.file_stem().and_then(|stem| stem.to_str()) else {
            return Vec::new();
        };
        if stem == "default" {
            return vec!["phone".into()];
        }
        vec![stem.into()]
    }
}
