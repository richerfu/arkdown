# SPEC-047: Watch Mode (`--watch`)

**Crate**: `arkdown-watcher` + `arkdown-cli`
**Priority**: P2
**Blocked by**: SPEC-013 (CLI)
**Blocks**: SPEC-025 (preview mode resource watcher)

---

## Summary

`arkdown build --watch` keeps the process alive and re-runs the full build
pipeline whenever source `.ets`, `.ts`, `.js`, `.json`, or `.json5` files
change.  A 100 ms debounce window is applied before triggering the rebuild,
matching hvigor's `buildDelay: 100` option from `getWatchConfig()`.

---

## CLI Interface

```
arkdown build --watch [--target hap|hsp|har] [--mode debug|release]
                      [--module <name>] [--product <name>] [--project <path>]
```

- `--watch` is a boolean flag on `BuildArgs` (default: false).
- All other `build` flags are forwarded unchanged to each rebuild pass.
- Watch mode performs an **initial build** on startup, then waits for changes.
- A failed initial build is reported but does **not** exit — the watcher
  continues so the developer can fix the error and save.

---

## Implementation

### `arkdown-watcher` crate (`crates/arkdown-watcher/src/lib.rs`)

**`FileWatcher`** — the core watch engine:

```rust
pub struct FileWatcher {
    config: Arc<ArkdownConfig>,
    watch_dirs: Vec<PathBuf>,
    debounce: Duration,   // always 100ms
}

impl FileWatcher {
    pub fn new(config: Arc<ArkdownConfig>, watch_dirs: Vec<PathBuf>) -> Self;
    pub fn config(&self) -> &ArkdownConfig;

    /// Start watching.  Calls `on_change` with the deduplicated list of
    /// changed paths after each debounced batch.  Loops forever.
    pub fn watch<F>(&self, mut on_change: F) -> Result<(), WatchError>
    where
        F: FnMut(&[PathBuf]) -> Result<(), String>;
}
```

**`watch_dirs_for_module`** — returns the two directories to watch per module:

```rust
pub fn watch_dirs_for_module(module_src_path: &Path) -> Vec<PathBuf> {
    vec![
        module_src_path.join("src/main/ets"),
        module_src_path.join("src/main/resources"),
    ]
}
```

**Debounce algorithm**:
1. Block on the channel until the first filesystem event arrives (up to 5 min
   timeout to avoid busy-waiting).
2. After the first event, collect all additional events that arrive within
   the next 100 ms.
3. Deduplicate paths; filter to source extensions only
   (`ets`, `ts`, `js`, `json`, `json5`).
4. Call `on_change` with the deduplicated list.

**File extension filter** (matches hvigor watch config):
- `.ets`, `.ts`, `.js` — source files
- `.json`, `.json5` — config and resource files

### `arkdown-cli` wiring (`crates/arkdown-cli/src/cmd/build.rs`)

```rust
pub struct BuildArgs {
    // ... existing fields ...
    /// Watch source files and rebuild on change (100ms debounce)
    #[arg(long)]
    pub watch: bool,
}
```

`run(args)` dispatches to `run_watch(...)` when `args.watch` is true.

`run_build_once(project_root, target, mode, module, product)` contains the
full 6-step build pipeline (config → profiles → bundling → importees →
ABC → packaging).  Both normal builds and watch-mode rebuilds call this
function.

`run_watch(...)`:
1. Calls `run_build_once` for the initial build.
2. Loads config to enumerate module `src_path` values; calls
   `watch_dirs_for_module` for each; adds `AppScope/resources` if present.
3. Constructs a `FileWatcher` with the computed directories.
4. Spawns a dedicated OS thread (`std::thread::spawn`) that calls
   `FileWatcher::watch`.  Inside the callback, a new single-threaded tokio
   runtime is built to call `run_build_once` asynchronously.
5. Joins the thread; propagates any watcher error.

---

## hvigor Reference

`hvigor-ohos-plugin` `getWatchConfig()` configures:
- `buildDelay: 100` — debounce window in milliseconds
- Watched extensions: `.ets`, `.ts`, `.js`, `.json`, `.json5`
- Watched paths: module `src/main/ets` and `src/main/resources`

arkdown matches all three of these parameters.

---

## Dependency Changes

`crates/arkdown-cli/Cargo.toml` adds:

```toml
arkdown-watcher = { path = "../arkdown-watcher" }
```

---

## Test Requirements

### `arkdown-watcher` unit tests

- `test_watch_dirs_for_module` — result contains `src/main/ets` and
  `src/main/resources` suffixes.
- `test_watch_dirs_for_module_count` — result has exactly 2 entries.
- `test_watcher_creation` — `FileWatcher::new` does not panic.
- `test_debounce_is_100ms` — `watcher.debounce == Duration::from_millis(100)`.
- `test_watcher_config_accessor` — `watcher.config()` returns the config
  passed to `new`.

### CLI integration

- `arkdown build --watch` flag is accepted by clap without error.
- Normal `arkdown build` (no `--watch`) is unaffected — all existing e2e
  tests continue to pass.

---

## Acceptance Criteria

- `arkdown build --watch` performs an initial build then enters the watch loop.
- Saving a `.ets` file triggers a rebuild within ~200 ms (100 ms debounce +
  build latency).
- A build error in watch mode is printed to stderr but does not exit the
  process.
- `cargo test` passes for both `arkdown-watcher` and `arkdown-cli`.
