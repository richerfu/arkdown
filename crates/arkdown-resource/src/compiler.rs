//! Resource compiler orchestration.
//!
//! [`ResourceCompiler`] walks one or more `resources/` directories, compiles
//! element JSON files, copies media and profile files, and writes
//! `resources.index` and `ResourceTable.txt` to the output directory.

use std::collections::BTreeMap;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::ResourceError;
use crate::ids::ResourceIdAllocator;
use crate::layout::ResourceOutputLayout;
use crate::scan::ResourceCategory;
use crate::{element, index, scan, table};

/// Build-time configuration for the resource compiler.
#[derive(Debug, Clone)]
pub struct ResourceCompileConfig {
    /// Application bundle name (e.g. `"com.example.myapp"`).
    pub package_name: String,
    /// HAP module name (e.g. `"entry"`).
    pub module_name: String,
    /// Full restool `moduleNames` set. When present, restool sorts the module
    /// names and chooses this module's resource ID cluster from its sorted
    /// position.
    pub module_names: Vec<String>,
    /// Target device types (e.g. `["phone", "tablet"]`).
    pub device_types: Vec<String>,
    /// Whether this is a debug build.
    pub is_debug: bool,
}

/// Input paths for a single compilation run.
#[derive(Debug, Clone)]
pub struct ResourceCompileInput {
    /// One or more `resources/` roots to process (merged in order).
    pub resources_dirs: Vec<PathBuf>,
    /// Directory where all outputs will be written.
    pub output_dir: PathBuf,
}

/// Paths of all artifacts produced by a successful compilation.
#[derive(Debug, Clone)]
pub struct ResourceCompileOutput {
    /// Path to the generated `resources.index` binary.
    pub resources_index: PathBuf,
    /// Path to the generated `ResourceTable.txt`.
    pub resource_table: PathBuf,
    /// Path to the generated `ids_map/id_defined.json`.
    pub ids_defined: PathBuf,
    /// Directory whose contents should be packaged under `resources/` in HAP/HSP.
    pub resources_dir: PathBuf,
    /// `(name, path)` pairs for every profile file that was copied.
    pub profiles: Vec<(String, PathBuf)>,
    /// Map of resource name → assigned numeric ID (e.g. `"app_name"` → `0x01000000`).
    ///
    /// These IDs are used by the profile processor to write `iconId`, `labelId`, and
    /// `descriptionId` into `module.json`.
    pub resource_ids: FxHashMap<String, u32>,
    /// Map of `"type.name"` → runtime resource info used by ets-loader's
    /// `$r()` transform, e.g. `"string.app_name"` or `"media.icon"`.
    pub resource_infos: FxHashMap<String, ResourceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceInfo {
    pub id: u32,
    pub runtime_type: u32,
}

/// A single compiled resource entry ready to be written to `resources.index`.
#[derive(Debug, Clone)]
pub struct CompiledEntry {
    /// Unique resource identifier (assigned sequentially, starting at `0x01000000`).
    pub id: u32,
    /// Resource type code (`0x4` = string, `0x5` = integer, `0x6` = float,
    /// `0xA` = color, `0x1` = media).
    pub entry_type: u32,
    /// Resource name (matches the `"name"` field in element JSON, or the file
    /// stem for media/profile files).
    pub name: String,
    /// String representation of the value (for element resources) or a
    /// relative path (for media resources).
    pub value: String,
    /// Qualifier string; empty for the `base` qualifier.
    pub qualifier: String,
}

/// Newtype wrapper around a raw resource type code (`u32`).
///
/// Provides named methods for the two common mappings:
/// - [`type_name`](ResourceTypeCode::type_name) — human-readable name used in
///   `ResourceTable.txt` and `id_defined.json`.
/// - [`runtime_type`](ResourceTypeCode::runtime_type) — numeric runtime type
///   used by the `$r()` transform in ets-loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceTypeCode(pub u32);

impl ResourceTypeCode {
    /// Map this type code to its human-readable name.
    ///
    /// Returns `"unknown"` for unrecognised codes.
    pub fn type_name(self) -> &'static str {
        match self.0 {
            0x08 => "integer",
            0x12 => "float",
            0x09 => "string",
            0x0E => "color",
            0x13 => "media",
            0x14 => "profile",
            _ => "unknown",
        }
    }

    /// Map this type code to the runtime resource type integer consumed by the
    /// ArkTS `$r()` transform.
    ///
    /// Returns `0` for unrecognised codes.
    pub fn runtime_type(self) -> u32 {
        match self.0 {
            0x0E => 10001,
            0x12 => 10002,
            0x09 => 10003,
            0x08 => 10007,
            0x13 => 20000,
            _ => 0,
        }
    }
}

#[derive(Debug)]
struct ResourceCopy {
    source: PathBuf,
    relative_path: PathBuf,
    category: ResourceCategory,
}

#[derive(Debug, Clone)]
struct PendingEntry {
    entry_type: ResourceTypeCode,
    name: String,
    value: String,
    qualifier: String,
    resource_key: Option<String>,
    runtime_type: u32,
}

/// Pure-Rust implementation of OpenHarmony's `restool` resource compiler.
pub struct ResourceCompiler {
    config: ResourceCompileConfig,
}

impl ResourceCompiler {
    /// Create a new compiler with the given configuration.
    pub fn new(config: ResourceCompileConfig) -> Self {
        ResourceCompiler { config }
    }

    /// Compile all resources found in `input.resources_dirs` and write outputs
    /// to `input.output_dir`.
    pub fn compile(
        &self,
        input: ResourceCompileInput,
    ) -> Result<ResourceCompileOutput, ResourceError> {
        let layout = ResourceOutputLayout::new(input.output_dir.clone());
        fs_err::create_dir_all(layout.root())?;

        let mut all_entries: Vec<CompiledEntry> = Vec::new();
        let mut profiles: Vec<(String, PathBuf)> = Vec::new();
        let mut resource_copies: Vec<ResourceCopy> = Vec::new();
        let mut emitted_resources: FxHashSet<(String, u32, String)> = FxHashSet::default();
        let mut id_allocator =
            ResourceIdAllocator::new(&self.config.module_name, &self.config.module_names);
        let mut resource_infos: FxHashMap<String, ResourceInfo> = FxHashMap::default();

        for res_dir in &input.resources_dirs {
            let mut files = scan::scan_resources(res_dir);
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let mut seen_in_module: FxHashSet<(String, u32, String)> = FxHashSet::default();
            let mut element_entries: BTreeMap<(u32, String), Vec<PendingEntry>> = BTreeMap::new();
            let mut media_entries: BTreeMap<(u32, String), Vec<PendingEntry>> = BTreeMap::new();
            let mut profile_entries: BTreeMap<(u32, String), Vec<PendingEntry>> = BTreeMap::new();

            for file in &files {
                match file.category {
                    ResourceCategory::Element => {
                        let entries = element::parse_element_file(&file.path);
                        for e in entries {
                            let entry_type = ResourceTypeCode(e.entry_type.resource_type_id());
                            if !seen_in_module.insert((
                                file.qualifier.0.clone(),
                                entry_type.0,
                                e.name.clone(),
                            )) {
                                continue;
                            }
                            let runtime_type = entry_type.runtime_type();
                            let resource_key =
                                format!("{}.{}", entry_type.type_name(), e.name.clone());
                            element_entries
                                .entry((entry_type.0, e.name.clone()))
                                .or_default()
                                .push(PendingEntry {
                                    entry_type,
                                    name: e.name,
                                    value: e.value,
                                    qualifier: file.qualifier.0.clone(),
                                    resource_key: Some(resource_key),
                                    runtime_type,
                                });
                        }
                    }

                    ResourceCategory::Media => {
                        let filename = file.path.file_name().expect("media file has no filename");
                        let qualifier = qualifier_dir(&file.qualifier.0);
                        let entry_type = ResourceTypeCode(0x13); // MEDIA type (matches restool 6.0)
                        if !seen_in_module.insert((
                            file.qualifier.0.clone(),
                            entry_type.0,
                            file.name.clone(),
                        )) {
                            continue;
                        }
                        let relative_path = PathBuf::from(qualifier).join("media").join(filename);
                        resource_copies.push(ResourceCopy {
                            source: file.path.clone(),
                            relative_path: relative_path.clone(),
                            category: file.category.clone(),
                        });
                        media_entries
                            .entry((entry_type.0, file.name.clone()))
                            .or_default()
                            .push(PendingEntry {
                                entry_type,
                                name: file.name.clone(),
                                value: resource_value_path(
                                    &self.config.module_name,
                                    &relative_path,
                                ),
                                qualifier: file.qualifier.0.clone(),
                                resource_key: Some(format!("media.{}", file.name)),
                                runtime_type: entry_type.runtime_type(),
                            });
                    }

                    ResourceCategory::Profile => {
                        let filename = file.path.file_name().expect("profile file has no filename");
                        let qualifier = qualifier_dir(&file.qualifier.0);
                        let entry_type = ResourceTypeCode(0x14); // PROFILE type (matches restool 6.0)
                        if !seen_in_module.insert((
                            file.qualifier.0.clone(),
                            entry_type.0,
                            file.name.clone(),
                        )) {
                            continue;
                        }
                        let relative_path = PathBuf::from(qualifier).join("profile").join(filename);
                        resource_copies.push(ResourceCopy {
                            source: file.path.clone(),
                            relative_path: relative_path.clone(),
                            category: file.category.clone(),
                        });
                        let out_path = layout.resource_path(&relative_path);
                        profiles.push((file.name.clone(), out_path));
                        profile_entries
                            .entry((entry_type.0, file.name.clone()))
                            .or_default()
                            .push(PendingEntry {
                                entry_type,
                                name: file.name.clone(),
                                value: resource_value_path(
                                    &self.config.module_name,
                                    &relative_path,
                                ),
                                qualifier: file.qualifier.0.clone(),
                                resource_key: None,
                                runtime_type: entry_type.runtime_type(),
                            });
                    }

                    ResourceCategory::Rawfile => {
                        let Some(res_root) = input
                            .resources_dirs
                            .iter()
                            .find(|dir| file.path.starts_with(dir))
                        else {
                            continue;
                        };
                        let Ok(relative_path) = file.path.strip_prefix(res_root) else {
                            continue;
                        };
                        resource_copies.push(ResourceCopy {
                            source: file.path.clone(),
                            relative_path: relative_path.to_path_buf(),
                            category: file.category.clone(),
                        });
                    }
                }
            }

            commit_pending_entries(
                &element_entries,
                &mut id_allocator,
                &mut resource_infos,
                &mut emitted_resources,
                &mut all_entries,
            );
            commit_pending_entries(
                &media_entries,
                &mut id_allocator,
                &mut resource_infos,
                &mut emitted_resources,
                &mut all_entries,
            );
            commit_pending_entries(
                &profile_entries,
                &mut id_allocator,
                &mut resource_infos,
                &mut emitted_resources,
                &mut all_entries,
            );
        }

        let resources_dir = layout.resources_dir();
        copy_resource_files(
            &resource_copies,
            &resources_dir,
            id_allocator.resource_ids(),
        )?;

        // Write binary index.
        let index_path = layout.resources_index();
        index::write_resources_index(&all_entries, &index_path)?;

        // Write human-readable table.
        let table_path = layout.resource_table();
        table::write_resource_table(&all_entries, &table_path)?;

        let ids_defined_path = layout.ids_defined();
        write_id_defined(&all_entries, &ids_defined_path)?;

        Ok(ResourceCompileOutput {
            resources_index: index_path,
            resource_table: table_path,
            ids_defined: ids_defined_path,
            resources_dir,
            profiles,
            resource_ids: id_allocator.into_resource_ids(),
            resource_infos,
        })
    }
}

fn commit_pending_entries(
    groups: &BTreeMap<(u32, String), Vec<PendingEntry>>,
    id_allocator: &mut ResourceIdAllocator,
    resource_infos: &mut FxHashMap<String, ResourceInfo>,
    emitted_resources: &mut FxHashSet<(String, u32, String)>,
    all_entries: &mut Vec<CompiledEntry>,
) {
    for ((entry_type, id_name), entries) in groups {
        let id = id_allocator.assign(*entry_type, id_name);
        for entry in entries {
            if let Some(resource_key) = &entry.resource_key {
                resource_infos
                    .entry(resource_key.clone())
                    .or_insert(ResourceInfo {
                        id,
                        runtime_type: entry.runtime_type,
                    });
            }
            if !emitted_resources.insert((
                entry.qualifier.clone(),
                entry.entry_type.0,
                entry.name.clone(),
            )) {
                continue;
            }
            all_entries.push(CompiledEntry {
                id,
                entry_type: entry.entry_type.0,
                name: entry.name.clone(),
                value: entry.value.clone(),
                qualifier: entry.qualifier.clone(),
            });
        }
    }
}

fn qualifier_dir(qualifier: &str) -> &str {
    if qualifier.is_empty() {
        "base"
    } else {
        qualifier
    }
}

fn resource_value_path(module_name: &str, relative_path: &Path) -> String {
    format!(
        "{}/resources/{}",
        module_name,
        relative_path.to_string_lossy().replace('\\', "/")
    )
}

fn copy_resource_files(
    copies: &[ResourceCopy],
    resources_dir: &Path,
    resource_ids: &FxHashMap<String, u32>,
) -> Result<(), ResourceError> {
    fs_err::create_dir_all(resources_dir)?;
    for copy in copies {
        let out_path = resources_dir.join(&copy.relative_path);
        if let Some(parent) = out_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        if out_path.exists() {
            continue;
        }

        if copy.category == ResourceCategory::Media
            && copy.source.extension().and_then(|e| e.to_str()) == Some("json")
        {
            let content = fs_err::read_to_string(&copy.source)?;
            if let Ok(mut json) = serde_json::from_str::<Value>(&content) {
                rewrite_media_refs(&mut json, resource_ids);
                let bytes =
                    serde_json::to_vec_pretty(&json).unwrap_or_else(|_| content.into_bytes());
                fs_err::write(&out_path, bytes)?;
                continue;
            }
        }

        fs_err::copy(&copy.source, &out_path)?;
    }
    Ok(())
}

fn rewrite_media_refs(value: &mut Value, resource_ids: &FxHashMap<String, u32>) {
    match value {
        Value::String(s) => {
            if let Some(name) = s.strip_prefix("$media:") {
                if let Some(id) = resource_ids.get(name) {
                    *s = format!("$media:{id}");
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_media_refs(item, resource_ids);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                rewrite_media_refs(item, resource_ids);
            }
        }
        _ => {}
    }
}

fn write_id_defined(entries: &[CompiledEntry], output: &Path) -> Result<(), ResourceError> {
    if let Some(parent) = output.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let mut unique = entries.iter().collect::<Vec<_>>();
    unique.sort_by(|a, b| a.id.cmp(&b.id));
    unique.dedup_by(|a, b| a.id == b.id);
    let records = unique
        .iter()
        .map(|entry| {
            serde_json::json!({
                "type": table::type_name(entry.entry_type),
                "name": entry.name,
                "id": format!("0x{:08X}", entry.id),
            })
        })
        .collect::<Vec<_>>();
    let content = serde_json::to_vec_pretty(&serde_json::json!({ "record": records }))
        .expect("serializing id_defined.json should not fail");
    fs_err::write(output, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_compiler() -> ResourceCompiler {
        ResourceCompiler::new(ResourceCompileConfig {
            package_name: "com.example.test".into(),
            module_name: "entry".into(),
            module_names: vec![],
            device_types: vec!["phone".into()],
            is_debug: true,
        })
    }

    #[test]
    fn test_compile_minimal_resources() {
        let tmp = tempfile::tempdir().unwrap();
        let res_dir = tmp.path().join("resources/base/element");
        fs::create_dir_all(&res_dir).unwrap();
        fs::write(
            res_dir.join("string.json"),
            r#"{"string":[{"name":"app_name","value":"My App"}]}"#,
        )
        .unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("resources")],
                output_dir: out_dir.clone(),
            })
            .unwrap();

        assert!(result.resources_index.exists());
        assert!(result.resource_table.exists());
        assert!(result.ids_defined.exists());
        assert!(result.resources_dir.exists());

        let table = fs::read_to_string(&result.resource_table).unwrap();
        assert!(table.contains("app_name"));
        assert!(table.contains("string"));
    }

    #[test]
    fn test_compile_media_is_copied() {
        let tmp = tempfile::tempdir().unwrap();
        let media_dir = tmp.path().join("resources/base/media");
        fs::create_dir_all(&media_dir).unwrap();
        fs::write(media_dir.join("icon.png"), b"\x89PNG").unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("resources")],
                output_dir: out_dir.clone(),
            })
            .unwrap();

        assert!(out_dir.join("resources/base/media/icon.png").exists());

        let table = fs::read_to_string(&result.resource_table).unwrap();
        assert!(table.contains("icon"));
        assert!(table.contains("media"));
    }

    #[test]
    fn test_compile_profile_is_copied() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("resources/base/profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("router_map.json"), r#"{"routerMap":[]}"#).unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("resources")],
                output_dir: out_dir.clone(),
            })
            .unwrap();

        assert!(out_dir
            .join("resources/base/profile/router_map.json")
            .exists());
        assert_eq!(result.profiles.len(), 1);
        assert_eq!(result.profiles[0].0, "router_map");
    }

    #[test]
    fn test_compile_multiple_resource_dirs_merged() {
        let tmp = tempfile::tempdir().unwrap();

        let dir1 = tmp.path().join("res1/base/element");
        fs::create_dir_all(&dir1).unwrap();
        fs::write(
            dir1.join("string.json"),
            r#"{"string":[{"name":"hello","value":"Hello"}]}"#,
        )
        .unwrap();

        let dir2 = tmp.path().join("res2/base/element");
        fs::create_dir_all(&dir2).unwrap();
        fs::write(
            dir2.join("string.json"),
            r#"{"string":[{"name":"world","value":"World"}]}"#,
        )
        .unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("res1"), tmp.path().join("res2")],
                output_dir: out_dir.clone(),
            })
            .unwrap();

        let table = fs::read_to_string(&result.resource_table).unwrap();
        assert!(table.contains("hello"));
        assert!(table.contains("world"));
    }

    #[test]
    fn test_resources_index_binary_has_correct_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let res_dir = tmp.path().join("resources/base/element");
        fs::create_dir_all(&res_dir).unwrap();
        fs::write(
            res_dir.join("string.json"),
            r#"{"string":[{"name":"x","value":"y"}]}"#,
        )
        .unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("resources")],
                output_dir: out_dir,
            })
            .unwrap();

        let bytes = fs::read(&result.resources_index).unwrap();
        // Restool 6.0 magic: b"Restool 6.0.0.001\x00" (18 bytes)
        assert_eq!(&bytes[0..18], b"Restool 6.0.0.001\x00");
    }
}
