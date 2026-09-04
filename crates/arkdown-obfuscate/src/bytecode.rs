use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_path::ArtifactPathSerializer;
use serde::{Deserialize, Serialize};

use crate::{ArkguardAdapterOptions, ObfuscationError, ObfuscationModuleInput, ObfuscationOptions};

/// Original source path to ABC record mapping used by SDK keep-rule handling.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationModuleRecord {
    pub source_path: PathBuf,
    pub record_name: String,
}

/// Inputs that become available only after `es2abc` completes.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationInput {
    pub module: ObfuscationModuleInput,
    pub modules_abc_path: PathBuf,
    pub module_records: Vec<BytecodeObfuscationModuleRecord>,
    pub record_name_white_list: Vec<String>,
}

/// SDK bytecode-obfuscation artifacts retained in the CompileArkTS cache.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationOutput {
    pub modules_abc_path: PathBuf,
    pub config_path: PathBuf,
    pub name_cache_path: PathBuf,
    pub source_maps_path: PathBuf,
    pub origin_abc_path: PathBuf,
    pub obfuscated_abc_path: PathBuf,
}

pub(crate) struct ArkguardBytecodeRunner<'a> {
    options: &'a ObfuscationOptions,
    adapter: &'a ArkguardAdapterOptions,
}

impl<'a> ArkguardBytecodeRunner<'a> {
    pub(crate) fn new(
        options: &'a ObfuscationOptions,
        adapter: &'a ArkguardAdapterOptions,
    ) -> Self {
        Self { options, adapter }
    }

    pub(crate) fn run(
        &self,
        input: BytecodeObfuscationInput,
    ) -> Result<BytecodeObfuscationOutput, ObfuscationError> {
        self.validate(&input)?;
        std::fs::create_dir_all(&self.options.cache_dir)?;
        let bridge_path = self
            .options
            .cache_dir
            .join("arkdown-arkguard-bytecode-bridge.cjs");
        let request_path = self.options.cache_dir.join(format!(
            "arkdown-arkguard-bytecode-request-{}.json",
            std::process::id()
        ));
        let response_path = self.options.cache_dir.join(format!(
            "arkdown-arkguard-bytecode-response-{}.json",
            std::process::id()
        ));
        std::fs::write(&bridge_path, include_str!("bytecode_arkguard_bridge.cjs"))?;
        std::fs::write(&request_path, serde_json::to_vec(&self.request(&input)?)?)?;
        let process = Command::new(&self.adapter.node_executable)
            .arg(&bridge_path)
            .arg(&request_path)
            .arg(&response_path)
            .output()
            .map_err(|source| ObfuscationError::ArkguardProcessStart {
                executable: self.adapter.node_executable.clone(),
                source,
            });
        let _ = std::fs::remove_file(&request_path);
        let _ = std::fs::remove_file(&bridge_path);
        let process = process?;
        if !process.status.success() {
            let _ = std::fs::remove_file(&response_path);
            return Err(ObfuscationError::ArkguardProcessFailed {
                status: process.status.code(),
                stderr: String::from_utf8_lossy(&process.stderr).trim().to_string(),
            });
        }
        let response: BytecodeArkguardResponse =
            serde_json::from_slice(&std::fs::read(&response_path)?)?;
        let _ = std::fs::remove_file(&response_path);
        self.validate_response(&response)?;

        let bundle_source_maps = input
            .module
            .source_maps_path
            .as_ref()
            .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?;
        if response.source_maps_path != *bundle_source_maps {
            std::fs::copy(&response.source_maps_path, bundle_source_maps)?;
        }

        Ok(BytecodeObfuscationOutput {
            modules_abc_path: response.modules_abc_path,
            config_path: response.config_path,
            name_cache_path: response.name_cache_path,
            source_maps_path: response.source_maps_path,
            origin_abc_path: response.origin_abc_path,
            obfuscated_abc_path: response.obfuscated_abc_path,
        })
    }

    fn validate(&self, input: &BytecodeObfuscationInput) -> Result<(), ObfuscationError> {
        if !self.adapter.ets_loader_path.is_dir() {
            return Err(ObfuscationError::EtsLoaderNotFound(
                self.adapter.ets_loader_path.clone(),
            ));
        }
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
        if self.adapter.compatible_sdk_version < 13
            && !(self.adapter.compatible_sdk_version == 12
                && self.adapter.compatible_sdk_version_stage.as_deref() == Some("beta3"))
        {
            return Err(
                ObfuscationError::BytecodeObfuscationUnsupportedCompatibleSdk {
                    version: self.adapter.compatible_sdk_version,
                    stage: self.adapter.compatible_sdk_version_stage.clone(),
                },
            );
        }
        if !input.modules_abc_path.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                input.modules_abc_path.clone(),
            ));
        }
        let expected_abc = self.adapter.ace_module_build.join("modules.abc");
        if input.modules_abc_path != expected_abc {
            return Err(ObfuscationError::BytecodeObfuscationAbcPathMismatch {
                expected: expected_abc,
                actual: input.modules_abc_path.clone(),
            });
        }
        let source_maps = input
            .module
            .source_maps_path
            .as_ref()
            .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?;
        if !source_maps.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                source_maps.clone(),
            ));
        }
        let files_info = self.adapter.cache_path.join("filesInfo.txt");
        if !files_info.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                files_info,
            ));
        }
        Ok(())
    }

    fn validate_response(
        &self,
        response: &BytecodeArkguardResponse,
    ) -> Result<(), ObfuscationError> {
        for path in [
            &response.modules_abc_path,
            &response.config_path,
            &response.name_cache_path,
            &response.source_maps_path,
            &response.origin_abc_path,
            &response.obfuscated_abc_path,
        ] {
            if !path.is_file() {
                return Err(ObfuscationError::BytecodeObfuscationOutputNotFound(
                    path.clone(),
                ));
            }
        }
        Ok(())
    }

    fn request(
        &self,
        input: &BytecodeObfuscationInput,
    ) -> Result<BytecodeArkguardRequest, ObfuscationError> {
        let module_path_map = self
            .adapter
            .module_path_map
            .iter()
            .map(|(name, path)| Ok((name.clone(), BytecodeArkguardPath::new(path).text()?)))
            .collect::<Result<_, ObfuscationError>>()?;
        let files = input
            .module
            .files
            .iter()
            .map(|file| {
                Ok(BytecodeArkguardFile {
                    build_path: BytecodeArkguardPath::new(&file.build_path).text()?,
                    original_path: BytecodeArkguardPath::new(&file.original_path).text()?,
                    insight_intent_global_names: file.insight_intent_global_names.clone(),
                    insight_intent_property_names: file.insight_intent_property_names.clone(),
                })
            })
            .collect::<Result<Vec<_>, ObfuscationError>>()?;
        let module_records = input
            .module_records
            .iter()
            .map(|record| {
                Ok(BytecodeArkguardModuleRecord {
                    source_path: BytecodeArkguardPath::new(&record.source_path).text()?,
                    record_name: record.record_name.clone(),
                })
            })
            .collect::<Result<Vec<_>, ObfuscationError>>()?;
        Ok(BytecodeArkguardRequest {
            ets_loader_path: BytecodeArkguardPath::new(&self.adapter.ets_loader_path).text()?,
            arkguard_path: BytecodeArkguardPath::new(&self.adapter.arkguard_package).text()?,
            rule_files: BytecodeArkguardPath::many(&self.options.rule_files)?,
            dependency_rule_files: BytecodeArkguardPath::many(&self.options.dependency_rule_files)?,
            sdk_apis: BytecodeArkguardPath::many(&self.adapter.sdk_api_paths)?,
            cache_dir: BytecodeArkguardPath::new(&self.options.cache_dir).text()?,
            export_rule_path: self
                .options
                .export_rule_path
                .as_deref()
                .map(|path| BytecodeArkguardPath::new(path).text())
                .transpose()?
                .unwrap_or_default(),
            module_root: BytecodeArkguardPath::new(&self.adapter.module_root).text()?,
            project_root: BytecodeArkguardPath::new(&self.adapter.project_root).text()?,
            project_path: BytecodeArkguardPath::new(&self.adapter.project_path).text()?,
            cache_path: BytecodeArkguardPath::new(&self.adapter.cache_path).text()?,
            ace_module_build: BytecodeArkguardPath::new(&self.adapter.ace_module_build).text()?,
            ace_module_json_path: BytecodeArkguardPath::new(&self.adapter.ace_module_json_path)
                .text()?,
            compile_sdk_version: self.adapter.compile_sdk_version.clone(),
            compatible_sdk_version: self.adapter.compatible_sdk_version,
            compatible_sdk_version_stage: self.adapter.compatible_sdk_version_stage.clone(),
            entry_package_info: self.adapter.entry_package_info.clone(),
            package_dir: self.adapter.package_dir.clone(),
            module_path_map,
            entry_array: self.adapter.entry_array.clone(),
            resource_table_path: self
                .adapter
                .resource_table_path
                .as_deref()
                .map(|path| BytecodeArkguardPath::new(path).text())
                .transpose()?,
            local_packages: self.adapter.local_packages.clone(),
            compile_har: self.adapter.compile_har,
            compile_shared: self.adapter.compile_shared,
            use_normalized: self.adapter.use_normalized,
            use_ts_har: self.adapter.use_ts_har,
            byte_code_har_info: self.adapter.byte_code_har_info.clone(),
            modules_abc_path: BytecodeArkguardPath::new(&input.modules_abc_path).text()?,
            source_maps_path: BytecodeArkguardPath::new(
                input
                    .module
                    .source_maps_path
                    .as_ref()
                    .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?,
            )
            .text()?,
            record_name_white_list: input.record_name_white_list.clone(),
            module_records,
            files,
        })
    }
}

struct BytecodeArkguardPath<'a> {
    path: &'a Path,
}

impl<'a> BytecodeArkguardPath<'a> {
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
struct BytecodeArkguardRequest {
    ets_loader_path: String,
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
    compatible_sdk_version: u32,
    compatible_sdk_version_stage: Option<String>,
    entry_package_info: String,
    package_dir: String,
    module_path_map: std::collections::BTreeMap<String, String>,
    entry_array: Vec<String>,
    resource_table_path: Option<String>,
    local_packages: Vec<String>,
    compile_har: bool,
    compile_shared: bool,
    use_normalized: bool,
    use_ts_har: bool,
    byte_code_har_info: serde_json::Value,
    modules_abc_path: String,
    source_maps_path: String,
    record_name_white_list: Vec<String>,
    module_records: Vec<BytecodeArkguardModuleRecord>,
    files: Vec<BytecodeArkguardFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeArkguardFile {
    build_path: String,
    original_path: String,
    insight_intent_global_names: Vec<String>,
    insight_intent_property_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeArkguardModuleRecord {
    source_path: String,
    record_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeArkguardResponse {
    modules_abc_path: PathBuf,
    config_path: PathBuf,
    name_cache_path: PathBuf,
    source_maps_path: PathBuf,
    origin_abc_path: PathBuf,
    obfuscated_abc_path: PathBuf,
}
