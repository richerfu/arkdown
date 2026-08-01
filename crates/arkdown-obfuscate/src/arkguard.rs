use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_path::ArtifactPathSerializer;
use serde::{Deserialize, Serialize};

use crate::{ObfuscationError, ObfuscationOptions};

#[derive(Debug, Clone)]
pub struct ArkguardAdapterOptions {
    pub node_executable: PathBuf,
    pub ets_loader_path: PathBuf,
    pub arkguard_package: PathBuf,
    pub sdk_api_paths: Vec<PathBuf>,
    pub module_root: PathBuf,
    pub project_root: PathBuf,
    pub project_path: PathBuf,
    pub cache_path: PathBuf,
    pub ace_module_build: PathBuf,
    pub ace_module_json_path: PathBuf,
    pub compile_sdk_version: String,
    pub compatible_sdk_version: u32,
    pub compatible_sdk_version_stage: Option<String>,
    pub entry_package_info: String,
    pub package_dir: String,
    pub module_path_map: BTreeMap<String, PathBuf>,
    pub entry_array: Vec<String>,
    pub resource_table_path: Option<PathBuf>,
    pub local_packages: Vec<String>,
    pub compile_har: bool,
    pub compile_shared: bool,
    pub use_normalized: bool,
    pub use_ts_har: bool,
    pub byte_code_har_info: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleFile {
    pub build_path: PathBuf,
    pub original_path: PathBuf,
    pub relative_path: String,
    pub source_map_key: String,
    pub previous_source_map: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleInput {
    pub files: Vec<ObfuscationModuleFile>,
    pub source_maps_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleOutput {
    pub name_cache_path: PathBuf,
    pub output_files: Vec<PathBuf>,
}

pub(crate) struct ArkguardModuleRunner<'a> {
    options: &'a ObfuscationOptions,
    adapter: &'a ArkguardAdapterOptions,
}

impl<'a> ArkguardModuleRunner<'a> {
    pub(crate) fn new(
        options: &'a ObfuscationOptions,
        adapter: &'a ArkguardAdapterOptions,
    ) -> Self {
        Self { options, adapter }
    }

    pub(crate) fn run(
        &self,
        input: ObfuscationModuleInput,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        self.validate_sdk_adapter()?;
        std::fs::create_dir_all(&self.options.cache_dir)?;
        let bridge_path = self.options.cache_dir.join("arkdown-arkguard-bridge.cjs");
        let request_path = self.options.cache_dir.join(format!(
            "arkdown-arkguard-request-{}.json",
            std::process::id()
        ));
        let response_path = self.options.cache_dir.join(format!(
            "arkdown-arkguard-response-{}.json",
            std::process::id()
        ));
        std::fs::write(&bridge_path, include_str!("arkguard_bridge.cjs"))?;
        let request = self.request(&input)?;
        std::fs::write(&request_path, serde_json::to_vec(&request)?)?;
        let output = Command::new(&self.adapter.node_executable)
            .arg(&bridge_path)
            .arg(&request_path)
            .arg(&response_path)
            .output()
            .map_err(|source| ObfuscationError::ArkguardProcessStart {
                executable: self.adapter.node_executable.clone(),
                source,
            })?;
        let _ = std::fs::remove_file(&request_path);
        let _ = std::fs::remove_file(&bridge_path);
        if !output.status.success() {
            return Err(ObfuscationError::ArkguardProcessFailed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        let response: ArkguardResponse = serde_json::from_slice(&std::fs::read(&response_path)?)?;
        let _ = std::fs::remove_file(&response_path);
        if let Some(source_maps_path) = input.source_maps_path.as_ref() {
            if let Some(parent) = source_maps_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                source_maps_path,
                serde_json::to_vec_pretty(&response.source_maps)?,
            )?;
        }
        Ok(ObfuscationModuleOutput {
            name_cache_path: response.name_cache_path,
            output_files: response
                .output_files
                .into_iter()
                .map(|file| file.output_path)
                .collect(),
        })
    }

    fn validate_sdk_adapter(&self) -> Result<(), ObfuscationError> {
        if !self.adapter.arkguard_package.is_dir() {
            return Err(ObfuscationError::ArkguardPackageNotFound(
                self.adapter.arkguard_package.clone(),
            ));
        }
        if self.adapter.sdk_api_paths.is_empty()
            || self.adapter.sdk_api_paths.iter().all(|path| !path.exists())
        {
            return Err(ObfuscationError::ArkguardSdkApisNotFound(
                self.adapter.sdk_api_paths.clone(),
            ));
        }
        Ok(())
    }

    fn request(&self, input: &ObfuscationModuleInput) -> Result<ArkguardRequest, ObfuscationError> {
        let module_path_map = self
            .adapter
            .module_path_map
            .iter()
            .map(|(name, path)| Ok((name.clone(), ArkguardPath::new(path).text()?)))
            .collect::<Result<BTreeMap<_, _>, ObfuscationError>>()?;
        let files = input
            .files
            .iter()
            .map(|file| {
                Ok(ArkguardRequestFile {
                    build_path: ArkguardPath::new(&file.build_path).text()?,
                    original_path: ArkguardPath::new(&file.original_path).text()?,
                    relative_path: file.relative_path.clone(),
                    source_map_key: file.source_map_key.clone(),
                    previous_source_map: file.previous_source_map.clone(),
                })
            })
            .collect::<Result<Vec<_>, ObfuscationError>>()?;
        Ok(ArkguardRequest {
            arkguard_path: ArkguardPath::new(&self.adapter.arkguard_package).text()?,
            rule_files: ArkguardPath::many(&self.options.rule_files)?,
            dependency_rule_files: ArkguardPath::many(&self.options.dependency_rule_files)?,
            sdk_apis: ArkguardPath::many(&self.adapter.sdk_api_paths)?,
            cache_dir: ArkguardPath::new(&self.options.cache_dir).text()?,
            export_rule_path: self
                .options
                .export_rule_path
                .as_deref()
                .map(|path| ArkguardPath::new(path).text())
                .transpose()?
                .unwrap_or_default(),
            module_root: ArkguardPath::new(&self.adapter.module_root).text()?,
            project_root: ArkguardPath::new(&self.adapter.project_root).text()?,
            project_path: ArkguardPath::new(&self.adapter.project_path).text()?,
            cache_path: ArkguardPath::new(&self.adapter.cache_path).text()?,
            ace_module_build: ArkguardPath::new(&self.adapter.ace_module_build).text()?,
            ace_module_json_path: ArkguardPath::new(&self.adapter.ace_module_json_path).text()?,
            compile_sdk_version: self.adapter.compile_sdk_version.clone(),
            entry_package_info: self.adapter.entry_package_info.clone(),
            package_dir: self.adapter.package_dir.clone(),
            module_path_map,
            entry_array: self.adapter.entry_array.clone(),
            local_packages: self.adapter.local_packages.clone(),
            compile_har: self.adapter.compile_har,
            compile_shared: self.adapter.compile_shared,
            use_normalized: self.adapter.use_normalized,
            use_ts_har: self.adapter.use_ts_har,
            files,
        })
    }
}

struct ArkguardPath<'a> {
    path: &'a Path,
}

impl<'a> ArkguardPath<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn text(&self) -> Result<String, ObfuscationError> {
        Ok(ArtifactPathSerializer::path_to_slash_string(self.path)?)
    }

    fn many(paths: &[PathBuf]) -> Result<Vec<String>, ObfuscationError> {
        paths
            .iter()
            .map(|path| {
                ArtifactPathSerializer::path_to_slash_string(path).map_err(ObfuscationError::from)
            })
            .collect()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArkguardRequest {
    arkguard_path: String,
    rule_files: Vec<String>,
    dependency_rule_files: Vec<String>,
    sdk_apis: Vec<String>,
    cache_dir: String,
    export_rule_path: String,
    module_root: String,
    project_root: String,
    project_path: String,
    cache_path: String,
    ace_module_build: String,
    ace_module_json_path: String,
    compile_sdk_version: String,
    entry_package_info: String,
    package_dir: String,
    module_path_map: BTreeMap<String, String>,
    entry_array: Vec<String>,
    local_packages: Vec<String>,
    compile_har: bool,
    compile_shared: bool,
    use_normalized: bool,
    use_ts_har: bool,
    files: Vec<ArkguardRequestFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArkguardRequestFile {
    build_path: String,
    original_path: String,
    relative_path: String,
    source_map_key: String,
    previous_source_map: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArkguardResponse {
    source_maps: BTreeMap<String, serde_json::Value>,
    name_cache_path: PathBuf,
    output_files: Vec<ArkguardResponseFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArkguardResponseFile {
    output_path: PathBuf,
}
