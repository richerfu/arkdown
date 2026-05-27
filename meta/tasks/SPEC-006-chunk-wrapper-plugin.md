# SPEC-006: Chunk Wrapper Plugin (IIFE)

**Crate**: `arkdown-plugin-chunk-wrapper`
**Priority**: P0 — without this, es2abc receives invalid input
**Blocked by**: SPEC-005
**Blocks**: SPEC-009

---

## Summary

Implements the `render_chunk` plugin hook that wraps every output chunk in an IIFE and, when
`supportChunks` is enabled, replaces CJS `require()` calls with `globalThis['name_hash']`
references. This is a direct Rust reimplementation of `wrapper-chunk-plugin.js`.

The ArkRuntime does not support native ESM. Every chunk MUST be wrapped in an IIFE. This
plugin is always active — there is no config flag to disable the IIFE wrapping.

---

## Exact Behavior (from wrapper-chunk-plugin.js)

### Always Applied

Every chunk is wrapped:

```javascript
// Input (CJS from rolldown codegen):
"use strict";
var _helper = require("./helper");
exports.MyComponent = function() { ... };

// Output:
(function () {
"use strict";
var _helper = require("./helper");
exports.MyComponent = function() { ... };
})()
```

### When `supportChunks = true`

Additionally:

**1. Replace `var x = require('./chunkId')` with `var x = globalThis['chunkId_hash']`**

The `replaceRequireVariable` function finds all `VariableDeclaration` nodes where the
init is a `CallExpression` with callee `require`. For each matched chunk import, replaces
the entire `require(...)` call with the globalThis lookup:

```javascript
// Before:
var _commons = require("./commons");

// After:
var _commons = globalThis["commons_e5f6a1b2"];
```

Where `e5f6a1b2` is `chunks['commons'].chunkHash`.

**2. Remove bare side-effect `require('./chunkId')` (no binding)**

The `removeEmptyRequire` function removes `ExpressionStatement` nodes that are bare
`require()` calls with no variable binding, where the imported chunk has no bindings
in `chunk.importedBindings[chunkId]`.

**3. Append globalThis registry export**

The `analyzeExport` function appends to each exporting chunk:

```javascript
globalThis["fileName_chunkHash"] = globalThis["fileName_chunkHash"] || {
  ExportA: exports.ExportA,
  ExportB: exports.ExportB,
};
```

Where `fileName` is `chunk.fileName` and `chunkHash` is `chunk.chunkHash`.

---

## Implementation

```rust
pub struct ChunkWrapperPlugin {
    support_chunks: bool,
}

impl ArkdownPlugin for ChunkWrapperPlugin {
    fn name(&self) -> &str { "chunk-wrapper" }

    async fn render_chunk(
        &self,
        ctx: &RenderChunkContext,
        code: String,
        chunk: &ArkChunk,
    ) -> PluginResult<Option<RenderedChunk>> {
        let mut output = String::new();
        output.push_str("(function () {\n");

        if self.support_chunks {
            let processed = self.process_chunk_imports(ctx, &code, chunk)?;
            output.push_str(&processed);
        } else {
            output.push_str(&code);
        }

        output.push_str("\n})()");

        if self.support_chunks && !chunk.exports_by_name.is_empty() {
            output.push_str(&self.generate_registry_export(chunk));
        }

        Ok(Some(RenderedChunk { code: output, map: None }))
    }
}
```

### Import Processing

Use oxc to parse the CJS chunk code and find `require()` calls:

```rust
fn process_chunk_imports(&self, ctx: &RenderChunkContext, code: &str, chunk: &ArkChunk)
    -> PluginResult<String>
{
    let alloc = Allocator::default();
    let ast = Parser::new(&alloc, code, SourceType::cjs()).parse().program;

    // Walk AST for VariableDeclaration nodes with require() init
    // Walk AST for ExpressionStatement nodes with bare require() calls
    // Use oxc_codegen or string replacement to rewrite
}
```

### chunk_hash Lookup

The chunk hash for a required chunk comes from `ctx.chunks[chunk_id].chunk_hash`. The
`RenderChunkContext` must expose the full chunk manifest for the current build.

---

## Source Map

The plugin modifies source content. Source maps must be updated accordingly. Use oxc's
`MagicString`-equivalent (or the `string_wizard` crate from rolldown, which is already in
the rolldown workspace) for position-tracking string modifications.

---

## Files to Create

```
crates/arkdown-plugin-chunk-wrapper/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── plugin.rs           (ChunkWrapperPlugin: ArkdownPlugin impl)
    ├── rewrite.rs          (require() → globalThis[] AST rewriting)
    └── registry.rs         (globalThis registry export generation)
```

---

## Acceptance Criteria

- Every chunk output starts with `(function () {` and ends with `})()`
- With `supportChunks: false`, no AST analysis is done — just string prepend/append
- With `supportChunks: true`, all `var x = require('./chunkId')` patterns replaced with `globalThis['chunkId_hash']`
- `require()` with unknown chunk IDs (external modules) are left unchanged
- Source maps account for the prepended `(function () {\n` offset

---

## Design References

- `meta/design/chunk-model.md` — complete IIFE pattern specification with exact code
- `meta/design/plugins.md` — `wrapper-chunk-plugin` source analysis
- `meta/design/abc-pipeline.md` — this plugin's output is the input to es2abc

---

## Rolldown Alignment

| arkdown concept                                         | rolldown equivalent                                          | note                                                                                                                                                                                |
| ------------------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ChunkWrapperPlugin::render_chunk`                      | `Plugin::render_chunk`                                       | Same hook; rolldown passes `RenderedChunk` meta; arkdown passes `&ArkChunk`                                                                                                         |
| IIFE wrapping `(function () { ... })()`                 | rolldown `OutputOptions::format = Iife`                      | rolldown can emit IIFE natively; arkdown implements it as an explicit plugin because it also needs the `globalThis` registry injection that rolldown's IIFE format does not produce |
| `require()` → `globalThis['name_hash']` replacement     | rolldown cross-chunk `import` → `require()` in CJS format    | arkdown goes one step further: CJS `require()` calls for same-bundle chunks are replaced with `globalThis` lookups because ArkRuntime has no module loader                          |
| `globalThis['fileName_hash'] = { ... }` registry export | rolldown chunk's `exports` object                            | Rolldown emits `Object.defineProperty(exports, ...)` in CJS; arkdown appends a `globalThis` assignment instead                                                                      |
| `chunk.chunk_hash` in registry key                      | rolldown `Chunk::content_hash`                               | Same value; both are derived from chunk content                                                                                                                                     |
| `RenderChunkContext::chunks` (full chunk manifest)      | rolldown `RenderChunkContext::get_module_info` + chunk graph | arkdown's context must expose the full `ChunkGraph` so `chunk_hash` for other chunks can be looked up                                                                               |

**Key divergences from rolldown that must always be preserved:**

- IIFE wrapping is always applied, even for single-chunk builds (ArkRuntime requires it)
- `globalThis` registry export is arkdown-specific: rolldown does not produce `globalThis` assignments
- `require()` calls to unknown (non-chunk) IDs must be left unchanged — external references use the OHM URL as their ID, not a relative path

---

## Test Requirements

- Every chunk output starts with exactly `(function () {\n` and ends with `\n})()`
- With `support_chunks: false`: no AST parsing occurs; output is purely string prepend/append
- With `support_chunks: true`: `var _commons = require("./commons")` replaced with `var _commons = globalThis["commons_e5f6a1b2"]` where the hash matches `ctx.chunks["commons"].chunk_hash`
- Bare side-effect `require("./commons")` (no variable binding, chunk has no imported bindings) is removed entirely
- `require("@bundle:com.foo/mod/path")` (external reference) is left unchanged (not replaced with globalThis)
- Registry export: chunk with `exports.MyComponent` gets `globalThis["chunkName_hash"] = globalThis["chunkName_hash"] || { MyComponent: exports.MyComponent }`
- Registry export uses `||` short-circuit so a chunk loaded multiple times does not overwrite itself
- Source map offset is correct: the prepended `(function () {\n` shifts all line numbers by 1; verify the source map line offset is accounted for
- Empty chunk (no exports, no imports) still gets IIFE wrapping
- Plugin name returns `"chunk-wrapper"`
