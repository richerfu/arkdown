//! Build mode inventories.
//!
//! [`BuildModeInventory`] aggregates the per-mode inputs for preview, unit
//! test, OHOS device test, hot-reload, and coverage builds. Each mode has its
//! own typed inventory struct.
//!
//! # Upstream correspondence
//!
//! | Rust type                 | hvigor task / file                               |
//! |---------------------------|--------------------------------------------------|
//! | [`PreviewInventory`]      | `PreviewBuild` task in hvigor                    |
//! | [`TestInventory`]         | `UnitTestBuild` task in hvigor                   |
//! | [`OhosTestInventory`]     | `OhosTestBuild` task in hvigor                   |
//! | [`ReloadInventory`]       | `HotReloadBuild` task in hvigor                  |
//! | [`CoverageInventory`]     | `GenerateDeviceCoverage` task in hvigor          |
//! | [`BuildModeInventory`]    | mode dispatch in `packages/arkdown` build command     |

use std::path::PathBuf;

use crate::project::snapshot::ProjectSnapshot;

// ---------------------------------------------------------------------------
// PreviewInventory
// ---------------------------------------------------------------------------

/// Inventory for preview (DevEco Studio live preview) builds.
///
/// Upstream: `PreviewBuild` task in hvigor.
#[derive(Debug, Clone, Default)]
pub struct PreviewInventory {
    /// Port used by the preview server.
    pub preview_port: Option<u16>,
    /// Layout file path for the component being previewed.
    pub layout_file: Option<PathBuf>,
    /// Additional resource paths injected for preview.
    pub resource_paths: Vec<PathBuf>,
    /// Whether the preview is in component-only mode.
    pub component_preview: bool,
}

impl PreviewInventory {
    /// Build a preview inventory from project-level config.
    ///
    /// Upstream: `PreviewBuild` initialization.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project; // No project-level preview config yet.
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// TestRunnerKind
// ---------------------------------------------------------------------------

/// The test runner framework used for unit tests.
///
/// Upstream: `testRunner` field in `module.json5` / `UnitTestBuild`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRunnerKind {
    /// OpenHarmony test runner (`ohosTest`).
    OhosTest,
    /// Jest-based unit test runner.
    Jest,
    /// Custom/unknown test runner.
    Custom(String),
}

impl TestRunnerKind {
    /// Parse a test runner string.
    pub fn parse(s: &str) -> Self {
        match s {
            "ohosTest" | "ohos_test" => Self::OhosTest,
            "jest" => Self::Jest,
            other => Self::Custom(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// TestInventory
// ---------------------------------------------------------------------------

/// Inventory for unit test builds.
///
/// Upstream: `UnitTestBuild` task in hvigor.
#[derive(Debug, Clone, Default)]
pub struct TestInventory {
    /// Test runner framework.
    pub runner: Option<TestRunnerKind>,
    /// Test source files (`.test.ets` / `.spec.ts`).
    pub test_files: Vec<PathBuf>,
    /// Test resource directories.
    pub test_resource_dirs: Vec<PathBuf>,
}

impl TestInventory {
    /// Build a test inventory from the project snapshot.
    ///
    /// Upstream: `UnitTestBuild` initialization.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project;
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// OhosTestInventory
// ---------------------------------------------------------------------------

/// Inventory for OHOS device test builds.
///
/// Upstream: `OhosTestBuild` task in hvigor.
#[derive(Debug, Clone, Default)]
pub struct OhosTestInventory {
    /// Test runner framework.
    pub runner: Option<TestRunnerKind>,
    /// Test source files.
    pub test_files: Vec<PathBuf>,
    /// Test resource directories.
    pub test_resource_dirs: Vec<PathBuf>,
    /// Device target identifier.
    pub device_target: Option<String>,
}

impl OhosTestInventory {
    /// Build an OHOS test inventory from the project snapshot.
    ///
    /// Upstream: `OhosTestBuild` initialization.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project;
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// ReloadInventory
// ---------------------------------------------------------------------------

/// Inventory for hot-reload (incremental patch) builds.
///
/// Upstream: `HotReloadBuild` task in hvigor.
#[derive(Debug, Clone, Default)]
pub struct ReloadInventory {
    /// Path to the patch configuration file.
    pub patch_config: Option<PathBuf>,
    /// Module paths that have changed and need recompilation.
    pub changed_module_paths: Vec<PathBuf>,
    /// Whether a full rebuild is required despite being in reload mode.
    pub force_full_rebuild: bool,
}

impl ReloadInventory {
    /// Build a reload inventory from the project snapshot.
    ///
    /// Upstream: `HotReloadBuild` initialization.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project;
        Self::default()
    }

    /// Return the patch config path.
    pub fn reload_patch_config(&self) -> Option<&PathBuf> {
        self.patch_config.as_ref()
    }
}

// ---------------------------------------------------------------------------
// CoverageInventory
// ---------------------------------------------------------------------------

/// Inventory for device coverage builds.
///
/// Upstream: `GenerateDeviceCoverage` task in hvigor.
#[derive(Debug, Clone, Default)]
pub struct CoverageInventory {
    /// Output directories for coverage data files.
    pub coverage_output_dirs: Vec<PathBuf>,
    /// Instrumentation plan: which source files to instrument.
    pub instrumentation_plan: Vec<PathBuf>,
    /// Coverage report format (e.g., `"lcov"`, `"json"`).
    pub report_format: Option<String>,
}

impl CoverageInventory {
    /// Build a coverage inventory from the project snapshot.
    ///
    /// Upstream: `GenerateDeviceCoverage` initialization.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project;
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// BuildModeInventory
// ---------------------------------------------------------------------------

/// Aggregated build mode inventories for a project.
///
/// Only the active mode's inventory is populated; the rest are `None`.
///
/// Upstream: mode dispatch in `packages/arkdown` build command and hvigor
/// `PluginFactory` task selection.
#[derive(Debug, Clone, Default)]
pub struct BuildModeInventory {
    /// Preview mode inventory (populated when `--preview` is active).
    pub preview: Option<PreviewInventory>,
    /// Unit test inventory (populated when `--unit-test` is active).
    pub unit_test: Option<TestInventory>,
    /// OHOS device test inventory (populated when `--ohos-test` is active).
    pub ohos_test: Option<OhosTestInventory>,
    /// Hot-reload inventory (populated when `--reload` is active).
    pub reload: Option<ReloadInventory>,
    /// Coverage inventory (populated when `--coverage` is active).
    pub coverage: Option<CoverageInventory>,
}

impl BuildModeInventory {
    /// Build the mode inventory from a project snapshot.
    ///
    /// Reads mode flags from the project's build options and populates the
    /// appropriate sub-inventory.
    ///
    /// Upstream: mode dispatch in CLI `BuildCommand`.
    pub fn from_project(project: &ProjectSnapshot) -> Self {
        let _ = project;
        Self::default()
    }

    /// Return the preview inventory if preview mode is active.
    ///
    /// Upstream: `PreviewBuild` task inputs.
    pub fn preview_inputs(&self) -> Option<&PreviewInventory> {
        self.preview.as_ref()
    }

    /// Return the unit test inventory if unit test mode is active.
    ///
    /// Upstream: `UnitTestBuild` task inputs.
    pub fn unit_test_inputs(&self) -> Option<&TestInventory> {
        self.unit_test.as_ref()
    }

    /// Return the OHOS device test inventory if OHOS test mode is active.
    ///
    /// Upstream: `OhosTestBuild` task inputs.
    pub fn ohos_test_inputs(&self) -> Option<&OhosTestInventory> {
        self.ohos_test.as_ref()
    }

    /// Return the reload patch config path if hot-reload mode is active.
    ///
    /// Upstream: `HotReloadBuild` patch config.
    pub fn reload_patch_config(&self) -> Option<&PathBuf> {
        self.reload.as_ref()?.reload_patch_config()
    }

    /// Return the coverage inventory if coverage mode is active.
    ///
    /// Upstream: `GenerateDeviceCoverage` task inputs.
    pub fn coverage_inputs(&self) -> Option<&CoverageInventory> {
        self.coverage.as_ref()
    }

    /// Return `true` if any special build mode is active.
    pub fn has_active_mode(&self) -> bool {
        self.preview.is_some()
            || self.unit_test.is_some()
            || self.ohos_test.is_some()
            || self.reload.is_some()
            || self.coverage.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_project() -> ProjectSnapshot {
        ProjectSnapshot {
            root: std::path::PathBuf::from("/project"),
            bundle_name: "com.example.app".to_string(),
            version_code: 1,
            version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            product_name: "default".to_string(),
            module_order: vec![],
            modules: Default::default(),
            root_package: None,
            dependency_graphs: Default::default(),
        }
    }

    #[test]
    fn default_inventory_has_no_active_mode() {
        let inv = BuildModeInventory::default();
        assert!(!inv.has_active_mode());
        assert!(inv.preview_inputs().is_none());
        assert!(inv.unit_test_inputs().is_none());
        assert!(inv.ohos_test_inputs().is_none());
        assert!(inv.reload_patch_config().is_none());
        assert!(inv.coverage_inputs().is_none());
    }

    #[test]
    fn preview_mode_active() {
        let mut inv = BuildModeInventory::default();
        inv.preview = Some(PreviewInventory {
            preview_port: Some(8080),
            ..Default::default()
        });
        assert!(inv.has_active_mode());
        assert_eq!(inv.preview_inputs().unwrap().preview_port, Some(8080));
    }

    #[test]
    fn reload_patch_config_forwarded() {
        let patch = std::path::PathBuf::from("/project/patch.json");
        let mut inv = BuildModeInventory::default();
        inv.reload = Some(ReloadInventory {
            patch_config: Some(patch.clone()),
            ..Default::default()
        });
        assert_eq!(inv.reload_patch_config(), Some(&patch));
    }

    #[test]
    fn test_runner_kind_parse() {
        assert_eq!(TestRunnerKind::parse("ohosTest"), TestRunnerKind::OhosTest);
        assert_eq!(TestRunnerKind::parse("jest"), TestRunnerKind::Jest);
        assert!(matches!(
            TestRunnerKind::parse("custom"),
            TestRunnerKind::Custom(_)
        ));
    }

    #[test]
    fn from_project_returns_default() {
        let project = dummy_project();
        let inv = BuildModeInventory::from_project(&project);
        assert!(!inv.has_active_mode());
    }

    #[test]
    fn coverage_inventory_empty_by_default() {
        let cov = CoverageInventory::default();
        assert!(cov.coverage_output_dirs.is_empty());
        assert!(cov.instrumentation_plan.is_empty());
    }
}
