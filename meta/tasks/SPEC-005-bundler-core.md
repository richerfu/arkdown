# SPEC-005: Bundler Core (rolldown integration)

**Crate**: `arkdown`
**Priority**: P0
**Blocked by**: SPEC-001, SPEC-003, SPEC-004
**Blocks**: SPEC-007, SPEC-008, SPEC-009, SPEC-014, SPEC-016, SPEC-017, SPEC-018

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

For each chunk, rolldown generates JavaScript source. Output format is **ESM** for the
CompileArkTS module-mode pipeline, matching hvigor's `es2abc --module` input contract.

```rust
let codegen = CodeGenerator::new()
    .with_options(CodegenOptions { source_map: true, ..Default::default() });
let output = codegen.build(&ast);
// output.code: String, output.map: Option<SourceMap>
```

### Stage 5: render_chunk Plugin Hook

For each chunk, rolldown runs the `render_chunk` plugin hook chain. arkdown does not add
an IIFE/CommonJS wrapper in this stage.

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

## Required Plugin Wiring Order

The bundler (and CLI) must wire exactly these 6 plugins, **in this order**:

```
1. EtsTransformPlugin      (arkdown-plugin-ets-transform)   — ArkTS desugar; runs first on .ets files
2. OhosResolvePlugin       (arkdown-plugin-ohos-resolve)    — OHOS module resolution
3. ModuleMetaPlugin        (arkdown-plugin-module-meta)     — per-module OHOS metadata annotation
4. Json5Plugin             (arkdown-plugin-json5)           — JSON/JSON5 import support
5. DuplicateCheckPlugin    (arkdown-plugin-duplicate-check) — filename uniqueness enforcement
```

`EtsTransformPlugin` must be first (transform must happen before resolve + parse). Order of 2–5 is
stable.

```rust
let plugins: Vec<Arc<dyn ArkdownPlugin>> = vec![
    Arc::new(EtsTransformPlugin::new(&config)),
    Arc::new(OhosResolvePlugin::new(&config, resolver.clone())),
    Arc::new(ModuleMetaPlugin::new(&config)),
    Arc::new(Json5Plugin::new()),
    Arc::new(DuplicateCheckPlugin::new()),
];
let bundler = Bundler::new(config, plugins);
```

## loader.json Integration

The bundler reads `loader.json` (written by SPEC-010) to get entry points. The `loader.json`
contract (from `GenerateLoaderJson`):

```json
{
  "compileEntry": ["./ets/entryability/EntryAbility.ets", "./ets/pages/Index.ets"],
  "projectRootPath": "/path/to/project",
  "moduleRootPath": "/path/to/entry/src/main",
  "moduleName": "entry",
  "apiVersion": 12,
  "modulePathMap": { "entry": "/path/to/entry" },
  "fallbackAnBuild": false,
  "cachePath": "build/default/intermediates/ets/.../cache/",
  "aceModuleBuild": "build/default/intermediates/ets/.../loader_out/default/",
  "buildMode": "debug",
  "supportChunks": false,
  "useNormalizedOHMUrl": true,
  "pkgNameToPkgBriefInfo": { ... }
}
```

Key fields added/updated:

| Field             | Value                               | Note                                            |
| ----------------- | ----------------------------------- | ----------------------------------------------- |
| `moduleRootPath`  | `/abs/path/to/entry/src/main`       | Points to `src/main`, not just module root      |
| `moduleName`      | `"entry"`                           | OHOS module name                                |
| `apiVersion`      | `12`                                | Numeric API version (from compatibleSdkVersion) |
| `modulePathMap`   | `{ "entry": "/abs/path/to/entry" }` | Maps module name → absolute path                |
| `fallbackAnBuild` | `false`                             | AN (ahead-of-native) build fallback flag        |

The bundler consumes `compileEntry` for entry points and writes output to `aceModuleBuild`.

---

## Files to Create

```
crates/arkdown/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── bundler.rs          (Bundler struct, bundle() orchestration)
    ├── compile_entry.rs    (hvigor compileEntry naming)
    └── output.rs           (write chunk files to aceModuleBuild/)
```

---

## Acceptance Criteria

- `Bundler::bundle()` on `MyApplication/entry` produces `.js` chunk files in `aceModuleBuild/`
- ESM output suitable for `es2abc --module`
- Release builds tree-shake unused exports; debug builds include everything
- With `supportChunks: false`, single chunk per entry point
- Source maps are emitted as hvigor-compatible `sourceMaps.map`

---

## Design References

- `meta/design/pipeline.md` — Phases 5–8
- `meta/design/chunk-model.md` — legacy chunk-wrapper background only
- `meta/design/hvigorw-analysis.md` — loader.json schema, aceModuleBuild path
- `meta/design/types.md` — `ModuleGraph`, `ChunkGraph`, `ArkChunk`

---

## Rolldown Alignment

| arkdown concept                       | rolldown equivalent                           | note                                                                                                                   |
| ------------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `Bundler::bundle()`                   | rolldown `Bundler::write()`                   | Same: takes entry points, returns chunk graph and writes files                                                         |
| Stage 1 (Link)                        | rolldown link stage                           | Identical: resolve cross-module symbol references                                                                      |
| Stage 2 (Tree Shake)                  | rolldown tree-shaking (`treeshake: true`)     | Same algorithm (inclusion marking); arkdown enables only in `BuildMode::Release`                                       |
| Stage 3 (Chunk Split)                 | rolldown `manualChunks` + automatic splitting | arkdown reads `loader.json` `compileEntry`; rolldown reads `input` option; both produce `ChunkGraph`                   |
| Stage 4 (Code Generation, ESM output) | rolldown `OutputOptions::format = Esm`        | Matches hvigor CompileArkTS module-mode input                                                                         |
| `render_chunk` hook dispatch          | rolldown `PluginDriver::render_chunk()`       | Identical hook semantics                                                                                               |
| `generate_bundle` hook dispatch       | rolldown `PluginDriver::generate_bundle()`    | Identical                                                                                                              |
| `BundleOutput::chunk_graph`           | rolldown `BundleOutput` / `RolldownOutput`    | Same concept; arkdown adds `output_dir` and `chunk_files` for direct file path tracking                                |
| `LoaderJson::compileEntry`            | rolldown `InputOptions::input`                | Functionally equivalent: both specify entry point file paths                                                           |
| `supportChunks` flag                  | rolldown always splits chunks                 | OHOS-specific: when `false`, one chunk per entry (no shared chunks)                                                    |
| `ArkChunk::chunk_hash`                | rolldown `Chunk::content_hash`                | Same purpose (fingerprinting)                                                                                         |
| `cross_chunk_imports` on `ArkChunk`   | rolldown `Chunk::cross_chunk_imports`         | Identical semantics                                                                                                   |

**Key divergences from rolldown that must always be preserved:**

1. Output format is ESM for module-mode ABC compilation; do not add CJS/IIFE wrapping unless upstream hvigor introduces that contract for this path.
2. Entry points come from `loader.json` `compileEntry`, not from an `input` option struct.
3. `supportChunks = false` (the most common OHOS case) means exactly one chunk per entry — no shared chunk extraction.

---

## Test Requirements

- `Bundler::bundle()` on a two-file project (`A.ets` imports from `B.ets`) produces a single chunk per entry with both modules inlined (when `supportChunks: false`)
- ESM output verification: chunk code stays compatible with module-mode ABC compilation
- With `supportChunks: true`, a module imported from two entries is extracted into a separate chunk
- Tree shaking in release mode: an exported but never-imported function is absent from the output
- Tree shaking in debug mode: all exports are present regardless of whether they are imported
- Source maps written to hvigor-compatible `sourceMaps.map` in `aceModuleBuild/`
- `render_chunk` hook is called once per chunk; returning `None` from all plugins leaves code unchanged
- `generate_bundle` hook is called once after all chunks are rendered; modifying `bundle.chunks` in the hook affects written output
- `loader.json` with `compileEntry: ["./ets/pages/Index.ets"]` produces exactly one entry chunk named `"Index"`
- Circular import between two modules is warned about (via `tracing::warn!`) but does not abort the build
- `BundleOutput::chunk_files` entries all exist on disk after `bundle()` completes
