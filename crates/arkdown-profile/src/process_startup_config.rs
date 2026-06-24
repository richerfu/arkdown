//! `ProcessStartupConfig` — merge startup task configs from HAR/HSP dependencies.
//!
//! Mirrors the hvigor `ProcessStartupConfig` task
//! (`hvigor-ohos-plugin/src/tasks/process-startup-config.js`).
//!
//! ## What it does
//!
//! 1. Reads the current module's own `appStartup` profile (if declared in
//!    `module.json5` as `module.appStartup = "$profile:<name>"`).
//! 2. Walks host/project declared dependencies for installed HAR and HSP packages.
//! 3. For each dependency that declares `module.appStartup`, reads its startup
//!    profile JSON from `resources/base/profile/<name>.json`.
//! 4. Merges `startupTasks` and `appPreloadHintStartupTasks` arrays, annotating
//!    each task with `moduleName` and `startupConfigPath`.
//! 5. Validates that no two tasks within the same category share the same `name`
//!    (duplicate task names cause runtime ordering ambiguity).
//! 6. Writes the merged result as a JSON object to the intermediates path supplied
//!    by the caller.  When there are no startup tasks the output file is removed
//!    (matching hvigor's behaviour).
//!
//! ## Output path (hvigor convention)
//!
//! ```text
//! <module_root>/build/default/intermediates/startup_config/default/temp-startup-config.json
//! ```

use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use arkdown_path::{ArtifactPathSerializer, OhmUrl};

use arkdown_core::{AppStartupProfile, ArkdownError, MergedStartupConfig, StartupTask};

use crate::layout::ProfileModuleLayout;
use crate::resource_ref::ProfileResourceRef;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Input for the startup-config merge pass.
pub struct StartupConfigInput<'a> {
    /// Root directory of the project containing the module.
    pub project_root: &'a Path,
    /// Root directory of the consuming module (e.g. `/project/entry`).
    pub module_root: &'a Path,
    /// Name of the consuming module (e.g. `"entry"`).
    pub module_name: &'a str,
    /// Bundle name (e.g. `"com.example.app"`), used for OHM URL generation.
    /// When `None`, `ohmurl` fields are left empty.
    pub bundle_name: Option<&'a str>,
    /// Value of `module.appStartup` from the module's own `module.json5`, e.g.
    /// `"$profile:startup_config"`.  `None` when the module has no startup config.
    pub app_startup: Option<String>,
    /// Whether to merge ByteCode HAR `META-INF/preload.json` system preload tasks.
    /// The caller computes hvigor's target gate:
    /// HAP module, not preview/cold/local/ohosTest, and `buildOption.preloadSystemSo`.
    pub collect_preload_system_so: bool,
}

/// Merged startup config result.
#[derive(Debug, Clone)]
pub struct StartupConfigOutput {
    /// Merged startup config, or `None` when no startup tasks were found anywhere.
    pub config: Option<MergedStartupConfig>,
}

#[derive(Deserialize)]
struct StartupDependencyPackageJson {
    #[serde(default)]
    dependencies: IndexMap<String, String>,
}

/// Processes startup task configs from the module and its HAR/HSP dependencies.
///
/// Mirrors the hvigor `ProcessStartupConfig` task
/// (`hvigor-ohos-plugin/src/tasks/process-startup-config.js`):
/// reads the module's own `appStartup` profile, walks declared dependencies,
/// merges `startupTasks` and `appPreloadHintStartupTasks`, and writes the result.
pub struct StartupConfigProcessor;

impl StartupConfigProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        StartupConfigProcessor
    }

    /// Collect and merge startup task configs from the module and all its
    /// HAR/HSP dependencies.
    ///
    /// Returns `Err` when two tasks in the same category share the same name.
    pub fn merge_startup_config(
        &self,
        input: StartupConfigInput<'_>,
    ) -> Result<StartupConfigOutput, ArkdownError> {
        let mut merged = MergedStartupConfig::default();
        let mut found_any = false;

        // 1. Process the module's own startup config.
        if let Some(app_startup_ref) = &input.app_startup {
            if let Some(profile_name) = StartupConfigInput::parse_profile_name(app_startup_ref) {
                let profile_path =
                    ProfileModuleLayout::new(input.module_root).profile_json(profile_name);
                if let Some(profile) = StartupConfigInput::read_startup_profile(&profile_path) {
                    StartupConfigInput::merge_tasks_from_profile_with_bundle(
                        &profile,
                        input.module_name,
                        input.bundle_name,
                        &profile_path,
                        &mut merged,
                    );
                    found_any = true;
                }
            }
        }

        // 2. Walk declared oh_modules dependencies to find HAR/HSP startup configs.
        let dependency_roots = StartupConfigInput::collect_declared_dependency_roots(
            input.project_root,
            input.module_root,
        );
        StartupConfigInput::collect_from_dependencies(
            &dependency_roots,
            &mut merged,
            &mut found_any,
        );
        if input.collect_preload_system_so {
            StartupConfigInput::collect_system_preload_from_bytecode_hars(
                &dependency_roots,
                &mut merged,
                &mut found_any,
            );
        }

        if !found_any {
            return Ok(StartupConfigOutput { config: None });
        }

        // 3. Validate: no two tasks in the same category may share the same name.
        StartupConfigInput::check_duplicate_task_names(&merged.startup_tasks, "startupTask")?;
        StartupConfigInput::check_duplicate_task_names(
            &merged.app_preload_hint_startup_tasks,
            "appPreloadHintStartupTask",
        )?;

        Ok(StartupConfigOutput {
            config: Some(merged),
        })
    }

    /// Write the merged startup config to `output_path`.
    ///
    /// When `output.config` is `None`, any existing file at `output_path` is
    /// removed — matching hvigor's `removeSync` call.
    pub fn write_startup_config(
        &self,
        output: &StartupConfigOutput,
        output_path: &Path,
    ) -> Result<(), std::io::Error> {
        match &output.config {
            None => {
                // Remove stale file from a previous build if present.
                if output_path.exists() {
                    std::fs::remove_file(output_path)?;
                }
                Ok(())
            }
            Some(config) => {
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let json = serde_json::to_string(config)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                std::fs::write(output_path, json)?;
                Ok(())
            }
        }
    }
}

impl Default for StartupConfigProcessor {
    fn default() -> Self {
        StartupConfigProcessor::new()
    }
}

// ---------------------------------------------------------------------------
// Private helpers — grouped on StartupConfigInput
// ---------------------------------------------------------------------------

impl<'a> StartupConfigInput<'a> {
    /// Parse a `$profile:<name>` reference and return the bare profile name.
    ///
    /// Returns `None` for values that are not `$profile:` references.
    fn parse_profile_name(value: &str) -> Option<&str> {
        ProfileResourceRef::parse(value).map(|profile_ref| profile_ref.name())
    }

    /// Read and parse a startup profile JSON file.
    ///
    /// Returns `None` when the file does not exist or cannot be parsed.
    fn read_startup_profile(path: &Path) -> Option<AppStartupProfile> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<AppStartupProfile>(&content).ok()
    }

    /// Merge tasks from a parsed profile into `merged`, annotating each task with
    /// `module_name`, `startup_config_path`, `ohmurl`, and `src_entry_absolute_path`.
    ///
    /// Matches hvigor's `processModuleStartupConfig()` which populates all four
    /// fields on each startup task for downstream loader.json and runtime use.
    fn merge_tasks_from_profile(
        profile: &AppStartupProfile,
        module_name: &str,
        profile_path: &Path,
        merged: &mut MergedStartupConfig,
    ) {
        Self::merge_tasks_from_profile_with_bundle(profile, module_name, None, profile_path, merged)
    }

    fn merge_tasks_from_profile_with_bundle(
        profile: &AppStartupProfile,
        module_name: &str,
        bundle_name: Option<&str>,
        profile_path: &Path,
        merged: &mut MergedStartupConfig,
    ) {
        let config_path_str = StartupConfigPathText::new(profile_path).string();

        let annotate = |task: &StartupTask| -> StartupTask {
            let mut t = task.clone();
            t.module_name = Some(module_name.to_string());
            t.startup_config_path = Some(config_path_str.clone());

            // src_entry_absolute_path: resolve src_entry relative to the
            // owning module's src/main directory.  Source packages place the
            // profile at <module>/src/main/resources/base/profile/<name>.json;
            // prebuilt packages may place it at <module>/resources/base/profile.
            if t.src_entry_absolute_path.is_none() {
                if let Some(src_main_root) = Self::src_main_root_for_profile(profile_path) {
                    let src = arkdown_path::strip_relative_prefix(&task.src_entry);
                    let abs = src_main_root.join(src);
                    t.src_entry_absolute_path = Some(StartupConfigPathText::new(&abs).string());
                }
            }

            // ohmurl: @bundle:<bundleName>/<moduleName>/<srcEntryWithoutExt>
            // Matches hvigor's processModuleStartupConfig() OHM URL format.
            if t.ohmurl.is_none() {
                let Some(src) = Self::src_entry_stem_path(&task.src_entry) else {
                    return t;
                };
                let src = ArtifactPathSerializer::rel_to_slash(&src);
                let ohm = if let Some(bundle_name) = bundle_name {
                    OhmUrl::bundle(bundle_name, module_name, &src).into_string()
                } else {
                    OhmUrl::bundle("__bundle__", module_name, &src).into_string()
                };
                t.ohmurl = Some(ohm);
            }

            t
        };

        for task in &profile.startup_tasks {
            merged.startup_tasks.push(annotate(task));
        }

        for task in &profile.app_preload_hint_startup_tasks {
            merged.app_preload_hint_startup_tasks.push(annotate(task));
        }
    }

    fn src_main_root_for_profile(profile_path: &Path) -> Option<PathBuf> {
        let profile_dir = profile_path.parent()?;
        let base_dir = profile_dir.parent()?;
        let resources_dir = base_dir.parent()?;
        let owner = resources_dir.parent()?;

        if owner.file_name().and_then(|name| name.to_str()) == Some("main")
            && owner
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                == Some("src")
        {
            return Some(owner.to_path_buf());
        }

        Some(ProfileModuleLayout::new(owner).src_main())
    }

    fn src_entry_stem_path(src_entry: &str) -> Option<PathBuf> {
        let src_entry = arkdown_path::strip_relative_prefix(src_entry);
        let mut path = PathBuf::new();
        for component in Path::new(src_entry).components() {
            if matches!(component, std::path::Component::CurDir) {
                continue;
            }
            path.push(component.as_os_str());
        }
        if path.as_os_str().is_empty() {
            return None;
        }
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts")
        ) {
            path.set_extension("");
        }
        Some(path)
    }

    /// Walk declared dependency roots, find packages with `appStartup` declared in their
    /// `module.json5`, and merge their startup tasks.
    fn collect_from_dependencies(
        dependency_roots: &[PathBuf],
        merged: &mut MergedStartupConfig,
        found_any: &mut bool,
    ) {
        for pkg_dir in dependency_roots {
            let pkg_name = pkg_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if let Some((profile_name, module_name)) =
                Self::read_app_startup_from_pkg(&pkg_dir, &pkg_name)
            {
                // Locate the startup profile JSON.
                let profile_path = Self::find_startup_profile_in_pkg(&pkg_dir, &profile_name);
                if let Some(path) = profile_path {
                    if let Some(profile) = Self::read_startup_profile(&path) {
                        Self::merge_tasks_from_profile(&profile, &module_name, &path, merged);
                        *found_any = true;
                    }
                }
            }
        }
    }

    fn collect_system_preload_from_bytecode_hars(
        dependency_roots: &[PathBuf],
        merged: &mut MergedStartupConfig,
        found_any: &mut bool,
    ) {
        for pkg_dir in dependency_roots {
            if !Self::is_bytecode_har_package(&pkg_dir) {
                continue;
            }
            let preload_json = ProfileModuleLayout::new(pkg_dir).meta_inf_preload_json();
            let Some(mut preload) = Self::read_system_preload_profile(&preload_json) else {
                continue;
            };
            if preload.system_preload_hint_startup_tasks.is_empty() {
                continue;
            }
            merged
                .system_preload_hint_startup_tasks
                .append(&mut preload.system_preload_hint_startup_tasks);
            *found_any = true;
        }
    }

    fn collect_declared_dependency_roots(project_root: &Path, module_root: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut queue = Vec::new();
        let mut seen = FxHashSet::default();
        queue.push(module_root.to_path_buf());
        if project_root != module_root {
            queue.push(project_root.to_path_buf());
        }

        while let Some(pkg_root) = queue.pop() {
            let Some(pkg) = Self::read_dependency_package_json(&pkg_root) else {
                continue;
            };
            for dep_name in pkg.dependencies.keys() {
                let Some(dep_root) =
                    Self::resolve_dependency_root(&pkg_root, project_root, dep_name)
                else {
                    continue;
                };
                if !seen.insert(dep_root.clone()) {
                    continue;
                }
                queue.push(dep_root.clone());
                roots.push(dep_root);
            }
        }

        roots
    }

    fn read_dependency_package_json(pkg_root: &Path) -> Option<StartupDependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<StartupDependencyPackageJson>(&content).ok()
    }

    fn resolve_dependency_root(
        issuer_root: &Path,
        project_root: &Path,
        dep_name: &str,
    ) -> Option<PathBuf> {
        let mut current = Some(issuer_root);
        while let Some(root) = current {
            let candidate = root.join("oh_modules").join(dep_name);
            if candidate.is_dir() {
                return Some(candidate);
            }
            if root == project_root {
                break;
            }
            current = root.parent();
        }
        let candidate = project_root.join("oh_modules").join(dep_name);
        candidate.is_dir().then_some(candidate)
    }

    fn read_system_preload_profile(path: &Path) -> Option<MergedStartupConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        json5::from_str::<MergedStartupConfig>(&content).ok()
    }

    fn is_bytecode_har_package(pkg_dir: &Path) -> bool {
        let layout = ProfileModuleLayout::new(pkg_dir);
        if layout.ets_modules_abc().is_file() || layout.index_abc().is_file() {
            return true;
        }
        let package_json = layout.oh_package_json5();
        let Ok(content) = std::fs::read_to_string(package_json) else {
            return false;
        };
        json5::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/metadata/byteCodeHar")
                    .and_then(|value| value.as_bool())
            })
            .unwrap_or(false)
    }

    /// Read the `appStartup` reference and module name from an installed package's
    /// `module.json5` (or `module.json`).
    ///
    /// Returns `(profile_name, module_name)` when the package declares `appStartup`,
    /// or `None` otherwise.
    fn read_app_startup_from_pkg(pkg_dir: &Path, pkg_name: &str) -> Option<(String, String)> {
        let layout = ProfileModuleLayout::new(pkg_dir);
        let candidates = [
            layout.module_json5(),
            layout.package_module_json5(),
            layout.package_module_json(),
        ];

        for candidate in &candidates {
            if !candidate.exists() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(candidate) else {
                continue;
            };
            let Ok(raw): Result<serde_json::Value, _> = json5::from_str(&content) else {
                continue;
            };

            let app_startup = raw
                .pointer("/module/appStartup")
                .and_then(|v| v.as_str())?
                .to_string();

            let profile_name = Self::parse_profile_name(&app_startup)?.to_string();
            let module_name = raw
                .pointer("/module/name")
                .and_then(|v| v.as_str())
                .unwrap_or(pkg_name)
                .to_string();

            return Some((profile_name, module_name));
        }

        None
    }

    /// Find the startup profile JSON file within an installed package.
    ///
    /// Tries the source layout (`src/main/resources/base/profile/<name>.json`) and
    /// the pre-built layout (`resources/base/profile/<name>.json`).
    fn find_startup_profile_in_pkg(pkg_dir: &Path, profile_name: &str) -> Option<PathBuf> {
        let layout = ProfileModuleLayout::new(pkg_dir);
        let candidates = [
            layout.profile_json(profile_name),
            layout.prebuilt_profile_json(profile_name),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
        None
    }

    /// Returns `Err` when two tasks in the same category share the same name.
    ///
    /// Hvigor calls `printErrorExit` here; arkdown now matches that hard-error behaviour.
    fn check_duplicate_task_names(
        tasks: &[StartupTask],
        category: &str,
    ) -> Result<(), ArkdownError> {
        let mut seen: FxHashSet<&str> = FxHashSet::default();
        for task in tasks {
            if !seen.insert(task.name.as_str()) {
                return Err(ArkdownError::Bundler(format!(
                    "ProcessStartupConfig: duplicate {} name '{}'",
                    category, task.name
                )));
            }
        }
        Ok(())
    }
}

struct StartupConfigPathText<'a> {
    path: &'a Path,
}

impl<'a> StartupConfigPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(&self) -> String {
        ArtifactPathSerializer::path_to_slash_string(self.path)
            .unwrap_or_else(|_| self.path.display().to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_merge_startup_config_no_startup() {
        let tmp = tempfile::tempdir().unwrap();
        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();
        assert!(output.config.is_none());
    }

    #[test]
    fn test_merge_startup_config_self_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp
            .path()
            .join("src/main/resources/base/profile/startup_config.json");
        write_file(
            &profile_path,
            r#"{
                "startupTasks": [
                    { "name": "MyTask", "srcEntry": "./ets/startup/MyTask.ets" }
                ]
            }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: Some("$profile:startup_config".to_string()),
                collect_preload_system_so: false,
            })
            .unwrap();

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 1);
        assert_eq!(config.startup_tasks[0].name, "MyTask");
        assert_eq!(
            config.startup_tasks[0].module_name.as_deref(),
            Some("entry")
        );
    }

    #[test]
    fn test_merge_startup_config_populates_ohmurl_and_absolute_src_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp
            .path()
            .join("src/main/resources/base/profile/startup_config.json");
        write_file(
            &profile_path,
            r#"{
                "startupTasks": [
                    { "name": "MyTask", "srcEntry": "./ets/startup/MyTask.ets" }
                ]
            }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: Some("com.example.app"),
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: Some("$profile:startup_config".to_string()),
                collect_preload_system_so: false,
            })
            .unwrap();

        let task = &output.config.unwrap().startup_tasks[0];
        let expected_abs = tmp
            .path()
            .join("src/main/ets/startup/MyTask.ets")
            .to_string_lossy()
            .into_owned();
        let expected_profile = profile_path.to_string_lossy().into_owned();
        assert_eq!(
            task.ohmurl.as_deref(),
            Some("@bundle:com.example.app/entry/ets/startup/MyTask")
        );
        assert_eq!(
            task.src_entry_absolute_path.as_deref(),
            Some(expected_abs.as_str())
        );
        assert_eq!(
            task.startup_config_path.as_deref(),
            Some(expected_profile.as_str())
        );
    }

    #[test]
    fn test_merge_startup_config_dep_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let dep_dir = oh_modules.join("my-har");
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "my-har": "1.0.0" } }"#,
        );

        // Write dep's module.json5
        write_file(
            &dep_dir.join("src/main/module.json5"),
            r#"{
                "module": {
                    "name": "my-har",
                    "type": "har",
                    "appStartup": "$profile:startup_config"
                }
            }"#,
        );
        // Write dep's startup profile
        write_file(
            &dep_dir.join("src/main/resources/base/profile/startup_config.json"),
            r#"{
                "startupTasks": [
                    { "name": "HarTask", "srcEntry": "./ets/startup/HarTask.ets" }
                ]
            }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 1);
        assert_eq!(config.startup_tasks[0].name, "HarTask");
        assert_eq!(
            config.startup_tasks[0].module_name.as_deref(),
            Some("my-har")
        );
    }

    #[test]
    fn test_merge_startup_config_ignores_undeclared_dependency_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let dep_dir = tmp.path().join("oh_modules/undeclared-har");
        write_file(
            &dep_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "undeclared-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        write_file(
            &dep_dir.join("src/main/resources/base/profile/startup_config.json"),
            r#"{ "startupTasks": [{ "name": "UndeclaredTask", "srcEntry": "./ets/startup/UndeclaredTask.ets" }] }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();

        assert!(output.config.is_none());
    }

    #[test]
    fn test_merge_startup_config_follows_transitive_declared_dependency_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let module_root = tmp.path().join("entry");
        let direct_dir = module_root.join("oh_modules/direct-har");
        let transitive_dir = module_root.join("oh_modules/transitive-har");
        write_file(
            &module_root.join("oh-package.json5"),
            r#"{ dependencies: { "direct-har": "1.0.0" } }"#,
        );
        write_file(
            &direct_dir.join("oh-package.json5"),
            r#"{ dependencies: { "transitive-har": "1.0.0" } }"#,
        );
        write_file(
            &transitive_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "transitive-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        write_file(
            &transitive_dir.join("src/main/resources/base/profile/startup_config.json"),
            r#"{ "startupTasks": [{ "name": "TransitiveTask", "srcEntry": "./ets/startup/TransitiveTask.ets" }] }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: &module_root,
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 1);
        assert_eq!(config.startup_tasks[0].name, "TransitiveTask");
    }

    #[test]
    fn test_merge_startup_config_merges_self_and_dep() {
        let tmp = tempfile::tempdir().unwrap();

        // Self startup profile
        let self_profile = tmp
            .path()
            .join("src/main/resources/base/profile/startup_config.json");
        write_file(
            &self_profile,
            r#"{
                "startupTasks": [
                    { "name": "SelfTask", "srcEntry": "./ets/startup/SelfTask.ets" }
                ]
            }"#,
        );

        // Dep startup profile
        let dep_dir = tmp.path().join("oh_modules/dep-har");
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "dep-har": "1.0.0" } }"#,
        );
        write_file(
            &dep_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "dep-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        write_file(
            &dep_dir.join("src/main/resources/base/profile/startup_config.json"),
            r#"{ "startupTasks": [{ "name": "DepTask", "srcEntry": "./ets/startup/DepTask.ets" }] }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: Some("$profile:startup_config".to_string()),
                collect_preload_system_so: false,
            })
            .unwrap();

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 2);
        let names: Vec<&str> = config
            .startup_tasks
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert!(names.contains(&"SelfTask"));
        assert!(names.contains(&"DepTask"));
    }

    #[test]
    fn test_write_startup_config_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp
            .path()
            .join("intermediates/startup_config/default/temp-startup-config.json");

        let output = StartupConfigOutput {
            config: Some(MergedStartupConfig {
                startup_tasks: vec![StartupTask {
                    name: "Task1".to_string(),
                    src_entry: "./ets/startup/Task1.ets".to_string(),
                    run_on_thread: None,
                    wait_on_main_thread: None,
                    exclude_from_auto_start: None,
                    dependencies: vec![],
                    module_name: Some("entry".to_string()),
                    ohmurl: None,
                    src_entry_absolute_path: None,
                    startup_config_path: None,
                    is_remote_byte_code_har: None,
                }],
                app_preload_hint_startup_tasks: vec![],
                system_preload_hint_startup_tasks: vec![],
            }),
        };
        StartupConfigProcessor::new()
            .write_startup_config(&output, &out_path)
            .unwrap();

        assert!(out_path.exists());
        let content = fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["startupTasks"][0]["name"], "Task1");
    }

    #[test]
    fn test_write_startup_config_removes_file_when_none() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("temp-startup-config.json");

        // Pre-create the file to simulate a stale build artifact.
        fs::write(&out_path, b"stale").unwrap();
        assert!(out_path.exists());

        let output = StartupConfigOutput { config: None };
        StartupConfigProcessor::new()
            .write_startup_config(&output, &out_path)
            .unwrap();

        assert!(
            !out_path.exists(),
            "stale startup config file must be removed when there are no tasks"
        );
    }

    #[test]
    fn test_parse_profile_name() {
        assert_eq!(
            StartupConfigInput::parse_profile_name("$profile:startup_config"),
            Some("startup_config")
        );
        assert_eq!(
            StartupConfigInput::parse_profile_name("$profile:my_config"),
            Some("my_config")
        );
        assert_eq!(
            StartupConfigInput::parse_profile_name("not_a_profile"),
            None
        );
        assert_eq!(StartupConfigInput::parse_profile_name(""), None);
    }

    #[test]
    fn test_merge_startup_config_missing_profile_file() {
        // Dep declares appStartup but the profile file doesn't exist.
        let tmp = tempfile::tempdir().unwrap();
        let dep_dir = tmp.path().join("oh_modules/missing-profile-har");
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "missing-profile-har": "1.0.0" } }"#,
        );
        write_file(
            &dep_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "missing-profile-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        // No profile JSON file written.

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();
        // No tasks found → config is None (no panic).
        assert!(output.config.is_none());
    }

    #[test]
    fn test_merge_startup_config_app_preload_hint_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp
            .path()
            .join("src/main/resources/base/profile/startup_config.json");
        write_file(
            &profile_path,
            r#"{
                "appPreloadHintStartupTasks": [
                    { "name": "PreloadTask", "srcEntry": "./ets/startup/PreloadTask.ets" }
                ]
            }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: Some("$profile:startup_config".to_string()),
                collect_preload_system_so: false,
            })
            .unwrap();

        let config = output.config.unwrap();
        assert!(config.startup_tasks.is_empty());
        assert_eq!(config.app_preload_hint_startup_tasks.len(), 1);
        assert_eq!(config.app_preload_hint_startup_tasks[0].name, "PreloadTask");
    }

    #[test]
    fn test_merge_startup_config_collects_bytecode_har_system_preload_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let dep_dir = tmp.path().join("oh_modules/bytecode-har");
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_file(
            &dep_dir.join("oh-package.json5"),
            r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
        );
        write_file(&dep_dir.join("ets/modules.abc"), "PANDA");
        write_file(
            &dep_dir.join("META-INF/preload.json"),
            r#"{
                systemPreloadHintStartupTasks: [
                    {
                        name: "PreloadLibCrypto",
                        srcEntry: "libcrypto.z.so",
                        ohmurl: "@normalized:Y&bytecode_har&com.example.app&libcrypto.z.so&1.0.0"
                    }
                ]
            }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: true,
            })
            .unwrap();

        let config = output.config.expect("system preload config");
        assert!(config.startup_tasks.is_empty());
        assert!(config.app_preload_hint_startup_tasks.is_empty());
        assert_eq!(config.system_preload_hint_startup_tasks.len(), 1);
        let task = &config.system_preload_hint_startup_tasks[0];
        assert_eq!(task.name, "PreloadLibCrypto");
        assert_eq!(task.src_entry, "libcrypto.z.so");
        assert_eq!(
            task.ohmurl.as_deref(),
            Some("@normalized:Y&bytecode_har&com.example.app&libcrypto.z.so&1.0.0")
        );
    }

    #[test]
    fn test_merge_startup_config_skips_bytecode_har_system_preload_when_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let dep_dir = tmp.path().join("oh_modules/bytecode-har");
        write_file(
            &tmp.path().join("oh-package.json5"),
            r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
        );
        write_file(
            &dep_dir.join("oh-package.json5"),
            r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
        );
        write_file(
            &dep_dir.join("META-INF/preload.json"),
            r#"{ systemPreloadHintStartupTasks: [{ name: "PreloadLibCrypto", srcEntry: "libcrypto.z.so", ohmurl: "@app:com.example.app/entry/crypto.z" }] }"#,
        );

        let output = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();

        assert!(output.config.is_none());
    }

    #[test]
    fn test_merge_startup_config_duplicate_task_names_error() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp
            .path()
            .join("src/main/resources/base/profile/startup_config.json");
        write_file(
            &profile_path,
            r#"{
                "startupTasks": [
                    { "name": "SameTask", "srcEntry": "./ets/startup/A.ets" },
                    { "name": "SameTask", "srcEntry": "./ets/startup/B.ets" }
                ]
            }"#,
        );

        let err = StartupConfigProcessor::new()
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: Some("$profile:startup_config".to_string()),
                collect_preload_system_so: false,
            })
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("duplicate startupTask name 'SameTask'"),
            "duplicate startup task names must be a hard error: {err}"
        );
    }

    #[test]
    fn test_startup_config_processor_new_and_default() {
        let _p1 = StartupConfigProcessor::new();
        let _p2 = StartupConfigProcessor::default();
    }

    #[test]
    fn test_startup_config_processor_merge_no_startup() {
        let tmp = tempfile::tempdir().unwrap();
        let processor = StartupConfigProcessor::new();
        let output = processor
            .merge_startup_config(StartupConfigInput {
                bundle_name: None,
                project_root: tmp.path(),
                module_root: tmp.path(),
                module_name: "entry",
                app_startup: None,
                collect_preload_system_so: false,
            })
            .unwrap();
        assert!(output.config.is_none());
    }

    #[test]
    fn test_startup_config_processor_write_removes_stale_file() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("startup.json");
        fs::write(&out_path, b"stale").unwrap();
        let processor = StartupConfigProcessor::new();
        processor
            .write_startup_config(&StartupConfigOutput { config: None }, &out_path)
            .unwrap();
        assert!(!out_path.exists());
    }
}
