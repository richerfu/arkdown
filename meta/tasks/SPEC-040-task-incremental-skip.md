# SPEC-040: Task-Level Incremental Skip

**Crate**: `arkdown-cli`
**Priority**: P1 — dev loop speed
**Blocked by**: SPEC-039 (rolldown cache), SPEC-038 (obfuscation pipeline)
**Blocks**: SPEC-047 (watch mode)

---

## Summary

Before running the full ArkCompile task, compare a set of declared inputs against
a persisted snapshot. If all inputs match and all expected outputs exist, skip the
entire build task. This mirrors hvigor's `ArkCompile::declareInputs()` and
`beforeTask()` cache check logic.

---

## Background: What hvigor Does

From `ark-compile.js::declareInputs()`:

```javascript
declareInputs() {
    const inputs = super.declareInputs()
        .set("arkTsWidget",        this.needSubmitArkTsWidget)
        .set("obfuscationEnable",  String(this.getObfuscationEnable()))
        .set("obfuscationFilesHash", this.obfuscationHash)
        .set("tscMaxFlowDepth",    String(buildOption.arkOptions?.tscConfig?.maxFlowDepth))
        .set("noExternalImportByPath", this.targetService.isNoExternalImportByPath())
        .set("copyCodeResourceEnable", ...)
        .set("ohosUiTransformOptimization", ...)
        .set("localModuleParamMap", JSON.stringify(...));
    return inputs;
}
```

From `beforeTask()`:

```javascript
async beforeTask() {
    await super.beforeTask();
    // Remove stale preload SO file
    if (fse.existsSync(preloadSoPath)) fse.removeSync(preloadSoPath);

    if (!this.targetService.isDebug()) {
        if (this.hasCacheBoosterPlugin()) {
            await this.checkAndCleanCache();
        } else {
            // Compare obfuscation enable/hash against last build
            const cachedEnable = this.getIncrementalCacheItem(OBFUSCATION_ENABLE);
            if (String(cachedEnable) !== String(this.getObfuscationEnable())) {
                await this.cleanCache();
                return;
            }
            const cachedHash = this.getIncrementalCacheItem(OBFUSCATION_FILES_HASH);
            if (cachedHash !== this.obfuscationHash) {
                await this.cleanCache();
            }
        }
    }
}
```

`checkAndCleanCache()` reads `TaskCompileArkTS.cache` (JSON):

```javascript
async checkAndCleanCache() {
    const cacheFile = path.resolve(taskTempDir, "TaskCompileArkTS.cache");
    const currentEnable = String(this.getObfuscationEnable());
    const currentHash = this.obfuscationHash;
    let cachedEnable, cachedHash;
    if (await fse.pathExists(cacheFile)) {
        const { enable, hash } = JSON.parse(await fse.readFile(cacheFile, "utf-8"));
        cachedEnable = enable; cachedHash = hash;
    }
    if (currentEnable !== cachedEnable) await this.cleanCache();
    if (currentHash !== cachedHash) await this.cleanCache();
    await fse.writeFile(cacheFile, JSON.stringify({ enable: currentEnable, hash: currentHash }));
}
```

hvigor's task framework also compares `declareInputFiles()` entries (source file mtimes
and hashes) via its own incremental task engine. arkdown implements a simplified version
using a `task-state.json` file.

---

## Scope

### 1. `TaskState` Struct

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TaskState {
    /// Hash of all compile-entry source files (FNV-1a over sorted contents).
    pub source_hash: u64,
    /// Hash of build configuration (build_mode, api_version, bundle_name, etc.).
    pub config_hash: u64,
    /// Obfuscation enabled flag from last build.
    pub obfuscation_enabled: bool,
    /// SHA-256 of all obfuscation rule files from last build.
    pub obfuscation_hash: String,
    /// compileEnv key from last build.
    pub compile_env_key: String,
    /// Timestamp of last successful build (Unix seconds).
    pub last_build_ts: u64,
}
```

Stored at `<cachePath>/TaskCompileArkTS.cache` (JSON, matches hvigor filename).

### 2. `TaskStateCache` API

```rust
pub struct TaskStateCache {
    path: PathBuf,
    state: Option<TaskState>,
}

impl TaskStateCache {
    pub fn load(cache_dir: &Path) -> Self;

    /// Returns true if all inputs match the persisted state AND all expected
    /// output files exist on disk.
    pub fn is_valid(
        &self,
        current: &TaskState,
        expected_outputs: &[PathBuf],
    ) -> bool;

    /// Persist the current state to disk.
    pub fn save(&self, state: &TaskState) -> std::io::Result<()>;

    /// Delete the cache file (force full rebuild on next run).
    pub fn invalidate(&self) -> std::io::Result<()>;
}
```

`is_valid()` returns `true` when:
1. `state` is `Some` (cache file exists and parses).
2. `current.source_hash == state.source_hash`.
3. `current.config_hash == state.config_hash`.
4. `current.obfuscation_enabled == state.obfuscation_enabled`.
5. `current.obfuscation_hash == state.obfuscation_hash`.
6. `current.compile_env_key == state.compile_env_key`.
7. All paths in `expected_outputs` exist on disk.

### 3. `compute_task_state()` Helper

```rust
pub fn compute_task_state(
    config: &ArkdownConfig,
    module: &ModuleConfig,
    loader_json: &LoaderJson,
    obfuscation_enabled: bool,
    obfuscation_hash: &str,
    compile_env: &CompileEnv,
) -> TaskState {
    // source_hash: FNV-1a over sorted compile-entry file contents
    let source_hash = compute_source_hash(&loader_json.compile_entry);
    // config_hash: FNV-1a of build_mode:bundle_name:api_version:module_name
    let config_hash = compute_config_hash(config, module);
    TaskState {
        source_hash,
        config_hash,
        obfuscation_enabled,
        obfuscation_hash: obfuscation_hash.to_string(),
        compile_env_key: compile_env.key(),
        last_build_ts: unix_now(),
    }
}
```

### 4. CLI Integration

In `crates/arkdown-cli/src/build.rs`:

```rust
// Before running rolldown + ABC:
let current_state = compute_task_state(
    &config, &module, &loader_json,
    obfuscation_enabled, &obfuscation_hash, &compile_env,
);
let expected_outputs = vec![modules_abc_path.clone()];
let task_cache = TaskStateCache::load(&config.cache_dir);

if task_cache.is_valid(&current_state, &expected_outputs) {
    tracing::info!("Task cache hit — skipping ArkCompile for module '{}'", module.name);
    continue; // skip rolldown + ABC
}

// ... run rolldown + obfuscation + ABC ...

// After successful build:
task_cache.save(&current_state)?;
```

### 5. Obfuscation Cache Wipe

Before running ABC (release builds only), check obfuscation state change:

```rust
if config.build_mode == BuildMode::Release {
    if let Some(prev) = task_cache.state() {
        if prev.obfuscation_enabled != obfuscation_enabled
            || prev.obfuscation_hash != obfuscation_hash
        {
            tracing::debug!("Wiping ABC cache due to obfuscation change");
            let _ = std::fs::remove_dir_all(&config.cache_dir.join("abc"));
        }
    }
}
```

---

## Files to Change

```
crates/arkdown-cache/src/
├── lib.rs              — re-export TaskStateCache, TaskState, compute_task_state
└── task_state.rs       — NEW: TaskState, TaskStateCache, compute_task_state()

crates/arkdown-cli/src/
└── build.rs            — call is_valid() before build; save() after success;
                          wipe ABC cache on obfuscation change
```

---

## Test Plan

- First build: `TaskCompileArkTS.cache` is written after a successful compile.
- Second build with no source changes: `is_valid()` returns `true`; build is skipped;
  `modules.abc` is not regenerated.
- Source change: `source_hash` differs; `is_valid()` returns `false`; full rebuild runs.
- Config change (e.g., `api_version` bump): `config_hash` differs; `is_valid()` returns
  `false`; full rebuild runs.
- Obfuscation enable flip: ABC cache dir is wiped; full rebuild runs.
- Obfuscation rule file change: `obfuscation_hash` differs; ABC cache wiped; rebuild runs.
- `modules.abc` deleted manually: `is_valid()` returns `false` (missing expected output).
- `TaskCompileArkTS.cache` deleted manually: cold rebuild occurs.
- Corrupt cache file: `TaskStateCache::load()` returns `state: None`; treated as cold.
- `compute_source_hash()` is deterministic for the same set of files.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `declareInputs()`, `beforeTask()`, `checkAndCleanCache()`, `cleanCache()`,
  `getIncrementalCacheItem()`, `TaskCompileArkTS.TASK_COMPILE_ARK_TS_FILE_NAME`
- `meta/tasks/SPEC-019-cache.md` — existing `BuildCache` and `BundleCache`
- `meta/tasks/SPEC-029-rolldown-cache.md` — `CompileEnv` key definition
- `meta/tasks/SPEC-028-obfuscation-pipeline.md` — `obfuscation_hash` computation
