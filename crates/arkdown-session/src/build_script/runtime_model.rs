//! Runtime model inventory used by the arbitrary build-script compatibility boundary.
//!
//! JavaScript must execute user-authored callbacks, but it must not discover
//! or parse the project model itself. This inventory reads every stable model
//! file in Rust and returns immutable, lossless JSON values for the thin V8
//! facade.

use std::path::{Path, PathBuf};

use arkdown_config::{BuildScriptSchemaDiagnostic, BuildScriptSchemaValidator};
use arkdown_path::lexical_normalize;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BuildScriptProjectCacheIdentity, BuildScriptProjectCacheIdentityError};

const BUILD_SCRIPT_FILE_NAMES: [&str; 4] = [
    "hvigorfile.ts",
    "hvigorfile.js",
    "hvigorfile.cjs",
    "hvigorfile.mjs",
];
const BUILD_CONFIG_FILE_NAMES: [&str; 4] = [
    "hvigorconfig.ts",
    "hvigorconfig.js",
    "hvigorconfig.cjs",
    "hvigorconfig.mjs",
];

#[derive(Debug, Error)]
pub enum BuildScriptRuntimeModelError {
    #[error("failed to resolve the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("failed to read build-script runtime model {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse build-script runtime model {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("module descriptor has neither a usable name nor srcPath: {0}")]
    InvalidModuleDescriptor(serde_json::Value),
    #[error(transparent)]
    Schema(#[from] BuildScriptSchemaDiagnostic),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeModelRequest {
    pub project_root: PathBuf,
    #[serde(default)]
    pub hvigor_user_home: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptModuleModelRequest {
    pub project_root: PathBuf,
    pub descriptor: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptFileInventoryRequest {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptRuntimeModelSnapshot {
    pub project_root: PathBuf,
    pub project_cache_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hvigor_workspace: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_config_file: Option<PathBuf>,
    pub project_profile: serde_json::Value,
    pub project_package: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_hvigor_config_path: Option<PathBuf>,
    pub project_hvigor_config: serde_json::Value,
    pub user_hvigor_config: serde_json::Value,
    pub modules: Vec<BuildScriptModuleModelSnapshot>,
    pub fallback_module_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildScriptModuleModelSnapshot {
    pub name: String,
    pub src_path: String,
    pub root: PathBuf,
    pub descriptor: serde_json::Value,
    pub build_profile: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_json: Option<serde_json::Value>,
    pub package: serde_json::Value,
}

/// Loads one project-wide build-script model snapshot.
pub struct BuildScriptRuntimeModelLoader {
    request: BuildScriptRuntimeModelRequest,
}

impl BuildScriptRuntimeModelLoader {
    pub fn new(request: BuildScriptRuntimeModelRequest) -> Self {
        Self { request }
    }

    pub fn load(self) -> Result<BuildScriptRuntimeModelSnapshot, BuildScriptRuntimeModelError> {
        let project_root = RuntimeModelPath::absolute(self.request.project_root)?;
        let cache_identity =
            BuildScriptProjectCacheIdentity::new(&project_root).map_err(|error| match error {
                BuildScriptProjectCacheIdentityError::CurrentDirectory(source) => {
                    BuildScriptRuntimeModelError::CurrentDirectory(source)
                }
            })?;
        let hvigor_user_home = self
            .request
            .hvigor_user_home
            .map(RuntimeModelPath::absolute)
            .transpose()?;
        let project_cache_hash = cache_identity.hash();
        let hvigor_workspace = hvigor_user_home
            .as_deref()
            .map(|home| cache_identity.workspace(home));
        let reader = RuntimeModelReader;
        let build_config_file =
            RuntimeModelPath::first_file(&project_root, &BUILD_CONFIG_FILE_NAMES);
        let project_profile_path = project_root.join("build-profile.json5");
        let project_profile = reader.read_or_empty(&project_profile_path)?;
        BuildScriptSchemaValidator::new(&project_profile_path)
            .validate_project_profile(&project_profile)?;
        let project_package = reader.read_or_empty(&project_root.join("oh-package.json5"))?;
        let app_json = reader.read_optional(&project_root.join("AppScope").join("app.json5"))?;
        let project_hvigor_config_path = project_root.join("hvigor").join("hvigor-config.json5");
        let project_hvigor_config = reader.read_or_empty(&project_hvigor_config_path)?;
        let project_hvigor_config_path = project_hvigor_config_path
            .is_file()
            .then_some(project_hvigor_config_path);
        let user_hvigor_config = match hvigor_user_home {
            Some(root) => reader.read_or_empty(&root.join("hvigor-config.json5"))?,
            None => serde_json::Value::Object(serde_json::Map::new()),
        };
        let descriptors = project_profile
            .get("modules")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut modules = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            modules.push(
                BuildScriptModuleModelLoader::new(BuildScriptModuleModelRequest {
                    project_root: project_root.clone(),
                    descriptor,
                })
                .load()?,
            );
        }
        let fallback_module_roots = if modules.is_empty() {
            BuildScriptFileInventory::fallback_module_roots(&project_root)?
        } else {
            Vec::new()
        };

        Ok(BuildScriptRuntimeModelSnapshot {
            project_root,
            project_cache_hash,
            hvigor_workspace,
            build_config_file,
            project_profile,
            project_package,
            app_json,
            project_hvigor_config_path,
            project_hvigor_config,
            user_hvigor_config,
            modules,
            fallback_module_roots,
        })
    }
}

/// Loads and parses the stable files owned by one module descriptor.
pub struct BuildScriptModuleModelLoader {
    request: BuildScriptModuleModelRequest,
}

impl BuildScriptModuleModelLoader {
    pub fn new(request: BuildScriptModuleModelRequest) -> Self {
        Self { request }
    }

    pub fn load(self) -> Result<BuildScriptModuleModelSnapshot, BuildScriptRuntimeModelError> {
        let project_root = RuntimeModelPath::absolute(self.request.project_root)?;
        let name = self
            .request
            .descriptor
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        let src_path = self
            .request
            .descriptor
            .get("srcPath")
            .and_then(serde_json::Value::as_str)
            .filter(|path| !path.is_empty())
            .map(str::to_string);
        let root = self
            .request
            .descriptor
            .get("root")
            .and_then(serde_json::Value::as_str)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .map_or_else(
                || {
                    src_path
                        .as_deref()
                        .or(name.as_deref())
                        .map(|path| project_root.join(path))
                },
                Some,
            )
            .ok_or_else(|| {
                BuildScriptRuntimeModelError::InvalidModuleDescriptor(
                    self.request.descriptor.clone(),
                )
            })?;
        let root = RuntimeModelPath::normalize_absolute(&root, &project_root);
        let name = name
            .or_else(|| {
                root.file_name()
                    .map(|value| value.to_string_lossy().into_owned())
            })
            .ok_or_else(|| {
                BuildScriptRuntimeModelError::InvalidModuleDescriptor(
                    self.request.descriptor.clone(),
                )
            })?;
        let src_path = src_path.unwrap_or_else(|| {
            root.strip_prefix(&project_root)
                .unwrap_or(root.as_path())
                .to_string_lossy()
                .into_owned()
        });
        let reader = RuntimeModelReader;

        let build_profile_path = root.join("build-profile.json5");
        let build_profile = reader.read_or_empty(&build_profile_path)?;
        BuildScriptSchemaValidator::new(&build_profile_path)
            .validate_module_profile(&build_profile)?;

        Ok(BuildScriptModuleModelSnapshot {
            name,
            src_path,
            root: root.clone(),
            descriptor: self.request.descriptor,
            build_profile,
            module_json: reader
                .read_optional(&root.join("src").join("main").join("module.json5"))?,
            package: reader.read_or_empty(&root.join("oh-package.json5"))?,
        })
    }
}

/// Selects one configuration script per root with latest-hvigor precedence.
pub struct BuildScriptFileInventory {
    roots: Vec<PathBuf>,
}

impl BuildScriptFileInventory {
    pub fn new(request: BuildScriptFileInventoryRequest) -> Self {
        Self {
            roots: request.roots,
        }
    }

    pub fn discover(self) -> Result<Vec<PathBuf>, BuildScriptRuntimeModelError> {
        let mut files = Vec::with_capacity(self.roots.len());
        for root in self.roots {
            let root = RuntimeModelPath::absolute(root)?;
            if let Some(file) = Self::file_for_root(&root) {
                files.push(file);
            }
        }
        Ok(files)
    }

    fn file_for_root(root: &Path) -> Option<PathBuf> {
        RuntimeModelPath::first_file(root, &BUILD_SCRIPT_FILE_NAMES)
    }

    fn fallback_module_roots(
        project_root: &Path,
    ) -> Result<Vec<PathBuf>, BuildScriptRuntimeModelError> {
        let entries = std::fs::read_dir(project_root).map_err(|source| {
            BuildScriptRuntimeModelError::Read {
                path: project_root.to_path_buf(),
                source,
            }
        })?;
        let mut roots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| BuildScriptRuntimeModelError::Read {
                path: project_root.to_path_buf(),
                source,
            })?;
            let file_type =
                entry
                    .file_type()
                    .map_err(|source| BuildScriptRuntimeModelError::Read {
                        path: entry.path(),
                        source,
                    })?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !file_type.is_dir() || name == "node_modules" || name.starts_with('.') {
                continue;
            }
            let root = entry.path();
            if Self::file_for_root(&root).is_some() {
                roots.push(lexical_normalize(&root));
            }
        }
        roots.sort();
        Ok(roots)
    }
}

struct RuntimeModelReader;

impl RuntimeModelReader {
    fn read_optional(
        &self,
        path: &Path,
    ) -> Result<Option<serde_json::Value>, BuildScriptRuntimeModelError> {
        if !path.is_file() {
            return Ok(None);
        }
        let source =
            std::fs::read_to_string(path).map_err(|source| BuildScriptRuntimeModelError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        json5::from_str(&source)
            .map(Some)
            .map_err(|error| BuildScriptRuntimeModelError::Parse {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
    }

    fn read_or_empty(
        &self,
        path: &Path,
    ) -> Result<serde_json::Value, BuildScriptRuntimeModelError> {
        Ok(self
            .read_optional(path)?
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())))
    }
}

struct RuntimeModelPath;

impl RuntimeModelPath {
    fn first_file<const N: usize>(root: &Path, names: &[&str; N]) -> Option<PathBuf> {
        names
            .iter()
            .map(|name| root.join(name))
            .find(|path| path.is_file())
            .map(|path| lexical_normalize(&path))
    }

    fn absolute(path: PathBuf) -> Result<PathBuf, BuildScriptRuntimeModelError> {
        if path.is_absolute() {
            return Ok(lexical_normalize(&path));
        }
        let current =
            std::env::current_dir().map_err(BuildScriptRuntimeModelError::CurrentDirectory)?;
        Ok(lexical_normalize(&current.join(path)))
    }

    fn normalize_absolute(path: &Path, project_root: &Path) -> PathBuf {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_root.join(path)
        };
        lexical_normalize(&path)
    }
}
