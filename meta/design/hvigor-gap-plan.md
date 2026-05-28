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

## Current arkdown Status

`arkdown-cli` currently performs a simplified fixed pipeline:

1. parse config;
2. compile resources and process profiles;
3. bundle each module through rolldown (ESM output, SDK imports external, treeshake
   disabled for debug);
4. compile written `.js` chunks through `arkdown-abc`;
5. package and sign HAP/HSP or package source HAR.

As of 2026-05-28: the pipeline produces correct ESM output for both entry files
(`EntryAbility.ets` → clean ESM with `import UIAbility from "@ohos:..."` syntax,
`Index.ets` → ViewPU class with `registerNamedRoute` side-effect call). Both are
compiled to ABC. HAP zip structure matches hvigor exactly (11 entries).
The remaining diff is only in `ets/modules.abc` (content mismatch) and
`ets/sourceMaps.map` (different source positions from different transform output).
The root cause is `arkdown-abc` generating incomplete/different bytecode vs `es2abc`.

## Major Missing Capabilities

| Area                     | Current implementation                                                                                                                                        | Gap to hvigor                                                                                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loader.json`            | Writes core fields, current module entry list, empty maps for many dependency fields.                                                                         | Must populate workers, router map, dynamic imports, ByteCode HAR metadata, HSP/HAR OHM maps, `otherCompileFiles`, dependency package brief info, and exact hvigor path layout.  |
| ArkTS transform          | Text-based `struct` to `class` rewrite; `build()` bodies are stubbed with `return;`.                                                                          | Must implement real `ets-loader` semantics: ArkUI DSL lowering, state/property decorators, `@Builder`, `@Entry`, checker diagnostics, SDK API transforms, and source maps.      |
| Rollup/rolldown options  | **Fixed (2026-05-28)**: @ohos: / @ohos/ / @system. / @native: imports are now marked external so rolldown emits clean ESM `import` statements instead of CJS interop. Tree-shaking disabled for debug builds so side-effect-only `@Entry` pages (no exports) are included. Output is `OutputFormat::Esm` with hidden source maps. | Must still match hvigor `generateConfig()` for: `manualChunks`, release treeshake/minification, chunk filenames, custom compile plugins, `supportChunks`, and ByteCode HAR/HSP externals. |
| ByteCode HAR importees   | `rewrite_bytecode_har_imports()` is a no-op.                                                                                                                  | Must implement `processImportees()` equivalent and write `nonEntryImportees`, `compiledImportees`, and direct importee caches.                                                  |
| ABC compiler             | Emits Panda file skeleton plus partial bytecode for simple JS constructs. `debug_info`, `opt_level`, `cache_path`, and source maps are mostly metadata flags. | Must match `es2abc`: full JS bytecode semantics, module annotations, literal arrays, method records, debug info, source map chain, cache, optimization, and widget compilation. |
| Obfuscation/minification | `arkdown-obfuscate` exists but is not wired into `arkdown-cli`; terser-equivalent release minification is not integrated.                                     | hvigor release builds run SDK obfuscation/consumer-rule generation and terser/minification under specific conditions before ABC generation.                                     |
| Incremental cache        | `arkdown-cache` exists but the CLI does not drive it; `arkdown-abc` does not persist function-level cache.                                                    | Must match hvigor rollup msgpack cache layout and `es2abc --cache-file` invalidation inputs: SDK/API version, module type, obfuscation state, compile env, changed modules.     |
| Source maps/debug        | ETS transform now generates per-token oxc_codegen source maps from the final transformed TypeScript and passes them through the plugin adapter (`HookTransformOutputMap::Sourcemap`). `sourceMaps.map` size moved from ~1057B to ~1921B (target ~1847B). ABC source map chain and release build source map removal are not yet implemented. | Must preserve full `.ets -> transform -> bundle -> abc` mapping chain for debug builds and suppress maps in release builds.                                                      |
| Packaging metadata       | HAP packaging exists, but `pkgContextInfo.json` currently has blank `moduleName`/`bundleName` and `entryPath: "src/main/"`.                                   | Must match hvigor runtime metadata: real bundle name, module name, dependency alias, package name, and `entryPath: "ets/modules.abc"` for stage model packages.                 |

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
