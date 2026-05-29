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
- Phase 3 (bundler): ✅ clean ESM, @ohos: external, treeshake off for debug / on for release, manualChunks for supportChunks (commons + vendors groups)
- Phase 4 (importee): ✅ process_importees() ports hvigor process-importee.js
- Phase 5 (ABC): 🔄 per-instruction LNP, loops, throw/break/continue, conditional/template, optional chaining, arrays, destructuring, for-in/for-of stubs; test fixture 6881B/9844B; real-app baseline 8565B vs 9844B
- Phase 6 (packaging): ✅ HAP zip identical to hvigor (11 entries)
- Phase 7 (ByteCode HAR): ✅ detect_bytecode_har(), scan_bytecode_har_deps(), byteCodeHarInfo in loader.json, compiledImportees OHM URL, merge_bytecode_har_abc(); fixture + 2 e2e tests

**Parity (minimal-hap, 2026-05-29):**
- HAP zip: ✅ identical (11 entries, all content matches except 2 files below)
- `ets/modules.abc`: hvigor 9844B. Per-instruction LNP ADVANCE sequences now implemented:
  `push()` emits one LNP ADVANCE entry per bytecode instruction (SPECIAL_OPCODE 1 byte each);
  `emit_expr()` updates `current_line` per sub-expression for accurate multi-line tracking;
  `SET_EPILOGUE_BEGIN (0x09)` emitted before every `return`/`returnundefined` instruction.
  Test fixture (simplified sources, debug_info=true): 6881B/9844B = 70% (+211B vs statement-only).
  Real-app delta scales with instruction count (larger sources → larger LNP gain).
  ✅ Literal arrays=10=[4,4,6,6,6,6,16,24,26,26] match hvigor exactly. ✅ Method records=20 match.
  Remaining gap: 225B non-LNP structural differences.
- `ets/sourceMaps.map`: hvigor 1847B, arkdown 1281B (pre-struct-fix). 🔄 **Fix applied (2026-05-29)**: struct-aware ETS→ViewPU position mapping implemented. `lower_build_body` now returns `(String, Vec<SpanHint>)` where each `SpanHint` records the body-line offset and original `.ets` `Span` of the ArkUI component expression. `transform_ets_viewpu_with_source_map` converts span byte offsets to `(src_line, src_col)` via `offset_to_line_col` and emits a Source Map v3 JSON with one segment per `observeComponentCreation2` call. The map's generated-side positions reference the ViewPU output; the original-side positions reference the `.ets` component call lines. Non-struct .ets (EntryAbility) continues to get dense per-token maps via `generate_code_and_source_map`. Remaining gap: hvigor includes a `names` array and denser per-token coverage for the ViewPU boilerplate lines.

## Major Missing Capabilities

| Area                     | Current implementation                                                                                                                                                                                                                                                                                                                      | Gap to hvigor                                                                                                                                                                             |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loader.json`            | ✅ **Fixed (2026-05-29)**: absolute paths via `canonicalize()`, all 18 fields match hvigor golden exactly. `nonEntryImportees`/`compiledImportees` populated by Phase 4 importee pass. | ByteCode HAR metadata, workers, HSP/HAR OHM maps — not needed for minimal-hap fixture. |
| ArkTS transform          | ✅ **Fixed (2026-05-29)**: all decorators + `@Builder`/`@Styles`. ✅ struct-aware ETS→ViewPU source maps via `SpanHint` + `source_map::build_ets_source_map`. | `@Builder` body full ArkUI desugaring; `names` array in source maps. |
| Rollup/rolldown options  | ✅ **Fixed (2026-05-29)**: clean ESM, treeshake off/on, `manualChunks` commons+vendors, IIFE/CJS removed. | ByteCode HAR/HSP externals in rolldown. |
| ByteCode HAR importees   | ✅ **Fixed (2026-05-29)**: `process_importees()`, `detect_bytecode_har()`, `scan_bytecode_har_deps()`, `compiledImportees`+`byteCodeHarInfo`, `merge_bytecode_har_abc()`. | Full Panda ClassIndex merge (append-only now); ByteCode HAR externals in rolldown. |
| ABC compiler             | 🔄 **Improved**: per-instruction LNP, `SET_EPILOGUE_BEGIN/PROLOGUE_END`, `START_LOCAL_EXTENDED`, all common ops. ✅ **Phase 5a**: loops, break/continue, conditional, template literals, throw. ✅ **Phase 5b**: optional chaining (`?.`), array literals, array/object destructuring, `for-in`/`for-of` stubs. | 225B structural gap; `for-in`/`for-of` full iteration opcodes; try/catch/finally, async/await, generators. |
| Obfuscation/minification | ✅ **Wired (2026-05-29)**: release-build obfuscation, rule hash in cache invalidation. | Terser minification parity pending. |
| Incremental cache        | ✅ **Wired (2026-05-29)**: FNV-1a content hash + config hash, ABC cache skip on hit, `cache_file` path wired. | Rollup msgpack graph-level cache not yet wired. |
| Source maps/debug        | ✅ **Fixed (2026-05-29)**: per-token maps for non-struct .ets/ts; struct-aware ETS→ViewPU via `SpanHint` VLQ for @Component files. | `names` array; per-token ViewPU boilerplate coverage. |
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

Status: ✅ complete — minimal-hap, minimal-har, minimal-hsp fixtures all covered.

Coverage added (2026-05-29):
- `crates/arkdown-cli/tests/hvigor_parity.rs`: `minimal_hsp_hvigor_parity_report` added
  alongside existing `minimal_hap` and `minimal_har` ignored tests; all three run with
  local hvigorw and write structured diff JSON to `target/parity/`.
- `crates/arkdown-cli/tests/e2e_test.rs`: non-ignored tests for HAR and HSP:
  - HAR: `test_har_exists_and_is_valid_zip`, `test_har_contains_required_entries`,
    `test_har_golden_entry_list` — verifies source HAR has `oh-package.json5` +
    `src/main/ets/` entries and NO `module.json`, `pack.info`, or `.abc` files.
  - HSP: `test_hsp_exists_and_is_valid_zip`, `test_hsp_contains_required_entries`,
    `test_hsp_abc_has_panda_magic`, `test_hsp_golden_entry_list` — verifies HSP has
    `module.json`, `pack.info`, `resources.index`, `ets/modules.abc` (PANDA magic).
  - ByteCode HAR importee: `test_non_entry_importees_populated_with_har_dep` —
    constructs a synthetic `ChunkGraph` with an entry chunk importing a utility
    chunk (simulating a HAR dep), calls `process_importees()`, and asserts that
    `nonEntryImportees` is populated with `@bundle:` OHM URL entries.
- `tests/fixtures/golden/minimal-har/expected-zip-entries.json` — required/forbidden
  entry lists for the HAR golden check.
- `tests/fixtures/golden/minimal-hsp/expected-zip-entries.json` — required/forbidden
  entry lists for the HSP golden check.

The parity harness tests (hvigor_parity.rs) still require a local DevEco toolchain
and are `#[ignore]`; run them with:
  `cargo test -p arkdown-cli -- --ignored minimal_hsp_hvigor_parity_report`

The e2e tests run without any external toolchain and are part of the default
`cargo test -p arkdown-cli` run.

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

✅ **Completed (2026-05-29)**:

- Release treeshake (`TreeshakeOptions::Boolean(true)`) and debug no-treeshake.
- `manualChunks` via `ManualCodeSplittingOptions` for `supportChunks=true`:
  - Group `commons`: Regex `oh_modules|node_modules`
  - Group `vendors`: dynamic name function — returns `"vendors"` for non-entry modules
- Chunk filenames/stems preserved via `InputItem.name` from file stem.
- Source maps enabled (`SourceMapType::Hidden`) with `sourceMaps.map` written.

Remaining:
- External handling for SDK modules, HSP, ByteCode HAR, declarations, native `.so`,
  and deduplicated HAR modules.
- Custom compile plugin compatibility path (after core parity).

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

**Partially complete (2026-05-29):**

- ✅ Wire `arkdown-obfuscate` into the CLI between bundling and ABC compilation.
  Runs for Release builds when `obfuscation.enable_obfuscation=true`.
- ✅ Invalidate ABC cache when obfuscation rules or SDK compile env change.
  `compute_config_hash()` mixes build mode + bundle name + API version +
  obfuscation rule FNV hash into a single `u64` config key.
- ✅ Wire `arkdown-cache` incremental ABC skip: if all compile-entry content
  hashes match and `modules.abc` exists, ABC compilation is skipped entirely.
- ✅ Pass `cache_file` to `compile_to_abc` (`build/default/cache/abc/modules.abc.cache`).
- ✅ Integration tests in `crates/arkdown-cli/tests/cache_test.rs`:
  `test_first_build_writes_cache_entries`, `test_first_build_writes_abc_cache_file`,
  `test_two_builds_produce_identical_abc`.
- TODO: Fix `pkgContextInfo.json`, `pack.info`, source map packaging, native libs,
  integrated HSP, privacy profile, startup/share config, and router map output.
- TODO: Add HSP/HAR-specific flows instead of routing HSP through the same HAP path
  with only archive extension changes.
- TODO: Terser minification and compact/remove-log obfuscation rules inside
  `arkdown-obfuscate`.
- TODO: Wire rollup-level module-graph cache (hvigor msgpack cache) to skip
  re-bundling unchanged source trees.

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
