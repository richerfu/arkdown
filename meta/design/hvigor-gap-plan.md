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
- `arkcompiler_ets_frontend/es2panda/util/moduleHelpers.cpp`
  `CompileNpmModuleEntryList()` lowers each `npmEntries.txt` line into a
  standalone pandasm record with one `u8=false` field.
- `arkcompiler_ets_frontend/es2panda/compiler/core/emitter/emitter.cpp` assigns
  per-instruction source line/column. Return instructions use function or block
  end positions.
- `arkcompiler_runtime_core/static_core/assembler/assembly-emitter.cpp`
  `BuildLineNumberProgram()` emits line and column events from the pandasm
  instruction stream, with real bytecode sizes and local-variable events.
- `arkcompiler_runtime_core/libabckit/src/codegen/codegen_dynamic.cpp` shows
  generated/inserted instructions inherit the nearest previous emitted source
  position when no explicit source position is attached.

## Current arkdown Status (2026-06-01)

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
- Phase 3 (bundler): rolldown output is still produced for loader/package side
  effects; final ABC input now follows hvigor's CompileArkTS cache-TS route.
- Phase 4 (CompileArkTS handoff): `sourceMaps.json` and `npmEntries.txt` match hvigor;
  `filesInfo.txt` and `modules.cache` differ only by absolute project root paths.
- Phase 5 (ABC): `ets/modules.abc` is byte-identical to hvigor for the minimal
  fixture.
- Phase 6 (packaging): unsigned HAP entry order and all entries are
  byte-identical.

Current byte-level parity for the local minimal-hap fixture:

| Artifact | hvigor | arkdown | Status |
|---|---:|---:|---|
| unsigned HAP entry order | 11 entries | 11 entries | identical |
| HAP entries | n/a | n/a | byte-identical |
| `ets/modules.abc` | 9844B | 9844B | byte-identical |
| ABC method CodeItems | n/a | n/a | byte-identical for all compared methods |
| ABC DebugInfo/LNP/column program | n/a | n/a | byte-identical for all compared methods |

The earlier 7174B/9844B, 7525B/9844B, 8675B/9844B, 9700B/9844B,
9727B/9844B, and 9842B/9844B reports are obsolete. Those gaps covered
already-fixed structural issues, the old chunk-source ABC route, or the
now-removed source-map-driven LNP route. The current minimal-hap ABC and
unsigned HAP are byte-identical to hvigor.

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

For the minimal fixture, the generated cache TypeScript bytes match hvigor.
Compiling those cache TS files through `arkdown-abc` now emits a 9844B
`modules.abc` that is byte-identical to hvigor, and the CLI uses this route for
final ABC generation. The fixes came directly from the upstream lowering path:
`ClassDeclaration::Compile` / `ClassDefinition::Compile` scoped class temporaries,
`ExportDefaultDeclaration` records named default classes under the class local
name, top-level block traversal collects call-argument arrows, and
`FunctionEmitter::GenInstructionDebugInfo` / `CompileFunctionParameterDeclaration`
use 0-based source lines plus per-node source positions for default parameters,
`typeof`/unary logical-not, binary expressions, condition comparisons, member
receivers/writes, `new` expressions, class missing-property scope/local events,
class-method return end positions, `super(...)` post-call instructions, implicit
return `INVALID_SOURCE_LOCATION` (`u32::MAX` line/column) before generated
`ldundefined`, right-operand `undefined` source attribution in condition
comparisons, lexical-bound `this` omission for methods whose nested arrows
capture `this`, hidden `newlexenvwithname` prologue filtering, derived
constructor final super-call validation source positions, and top-level
call-argument arrow return positions. The current source-position pass also
aligns class declaration/class export stores, variable declarator stores, call
argument stores, getter/setter property-key stores, class local closeout,
module-entry implicit return source reset, and no-argument `return;` lowering to
the AST nodes used by upstream. These match
`FunctionBuilder::ImplicitReturn()`, `Condition::Compile()`,
`Identifier::Compile()`, `GenScopeVariableInfo()`, and
`CheckIfSuperCorrectCallBeforeReturn()` in upstream es2panda. Final byte parity
also required upstream `LineNumberProgramItem::EmitSpecialOpcode()` semantics
for pc deltas, hvigor's 4-byte-aligned trailing LNP index, sorted parameter-name
string insertion, and MethodItem tag order `CODE -> SOURCE_LANG -> DEBUG_INFO
-> ANNOTATION -> NOTHING`.

Source-map routing was also checked against this state. Using rolldown chunk
source maps for ABC debug info regressed the minimal fixture to 9679B, and the
old CompileArkTS source-map-backed route produced 9727B. Both were incorrect:
upstream `gen_merged_abc.ts` passes `filesInfo.txt` and `npmEntries.txt` to
`es2abc`, not a source-map argument for LNP generation. `arkdown-abc` now accepts
source maps only for API compatibility; `source_file` is passed separately from
`filesInfo`-style metadata, and LNP coordinates come from parsed source spans.
The old chunk source-derived route emitted 9700B while preserving CodeItems, but
it is no longer the final ABC route. The direct cache-TS route is now the default
and matches hvigor byte-for-byte for the minimal fixture.

## Remaining Minimal-HAP Gap

There is no remaining `modules.abc` or unsigned HAP packaging gap for the
minimal fixture. The unsigned archive has the same 11 entries, in the same
order, with byte-identical payloads. Signed HAP output may be reordered by the
signing layer and is not the unsigned packaging oracle.

## HmosWorld Evidence (2026-06-01)

HmosWorld is tracked from the GitHub mirror `https://github.com/susns/HmosWorld`.
The current local comparison uses the `phone` HAP module with dependencies installed
by `ohpm`.

The CompileArkTS resource-lowering path now follows SDK `ets-loader` source instead
of decompiled ABC inference:

- app resources are read from `arkdown-resource` as `app.<type>.<name>`;
- SDK system resources are parsed with OXC AST from `ets-loader/sysResource.js`,
  matching hvigor's `initSystemResource()` load of `resources.sys`;
- `$r(...)` and `$rawfile(...)` are rewritten by AST call-expression matching, not
  source-string scanning;
- the post-transform HmosWorld CompileArkTS cache contains no remaining
  `$r` or `$rawfile` calls, matching the hvigor cache on that check.

The CompileArkTS graph handoff now also follows the upstream TypeScript
transform path:

- imported const-enum member reads are lowered from OXC AST using the upstream
  `getTypeExportImportAndConstEnumTransformer` model, then OXC's TypeScript
  transformer strips types for files whose hvigor cache output is `.js`;
- const-enum-only dependency modules no longer remain in arkdown's Rollup module
  graph after they become type-only imports.

Current HmosWorld artifact status after a clean arkdown rebuild:

| Artifact | hvigor | arkdown | Status |
|---|---:|---:|---|
| residual `$r` / `$rawfile` in cache TS | 0 | 0 | identical on this check |
| `filesInfo.txt` record set | 571 | 571 | identical set; order differs at line 3 |
| `modules.cache` entries | 572 | 572 | identical set; order differs at line 3 |
| `npmEntries.txt` entries | 57 | 57 | identical set; order differs at line 1 |
| cache TS content hashes | 571 files | 571 files | 81 exact; 64 whitespace/comment-only; 426 semantic-like diffs |
| `ets/modules.abc` | 2,875,016B | 3,921,096B | not aligned |
| `ets/widgets.abc` | 28,424B | 38,936B | not aligned |
| ABC file version | `0x0002000c` | `0x0002000c` | aligned |

The ABC byte delta is therefore not caused by missing app/sys resource objects
in the generated TypeScript cache or by missing CompileArkTS handoff modules.
`arkdown-abc` now also lowers `npmEntries.txt` using the upstream
`CompileNpmModuleEntryList()` record shape, wraps pandasm record names as
`L<record>;`, de-duplicates fixed SDK stubs when real npm-entry records provide
the same SDK namespace classes, and collects function/arrow records with the
upstream `Binder::LegacyBuildFunction()` FNV-1a naming model. The ETS transform
now follows upstream `process_component_build.js` for `ListItem` lazy create
blocks, `Button.createWithLabel/createWithChild`, builder parent-context
framework calls, and `filterRegularAttrNode()` literal/enum attribute hoisting
into `if (isInitialRender)`. The regular-attribute split uses OXC AST nodes and
SDK enum declarations instead of source-string matching. Component member
lowering follows upstream `process_component_member.js` for direct `@Prop`
`updateStateVars().reset(params.x)`, and upstream `process_component_class.js`
for `LocalStorageProp` class field initializers, reusable-component
`updateRecycleElmtId()` generation, and `processBuildMember()` /
`isNeedGetRawObject()` wrapping of non-simple observed state fields passed as
call arguments. The raw-object wrapping is driven by OXC AST call/property nodes
and the same simple-type classification used for state fields; direct object
literal params remain unchanged, matching upstream. Custom component constructor
params also follow upstream `createReference()` for child `@Link` fields:
constructor params receive the parent backing field (`this.__x`), while
`paramsLambda` keeps the public getter (`this.x`) and `updateStateVars` skips
`@Link`. Remaining cache-source gaps are still broad, with known
printer/comment/format gaps and additional ViewPU custom-component/paramsLambda
forms still to audit from upstream source.

The struct-transform source map offset is also corrected after final ViewPU
output normalization. `finalizeConstruction` shim insertion and interface
hoisting can move generated lines; mappings now locate the final
`class ... extends ViewPU` line instead of reusing the pre-finalization offset.

Rollup/Rolldown order note: hvigor's SDK Rollup plugin context exposes
`getModuleIds()` as `graph.modulesById.keys()` over a JavaScript `Map`; Rolldown
1.0.3 exposes `get_module_ids()` from a `DashMap` iterator. Arkdown therefore
tracks Rolldown `ModuleIdx` as the closest currently available stable graph
order, but exact hvigor insertion order is still not proven. This order affects
CompileArkTS handoff files and can affect byte-for-byte ABC/HAP parity whenever
the order is consumed by `filesInfo.txt`, `modules.cache`, `npmEntries.txt`, or
record/source-map metadata. If the same module set is compiled with an order
that does not enter those final compiler inputs, it is only an intermediate
cache ordering difference and can be ignored for runtime behavior.

## Major Remaining Capabilities Beyond Minimal-HAP

| Area | Status | Remaining work |
|---|---|---|
| ABC DebugInfo/LNP | minimal fixture aligned | Broaden golden coverage beyond minimal-hap. |
| Direct cache-TS ABC route | minimal fixture aligned; HmosWorld handoff and npm-entry records wired | Keep as final ABC route; implement hvigor `modules.cache`/protoBin ProgramCache merge semantics. |
| ArkTS transform breadth | partial | Broaden beyond minimal fixture: remaining custom-component, `@ComponentV2`, paramsLambda, printer/comment, and diagnostic forms. Resource lowering, imported const-enum lowering, Button creation, builder parent context, and `@Prop` updateStateVars are AST/upstream-source based for HmosWorld. |
| Release builds | partial | Obfuscation, terser/minification, release treeshake/manual chunks, release source maps. |
| Incremental cache | partial | Rollup msgpack cache, task input cache, real ABC `--cache-file` equivalent. |
| Multi-module dependencies | partial | ByteCode HAR, dynamic imports, HSP/HAR transitive maps, workers. |
| HSP/HAR packaging | partial | HAP path is parity-checked; HSP/HAR-specific outputs need more golden fixtures. |

## Immediate Next Tasks

Priority order from the current 2026-06-01 state:

1. **[HmosWorld ABC delta] Diff hvigor and arkdown CompileArkTS bytes.** The HAP
   entry list, `$r/$rawfile` cache check, `filesInfo.txt`, `modules.cache`, and
   `npmEntries.txt` now align by set and npm-entry records are lowered, but
   `modules.abc` remains larger in arkdown. Next compare transformed TS/JS bytes
   by upstream transform area, starting with custom-component recycle/update
   TypeScript printer/comment preservation and remaining custom-component
   paramsLambda formatting/ordering forms, then continue into native
   `--cache-file @modules.cache` ProgramCache/protoBin merge behavior from
   upstream C++.
2. **[Parity harness] Add a stable ABC diff helper.** It should compare
   CodeItems, DebugInfo/LNP, method records, indexes, literal arrays, and final
   bytes separately so future regressions are localized immediately.

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
