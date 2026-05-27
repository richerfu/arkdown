# TEST-006: arkdown-plugin-chunk-wrapper

**Crate**: `arkdown-plugin-chunk-wrapper`
**Original source**: `hvigor-arkts-compose/dist/src/plugins/wrapper-chunk-plugin.js`

All behavioral test cases are derived directly from reading `wrapper-chunk-plugin.js`.
Passing every case here is equivalent to matching the original plugin's behavior.

---

## Test File Layout

```
crates/arkdown-plugin-chunk-wrapper/src/
├── tests/
│   ├── iife_wrap.rs          ← basic IIFE wrapping (always active)
│   ├── require_rewrite.rs    ← require() → globalThis[] replacement
│   ├── export_registry.rs    ← globalThis export registration appended to chunk
│   ├── empty_require.rs      ← bare side-effect require() removal
│   └── source_map.rs         ← source map position offset
```

---

## iife_wrap.rs — Basic IIFE Wrapping

Applied to every chunk regardless of `supportChunks`.

### TC-001: plain code wrapped in IIFE

- **Input** (`support_chunks = false`):
  ```javascript
  "use strict";
  var x = 1;
  ```
- **Expected**:
  ```javascript
  (function () {
    "use strict";
    var x = 1;
  })();
  ```
- **Origin**: `renderChunk` always prepends `(function () {\n` and appends `\n})()`

### TC-002: empty chunk still wrapped

- **Input**: `"use strict";`
- **Expected**: `(function () {\n"use strict";\n})()`

### TC-003: `support_chunks = false` performs no AST analysis

- **Input** (CJS with require): `var _x = require('./commons');`
- **Expected** (unchanged inside IIFE): `(function () {\nvar _x = require('./commons');\n})()`
- **Origin**: `analyzeImportAndExport` only called when `projectConfig.supportChunks === true`

---

## require_rewrite.rs — require() → globalThis[]

Only active when `support_chunks = true`.

### TC-004: bound require → globalThis lookup

- **Input**:
  ```javascript
  var _commons = require("./commons");
  ```
  with chunk manifest `{ "commons": { chunkHash: "e5f6a1b2" } }`
- **Expected**:
  ```javascript
  var _commons = globalThis["commons_e5f6a1b2"];
  ```
- **Origin**: `replaceRequireVariable` matches `VariableDeclaration` where init is `CallExpression`
  with callee name `"require"`. Rewrites init to `globalThis['${chunkId}_${chunks[chunkId].chunkHash}']`.

### TC-005: require with unknown chunk ID left unchanged

- **Input**: `var _path = require('path');`
  (no entry for `'path'` in the chunk manifest's `importsPathMap`)
- **Expected**: `var _path = require('path');` (unchanged)
- **Origin**: `replaceRequireVariable` only rewrites if `n.importsPathMap.get(specifier)` is truthy.

### TC-006: require with renamed binding

- **Input**: `var Utils = require('./utils');` with `{ "utils": { chunkHash: "a1b2c3d4" } }`
- **Expected**: `var Utils = globalThis['utils_a1b2c3d4'];`

### TC-007: destructured require not rewritten (only simple var assignments)

- **Input**: `var { foo, bar } = require('./commons');`
- **Expected**: unchanged — `replaceRequireVariable` only handles the simple init pattern,
  destructured bindings are not `VariableDeclarator` with a direct `require()` call as init.

---

## empty_require.rs — Bare Side-Effect require() Removal

### TC-008: bare require with no binding removed

- **Input**:
  ```javascript
  require("./commons");
  ```
  where `chunk.importedBindings['./commons']` is empty (`[]`)
- **Expected**: the `require('./commons');` statement is removed entirely
- **Origin**: `removeEmptyRequire` matches `ExpressionStatement` nodes whose expression is a
  `CallExpression` with callee `require`, where the imported chunk has no bindings.

### TC-009: bare require with non-empty bindings NOT removed

- **Input**: `require('./commons');`
  where `chunk.importedBindings['./commons'] = ['MyComponent']`
- **Expected**: statement kept (not removed)
- **Origin**: `removeEmptyRequire` only removes when `n.importedBindings[e].length === 0`.

### TC-010: bare require with unknown chunk ID not removed

- **Input**: `require('fs');`
- **Expected**: statement kept

---

## export_registry.rs — globalThis Export Registration

### TC-011: exporting chunk gets registry appended

- **Input** chunk has exports: `exportsByName = [['MyComponent', ...], ['helperFn', ...]]`
  with `fileName = 'commons'` and `chunkHash = 'e5f6a1b2'`
- **Expected** appended to chunk (after IIFE close):
  ```javascript
  globalThis["commons_e5f6a1b2"] = globalThis["commons_e5f6a1b2"] || {
    MyComponent: exports.MyComponent,
    helperFn: exports.helperFn,
  };
  ```
- **Origin**: `analyzeExport` iterates `chunk.exportsByName`, builds property list, appends
  via `allChunks.append(...)`.

### TC-012: chunk with no exports gets no registry line

- **Input** chunk: `exportsByName` is empty
- **Expected**: no `globalThis[...]` line appended

### TC-013: registry key is `fileName_chunkHash` (underscore separator)

- **Input**: `fileName = 'EntryAbility'`, `chunkHash = 'a1b2c3d4'`
- **Expected key**: `'EntryAbility_a1b2c3d4'`
- **Origin**: template literal `` `${chunk.fileName}_${chunk.chunkHash}` ``

### TC-014: registry uses OR pattern for idempotency

- **Expected**: `globalThis['X'] = globalThis['X'] || { ... }`
- **Origin**: prevents overwriting if chunk is evaluated twice

---

## source_map.rs — Source Map Correctness

### TC-015: prepended IIFE prefix offsets all original positions by one line

- **Input**: source at line 1 col 0
- **Expected**: mapped position in output is line 2 col 0 (offset by `(function () {\n`)
- **Note**: the MagicString-equivalent must track byte offsets for the prepend

### TC-016: source map produced when input chunk has a source map

- **Input**: chunk with `.source_map = Some(map)` pointing to original `.ets` file
- **Expected**: output `RenderedChunk::map` is `Some(...)` with positions adjusted
