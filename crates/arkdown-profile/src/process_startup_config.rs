//! `ProcessStartupConfig` — merge startup task configs from HAR/HSP dependencies.
//!
//! Mirrors the hvigor `ProcessStartupConfig` task
//! (`hvigor-ohos-plugin/src/tasks/process-startup-config.js`).
//!
//! ## What it does
//!
//! 1. Reads the current module's own `appStartup` profile (if declared in
//!    `module.json5` as `module.appStartup = "$profile:<name>"`).
//! 2. Scans `oh_modules/` under the module root for installed HAR and HSP packages.
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

use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

use arkdown_core::{AppStartupProfile, MergedStartupConfig, StartupTask};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Input for the startup-config merge pass.
pub struct StartupConfigInput<'a> {
    /// Root directory of the consuming module (e.g. `/project/entry`).
    pub module_root: &'a Path,
    /// Name of the consuming module (e.g. `"entry"`).
    pub module_name: &'a str,
    /// Value of `module.appStartup` from the module's own `module.json5`, e.g.
    /// `"$profile:startup_config"`.  `None` when the module has no startup config.
    pub app_startup: Option<String>,
}

/// Merged startup config result.
#[derive(Debug, Clone)]
pub struct StartupConfigOutput {
    /// Merged startup config, or `None` when no startup tasks were found anywhere.
    pub config: Option<MergedStartupConfig>,
}

/// Collect and merge startup task configs from the module and all its
/// HAR/HSP dependencies, returning the merged result.
///
/// Missing or malformed config files in individual dependencies are silently
/// skipped (matching hvigor's behaviour where packages without `appStartup`
/// simply contribute nothing).
pub fn merge_startup_config(input: StartupConfigInput<'_>) -> StartupConfigOutput {
    let mut merged = MergedStartupConfig::default();
    let mut found_any = false;

    // 1. Process the module's own startup config.
    if let Some(app_startup_ref) = &input.app_startup {
        if let Some(profile_name) = StartupConfigInput::parse_profile_name(app_startup_ref) {
            let profile_path = input
                .module_root
                .join("src/main/resources/base/profile")
                .join(format!("{}.json", profile_name));
            if let Some(profile) = StartupConfigInput::read_startup_profile(&profile_path) {
                StartupConfigInput::merge_tasks_from_profile(
                    &profile,
                    input.module_name,
                    &profile_path,
                    &mut merged,
                );
                found_any = true;
            }
        }
    }

    // 2. Walk oh_modules/ to find HAR/HSP dependencies with startup configs.
    let oh_modules_dir = input.module_root.join("oh_modules");
    if oh_modules_dir.is_dir() {
        StartupConfigInput::collect_from_oh_modules(&oh_modules_dir, &mut merged, &mut found_any);
    }

    if !found_any {
        return StartupConfigOutput { config: None };
    }

    // 3. Validate: no two tasks in the same category may share the same name.
    StartupConfigInput::check_duplicate_task_names(&merged.startup_tasks, "startupTask");
    StartupConfigInput::check_duplicate_task_names(
        &merged.app_preload_hint_startup_tasks,
        "appPreloadHintStartupTask",
    );

    StartupConfigOutput {
        config: Some(merged),
    }
}

/// Write the merged startup config to `output_path`.
///
/// When `output.config` is `None` (no startup tasks were found), any existing
/// file at `output_path` is removed — matching hvigor's `removeSync` call.
///
/// Creates parent directories if they do not exist.
pub fn write_startup_config(
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
            let json =
                serde_json::to_string(config).map_err(|e| std::io::Error::other(e.to_string()))?;
            std::fs::write(output_path, json)?;
            Ok(())
        }
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
        value.strip_prefix("$profile:")
    }

    /// Read and parse a startup profile JSON file.
    ///
    /// Returns `None` when the file does not exist or cannot be parsed.
    fn read_startup_profile(path: &Path) -> Option<AppStartupProfile> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<AppStartupProfile>(&content).ok()
    }

    /// Merge tasks from a parsed profile into `merged`, annotating each task with
    /// `module_name` and `startup_config_path`.
    fn merge_tasks_from_profile(
        profile: &AppStartupProfile,
        module_name: &str,
        profile_path: &Path,
        merged: &mut MergedStartupConfig,
    ) {
        let config_path_str = profile_path.to_string_lossy().into_owned();

        for task in &profile.startup_tasks {
            let mut t = task.clone();
            t.module_name = Some(module_name.to_string());
            t.startup_config_path = Some(config_path_str.clone());
            merged.startup_tasks.push(t);
        }

        for task in &profile.app_preload_hint_startup_tasks {
            let mut t = task.clone();
            t.module_name = Some(module_name.to_string());
            t.startup_config_path = Some(config_path_str.clone());
            merged.app_preload_hint_startup_tasks.push(t);
        }
    }

    /// Walk `oh_modules_dir`, find packages with `appStartup` declared in their
    /// `module.json5`, and merge their startup tasks.
    fn collect_from_oh_modules(
        oh_modules_dir: &Path,
        merged: &mut MergedStartupConfig,
        found_any: &mut bool,
    ) {
        let Ok(read_dir) = std::fs::read_dir(oh_modules_dir) else {
            return;
        };

        for entry in read_dir.flatten() {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }

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

    /// Read the `appStartup` reference and module name from an installed package's
    /// `module.json5` (or `module.json`).
    ///
    /// Returns `(profile_name, module_name)` when the package declares `appStartup`,
    /// or `None` otherwise.
    fn read_app_startup_from_pkg(pkg_dir: &Path, pkg_name: &str) -> Option<(String, String)> {
        let candidates: &[PathBuf] = &[
            pkg_dir.join("src/main/module.json5"),
            pkg_dir.join("module.json5"),
            pkg_dir.join("module.json"),
        ];

        for candidate in candidates {
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
        let candidates = [
            pkg_dir
                .join("src/main/resources/base/profile")
                .join(format!("{}.json", profile_name)),
            pkg_dir
                .join("resources/base/profile")
                .join(format!("{}.json", profile_name)),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
        None
    }

    /// Emit a warning when two tasks in the same category share the same name.
    ///
    /// Hvigor calls `printErrorExit` here; arkdown downgrades to a non-fatal
    /// warning so the build can continue.
    fn check_duplicate_task_names(tasks: &[StartupTask], category: &str) {
        let mut seen: FxHashSet<&str> = FxHashSet::default();
        for task in tasks {
            if !seen.insert(task.name.as_str()) {
                tracing::warn!(
                    category,
                    name = task.name,
                    module_name = ?task.module_name,
                    "startup config duplicate task name"
                );
            }
        }
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
        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: None,
        });
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

        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: Some("$profile:startup_config".to_string()),
        });

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 1);
        assert_eq!(config.startup_tasks[0].name, "MyTask");
        assert_eq!(
            config.startup_tasks[0].module_name.as_deref(),
            Some("entry")
        );
    }

    #[test]
    fn test_merge_startup_config_dep_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let dep_dir = oh_modules.join("my-har");

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

        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: None,
        });

        let config = output.config.unwrap();
        assert_eq!(config.startup_tasks.len(), 1);
        assert_eq!(config.startup_tasks[0].name, "HarTask");
        assert_eq!(
            config.startup_tasks[0].module_name.as_deref(),
            Some("my-har")
        );
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
            &dep_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "dep-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        write_file(
            &dep_dir.join("src/main/resources/base/profile/startup_config.json"),
            r#"{ "startupTasks": [{ "name": "DepTask", "srcEntry": "./ets/startup/DepTask.ets" }] }"#,
        );

        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: Some("$profile:startup_config".to_string()),
        });

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
                    dependencies: vec![],
                    module_name: Some("entry".to_string()),
                    ohmurl: None,
                    src_entry_absolute_path: None,
                    startup_config_path: None,
                }],
                app_preload_hint_startup_tasks: vec![],
            }),
        };
        write_startup_config(&output, &out_path).unwrap();

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
        write_startup_config(&output, &out_path).unwrap();

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
            &dep_dir.join("src/main/module.json5"),
            r#"{ "module": { "name": "missing-profile-har", "type": "har", "appStartup": "$profile:startup_config" } }"#,
        );
        // No profile JSON file written.

        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: None,
        });
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

        let output = merge_startup_config(StartupConfigInput {
            module_root: tmp.path(),
            module_name: "entry",
            app_startup: Some("$profile:startup_config".to_string()),
        });

        let config = output.config.unwrap();
        assert!(config.startup_tasks.is_empty());
        assert_eq!(config.app_preload_hint_startup_tasks.len(), 1);
        assert_eq!(config.app_preload_hint_startup_tasks[0].name, "PreloadTask");
    }
}
