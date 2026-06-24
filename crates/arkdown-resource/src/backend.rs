use std::path::{Path, PathBuf};
use std::process::Command;

use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::error::ResourceError;
use crate::model::{
    ResourceCompileConfig, ResourceCompileInput, ResourceCompileOutput, ResourceInfo,
};
use crate::runtime_key::{ResourceRuntimeKey, ResourceRuntimeKeyText};

pub trait ResourceBackend: Send + Sync {
    fn compile(
        &self,
        config: &ResourceCompileConfig,
        input: ResourceCompileInput,
    ) -> Result<ResourceCompileOutput, ResourceError>;
}

#[derive(Debug, Clone)]
pub struct SdkRestoolBackend {
    restool_path: PathBuf,
}

impl SdkRestoolBackend {
    pub fn new(restool_path: PathBuf) -> Self {
        Self { restool_path }
    }
}

impl ResourceBackend for SdkRestoolBackend {
    fn compile(
        &self,
        config: &ResourceCompileConfig,
        input: ResourceCompileInput,
    ) -> Result<ResourceCompileOutput, ResourceError> {
        let output_dir = input.output_dir.clone();
        fs_err::create_dir_all(&output_dir)?;
        let ids_defined = output_dir.join("ids_map").join("id_defined.json");
        if let Some(parent) = ids_defined.parent() {
            fs_err::create_dir_all(parent)?;
        }

        let resource_table = output_dir.join("ResourceTable.txt");
        let module_json = output_dir.join("module.json");
        let res_config = output_dir.join("resConfig.json");

        let mut command = Command::new(&self.restool_path);
        if res_config.exists() {
            RestoolConfigFile::new(&res_config).prepare_output_paths()?;
            IdDefinedSeedFile::new(&ids_defined).ensure()?;
            command.arg("-l").arg(&res_config);
        } else {
            command
                .arg("--packageName")
                .arg(&config.package_name)
                .arg("--outputPath")
                .arg(&output_dir)
                .arg("--ids")
                .arg(&ids_defined)
                .arg("--forceWrite")
                // -r/--resHeader: resource header file path (required by restool).
                .arg("-r")
                .arg(&resource_table);

            // -j/--json: processed module.json for Stage model.
            // Written by the profile stage before resource compilation.
            if module_json.exists() {
                command.arg("-j").arg(&module_json);
            }

            if !config.module_names.is_empty() {
                command.arg("--modules").arg(config.module_names.join(","));
            }
            for dir in &input.resources_dirs {
                command.arg("--inputPath").arg(dir);
            }
        }

        let output = command
            .output()
            .map_err(|source| ResourceError::Io(source))?;
        if !output.status.success() {
            return Err(ResourceError::Backend {
                message: format!(
                    "restool failed with status {}: {}{}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let restool_ids = RestoolIdMapReader::new(&ids_defined).read()?;

        Ok(ResourceCompileOutput {
            resources_index: output_dir.join("resources.index"),
            resource_table: output_dir.join("ResourceTable.txt"),
            ids_defined,
            resources_dir: output_dir.join("resources"),
            profiles: Vec::new(),
            resource_ids: restool_ids.resource_ids,
            resource_infos: restool_ids.resource_infos,
        })
    }
}

pub struct ResourceBackendRunner {
    backend: Box<dyn ResourceBackend>,
}

impl ResourceBackendRunner {
    pub fn default_sdk(restool_path: PathBuf) -> Self {
        Self {
            backend: Box::new(SdkRestoolBackend::new(restool_path)),
        }
    }

    pub fn compile(
        &self,
        config: &ResourceCompileConfig,
        input: ResourceCompileInput,
    ) -> Result<ResourceCompileOutput, ResourceError> {
        self.backend.compile(config, input)
    }
}

#[derive(Debug, Deserialize)]
struct RestoolConfigFileContent {
    #[serde(default, rename = "ResourceTable")]
    resource_table: Vec<PathBuf>,
}

struct RestoolConfigFile<'a> {
    path: &'a Path,
}

impl<'a> RestoolConfigFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn prepare_output_paths(&self) -> Result<(), ResourceError> {
        let content = fs_err::read_to_string(self.path)?;
        let parsed: RestoolConfigFileContent =
            serde_json::from_str(&content).map_err(|error| ResourceError::Parse {
                file: self.path.display().to_string(),
                message: error.to_string(),
            })?;
        for path in parsed.resource_table {
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)?;
            }
        }
        Ok(())
    }
}

struct IdDefinedSeedFile<'a> {
    path: &'a Path,
}

impl<'a> IdDefinedSeedFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn ensure(&self) -> Result<(), ResourceError> {
        if self.path.exists() && fs_err::metadata(self.path)?.len() > 0 {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(self.path, br#"{"record":[]}"#)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IdMapFile {
    Record { record: Vec<IdMapEntry> },
    Entries(Vec<IdMapEntry>),
    Object(FxHashMap<String, IdValue>),
}

#[derive(Debug, Deserialize)]
struct IdMapEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    id: Option<IdValue>,
    #[serde(default, rename = "resourceId")]
    resource_id: Option<IdValue>,
    #[serde(default, rename = "type")]
    resource_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IdValue {
    Number(u32),
    Text(String),
}

impl IdValue {
    fn into_u32(self) -> Option<u32> {
        match self {
            IdValue::Number(id) => Some(id),
            IdValue::Text(text) => IdText::new(&text).parse(),
        }
    }
}

#[derive(Debug, Default)]
struct RestoolIds {
    resource_ids: FxHashMap<String, u32>,
    resource_infos: FxHashMap<String, ResourceInfo>,
}

struct RestoolIdMapReader<'a> {
    path: &'a Path,
}

impl<'a> RestoolIdMapReader<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn read(&self) -> Result<RestoolIds, ResourceError> {
        if !self.path.exists() {
            return Ok(RestoolIds::default());
        }
        let content = fs_err::read_to_string(self.path)?;
        let parsed: IdMapFile =
            serde_json::from_str(&content).map_err(|error| ResourceError::Parse {
                file: self.path.display().to_string(),
                message: error.to_string(),
            })?;
        let mut ids = RestoolIds::default();
        match parsed {
            IdMapFile::Record { record } => {
                RestoolIdEntries::new(record).insert_into(&mut ids);
            }
            IdMapFile::Entries(entries) => {
                RestoolIdEntries::new(entries).insert_into(&mut ids);
            }
            IdMapFile::Object(map) => RestoolIdObject::new(map).insert_into(&mut ids),
        }
        Ok(ids)
    }
}

struct RestoolIdEntries {
    entries: Vec<IdMapEntry>,
}

impl RestoolIdEntries {
    fn new(entries: Vec<IdMapEntry>) -> Self {
        Self { entries }
    }

    fn insert_into(self, ids: &mut RestoolIds) {
        let runtime_type_classifier = ResourceRuntimeTypeClassifier::new();
        for entry in self.entries {
            let Some(id) = entry.id.or(entry.resource_id).and_then(IdValue::into_u32) else {
                continue;
            };
            if entry.name.is_empty() {
                continue;
            }
            ids.resource_ids.entry(entry.name.clone()).or_insert(id);
            if let Some(resource_type) = entry.resource_type {
                let runtime_key = ResourceRuntimeKey::new(&resource_type, &entry.name).to_key();
                ids.resource_infos
                    .entry(runtime_key.clone())
                    .or_insert(ResourceInfo {
                        id,
                        runtime_type: runtime_type_classifier.runtime_type_from_key(&runtime_key),
                    });
            }
        }
    }
}

struct RestoolIdObject {
    values: FxHashMap<String, IdValue>,
}

impl RestoolIdObject {
    fn new(values: FxHashMap<String, IdValue>) -> Self {
        Self { values }
    }

    fn insert_into(self, ids: &mut RestoolIds) {
        let runtime_type_classifier = ResourceRuntimeTypeClassifier::new();
        for (key, value) in self.values {
            let Some(id) = value.into_u32() else {
                continue;
            };
            ids.resource_ids
                .entry(ResourceIdKeyText::new(&key).profile_name().to_string())
                .or_insert(id);
            ids.resource_infos
                .entry(key.clone())
                .or_insert(ResourceInfo {
                    id,
                    runtime_type: runtime_type_classifier.runtime_type_from_key(&key),
                });
        }
    }
}

struct ResourceIdKeyText<'a> {
    raw: &'a str,
}

impl<'a> ResourceIdKeyText<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn profile_name(&self) -> &'a str {
        self.raw
            .split_once('.')
            .map(|(_, name)| name)
            .unwrap_or(self.raw)
    }
}

struct IdText<'a> {
    raw: &'a str,
}

impl<'a> IdText<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn parse(&self) -> Option<u32> {
        let text = self.raw.trim();
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            return u32::from_str_radix(hex, 16).ok();
        }
        text.parse().ok()
    }
}

struct ResourceRuntimeTypeClassifier;

impl ResourceRuntimeTypeClassifier {
    fn new() -> Self {
        Self
    }

    fn runtime_type_from_key(&self, key: &str) -> u32 {
        match ResourceRuntimeKeyText::new(key).resource_type() {
            "color" => 10001,
            "float" => 10002,
            "string" => 10003,
            "integer" | "intarray" => 10007,
            "media" => 20000,
            _ => 0,
        }
    }
}
