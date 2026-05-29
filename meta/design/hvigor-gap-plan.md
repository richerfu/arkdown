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
- Phase 2 (ETS transform): ✅ @Prop, @Link, @ObjectLink, @Provide, @Consume, @Watch, @BuilderParam, @Builder, @Styles
- Phase 3 (bundler): ✅ clean ESM, @ohos: external, treeshake off for debug
- Phase 4 (importee): ✅ process_importees() ports hvigor process-importee.js
- Phase 5 (ABC): 🔄 modules.abc (per-instruction LNP implemented; test fixture 6881B/9844B with debug_info=true; real-app parity improving from 87% baseline)
- Phase 6 (packaging): ✅ HAP zip identical to hvigor (11 entries)

**Parity (minimal-hap, 2026-05-29):**
- HAP zip: ✅ identical (11 entries, all content matches except 2 files below)
- `ets/modules.abc`: hvigor 9844B. Per-instruction LNP ADVANCE sequences now implemented:
  `push()` emits one LNP ADVANCE entry per bytecode instruction (SPECIAL_OPCODE 1 byte each);
  `emit_expr()` updates `current_line` per sub-expression for accurate multi-line tracking;
  `SET_EPILOGUE_BEGIN (0x09)` emitted before every `return`/`returnundefined` instruction.
  Test fixture (simplified sources, debug_info=true): 6881B/9844B = 70% (+211B vs statement-only).
  Real-app delta scales with instruction count (larger sources → larger LNP gain).
  ✅ Literal arrays=10=[4,4,6,6,6,6,16,24,26,26] match hvigor exactly. ✅ Method records=20 match.
  Remaining gap: 225B non-LNP structural differences + source-map density.
- `ets/sourceMaps.map`: hvigor 1847B, arkdown 1281B — gap is per-statement vs per-token maps

## Major Missing Capabilities

| Area                     | Current implementation                                                                                                                                                                                                                                                                                                                      | Gap to hvigor                                                                                                                                                                             |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loader.json`            | ✅ **Fixed (2026-05-29)**: absolute paths via `canonicalize()`, all 18 fields match hvigor golden exactly. `nonEntryImportees`/`compiledImportees` populated by Phase 4 importee pass. | ByteCode HAR metadata, workers, HSP/HAR OHM maps — not needed for minimal-hap fixture. |
| ArkTS transform          | ✅ **Fixed (2026-05-29)**: `@State`, `@Prop`, `@Link`, `@ObjectLink`, `@Provide`, `@Consume`, `@Watch`, `@BuilderParam`, storage decorators all implemented. ✅ **Fixed (2026-05-29)**: `@Builder` (decorator stripped, method preserved, body lowered via `lower_build_body_or_raw`), `@Styles` (decorator stripped, `private` method, `.attr(x)` → `this.attr(x)` body lowering). Per-statement source maps via oxc_codegen. | Full ArkUI desugaring in `@Builder` bodies requires OXC ETS parser to emit `ArkUIComponentExpression` for non-`build()` methods. Precise per-token source maps (gap: 566B vs hvigor). |
| Rollup/rolldown options  | ✅ **Fixed (2026-05-28)**: Clean ESM output, `@ohos:` imports external, treeshake disabled for debug. **IIFE/CJS compatibility paths removed** from both bundler and ABC compiler. | Release treeshake, `manualChunks` for `supportChunks`, ByteCode HAR/HSP externals. |
| ByteCode HAR importees   | ✅ **Fixed (2026-05-29)**: `process_importees()` ports `process-importee.js`. BFS from entry chunks, OHM URL generation, importee.json cache written. `nonEntryImportees` in loader.json. | ByteCode HAR compilation flow (requires HAR input fixture). |
| ABC compiler             | 🔄 **Improved**: Per-instruction LNP ADVANCE via `push()` hook, `emit_expr()` per-sub-expression line update, `SET_EPILOGUE_BEGIN (0x09)` before returns, `SET_PROLOGUE_END (0x08)`, `START_LOCAL_EXTENDED (0x04)`, START_LOCAL scope/param vars, SET_FILE debug info, OHM URL scope names, binary/unary/logical ops, static-object `createobjectwithbuffer`, call-argument closures, NewLexEnvWithName arrow captures, class-decl lexical bindings. | 225B structural gap (non-LNP); per-token source map granularity; remaining opcode coverage. |
| Obfuscation/minification | `arkdown-obfuscate` exists but not wired in.                                                                                                                                           | hvigor release builds run SDK obfuscation + terser before ABC.                          |
| Incremental cache        | `arkdown-cache` exists but not wired in.                                                                                                                                               | hvigor rollup msgpack cache + es2abc `--cache-file` support.                            |
| Source maps/debug        | ✅ Per-statement source maps from kit-rewritten ETS source (pre-struct-transform). Pretty-printed JSON. sourceMaps.map 1281B vs 1847B golden (gap: per-statement vs per-token VLQ).   | Per-token source map granularity to match tsc/ets-loader output.                        |
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

1. Extend the golden parity harness from the minimal HAP to native implementation,
   HSP, HAR, dependency, and release fixtures.
2. Fix `pkgContextInfo.json` and output path layout; these are high-impact and
   low-risk.
3. Populate missing `loader.json` dependency fields.
4. Wire `cache_path` and source map plumbing through CLI -> bundler -> ABC.
5. Close `arkdown-abc` bytecode differences using the golden bytecode reports.
