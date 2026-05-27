# P2 Features: SPEC-023 through SPEC-027

---

## SPEC-023: Watch Mode

**Crate**: `arkdown-watcher`
**Priority**: P2
**Blocked by**: SPEC-013

Watch source files and incrementally rebuild on changes. Uses `notify` crate for filesystem
events.

### Behavior

```
arkdown watch --target hap --module entry
```

1. Complete initial build (SPEC-013 `build` pipeline)
2. Watch all source directories in `ModuleConfig.src_path/src/main/ets/`
3. On change: identify which modules changed, re-run load/transform/bundle for affected
   modules only (using SPEC-019 cache), re-run es2abc, re-package
4. Report incremental build time to stdout

### Architecture

```rust
pub struct Watcher {
    config: Arc<ArkdownConfig>,
    last_build: Option<BundleOutput>,
}

impl Watcher {
    pub async fn watch(&mut self) -> ! {
        self.initial_build().await.unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut watcher = notify::recommended_watcher(move |evt| tx.send(evt))?;
        watcher.watch(&src_path, RecursiveMode::Recursive)?;
        loop {
            let events = collect_events(&rx, Duration::from_millis(50)); // debounce
            if !events.is_empty() {
                self.incremental_build(&events).await;
            }
        }
    }
}
```

### Acceptance Criteria

- Saving a `.ets` file triggers rebuild within 2 seconds
- Only changed modules and their dependents are recompiled

---

## SPEC-024: Hot-Reload Importer Graph Plugin

**Crate**: `arkdown-plugin-importers`
**Priority**: P2 (hot-reload mode only)
**Blocked by**: SPEC-005

Builds a reverse importer graph for use by DevEco Studio's hot-reload feature. When
`patchConfig.mode` is `"hotReload"` or `"coldReload"`, this plugin collects which modules
import which other modules and writes the result to a JSON file that DevEco Studio reads.

```rust
pub struct CollectImportersPlugin {
    mode: PatchMode, // HotReload | ColdReload
    output_path: PathBuf,
}

impl ArkdownPlugin for CollectImportersPlugin {
    async fn build_end(&self, ctx: &BuildContext, _error: Option<&BuildError>) -> PluginResult<()> {
        let importer_map: HashMap<String, Vec<String>> = /* built in module_parsed hook */;
        let json = serde_json::to_string_pretty(&importer_map)?;
        fs_err::write(&self.output_path, json)?;
        Ok(())
    }
}
```

---

## SPEC-025: Preview Mode Resource Watcher

**Crate**: `arkdown-plugin-preview-watch`
**Priority**: P2 (preview mode only)
**Blocked by**: SPEC-023

In DevEco Studio's in-IDE previewer, resource file changes must trigger incremental resource
recompilation without a full rebuild. This plugin watches `resources/` and triggers
`arkdown-resource` (SPEC-011) on file changes.

Only activated when `config.output_target == OutputTarget::Preview`.

---

## SPEC-026: HAR TGZ Clean Plugin

**Crate**: `arkdown-plugin-ets-tgz-clean`
**Priority**: P2 (HAR ohpm publishing only)
**Blocked by**: SPEC-005

When publishing a HAR to the ohpm registry, strip ArkTS-specific syntax from source files
before packaging into the TGZ tarball. The published TGZ should contain type declarations
and cleaned source, not ArkUI decorators.

Uses `ohos-rs/oxc` transformer to strip `@Component`, `@Entry`, `@State` decorators from
the source before packaging.

---

## SPEC-027: ArkTS Widget Compilation

**Crate**: `arkdown-abc` (extension to SPEC-009)
**Priority**: P2
**Blocked by**: SPEC-009

ArkTS widgets (used in the system launcher) require a separate compilation pass from the
main module. From `ArkCompile.doTaskAction()`:

```javascript
if (this.needSubmitArkTsWidget) {
    const widgetConfig = { ...config, widgetCompile: "true",
                           cachePath: taskTempDir + "_widget",
                           aceBuildJson: pathInfo.getWidgetLoaderJsonPath() };
    await submitArkCompileWork(...widgetConfig...);
}
```

Extend `AbcCompiler` with a `compile_widget()` method that runs a second es2abc invocation
for widget source files, using a separate cache directory and the widget's `loader.json`.

### Activation Condition

```rust
let need_widget_compile = module_json.form_json_paths.len() > 0
    && module_json.module.type_ != "har";
```

Widget source files are identified by `buildOption.arkOptions` referencing form extension
source files listed in `formJsonPathArr`.

---

## Rolldown Alignment

### SPEC-023: Watch Mode

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `Watcher` | rolldown `Bundler` in watch mode (`--watch`) | rolldown has built-in watch mode; arkdown implements watch via `notify` crate and wraps the full pipeline (not just bundling) |
| `notify::recommended_watcher` | rolldown uses OS filesystem events via `notify` crate too | Same underlying crate; rolldown watches only JS/TS files; arkdown must also watch `.ets`, `resources/`, and `module.json5` |
| `collect_events` debounce | rolldown 50ms default debounce | Same default debounce window |
| `incremental_build` using SPEC-019 cache | rolldown incremental rebuild via module cache | Both re-run only changed modules; rolldown uses rollup cache format; arkdown uses msgpack cache (SPEC-019) |
| `WatchChangeKind::Create\|Update\|Delete` | rolldown `WatchChangeKind` enum | Identical enum; arkdown maps `notify::EventKind` to this |

### SPEC-024: Hot-Reload Importer Graph Plugin

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `CollectImportersPlugin::build_end` | `Plugin::build_end` | Same hook |
| `importer_map: HashMap<String, Vec<String>>` | rolldown `ModuleInfo::importers` | rolldown tracks importers in `ModuleInfo`; arkdown writes them to a JSON file for DevEco Studio |

### SPEC-025: Preview Mode Resource Watcher

No rolldown equivalent. Preview mode is DevEco Studio-specific.

### SPEC-026: HAR TGZ Clean Plugin

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| Strip ArkTS decorators from source | Babel `@babel/plugin-strip-type-annotations` | Similar: remove runtime-irrelevant syntax before publishing; different: ArkTS decorators are more complex |

### SPEC-027: ArkTS Widget Compilation

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `AbcCompiler::compile_widget()` | rolldown multi-entry build with separate `output.dir` | Both run a separate compilation pass; rolldown uses separate output options; arkdown uses a separate cache dir and loader.json |

---

## Test Requirements

### SPEC-023

- Initial build completes and HAP is produced before watch loop starts
- Saving a `.ets` file triggers a rebuild within 2 seconds
- Saving an untracked file outside `src/main/ets/` does not trigger a rebuild
- Only the changed module and its dependents are recompiled (verified via `tracing::debug` log: unchanged modules say "cache hit")
- `WatchChangeKind::Delete`: removing a source file triggers rebuild; build fails if the file is still imported
- Debounce: saving 10 files in rapid succession triggers exactly one rebuild (not 10)
- `Ctrl+C` exits the watch loop cleanly (no zombie processes or dangling file handles)

### SPEC-024

- `CollectImportersPlugin` writes a JSON file to `output_path` after `build_end`
- The JSON maps module IDs to arrays of importer module IDs
- A module with no importers maps to an empty array
- Only active when `PatchMode::HotReload` or `PatchMode::ColdReload`; inactive in normal build mode
- Plugin name returns `"collect-importers"`

### SPEC-025

- `ResourceWatcherPlugin` only activates when `config.output_target == OutputTarget::Preview`
- A change to `resources/base/element/string.json` triggers `arkdown-resource::compile()` without a full rebuild
- Non-resource file changes (`.ets`) do not trigger resource recompilation

### SPEC-026

- `EtsTgzCleanPlugin` strips `@Component`, `@Entry`, `@State` decorators from `.ets` files
- Resulting cleaned source is valid TypeScript (parseable without ArkTS extension)
- Non-decorator content (regular class methods, imports) is preserved unchanged

### SPEC-027

- `compile_widget()` is called when `need_widget_compile = true` (form JSON paths present in `module.json5`)
- Widget compilation uses a separate cache directory from the main module compilation
- Widget `.abc` output is placed at the widget-specific path, not overwriting the main `modules.abc`
- `compile_widget()` is never called for HAR modules (`module.type_ == "har"`)
