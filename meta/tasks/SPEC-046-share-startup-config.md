# SPEC-046: CrossAppSharedConfig + StartupTask Config Merging

**Crate**: `arkdown-profile`, `arkdown-resolver`
**Priority**: P2 — projects using cross-app HSP sharing or startup tasks
**Blocked by**: SPEC-010 (profile processor), SPEC-003 (resolver)
**Blocks**: —

---

## Summary

Implement two related profile processing tasks:

1. **ProcessShareConfig**: Read `crossAppSharedConfig` from HSP dependency
   `module.json5` and write the merged config to the HAP intermediates.

2. **ProcessStartupConfig**: Merge `startupTasks` configs from all HAR/HSP
   dependencies and validate task dependency ordering. Write the merged startup config
   for embedding in the HAP.

Both tasks mirror hvigor's `ProcessShareConfig` (`process-share-config.js`) and
`ProcessStartupConfig` (`process-startup-config.js`).

---

## Background: What hvigor Does

### ProcessShareConfig

From `process-share-config.js`:

```javascript
class ProcessShareConfig extends AbstractTask {
    async doTaskAction() {
        const hspDeps = this.dependencyManager.getHspDependencies();

        const sharedConfigs = [];
        for (const dep of hspDeps) {
            const moduleJson = dep.getModuleJsonObj();
            const crossAppConfig = moduleJson?.module?.crossAppSharedConfig;
            if (crossAppConfig) {
                sharedConfigs.push({
                    bundleName: dep.getBundleName(),
                    moduleName: dep.getModuleName(),
                    ...crossAppConfig,
                });
            }
        }

        if (sharedConfigs.length > 0) {
            const outputPath = path.resolve(this.intermediatesDir, "crossAppSharedConfig.json");
            fse.writeJsonSync(outputPath, { crossAppSharedConfig: sharedConfigs });
        }
    }
}
```

`crossAppSharedConfig` in `module.json5`:

```json5
{
  module: {
    name: "remoteHsp",
    type: "shared",
    crossAppSharedConfig: {
      // Configuration for cross-app HSP sharing
      proxyData: [
        { uri: "datashareproxy://com.example/data", requiredReadPermission: "..." }
      ]
    }
  }
}
```

### ProcessStartupConfig

From `process-startup-config.js`:

```javascript
class ProcessStartupConfig extends AbstractTask {
    async doTaskAction() {
        const allDeps = [
            ...this.dependencyManager.getHarDependencies(),
            ...this.dependencyManager.getHspDependencies(),
        ];

        const startupTasks = [];
        const seen = new Map(); // taskName → dep for duplicate detection

        for (const dep of allDeps) {
            const moduleJson = dep.getModuleJsonObj();
            const tasks = moduleJson?.module?.appStartup?.startupTasks ?? [];
            for (const task of tasks) {
                if (seen.has(task.name)) {
                    throw new Error(`Duplicate startup task: ${task.name}`);
                }
                seen.set(task.name, dep.getPackageName());
                startupTasks.push(task);
            }
        }

        // Validate dependency ordering: task.dependencies must all be defined
        for (const task of startupTasks) {
            for (const dep of task.dependencies ?? []) {
                if (!seen.has(dep)) {
                    throw new Error(
                        `Startup task '${task.name}' depends on unknown task '${dep}'`
                    );
                }
            }
        }

        if (startupTasks.length > 0) {
            const outputPath = path.resolve(this.intermediatesDir, "startupConfig.json");
            fse.writeJsonSync(outputPath, { startupTasks });
        }
    }
}
```

`appStartup.startupTasks` in `module.json5`:

```json5
{
  module: {
    appStartup: {
      startupTasks: [
        {
          name: "MyStartupTask",
          srcEntry: "./ets/startup/MyStartupTask.ets",
          runOnThread: "taskPool",
          dependencies: ["OtherTask"],
          waitOnMainThread: false,
          excludeFromAutoStart: false,
        }
      ]
    }
  }
}
```

---

## Scope

### 1. Data Types

```rust
/// A single cross-app shared config entry from an HSP dependency.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossAppSharedEntry {
    pub bundle_name: String,
    pub module_name: String,
    #[serde(flatten)]
    pub config: serde_json::Value,  // passthrough of crossAppSharedConfig contents
}

/// Merged cross-app shared config output.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossAppSharedConfig {
    pub cross_app_shared_config: Vec<CrossAppSharedEntry>,
}

/// A single startup task definition.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StartupTask {
    pub name: String,
    pub src_entry: String,
    #[serde(default)]
    pub run_on_thread: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub wait_on_main_thread: bool,
    #[serde(default)]
    pub exclude_from_auto_start: bool,
}

/// Merged startup config output.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupConfig {
    pub startup_tasks: Vec<StartupTask>,
}
```

### 2. `process_share_config()` Function

```rust
/// Collect crossAppSharedConfig from HSP dependencies and write to intermediates.
/// Returns Ok(true) if any shared configs were found, Ok(false) otherwise.
pub fn process_share_config(
    hsp_deps: &[HspDependency],
    intermediates_dir: &Path,
) -> Result<bool, ProfileError> {
    let shared_configs: Vec<CrossAppSharedEntry> = hsp_deps
        .iter()
        .filter_map(|dep| {
            let module_json = dep.module_json.as_ref()?;
            let config = module_json.module.cross_app_shared_config.as_ref()?;
            Some(CrossAppSharedEntry {
                bundle_name: dep.bundle_name.clone(),
                module_name: dep.module_name.clone(),
                config: config.clone(),
            })
        })
        .collect();

    if shared_configs.is_empty() {
        return Ok(false);
    }

    let output = CrossAppSharedConfig { cross_app_shared_config: shared_configs };
    let output_path = intermediates_dir.join("crossAppSharedConfig.json");
    let json = serde_json::to_string_pretty(&output)?;
    std::fs::write(&output_path, json)?;
    Ok(true)
}
```

### 3. `process_startup_config()` Function

```rust
/// Merge startup tasks from all HAR/HSP deps, validate dep ordering, write to intermediates.
/// Returns Err if duplicate task names or missing task dependencies are found.
pub fn process_startup_config(
    all_deps: &[DependencyModuleJson],
    intermediates_dir: &Path,
) -> Result<bool, ProfileError> {
    let mut startup_tasks: Vec<StartupTask> = Vec::new();
    let mut seen: HashMap<String, String> = HashMap::new(); // task_name → pkg_name

    for dep in all_deps {
        let tasks = dep.module_json.module.app_startup
            .as_ref()
            .map(|s| s.startup_tasks.as_slice())
            .unwrap_or(&[]);

        for task in tasks {
            if let Some(existing_pkg) = seen.get(&task.name) {
                return Err(ProfileError::DuplicateStartupTask {
                    task_name: task.name.clone(),
                    pkg1: existing_pkg.clone(),
                    pkg2: dep.package_name.clone(),
                });
            }
            seen.insert(task.name.clone(), dep.package_name.clone());
            startup_tasks.push(task.clone());
        }
    }

    // Validate dependencies
    for task in &startup_tasks {
        for dep_name in &task.dependencies {
            if !seen.contains_key(dep_name) {
                return Err(ProfileError::UnknownStartupTaskDep {
                    task_name: task.name.clone(),
                    dep_name: dep_name.clone(),
                });
            }
        }
    }

    if startup_tasks.is_empty() {
        return Ok(false);
    }

    let output = StartupConfig { startup_tasks };
    let output_path = intermediates_dir.join("startupConfig.json");
    let json = serde_json::to_string_pretty(&output)?;
    std::fs::write(&output_path, json)?;
    Ok(true)
}
```

### 4. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    // ... existing variants ...
    #[error("duplicate startup task '{task_name}' defined in both '{pkg1}' and '{pkg2}'")]
    DuplicateStartupTask { task_name: String, pkg1: String, pkg2: String },

    #[error("startup task '{task_name}' depends on unknown task '{dep_name}'")]
    UnknownStartupTaskDep { task_name: String, dep_name: String },
}
```

### 5. CLI Integration

In the build pipeline, after profile processing and dependency resolution:

```rust
// Process share config (HSP deps only)
process_share_config(&hsp_deps, &config.intermediates_dir)?;

// Process startup config (all HAR + HSP deps)
let all_deps: Vec<_> = har_deps.iter().chain(hsp_deps.iter())
    .map(|d| d.as_dep_module_json())
    .collect();
process_startup_config(&all_deps, &config.intermediates_dir)?;
```

---

## Files to Change

```
crates/arkdown-profile/src/
├── share_config.rs      — NEW: CrossAppSharedEntry, CrossAppSharedConfig,
│                          process_share_config()
├── startup_config.rs    — NEW: StartupTask, StartupConfig, process_startup_config()
└── lib.rs               — re-export; add DuplicateStartupTask/UnknownStartupTaskDep
                           to ProfileError

crates/arkdown-cli/src/
└── build.rs             — call process_share_config() and process_startup_config()
                           in the build pipeline
```

---

## Test Plan

**ProcessShareConfig:**
- No HSP deps: `process_share_config()` returns `Ok(false)`; no output file written.
- HSP dep without `crossAppSharedConfig`: skipped; output file not written.
- HSP dep with `crossAppSharedConfig`: `crossAppSharedConfig.json` is written to
  intermediates with correct `bundleName` and `moduleName`.
- Multiple HSP deps with shared configs: all entries appear in the output array.

**ProcessStartupConfig:**
- No deps with startup tasks: `process_startup_config()` returns `Ok(false)`.
- Single dep with one startup task: `startupConfig.json` written with that task.
- Two deps with non-overlapping tasks: both tasks appear in the merged output.
- Duplicate task name across deps: returns `Err(DuplicateStartupTask)`.
- Task depends on unknown task name: returns `Err(UnknownStartupTaskDep)`.
- Task depends on task defined in another dep: valid; no error.
- Output JSON round-trips through `serde_json::from_str::<StartupConfig>`.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/process-share-config.js`
  — `doTaskAction()`, `crossAppSharedConfig` collection
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/process-startup-config.js`
  — `doTaskAction()`, duplicate/missing dependency validation
- `meta/tasks/SPEC-010-profile-processor.md` — `ProfileProcessor`, `ModuleJson5`
- `meta/design/hvigorw-analysis.md` §B4, §B5 — gap analysis entries
