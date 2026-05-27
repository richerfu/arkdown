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
            tx.send(import.resolved.id.clone());
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
