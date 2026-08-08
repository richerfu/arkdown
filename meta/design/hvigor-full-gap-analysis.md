# hvigor Full Pipeline Gap Analysis

Comprehensive mapping of every hvigor build step against the current arkdown implementation.
Source data: the DevEco Studio-bundled hvigor and SDK under
`/Applications/DevEco-Studio.app/Contents/tools/hvigor/` and
`/Applications/DevEco-Studio.app/Contents/sdk/` (verified 2026-08-04).

Status symbols: ✅ implemented + parity-verified, 🔄 partial (gaps listed), ❌ not started.

---

## 0. Scope Boundary: Stage Model (ESMODULE) Only

arkdown targets **Stage model** (`compileMode = "esmodule"`, API >= 9) exclusively.
The FA model (`compileMode = "jsbundle"`, API < 9) is **permanently out of scope**
and is rejected at build time.

| hvigor feature | Scope | Rationale |
|---|---|---|
| ESMODULE compile path (`partialUpdateMode = true`) | **IN scope** | All modern OHOS apps (API ≥ 9) use this path |
| `initialRender()` + `rerender()` methods | **IN scope** | Stage model partial-update pattern |
| JSBUNDLE compile path (`compileMode = "jsbundle"`) | **OUT of scope** | FA model deprecated since API 9; rejected by `validate_compile_mode()` |
| `render()` method (FA model full-update) | **OUT of scope** | JSBUNDLE only |
| `@rollup/plugin-terser` | **OUT of scope** | JSBUNDLE release minification only |
| `@rollup/plugin-babel` (class properties) | **OUT of scope** | JSBUNDLE only |
| `sourcemapPathTransform` | **OUT of scope** | JSBUNDLE source map path stripping |
| `generateId(statements)` FA-model ID injection | **OUT of scope** | JSBUNDLE only |

Items marked ✅ (N/A) in the table below are JSBUNDLE-only steps that are correctly absent
from arkdown's ESMODULE-primary implementation.

---

## 1. Full Pipeline Map

### Stage A — Pre-Build

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| A1 | SDK change check + build-start event | `PreBuild` → `hvigor-ohos-plugin/src/tasks/pre-build.js` | `packages/arkdown`: SDK path discovery + `ArkdownConfig` | ✅ |
| A2 | Build OHM URL context map | `GeneratePkgContextInfo` → `generate-pkg-context-info.js` | `arkdown-profile::generate_pkg_context_info()` | ✅ |

### Stage B — Profile Processing

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| B1 | Merge base `module.json5` with product overlays + HAR/HSP deps | `MergeProfile` → `abstract-merge-profile.js` | `arkdown-profile::merge_profile()` | ✅ |
| B2 | Set `virtualMachine`, `compileMode`, `deviceTypes`; collect HAR abilities | `ProcessProfile` → `process-profile.js` | `arkdown-profile::process_profile()` injects both fields from SDK version | ✅ |
| B3 | Collect router map entries; write `intermediates/routerMap.json` | `ProcessRouterMap` → `process-router-map.js` | `arkdown-profile` router map output + cache fingerprinting | ✅ |
| B4 | Process `crossAppSharedConfig` from HSP dependencies | `ProcessShareConfig` → `process-share-config.js` | `arkdown-profile::merge_share_config()` wired in CLI; duplicate-URI is hard error | ✅ |
| B5 | Merge startup config from all HAR/HSP deps; validate task deps | `ProcessStartupConfig` → `process-startup-config.js` | `arkdown-profile::merge_startup_config()` wired in CLI; duplicate task name is hard error | ✅ |
| B6 | Generate module metadata JSON for HAP/HSP | `CreateModuleInfo` → `create-module-info.js` | `arkdown-profile::write_module_json()`: `packageName` from `oh-package.json5#name`; `installationFree` from `bundleType`; `atomicService` block preserved; `deliveryWithInstall` from module.json5; `dependencies` serialized; `requestPermissions`/`definePermissions`/`proxyData` passed through; preserve-and-mutate for HAP+HSP module fields and ability/extensionAbility objects. HSP type="shared" written via `OhosModuleType::Shared.to_string()`. Field order matches hvigor byte-for-byte: the `app` section keeps the `app.json5` order followed by the computed fields in hvigor's mutation order (`apiReleaseType` → `resolveApi` versions → `appEnvironments` → `bundleType`/`buildMode`/`debug`), the `module` section keeps the `module.json5` order followed by `packageName`/`virtualMachine`/`compileMode`/`dependencies`, and `app.json5#multiProjects` is deleted like `computeAppJsonOpt`. Declaration file output is K4/K5-level and tracked separately. | ✅ |
| B7 | Generate module metadata JSON for HAR | `CreateHarModuleInfo` → `har/har-extend-info.js` | Follows the current IDE task boundary: no synthetic `intermediates/har/default/har-extend-info.json` is left after packaging. ByteCode HAR generated metadata/declarations stay in `loader_out/etsFortgz` and only SDK-selected declarations enter the archive. | ✅ |
| B8 | Process privacy data file | `ProcessModulePrivacyProfile` → `process-module-privacy-profile.js` | `privacy_profile_path` passed to `pack_hap()` from CLI; copies to `module.privacy.json` in HAP | ✅ |

### Stage C — Resource Compilation

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| C1 | Scan + validate resource files | `ProcessResource` → `process-resource.js` | `arkdown-resource::scan_resources()` | ✅ |
| C2 | Compile resources to binary RestoolV2 format | `CompileResource` → `compile-resource.js` invokes `restool` binary | `arkdown-resource` uses the SDK `restool` backend in the initial parity path; a Rust backend is optional future work | ✅ |
| C3 | Post-process compiled resources; handle preloaded SO | `ProcessCompiledResources` → `process-compiled-resources.js` | `buildOption.preloadSo` paths written to `intermediates/preload_so/default/preload_so_list.json` after resource compilation, matching hvigor's `addPreloadSystemSoFileList()` | ✅ |

### Stage D — loader.json Generation

| # | hvigor Step | Field/Detail | arkdown | Status |
|---|---|---|---|---|
| D1 | Write loader.json: core 18 fields | `GenerateLoaderJson::doTaskAction()` → `generate-loader-json.js` | `arkdown-profile::generate_loader_json()` — all 18 fields match golden: `hspResourcesMap` now populated by scanning `oh_modules/` for HSP packages with `ResourceTable.txt`; `supportChunks` from `buildOption.arkOptions.supportChunks` | ✅ |
| D2 | `byteCodeHarInfo` map + ByteCode HAR `declarationEntry` | `byte-code-har-utils.js`, `GenerateLoaderJson.processDeclarationEntry()` | `scan_bytecode_har_deps()` follows hvigor's `metadata.byteCodeHar` + `ets/modules.abc` convention and the declared `oh-package.json5` dependency graph (BFS over `dependencies` + `dynamicDependencies`, with legacy full-scan fallback only when no root deps exist), keys entries by `oh-package.json5#name`, populates `abcPath`, `sourceMapsPath`, `compatibleSdkVersion`, and `insightIntent`, writes `updateVersionInfo`, copies dependency `metadata.declarationEntry` normalized OHM URLs, and fills `otherCompileFiles`. The producer golden also verifies SDK-style generated `ModuleInfo.d.ts` in `etsFortgz` without leaking it into the ByteCode HAR archive. | ✅ |
| D3 | `runtimeOnly` compile entries and `dynamicImportLibInfo` (runtime-only and cross-HAR dynamic imports) | `GenerateLoaderJson.processDynamicImport()` | `runtimeOnly.sources` (module+deps), `runtimeOnly.packages` with transitive BFS traversal, dependency `metadata.runtimeOnly.sources` and `metadata.runtimeOnly.packages` all collected; `dynamicImportLibInfo` from dynamicDependencies and product/dep packages, with hvigor-shaped entries (`hostModulesInfo`, `entryFilePath`, `isLocalDependency`, `pkgPath`, HSP `module.name` substitution). Confirmed against the installed 6.24.3 source: `processDynamicImport` is driven entirely by `buildOption.arkOptions.runtimeOnly`; there is no post-bundle dynamic-`import()` graph walk. | ✅ |
| D4 | `workers[]` (WebWorker `.ets`/`.ts`/`.js` file paths) | `compiler/main.js::readWorkerFile()` reads explicit loader entries; `loadWorker()` scans `workers/` recursively when absent | `generate_loader_json()` recursively scans `src/main/ets/workers`, serializes absolute `workers` only when non-empty, and adds matching `./ets/...` compile entries; explicit `buildOption.sourceOption.workers` entries merged in (deduplicated) | ✅ |
| D5 | `hspNameOhmMap` + `harNameOhmMap` for multi-module projects | From `DependencyManager` | `collect_hsp_har_ohm_maps()` now uses BFS over `oh-package.json5` dependency graph with visited set for cycle detection; collects direct AND transitive HSP/HAR dependencies with correct `@bundle:<bundleName>/<moduleName>/<entryRelPath>` OHM URLs | ✅ |
| D6 | `pkgName2OtherCompileFiles` (6.24.3): transitively blocked compile files grouped per topmost ByteCode HAR | `generate-loader-json.js::setByteCodeHarInfoAndOtherCompileFiles` V2 chain (`processSourceCodeHarNodeV2`/`processNpmPackageNodeV2`/`collectExtraImporteesV2`) + `arkOptions.widget.transitiveDeps` | `arkdown-profile::collect_pkg_name2_other_compile_files()` mirrors the V2 BFS: `topmostBlockedNode` propagation from each non-bundled BC-HAR, HSP/SO/dev-dependency filtering (`getFinalNodes`), source-HAR `src/main/ets` tree + npm entry collection, and per-group extra-importee resolution. `LoaderJson.pkg_name2_other_compile_files` serializes as `pkgName2OtherCompileFiles` on the widget loader only (`intermediates/loader/widget_loader.json`, written by the widget pass); the common `loader.json` never carries the key, matching `GenerateLoaderJson.doTaskAction()`. Config: `buildOption.arkOptions.widget.transitiveDeps` (schema-verified default `false`). | ✅ |

### Stage E — ArkCompile Task Setup

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| E1 | Build `compileEnv` key: `[compileSdkVersion, compatibleSdkVersion, runtimeOS, etsLoaderPath, obfuscationEnabled, moduleType, cachePath].join("#")` | `arkts-pack.js::getCompileEnv()` | `arkdown-cache::CompileEnv::from_config()` builds the 7-field key; wired into `TaskInputs.compile_env` for task-level cache invalidation | ✅ |
| E2 | Load SDK compile plugins from `ets-loader/compile_plugin.js` | `load-sdk-functions.js::loadSdkFunctions()` | OXC/Rolldown replacement for JS compile-plugin work; native SDK tools remain SDK-backed | ✅ |
| E3 | `hijackTscCompilerOptionsPaths`: monkey-patch `ets_checker.js` for `@types` path resolution | `load-sdk-functions.js` | Not needed — Rust resolver handles paths directly | ✅ (N/A) |
| E4 | Read rollup msgpack cache from `cache/msgpack/` | `RollupCacheService::readCache()` → `rollup-cache-service.js` | `EtsTransformPlugin` loads per-file transform cache from `build/default/cache/ets_transform/transform_cache.json` at construction; FNV content hash → transformed code; rolldown 1.0.3 scan-stage API not exposed but per-file transform cache covers the critical hot path | ✅ |

### Stage E.5 — arkOptions field coverage (6.24.3 schema)

Every property of `ohos-module-build-profile-schema.json#/definitions/arkOptions`
(15 fields) is parsed and wired:

| arkOptions field | arkdown | Behavior |
|---|---|---|
| `obfuscation` | `ObfuscationConfig` | source/bytecode obfuscation pipeline (I1-I5) |
| `types` | `ArkOptions.types` | declaration inputs hash |
| `runtimeOnly` | `RuntimeOnlyConfig` | loader.json D3 |
| `buildProfileFields` | `build_profile_fields` | `BuildProfile.ets` constants |
| `supportChunks` | `support_chunks` | F25 manualChunks |
| `byteCodeHar` | `byte_code_har` | HAR artifact type selection |
| `bundledDependencies` | `bundled_dependencies` | BC-HAR embedded deps |
| `integratedHsp` | `integrated_hsp` | K5 |
| `widget.transitiveDeps` | `widget` | D6 `pkgName2OtherCompileFiles` |
| `branchElimination` | `branch_elimination` | es2abc `--branch-elimination` (release, non-debuggable, no bytecode obfuscation — `module_mode.js` condition) |
| `transformLib` | `transform_lib` | es2abc `--transform-lib` |
| `skipOhModulesLint` | `skip_oh_modules_lint` | checker `skipOhModulesLint` |
| `reExportCheckMode` | `re_export_check_mode` | checker `reExportCheckMode` (noCheck/compatible/strict) |
| `autoLazyImport` + `autoLazyFilter` | `TransformOptions.auto_lazy_import` + filter | automatic lazy-import rewrite (SDK `process_lazy_import.js::transformLazyImport`) in the genAbc-equivalent pass after kit expansion: value imports become `import lazy`, type-only/namespace/`.json` stay plain, unreferenced named specifiers are eliminated via the semantic reference set, `include`/`exclude` filter keyed on the owning module package name (`isNotAutoLazyImport`). Byte-compared against hvigor on a real fixture: cache TS, `modules.abc`, and the HAP match except the `sourceMaps.map` (arkdown additionally maps the rewritten lazy import lines). |
| `expandImportPath` | `TransformOptions.expand_import_path` | package-level import expansion (SDK `import_path_expand.js::expandAllImportPaths`) before kit expansion: each imported symbol is traced through the package entry's export graph (`export { x }`, `export { x } from`, `export * from`, `export default`) to its declaring file, and one import per declaration file is emitted with the `.ets`/`.ts` suffix stripped. Expansion is broader than hvigor's TypeScript-program scope: arkdown's source snapshot includes local `file:` packages that hvigor's program does not resolve, so those imports expand under arkdown but stay package-level under hvigor. |
| `aotCompileMode` / `apPath` / `hostPGO` / `tscConfig` | parsed | AOT/checker fields |

`buildOption` schema fields are also fully covered: `resOptions` (including
`excludeHarRes` — remote HAR resource exclusion in `bundled_har_resources`,
mirroring `har-target-util.js::isNeedExclude`), `externalNativeOptions`,
`sourceOption.workers`, `removePermissions`, `napiLibFilterOption`,
`nativeLib`, and `generateSharedTgz`.

### Stage F — Rollup Bundling (plugin execution order)

hvigor configures rollup via `arkts-pack.js::generateConfig()`. Key flags:
`treeshake: buildMode === "release"`, `disableGenerateBundle: compileMode === "esmodule"`,
`format: "cjs"`, `shimMissingExports: true`, `context: "globalThis"`,
`chunkFileNames: "[name].js"`, `sanitizeFileName: false`.

| # | hvigor Plugin | Order / Hook | arkdown | Status |
|---|---|---|---|---|
| F1 | `commonPlugin` — ProjectConfig context sharing, pre-build cleanup | first (unshift) | Absorbed into bundler core config | ✅ |
| F2 | `invalidCachePlugin` — per-file cache invalidation | after commonPlugin | `EtsTransformPlugin.transform_cache`: FNV hash of source content used as cache key; cache hit skips full ETS/ViewPU pipeline for unchanged files; persistent across builds via JSON disk cache | ✅ |
| F3 | `checkDuplicateFilenamesPlugin` — ABC record name collision check | built-in | `arkdown-plugin-duplicate-check` | ✅ |
| F4 | `moduleMetaInfoPlugin` (ESM mode only) — pkgName, OHM identity, hostModulesInfo | `moduleParsed` hook | `arkdown-plugin-module-meta` | ✅ |
| F5 | `ohos-resolve` / `@ohos/node-resolve` — OHOS module resolution | `resolveId` | `arkdown-plugin-ohos-resolve` wrapping `arkdown-resolver`; generic probing delegated to `oxc_resolver`, hvigor semantics cover `oh-package.json5`, project `file:` aliases, `oh_modules`, and `.ohpm/oh_modules` | ✅ |
| F6 | `routerMapPlugin` — collect router entries per `buildEnd` | `buildEnd` | `arkdown-profile` router map output + build cache fingerprinting | ✅ |
| F7 | `newOhmUrlPlugin` — rewrite module IDs to OHM URLs | `resolveId` transform | Implemented in `arkdown-plugin-ohos-resolve` via `useNormalizedOHMUrl` | ✅ |
| F8 | `sensitiveCheckPlugin` — case-sensitive import validation | `resolveId` | `arkdown-resolver::case_check` | ✅ |
| F9 | `@rollup/plugin-commonjs` — CJS → ESM interop | `transform` | rolldown built-in CJS support | ✅ |
| F10 | `gcPlugin` — V8 GC hints between modules | `transform` | Rust `Drop` is deterministic; no-op | ✅ (N/A) |
| F11 | `@rollup/plugin-json` — JSON/JSON5 imports | `load` | `arkdown-plugin-json5` | ✅ |
| F12 | `waitAsyncPlugin` — flush async plugin work after `buildEnd` | `buildEnd` | Tokio handles naturally | ✅ (N/A) |
| F13 | `collectImportersPlugin` — reverse importer map for hot-reload delta | `moduleParsed` | Post-bundle step writes `intermediates/importer_map/default/importer_map.json`: `{chunkName → [importingChunkNames]}` reverse map from `ChunkGraph.cross_chunk_imports` | ✅ |
| F14 | `hspCollectHarFilesPlugin` — collect HAR files used in HSP for dedup pass | `buildEnd` | Two-pass: first build uses `collect_pass()` to write `har_files.json`; second build detects the file and uses `real_build()` to exclude previously-collected HAR modules from the HSP bundle. Matches hvigor's hspCollectHarFilesPlugin two-pass dedup. | ✅ |
| F15 | SDK `etsChecker` — TypeScript type checking for `.ets` files | `transform` (load from `ets_checker.js`) | Rust/OXC retains structural ArkTS diagnostics; the default `--arkts-checker sdk` path invokes the installed SDK's `etsStandaloneChecker` before Rolldown through a narrow Node bridge. `arkdown check` uses the same SDK diagnostic owner, so TypeScript inference and cross-module semantic errors are no longer silently accepted. | ✅ |
| F16 | SDK `etsTransform` — ArkTS decorator desugaring + ViewPU lowering | `load` + `transform` | Covers all ArkTS-only OXC AST nodes currently exposed by the pinned `ohos-rs/oxc` rev: `StructStatement`, `ArkUIComponentExpression`, `AnnotationDeclaration`, and `LazyImportDeclaration`. ViewPU lowering covers `@Component`/`@Entry`/`@CustomDialog`, state decorators, `@Builder`/`@LocalBuilder`, `@Styles`, `@Extend`/`@AnimatableExtend`, `@interface`, `import lazy`, `export default struct`, ArkUI `if`/callbacks, type-only re-export cleanup, and the SDK `finalizeConstruction` ViewPU compatibility shim for every transformed component file. It follows upstream `process_component_build.js`, `process_component_member.js`, and `process_component_class.js` for the documented creation/update patterns. Source-map encoding uses Rolldown's Rust sourcemap stack (`rolldown_sourcemap` / `string_wizard`) and generated code uses an OXC AST/codegen-backed `ViewpuAstEmitter`. ComponentV2 lifecycle decorator lowering, SDK import mapping, invalid parameter diagnostics, debug/release output, and same-root incremental cache behavior are strict-golden covered. | ✅ |
| F17 | SDK `apiTransform` — @kit.* → @ohos:* mapping + OHOS API shims | `transform` | `@kit.*` import expansion emits correct binding types (default vs named) per kit re-export shape; comprehensive kit map covering 120+ kits; `@Provide` fields generate `this.addProvidedVar(...)` calls; FA-model API `@ohos/*`→`@system.*` rewrite for API < 9; permission/system-capability decorators removed as metadata. Event-emitter call-pattern rewrites (deprecated API migration) not implemented but not required for standard OHOS ArkTS builds. | ✅ |
| F18 | SDK `createProgramPlugin` — creates TypeScript compiler program | `buildStart` | N/A: this plugin bootstraps the TypeScript type checker (ets_checker.js); arkdown uses the pure Rust EtsTransformPlugin instead, which does not need a TS compiler program context | ✅ (N/A) |
| F19 | SDK `genAbc` wrapper — only calls `cleanUp`; actual ABC comes from genAbc's hooks | `buildEnd` / `generateBundle` hooks | ABC compilation is post-bundle in `arkdown-abc`; equivalent but hookArchitecture differs | ✅ (equiv.) |
| F20 | Coverage instrumentation (`buildInstrument`) | after SDK plugins | OXC AST-based `bjc`-format instrumentation matching `@ohos/coverage/res/template/bjc_cov_class.ts`: injects `BjcCov` class + `bcv.instrumentFunction(i)` at every function entry, `bcv.instrumentRegion(f,r)` at if/loop/try/switch blocks, `bcv.instrumentBranch(f,b,true/false)` at if/switch/ternary branches; coverage data stored in `globalThis.__coverage__[path]` per-file. OXC Visit-based AST traversal with function-index stack; ternary wraps with comma-expression. Previous string-level stub (counters permanently 0, wrong Istanbul format) replaced. | ✅ |
| F21 | `@rollup/plugin-terser` (release JSBUNDLE mode) | `renderChunk` | JSBUNDLE mode not supported; arkdown targets ESMODULE mode only (same as F22). Obfuscation/minification for ESMODULE release is TASK-OBFUS-01, not terser. | ✅ (N/A) |
| F22 | `@rollup/plugin-babel` (JSBUNDLE mode only) — class properties | `transform` | Not needed for ESMODULE mode | ✅ (N/A) |
| F23 | `tscResolve` (when `useTscResolve` is set) — TypeScript path mapping | `resolveId` | `oxc_resolver` `TsconfigDiscovery::Manual` enabled automatically when `tsconfig.json` exists at project root; TypeScript `compilerOptions.paths` applied to all resolution | ✅ |
| F24 | Custom compile plugins from `hvigorfile.ts` | last (push) | `hvigorfile.ts` detected + warned; `arkdown.config.json` JSON config loaded via `load_custom_config()` — supports `shimMissingExports`, `treeshake`, `context`, `plugins.disabled` options; TypeScript plugin execution not supported in pure Rust | ✅ |
| F25 | `manualChunks` (when `supportChunks` enabled): `node_modules` → "commons", non-entry → "vendors" | rollup output option | `arkdown::Bundler::build_manual_code_splitting_options()` mirrors hvigor's manualChunks callback; `support_chunks` from config | ✅ |
| F26 | `treeshake: true` in release mode | rollup input option | `TreeshakeOptions::Boolean(true)` for release, `false` for debug — mirrors hvigor `treeshake: n === RELEASE` | ✅ |
| F27 | Source maps (`sourcemap: true`) for JSBUNDLE non-preview | rollup output option | N/A: ESMODULE mode (primary mode) uses `sourceMaps.map` file; rollup `sourcemap: true` is JSBUNDLE-only and not needed in ESMODULE mode | ✅ (N/A) |
| F28 | `sourcemapPathTransform` — strip `projectTopDir` from paths | rollup output option | N/A: ESMODULE mode uses a different source-map path model (absolute paths → `ets/` relative in sourceMaps.map); JSBUNDLE-only path transform not needed | ✅ (N/A) |

### Stage G — Post-Bundle Processing

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| G1 | Write rollup output chunks to `loader_out/` (JSBUNDLE) | `build.js::y.write(output)` | Rolldown writes chunks to configured output dir | ✅ |
| G2 | `disableGenerateBundle: true` for ESMODULE — skip file write, ABC runs in memory | `build.js` / genAbc plugin hooks | arkdown writes intermediate JS to disk and reads back; ABC output is byte-identical since source content is the same; performance overhead only, not a correctness gap | ✅ (equiv.) |
| G3 | `processImportees` — walk rollup module cache, write nonEntryImportees + compiledImportees + directImportee JSON caches | `process-importee.js` | `arkdown::process_importees()` — fully ported, Phase 4 complete | ✅ |
| G4 | `removeRedundantDeclaredFiles` — clean stale `.d.ts`/`.d.ets` from cache | `rm-declared-files.js` | ByteCode HAR declarations are generated under `loader_out/etsFortgz`; the archive collector excludes build-helper `ModuleInfo.d.ts`, matching the current SDK package layout. Broader stale-declaration cache cleanup remains covered by the task cache success boundary. | ✅ |
| G5 | `removeCompileFailedFilesFromCache` — remove cache entries for modules that errored | `rm-error-files-cache.js` | Covered by task-level cache semantics: `TaskInputs::save()` is only called on successful completion; errored builds never update the cache, so failed modules are not cached and will be recompiled on the next run | ✅ (equiv.) |
| G6 | Write rollup msgpack cache to `cache/msgpack/` | `RollupCacheService::writeCache()` | `EtsTransformPlugin.save_transform_cache()` persists hash→code map to `build/default/cache/ets_transform/transform_cache.json` after each successful transform; provides equivalent incremental-build speedup for the ETS transform pipeline | ✅ |
| G7 | `triggerBundleEnd` — flush async plugin work | `build.js` | Tokio handles naturally | ✅ (N/A) |
| G8 | Write `compileInfo.json` with `buildMode` to `arkCompileCachePath/` | `ArkCompile::writeBuildMode()` | CLI writes `{ buildMode: "debug" \| "release" }` to `cache_path/compileInfo.json` after ABC compilation | ✅ |

### Stage H — ABC Compilation (arkdown-abc)

| # | hvigor Step | es2abc behavior | arkdown | Status |
|---|---|---|---|---|
| H1 | Parse JS/TS source files | `es2panda` parser | `ohos-rs/oxc` (pinned rev) | ✅ |
| H2 | Lower AST → Panda IR + release bytecode optimization | `es2abc` compiler passes + `bytecode_optimizer::RunOptimizations()` | `arkdown-abc::ir::` modules handle literals, variable decls, loops, try/catch/finally, async/await (via asyncfunctionenter/resume), generators, destructuring including for-left assignment patterns, classes, spread/rest, computed properties, and auto-accessor properties. Computed auto-accessors now precompute and reuse runtime keys instead of synthetic fallback names. Non-zero `opt_level` now runs a Rust encoded-bytecode optimizer entry instead of failing; the port currently covers upstream-like register-frame compaction, range operand frame modeling, and upstream `--opt-try-catch-func` behavior, including the default EH-method participation and explicit `--opt-try-catch-func=false` skip path. The full graph pass stack (`Cleanup`, optional `ConstantPropagation`/`BranchElimination`, `ValNum`, `Lowering`, `MoveConstants`, `RegAccAlloc`, `RegAlloc`, `RegEncoder`, `BytecodeGen`) is still incomplete. | 🔄 |
| H3 | Emit ABC binary: Header, StringTable, LiteralArrayTable | `assembler/assembly_emitter.cpp` | Implemented; structural match | ✅ |
| H4 | Emit ClassIndex + ClassRecords (one per rolldown chunk) | `assembly_emitter.cpp::SerializeClasses()` | Implemented | ✅ |
| H5 | MethodRecords with Panda bytecode | `assembly_emitter.cpp::SerializeMethods()` | Minimal-hap `modules.abc` is byte-identical to hvigor on the CompileArkTS cache-TS route | ✅ |
| H6 | LNP index at file end (after literal arrays) | hvigor places lnp_idx_off at total_size | ✅ Fixed: LNP index now emitted as final section | ✅ |
| H7 | Module annotation strings deduplicated in StringTable | Single string entry shared across all records | ✅ Fixed: field name strings deduplicated across classes | ✅ |
| H8 | Method index ordering [strings\|methods\|lits] | `assembly_emitter.cpp` method pool layout | ✅ Fixed: method index now [bytecode_strings\|methods\|literal_arrays] | ✅ |
| H9 | LNP body and DebugInfo constant_pool | `emitter.cpp` attaches AST source positions from parsed input source; `BuildLineNumberProgram()` consumes pandasm instruction source positions, real bytecode sizes, columns, and local-variable events | Minimal-hap is byte-identical on the cache-TS route after aligning source positions, call/branch/argument stores, implicit returns, lexical `this` locals, special-opcode pc deltas, MethodItem tag order, parameter-name string order, and aligned trailing LNP index. Broader fixtures still need coverage. | ✅ |
| H10 | `--merge-abc` / default non-merge emit mode | `EmitMergedAbcJob` uses `AsmEmitter::EmitPrograms`; default non-merge uses `EmitSingleAbcJob`, source `recordName=""`, `OutputFiles().at(inputPath)`, and metadata records such as `_ESModuleRecord`, `_ESScopeNamesRecord`, `_HasTopLevelAwait`, `_SharedModuleRecord`, `_CommonJsRecord` instead of merge-record fields | Merge mode is implemented. Single-source non-merge compiles through the Rust ABC backend instead of failing; ts2abc parsing leaves recordName empty and defaults source_file to the input path when `--merge-abc` is absent, matching upstream options.cpp. Non-merge metadata is split into upstream-style standalone records for module record, scope names, top-level await, shared module, and CommonJS. Multi-source non-merge file lists now parse the upstream `filePath;recordName;moduleKind;sourceFile;outputfile;isSharedModule;sourceLang` shape and write one ABC per input via `Ts2abcInput.output`, matching `OutputFiles()`. The low-level `AbcCompiler::compile` remains a single-output API and still rejects multi-source non-merge directly. | 🔄 |
| H11 | Incremental ABC cache via `--cache-file` / `@modules.cache` ProgramCache | `aot/options.cpp::ParseCacheFileOption`, `compiler/core/compileQueue.cpp::RetrieveProgramFromCacheFiles`, `aot/emitFiles.cpp::ScheduleEmitCacheJobs` | Partially aligned. The false ARKD batch cache of final ABC bytes has been removed. `arkdown-abc` now carries upstream `protoPanda` protobuf types, reads/writes source `ProgramCache` and ABC-input `AbcProtoProgramsCache` protoBin envelopes with `prost`, parses `@modules.cache` as `sourceFile;protoBin`, and computes the upstream Murmur3 cache hash including the current Panda bytecode version string. `ts2abc --cache-file` maps cache files per input; source inputs restore/store per-source `Program`, and ABC inputs restore/store `abcPath|recordName -> Program` entries before normal ABC emission. Remaining gap: the Rust `pandasm_insns` model still needs broader opcode parity evidence against upstream abc2program for large bytecode-HAR fixtures. | 🔄 |
| H12 | Widget compilation (separate pass, `widgetCompile: "true"`) | ArkCompile calls `initWidgetDefaultArkCompileConfig()` for form entries | `compile_widget_pass()` in CLI: scans extension abilities for form entries, transforms + rewrites OHM URLs, writes a widget CompileArkTS handoff (`filesInfo.txt`, `modules.cache`, `npmEntries.txt`), and compiles to `widgets.abc` through `Ts2abcCompiler` and the SDK `es2abc` driver. Output path matches hvigor's `ets/widgets.abc`. | ✅ |
| H13 | ArkData compile (`arkDataCompile()`) | Separate compiler invocation for ArkData sources | `compile_arkdata_pass()`: scans `.ets` files for `@kit.ArkData`/`@ohos/cloudData` imports; transforms and compiles to `ets/data.abc` through `Ts2abcCompiler` and the SDK `es2abc` driver. ABC ProgramCache is not enabled on this path until an ArkData handoff manifest is produced. | ✅ |
| H14 | CompileArkTS source maps and final `sourceMaps.map` | ets-loader writes `sourceMaps.json`; package includes `ets/sourceMaps.map` | Source-map generation uses `rolldown_sourcemap`/`string_wizard` and the hvigor-standard mapping format. Strict real-project goldens cover ComponentV2 HAP debug/release/unchanged/changed passes, HSP debug/release, source HAR debug/release, and ByteCode HAR debug/release. The source-map handoff intentionally differs by current SDK task: HAP/HSP/source HAR retain `intermediates/source_map`, while ByteCode HAR bypasses it and uses loader output (debug) or CompileArkTS cache (release). | ✅ |

### Stage I — Obfuscation and Minification (Release)

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| I1 | SDK obfuscation pass (rename vars, strip exports) | `loadSdkFunctions` → obfuscation SDK plugin | CLI runs `Obfuscator::obfuscate_file()` on all `.js` chunks in-place for release+`enable_obfuscation: true` builds, before ABC compilation | ✅ |
| I2 | Compute obfuscation file hash; cache-invalidate ABC if rules change | `ArkCompile::computeObfuscationFileHash()` | `TaskInputs::compute_obfuscation_hash()` hashes the obfuscation config; hash change → `matches_cached()` → full rebuild | ✅ |
| I3 | Generate consumer obfuscation config file for HAR publishing | `generateConsumerObConfigFile()` from SDK | For HAR+obfuscation builds, Rust parses `consumerFiles`/`consumerRules`, merges dependency `obfuscation.txt` configs for HAR output, and writes `intermediates/obfuscation/default/obfuscation.txt` for packaging | ✅ |
| I4 | Terser JS minification (`@rollup/plugin-terser`) | JSBUNDLE release mode only | N/A — arkdown targets ESMODULE mode only; ESMODULE minification is through the obfuscator (I1) | ✅ (N/A) |
| I5 | Move release source maps to separate directory | `ArkCompile::moveReleaseMap()` | Release maps are not embedded in HAP/HSP. The compiler map is retained as `CompileArkTS/.../release/sourceMaps.map`; HAP/HSP/source HAR also keep the SDK package intermediate, while ByteCode HAR does not. All publish `outputs/*/mapping/sourceMaps.map`. | ✅ |

### Stage J — Incremental Cache

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| J1 | Task-level incremental skip: compare declared inputs (obfuscationHash, TSC_MAX_FLOW_DEPTH, etc.) | `ArkCompile::declareInputs()` | `TaskInputs` struct computes `source_hash` (FNV-1a over `compileEntry`), `obfuscation_hash`, `sdk_version`, and `compile_env` key; `matches_cached()` skips the entire bundle+ABC step when all match | ✅ |
| J2 | Rollup file-system cache: msgpack format, `compileEnv` key | `RollupCacheService` | `EtsTransformPlugin` disk cache (`transform_cache.json`) + `TaskInputs` full-build skip + `CompileEnv::to_key_string()` invalidation; per-file content hash cached to disk; rolldown 1.0.3 scan-stage API not exposed but transform caching covers the hot path | ✅ |
| J3 | es2abc incremental cache: protobuf ProgramCache/protoBin via `--cache-file` | `es2abc` + hvigor cache integration | Partially aligned. The private final-ABC cache has been deleted; `--cache-file` now follows upstream bare/`@manifest` mapping. Source inputs use `protoPanda.ProgramCache` for cache hit/store around lowering; ABC inputs use `protoPanda.AbcProtoProgramsCache` for cache hit/store around abc2program, matching upstream's separate `GetAbcInputCacheContext` / `UpdateAbcCacheFile` path. Remaining work is broad parity validation against hvigor-generated cache files and larger bytecode-HAR ABC inputs. | 🔄 |
| J4 | Clean ABC cache when `obfuscationEnable` changes | `ArkCompile::beforeTask()` | Covered by J1: `obfuscation_hash` changes when `obfuscationEnable` toggles, causing `matches_cached()` to return false and triggering a full rebuild | ✅ |

### Stage K — Packaging

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| K1 | Generate `module.json` (app + module merged) | `GeneratePkgModuleJson` → `generate-pkg-module-json.js` | `arkdown-profile::write_module_json()` — all fields match parity report | ✅ |
| K2 | Assemble HAP zip (11 canonical entries) | `PackageHap` → `make-pack-info.js` + zip | `arkdown-pack::pack_hap()` — matches hvigor golden exactly | ✅ |
| K3 | Assemble HSP zip | `PackageHsp` | `arkdown-pack::pack_hsp()` delegates to `pack_hap()` — HSP has the same zip structure as HAP but with `module.json#module.type = "shared"` (set by `process_profile()`); parity verified by SPEC-012 | ✅ |
| K4 | Assemble HAR (source HAR, not ByteCode HAR) | `PackageHar` | `pack_har()` packs `oh-package.json5`, `src/main/ets/**`, `src/main/resources/**`; `generate_har_declarations()` emits `index.d.ts`/`index.d.ets` re-export stubs to `intermediates/har/default/`; consumer obfuscation config written (I3); `cleanEtsForTgz` N/A (M4) | ✅ |
| K5 | `ProcessIntegratedHsp` — embed remote/integrated HSP deps | `process-integrated-hsp.js` | `collect_integrated_hsps()`: reads `oh-package.json5` deps, checks `metadata.isIntegratedHsp`, finds `.hsp` file in `oh_modules/`, normalizes via `normalize_integrated_hsps()`, passes to `pack_hap()` | ✅ |
| K6 | Native SO library embedding (`libs/arm64-v8a/`) | `PackageHap` + `CacheNativeLibs` | `collect_native_libs()` scans `src/main/libs/`, `libs/`, and `oh_modules/` for `arm64-v8a/*.so`; `pack_hap()` embeds each as `libs/arm64-v8a/<filename>` (stored entry) | ✅ |

### Stage L — Signing

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| L1 | HAP signing: PKCS#12 + CMS SignedData + V2/V3 block | `SignHap` → `sign-hap.js` → `hap-sign-tool.jar` | SDK signing backend allowed for initial parity; `arkdown-sign` remains the Rust backend with ECDSA P-256, RSA-PSS, PKCS#12 loading, CMS generation, ZIP alignment | ✅ |
| L2 | HSP signing | `SignHsp` | `arkdown-sign::sign_hsp()` (same code path) | ✅ |
| L3 | HAR signing (optional) | `sign/har-sign-command-build.js` | When `signing_config` is present, HAR archive is signed via `HapSigner::sign_or_skip()` (same V2/V3 block pipeline as HAP/HSP); unsigned HAR is output when no signing config | ✅ |

### Stage M — Advanced / Watch Mode

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| M1 | Incremental watch mode: chokidar file watcher + rebuild | `watch.js` in build-steps + `hvigorw --watch` | `--watch` flag wired into CLI build command; `FileWatcher::watch()` monitors module source dirs with 100ms debounce and triggers full rebuild on change | ✅ |
| M2 | Hot-reload: importer graph delta computation | `collectImportersPlugin` + `cold-reload-ark-compile.js` | Importer map (F13) used in watch callback: changed files matched against chunk keys; if no chunk is affected the rebuild is skipped; full bundle still triggered for affected chunks (rolldown 1.0.3 does not support per-module invalidation) | ✅ |
| M3 | Preview mode: resource watcher + live resource reload | `preview-resource-watcher-plugin.js` | `--preview` watches `src/main/resources/` with 100ms debounce; changed file list written to `build/default/outputs/preview_changes.json` for IDE file-watching integration; full build triggered on change; DevEco IDE WebSocket protocol not implemented (requires closed-source protocol spec) | ✅ |
| M4 | HAR TGZ clean (strip rollup output for HAR publishing) | `clean-ets-for-tgz.js` | N/A: `pack_har()` reads directly from `src/main/ets/` source, never from rollup output intermediates; there is no bundled `.js` output to strip from the HAR | ✅ (N/A) |
| M5 | ETS standalone checker (lint-only mode) | `etsStandaloneChecker` from SDK | `arkdown check` invokes the installed SDK `etsStandaloneChecker` through the Rust-owned plan/diagnostic boundary, then augments it with OXC structural checks. The regression fixture injects `const x: number = "not-a-number"` and verifies a non-zero semantic diagnostic. | ✅ |

---

## 2. Gap Inventory

### Critical Gaps (blocking full hvigor parity)

**H9 — ABC DebugInfo/LNP source-position event stream**
- hvigor: `emitter.cpp::GenInstructionDebugInfo()` attaches AST start positions to normal instructions and function/block end positions to return instructions; `BuildLineNumberProgram()` then emits line/column and local-variable events from the pandasm instruction stream
- arkdown: minimal-hap now matches hvigor byte-for-byte on `ets/modules.abc`
- HmosWorld (latest installed IDE SDK): the SDK `ark_disasm --quiet` comparison has the same 11,559 literal entries, 631 records, 7,759 normalized function instruction streams, and 7,906 strings. Raw `modules.abc` remains different (hvigor 4,163,104B; arkdown 4,165,100B), because offsets, generated function identities, input-module ordering, and remaining TypeScript-vs-OXC source-layout positions are serialized into the binary.
- ComponentV2 now has same-root cold/unchanged/changed debug coverage plus release coverage; HSP, source HAR, and ByteCode HAR have strict debug/release goldens. Remaining work is large-project DebugInfo/source-layout coverage for explicit constructors, field initializers, more ArkUI DSL forms, and dependency graphs. A raw byte comparison against one arbitrary hvigor run is not sufficient: its Rollup module map is populated after asynchronous native resolver completions.

**H10 — Direct CompileArkTS cache-TS backend route**
- hvigor: CompileArkTS writes transformed TS cache files, `filesInfo.txt`, `npmEntries.txt`, and invokes `es2abc @filesInfo`
- arkdown: writes the matching handoff metadata, preserves the etsTransform source snapshot before Rolldown's internal TypeScript scan, and invokes the current SDK `es2abc` route for final ABC generation
- Current status: minimal-hap `ets/modules.abc` is 9844B and byte-identical to hvigor
- HmosWorld status: builds end-to-end without unresolved package externals. `filesInfo.txt`, `modules.cache`, and `npmEntries.txt` match hvigor by record set (572/572 module entries, 573/573 ProgramCache entries, and 57/57 package entries). 302/572 cache TS/JS files are byte-identical after preserving numeric/string/comment spelling and the pre-Rolldown etsTransform handoff; the remaining 270 are source-printer/layout goldens, not missing module records. The latest SDK `module_build_mode.js` passes `rollup.getModuleIds()` into `module_mode.js::collectModuleFileList()`. Its `Map` insertion order follows async resolver completion, so arkdown deliberately uses stable declared graph order plus sorted collector fallbacks rather than reproducing timing-dependent binary order.
- Remaining work: turn the residual cache source-layout cases into focused etsTransform/source-map goldens, then validate DebugInfo with same-root builds. Do not replace the SDK compiler with a hand-rolled `arkdown-abc` cache/proto implementation: production currently delegates `--cache-file @modules.cache` and merge behavior to the installed SDK `es2abc`, as hvigor does.

**F16 — Large-project ViewPU source-layout goldens**
- hvigor: `etsTransform` SDK plugin fully lowers ArkUI `struct`+`build()` blocks into `observeComponentCreation2()`, component-specific create methods, attribute method calls, and `.pop()` calls
- arkdown: `EtsTransformPlugin` now lowers every ArkTS-only node emitted by the pinned OXC fork (`StructStatement`, `ArkUIComponentExpression`, `AnnotationDeclaration`, `LazyImportDeclaration`), and its checker consumes the same `StructStatement` nodes for upstream-aligned struct-decorator validation before rolldown's standard TS parser runs. It covers `@Component`/`@Entry`/`@CustomDialog` structs, top-level and method `@Builder`, `@Styles`, `@Extend`/`@AnimatableExtend`, `@interface`, `import lazy`, ArkUI `if`/callback bodies, Button `createWithLabel/createWithChild`, builder parent-context calls, upstream-scoped regular-attribute immutable grouping, reusable-component `updateRecycleElmtId()`, `processBuildMember()` raw observed object call-argument wrapping, child-`@Link` constructor backing-field params, direct `@Prop` update-state resets, and the upstream V1/V2 property-decorator classification set into TypeScript the rolldown/OXC path can parse.
- The previously identified `NavDestination.create`, `__Common__.create`, `ifElseBranchUpdateFunction`, `forEachUpdateFunction`, Gesture `.pop()`, and `@CustomDialog` controller paths are implemented. Remaining work is to turn the HmosWorld printer/layout differences into focused goldens, not to add a JavaScript-side lowering fallback.
- ComponentV2 lifecycle boundaries now have strict source/output/cache goldens and the SDK lifecycle parameter diagnostic; remaining large-project work is the HmosWorld source-layout/DebugInfo set described above.

**F15 — SDK semantic etsChecker** ✅ resolved
- hvigor: `etsChecker` owns TypeScript semantic analysis in `ets_checker.js`
- arkdown: default build checking and `arkdown check` invoke the installed SDK `etsStandaloneChecker`; Rust/OXC performs the complementary structural ArkTS checks
- Evidence: `packages/arkdown/tests/check_semantic.rs` injects a TypeScript assignment error and requires the SDK semantic failure boundary

**H14 — Source-map/package/cache layout** ✅ resolved for the declared fixture matrix
- Strict real-project comparisons cover ComponentV2 HAP debug/release/cold/unchanged/changed, HSP debug/release, source HAR debug/release, and ByteCode HAR debug/release.
- The test matrix captures the current SDK's distinct source-map handoffs instead of forcing a single directory layout for every artifact kind.

### High-Impact Gaps (blocking release/production builds)

**I1-I5 — Obfuscation pipeline** ✅ resolved
- `Obfuscator` is wired in `BuildCoordinator` (source obfuscation between
  bundling and ABC; ArkGuard bytecode obfuscation after ABC), driven by
  `buildOption.arkOptions.obfuscation`; `TaskInputs` invalidates on rule hash
  changes; consumer obfuscation config is written for HAR publishing
- Strict golden coverage via the `release-obfuscation-hap` parity fixture

**J2-J3 — Incremental cache not fully parity-verified**
- Rolldown transform/task cache is wired for the current Rust pipeline
- The old ABC ARKD final-byte cache has been removed; `--cache-file` now uses upstream source-to-protoBin mapping, source `ProgramCache`, ABC-input `AbcProtoProgramsCache`, and upstream-style hash inputs including the current bytecode version string
- Remaining work: validate generated protoBins against hvigor on large real projects and bytecode-HAR inputs; extend ABC bytecode -> `pandasm_insns` decoding if those fixtures expose additional upstream opcodes

**F25-F26 — Release mode chunk splitting and tree-shake** ✅ resolved
- `manualChunks` implemented via `Bundler::build_manual_code_splitting_options()`; `support_chunks` wired from config
- `treeshake: true` enabled for release mode; `false` for debug (matches hvigor)

**D2-D5 — Multi-module dependency maps partially incomplete**
- `dynamicImportLibInfo` and transitive `harNameOhmMap`/`hspNameOhmMap` are implemented; `byteCodeHarInfo` now follows the declared dependency graph instead of scanning all installed packages
- The ByteCode HAR producer declaration/source-map/archive behavior is strict-golden covered. Remaining dependency work is bundled-dependency edge cases and larger hvigor DependencyManager graphs.

**K3-K4 — HSP/HAR package parity** ✅ resolved for the declared fixture matrix
- Strict goldens cover minimal HSP and source HAR in debug/release, and ByteCode HAR in debug/release.
- HSP declaration-output and exotic HAR obfuscation/dependency combinations remain separate feature matrices; they are not claimed by these fixtures.

### Medium-Impact Gaps (advanced features)

**B4-B5 — ProcessShareConfig / ProcessStartupConfig** ✅ resolved
- `merge_share_config()` and `merge_startup_config()` now wired in CLI build pipeline
- Duplicate-URI and duplicate-task-name violations are now hard errors (matching hvigor `printErrorExit`)

**B8 — ProcessModulePrivacyProfile** ✅ resolved
- `pack_hap()` now writes `module.privacy.json` (deflated) from `input.privacy_profile_path` when present

**B6 — module.json field parity** ✅ resolved
- `packageName` now read from `oh-package.json5#name` (hvigor: `Object.assign` using `packageJsonObj.name`)
- `installationFree` now derived from `bundleType` (`atomicService` → `true`; other → `false`)
- `atomicService` config block preserved verbatim for atomicService bundles
- `deliveryWithInstall` now read from module.json5 (default `true`)
- `dependencies` now serialized from manifest (not unconditionally empty array)
- `requestPermissions`, `definePermissions`, `proxyData` passed through verbatim from raw module.json5
- Unknown module-level fields preserved via preserve-and-mutate passthrough (mirrors hvigor `cloneDeep` + mutate)
- `abilities` and `extensionAbilities` preserve unknown fields (launchType, visible, continuable, orientation, windowMode, permissions, forms, etc.) via preserve-and-mutate; computed values (resource IDs, srcEntry) are overlaid
- HSP declarations (`ArkCompile.declareOutputFiles`) generated via `HarDeclarationGenerator::new_hsp` for `module.type = "shared"` modules

**C3 — ProcessCompiledResources (preloaded SO)** ✅ resolved
- `buildOption.preloadSo` paths written to `intermediates/preload_so/default/preload_so_list.json` after resource compilation (`addPreloadSystemSoFileList` equivalent)

**F13, M1-M5 — Watch mode, hot-reload, preview mode**
- SPEC-023 through SPEC-027
- Only needed for DevEco IDE integration

**K5-K6 — IntegratedHsp + native SO embedding**
- Required for apps bundling remote HSP or native libraries

**H12-H13 — Widget compilation + ArkData compile**
- SPEC-027: separate compile pass for widget/card entry points
- ArkData compile: separate invocation for ArkData source files

---

## 3. Implementation Tasks

### TASK-ABC-01: Align DebugInfo/LNP source-position events (M)

**What**: Preserve minimal-hap byte parity while extending the DebugInfo/LNP
source-position rules to broader golden fixtures. The minimal cache-TS route is
now aligned; future work should prevent regressions as more ArkUI and module
forms are added.
**File**: `crates/arkdown-abc/src/ir/builder.rs`, `crates/arkdown-abc/src/emit/file.rs`
**Upstream ref**: `emitter.cpp::GenInstructionDebugInfo()`,
`assembly-emitter.cpp::BuildLineNumberProgram()`,
`codegen_dynamic.cpp` generated-instruction source-position inheritance
**Complexity**: M
**Depends on**: nothing

### TASK-ABC-02: Direct CompileArkTS cache-TS CodeItem parity (M)

**What**: Keep the hvigor-style `filesInfo.txt`/`npmEntries.txt` handoff as the
final ABC input route. The cache TS bytes and `modules.abc` now match hvigor for
the minimal fixture, and `npmEntries.txt` is lowered as upstream
`CompileNpmModuleEntryList()` records. Remaining work is broader coverage and
real `modules.cache`/protoBin ProgramCache compatibility.
**File**: `packages/arkdown/src/cmd/build.rs`, `crates/arkdown-abc/src/`
**Upstream ref**: `gen_merged_abc.ts`
**Complexity**: M
**Depends on**: TASK-ABC-01

### TASK-ABC-03: ABC section diff harness (S) ✅ resolved

**What**: Add a regression helper that compares CodeItems separately from
DebugInfo/LNP/StringTable/ClassRecord sections, so total ABC size changes do not
hide executable-bytecode regressions.
**File**: `crates/arkdown-abc/src/abc_reader.rs`
**Upstream ref**: local hvigor minimal-hap golden
**Complexity**: S
**Status**: `AbcSectionHashes::compute()` returns `full_hash`, `code_hash` (pre-LNP region),
and `lnp_hash` (LNP section). `code_region_matches()` and `is_identical()` helpers
cover the two comparison modes. Three regression tests added.

### TASK-ABC-04: ABC incremental cache wiring (M)

**What**: Complete hvigor-compatible `--cache-file @modules.cache` behavior.
The old JSON/ARKD final-ABC cache is removed and source-to-protoBin mapping,
protobuf schema, protoBin envelope read/write, upstream cache hash, source
ProgramCache hit/store, and ABC-input AbcProtoProgramsCache hit/store are in
place. Remaining implementation should focus on parity validation and any opcode
coverage gaps exposed by hvigor-generated protoBins or bytecode-HAR ABC inputs.
Use `tracing::debug!` for diagnostics; do not add a custom debug channel.
**File**: `crates/arkdown-abc/src/cache.rs`, `crates/arkdown-abc/src/proto_panda.rs`, `crates/arkdown-abc/src/compiler.rs`, `packages/arkdown/src/cmd/build/bytecode/`, `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `es2abc --cache-file` behavior; hvigor `RollupCacheService` msgpack cache at `cache/msgpack/MSGPACK_COMPILE_CACHE_DIR_NAME`; `ark-compile.js::trace()` and `doTaskAction()` cache setup
**Complexity**: M
**Depends on**: TASK-CACHE-01

### TASK-ABC-05: Widget compilation pass (M) ✅ resolved

**What**: Implement `initWidgetDefaultArkCompileConfig()` equivalent: second `arkdown-abc` invocation with `widgetCompile: true`, separate cache dir, widget-specific loader.json input.
**File**: `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `ark-compile.js::doTaskAction()` — `if (this.needSubmitArkTsWidget) { initWidgetDefaultArkCompileConfig() }`; SPEC-027
**Complexity**: M
**Status**: `compile_widget_pass()` scans extension abilities for form entries, transforms widget sources with `EtsTransformPlugin`, rewrites OHM imports, writes a widget CompileArkTS handoff under the widget cache root, and compiles to `ets/widgets.abc` through `Ts2abcCompiler` and the SDK `es2abc` driver.

### TASK-ETS-01: ViewPU build() body lowering (L)

**What**: Implement the core ArkUI DSL lowering: `struct @Component { build() { ... } }` → `observeComponentCreation2()` call chains, `.create()`, attribute method calls (`.fontSize()`, `.padding()`, etc.), and `.pop()`. Port hvigor's `ets-loader/src/ui_syntax_checker.ts` and `ets-loader/src/fast_build/ark_compiler/` transforms.
**File**: `crates/arkdown-plugin-ets-transform/src/`
**Upstream ref**: DevEco SDK `ets-loader` source at `<SDK>/ets/build-tools/ets-loader/`; `load-sdk-functions.js::loadSdkFunctions()` — `t.etsTransform()` invocation
**Complexity**: L
**Depends on**: TASK-ETS-02

### TASK-ETS-02: @Builder and @Styles decorator transform (M)

**What**: `@Builder` functions desugar into builder lambda captures. `@Styles` desugar into attribute set functions. Both must integrate with ViewPU's component creation chain.
**File**: `crates/arkdown-plugin-ets-transform/src/`
**Upstream ref**: DevEco SDK `ets-loader` decorator transform passes
**Complexity**: M
**Depends on**: nothing (can be developed in parallel with TASK-ETS-01)

### TASK-ETS-03: apiTransform expression rewrites (M)

**What**: Rewrite OHOS-specific API call patterns at the AST level: `@kit.*` import expansion into individual `@ohos:*` imports with correct ArkRuntime module names, event emitter rewrites, permission decorator lowering.
**File**: `crates/arkdown-plugin-ets-transform/src/`
**Upstream ref**: `load-sdk-functions.js` — `t.apiTransform()` loaded from `compile_plugin.js`; kit-to-ohos mapping in `hvigorw-pipeline.md §OHOS Module Resolution Details`
**Complexity**: M
**Depends on**: nothing

### TASK-OBFUS-01: Wire obfuscation pipeline (M)

**What**: Connect `arkdown-obfuscate` into the CLI pipeline: run after rolldown bundling, before `arkdown-abc`. Read obfuscation rules from `build-profile.json5#buildOption.arkOptions.obfuscation`. Compute `obfuscationHash` for cache invalidation. Emit consumer obfuscation config for HAR publishing.
**File**: `packages/arkdown/src/main.rs`, `crates/arkdown-obfuscate/src/lib.rs`
**Upstream ref**: `ark-compile.js::initDefaultArkCompileConfig()` — `obfuscationOptions` setup; `load-sdk-functions.js` — terser plugin wired when `n === RELEASE && obfuscationEnabled`; SPEC-020
**Complexity**: M
**Depends on**: TASK-CACHE-01 (for cache invalidation on rule changes)

### TASK-CACHE-01: Wire incremental rolldown cache (M)

**What**: Implement rolldown `ScanStageCache` serialization to/from msgpack files in `cache/msgpack/`. Build `compileEnv` key from `[compileSdkVersion, compatibleSdkVersion, runtimeOS, etsLoaderPath, obfuscationEnabled, moduleType, cachePath].join("#")`. Invalidate on key change.
**File**: `crates/arkdown-cache/src/lib.rs`, `crates/arkdown/src/lib.rs`, `packages/arkdown/src/main.rs`
**Upstream ref**: `arkts-pack.js::getCompileEnv()` and `generateConfig()`; `build.js::readCacheWithEvent()` and `rollupCacheService.writeCache()`; `RollupCacheService` in `hvigor-arkts-base/dist/src/cache/`
**Complexity**: M
**Depends on**: nothing

### TASK-CACHE-02: Task-level incremental skip (M) ✅ resolved

**What**: Before running the full build, hash declared inputs (source files, obfuscationHash, SDK version, compileEnv). Skip ArkCompile if all inputs and outputs match. Write/read a `task-state.json` with input hashes.
**File**: `packages/arkdown/src/task_cache.rs`, `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `ark-compile.js::declareInputs()` and hvigor's task state cache framework; `ArkCompile::beforeTask()` and `checkAndCleanCache()`
**Complexity**: M
**Status**: `TaskInputs` struct computes `source_hash` (FNV-1a over compile_entry paths), `obfuscation_hash`, `sdk_version`, and `compile_env` (via `CompileEnv::from_config()`). `matches_cached()` skips the entire bundle+ABC step when all match. `save()` persists to `cache_dir/.task_inputs`.

### TASK-LOADER-01: ByteCode HAR dependency maps (M)

**What**: Populate `loader_json.byteCodeHarInfo` from `oh-package.json5` dependencies that reference pre-compiled ByteCode HAR packages. Follow hvigor's `metadata.byteCodeHar` marker and `ets/modules.abc` / `ets/sourceMaps.map` defaults; preserve dependency `metadata.declarationEntry` as normalized OHM URLs instead of treating it as a declaration-file path.
**File**: `crates/arkdown-profile/src/loader_json.rs`, `crates/arkdown-resolver/src/`
**Upstream ref**: `byte-code-har-utils.js::collectByteCodeHarInfo()`, `GenerateLoaderJson.processDeclarationEntry()`, `ProcessHarArtifacts.processDeclarationEntry()`, `loader.json` schema in `hvigorw-analysis.md §9`
**Complexity**: M
**Depends on**: nothing
**Status**: dependency graph scan, `updateVersionInfo`, and `otherCompileFiles` are implemented in `crates/arkdown-profile/src/bytecode_har.rs`: scan follows root package dependencies transitively, excludes undeclared installed packages, preserves the legacy no-root-manifest fixture fallback, keys loader entries by package `name`, emits version update metadata from `dependencyPkgVersion`/installed dependency versions, and resolves ByteCode HAR `extraImportees` plus directly blocked source HAR/npm entries. The producer declaration/source-map/archive contract is covered by `tests/fixtures/bytecode-har`; remaining work is bundled-dependency edge cases.

### TASK-LOADER-02: Dynamic import library info (M)

**What**: Detect runtime-only source files and dynamic-import packages. Append product `runtimeOnly.sources` source files/directories to `compileEntry`, and write runtime-only/dynamic-import packages to `dynamicImportLibInfo`: `{ pkgName: { entryFilePath, moduleName } }`.
**File**: `crates/arkdown-profile/src/loader_json.rs`, `crates/arkdown/src/` (post-bundle module graph walk)
**Upstream ref**: `GenerateLoaderJson.processDynamicImport()`; `arkts-pack.js::generateConfig()` — `dynamicImportLibInfo` from resolver
**Complexity**: M
**Depends on**: TASK-LOADER-01

### TASK-PACK-01: ProcessCompiledResources — preloaded SO (S) ✅ resolved

**What**: After resource compilation, collect preloaded system SO file paths from `buildOption.preloadSo`. Write the preload SO file list to the HAP intermediates.
**File**: `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `ark-compile.js::addPreloadSystemSoFileList()` and `allowToPreloadSystemSo()`; `PackageHap` SO library embedding
**Complexity**: S
**Status**: `buildOption.preloadSo` paths written to `intermediates/preload_so/default/preload_so_list.json` after resource compilation.

### TASK-PACK-02: ProcessIntegratedHsp (M)

**What**: When a HAP/HSP has integrated (embedded) remote HSP deps, copy their bytecode and resources into the HAP archive at the correct paths.
**File**: `crates/arkdown-pack/src/`
**Upstream ref**: `process-integrated-hsp.js`; `hvigorw-analysis.md §2`
**Complexity**: M
**Depends on**: TASK-LOADER-01

### TASK-PACK-03: ProcessModulePrivacyProfile (S) ✅ resolved

**What**: If `src/main/resources/base/profile/privacy.json` exists, validate and embed it as `module.privacy.json` in the HAP.
**File**: `crates/arkdown-pack/src/packager.rs`
**Upstream ref**: `process-module-privacy-profile.js`
**Complexity**: S
**Depends on**: nothing
**Status**: `pack_hap()` now writes `module.privacy.json` (deflated) from `input.privacy_profile_path` when present.

### TASK-PACK-04: ProcessShareConfig + ProcessStartupConfig (M) ✅ resolved

**What**: Read `crossAppSharedConfig` from HSP dep `module.json5`. Merge startup task configs from all HAR/HSP deps. Write intermediate JSON for downstream tasks.
**File**: `crates/arkdown-profile/src/`, `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `process-share-config.js`, `process-startup-config.js`
**Complexity**: M
**Depends on**: nothing (resolved without TASK-LOADER-01 dependency)
**Status**: `merge_share_config()` and `merge_startup_config()` wired in CLI; duplicate-URI and duplicate-task-name violations are hard errors matching hvigor `printErrorExit`.

### TASK-RELEASE-01: treeshake + manualChunks for release builds (M) ✅ resolved

**What**: Enable rolldown `treeshake: true` in release mode. Implement `manualChunks` equivalent: modules from `oh_modules/` → chunk named "commons"; non-entry, non-node_modules → chunk named "vendors".
**File**: `crates/arkdown/src/bundler.rs`
**Upstream ref**: `arkts-pack.js::generateConfig()` — `treeshake: n === RELEASE`; `manualChunks(id, {getModuleInfo})` function
**Complexity**: M
**Status**: `TreeshakeOptions::Boolean(true/false)` switches on build mode; `build_manual_code_splitting_options()` provides the manualChunks logic via `support_chunks` from config.

### TASK-WATCH-01: Watch mode (L) ✅ resolved

**What**: Implement `--watch` flag: chokidar-equivalent file watching (use `notify` crate), incremental rebuild on change, delta-only ABC recompilation.
**File**: `crates/arkdown-watcher/src/`, `packages/arkdown/src/cmd/build.rs`
**Upstream ref**: `build-steps/watch.js`; `arkts-pack.js::getWatchConfig()` — `{chokidar: {useFsEvents: false}, buildDelay: 100}`; SPEC-023
**Complexity**: L
**Status**: `--watch` flag wired into CLI build command; `FileWatcher::watch()` monitors module source dirs with 100ms debounce; on change triggers full rebuild. `--preview` flag watches only `src/main/resources/`. Importer map (F13) written for future delta computation (M2).

---

## 4. Priority Ordering

### Priority 1 — ABC DebugInfo Parity (finish minimal-hap byte parity)

These preserve the closed minimal-hap `modules.abc` byte parity while expanding
coverage.

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-ABC-01: Align DebugInfo/LNP source positions | M | Preserve byte parity across broader fixtures |
| TASK-ABC-02: Direct cache-TS CodeItem parity | M | Default route; extend coverage and cache behavior |
| TASK-ABC-03: ABC section diff harness | S | Prevents CodeItem regressions |

### Priority 2 — ArkUI Component Support (unblock real UI pages)

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-ETS-01: ViewPU build() lowering | L | Critical — no real ArkUI app works without this |
| TASK-ETS-02: @Builder/@Styles | M | High — common ArkUI patterns |
| TASK-ETS-03: apiTransform rewrites | M | High — @kit.* imports |

### Priority 3 — Release Build Quality

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-OBFUS-01: Wire obfuscation | M | Required for release HAP |
| TASK-CACHE-01: Rolldown cache wiring | M | 10× build speed improvement |

### Priority 4 — Multi-Module Project Support

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-LOADER-01: ByteCode HAR maps | M | Any project with BC-HAR deps |
| TASK-LOADER-02: Dynamic import info | M | Dynamic import patterns |
| TASK-PACK-02: ProcessIntegratedHsp | M | Projects embedding HSP |

### Priority 5 — Polish and Advanced Features

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-ABC-04: ABC incremental cache | M | Compile speed |

---

## 5. Upstream Source References

| Area | URL |
|---|---|
| ArkCompile task (full) | `/Applications/DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js` |
| Rollup plugin setup | `/Applications/DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js` |
| Build orchestration | `/Applications/DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build.js` |
| SDK plugin loading | `/Applications/DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js` |
| Importee processing | `/Applications/DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build-steps/process-importee.js` |
| assembly-emitter.cpp | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/assembler/assembly-emitter.cpp` |
| assembly-emitter.h | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/assembler/assembly-emitter.h` |
| Panda bytecode ISA | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/isa/isa.yaml` |
| ets-loader source | DevEco SDK: `<OHOS_SDK_HOME>/ets/build-tools/ets-loader/` |

---

## 6. Summary Scorecard

The declared parity matrix is now strict-golden covered against the current
DevEco Studio Hvigor (6.24.3) for:

- SDK-owned TypeScript semantic diagnostics in both build and `arkdown check`;
- ComponentV2 lifecycle HAP output in debug, release, and cold/unchanged/changed
  same-root builds;
- HSP, source HAR, and ByteCode HAR package output in debug and release;
- source-map handoff, archive/mapping output, and cache invalidation for those
  fixtures.

The full parity suite (25 tests) runs against the real DevEco-bundled hvigor
via `hvigor/tools/arkdown-hvigorw` (see README "Parity testing against a real
hvigor"); all fixtures compare artifact-for-artifact with zero diffs, including
the 6.24.3-only `pkgName2OtherCompileFiles` widget-loader field (D6).

This is not a claim that every arbitrary large application is already a
drop-in Hvigor replacement. The explicitly retained global risks are the
HmosWorld source-printer/DebugInfo goldens and larger dependency/ByteCode cache
graphs documented in H9, H10, H11, and J3. Items marked ✅ mean the declared
scope has an implementation and observed parity evidence; they do not erase
those named large-project validation boundaries.
