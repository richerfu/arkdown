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
