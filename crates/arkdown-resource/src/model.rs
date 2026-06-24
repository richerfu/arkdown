use std::path::PathBuf;

use rustc_hash::FxHashMap;

/// Build-time configuration for the SDK restool invocation.
#[derive(Debug, Clone)]
pub struct ResourceCompileConfig {
    /// Application bundle name, for example `com.example.myapp`.
    pub package_name: String,
    /// HAP/HSP/HAR module name, for example `entry`.
    pub module_name: String,
    /// Full restool `moduleNames` set used for resource ID cluster assignment.
    pub module_names: Vec<String>,
    /// Target device types, for example `phone` or `tablet`.
    pub device_types: Vec<String>,
    /// Whether this is a debug build.
    pub is_debug: bool,
}

/// Input paths for one SDK restool run.
#[derive(Debug, Clone)]
pub struct ResourceCompileInput {
    /// One or more `resources/` roots passed as `--inputPath`.
    pub resources_dirs: Vec<PathBuf>,
    /// Directory where restool writes compiled resources.
    pub output_dir: PathBuf,
}

/// Paths and ID maps produced by a successful SDK restool run.
#[derive(Debug, Clone)]
pub struct ResourceCompileOutput {
    /// Path to the generated `resources.index` binary.
    pub resources_index: PathBuf,
    /// Path to the generated `ResourceTable.txt`.
    pub resource_table: PathBuf,
    /// Path to the generated `ids_map/id_defined.json`.
    pub ids_defined: PathBuf,
    /// Directory packaged under `resources/` in HAP/HSP.
    pub resources_dir: PathBuf,
    /// `(name, path)` pairs for profile files copied by restool.
    pub profiles: Vec<(String, PathBuf)>,
    /// Map of resource name to assigned numeric ID.
    pub resource_ids: FxHashMap<String, u32>,
    /// Map of `type.name` to runtime resource info used by `$r()` transforms.
    pub resource_infos: FxHashMap<String, ResourceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceInfo {
    pub id: u32,
    pub runtime_type: u32,
}
