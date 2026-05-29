# SPEC-046: Share Config and Startup Config Merging

**Crate**: `arkdown-profile`
**Priority**: P1
**Blocked by**: SPEC-010
**Status**: Implemented

---

## Summary

Implements two hvigor tasks in `arkdown-profile`:

1. **ProcessShareConfig** — scans `oh_modules/` for installed HSP packages, collects their
   `crossAppSharedConfig` entries, and writes a merged `temp-share-config.json`.

2. **ProcessStartupConfig** — scans `oh_modules/` for installed HAR/HSP packages that
   declare `appStartup` in their `module.json5`, reads each package's startup profile JSON,
   and writes a merged `temp-startup-config.json`.

---

## Reference Implementation

Both tasks are ported from:

- `hvigor-ohos-plugin/src/tasks/process-share-config.js`
- `hvigor-ohos-plugin/src/tasks/process-startup-config.js`

---

## Part A: ProcessShareConfig

### Input

- `oh_modules/<pkg>/src/main/module.json5` (or `module.json5` / `module.json` at pkg root)
  for each installed package
- Module's own `crossAppSharedConfig` entries (for HSP modules that export shared components)

### Processing

1. Walk `oh_modules/` under the module root.
2. For each package directory, read `module.json5` (or `module.json`).
3. Only process packages where `module.type == "shared"` (HSP packages).
4. Extract `module.crossAppSharedConfig` array.
5. Fill in `packageName` from the directory name when absent.
6. Merge all entries including the module's own `self_entries`.
7. Warn (non-fatal) on duplicate URIs across packages.

### Output

```json
{
  "crossAppSharedConfig": [
    { "packageName": "my-hsp", "uri": "myapp://shared/Foo", "srcEntry": "./ets/Foo.ets" }
  ]
}
```

Written to:
```
<module_root>/build/default/intermediates/share_config/default/temp-share-config.json
```

### New Types (arkdown-core)

```rust
pub struct CrossAppSharedConfigEntry {
    pub package_name: String,
    pub uri: String,
    pub src_entry: Option<String>,
}
```

Also added to `ModuleJson5Module`:
```rust
pub app_startup: Option<String>,
pub cross_app_shared_config: Vec<CrossAppSharedConfigEntry>,
```

---

## Part B: ProcessStartupConfig

### Input

- Module's own `module.json5` `module.appStartup` reference (e.g. `"$profile:startup_config"`)
- `oh_modules/<pkg>/src/main/module.json5` for each installed package
- Startup profile JSONs at `resources/base/profile/<name>.json`

### Processing

1. If the module itself declares `appStartup`, read its startup profile from
   `src/main/resources/base/profile/<name>.json`.
2. Walk `oh_modules/` and for each package with `module.appStartup` declared,
   find and read its startup profile JSON.
3. Merge `startupTasks` and `appPreloadHintStartupTasks` arrays, annotating each
   task with `moduleName` and `startupConfigPath`.
4. Validate: warn (non-fatal) on duplicate task names within each category.
5. When no tasks are found anywhere, remove any stale output file.

### Output

```json
{
  "startupTasks": [
    {
      "name": "MyTask",
      "srcEntry": "./ets/startup/MyTask.ets",
      "moduleName": "entry",
      "startupConfigPath": "/abs/path/to/startup_config.json"
    }
  ],
  "appPreloadHintStartupTasks": []
}
```

Written to:
```
<module_root>/build/default/intermediates/startup_config/default/temp-startup-config.json
```

When no startup tasks are found, the file is removed (or never created).

### New Types (arkdown-core)

```rust
pub struct StartupTask {
    pub name: String,
    pub src_entry: String,
    pub run_on_thread: Option<String>,
    pub wait_on_main_thread: Option<bool>,
    pub dependencies: Vec<String>,
    pub module_name: Option<String>,
    pub ohmurl: Option<String>,
    pub src_entry_absolute_path: Option<String>,
    pub startup_config_path: Option<String>,
}

pub struct MergedStartupConfig {
    pub startup_tasks: Vec<StartupTask>,
    pub app_preload_hint_startup_tasks: Vec<StartupTask>,
}

pub struct AppStartupProfile {
    pub startup_tasks: Vec<StartupTask>,
    pub app_preload_hint_startup_tasks: Vec<StartupTask>,
    pub config_entry: Option<String>,
}
```

---

## API

```rust
// crates/arkdown-profile/src/process_share_config.rs
pub fn merge_share_config(input: ShareConfigInput<'_>) -> ShareConfigOutput;
pub fn write_share_config(output: &ShareConfigOutput, output_path: &Path) -> Result<(), std::io::Error>;

// crates/arkdown-profile/src/process_startup_config.rs
pub fn merge_startup_config(input: StartupConfigInput<'_>) -> StartupConfigOutput;
pub fn write_startup_config(output: &StartupConfigOutput, output_path: &Path) -> Result<(), std::io::Error>;
```

---

## Files Modified / Created

```
crates/arkdown-core/src/manifest.rs           — added CrossAppSharedConfigEntry, StartupTask,
                                                 MergedStartupConfig, AppStartupProfile;
                                                 extended ModuleJson5Module with app_startup
                                                 and cross_app_shared_config fields
crates/arkdown-profile/src/lib.rs             — added module declarations and pub re-exports
crates/arkdown-profile/src/process_share_config.rs   — new: ProcessShareConfig implementation
crates/arkdown-profile/src/process_startup_config.rs — new: ProcessStartupConfig implementation
crates/arkdown-profile/src/process_profile.rs — updated test fixture for new fields
crates/arkdown-profile/tests/profile_tests.rs — updated test fixtures for new fields
crates/arkdown-cli/src/cmd/build.rs           — updated fallback ModuleJson5Module initializer
```

---

## Acceptance Criteria

- `merge_share_config` returns empty when `oh_modules/` does not exist
- `merge_share_config` collects entries only from HSP packages (`type == "shared"`)
- `merge_share_config` fills `packageName` from directory name when absent
- `write_share_config` produces `{"crossAppSharedConfig":[...]}` JSON
- `merge_startup_config` returns `None` when no `appStartup` is declared anywhere
- `merge_startup_config` merges tasks from both self and dep profiles
- `merge_startup_config` annotates tasks with `moduleName` and `startupConfigPath`
- `write_startup_config` removes stale file when config is `None`
- All existing tests continue to pass (`cargo test`)

---

## Design References

- `meta/design/hvigorw-analysis.md` — ProcessShareConfig, ProcessStartupConfig task descriptions
- `meta/tasks/SPEC-010-profile-processor.md` — profile processor architecture
