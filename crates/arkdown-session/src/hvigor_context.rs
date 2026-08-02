//! Rust-owned projections for the public OHOS hvigor plugin contexts.
//!
//! Upstream correspondence:
//! - `TaskService.getOhpmDependencyInfo`
//! - `TaskService.getOhpmRemoteHspDependencies`
//! - `GlobalProjectDataService.initAllModuleNameHash`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_config::RuntimeContextOverrides;
use arkdown_path::canonicalize_or_owned;
use indexmap::IndexMap;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::dependency_graph::builder::DependencyGraphBuilder;
use crate::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};

#[derive(Debug, Error)]
pub enum HvigorContextInspectError {
    #[error("required file not found: {0}")]
    FileNotFound(PathBuf),
    #[error("failed to read hvigor context input {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse hvigor context input {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("project path has no file name: {0}")]
    ProjectName(PathBuf),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorContextSnapshot {
    pub all_module_name_hash: String,
    pub project: HvigorDependencyContextSnapshot,
    pub modules: BTreeMap<PathBuf, HvigorDependencyContextSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorDependencyContextSnapshot {
    pub ohpm_dependency_info: IndexMap<String, OhpmDependencyInfo>,
    pub remote_hsp_dependency_info: IndexMap<String, OhpmDependencyInfo>,
    pub signed_remote_hsp_dependency_info: IndexMap<String, OhpmDependencyInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OhpmDependencyInfo {
    pub name: String,
    pub version: String,
    pub dependencies: IndexMap<String, String>,
    pub package_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_hsp_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_remote_hsp_path: Option<PathBuf>,
}

/// Inspects context data synchronously because hvigor's public getters are
/// synchronous. All filesystem and graph behavior remains in Rust.
pub struct HvigorContextInspector {
    project_root: PathBuf,
    product_name: String,
    runtime_overrides: Arc<RuntimeContextOverrides>,
}

impl HvigorContextInspector {
    pub fn new(
        project_root: impl Into<PathBuf>,
        product_name: impl Into<String>,
        runtime_overrides: Arc<RuntimeContextOverrides>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            product_name: product_name.into(),
            runtime_overrides,
        }
    }

    pub fn inspect(self) -> Result<HvigorContextSnapshot, HvigorContextInspectError> {
        let project_root = canonicalize_or_owned(&self.project_root);
        let profile = self.project_profile(&project_root)?;
        let modules = self.module_descriptors(&project_root, &profile);
        let remote_hsps = RemoteHspInventory::discover(&project_root, &modules)?;

        let project_graph = DependencyGraphBuilder::new(
            self.project_name(&project_root)?,
            project_root.clone(),
            project_root.clone(),
        )
        .with_runtime_overrides(Arc::clone(&self.runtime_overrides))
        .build();
        let project =
            self.context_for_graph(&project_graph, &project_root, &project_root, &remote_hsps);

        let mut module_contexts = BTreeMap::new();
        for module in &modules {
            let graph = DependencyGraphBuilder::new(
                module.name.clone(),
                project_root.clone(),
                module.root.clone(),
            )
            .with_runtime_overrides(Arc::clone(&self.runtime_overrides))
            .build();
            module_contexts.insert(
                module.root.clone(),
                self.context_for_graph(&graph, &module.root, &project_root, &remote_hsps),
            );
        }

        Ok(HvigorContextSnapshot {
            all_module_name_hash: self.all_module_name_hash(&project_root, &modules)?,
            project,
            modules: module_contexts,
        })
    }

    fn project_profile(
        &self,
        project_root: &Path,
    ) -> Result<serde_json::Value, HvigorContextInspectError> {
        if let Some(value) = self.runtime_overrides.project.build_profile.as_ref() {
            return Ok(value.clone());
        }
        let path = project_root.join("build-profile.json5");
        if !path.is_file() {
            return Err(HvigorContextInspectError::FileNotFound(path));
        }
        let content =
            std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                path: path.clone(),
                source,
            })?;
        json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
            path,
            message: error.to_string(),
        })
    }

    fn module_descriptors(
        &self,
        project_root: &Path,
        profile: &serde_json::Value,
    ) -> Vec<HvigorModuleDescriptor> {
        profile
            .get("modules")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|module| {
                let name = module.get("name")?.as_str()?.to_string();
                let src_path = module
                    .get("srcPath")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| name.clone());
                Some(HvigorModuleDescriptor {
                    name,
                    root: canonicalize_or_owned(&project_root.join(&src_path)),
                })
            })
            .collect()
    }

    fn project_name(&self, project_root: &Path) -> Result<String, HvigorContextInspectError> {
        project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| HvigorContextInspectError::ProjectName(project_root.to_path_buf()))
    }

    fn context_for_graph(
        &self,
        graph: &DependencyGraphSnapshot,
        node_root: &Path,
        project_root: &Path,
        remote_hsps: &RemoteHspInventory,
    ) -> HvigorDependencyContextSnapshot {
        let mut context = HvigorDependencyContextSnapshot::default();
        for edge in &graph.edges {
            if context.ohpm_dependency_info.contains_key(&edge.alias) {
                continue;
            }
            let Some(node) = graph.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(package) = node.package.as_ref() else {
                continue;
            };
            context.ohpm_dependency_info.insert(
                edge.alias.clone(),
                OhpmDependencyInfo {
                    name: node.package_name.clone(),
                    version: package.version.clone(),
                    dependencies: package.package_json.dependencies.clone(),
                    package_path: edge.package_root.clone(),
                    remote_hsp_path: None,
                    signed_remote_hsp_path: None,
                },
            );
        }

        for edge in &graph.edges {
            if context.remote_hsp_dependency_info.contains_key(&edge.alias) {
                continue;
            }
            let Some(node) = graph.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(info) = self.remote_hsp_info(
                node,
                &edge.alias,
                node_root,
                project_root,
                remote_hsps,
                false,
            ) else {
                continue;
            };
            context
                .remote_hsp_dependency_info
                .insert(edge.alias.clone(), info);
            if let Some(info) = self.remote_hsp_info(
                node,
                &edge.alias,
                node_root,
                project_root,
                remote_hsps,
                true,
            ) {
                context
                    .signed_remote_hsp_dependency_info
                    .insert(edge.alias.clone(), info);
            }
        }
        context
    }

    fn remote_hsp_info(
        &self,
        node: &DependencyNodeSnapshot,
        _alias: &str,
        node_root: &Path,
        project_root: &Path,
        remote_hsps: &RemoteHspInventory,
        signed: bool,
    ) -> Option<OhpmDependencyInfo> {
        if node.node_type != DependencyNodeType::SharedHsp {
            return None;
        }
        let package_root = node.package_root.as_ref()?;
        let hsp_dir_name = RemoteHspInventory::dependency_directory_name(package_root)?;
        let hsp = remote_hsps.by_package_name.get(&node.package_name)?;
        let package = node.package.as_ref()?;
        let integrated = package
            .package_json
            .extra
            .get("metadata")
            .and_then(|metadata| metadata.get("integratedHsp"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let (remote_hsp_path, signed_remote_hsp_path) = if signed {
            (
                None,
                Some(
                    project_root
                        .join("build")
                        .join("cache")
                        .join(&self.product_name)
                        .join("remote_hsp")
                        .join(hsp_dir_name)
                        .join(hsp.file_name.replace(".hsp", "-signed.hsp")),
                ),
            )
        } else {
            let base = if integrated {
                project_root
                    .join("build")
                    .join("cache")
                    .join(&self.product_name)
                    .join("integrated_hsp")
            } else {
                node_root.join("oh_modules").join(".hsp")
            };
            (Some(base.join(hsp_dir_name).join(&hsp.file_name)), None)
        };
        Some(OhpmDependencyInfo {
            name: node.package_name.clone(),
            version: package.version.clone(),
            dependencies: package.package_json.dependencies.clone(),
            package_path: package_root.clone(),
            remote_hsp_path,
            signed_remote_hsp_path,
        })
    }

    fn all_module_name_hash(
        &self,
        project_root: &Path,
        modules: &[HvigorModuleDescriptor],
    ) -> Result<String, HvigorContextInspectError> {
        let mut names = String::new();
        for module in modules {
            names.push_str(&self.source_set_module_name(module, "main")?);
            let profile = self.module_build_profile(module)?;
            let has_ohos_test = profile
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target.get("name").and_then(serde_json::Value::as_str) == Some("ohosTest")
                    })
                });
            if has_ohos_test {
                names.push_str(&self.source_set_module_name(module, "ohosTest")?);
            }
        }
        names.push_str(&self.project_name(project_root)?);
        let digest = Sha256::digest(names.as_bytes());
        Ok(format!("{digest:x}"))
    }

    fn source_set_module_name(
        &self,
        module: &HvigorModuleDescriptor,
        source_set: &str,
    ) -> Result<String, HvigorContextInspectError> {
        let runtime_value = (source_set == "main")
            .then(|| self.runtime_overrides.module_for_root(&module.root))
            .flatten()
            .and_then(|runtime| runtime.module_json.as_ref());
        let path = module
            .root
            .join("src")
            .join(source_set)
            .join("module.json5");
        let value = if let Some(value) = runtime_value {
            Some(value.clone())
        } else if path.is_file() {
            let content =
                std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                    path: path.clone(),
                    source,
                })?;
            Some(
                json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?,
            )
        } else {
            None
        };
        Ok(value
            .as_ref()
            .and_then(|value: &serde_json::Value| value.pointer("/module/name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(module.name.as_str())
            .to_string())
    }

    fn module_build_profile(
        &self,
        module: &HvigorModuleDescriptor,
    ) -> Result<serde_json::Value, HvigorContextInspectError> {
        if let Some(value) = self
            .runtime_overrides
            .module_for_root(&module.root)
            .and_then(|runtime| runtime.build_profile.as_ref())
        {
            return Ok(value.clone());
        }
        let path = module.root.join("build-profile.json5");
        if !path.is_file() {
            return Ok(serde_json::Value::Null);
        }
        let content =
            std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                path: path.clone(),
                source,
            })?;
        json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
            path,
            message: error.to_string(),
        })
    }
}

#[derive(Debug)]
struct HvigorModuleDescriptor {
    name: String,
    root: PathBuf,
}

#[derive(Debug, Default)]
struct RemoteHspInventory {
    by_package_name: BTreeMap<String, RemoteHspArtifact>,
}

#[derive(Debug)]
struct RemoteHspArtifact {
    version: Version,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct RemoteHspPackage {
    name: String,
    version: String,
}

impl RemoteHspInventory {
    fn discover(
        project_root: &Path,
        modules: &[HvigorModuleDescriptor],
    ) -> Result<Self, HvigorContextInspectError> {
        let mut inventory = Self::default();
        for root in modules
            .iter()
            .map(|module| module.root.as_path())
            .chain(std::iter::once(project_root))
        {
            inventory.scan_directory(&root.join("oh_modules").join(".hsp"))?;
        }
        Ok(inventory)
    }

    fn scan_directory(&mut self, root: &Path) -> Result<(), HvigorContextInspectError> {
        if !root.is_dir() {
            return Ok(());
        }
        let entries = std::fs::read_dir(root).map_err(|source| HvigorContextInspectError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| HvigorContextInspectError::Io {
                path: root.to_path_buf(),
                source,
            })?;
            let directory = entry.path();
            if !directory.is_dir() {
                continue;
            }
            let package_path = directory.join("oh-package.json5");
            if !package_path.is_file() {
                continue;
            }
            let content = std::fs::read_to_string(&package_path).map_err(|source| {
                HvigorContextInspectError::Io {
                    path: package_path.clone(),
                    source,
                }
            })?;
            let package: RemoteHspPackage =
                json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
                    path: package_path,
                    message: error.to_string(),
                })?;
            let Ok(version) = Version::parse(&package.version) else {
                continue;
            };
            let files =
                std::fs::read_dir(&directory).map_err(|source| HvigorContextInspectError::Io {
                    path: directory.clone(),
                    source,
                })?;
            let mut hsp_file = None;
            for file in files {
                let file = file.map_err(|source| HvigorContextInspectError::Io {
                    path: directory.clone(),
                    source,
                })?;
                if file.path().extension().and_then(|value| value.to_str()) == Some("hsp") {
                    hsp_file = file.file_name().to_str().map(str::to_string);
                    break;
                }
            }
            let Some(file_name) = hsp_file else {
                continue;
            };
            let should_replace = self
                .by_package_name
                .get(&package.name)
                .is_none_or(|current| version >= current.version);
            if should_replace {
                self.by_package_name
                    .insert(package.name, RemoteHspArtifact { version, file_name });
            }
        }
        Ok(())
    }

    fn dependency_directory_name(package_root: &Path) -> Option<String> {
        let components = package_root.components().collect::<Vec<_>>();
        let index = components
            .iter()
            .rposition(|component| component.as_os_str() == "oh_modules")?;
        (index > 0).then(|| {
            components[index - 1]
                .as_os_str()
                .to_string_lossy()
                .into_owned()
        })
    }
}
