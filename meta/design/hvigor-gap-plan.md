# hvigor Alignment Gap Analysis and Plan

This document records the current gap between `arkdown` and the locally installed
hvigor pipeline, with emphasis on the code compilation path. It is based on the
repository implementation plus a local DevEco command-line-tools installation.

## Local hvigor Evidence

The top-level `hvigorw` wrapper resolves to:

- `<command-line-tools>/bin/hvigorw`
- real entry: `<command-line-tools>/hvigor/bin/hvigorw`
- packages: `@ohos/hvigor-arkts-base@6.21.1`,
  `@ohos/hvigor-arkts-compose@6.21.1`
- bundler: `rollup@3.19.1-htrunk1.AllScenario.devecostudio.r8`

Important source files:

- `@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
- `@ohos/hvigor-arkts-base/dist/src/build.js`
- `@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js`
- `@ohos/hvigor-arkts-base/dist/src/build-steps/process-importee.js`

The local `build.js` flow is:

1. read rollup msgpack cache;
2. run `rollup(config)`;
3. write or generate output;
4. run `processImportees()` for ByteCode HAR and direct importee caches;
5. remove redundant declarations and failed cache files;
6. write rollup cache;
7. trigger bundle-end async work.

`load-sdk-functions.js` injects SDK compile plugins from `ets-loader`:
`etsChecker`, `etsTransform`, `apiTransform`, coverage instrumentation, `genAbc`,
and release-mode terser/minification when applicable.

## Current arkdown Status (2026-05-29)

`arkdown-cli` performs the full production pipeline:

1. parse config;
2. compile resources and process profiles (loader.json written with absolute paths);
3. bundle via rolldown (clean ESM, `@ohos:` imports external, treeshake disabled for debug,
   no IIFE/CJS wrapper — IIFE-unwrap and `compile_commonjs_main_wrapper` removed);
4. post-bundle importee processing (nonEntryImportees, compiledImportees, importee cache);
5. compile `.js` chunks via `arkdown-abc` (with LNP line tracking and scopeNames);
6. package and sign HAP/HSP, or package source HAR.

**Phase delivery (2026-05-29):**
- Phase 0 (parity harness): ✅ minimal-hap + minimal-har + minimal-hsp fixtures
- Phase 1 (loader.json): ✅ absolute paths, 18 fields match hvigor golden exactly
- Phase 2 (ETS transform): ✅ @Prop, @Link, @ObjectLink, @Provide, @Consume, @Watch, @BuilderParam
- Phase 3 (bundler): ✅ clean ESM, @ohos: external, treeshake off for debug
- Phase 4 (importee): ✅ process_importees() ports hvigor process-importee.js
- Phase 5 (ABC): 🔄 modules.abc 8675B vs hvigor 9844B (88.1% parity — structural fixes applied).
  Changes: LNP index moved to file end (matching hvigor layout), OHM URL method names restored,
  field name string deduplication across classes, method index reordered to
  [strings | methods | literal_arrays] matching hvigor. 51 abc tests pass.
  Remaining gap (1169B): LNP body content density (~737B) + method index ordering differences.
- Phase 6 (packaging): ✅ HAP zip identical to hvigor (11 entries)

**Parity (minimal-hap, 2026-05-29) — see `meta/design/parity-report-minimal-hap.md`:**
- `loader.json`: ✅ 100% match (all 18 fields identical; absolute paths differ by machine).
- `module.json`: 🔄 ~75% — 3 schema fields missing (`virtualMachine`, `compileMode`,
  `dependencies`) and 8 resource integer IDs missing (`iconId`, `labelId`, `descriptionId`,
  `startWindowIconId`, `startWindowBackgroundId` in abilities/app). Root cause: `arkdown-resource`
  does not yet write resolved IDs back into merged module.json; `virtualMachine` not injected
  from SDK metadata.
- `ets/modules.abc`: 🔄 88.1% by size (8675B vs 9844B; gap 1169B). ✅ Literal arrays,
  method records, class method OHM names match hvigor. ✅ LNP index at file end. ✅ Field name
  string deduplication. ✅ Method index ordering [strings | methods | lits]. Known gaps:
  1. **LNP body density**: 81B/LNP vs hvigor 113B/LNP (per-statement vs per-instruction ADVANCE).
  2. **Method index count**: 98 vs 77 (we have extra entries due to per-class string dedup).
  3. **BUG**: absolute filesystem path (`/Users/.../pages/Index`) leaks into StringItems.
- `ets/sourceMaps.map`: 🔄 ~80% structural match. Keys and sources arrays identical.
  EntryAbility VLQ density: 329 chars vs 910 in golden (per-statement vs per-token maps).
  Index.ts VLQ: 381 chars vs 366 (close match).

## Major Missing Capabilities

| Area                     | Current implementation                                                                                                                                                                                                                                                                                                                      | Gap to hvigor                                                                                                                                                                             |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loader.json`            | ✅ **Fixed (2026-05-29)**: absolute paths via `canonicalize()`, all 18 fields match hvigor golden exactly. `nonEntryImportees`/`compiledImportees` populated by Phase 4 importee pass. | ByteCode HAR metadata, workers, HSP/HAR OHM maps — not needed for minimal-hap fixture. |
| ArkTS transform          | ✅ **Fixed (2026-05-29)**: `@State`, `@Prop`, `@Link`, `@ObjectLink`, `@Provide`, `@Consume`, `@Watch`, `@BuilderParam`, storage decorators all implemented. Per-statement source maps via oxc_codegen. | `@Builder`, `@Styles` functions; precise per-token source maps. EntryAbility VLQ: 329 vs 910 chars. |
| Rollup/rolldown options  | ✅ **Fixed (2026-05-28)**: Clean ESM output, `@ohos:` imports external, treeshake disabled for debug. **IIFE/CJS compatibility paths removed** from both bundler and ABC compiler. | Release treeshake, `manualChunks` for `supportChunks`, ByteCode HAR/HSP externals. |
| ByteCode HAR importees   | ✅ **Fixed (2026-05-29)**: `process_importees()` ports `process-importee.js`. BFS from entry chunks, OHM URL generation, importee.json cache written. `nonEntryImportees` in loader.json. | ByteCode HAR compilation flow (requires HAR input fixture). |
| module.json merge        | 🔄 **Partial**: structural merge correct. Missing: `virtualMachine`, `compileMode`, `dependencies` fields; 8 resource integer IDs (`iconId`, `labelId`, `descriptionId`, `startWindowIconId`, `startWindowBackgroundId`). | Write resolved resource IDs from `arkdown-resource` back into profile. Inject `virtualMachine` from SDK metadata. |
| ABC compiler             | 🔄 **Improved**: 88.1% parity (8675B vs 9844B). **Phase 5 structural fixes**: LNP index moved to file end, OHM URL method names, field name dedup, method index reordered [strings\|methods\|lits]. Known gaps: (1) LNP body density 81B vs 113B/LNP; (2) method index 98 vs 77 entries; (3) absolute fs path in StringItems. | See parity-report-minimal-hap.md §5 for complete string diff. |
| Obfuscation/minification | `arkdown-obfuscate` exists but not wired in.                                                                                                                                           | hvigor release builds run SDK obfuscation + terser before ABC.                          |
| Incremental cache        | `arkdown-cache` exists but not wired in.                                                                                                                                               | hvigor rollup msgpack cache + es2abc `--cache-file` support.                            |
| Source maps/debug        | 🔄 Per-statement source maps. sourceMaps.map 1281B vs 1847B golden. Keys and sources arrays match 100%. Index.ts mapping close (381 vs 366 chars). EntryAbility 2.8× short (329 vs 910). | Per-token source map granularity to match tsc/ets-loader output. Low priority until Phase 2 complete. |
| Packaging metadata       | ✅ **Fixed (2026-05-28)**: HAP zip identical to hvigor — correct module.json merge, pack.info, pkgContextInfo.json (intermediate, not in HAP), resources.index. | None for minimal-hap. HAR/HSP-specific pack flows for complex projects.                 |

## Code Compilation Gap

The critical chain to align is:

```text
loader.json
  -> ArkTS SDK plugin config
  -> rollup/rolldown build
  -> post-rollup importee processing
  -> release obfuscation/minification
  -> es2abc/genAbc
  -> modules.abc + sourceMaps.map + importee caches
```

Current `arkdown` short-circuits several parts:

- `EtsTransformPlugin` rewrites `@kit.*` and stubs ArkUI `build()` bodies, so UI code
  can compile syntactically but not semantically.
- `transform::transform_ets()` is a conservative text pass rather than the full
  SDK `etsTransform`.
- `arkdown::Bundler` drops rolldown source maps and does not configure release
  tree-shake/manual chunk behavior to mirror hvigor.
- `arkdown-abc::AbcCompiler` ignores `cache_path`, does not emit real debug
  sections, and only lowers a subset of JavaScript expressions/statements.
- CLI passes `cache_path: None` into `AbcCompiler` and does not run obfuscation
  before ABC generation.

The result is that code output can be structurally packageable but is not yet a
runtime-equivalent replacement for hvigor for real ArkUI applications.

## Implementation Plan

### Phase 0: Golden Parity Harness

Status: implemented for the minimal HAP fixture as an ignored CLI integration
test. It builds one clean fixture copy with hvigor and another with native
`arkdown build`, then writes a structured diff report.

Keep expanding the harness before broadening native implementation:

- Build the same fixture with local hvigor:
  `<command-line-tools>/bin/hvigorw assembleHap`.
- Build it with `cargo run -p arkdown-cli -- build --project <fixture>`.
- Compare intermediate files: `loader.json`, rollup output JS, importee cache
  JSON, `sourceMaps.map`, `module.json`, `pkgContextInfo.json`, `pack.info`,
  `resources.index`, and `ets/modules.abc`.
- Add an artifact diff report that classifies differences as path/layout,
  semantic JSON, bytecode header, bytecode records, or packaging.

For the current fixture, native comparison is expected to report the remaining
implementation gaps until the phases below land.

### Phase 1: Exact loader and compile configuration

Implement the hvigor `generateConfig()` contract in Rust:

- Fill all `loader.json` fields used by `arkts-pack.js` and SDK plugins.
- Build dependency maps for local HAR, remote HAR, HSP, dynamic dependencies,
  ByteCode HAR, and package source roots.
- Add workers and router map entries to `compileEntry`.
- Preserve hvigor intermediate paths:
  `build/default/intermediates/loader/default/loader.json`,
  `ets/default/loader_out/default/`, and `cache/msgpack/`.
- Match release/debug flags, `supportChunks`, `useNormalizedOHMUrl`,
  package manager type, API version, and source root resolution.

### Phase 2: Real ArkTS transform

Replace the current text/stub implementation with a real `ets-loader` port:

- Parse ArkTS component structs and generate ViewPU-compatible classes.
- Lower `build()` DSL into `observeComponentCreation2`, `.create()`,
  attribute calls, and `.pop()` calls.
- Implement decorators: `@State`, `@Prop`, `@Link`, storage decorators,
  `@Provide`, `@Consume`, `@ObjectLink`, `@Watch`, `@Builder`, and
  `@BuilderParam`.
- Preserve checker diagnostics and fail on SDK-equivalent fatal errors.
- Generate source maps from original `.ets` to transformed TypeScript.

Until this is done, any real ArkUI UI page is only syntactically compiled.

### Phase 3: Align rolldown with hvigor rollup

Make rolldown output match hvigor's rollup contract:

- Configure release `treeshake` and debug no-treeshake behavior.
- Implement hvigor `manualChunks` for `supportChunks`.
- Preserve chunk filenames/stems used by ABC record names.
- Enable and serialize source maps.
- Implement external handling for SDK modules, HSP, ByteCode HAR, declarations,
  native `.so`, and deduplicated HAR modules.
- Add a compatibility path for custom compile plugins only after core parity.

### Phase 4: Post-rollup importee processing

Port `process-importee.js`:

- Walk rollup/rolldown cache module `resolvedIds`.
- Filter valid importees with hvigor's rules.
- Write `nonEntryImportees`, `compiledImportees`, and direct importee cache files.
- Wire ByteCode HAR and bundled dependency data into `loader.json` and resolver
  output.

This is required for ByteCode HAR correctness and incremental rebuilds.

### Phase 5: ABC parity

There are two practical tracks:

- Short-term parity aid: allow the golden tests to compare native output against
  hvigor output produced outside arkdown.
- Long-term Rust backend: continue completing `arkdown-abc` until its output is
  runtime-equivalent.

For the Rust backend, implement in this order:

1. real method/literal-array layout for functions, classes, closures, imports,
   exports, and CommonJS helpers;
2. full statement/expression lowering: loops, try/catch/finally, destructuring,
   optional chaining, spread, async/await, generators, template literals,
   object/array literals, class fields, private fields, decorators output;
3. module request annotations and metadata exactly as `es2abc` emits them;
4. debug info and source position sections;
5. `cache_path` support with hvigor-compatible invalidation keys;
6. release optimization and obfuscation/minification interaction;
7. widget/card-specific compile pass.

The current `arkdown-abc` skeleton is useful, but full hvigor alignment is a
large compiler project. Treat it as the longest pole in the plan.

### Phase 6: Release features and packaging cleanup

- Wire `arkdown-obfuscate` into the CLI between bundling and ABC compilation.
- Invalidate code/ABC caches when obfuscation rules or SDK compile env change.
- Fix `pkgContextInfo.json`, `pack.info`, source map packaging, native libs,
  integrated HSP, privacy profile, startup/share config, and router map output.
- Add HSP/HAR-specific flows instead of routing HSP through the same HAP path
  with only archive extension changes.

## Suggested Milestones

1. **Minimal hvigor-compatible HAP:** exact `loader.json`, `module.json`,
   `pkgContextInfo.json`, package layout, and native ABC parity.
2. **Real ArkUI page support:** full ArkTS transform for common `@Entry`
   pages and source maps.
3. **Dependency parity:** local/remote HAR, HSP, ByteCode HAR, dynamic imports,
   importee caches.
4. **Release parity:** obfuscation, minification, tree-shaking, cache
   invalidation, source-map packaging.
5. **Pure Rust compiler parity:** replace the SDK ABC backend for all supported
   language constructs after golden bytecode tests pass.

## Immediate Next Tasks

Priority order based on 2026-05-29 parity report:

1. **[BUG] Fix absolute path in modules.abc** — `pages/Index` absolute filesystem path
   leaks into StringItems in `arkdown-abc`. Locate the source file path derivation and
   replace with the OHM URL or relative path.

2. **[module.json] Add `virtualMachine`, `compileMode`, `dependencies`** — In
   `arkdown-profile`'s `process_profile` step, inject `virtualMachine: "ark12.0.6.0"` from
   SDK version metadata, always write `compileMode: "esmodule"` for debug HAP, and add
   `dependencies: []` for modules with no explicit deps.

3. **[module.json] Write resource IDs from restool into profile** — `arkdown-resource`
   needs to pass resolved integer IDs (`iconId`, `labelId`, `descriptionId`, etc.) back to
   `arkdown-profile` so they appear in the merged module.json.

4. **[modules.abc] Fix `func_main_0` OHM prefix** — Strip the module OHM path from
   function NameItems. Only the module record NameItem should carry the OHM path.

5. **[modules.abc] Deduplicate annotation strings** — `pkgName@entry`, `isCommonjs`,
   `hasTopLevelAwait`, `isSharedModule`, `scopeNames`, `moduleRecordIdx` are being
   emitted once per chunk; consolidate into a single string table entry shared across records.

6. Extend the golden parity harness from minimal-hap to minimal-hsp, minimal-har,
   and dependency fixtures.
7. Wire `cache_path` and source map plumbing through CLI -> bundler -> ABC.
8. Close remaining `arkdown-abc` LNP density gap (per-instruction ADVANCE sequences).
