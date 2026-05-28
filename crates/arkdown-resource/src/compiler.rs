//! Resource compiler orchestration.
//!
//! [`ResourceCompiler`] walks one or more `resources/` directories, compiles
//! element JSON files, copies media and profile files, and writes
//! `resources.index` and `ResourceTable.txt` to the output directory.

use std::collections::HashMap;
use std::path::PathBuf;
use crate::{element, index, scan, table};
use crate::error::ResourceError;
use crate::scan::ResourceCategory;

/// Build-time configuration for the resource compiler.
#[derive(Debug, Clone)]
pub struct ResourceCompileConfig {
    /// Application bundle name (e.g. `"com.example.myapp"`).
    pub package_name: String,
    /// HAP module name (e.g. `"entry"`).
    pub module_name: String,
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
    /// `(name, path)` pairs for every profile file that was copied.
    pub profiles: Vec<(String, PathBuf)>,
    /// Map of resource name → assigned numeric ID (e.g. `"app_name"` → `0x01000000`).
    ///
    /// These IDs are used by the profile processor to write `iconId`, `labelId`, and
    /// `descriptionId` into `module.json`.
    pub resource_ids: HashMap<String, u32>,
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

/// Pure-Rust implementation of OpenHarmony's `restool` resource compiler.
pub struct ResourceCompiler {
    _config: ResourceCompileConfig,
}

impl ResourceCompiler {
    /// Create a new compiler with the given configuration.
    pub fn new(config: ResourceCompileConfig) -> Self {
        ResourceCompiler { _config: config }
    }

    /// Compile all resources found in `input.resources_dirs` and write outputs
    /// to `input.output_dir`.
    pub fn compile(
        &self,
        input: ResourceCompileInput,
    ) -> Result<ResourceCompileOutput, ResourceError> {
        fs_err::create_dir_all(&input.output_dir)?;

        let mut all_entries: Vec<CompiledEntry> = Vec::new();
        let mut profiles: Vec<(String, PathBuf)> = Vec::new();
        // Resource IDs start at 0x01000000 and increment monotonically.
        let mut next_id: u32 = 0x0100_0000;
        // Tracks name → numeric ID for every resource that is assigned an ID.
        let mut resource_ids: HashMap<String, u32> = HashMap::new();

        for res_dir in &input.resources_dirs {
            let files = scan::scan_resources(res_dir);

            for file in &files {
                match file.category {
                    ResourceCategory::Element => {
                        let entries = element::parse_element_file(&file.path);
                        for e in entries {
                            resource_ids.insert(e.name.clone(), next_id);
                            all_entries.push(CompiledEntry {
                                id: next_id,
                                entry_type: e.entry_type.resource_type_id(),
                                name: e.name,
                                value: e.value,
                                qualifier: file.qualifier.0.clone(),
                            });
                            next_id += 1;
                        }
                    }

                    ResourceCategory::Media => {
                        let media_out_dir = input.output_dir.join("media");
                        fs_err::create_dir_all(&media_out_dir)?;
                        let filename = file.path.file_name().expect("media file has no filename");
                        let out_path = media_out_dir.join(filename);
                        fs_err::copy(&file.path, &out_path)?;
                        resource_ids.insert(file.name.clone(), next_id);
                        all_entries.push(CompiledEntry {
                            id: next_id,
                            entry_type: 0x13, // MEDIA type (matches restool 6.0)
                            name: file.name.clone(),
                            value: format!(
                                "media/{}",
                                filename.to_string_lossy()
                            ),
                            qualifier: file.qualifier.0.clone(),
                        });
                        next_id += 1;
                    }

                    ResourceCategory::Profile => {
                        let profile_out_dir =
                            input.output_dir.join("base").join("profile");
                        fs_err::create_dir_all(&profile_out_dir)?;
                        let filename = file.path.file_name().expect("profile file has no filename");
                        let out_path = profile_out_dir.join(filename);
                        fs_err::copy(&file.path, &out_path)?;
                        profiles.push((file.name.clone(), out_path));
                    }
                }
            }
        }

        // Write binary index.
        let index_path = input.output_dir.join("resources.index");
        index::write_resources_index(&all_entries, &index_path)?;

        // Write human-readable table.
        let table_path = input.output_dir.join("ResourceTable.txt");
        table::write_resource_table(&all_entries, &table_path)?;

        Ok(ResourceCompileOutput {
            resources_index: index_path,
            resource_table: table_path,
            profiles,
            resource_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_compiler() -> ResourceCompiler {
        ResourceCompiler::new(ResourceCompileConfig {
            package_name: "com.example.test".into(),
            module_name: "entry".into(),
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

        assert!(out_dir.join("media/icon.png").exists());

        let table = fs::read_to_string(&result.resource_table).unwrap();
        assert!(table.contains("icon"));
        assert!(table.contains("media"));
    }

    #[test]
    fn test_compile_profile_is_copied() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("resources/base/profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("router_map.json"),
            r#"{"routerMap":[]}"#,
        )
        .unwrap();

        let out_dir = tmp.path().join("out");
        let result = make_compiler()
            .compile(ResourceCompileInput {
                resources_dirs: vec![tmp.path().join("resources")],
                output_dir: out_dir.clone(),
            })
            .unwrap();

        assert!(out_dir.join("base/profile/router_map.json").exists());
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
                resources_dirs: vec![
                    tmp.path().join("res1"),
                    tmp.path().join("res2"),
                ],
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
