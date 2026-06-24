//! Typed plan for the profile processing stage.
//!
//! [`ProfileProcessPlan`] is the single typed input to the profile processing
//! stage.  It is constructed by the CLI's `BuildPlanBuilder` from session
//! data and passed to [`ProfileProcessor`].
//!
//! Profile processing consists of three sub-tasks:
//! 1. **LoaderJson**: generate the `loader.json` compile-entry manifest.
//! 2. **PkgContextInfo**: generate `pkgContextInfo.json`.
//! 3. **PackageModuleJson**: generate the processed `module.json`.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ProfileProcessPlan` | `ProcessProfile` + `GenerateLoaderJson` task inputs |
//! | `LoaderJsonPlan` | `GenerateLoaderJson` inputs |
//! | `PkgContextInfoPlan` | `GeneratePkgContextInfo` inputs |
//! | `PackageModuleJsonPlan` | `MergeProfile`/`GeneratePkgModuleJson` inputs |
//! | `ProfileArtifactPaths` | `PathInfo` output paths for profile stage |

use crate::layout::{ProfileIntermediatesLayout, ProfileModuleLayout, ProfileProjectLayout};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ProfileArtifactPaths
// ---------------------------------------------------------------------------

/// Output artifact paths for the profile processing stage.
///
/// All paths are pre-resolved by the session `ArtifactLayout`.
///
/// # Upstream correspondence
///
/// Corresponds to `PathInfo` output directory constants used by
/// `ProcessProfile`, `GenerateLoaderJson`, and `GeneratePkgContextInfo`.
#[derive(Debug, Clone)]
pub struct ProfileArtifactPaths {
    /// Output path for `loader.json`.
    pub loader_json_path: PathBuf,
    /// Output path for `pkgContextInfo.json`.
    pub pkg_context_info_path: PathBuf,
    /// Output path for the processed `module.json`.
    pub module_json_path: PathBuf,
    /// Output directory for intermediates.
    pub intermediates_dir: PathBuf,
}

impl ProfileArtifactPaths {
    /// Resolve paths relative to a base intermediates directory.
    pub fn from_intermediates_dir(intermediates_dir: impl Into<PathBuf>) -> Self {
        let dir: PathBuf = intermediates_dir.into();
        let layout = ProfileIntermediatesLayout::new(&dir);
        Self {
            loader_json_path: layout.loader_json(),
            pkg_context_info_path: layout.pkg_context_info_json(),
            module_json_path: layout.module_json(),
            intermediates_dir: dir,
        }
    }
}

// ---------------------------------------------------------------------------
// LoaderJsonPlan
// ---------------------------------------------------------------------------

/// Inputs for the `loader.json` generation task.
///
/// `loader.json` is the entry manifest consumed by the rolldown bundler.
/// It records `compileEntry`, `otherCompileFiles`, `commonChunk`, and
/// dependency metadata for the `ArkCompile` stage.
///
/// # Upstream correspondence
///
/// Corresponds to `GenerateLoaderJson` task inputs in hvigor.
#[derive(Debug, Clone)]
pub struct LoaderJsonPlan {
    /// Absolute path to the module's `src/main/ets/` or `src/main/js/` root.
    pub source_root: PathBuf,
    /// Absolute path to the module root (`entry/`, `feature/`, etc.).
    pub module_root: PathBuf,
    /// OHOS module name.
    pub module_name: String,
    /// Bundle name (from `AppScope/app.json5`).
    pub bundle_name: String,
    /// Whether `useNormalizedOHMUrl` is active.
    pub use_normalized_ohm_url: bool,
    /// Whether this is a debug build.
    pub is_debug: bool,
    /// The OHOS module type (`"entry"`, `"feature"`, `"shared"`, `"har"`).
    pub module_type: String,
    /// Output path for `loader.json`.
    pub output_path: PathBuf,
    /// Paths to HAR dependency source roots (for `modulePathMap`).
    pub har_source_roots: Vec<(String, PathBuf)>,
    /// Whether this is a fast-incremental (bytecode-har) build.
    pub is_fast_build: bool,
}

impl LoaderJsonPlan {
    /// Validate the plan fields.
    pub fn validate(&self) -> Result<(), String> {
        if self.module_name.is_empty() {
            return Err("LoaderJsonPlan: module_name is required".to_string());
        }
        if self.bundle_name.is_empty() {
            return Err("LoaderJsonPlan: bundle_name is required".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PkgContextInfoPlan
// ---------------------------------------------------------------------------

/// Inputs for the `pkgContextInfo.json` generation task.
///
/// `pkgContextInfo.json` records the package context information for each
/// module, used by the runtime for module identity and OHM URL resolution.
///
/// # Upstream correspondence
///
/// Corresponds to `GeneratePkgContextInfo` task inputs in hvigor.
#[derive(Debug, Clone)]
pub struct PkgContextInfoPlan {
    /// Bundle name.
    pub bundle_name: String,
    /// OHOS module name.
    pub module_name: String,
    /// Version from `oh-package.json5`.
    pub module_version: String,
    /// Package name from `oh-package.json5`.
    pub package_name: Option<String>,
    /// Output path for `pkgContextInfo.json`.
    pub output_path: PathBuf,
    /// Dependency context entries (package → root path).
    pub dependency_contexts: Vec<(String, PathBuf)>,
}

impl PkgContextInfoPlan {
    /// Validate the plan fields.
    pub fn validate(&self) -> Result<(), String> {
        if self.bundle_name.is_empty() {
            return Err("PkgContextInfoPlan: bundle_name is required".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PackageModuleJsonPlan
// ---------------------------------------------------------------------------

/// Inputs for the `module.json` processing task.
///
/// The processed `module.json` is generated by merging the raw `module.json5`
/// with the `app.json5` app section and applying target/product overrides.
///
/// # Upstream correspondence
///
/// Corresponds to `MergeProfile` + `GeneratePkgModuleJson` task inputs in
/// hvigor.
#[derive(Debug, Clone)]
pub struct PackageModuleJsonPlan {
    /// Absolute path to the raw `module.json5`.
    pub module_json5_path: PathBuf,
    /// Absolute path to the processed `app.json5` content (for app section).
    pub app_json5_path: PathBuf,
    /// Bundle name.
    pub bundle_name: String,
    /// OHOS module name.
    pub module_name: String,
    /// Version code (from product config or app.json5).
    pub version_code: u32,
    /// Version name.
    pub version_name: String,
    /// Target device types.
    pub device_types: Vec<String>,
    /// Whether this is a debug build.
    pub is_debug: bool,
    /// Output path for the processed `module.json`.
    pub output_path: PathBuf,
}

// ---------------------------------------------------------------------------
// ProfileProcessPlan
// ---------------------------------------------------------------------------

/// Complete typed plan for the profile processing stage.
///
/// `ProfileProcessPlan` is the single input to [`ProfileProcessor`].
/// It is constructed by the CLI's `BuildPlanBuilder` from session data
/// and encapsulates all inputs for the profile, loader.json, pkgContextInfo,
/// and module.json generation.
///
/// # Upstream correspondence
///
/// Replaces the ad-hoc `ArkdownConfig`-based discovery in `ProfileProcessor`
/// (SPEC-048: domain crates must not read project files directly).
#[derive(Debug, Clone)]
pub struct ProfileProcessPlan {
    /// `loader.json` generation plan.
    pub loader_json: LoaderJsonPlan,
    /// `pkgContextInfo.json` generation plan.
    pub pkg_context_info: PkgContextInfoPlan,
    /// `module.json` processing plan.
    pub package_module_json: PackageModuleJsonPlan,
    /// All output artifact paths.
    pub artifacts: ProfileArtifactPaths,
}

impl ProfileProcessPlan {
    /// Construct a plan from pre-resolved context inputs.
    ///
    /// This is called by the CLI's `BuildPlanBuilder` after session startup
    /// is complete.  All inputs are pre-resolved; no filesystem reads are
    /// performed here.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to assembling `ProcessProfile` and `GenerateLoaderJson`
    /// task inputs from `TaskCreator` in hvigor.
    pub fn from_context_input(
        module_name: impl Into<String>,
        bundle_name: impl Into<String>,
        module_root: PathBuf,
        artifacts: ProfileArtifactPaths,
        is_debug: bool,
        device_types: Vec<String>,
        version_code: u32,
        version_name: impl Into<String>,
    ) -> Self {
        let module_name = module_name.into();
        let bundle_name = bundle_name.into();
        let version_name = version_name.into();
        let module_layout = ProfileModuleLayout::new(&module_root);
        let source_root = module_layout.ets_source_root();
        let module_json5_path = module_layout.module_json5();
        let project_layout =
            ProfileProjectLayout::new(module_root.parent().unwrap_or(module_root.as_path()));
        let app_json5_path = project_layout.app_scope_app_json5();
        Self {
            loader_json: LoaderJsonPlan {
                source_root,
                module_root: module_root.clone(),
                module_name: module_name.clone(),
                bundle_name: bundle_name.clone(),
                use_normalized_ohm_url: true,
                is_debug,
                module_type: "entry".to_string(),
                output_path: artifacts.loader_json_path.clone(),
                har_source_roots: Vec::new(),
                is_fast_build: false,
            },
            pkg_context_info: PkgContextInfoPlan {
                bundle_name: bundle_name.clone(),
                module_name: module_name.clone(),
                module_version: version_name.clone(),
                package_name: None,
                output_path: artifacts.pkg_context_info_path.clone(),
                dependency_contexts: Vec::new(),
            },
            package_module_json: PackageModuleJsonPlan {
                module_json5_path,
                app_json5_path,
                bundle_name,
                module_name,
                version_code,
                version_name,
                device_types,
                is_debug,
                output_path: artifacts.module_json_path.clone(),
            },
            artifacts,
        }
    }

    /// Validate the plan.
    ///
    /// Returns an error if any required fields are missing.
    pub fn validate(&self) -> Result<(), String> {
        self.loader_json.validate()?;
        self.pkg_context_info.validate()?;
        Ok(())
    }

    /// Return the `module.json5` input path.
    pub fn module_json_input(&self) -> &std::path::Path {
        &self.package_module_json.module_json5_path
    }

    /// Return the `loader.json` output path.
    pub fn loader_input(&self) -> &std::path::Path {
        &self.artifacts.loader_json_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plan() -> ProfileProcessPlan {
        let artifacts = ProfileArtifactPaths::from_intermediates_dir("/build/intermediates");
        ProfileProcessPlan::from_context_input(
            "entry",
            "com.example.app",
            PathBuf::from("/project/entry"),
            artifacts,
            true,
            vec!["phone".to_string()],
            1000000,
            "1.0.0",
        )
    }

    #[test]
    fn plan_validates_successfully() {
        let plan = make_plan();
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn plan_module_json_input_path() {
        let plan = make_plan();
        assert!(plan.module_json_input().ends_with("module.json5"));
    }

    #[test]
    fn plan_loader_input_path() {
        let plan = make_plan();
        assert!(plan.loader_input().ends_with("loader.json"));
    }

    #[test]
    fn artifact_paths_from_intermediates() {
        let paths = ProfileArtifactPaths::from_intermediates_dir("/build");
        assert_eq!(paths.loader_json_path, PathBuf::from("/build/loader.json"));
        assert_eq!(paths.module_json_path, PathBuf::from("/build/module.json"));
        assert_eq!(
            paths.pkg_context_info_path,
            PathBuf::from("/build/pkgContextInfo.json")
        );
    }
}
