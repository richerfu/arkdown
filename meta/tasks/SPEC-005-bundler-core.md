# SPEC-005: Bundler Core (rolldown integration)

**Crate**: `arkdown`
**Priority**: P0
**Blocked by**: SPEC-001, SPEC-003, SPEC-004
**Blocks**: SPEC-006, SPEC-007, SPEC-008, SPEC-009, SPEC-014, SPEC-016, SPEC-017, SPEC-018

---

## Summary

The core bundler: takes a loaded `ModuleGraph`, links cross-module references, applies tree
shaking, splits into chunks, renders chunk code (via `render_chunk` plugin hooks), and writes
output JS files to `aceModuleBuild/`. This is the rolldown-equivalent core. Reuse rolldown's
design patterns directly — the stages map 1:1.

---

## Stages

### Stage 1: Link

Resolves cross-module symbol references. For each `ImportEdge`, find the exporting module's
`ExportBinding` and create a link. Detect circular dependencies (warn, do not fail unless
they cause unresolvable bindings).

### Stage 2: Tree Shake (Release only)

From `arkts-pack.js`:

```javascript
treeshake: buildMode === RELEASE;
```

In release builds, mark included statements using side-effect analysis. Modules with
`has_side_effects: true` (default for `.ets` files) are always included. Modules in
`oh_modules/` without explicit `sideEffects: false` in `oh-package.json5` are included.

### Stage 3: Chunk Split

Entry points come from `loader.json` `compileEntry` array (built by SPEC-010):

- `module.json5#abilities[*].srcEntry`
- `module.json5#module.srcEntry`
- Worker files
- Router map `pageSourceFile` entries

`supportChunks` flag (from `ArkdownConfig`) controls manual chunk splitting:

```rust
if config.support_chunks {
    // nodes reachable only from oh_modules → "commons" chunk
    // non-entry non-ohmodule nodes → "vendors" chunk
    // entry nodes → entry-named chunks
} else {
    // one chunk per entry (default)
}
```

### Stage 4: Code Generation

For each chunk, use `oxc_codegen` to generate JavaScript source. Output format is **CJS**
(not ESM) — this is required because the ArkRuntime does not support native ESM. The CJS
output uses `require()` for cross-chunk references, which are then replaced by
`wrapper-chunk-plugin` (SPEC-006).

```rust
let codegen = CodeGenerator::new()
    .with_options(CodegenOptions { source_map: true, ..Default::default() });
let output = codegen.build(&ast);
// output.code: String, output.map: Option<SourceMap>
```

### Stage 5: render_chunk Plugin Hook

For each chunk, run the `render_chunk` plugin hook chain. Plugins that transform chunk code
(e.g., `arkdown-plugin-chunk-wrapper`) run here.

```rust
for chunk in &mut chunk_graph.chunks {
    let mut code = chunk.code.clone();
    for plugin in &plugins {
        if let Some(rendered) = plugin.render_chunk(ctx, code.clone(), chunk).await? {
            code = rendered.code;
        }
    }
    chunk.code = code;
}
```

### Stage 6: Write Output

Write each chunk's code to `aceModuleBuild/<chunkName>.js`. These files are the input to
`es2abc` (SPEC-009).

Output directory: `build/default/intermediates/ets/<target>/loader_out/<target>/`

---

## Bundler API

```rust
pub struct Bundler {
    config: Arc<ArkdownConfig>,
    plugins: Vec<Arc<dyn ArkdownPlugin>>,
}

impl Bundler {
    pub fn new(config: ArkdownConfig, plugins: Vec<Arc<dyn ArkdownPlugin>>) -> Self;

    /// Run the full bundle pipeline for one OHOS module.
    /// Returns the chunk graph and output file paths.
    pub async fn bundle(&self, module_config: &ModuleConfig, loader_json: &LoaderJson)
        -> Result<BundleOutput, ArkdownError>;
}

pub struct BundleOutput {
    pub chunk_graph: ChunkGraph,
    pub output_dir: PathBuf,              // aceModuleBuild/ path
    pub chunk_files: Vec<(String, PathBuf)>, // (chunk_name, file_path) pairs
}
```

---

## loader.json Integration

The bundler reads `loader.json` (written by SPEC-010) to get entry points. The `loader.json`
contract (from `GenerateLoaderJson`):

```json
{
  "compileEntry": ["./ets/entryability/EntryAbility.ets", "./ets/pages/Index.ets"],
  "projectRootPath": "/path/to/project",
  "moduleRootPath": "/path/to/entry",
  "cachePath": "build/default/intermediates/ets/.../cache/",
  "aceModuleBuild": "build/default/intermediates/ets/.../loader_out/default/",
  "buildMode": "debug",
  "supportChunks": false,
  "useNormalizedOHMUrl": true,
  "pkgNameToPkgBriefInfo": { ... }
}
```

The bundler consumes `compileEntry` for entry points and writes output to `aceModuleBuild`.

---

## Files to Create

```
crates/arkdown/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── bundler.rs          (Bundler struct, bundle() orchestration)
    ├── stages/
    │   ├── link.rs         (cross-module symbol resolution)
    │   ├── tree_shake.rs   (side-effect analysis, inclusion marking)
    │   ├── chunk_split.rs  (entry-based chunk assignment, supportChunks logic)
    │   └── codegen.rs      (oxc_codegen invocation, CJS output)
    ├── loader_json.rs      (LoaderJson struct, read from disk)
    └── output.rs           (write chunk files to aceModuleBuild/)
```

---

## Acceptance Criteria

- `Bundler::bundle()` on `MyApplication/entry` produces `.js` chunk files in `aceModuleBuild/`
- CJS output (not ESM) — `require()` calls for cross-module imports
- Release builds tree-shake unused exports; debug builds include everything
- With `supportChunks: false`, single chunk per entry point
- Source maps emitted alongside each `.js` file

---

## Design References

- `meta/design/pipeline.md` — Phases 5–8
- `meta/design/chunk-model.md` — chunk splitting, supportChunks, IIFE requirement
- `meta/design/hvigorw-analysis.md` — loader.json schema, aceModuleBuild path
- `meta/design/types.md` — `ModuleGraph`, `ChunkGraph`, `ArkChunk`
