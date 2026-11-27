use std::ffi::{OsStr, OsString};
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

    fn configure_library_search_path(&self, command: &mut Command) {
        let search_dirs = restool_library_search_dirs(&self.restool_path);
        if search_dirs.is_empty() {
            return;
        }
        let variable = restool_library_env_var();
        let existing = std::env::var_os(variable);
        if let Some(joined) = prepend_library_search_paths(&search_dirs, existing.as_deref()) {
            command.env(variable, joined);
        }
    }
}

fn restool_library_env_var() -> &'static str {
    if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else if cfg!(windows) {
        "PATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

fn restool_library_search_dirs(restool_path: &Path) -> Vec<PathBuf> {
    let Some(toolchains_dir) = restool_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
    else {
        return Vec::new();
    };

    let mut candidates = Vec::with_capacity(3);
    if let Some(component_dir) = toolchains_dir
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
    {
        // `libimage_transcoder_shared` links `liblz4_shared` from `openharmony/previewer/common/bin`.
        candidates.push(component_dir.join("previewer").join("common").join("bin"));
        if let Some(sdk_root) = component_dir
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
        {
            candidates.push(sdk_root.join("hms").join("toolchains").join("lib"));
        }
    }
    candidates.push(toolchains_dir.join("lib"));

    let mut dirs = Vec::new();
    for dir in candidates {
        if dir.is_dir() && !dirs.iter().any(|existing| existing == &dir) {
            dirs.push(dir);
        }
    }
    dirs
}

fn prepend_library_search_paths(prepend: &[PathBuf], existing: Option<&OsStr>) -> Option<OsString> {
    if prepend.is_empty() {
        return None;
    }

    let mut merged = Vec::new();
    for dir in prepend {
        if !dir.as_os_str().is_empty() && !merged.iter().any(|seen| seen == dir) {
            merged.push(dir.clone());
        }
    }
    if merged.is_empty() {
        return None;
    }

    if let Some(existing) = existing {
        for dir in std::env::split_paths(existing) {
            if !dir.as_os_str().is_empty() && !merged.iter().any(|seen| seen == &dir) {
                merged.push(dir);
            }
        }
    }

    std::env::join_paths(merged).ok()
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
        self.configure_library_search_path(&mut command);
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

        let output = command.output().map_err(ResourceError::Io)?;
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
            "plural" => 10004,
            "boolean" => 10005,
            "intarray" => 10006,
            "integer" => 10007,
            "pattern" => 10008,
            "strarray" => 10009,
            "media" => 20000,
            "rawfile" => 30000,
            "symbol" => 40000,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod runtime_type_tests {
    use super::ResourceRuntimeTypeClassifier;

    #[test]
    fn matches_source_compiler_resource_type_ids() {
        let classifier = ResourceRuntimeTypeClassifier::new();
        let expected = [
            ("color.value", 10001),
            ("float.value", 10002),
            ("string.value", 10003),
            ("plural.value", 10004),
            ("boolean.value", 10005),
            ("intarray.value", 10006),
            ("integer.value", 10007),
            ("pattern.value", 10008),
            ("strarray.value", 10009),
            ("media.value", 20000),
            ("rawfile.value", 30000),
            ("symbol.value", 40000),
        ];
        for (key, runtime_type) in expected {
            assert_eq!(classifier.runtime_type_from_key(key), runtime_type, "{key}");
        }
    }
}

#[cfg(test)]
mod restool_library_path_tests {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        prepend_library_search_paths, restool_library_env_var, restool_library_search_dirs,
        SdkRestoolBackend,
    };

    #[test]
    fn discovers_existing_sdk_library_dirs_in_priority_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk_default = temp.path().join("default");
        let toolchains = sdk_default.join("openharmony").join("toolchains");
        let previewer_bin = sdk_default
            .join("openharmony")
            .join("previewer")
            .join("common")
            .join("bin");
        let hms_lib = sdk_default.join("hms").join("toolchains").join("lib");
        let toolchains_lib = toolchains.join("lib");

        fs_err::create_dir_all(&toolchains).expect("create toolchains");
        fs_err::create_dir_all(&previewer_bin).expect("create previewer bin");
        fs_err::create_dir_all(&hms_lib).expect("create hms lib");
        fs_err::create_dir_all(&toolchains_lib).expect("create toolchains lib");

        let restool_path = toolchains.join("restool");
        assert_eq!(
            restool_library_search_dirs(&restool_path),
            vec![previewer_bin, hms_lib, toolchains_lib]
        );
    }

    #[test]
    fn omits_missing_sdk_library_dirs_and_handles_bare_executable_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk_default = temp.path().join("default");
        let toolchains = sdk_default.join("openharmony").join("toolchains");
        let previewer_bin = sdk_default
            .join("openharmony")
            .join("previewer")
            .join("common")
            .join("bin");

        fs_err::create_dir_all(&toolchains).expect("create toolchains");
        fs_err::create_dir_all(&previewer_bin).expect("create previewer bin");

        let restool_path = toolchains.join("restool");
        assert_eq!(
            restool_library_search_dirs(&restool_path),
            vec![previewer_bin]
        );
        assert!(restool_library_search_dirs(Path::new("restool")).is_empty());
    }

    #[test]
    fn prepends_sdk_library_dirs_while_preserving_and_deduplicating_existing_paths() {
        let dir_a = PathBuf::from("/sdk/openharmony/previewer/common/bin");
        let dir_b = PathBuf::from("/sdk/hms/toolchains/lib");
        let dir_c = PathBuf::from("/usr/local/lib");

        assert_eq!(
            prepend_library_search_paths(&[], Some(OsStr::new("/usr/local/lib"))),
            None
        );

        let joined_fresh =
            prepend_library_search_paths(&[dir_a.clone(), dir_b.clone()], None).expect("joined");
        assert_eq!(
            std::env::split_paths(&joined_fresh).collect::<Vec<_>>(),
            vec![dir_a.clone(), dir_b.clone()]
        );

        let existing =
            std::env::join_paths([dir_b.clone(), dir_c.clone(), dir_a.clone()]).expect("existing");
        let joined_merged = prepend_library_search_paths(
            &[dir_a.clone(), dir_b.clone(), dir_a.clone()],
            Some(existing.as_os_str()),
        )
        .expect("merged");
        assert_eq!(
            std::env::split_paths(&joined_merged).collect::<Vec<_>>(),
            vec![dir_a.clone(), dir_b, dir_c]
        );

        let joined_empty_existing =
            prepend_library_search_paths(std::slice::from_ref(&dir_a), Some(OsStr::new("")))
                .expect("empty existing");
        assert_eq!(
            std::env::split_paths(&joined_empty_existing).collect::<Vec<_>>(),
            vec![dir_a]
        );
    }

    #[test]
    fn configure_library_search_path_sets_command_env_only_when_dirs_exist() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk_default = temp.path().join("default");
        let toolchains = sdk_default.join("openharmony").join("toolchains");
        let previewer_bin = sdk_default
            .join("openharmony")
            .join("previewer")
            .join("common")
            .join("bin");
        let hms_lib = sdk_default.join("hms").join("toolchains").join("lib");

        fs_err::create_dir_all(&toolchains).expect("create toolchains");
        fs_err::create_dir_all(&previewer_bin).expect("create previewer bin");
        fs_err::create_dir_all(&hms_lib).expect("create hms lib");

        let restool_path = toolchains.join("restool");
        let backend = SdkRestoolBackend::new(restool_path.clone());
        let mut command = Command::new(&restool_path);
        backend.configure_library_search_path(&mut command);

        let env_var = OsStr::new(restool_library_env_var());
        let env_value = command
            .get_envs()
            .find_map(|(key, value)| (key == env_var).then_some(value))
            .flatten()
            .expect("library search path env var should be configured");
        let configured_paths: Vec<PathBuf> = std::env::split_paths(env_value).collect();
        assert_eq!(configured_paths[0], previewer_bin);
        assert_eq!(configured_paths[1], hms_lib);

        let empty_temp = tempfile::tempdir().expect("empty tempdir");
        let isolated_restool = empty_temp.path().join("restool");
        let isolated_backend = SdkRestoolBackend::new(isolated_restool.clone());
        let mut isolated_command = Command::new(&isolated_restool);
        isolated_backend.configure_library_search_path(&mut isolated_command);
        assert!(isolated_command.get_envs().all(|(key, _)| key != env_var));
    }
}
