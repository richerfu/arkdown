# hvigor Alignment Gap Analysis and Plan

This document records the current gap between `arkdown` and the locally installed
hvigor pipeline, with emphasis on the ArkTS -> ABC path. It is based on local
hvigor output plus upstream OpenHarmony source under `/private/tmp/arkdown-upstream`.

## Local hvigor Evidence

The local DevEco command-line-tools installation uses:

- packages: `@ohos/hvigor-arkts-base@6.21.1`,
  `@ohos/hvigor-arkts-compose@6.21.1`
- bundler: `rollup@3.19.1-htrunk1.AllScenario.devecostudio.r8`
- SDK compiler: `ets-loader/bin/ark/build-mac/bin/es2abc`

Important local hvigor package files:

- `@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
- `@ohos/hvigor-arkts-base/dist/src/build.js`
- `@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js`
- `@ohos/hvigor-arkts-base/dist/src/build-steps/process-importee.js`

Important upstream OpenHarmony refs:

- `developtools_ace_ets2bundle/compiler/src/fast_build/ets_ui/rollup-plugin-ets-typescript.ts`
  runs TypeScript Program emit with `interopTransform`, `processUISyntax`,
  `expandAllImportPaths`, `processKitImport`, and obfuscation name collection.
- `developtools_ace_ets2bundle/compiler/src/gen_merged_abc.ts` writes
  `filesInfo.txt`, `npmEntries.txt`, and calls `es2abc @filesInfo --npm-module-entry-list`.
- `arkcompiler_ets_frontend/es2panda/compiler/core/emitter/emitter.cpp` assigns
  per-instruction source line/column. Return instructions use function or block
  end positions.
- `arkcompiler_runtime_core/static_core/assembler/assembly-emitter.cpp`
  `BuildLineNumberProgram()` emits line and column events from the pandasm
  instruction stream, with real bytecode sizes and local-variable events.
- `arkcompiler_runtime_core/libabckit/src/codegen/codegen_dynamic.cpp` shows
  generated/inserted instructions inherit the nearest previous emitted source
  position when no explicit source position is attached.

## Current arkdown Status (2026-05-31)

`arkdown-cli` performs the full minimal HAP pipeline:

1. parse config;
2. compile resources and process profiles;
3. run the Rust ArkTS transform and bundle via rolldown;
4. write hvigor-style CompileArkTS cache files and ABC handoff metadata;
5. run post-bundle importee processing;
6. compile ABC via `arkdown-abc`;
7. package and sign the HAP.

Phase status for `tests/fixtures/minimal-hap`:

- Phase 0 (parity harness): implemented for minimal-hap/minimal-har/minimal-hsp.
- Phase 1 (loader/profile/resource): minimal-hap fields match hvigor.
- Phase 2 (ArkTS transform): minimal fixture transformed TS and source maps match the
  hvigor fixture for the exercised code.
- Phase 3 (bundler): output shape is close enough that all ABC CodeItems now match
  hvigor when compiling the current chunk inputs.
- Phase 4 (CompileArkTS handoff): `sourceMaps.json` and `npmEntries.txt` match hvigor;
  `filesInfo.txt` and `modules.cache` differ only by absolute project root paths.
- Phase 5 (ABC): remaining final HAP byte difference is only `ets/modules.abc`.
- Phase 6 (packaging): unsigned HAP entry order and all non-ABC entries are byte-identical.

Current byte-level parity for the local minimal-hap fixture:

| Artifact | hvigor | arkdown | Status |
|---|---:|---:|---|
| unsigned HAP entry order | 11 entries | 11 entries | identical |
| non-ABC HAP entries | n/a | n/a | byte-identical |
| `ets/modules.abc` | 9844B | 9700B | different |
| ABC method CodeItems | n/a | n/a | byte-identical for all compared methods |
| ABC DebugInfo/LNP/column program | n/a | n/a | remaining gap |

The earlier 7174B/9844B, 7525B/9844B, 8675B/9844B, and 9727B/9844B reports are
obsolete. Those gaps covered already-fixed structural issues or the now-removed
source-map-driven LNP route. The current 144B gap is not method bytecode: all
23 compared CodeItems are byte-identical. The remaining delta is DebugInfo,
LineNumberProgram, column program, and local-variable event encoding.

## CompileArkTS Source Route

hvigor does not feed the final rollup chunk text directly to `es2abc` in the same
way this Rust backend currently does. The SDK CompileArkTS path produces
transformed TypeScript cache files, writes `filesInfo.txt`, then invokes `es2abc`
with that file list and `npmEntries.txt`.

`arkdown` now writes the same handoff files:

- `build/default/cache/default/default@CompileArkTS/esmodule/debug/sourceMaps.json`
- `build/default/cache/default/default@CompileArkTS/esmodule/debug/filesInfo.txt`
- `build/default/cache/default/default@CompileArkTS/esmodule/debug/npmEntries.txt`
- `build/default/cache/default/default@CompileArkTS/esmodule/debug/modules.cache`

For the minimal fixture, the generated cache TypeScript bytes match hvigor. A
temporary experiment compiling those cache TS files directly through `arkdown-abc`
now emits 9758B and has the same 23 method records and byte-identical CodeItems
as hvigor. The fixes came directly from the upstream lowering path:
`ClassDeclaration::Compile` / `ClassDefinition::Compile` scoped class temporaries,
`ExportDefaultDeclaration` records named default classes under the class local
name, top-level block traversal collects call-argument arrows, and
`FunctionEmitter::GenInstructionDebugInfo` / `CompileFunctionParameterDeclaration`
use 0-based source lines plus per-node source positions for default parameters,
unary logical-not, binary expressions, member receivers, and `super(...)`
post-call instructions. The direct route still produces a smaller file overall,
so the remaining shrink is now in non-CodeItem ABC sections such as DebugInfo/LNP,
annotations, indexes, or literal metadata rather than executable bytecode.

Source-map routing was also checked against this state. Using rolldown chunk
source maps for ABC debug info regressed the minimal fixture to 9679B, and the
old CompileArkTS source-map-backed route produced 9727B. Both were incorrect:
upstream `gen_merged_abc.ts` passes `filesInfo.txt` and `npmEntries.txt` to
`es2abc`, not a source-map argument for LNP generation. `arkdown-abc` now accepts
source maps only for API compatibility; `source_file` is passed separately from
`filesInfo`-style metadata, and LNP coordinates come from parsed source spans.
This source-derived route currently emits 9700B while preserving all CodeItems.
The direct cache-TS route also preserves CodeItems now, but it is 9758B and still
needs section-level parity before becoming the final default path.

## Remaining ABC Gap

The remaining gap is in source-position metadata, not executable bytecode.

Observed LNP differences:

- Some generated methods still inherit incomplete AST/source positions in
  `arkdown-abc`, so their line_ops differ from hvigor. The default derived
  `EntryAbility #~@0=#EntryAbility` constructor is aligned; top-level closures
  such as `Index #*#` remain.
- `Index func_main_0` has identical CodeItem bytes but different line/column event
  density and column values. hvigor records source positions from es2panda's AST
  nodes and from generated instruction inheritance; `arkdown-abc` currently uses
  a smaller set of `set_debug_pos()` calls.
- Normal class methods show the same pattern: code bytes match, but the line and
  column coordinates come from a mixed chunk/cache coordinate system. For
  example, kept-for-inspection chunk JS had `EntryAbility.onBackground` on
  chunk lines 22-23 while the CompileArkTS cache TS has it on lines 24-25, and
  hvigor records the cache-TS coordinates.
- `sourceMaps.json` and final `ets/sourceMaps.map` are already byte-identical
  for the minimal fixture, so they are no longer considered the active ABC LNP
  input. They still matter for debugger remapping after ABC parity is achieved.
- The LNP algorithm shape is now close to upstream, but the input event stream is
  not yet identical. The next work is to align event generation, not package
  layout or method bytecode.

Required implementation work:

1. Extend the synthesized constructor source-position fix beyond the default
   derived constructor path when field initializers or explicit constructor
   bodies are present.
2. Make expression-body arrows and top-level call-argument closures use the arrow
   AST range for the first body instruction and function end range for return.
3. Make inserted instructions inherit the nearest previous emitted source position.
4. Keep `BuildLineNumberProgram` semantics based on real bytecode sizes, column
   changes, and local-variable start/end events.
5. Re-run the minimal-hap comparison and require: only `ets/modules.abc` differs
   until DebugInfo reaches byte parity; CodeItems must remain byte-identical.

## Major Remaining Capabilities Beyond Minimal-HAP

| Area | Status | Remaining work |
|---|---|---|
| ABC DebugInfo/LNP | partial | Align source-position event stream with es2panda/emitter.cpp. |
| Direct cache-TS ABC route | partial | CodeItems match; align remaining non-CodeItem sections before switching final generation to cache TS. |
| ArkTS transform breadth | partial | Broaden beyond minimal fixture: `@Builder`, `@Styles`, more ArkUI DSL forms, diagnostics. |
| Release builds | partial | Obfuscation, terser/minification, release treeshake/manual chunks, release source maps. |
| Incremental cache | partial | Rollup msgpack cache, task input cache, real ABC `--cache-file` equivalent. |
| Multi-module dependencies | partial | ByteCode HAR, dynamic imports, HSP/HAR transitive maps, workers. |
| HSP/HAR packaging | partial | HAP path is parity-checked; HSP/HAR-specific outputs need more golden fixtures. |

## Immediate Next Tasks

Priority order from the current 2026-05-31 state:

1. **[ABC DebugInfo] Align top-level closure source positions.** Start with
   `Index #*#` and `Index #*#^1`, using arrow AST start/end behavior from
   `emitter.cpp`.
2. **[ABC DebugInfo] Align `Index func_main_0` column stream.** CodeItem bytes
   already match; only LNP body and DebugInfo constant_pool differ.
3. **[ABC DebugInfo] Generalize constructor source-position events.** The default
   derived constructor now matches the minimal fixture; explicit constructors
   and field initializer variants still need golden coverage.
4. **[ABC Direct TS] Diff non-CodeItem sections on the cache-TS route.** The
   source TS is already equal to hvigor and all direct-route CodeItems match; the
   remaining 86B shrink must be isolated to DebugInfo/LNP, annotations, indexes,
   literal arrays, or other metadata.
5. **[Parity harness] Add an ABC diff helper that compares CodeItems separately
   from DebugInfo/LNP.** This prevents future regressions from being hidden by
   total file-size changes.

## SPEC Coverage Map

| Gap area | SPEC |
|---|---|
| ABC DebugInfo/LNP event stream and direct cache-TS parity | SPEC-028 |
| ViewPU ArkUI `build()` lowering | SPEC-029 |
| `@Builder` / `@Styles` decorator transform | SPEC-030 |
| apiTransform `@kit.*` expansion | SPEC-027 |
| Release obfuscation pipeline | SPEC-038 |
| Rolldown compileEnv-keyed msgpack cache | SPEC-039 |
| ABC per-function incremental cache | SPEC-025 |
| Release tree-shake + manualChunks | SPEC-034 |
| ByteCode HAR dependency map in loader.json | SPEC-041 |
| Dynamic import library info in loader.json | SPEC-042 |
| Preloaded SO file list | SPEC-043 |
| Integrated HSP processing | SPEC-044 |
| Privacy profile embedding | SPEC-045 |
| CrossAppSharedConfig + StartupTask merging | SPEC-046 |
| ArkData compile + ETS standalone checker | SPEC-037 |
| Task-level incremental skip | SPEC-040 |
