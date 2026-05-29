# SPEC-047: Watch Mode with Incremental Rebuild

**Crate**: `arkdown-watcher`, `arkdown-cli`
**Priority**: P2 — DevEco IDE integration
**Blocked by**: SPEC-039 (rolldown cache), SPEC-040 (task-level incremental skip)
**Blocks**: —

---

## Summary

Implement `arkdown watch` (the `--watch` flag) using the `notify` crate for filesystem
events, with a 100ms debounce, incremental rolldown rebuild on source change, and
delta-only ABC recompilation. This replaces hvigor's chokidar-based watch mode
(`build-steps/watch.js`, `arkts-pack.js::getWatchConfig()`).

The `arkdown-watcher` crate skeleton already exists (SPEC-023 stub). This SPEC provides
the full implementation detail.

---

## Background: What hvigor Does

From `arkts-pack.js::getWatchConfig()`:

```javascript
function getWatchConfig(compileMode) {
    return {
        chokidar: { useFsEvents: false },
        skipWrite: compileMode === ESMODULE,  // skip file write in ESMODULE mode
        buildDelay: 100,                       // 100ms debounce
    };
}
```

From `arkts-pack.js::watch()`:

```javascript
function watch(projectConfig, logger, ...) {
    projectConfig.watchMode = "true";
    return runArkPack(projectConfig, logger, ...);
}
```

From `build-steps/watch.js`:

```javascript
async function startWatch(config, isPreview, ...) {
    const watcher = rollup.watch({
        ...config,
        watch: getWatchConfig(config.compileMode),
    });

    watcher.on("event", async (event) => {
        if (event.code === "BUNDLE_END") {
            // Run post-bundle steps (processImportees, cache write, ABC compile)
            await processImportees(...);
            await rollupCacheService.writeCache(...);
            await triggerBundleEnd(event.result);
            // Signal rebuild complete
            resolveDonePromise();
        }
        if (event.code === "ERROR") {
            // Report error; continue watching
        }
    });

    return watcher;
}
```

Key hvigor watch behaviors:
- `buildDelay: 100` — debounce 100ms between filesystem events and rebuild trigger.
- `skipWrite: true` for ESMODULE — rollup doesn't write JS chunks; ABC runs directly.
- `chokidar.useFsEvents: false` — use polling/inotify, not macOS FSEvents (more portable).
- On `BUNDLE_END`: run the full post-bundle pipeline (importees, cache, ABC).
- On `ERROR`: log the error and continue watching (do not exit).
- `watchMode: "true"` in projectConfig changes several plugin behaviors.

---

## Scope

### 1. `WatchConfig`

```rust
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Directories to watch recursively.
    pub watch_dirs: Vec<PathBuf>,
    /// Debounce window in milliseconds (default: 100).
    pub debounce_ms: u64,
    /// Additional files to watch (e.g., module.json5, oh-package.json5).
    pub extra_files: Vec<PathBuf>,
}
```

Watched directories (mirrors hvigor's source watching):
- `<module>/src/main/ets/` — ArkTS source files
- `<module>/src/main/resources/` — resource files
- `<module>/src/ohosTest/ets/` — test source (if present)

Extra files:
- `<module>/module.json5`
- `<module>/oh-package.json5`
- `<module>/build-profile.json5`

### 2. `Watcher` Struct

```rust
pub struct Watcher {
    config: Arc<ArkdownConfig>,
    watch_config: WatchConfig,
    /// Module graph from last successful build (for delta computation).
    last_bundle: Option<BundleOutput>,
    /// Task state from last successful build.
    last_task_state: Option<TaskState>,
}

impl Watcher {
    pub fn new(config: Arc<ArkdownConfig>) -> Self;

    /// Run the watch loop. Does not return until Ctrl+C.
    pub async fn run(&mut self) -> Result<(), WatchError>;
}
```

### 3. Watch Loop Implementation

```rust
impl Watcher {
    pub async fn run(&mut self) -> Result<(), WatchError> {
        // 1. Initial full build
        tracing::info!("Starting initial build...");
        self.full_build().await?;
        tracing::info!("Watching for changes. Press Ctrl+C to stop.");

        // 2. Set up filesystem watcher
        let (tx, rx) = std::sync::mpsc::channel();
        let mut fs_watcher = notify::recommended_watcher(move |evt| {
            let _ = tx.send(evt);
        })?;
        for dir in &self.watch_config.watch_dirs {
            if dir.exists() {
                fs_watcher.watch(dir, notify::RecursiveMode::Recursive)?;
            }
        }
        for file in &self.watch_config.extra_files {
            if file.exists() {
                fs_watcher.watch(file, notify::RecursiveMode::NonRecursive)?;
            }
        }

        // 3. Event loop with debounce
        loop {
            // Collect events with 100ms debounce window
            let events = collect_debounced_events(&rx, Duration::from_millis(
                self.watch_config.debounce_ms
            ));

            if events.is_empty() { continue; }

            let changed_paths = classify_changed_paths(&events);
            tracing::info!("Detected {} changed file(s)", changed_paths.len());

            match self.incremental_build(&changed_paths).await {
                Ok(_) => tracing::info!("Rebuild complete."),
                Err(e) => tracing::error!("Rebuild failed: {e}. Watching for further changes."),
            }
        }
    }
}
```

### 4. `collect_debounced_events()`

```rust
/// Drain the channel for `debounce` duration, returning all unique changed paths.
fn collect_debounced_events(
    rx: &std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    debounce: Duration,
) -> Vec<PathBuf> {
    let mut paths = HashSet::new();
    // Wait for first event
    if let Ok(Ok(evt)) = rx.recv_timeout(Duration::from_secs(60)) {
        for path in evt.paths { paths.insert(path); }
    } else {
        return vec![];
    }
    // Drain remaining events within the debounce window
    let deadline = std::time::Instant::now() + debounce;
    while let Ok(Ok(evt)) = rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
        for path in evt.paths { paths.insert(path); }
        if std::time::Instant::now() >= deadline { break; }
    }
    paths.into_iter().collect()
}
```

### 5. `incremental_build()`

```rust
impl Watcher {
    async fn incremental_build(&mut self, changed_paths: &[PathBuf]) -> Result<(), WatchError> {
        // Classify changes
        let (source_changed, resource_changed, config_changed) =
            classify_changes(changed_paths);

        if config_changed {
            // module.json5 or oh-package.json5 changed → full rebuild
            return self.full_build().await;
        }

        if source_changed {
            // Re-run rolldown (uses msgpack cache for unchanged modules)
            let bundle = bundle_with_cache(&self.bundle_config, &self.compile_env).await?;
            // Re-run ABC compile (uses per-function SHA-256 cache)
            let abc_options = self.abc_options_for_changed(&bundle, changed_paths);
            compile_to_abc(&abc_options)?;
            // Re-pack
            pack_hap(&self.pack_config)?;
            self.last_bundle = Some(bundle);
        }

        if resource_changed {
            // Re-run resource compilation only
            compile_resources(&self.resource_config)?;
            // Re-pack (resources.index changed)
            pack_hap(&self.pack_config)?;
        }

        Ok(())
    }

    async fn full_build(&mut self) -> Result<(), WatchError> {
        // Runs the complete build pipeline (same as non-watch build)
        // Updates self.last_bundle and self.last_task_state on success
        todo!()
    }
}
```

### 6. CLI Integration

Add `--watch` flag to the CLI:

```rust
// In arkdown-cli/src/main.rs:
#[derive(clap::Args)]
pub struct BuildArgs {
    // ... existing args ...
    /// Watch source files and rebuild on changes.
    #[arg(long)]
    pub watch: bool,
}

// In build command handler:
if args.watch {
    let mut watcher = Watcher::new(Arc::new(config));
    watcher.run().await?;
} else {
    run_single_build(&config).await?;
}
```

### 7. Signal Handling

Handle `SIGINT` (Ctrl+C) gracefully:

```rust
tokio::select! {
    result = watcher.run() => result?,
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("Watch mode stopped.");
    }
}
```

---

## Files to Change

```
crates/arkdown-watcher/src/
├── lib.rs              — Watcher, WatchConfig, WatchError
├── debounce.rs         — collect_debounced_events()
└── classify.rs         — classify_changed_paths(), classify_changes()

crates/arkdown-cli/src/
├── main.rs             — add --watch flag; invoke Watcher::run()
└── build.rs            — extract full_build() as a reusable function for watch mode
```

---

## Test Plan

- Initial build completes before watch loop starts; HAP is produced.
- Saving a `.ets` file triggers a rebuild within 2 seconds.
- Saving a file outside watched directories does not trigger a rebuild.
- Saving 10 files in rapid succession (within 100ms) triggers exactly one rebuild.
- Source change: only the changed module and its dependents are recompiled (other
  modules use msgpack cache).
- Resource change: only `compile_resources()` and `pack_hap()` run; rolldown is skipped.
- `module.json5` change: full rebuild is triggered.
- Rebuild error: error is logged; watch loop continues; next change triggers another
  attempt.
- `Ctrl+C`: watch loop exits cleanly; no zombie processes or dangling file handles.
- `WatchChangeKind::Delete`: removing a source file that is still imported causes a
  build error (logged); watch continues.
- `--watch` flag not set: `Watcher::run()` is never called; single build runs as before.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build-steps/watch.js`
  — `startWatch()`, `BUNDLE_END` handler, error handling
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
  — `getWatchConfig()` (debounce 100ms, `useFsEvents: false`), `watch()`
- `meta/tasks/SPEC-023-027-p2-features.md` — SPEC-023 original watch mode stub
- `meta/tasks/SPEC-039-rolldown-cache.md` — msgpack cache for incremental rolldown
- `meta/tasks/SPEC-040-task-incremental-skip.md` — task-level skip for unchanged builds
- `meta/tasks/SPEC-025-abc-incremental-cache.md` — per-function ABC cache for delta recompile
