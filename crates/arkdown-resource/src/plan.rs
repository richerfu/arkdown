//! Typed compilation plan for the resource compilation stage.
//!
//! [`ResourceCompilePlan`] is the single structured input to the resource
//! compilation stage.  It is constructed by the CLI's `BuildPlanBuilder` from
//! session data and passed to the SDK restool backend runner.
//!
//! No filesystem reads are performed in this module — all paths are
//! pre-resolved by the CLI planner.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ResourceCompilePlan` | `ProcessResource` + `CompileResource` task declared inputs |
//! | `ResourceInputRoot` | One entry in restool `--inputPath` / `RestoolConfigBuilder.addModuleResources` |
//! | `ResourceDependencyInput` | HAR/HSP resource root passed via `CompileResourcesUtil` / `har-target-util` |
//! | `ResourceOutputPaths` | `PathInfo.getIntermediatesRes()` output paths used by `CompileResource` |
//! | `ResourceCompileOptions` | restool flags: `--packageName`, `--ids`, `--defined-ids`, `--forceOverwrite`, thread count |

use std::path::{Path, PathBuf};

use arkdown_path::ArtifactPathSerializer;

// ---------------------------------------------------------------------------
// ResourceInputRoot
// ---------------------------------------------------------------------------

/// A single resource root directory to compile.
///
/// Corresponds to one `--inputPath` entry passed to restool by
/// `RestoolCommandBuilder.build_command` / `restool-command-builder.js`.
///
/// A module typically has one root (`src/main/resources/`), but overlay
/// resources from `AppScope/resources/` or HAR shared resources may add
/// additional roots that are merged in declaration order.
///
/// # Upstream correspondence
///
/// Corresponds to `RestoolConfigBuilder.addModuleResources` and the
/// `inputPath` array in the restool config JSON written by
/// `restool-file-config-builder.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceInputRoot {
    /// Absolute path to the resources root directory
    /// (e.g. `<module>/src/main/resources/`).
    pub path: PathBuf,
    /// Human-readable label for diagnostics (e.g. `"entry"`, `"AppScope"`).
    pub label: String,
}

impl ResourceInputRoot {
    /// Create a new input root from a path and label.
    pub fn new(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
        }
    }

    /// Return the path as a `&Path`.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// ResourceDependencyInput
// ---------------------------------------------------------------------------

/// A HAR or HSP dependency's resource root that must be merged into the
/// compilation.
///
/// In hvigor, `CompileResourcesUtil` and `har-target-util.js` collect the
/// resource roots from each HAR/HSP dependency and pass them to restool so
/// that their IDs are allocated in a non-overlapping cluster.  The dependency
/// order determines the ID cluster assignment.
///
/// # Upstream correspondence
///
/// Corresponds to `CompileResourcesUtil.getHarResourceRoots` /
/// `har-target-util.js::getHarDependencies` inputs to
/// `RestoolConfigBuilder.addHarResources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDependencyInput {
    /// Absolute path to the dependency's resources root directory.
    pub resources_root: PathBuf,
    /// Package name of the dependency (from `oh-package.json5`).
    pub package_name: String,
    /// Version of the dependency package.
    pub package_version: String,
    /// Whether this is an HSP (shared library) dependency.
    ///
    /// HSP dependencies use a different ID cluster range than HAR dependencies.
    pub is_hsp: bool,
}

impl ResourceDependencyInput {
    /// Create a new HAR dependency input.
    pub fn har(
        resources_root: impl Into<PathBuf>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
    ) -> Self {
        Self {
            resources_root: resources_root.into(),
            package_name: package_name.into(),
            package_version: package_version.into(),
            is_hsp: false,
        }
    }

    /// Create a new HSP dependency input.
    pub fn hsp(
        resources_root: impl Into<PathBuf>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
    ) -> Self {
        Self {
            resources_root: resources_root.into(),
            package_name: package_name.into(),
            package_version: package_version.into(),
            is_hsp: true,
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceOutputPaths
// ---------------------------------------------------------------------------

/// Pre-resolved output artifact paths for the resource compilation stage.
///
/// All paths are absolute and pre-computed by the CLI's `BuildPlanBuilder`
/// from the session `ArtifactLayout`.  The resource compiler writes to these
/// paths without performing any path resolution of its own.
///
/// # Upstream correspondence
///
/// Corresponds to `PathInfo.getIntermediatesRes()` and the output directory
/// constants used by `CompileResource` and `ProcessResource` in hvigor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceOutputPaths {
    /// Root intermediates directory for this module's resource outputs.
    ///
    /// Typically `<build>/intermediates/res/<module>/`.
    pub intermediates_dir: PathBuf,
    /// Output path for the binary `resources.index` file.
    ///
    /// Typically `<intermediates_dir>/resources.index`.
    pub resources_index: PathBuf,
    /// Output path for the human-readable `ResourceTable.txt`.
    ///
    /// Typically `<intermediates_dir>/ResourceTable.txt`.
    pub resource_table: PathBuf,
    /// Output directory for compiled resource files (media, profile, rawfile).
    ///
    /// Typically `<intermediates_dir>/resources/`.
    pub resources_dir: PathBuf,
    /// Output path for the `ids_map/id_defined.json` file written by restool
    /// when `--ids` is passed.
    ///
    /// Typically `<intermediates_dir>/ids_map/id_defined.json`.
    pub ids_defined: PathBuf,
}

impl ResourceOutputPaths {
    /// Derive all output paths from a single intermediates root directory.
    ///
    /// This mirrors the path layout produced by
    /// `PathInfo.getIntermediatesRes()` in hvigor.
    pub fn from_intermediates_dir(intermediates_dir: impl Into<PathBuf>) -> Self {
        let dir: PathBuf = intermediates_dir.into();
        Self {
            resources_index: dir.join("resources.index"),
            resource_table: dir.join("ResourceTable.txt"),
            resources_dir: dir.join("resources"),
            ids_defined: dir.join("ids_map").join("id_defined.json"),
            intermediates_dir: dir,
        }
    }

    /// Return the intermediates root directory.
    pub fn intermediates_dir(&self) -> &Path {
        &self.intermediates_dir
    }
}

// ---------------------------------------------------------------------------
// ResourceCompileOptions
// ---------------------------------------------------------------------------

/// Compilation options that control restool behaviour.
///
/// These correspond to the restool command-line flags assembled by
/// `RestoolCommandBuilder` in `restool-command-builder.js` and the
/// `RestoolConfigBuilder` JSON written by `restool-file-config-builder.js`.
///
/// # Upstream correspondence
///
/// | Field | restool flag / config key |
/// |-------|--------------------------|
/// | `package_name` | `--packageName` / `packageName` |
/// | `module_name` | `--moduleName` / `moduleName` |
/// | `module_names` | `--moduleNames` (sorted cluster assignment) |
/// | `device_types` | `--deviceTypes` / `deviceTypes` |
/// | `is_debug` | `--debug` |
/// | `force_overwrite` | `--forceOverwrite` |
/// | `thread_count` | `--thread` |
/// | `ids_path` | `--ids` (output id_defined.json path) |
/// | `defined_ids_path` | `--defined-ids` (input existing id map) |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCompileOptions {
    /// Application bundle name (e.g. `"com.example.myapp"`).
    ///
    /// Corresponds to restool `--packageName`.
    pub package_name: String,
    /// HAP module name (e.g. `"entry"`).
    ///
    /// Corresponds to restool `--moduleName`.
    pub module_name: String,
    /// Full restool `moduleNames` set for ID cluster assignment.
    ///
    /// When present, restool sorts the module names and chooses this module's
    /// resource ID cluster from its sorted position.  Corresponds to
    /// `--moduleNames` in restool.
    pub module_names: Vec<String>,
    /// Target device types (e.g. `["phone", "tablet"]`).
    ///
    /// Corresponds to restool `--deviceTypes`.
    pub device_types: Vec<String>,
    /// Whether this is a debug build.
    ///
    /// Corresponds to restool `--debug`.
    pub is_debug: bool,
    /// Whether to overwrite existing output files without error.
    ///
    /// Corresponds to restool `--forceOverwrite`.
    pub force_overwrite: bool,
    /// Number of threads for parallel resource compilation.
    ///
    /// `None` uses the restool default.  Corresponds to restool `--thread`.
    pub thread_count: Option<u32>,
}

impl ResourceCompileOptions {
    /// Create options with required fields and sensible defaults.
    pub fn new(
        package_name: impl Into<String>,
        module_name: impl Into<String>,
        device_types: Vec<String>,
        is_debug: bool,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            module_name: module_name.into(),
            module_names: Vec::new(),
            device_types,
            is_debug,
            force_overwrite: false,
            thread_count: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceCompilePlan
// ---------------------------------------------------------------------------

/// Complete typed plan for the resource compilation stage.
///
/// `ResourceCompilePlan` is the single input to the SDK restool backend runner.
/// It is constructed by the CLI's `BuildPlanBuilder` after session startup is complete.  All inputs are
/// pre-resolved; no filesystem reads are performed here.
///
/// # Upstream correspondence
///
/// Replaces the ad-hoc `ResourceCompileConfig` + `ResourceCompileInput` pair.
/// Together they correspond to the declared inputs of `ProcessResource` and
/// `CompileResource` tasks in hvigor:
///
/// - `process-resource.js` / `abstract-process-resource.js` — profile
///   writing, router/share/startup/distro filter, startup task dependency
///   validation.
/// - `compile-resource.js` / `abstract-compile-resource.js` — restool
///   invocation, output artifact paths.
///
/// # SPEC-048 constraint
///
/// This type must NOT depend on `arkdown-session`.  All inputs are delivered
/// by the CLI planner; the domain crate executes them without project
/// discovery.
#[derive(Debug, Clone)]
pub struct ResourceCompilePlan {
    /// The module's own resource roots, merged in declaration order.
    ///
    /// Typically one entry (`src/main/resources/`), but may include
    /// `AppScope/resources/` as an overlay root.
    pub input_roots: Vec<ResourceInputRoot>,
    /// HAR/HSP dependency resource roots, in dependency order.
    ///
    /// These are passed to restool so that their ID clusters are allocated
    /// before the module's own IDs.
    pub dependency_inputs: Vec<ResourceDependencyInput>,
    /// Pre-resolved output artifact paths.
    pub output_paths: ResourceOutputPaths,
    /// Compilation options (package name, module name, device types, etc.).
    pub options: ResourceCompileOptions,
}

impl ResourceCompilePlan {
    /// Validate the plan for internal consistency.
    ///
    /// Returns an `Err` string if any required field is empty or obviously
    /// invalid.  This is a fast, allocation-light check — no filesystem I/O.
    ///
    /// # Upstream correspondence
    ///
    /// Mirrors the pre-condition checks that `CompileResource` performs before
    /// spawning restool (e.g. `packageName` / `moduleName` must be non-empty,
    /// at least one input path must be present).
    pub fn validate(&self) -> Result<(), String> {
        if self.options.package_name.is_empty() {
            return Err("ResourceCompilePlan: options.package_name is required".to_string());
        }
        if self.options.module_name.is_empty() {
            return Err("ResourceCompilePlan: options.module_name is required".to_string());
        }
        if self.input_roots.is_empty() {
            return Err("ResourceCompilePlan: at least one input_root is required".to_string());
        }
        if self.options.device_types.is_empty() {
            return Err("ResourceCompilePlan: options.device_types must not be empty".to_string());
        }
        Ok(())
    }

    /// Produce the restool config JSON input structure.
    ///
    /// Returns a serializable value that matches the JSON written by
    /// `restool-file-config-builder.js` when `--config` is passed to restool.
    /// The CLI can serialize this with `serde_json::to_string_pretty` and
    /// write it to a temp file.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `RestoolConfigBuilder.build_config_json` /
    /// `restool-file-config-builder.js::buildConfig`.
    pub fn restool_config_input(&self) -> RestoolConfigInput {
        let input_paths: Vec<String> = self
            .input_roots
            .iter()
            .map(|r| ResourcePlanPathText::new(&r.path).string())
            .collect();

        let har_paths: Vec<RestoolHarEntry> = self
            .dependency_inputs
            .iter()
            .map(|dep| RestoolHarEntry {
                path: ResourcePlanPathText::new(&dep.resources_root).string(),
                package_name: dep.package_name.clone(),
                version: dep.package_version.clone(),
            })
            .collect();

        RestoolConfigInput {
            package_name: self.options.package_name.clone(),
            module_name: self.options.module_name.clone(),
            module_names: self.options.module_names.clone(),
            device_types: self.options.device_types.clone(),
            input_paths,
            output_path: ResourcePlanPathText::new(&self.output_paths.intermediates_dir).string(),
            ids_path: self
                .output_paths
                .ids_defined
                .parent()
                .map(|p| ResourcePlanPathText::new(p).string()),
            har_paths,
            is_debug: self.options.is_debug,
            force_overwrite: self.options.force_overwrite,
        }
    }

    /// Return a reference to the pre-resolved output layout.
    ///
    /// Provides typed access to all output artifact paths without requiring
    /// callers to construct a `ResourceOutputLayout` separately.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `PathInfo.getIntermediatesRes()` output constants
    /// consumed by `CompileResource` and `PackageHap`.
    pub fn output_layout(&self) -> &ResourceOutputPaths {
        &self.output_paths
    }
}

struct ResourcePlanPathText<'a> {
    path: &'a Path,
}

impl<'a> ResourcePlanPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(&self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
    }
}

// ---------------------------------------------------------------------------
// RestoolConfigInput
// ---------------------------------------------------------------------------

/// Serializable restool config JSON structure.
///
/// This is the data model written to a temp file and passed to restool via
/// `--config`.  It is produced by [`ResourceCompilePlan::restool_config_input`]
/// and should not be constructed directly.
///
/// # Upstream correspondence
///
/// Corresponds to the JSON object written by
/// `restool-file-config-builder.js::buildConfig`.
#[derive(Debug, Clone)]
pub struct RestoolConfigInput {
    /// Application bundle name.
    pub package_name: String,
    /// Module name.
    pub module_name: String,
    /// Full module names list for ID cluster assignment.
    pub module_names: Vec<String>,
    /// Target device types.
    pub device_types: Vec<String>,
    /// Absolute paths of input resource roots.
    pub input_paths: Vec<String>,
    /// Absolute path of the output intermediates directory.
    pub output_path: String,
    /// Optional path for the `ids_map/` directory (for `--ids`).
    pub ids_path: Option<String>,
    /// HAR dependency resource entries.
    pub har_paths: Vec<RestoolHarEntry>,
    /// Debug build flag.
    pub is_debug: bool,
    /// Force overwrite flag.
    pub force_overwrite: bool,
}

/// A single HAR/HSP dependency entry in the restool config.
///
/// # Upstream correspondence
///
/// Corresponds to one entry in the `harPaths` array in the restool config
/// JSON written by `restool-file-config-builder.js`.
#[derive(Debug, Clone)]
pub struct RestoolHarEntry {
    /// Absolute path to the dependency's resources root.
    pub path: String,
    /// Package name.
    pub package_name: String,
    /// Package version.
    pub version: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plan() -> ResourceCompilePlan {
        ResourceCompilePlan {
            input_roots: vec![ResourceInputRoot::new(
                "/project/entry/src/main/resources",
                "entry",
            )],
            dependency_inputs: vec![],
            output_paths: ResourceOutputPaths::from_intermediates_dir(
                "/build/intermediates/res/entry",
            ),
            options: ResourceCompileOptions::new(
                "com.example.myapp",
                "entry",
                vec!["phone".to_string()],
                false,
            ),
        }
    }

    // -----------------------------------------------------------------------
    // ResourceDependencyInput
    // -----------------------------------------------------------------------

    #[test]
    fn dependency_input_har_sets_is_hsp_false() {
        let dep = ResourceDependencyInput::har(
            "/oh_modules/lib/src/main/resources",
            "@scope/lib",
            "1.0.0",
        );
        assert!(!dep.is_hsp);
        assert_eq!(dep.package_name, "@scope/lib");
        assert_eq!(dep.package_version, "1.0.0");
    }

    #[test]
    fn dependency_input_hsp_sets_is_hsp_true() {
        let dep = ResourceDependencyInput::hsp(
            "/oh_modules/shared/src/main/resources",
            "@scope/shared",
            "2.0.0",
        );
        assert!(dep.is_hsp);
    }

    // -----------------------------------------------------------------------
    // ResourceOutputPaths
    // -----------------------------------------------------------------------

    #[test]
    fn output_paths_derived_from_intermediates_dir() {
        let paths = ResourceOutputPaths::from_intermediates_dir("/build/intermediates/res/entry");
        assert_eq!(
            paths.resources_index,
            PathBuf::from("/build/intermediates/res/entry/resources.index")
        );
        assert_eq!(
            paths.resource_table,
            PathBuf::from("/build/intermediates/res/entry/ResourceTable.txt")
        );
        assert_eq!(
            paths.resources_dir,
            PathBuf::from("/build/intermediates/res/entry/resources")
        );
        assert_eq!(
            paths.ids_defined,
            PathBuf::from("/build/intermediates/res/entry/ids_map/id_defined.json")
        );
    }

    // -----------------------------------------------------------------------
    // ResourceCompilePlan::validate
    // -----------------------------------------------------------------------

    #[test]
    fn validate_succeeds_on_valid_plan() {
        let plan = make_plan();
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_fails_on_empty_package_name() {
        let mut plan = make_plan();
        plan.options.package_name = String::new();
        let err = plan.validate().unwrap_err();
        assert!(err.contains("package_name"));
    }

    #[test]
    fn validate_fails_on_empty_module_name() {
        let mut plan = make_plan();
        plan.options.module_name = String::new();
        let err = plan.validate().unwrap_err();
        assert!(err.contains("module_name"));
    }

    #[test]
    fn validate_fails_on_no_input_roots() {
        let mut plan = make_plan();
        plan.input_roots.clear();
        let err = plan.validate().unwrap_err();
        assert!(err.contains("input_root"));
    }

    #[test]
    fn validate_fails_on_empty_device_types() {
        let mut plan = make_plan();
        plan.options.device_types.clear();
        let err = plan.validate().unwrap_err();
        assert!(err.contains("device_types"));
    }

    // -----------------------------------------------------------------------
    // ResourceCompilePlan::restool_config_input
    // -----------------------------------------------------------------------

    #[test]
    fn restool_config_input_captures_input_paths() {
        let plan = make_plan();
        let config = plan.restool_config_input();
        assert_eq!(config.input_paths.len(), 1);
        assert!(config.input_paths[0].contains("entry/src/main/resources"));
    }

    #[test]
    fn restool_config_input_output_path_is_intermediates_dir() {
        let plan = make_plan();
        let config = plan.restool_config_input();
        assert!(config.output_path.contains("intermediates/res/entry"));
    }

    #[test]
    fn restool_config_input_har_paths_included() {
        let mut plan = make_plan();
        plan.dependency_inputs.push(ResourceDependencyInput::har(
            "/oh_modules/mylib/resources",
            "@scope/mylib",
            "1.2.3",
        ));
        let config = plan.restool_config_input();
        assert_eq!(config.har_paths.len(), 1);
        assert_eq!(config.har_paths[0].package_name, "@scope/mylib");
        assert_eq!(config.har_paths[0].version, "1.2.3");
    }

    #[test]
    fn restool_config_input_ids_path_points_to_ids_map_dir() {
        let plan = make_plan();
        let config = plan.restool_config_input();
        let ids_path = config.ids_path.expect("ids_path should be set");
        assert!(ids_path.contains("ids_map"));
    }

    #[test]
    fn restool_config_input_debug_flag_propagated() {
        let mut plan = make_plan();
        plan.options.is_debug = true;
        assert!(plan.restool_config_input().is_debug);

        plan.options.is_debug = false;
        assert!(!plan.restool_config_input().is_debug);
    }

    // -----------------------------------------------------------------------
    // Multi-root plan
    // -----------------------------------------------------------------------

    #[test]
    fn plan_with_appscope_overlay_root_validates() {
        let plan = ResourceCompilePlan {
            input_roots: vec![
                ResourceInputRoot::new("/project/AppScope/resources", "AppScope"),
                ResourceInputRoot::new("/project/entry/src/main/resources", "entry"),
            ],
            dependency_inputs: vec![],
            output_paths: ResourceOutputPaths::from_intermediates_dir("/build/res"),
            options: ResourceCompileOptions::new(
                "com.example.app",
                "entry",
                vec!["phone".to_string()],
                false,
            ),
        };
        assert!(plan.validate().is_ok());
        let config = plan.restool_config_input();
        assert_eq!(config.input_paths.len(), 2);
    }

    #[test]
    fn plan_with_module_names_for_cluster_assignment() {
        let mut plan = make_plan();
        plan.options.module_names = vec!["entry".to_string(), "feature".to_string()];
        assert!(plan.validate().is_ok());
        let config = plan.restool_config_input();
        assert_eq!(config.module_names, vec!["entry", "feature"]);
    }
}
