# SPEC-004: Module Loader + Plugin Pipeline

**Crate**: `arkdown-loader`
**Priority**: P0
**Blocked by**: SPEC-001, SPEC-003
**Blocks**: SPEC-005, SPEC-004a

---

## Summary

Implements the parallel module loading pipeline: for each module ID, invoke the plugin chain
(`resolve_id` → `load` → `transform` → `module_parsed`) and return a parsed `Module`.
Uses `ohos-rs/oxc` for parsing `.ets`/`.ts`/`.js` files. Runs module loading in parallel
via Rayon (one task per module, as in rolldown's `ModuleLoader`).

---

## Pipeline per Module

```
specifier + importer
    │
    ▼  resolve_id (plugin chain)
ResolvedId
    │
    ▼  load (plugin chain, fallback: fs read)
ModuleSource { content, source_map }
    │
    ▼  transform (plugin chain)
ModuleSource (transformed)
    │
    ▼  oxc_parser::parse()
OxcAst + diagnostics
    │
    ▼  oxc_semantic::SemanticBuilder
imports + exports extracted
    │
    ▼  module_parsed (plugin chain, async notification)
Module { id, imports, exports, meta, ast, ... }
```

---

## Key Requirements

### ArkTS / ETS Parsing

`.ets` files contain ArkTS syntax that `ohos-rs/oxc` parses natively (it's a fork with ArkTS
support). Pass the correct `oxc_parser::ParserOptions` for ArkTS:

```rust
let options = ParseOptions {
    parse_regular_expression: true,
    allow_v8_intrinsics: false,
    // ArkTS-specific: struct components, decorators, build() blocks
};
```

### SDK Transform (ets-loader)

The ArkTS transform (desugaring `@Component` structs, `@State` decorators, etc.) is
implemented by `arkdown-plugin-ets-transform` — a pure Rust port of the SDK's ets-loader.
See **SPEC-004a** for the full implementation spec.

The plugin is registered automatically when `module_type == ModuleType::Ets`. No subprocess
invocation, no `OHOS_SDK_HOME` dependency.

`transform.rs` in this crate is a thin shim: it delegates entirely to the plugin chain and
contains no SDK subprocess logic. When SPEC-004a is implemented, remove any subprocess
fallback from this file.

### Import/Export Extraction

After parsing, extract import/export edges using oxc's semantic analysis:

```rust
let semantic = SemanticBuilder::new().build(&ast);
for import in &semantic.module_record().import_entries {
    // extract specifier, imported names
}
for export in &semantic.module_record().exported_bindings {
    // extract export names
}
```

### JSON5 Files

When `module_type == ModuleType::Json5`, parse with the `json5` crate and generate a
synthetic ES module:

```javascript
export default { ...parsed_object };
```

This is handled by the `arkdown-plugin-json5` plugin (SPEC-007), not by this crate.

### ByteCode HAR Modules

When `module_type == ModuleType::BytecodeHar`, skip source loading entirely. Create a
synthetic `Module` with no imports/exports — the ByteCode HAR ABC is referenced directly
in the loader.json output (handled by SPEC-010, SPEC-012).

---

## Plugin Chain Execution

```rust
pub struct PluginDriver {
    plugins: Vec<Arc<dyn ArkdownPlugin>>,
}

impl PluginDriver {
    /// Run resolve_id hook chain: first Some(id) wins.
    pub async fn resolve_id(&self, specifier: &str, importer: Option<&str>)
        -> PluginResult<Option<ResolvedId>>;

    /// Run load hook chain: first Some(source) wins, fallback to fs::read.
    pub async fn load(&self, id: &str) -> PluginResult<ModuleSource>;

    /// Run transform hook chain: each plugin may modify source sequentially.
    pub async fn transform(&self, source: ModuleSource) -> PluginResult<ModuleSource>;

    /// Notify all plugins after a module is parsed (no return value).
    pub async fn module_parsed(&self, module: &Module) -> PluginResult<()>;
}
```

### Full BFS Traversal in `load_all()`

`load_all()` performs a **complete breadth-first traversal** of the import graph, not just
entry + first-level imports. The BFS continues until no new modules are discovered:

```rust
pub async fn load_all(
    entries: Vec<String>,
    plugin_driver: &PluginDriver,
) -> Result<Vec<Module>, ArkdownError> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::from(entries.clone());
    let mut modules = Vec::new();

    while let Some(id) = queue.pop_front() {
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        let module = load_module(&id, plugin_driver).await?;

        // Enqueue imports — but skip external modules (BFS leaves)
        for import in &module.imports {
            let specifier = &import.specifier;
            if is_external_specifier(specifier) {
                // @kit.*, @ohos/*, @bundle:, @shared: — skip, do not follow
                continue;
            }
            if !visited.contains(&import.resolved_id) {
                queue.push_back(import.resolved_id.clone());
            }
        }
        modules.push(module);
    }
    Ok(modules)
}

fn is_external_specifier(s: &str) -> bool {
    s.starts_with("@bundle:")
        || s.starts_with("@shared:")
        || s.starts_with("@kit.")
        || s.starts_with("@ohos/")
        || s.starts_with("@system/")
}
```

**Bug fix**: prior versions of `load_all()` only loaded entry modules plus their direct imports
(`_child_imports`), silently dropping transitive dependencies. The BFS loop above is the correct
implementation — all reachable local modules must be loaded.

### Parallel Loading

Use Rayon to load independent modules in parallel. Modules with circular dependencies
are detected during graph construction (SPEC-005), not here.

```rust
// From the entry points, kick off parallel loading
let (tx, rx) = crossbeam_channel::unbounded();
tx.send(entry_module_id);

// Worker loop (Rayon thread pool)
while let Ok(id) = rx.recv() {
    rayon::spawn(|| {
        let module = load_module(&id, &plugin_driver);
        for import in &module.imports {
            if !is_external_specifier(&import.specifier) {
                tx.send(import.resolved.id.clone());
            }
        }
        result_tx.send(module);
    });
}
```

---

## Files to Create

```
crates/arkdown-loader/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── loader.rs         (ModuleLoader, parallel loading loop)
    ├── plugin_driver.rs  (PluginDriver: hook chain execution)
    ├── parse.rs          (oxc_parser invocation + import/export extraction)
    └── transform.rs      (delegates .ets transform to plugin chain; see SPEC-004a)
```

---

## Acceptance Criteria

- Loads all `.ets`, `.ts`, `.js`, `.json5` files from `MyApplication/entry/src/main/ets/`
- Extracts correct import/export edges from each module
- SDK types (`@ohos/*`) resolve to `ModuleType::Declaration` and are skipped (no source load)
- `BytecodeHar` modules produce a synthetic stub `Module` with no imports
- Plugin chain executes in registration order; first `Some` return wins for `resolve_id`/`load`

---

## Design References

- `meta/design/pipeline.md` — Phase 3 (Module Loading), Phase 4 (Transform)
- `meta/design/types.md` — `Module`, `ModuleSource`, `ArkdownPlugin` trait
- `meta/design/plugins.md` — `common-plugin` and `module-meta-info-plugin` context

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `ModuleLoader` | rolldown `ModuleLoader` | Near-identical design: concurrent queue-based loader; rolldown uses Tokio tasks, arkdown uses Rayon |
| `PluginDriver` | rolldown `PluginDriver` | Same concept; runs hook chains in plugin registration order |
| `PluginDriver::resolve_id()` | rolldown `PluginDriver::resolve_id()` | Identical semantics: first `Some(id)` wins; fallback to default resolver |
| `PluginDriver::load()` | rolldown `PluginDriver::load()` with `HookLoadOutput` | Identical; fallback is `fs::read_to_string` |
| `PluginDriver::transform()` | rolldown `PluginDriver::transform()` with `HookTransformOutput` | Sequential: each plugin receives the output of the prior plugin |
| `PluginDriver::module_parsed()` | rolldown `PluginDriver::module_parsed()` | Notification only; no return value |
| Rayon parallel loading | rolldown uses Tokio `JoinSet` for concurrent module loading | Functional equivalent; both process independent modules concurrently |
| Import/export extraction via `oxc_semantic` | rolldown uses its own AST analysis | Same underlying oxc library; rolldown extracts `ModuleRecord`; arkdown reads `import_entries` + `exported_bindings` |
| `ModuleType::BytecodeHar` stub | rolldown `Module::External` | arkdown creates a real `Module` stub with empty imports; rolldown uses an `ExternalModule` |

**Key divergence:** rolldown's `ModuleLoader` uses a `ModuleId` type that is distinct from the string ID. arkdown uses `String` (the OHM URL or file path) as the canonical ID. No numeric module ID is exposed in the loader API; `ModuleIdx` is internal to `ModuleGraph`.

---

## Test Requirements

- Loading a `.ets` file invokes `EtsTransformPlugin::transform` and returns a `Module` with `module_type = ModuleType::Ts` (the transform changes the type)
- Loading a `.ts` file skips ETS transform; returns `module_type = ModuleType::Ts`
- Loading a `.json5` file returns synthetic ES module code (handled by `Json5Plugin`)
- Loading a `ModuleType::BytecodeHar` module produces a stub `Module` with empty `imports` and `exports`
- `PluginDriver::resolve_id`: first plugin returning `Some` wins; subsequent plugins are not called
- `PluginDriver::load`: first plugin returning `Some` wins; fallback `fs::read` is called only when all plugins return `None`
- `PluginDriver::transform`: all plugins run sequentially; second plugin receives output of first
- `module_parsed` notifies all plugins even when earlier plugins returned errors (notification, not pipeline)
- Circular import `A → B → A` detected during graph construction in SPEC-005, not in loader; loader does not panic on first encounter of a circular edge
- Parallel loading: 100 independent modules load concurrently faster than sequentially (property-based or timing test)
- SDK declaration modules (`external: true`, `module_type: Declaration`) are not passed to `load` hook
