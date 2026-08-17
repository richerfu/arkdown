# OHOS Chunk Architecture

This document is legacy background for the older wrapper-chunk model. The current arkdown
CompileArkTS path emits ESM chunks through rolldown and compiles them with `es2abc --module`;
it does not ship an `arkdown-plugin-chunk-wrapper` crate.

---

## 1. Legacy Constraint: IIFE Runtime Registry

OHOS's ArkRuntime does **not** support native ES Module `import`/`export` semantics at the JavaScript level. All code runs in a single global context after being compiled to `.abc` bytecode. The module system is implemented on top of `globalThis` as a registry, not via the ECMAScript module specification.

The older wrapper model used this shape:

- Format is `cjs` (CommonJS)
- Cross-chunk dependencies use `globalThis['chunkName_hash']` instead of `require()`/`import()`
- Each chunk is an IIFE that registers its exports into `globalThis`

Do not reintroduce this wrapper for current module-mode ABC compilation unless upstream
hvigor source proves that this path is active for the target build mode.

---

## 2. The IIFE Pattern

### Base IIFE (no chunks)

Every output chunk, regardless of content, is wrapped:

```javascript
(function () {
  "use strict";

  // ... bundled module code ...
})();
```

The legacy wrapper shape above is background for old compatibility analysis.
Current CompileArkTS output stays aligned with hvigor's module-mode source
contract: es2abc receives normal source files and `--module` controls ESM
parsing. When `supportChunks` is false, the output is a single module-mode file,
not a forced IIFE wrapper.

### With `supportChunks` Enabled

> Status: the `wrapperChunkPlugin` Rust adapter has been **deleted**. Its only
> hook (`render_chunk`) is unreachable on the production path — `Bundler::bundle()`
> uses `scan_module_sources()` and never renders chunks; the section below is
> retained as legacy-format background only.

When `supportChunks` is true (enabled via `projectConfig.supportChunks`), the `wrapperChunkPlugin` also processes cross-chunk imports and exports.

**Producing chunk** (exporting module):

```javascript
(function () {
  "use strict";

  // ... bundled code ...
  var exports = {};
  exports.MyComponent = MyComponent;
  exports.helperFn = helperFn;

  globalThis["commons_e5f6a1b2"] = globalThis["commons_e5f6a1b2"] || {
    MyComponent: exports.MyComponent,
    helperFn: exports.helperFn,
  };
})();
```

The `analyzeExport()` function generates the globalThis assignment:

```javascript
function analyzeExport(chunk, allChunks) {
  const exports = [];
  for (const [name] of chunk.exportsByName) {
    exports.push(`${name}: exports.${name}`);
  }
  allChunks.append(
    `\nglobalThis['${chunk.fileName}_${chunk.chunkHash}'] \n` +
      `        = globalThis['${chunk.fileName}_${chunk.chunkHash}'] || {\n` +
      `  ${exports.join(",\n  ")}\n}`,
  );
}
```

**Consuming chunk** (importing from another chunk):

```javascript
(function () {
  "use strict";

  // Before wrapper-chunk-plugin:
  // var _commons = require('./commons');  ← CJS require()

  // After wrapper-chunk-plugin replaceRequireVariable():
  var _commons = globalThis["commons_e5f6a1b2"];

  // ... uses _commons.MyComponent, _commons.helperFn ...

  globalThis["EntryAbility_a1c2d3e4"] = globalThis["EntryAbility_a1c2d3e4"] || {
    default: exports.default,
  };
})();
```

The `replaceRequireVariable()` function finds `var x = require('chunkId')` patterns and replaces them with `globalThis['chunkId_hash']`.

The `removeEmptyRequire()` function removes bare `require('chunkId')` calls (side-effect-only imports that produce no binding).

---

## 3. Chunk Hash

The `chunkHash` in `globalThis['fileName_chunkHash']` comes from rollup's chunk metadata — specifically `chunk.chunkHash` in the `renderChunk` hook's `chunks` parameter. This hash:

- Changes when the chunk content changes
- Provides cache-busting for incremental updates
- Is deterministic for the same input (content-addressed)

In the `renderChunk` hook, `chunks` is the full chunk manifest:

```javascript
renderChunk(code, chunk, options, { chunks }) {
    // chunks[chunkId].chunkHash is the hash for chunkId
    const hash = chunks[importedChunkId]?.chunkHash;
    replacement = `globalThis['${importedChunkId}_${hash}']`;
}
```

---

## 4. `@Entry` Decorator and Entry Points

In ArkTS, the `@Entry` decorator marks a component as an Ability entry point. Each `@Entry`-decorated struct corresponds to one entry file in the rolldown graph.

```typescript
// src/main/ets/pages/Index.ets
@Entry
@Component
struct Index {
    @State message: string = 'Hello World';

    build() {
        Column() {
            Text(this.message)
        }
    }
}
```

This corresponds to an entry in `module.json5`:

```json5
{
  module: {
    pages: "$profile:main_pages", // → main_pages.json lists page paths
    abilities: [
      {
        name: "EntryAbility",
        srcEntry: "./ets/entryability/EntryAbility.ets",
      },
    ],
  },
}
```

`GenerateLoaderJson` collects entry files:

1. `abilities[].srcEntry` — each Ability's source entry
2. `module.srcEntry` — module-level entry (for some module types)
3. Router map `pageSourceFile` entries — pages with named routes
4. Worker files from `buildOption.sourceOption.workers`

Each entry file becomes an entry point in the rolldown module graph → a separate output chunk (when `supportChunks` is true) or part of the merged single chunk.

---

## 5. `supportChunks` Flag Behavior

The `supportChunks` flag controls whether multiple output chunks are produced:

```javascript
// In arkts-pack.js getOutputConfig():
{
  manualChunks: config.supportChunks ? manualChunks : undefined;
}

function manualChunks(id, { getModuleInfo }) {
  if (id.includes(NODE_MODULES) || id.includes(OH_NODE_MODULES)) return "commons";
  const info = getModuleInfo(id);
  return info && !info.isEntry ? "vendors" : undefined;
}
```

**`supportChunks = false`** (default for most modules):

- All code bundled into a minimal number of chunks (one per entry)
- No `manualChunks` function → no explicit chunk splitting
- Simpler `globalThis` registry: no cross-chunk imports
- Most HAR and simple HAP builds

**`supportChunks = true`**:

- `oh_modules/` dependencies → `commons` chunk
- Non-entry local modules → `vendors` chunk
- Entry files → entry-named chunks
- legacy wrapper-chunk processing rewrites cross-chunk require() → globalThis[]
- Used for larger applications with many shared dependencies

---

## 6. Per-Module Bundle Layout

### HAP Module

```
entry.hap (zip)
└── modules.abc           ← merged from all compiled .js chunks
└── resources.index       ← compiled resources
└── module.json           ← processed module manifest
└── libs/arm64-v8a/*.so   ← native libraries (optional)
```

The `modules.abc` contains all module records merged via `es2abc --merge-abc`:

- `EntryAbility` record
- `Index` page record
- `commons` record (if chunks split)
- Worker records (if workers defined)

### HAR Module (Source HAR)

```
my-har/
├── src/main/ets/          ← original .ets source
├── src/main/resources/    ← resources
├── oh-package.json5       ← package manifest
├── index.d.ts / index.d.ets  ← generated type declarations
└── build/default/outputs/default/my-har.har  (zip of above)
```

Source HARs ship source files. The consuming module compiles them as part of its own bundle.

### HAR Module (ByteCode HAR)

```
my-bytecode-har/
├── index.abc              ← pre-compiled bytecode
├── index.d.ts             ← type declarations only
└── oh-package.json5       ← metadata.declarationEntry, metadata.abcPath
```

ByteCode HARs ship pre-compiled `.abc`. The consuming module's `byteCodeHarInfo` in `loader.json` references this `.abc` directly.

### HSP Module

```
shared.hsp (zip)
└── modules.abc           ← compiled bytecode (like HAP)
└── resources.index
└── module.json           ← type: "shared"
└── src/main/ets/         ← source for IDE type checking (in some configurations)
```

HSP modules are deployed once on-device and shared across multiple HAP modules via the `@bundle:` reference mechanism.

---

## 7. Dynamic Import and Chunk Loading

ArkTS supports dynamic import:

```typescript
const { LazyComponent } = await import("./components/LazyComponent");
```

With `supportChunks` enabled, dynamic imports trigger additional chunk splitting. The dynamically imported module becomes its own chunk. At runtime, the loading mechanism must:

1. Look up the chunk in `globalThis` by name+hash
2. If not present, the `.abc` loader must load and execute the chunk's bytecode

However, since es2abc compiles everything to a single `.abc` (via `--merge-abc`), all chunks are available immediately — the dynamic split is a logical split, not a lazy-load split. The `globalThis` registry ensures the module's exports are available when accessed.

`dynamicImportLibInfo` in `loader.json` tracks cross-HAR dynamic imports for the build system (not the runtime):

```json
{
  "dynamicImportLibInfo": {
    "my-har": {
      "hostModulesInfo": [{ "hostDependencyName": "my-har", "hostModuleName": "entry" }],
      "moduleName": "my-har",
      "entryFilePath": "/path/to/my-har/src/main/ets/index.ets",
      "isLocalDependency": true,
      "pkgPath": "/path/to/my-har"
    }
  }
}
```

---

## 8. `module.json5` Connection to Entry Chunk Selection

The `module.json5` `mainElement` and `srcEntry` fields determine which chunk is the "primary" entry:

```json5
{
  module: {
    mainElement: "EntryAbility", // launcher entry point
    abilities: [
      {
        name: "EntryAbility",
        srcEntry: "./ets/entryability/EntryAbility.ets", // → chunk entry
      },
    ],
  },
}
```

`mainElement` is the ability that appears as the HAP's home screen icon. Its `srcEntry` is the first entry point loaded when the HAP launches.

In rolldown chunk terminology: the `srcEntry` file is an entry module in the `ModuleGraph`. The resulting chunk for this entry is the "primary" chunk whose `globalThis` registration is evaluated first on HAP startup.

---

## 9. HAR vs HAP vs HSP Chunk Strategies

| Module Type               | Chunk Strategy                                          | Output                   |
| ------------------------- | ------------------------------------------------------- | ------------------------ |
| HAP (`entry`/`feature`)   | Single merged `.abc` per HAP; `supportChunks` optional  | `modules.abc` in HAP zip |
| HSP (`shared`)            | Same as HAP; separate HAR dedup pass removes duplicates | `modules.abc` in HSP zip |
| Source HAR                | No ABC compilation; source files shipped                | `.ets` files in HAR zip  |
| ByteCode HAR              | Single `.abc` per HAR; compiled from HAR's own entry    | `index.abc` in HAR zip   |
| HAR (obfuscated artifact) | Source stripped; declarations + bytecode                | `.d.ets` + `.abc`        |

### HAR Deduplication (HSP builds)

When an HSP depends on a HAR that is also a dependency of the consuming HAP, the HAR source is compiled into the HSP only (not duplicated in the HAP). This is the `HAR deduplication` feature:

- `hsp-collect-har-files.js` collects which HAR files belong to the HSP
- `har-deduplicate-util.js` identifies which HARs to exclude from the HAP compile
- `GenerateLoaderJson.processHarDeduplicate()` adjusts `compileEntry` to skip deduplicated HARs

In arkdown, `arkdown-plugin-har-dedup` implements this same deduplication pass.

---

## Cross-Reference

- [abc-pipeline.md](abc-pipeline.md) — current `es2abc --module` input contract
- [pipeline.md](pipeline.md) — current rolldown CompileArkTS pipeline
- [hvigorw-analysis.md](hvigorw-analysis.md) — `supportChunks` flag and loader.json
