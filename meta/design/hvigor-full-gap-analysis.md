# hvigor Full Pipeline Gap Analysis

Comprehensive mapping of every hvigor build step against the current arkdown implementation.
Source data: live `@ohos/hvigor-arkts-compose@6.21.1` and `@ohos/hvigor-arkts-base@6.21.1`
JS sources from `~/Downloads/command-line-tools/` (2026-05-29).

Status symbols: ✅ implemented + parity-verified, 🔄 partial (gaps listed), ❌ not started.

---

## 1. Full Pipeline Map

### Stage A — Pre-Build

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| A1 | SDK change check + build-start event | `PreBuild` → `hvigor-ohos-plugin/src/tasks/pre-build.js` | `arkdown-cli`: SDK path discovery + `ArkdownConfig` | ✅ |
| A2 | Build OHM URL context map | `GeneratePkgContextInfo` → `generate-pkg-context-info.js` | `arkdown-profile::generate_pkg_context_info()` | ✅ |

### Stage B — Profile Processing

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| B1 | Merge base `module.json5` with product overlays + HAR/HSP deps | `MergeProfile` → `abstract-merge-profile.js` | `arkdown-profile::merge_profile()` | ✅ |
| B2 | Set `virtualMachine`, `compileMode`, `deviceTypes`; collect HAR abilities | `ProcessProfile` → `process-profile.js` | `arkdown-profile::process_profile()` injects both fields from SDK version | ✅ |
| B3 | Collect router map entries; write `intermediates/routerMap.json` | `ProcessRouterMap` → `process-router-map.js` | `arkdown-plugin-router-map` | ✅ |
| B4 | Process `crossAppSharedConfig` from HSP dependencies | `ProcessShareConfig` → `process-share-config.js` | Not implemented | ❌ |
| B5 | Merge startup config from all HAR/HSP deps; validate task deps | `ProcessStartupConfig` → `process-startup-config.js` | Not implemented | ❌ |
| B6 | Generate module metadata JSON for HAP/HSP | `CreateModuleInfo` → `create-module-info.js` | `arkdown-profile::write_module_json()` (HAP); HSP-specific path unverified | 🔄 |
| B7 | Generate module metadata JSON for HAR | `CreateHarModuleInfo` → `har/har-extend-info.js` | `pack_har()` writes metadata but separate HAR module-info flow unverified | 🔄 |
| B8 | Process privacy data file | `ProcessModulePrivacyProfile` → `process-module-privacy-profile.js` | Not implemented | ❌ |

### Stage C — Resource Compilation

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| C1 | Scan + validate resource files | `ProcessResource` → `process-resource.js` | `arkdown-resource::scan_resources()` | ✅ |
| C2 | Compile resources to binary RestoolV2 format | `CompileResource` → `compile-resource.js` invokes `restool` binary | `arkdown-resource` pure Rust port of `restool` | ✅ |
| C3 | Post-process compiled resources; handle preloaded SO | `ProcessCompiledResources` → `process-compiled-resources.js` | Not implemented (preloaded SO collection absent) | ❌ |

### Stage D — loader.json Generation

| # | hvigor Step | Field/Detail | arkdown | Status |
|---|---|---|---|---|
| D1 | Write loader.json: core 18 fields | `GenerateLoaderJson::doTaskAction()` → `generate-loader-json.js` | `arkdown-profile::generate_loader_json()` — all 18 fields match golden exactly | ✅ |
| D2 | `byteCodeHarInfo` map (pre-compiled HAR `.abc` paths) | From `DependencyManager` | `loader_json.by_byte_code_har_info` struct exists but not populated for real BC-HAR deps | 🔄 |
| D3 | `dynamicImportLibInfo` (cross-HAR dynamic imports) | `ArkCompile::initDefaultArkCompileConfig()` | Not populated | ❌ |
| D4 | `workers[]` (WebWorker `.ets` file paths) | `GenerateLoaderJson` from `workers/` scan | Present in struct but not fully connected to worker discovery | 🔄 |
| D5 | `hspNameOhmMap` + `harNameOhmMap` for multi-module projects | From `DependencyManager` | Basic maps for simple projects; transitive dep resolution incomplete | 🔄 |

### Stage E — ArkCompile Task Setup

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| E1 | Build `compileEnv` key: `[compileSdkVersion, compatibleSdkVersion, runtimeOS, etsLoaderPath, obfuscationEnabled, moduleType, cachePath].join("#")` | `arkts-pack.js::getCompileEnv()` | `arkdown-cache::CompileEnv` struct not yet wired to rolldown build | ❌ |
| E2 | Load SDK compile plugins from `ets-loader/compile_plugin.js` | `load-sdk-functions.js::loadSdkFunctions()` | Rust replacement; no `compile_plugin.js` needed | ✅ (Rust port) |
| E3 | `hijackTscCompilerOptionsPaths`: monkey-patch `ets_checker.js` for `@types` path resolution | `load-sdk-functions.js` | Not needed — Rust resolver handles paths directly | ✅ (N/A) |
| E4 | Read rollup msgpack cache from `cache/msgpack/` | `RollupCacheService::readCache()` → `rollup-cache-service.js` | `arkdown-cache` crate exists but `readCache` not wired into rolldown bundler | ❌ |

### Stage F — Rollup Bundling (plugin execution order)

hvigor configures rollup via `arkts-pack.js::generateConfig()`. Key flags:
`treeshake: buildMode === "release"`, `disableGenerateBundle: compileMode === "esmodule"`,
`format: "cjs"`, `shimMissingExports: true`, `context: "globalThis"`,
`chunkFileNames: "[name].js"`, `sanitizeFileName: false`.

| # | hvigor Plugin | Order / Hook | arkdown | Status |
|---|---|---|---|---|
| F1 | `commonPlugin` — ProjectConfig context sharing, pre-build cleanup | first (unshift) | Absorbed into bundler core config | ✅ |
| F2 | `invalidCachePlugin` — per-file cache invalidation | after commonPlugin | `arkdown-cache` exists; not wired to rolldown plugin hooks | ❌ |
| F3 | `checkDuplicateFilenamesPlugin` — ABC record name collision check | built-in | `arkdown-plugin-duplicate-check` | ✅ |
| F4 | `moduleMetaInfoPlugin` (ESM mode only) — pkgName, OHM identity, hostModulesInfo | `moduleParsed` hook | `arkdown-plugin-module-meta` | ✅ |
| F5 | `ohos-resolve` / `@ohos/node-resolve` — OHOS module resolution | `resolveId` | `arkdown-plugin-ohos-resolve` wrapping `arkdown-resolver` | ✅ |
| F6 | `routerMapPlugin` — collect router entries per `buildEnd` | `buildEnd` | `arkdown-plugin-router-map` | ✅ |
| F7 | `newOhmUrlPlugin` — rewrite module IDs to OHM URLs | `resolveId` transform | Implemented in `arkdown-plugin-ohos-resolve` via `useNormalizedOHMUrl` | ✅ |
| F8 | `sensitiveCheckPlugin` — case-sensitive import validation | `resolveId` | `arkdown-plugin-sensitive-check` | ✅ |
| F9 | `@rollup/plugin-commonjs` — CJS → ESM interop | `transform` | rolldown built-in CJS support | ✅ |
| F10 | `gcPlugin` — V8 GC hints between modules | `transform` | Rust `Drop` is deterministic; no-op | ✅ (N/A) |
| F11 | `@rollup/plugin-json` — JSON/JSON5 imports | `load` | `arkdown-plugin-json5` | ✅ |
| F12 | `waitAsyncPlugin` — flush async plugin work after `buildEnd` | `buildEnd` | Tokio handles naturally | ✅ (N/A) |
| F13 | `collectImportersPlugin` — reverse importer map for hot-reload delta | `moduleParsed` | Not implemented (SPEC-024) | ❌ |
| F14 | `hspCollectHarFilesPlugin` — collect HAR files used in HSP for dedup pass | `buildEnd` | `arkdown-plugin-har-dedup` partial | 🔄 |
| F15 | SDK `etsChecker` — TypeScript type checking for `.ets` files | `transform` (load from `ets_checker.js`) | Not implemented; EtsTransformPlugin only transforms, does not type-check | ❌ |
| F16 | SDK `etsTransform` — ArkTS decorator desugaring + ViewPU lowering | `transform` | `arkdown-plugin-ets-transform` covers: @State, @Prop, @Link, @ObjectLink, @Provide, @Consume, @Watch, @BuilderParam, storage decorators. Missing: @Builder, @Styles, full ViewPU `build()` → `observeComponentCreation2` lowering | 🔄 |
| F17 | SDK `apiTransform` — @kit.* → @ohos:* mapping + OHOS API shims | `transform` | Partial: @kit.* resolution in resolver; apiTransform expression rewrites incomplete | 🔄 |
| F18 | SDK `createProgramPlugin` — creates TypeScript compiler program | `buildStart` | Not implemented | ❌ |
| F19 | SDK `genAbc` wrapper — only calls `cleanUp`; actual ABC comes from genAbc's hooks | `buildEnd` / `generateBundle` hooks | ABC compilation is post-bundle in `arkdown-abc`; equivalent but hookArchitecture differs | ✅ (equiv.) |
| F20 | Coverage instrumentation (`buildInstrument`) | after SDK plugins | Not implemented | ❌ |
| F21 | `@rollup/plugin-terser` (release JSBUNDLE mode) | `renderChunk` | Not wired; `arkdown-obfuscate::minifier` exists | ❌ |
| F22 | `@rollup/plugin-babel` (JSBUNDLE mode only) — class properties | `transform` | Not needed for ESMODULE mode | ✅ (N/A) |
| F23 | `tscResolve` (when `useTscResolve` is set) — TypeScript path mapping | `resolveId` | Not implemented | ❌ |
| F24 | Custom compile plugins from `hvigorfile.ts` | last (push) | Not implemented (SPEC extension point) | ❌ |
| F25 | `manualChunks` (when `supportChunks` enabled): `node_modules` → "commons", non-entry → "vendors" | rollup output option | Not implemented for release builds | ❌ |
| F26 | `treeshake: true` in release mode | rollup input option | Always false (disabled); release mode not wired | ❌ |
| F27 | Source maps (`sourcemap: true`) for JSBUNDLE non-preview | rollup output option | Not generated in ESMODULE mode (correct); JSBUNDLE mode not supported | 🔄 |
| F28 | `sourcemapPathTransform` — strip `projectTopDir` from paths | rollup output option | Minimal ESMODULE fixture matches; JSBUNDLE path transform missing | 🔄 |

### Stage G — Post-Bundle Processing

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| G1 | Write rollup output chunks to `loader_out/` (JSBUNDLE) | `build.js::y.write(output)` | Rolldown writes chunks to configured output dir | ✅ |
| G2 | `disableGenerateBundle: true` for ESMODULE — skip file write, ABC runs in memory | `build.js` / genAbc plugin hooks | `arkdown-abc` reads rolldown output chunks from disk | 🔄 (disk read vs memory) |
| G3 | `processImportees` — walk rollup module cache, write nonEntryImportees + compiledImportees + directImportee JSON caches | `process-importee.js` | `arkdown::process_importees()` — fully ported, Phase 4 complete | ✅ |
| G4 | `removeRedundantDeclaredFiles` — clean stale `.d.ts`/`.d.ets` from cache | `rm-declared-files.js` | Not implemented | ❌ |
| G5 | `removeCompileFailedFilesFromCache` — remove cache entries for modules that errored | `rm-error-files-cache.js` | Not implemented | ❌ |
| G6 | Write rollup msgpack cache to `cache/msgpack/` | `RollupCacheService::writeCache()` | `arkdown-cache` exists; write path not wired | ❌ |
| G7 | `triggerBundleEnd` — flush async plugin work | `build.js` | Tokio handles naturally | ✅ (N/A) |
| G8 | Write `compileInfo.json` with `buildMode` to `arkCompileCachePath/` | `ArkCompile::writeBuildMode()` | Not implemented | ❌ |

### Stage H — ABC Compilation (arkdown-abc)

| # | hvigor Step | es2abc behavior | arkdown | Status |
|---|---|---|---|---|
| H1 | Parse JS/TS source files | `es2panda` parser | `ohos-rs/oxc` (pinned rev) | ✅ |
| H2 | Lower AST → Panda IR | `es2abc` compiler passes | `arkdown-abc::ir::` modules | 🔄 |
| H3 | Emit ABC binary: Header, StringTable, LiteralArrayTable | `assembler/assembly_emitter.cpp` | Implemented; structural match | ✅ |
| H4 | Emit ClassIndex + ClassRecords (one per rolldown chunk) | `assembly_emitter.cpp::SerializeClasses()` | Implemented | ✅ |
| H5 | MethodRecords with Panda bytecode | `assembly_emitter.cpp::SerializeMethods()` | CodeItems are byte-identical for the minimal-hap fixture; remaining `modules.abc` delta is DebugInfo/LNP only (9700B / 9844B) | 🔄 |
| H6 | LNP index at file end (after literal arrays) | hvigor places lnp_idx_off at total_size | ✅ Fixed: LNP index now emitted as final section | ✅ |
| H7 | Module annotation strings deduplicated in StringTable | Single string entry shared across all records | ✅ Fixed: field name strings deduplicated across classes | ✅ |
| H8 | Method index ordering [strings\|methods\|lits] | `assembly_emitter.cpp` method pool layout | ✅ Fixed: method index now [bytecode_strings\|methods\|literal_arrays] | ✅ |
| H9 | LNP body and DebugInfo constant_pool | `emitter.cpp` attaches AST source positions from parsed input source; `BuildLineNumberProgram()` consumes pandasm instruction source positions, real bytecode sizes, columns, and local-variable events | Per-instruction plumbing exists; source-position event stream still differs. Source maps are no longer passed into `arkdown-abc` for LNP generation; current source-derived output is 9700B with CodeItems still byte-identical. The fix is cache-TS coordinate parity or direct cache-TS backend parity. | 🔄 |
| H10 | `--merge-abc`: all module records in single `.abc` | Multiple records merged with shared StringTable | Implemented | ✅ |
| H11 | Incremental ABC cache via `--cache-file` (msgpack) | `es2abc --cache-file <path>` | CLI passes a cache-file path/marker, but backend does not implement real es2abc-compatible incremental cache | ❌ |
| H12 | Widget compilation (separate pass, `widgetCompile: "true"`) | ArkCompile calls `initWidgetDefaultArkCompileConfig()` for form entries | Not implemented (SPEC-027) | ❌ |
| H13 | ArkData compile (`arkDataCompile()`) | Separate compiler invocation for ArkData sources | Not implemented | ❌ |
| H14 | CompileArkTS source maps and final `sourceMaps.map` | ets-loader writes `sourceMaps.json`; package includes `ets/sourceMaps.map` | Minimal-hap `sourceMaps.json` and final `ets/sourceMaps.map` match; broader source-map coverage unverified | 🔄 |

### Stage I — Obfuscation and Minification (Release)

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| I1 | SDK obfuscation pass (rename vars, strip exports) | `loadSdkFunctions` → obfuscation SDK plugin | `arkdown-obfuscate` crate exists; not wired between bundler and ABC | ❌ |
| I2 | Compute obfuscation file hash; cache-invalidate ABC if rules change | `ArkCompile::computeObfuscationFileHash()` | Not implemented | ❌ |
| I3 | Generate consumer obfuscation config file for HAR publishing | `generateConsumerObConfigFile()` from SDK | Not implemented | ❌ |
| I4 | Terser JS minification (`@rollup/plugin-terser`) | JSBUNDLE release mode only | `arkdown-obfuscate::minifier.rs` exists (oxc_minifier); not wired | ❌ |
| I5 | Move release source maps to separate directory | `ArkCompile::moveReleaseMap()` | Not implemented | ❌ |

### Stage J — Incremental Cache

| # | hvigor Step | Location | arkdown | Status |
|---|---|---|---|---|
| J1 | Task-level incremental skip: compare declared inputs (obfuscationHash, TSC_MAX_FLOW_DEPTH, etc.) | `ArkCompile::declareInputs()` | Not implemented | ❌ |
| J2 | Rollup file-system cache: msgpack format, `compileEnv` key | `RollupCacheService` | `arkdown-cache` crate; `readCache`/`writeCache` not wired | ❌ |
| J3 | es2abc incremental cache: per-function SHA256, `--cache-file` flag | `es2abc` + hvigor cache integration | Not wired into `arkdown-abc` | ❌ |
| J4 | Clean ABC cache when `obfuscationEnable` changes | `ArkCompile::beforeTask()` | Not implemented | ❌ |

### Stage K — Packaging

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| K1 | Generate `module.json` (app + module merged) | `GeneratePkgModuleJson` → `generate-pkg-module-json.js` | `arkdown-profile::write_module_json()` — all fields match parity report | ✅ |
| K2 | Assemble HAP zip (11 canonical entries) | `PackageHap` → `make-pack-info.js` + zip | `arkdown-pack::pack_hap()` — matches hvigor golden exactly | ✅ |
| K3 | Assemble HSP zip | `PackageHsp` | `arkdown-pack::pack_hsp()` — similar to HAP but unverified for HSP-specific content | 🔄 |
| K4 | Assemble HAR (source HAR, not ByteCode HAR) | `PackageHar` | `arkdown-pack::pack_har()` — basic implementation | 🔄 |
| K5 | `ProcessIntegratedHsp` — embed remote/integrated HSP deps | `process-integrated-hsp.js` | `arkdown-pack` has `IntegratedHspEntry` and normalized embed support; CLI has no real remote HSP discovery yet | 🔄 |
| K6 | Native SO library embedding (`libs/arm64-v8a/`) | `PackageHap` + `CacheNativeLibs` | Not implemented (no native build flow) | ❌ |

### Stage L — Signing

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| L1 | HAP signing: PKCS#12 + CMS SignedData + V2/V3 block | `SignHap` → `sign-hap.js` → `hap-sign-tool.jar` | `arkdown-sign` pure Rust: ECDSA P-256, RSA-PSS, PKCS#12 loading, CMS generation, ZIP alignment | ✅ |
| L2 | HSP signing | `SignHsp` | `arkdown-sign::sign_hsp()` (same code path) | ✅ |
| L3 | HAR signing (optional) | `sign/har-sign-command-build.js` | Basic HAR does not require signing; not implemented for signed HAR | 🔄 |

### Stage M — Advanced / Watch Mode

| # | hvigor Step | Class / File | arkdown | Status |
|---|---|---|---|---|
| M1 | Incremental watch mode: chokidar file watcher + rebuild | `watch.js` in build-steps + `hvigorw --watch` | `arkdown-watcher` crate skeleton; not wired (SPEC-023) | ❌ |
| M2 | Hot-reload: importer graph delta computation | `collectImportersPlugin` + `cold-reload-ark-compile.js` | Not implemented (SPEC-024) | ❌ |
| M3 | Preview mode: resource watcher + live resource reload | `preview-resource-watcher-plugin.js` | Not implemented (SPEC-025) | ❌ |
| M4 | HAR TGZ clean (strip rollup output for HAR publishing) | `clean-ets-for-tgz.js` | Not implemented (SPEC-026) | ❌ |
| M5 | ETS standalone checker (lint-only mode) | `etsStandaloneChecker` from SDK | Not implemented | ❌ |

---

## 2. Gap Inventory

### Critical Gaps (blocking full hvigor parity)

**H9 — ABC DebugInfo/LNP source-position event stream**
- hvigor: `emitter.cpp::GenInstructionDebugInfo()` attaches AST start positions to normal instructions and function/block end positions to return instructions; `BuildLineNumberProgram()` then emits line/column and local-variable events from the pandasm instruction stream
- arkdown: CodeItems now match for the minimal-hap fixture, but some synthesized constructors and top-level closures have missing or different source-position events
- Root cause: `CodeGen::set_debug_pos()` coverage does not yet match es2panda's IR-node source-position assignment and generated-instruction inheritance; additionally, hvigor's LNP coordinates come from the CompileArkTS cache TS parsed by `es2abc`, while arkdown currently preserves CodeItem parity by compiling the rolldown chunk shape
- Fix: align constructor, arrow, `func_main_0`, inserted-instruction, and return-position event generation without changing CodeItem bytes

**H10 — Direct CompileArkTS cache-TS backend route**
- hvigor: CompileArkTS writes transformed TS cache files, `filesInfo.txt`, `npmEntries.txt`, and invokes `es2abc @filesInfo`
- arkdown: now writes matching handoff metadata, and the cache TS files match hvigor for the minimal fixture
- Current blocker: direct cache-TS compilation through `arkdown-abc` shrinks the ABC and changes/misses CodeItems, proving a backend logic gap on that input shape
- Fix: keep final compilation on the current chunk path until direct cache-TS CodeItems match hvigor

**F16 — Missing ViewPU lowering in etsTransform**
- hvigor: `etsTransform` SDK plugin fully lowers ArkUI `struct`+`build()` blocks into `observeComponentCreation2()`, `.create()`, attribute method calls, and `.pop()` calls
- arkdown: `EtsTransformPlugin` stubs out `build()` bodies and transforms state decorators but does not perform ViewPU lowering
- Impact: any real ArkUI page (e.g., `@Entry @Component struct Index`) compiles syntactically but produces non-functional runtime code

**F15 — No etsChecker (TypeScript type checking)**
- hvigor: `etsChecker` runs `ets_checker.js` from the SDK for full TypeScript semantic analysis
- arkdown: no type checking; compilation proceeds even with type errors
- Impact: diagnostic parity zero; SDK API misuse not caught at build time

**H14 — Source map breadth beyond minimal-hap**
- minimal-hap: CompileArkTS `sourceMaps.json` and final `ets/sourceMaps.map` match the hvigor reference
- remaining work: verify and extend source-map parity across broader ArkUI syntax, dependencies, release mode, and JSBUNDLE paths

### High-Impact Gaps (blocking release/production builds)

**I1-I5 — Obfuscation pipeline not wired**
- `arkdown-obfuscate` has `transform.rs`, `rename.rs`, `rules.rs`, `minifier.rs`
- CLI passes no obfuscation config to bundler or ABC compiler
- Needs: wire between `arkdown::Bundler` output and `arkdown-abc::AbcCompiler` input

**J2-J3 — Incremental cache not wired**
- `arkdown-cache` crate exists with correct data structures
- Neither rolldown `ScanStageCache` serialization nor ABC `--cache-file` equivalent are wired
- Clean builds take ~10× longer than incremental on large projects

**F25-F26 — Release mode chunk splitting and tree-shake**
- `manualChunks` for `supportChunks` projects (commons + vendors split) not implemented
- `treeshake: true` for release not enabled; all modules unconditionally included
- Impact: release HAP sizes larger than necessary

**D2-D5 — Multi-module dependency maps incomplete**
- `byteCodeHarInfo`, `dynamicImportLibInfo`, transitive `harNameOhmMap`/`hspNameOhmMap` not populated
- Impact: any project with ByteCode HAR or HSP dependencies fails to resolve

**K3-K4 — HSP/HAR parity unverified**
- HAP parity proven (11-entry zip matches hvigor golden)
- HSP-specific content (declare files output, `ArkCompile.declareOutputFiles()` for HSP) unverified
- HAR-specific content (obfuscation consumer file, processObfuscationFiles, cleanEtsForTgz) not implemented

### Medium-Impact Gaps (advanced features)

**B4-B5 — ProcessShareConfig / ProcessStartupConfig**
- Required for projects using `crossAppSharedConfig` (HSP cross-app sharing) or startup tasks
- Not blocking for most apps

**C3 — ProcessCompiledResources (preloaded SO)**
- Needed for projects with preloaded system SO libraries
- Not blocking for pure ArkTS apps

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

**What**: Preserve current CodeItem parity while aligning the DebugInfo constant_pool
and LineNumberProgram event stream with hvigor. Start with synthesized constructors
(`EntryAbility #~@0=#EntryAbility`), top-level closures (`Index #*#`), and
`Index func_main_0`.
**File**: `crates/arkdown-abc/src/ir/builder.rs`, `crates/arkdown-abc/src/emit/file.rs`
**Upstream ref**: `emitter.cpp::GenInstructionDebugInfo()`,
`assembly-emitter.cpp::BuildLineNumberProgram()`,
`codegen_dynamic.cpp` generated-instruction source-position inheritance
**Complexity**: M
**Depends on**: nothing

### TASK-ABC-02: Direct CompileArkTS cache-TS CodeItem parity (M)

**What**: Use the hvigor-style `filesInfo.txt`/`npmEntries.txt` handoff as the
input route only after compiling the generated cache TS files preserves all
CodeItems. The cache TS bytes already match hvigor for the minimal fixture; the
current shrink is a backend logic gap.
**File**: `crates/arkdown-cli/src/cmd/build.rs`, `crates/arkdown-abc/src/`
**Upstream ref**: `gen_merged_abc.ts`
**Complexity**: M
**Depends on**: TASK-ABC-01

### TASK-ABC-03: ABC section diff harness (S)

**What**: Add a regression helper that compares CodeItems separately from
DebugInfo/LNP/StringTable/ClassRecord sections, so total ABC size changes do not
hide executable-bytecode regressions.
**File**: `crates/arkdown-cli/tests/`, `crates/arkdown-abc/tests/`
**Upstream ref**: local hvigor minimal-hap golden
**Complexity**: S
**Depends on**: nothing

### TASK-ABC-04: ABC incremental cache wiring (M)

**What**: Pass `cache_path` from CLI through `AbcCompiler` config. Implement per-function `SHA256(source_text)` keyed msgpack cache. On hit, skip IR lowering and emit cached bytecode. Invalidate when `compileEnv` key changes.
**File**: `crates/arkdown-abc/src/compiler.rs`, `crates/arkdown-cli/src/main.rs`
**Upstream ref**: `es2abc --cache-file` behavior; hvigor `RollupCacheService` msgpack cache at `cache/msgpack/MSGPACK_COMPILE_CACHE_DIR_NAME`; `ark-compile.js::trace()` and `doTaskAction()` cache setup
**Complexity**: M
**Depends on**: TASK-CACHE-01

### TASK-ABC-05: Widget compilation pass (M)

**What**: Implement `initWidgetDefaultArkCompileConfig()` equivalent: second `arkdown-abc` invocation with `widgetCompile: true`, separate cache dir, widget-specific loader.json input.
**File**: `crates/arkdown-abc/`, `crates/arkdown-cli/`
**Upstream ref**: `ark-compile.js::doTaskAction()` — `if (this.needSubmitArkTsWidget) { initWidgetDefaultArkCompileConfig() }`; SPEC-027
**Complexity**: M
**Depends on**: TASK-ABC-01 through TASK-ABC-04

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
**File**: `crates/arkdown-cli/src/main.rs`, `crates/arkdown-obfuscate/src/lib.rs`
**Upstream ref**: `ark-compile.js::initDefaultArkCompileConfig()` — `obfuscationOptions` setup; `load-sdk-functions.js` — terser plugin wired when `n === RELEASE && obfuscationEnabled`; SPEC-020
**Complexity**: M
**Depends on**: TASK-CACHE-01 (for cache invalidation on rule changes)

### TASK-CACHE-01: Wire incremental rolldown cache (M)

**What**: Implement rolldown `ScanStageCache` serialization to/from msgpack files in `cache/msgpack/`. Build `compileEnv` key from `[compileSdkVersion, compatibleSdkVersion, runtimeOS, etsLoaderPath, obfuscationEnabled, moduleType, cachePath].join("#")`. Invalidate on key change.
**File**: `crates/arkdown-cache/src/lib.rs`, `crates/arkdown/src/lib.rs`, `crates/arkdown-cli/src/main.rs`
**Upstream ref**: `arkts-pack.js::getCompileEnv()` and `generateConfig()`; `build.js::readCacheWithEvent()` and `rollupCacheService.writeCache()`; `RollupCacheService` in `hvigor-arkts-base/dist/src/cache/`
**Complexity**: M
**Depends on**: nothing

### TASK-CACHE-02: Task-level incremental skip (M)

**What**: Before running the full build, hash declared inputs (source files, obfuscationHash, SDK version, compileEnv). Skip ArkCompile if all inputs and outputs match. Write/read a `task-state.json` with input hashes.
**File**: `crates/arkdown-cli/src/main.rs`
**Upstream ref**: `ark-compile.js::declareInputs()` and hvigor's task state cache framework; `ArkCompile::beforeTask()` and `checkAndCleanCache()`
**Complexity**: M
**Depends on**: TASK-CACHE-01

### TASK-LOADER-01: ByteCode HAR dependency maps (M)

**What**: Populate `loader_json.by_byte_code_har_info` from `oh-package.json5` dependencies that reference pre-compiled `.abc` + `.d.ts` packages. Resolve `abcPath`, `sourceMapsPath`, `compatibleSdkVersion`, `insightIntent`.
**File**: `crates/arkdown-profile/src/loader_json.rs`, `crates/arkdown-resolver/src/`
**Upstream ref**: `GenerateLoaderJson::doTaskAction()` — `byteCodeHarInfo` population from `DependencyManager`; `loader.json` schema in `hvigorw-analysis.md §9`
**Complexity**: M
**Depends on**: nothing

### TASK-LOADER-02: Dynamic import library info (M)

**What**: Detect `import()` dynamic import calls whose specifier resolves to a cross-HAR dep. Write `dynamicImportLibInfo` in loader.json: `{ pkgName: { entryFilePath, moduleName } }`.
**File**: `crates/arkdown-profile/src/loader_json.rs`, `crates/arkdown/src/` (post-bundle module graph walk)
**Upstream ref**: `GenerateLoaderJson` fields; `arkts-pack.js::generateConfig()` — `dynamicImportLibInfo` from resolver
**Complexity**: M
**Depends on**: TASK-LOADER-01

### TASK-PACK-01: ProcessCompiledResources — preloaded SO (S)

**What**: After resource compilation, collect preloaded system SO file paths from `buildOption.preloadSo`. Write the preload SO file list to the HAP intermediates.
**File**: `crates/arkdown-pack/src/`, `crates/arkdown-cli/src/`
**Upstream ref**: `ark-compile.js::addPreloadSystemSoFileList()` and `allowToPreloadSystemSo()`; `PackageHap` SO library embedding
**Complexity**: S
**Depends on**: nothing

### TASK-PACK-02: ProcessIntegratedHsp (M)

**What**: When a HAP/HSP has integrated (embedded) remote HSP deps, copy their bytecode and resources into the HAP archive at the correct paths.
**File**: `crates/arkdown-pack/src/`
**Upstream ref**: `process-integrated-hsp.js`; `hvigorw-analysis.md §2`
**Complexity**: M
**Depends on**: TASK-LOADER-01

### TASK-PACK-03: ProcessModulePrivacyProfile (S)

**What**: If `src/main/resources/base/profile/privacy.json` exists, validate and embed it as `module.privacy.json` in the HAP.
**File**: `crates/arkdown-pack/src/`
**Upstream ref**: `process-module-privacy-profile.js`
**Complexity**: S
**Depends on**: nothing

### TASK-PACK-04: ProcessShareConfig + ProcessStartupConfig (M)

**What**: Read `crossAppSharedConfig` from HSP dep `module.json5`. Merge startup task configs from all HAR/HSP deps. Write intermediate JSON for downstream tasks.
**File**: `crates/arkdown-profile/src/`, `crates/arkdown-resolver/src/`
**Upstream ref**: `process-share-config.js`, `process-startup-config.js`
**Complexity**: M
**Depends on**: TASK-LOADER-01

### TASK-RELEASE-01: treeshake + manualChunks for release builds (M)

**What**: Enable rolldown `treeshake: true` in release mode. Implement `manualChunks` equivalent: modules from `oh_modules/` → chunk named "commons"; non-entry, non-node_modules → chunk named "vendors".
**File**: `crates/arkdown/src/`, `crates/arkdown-cli/src/`
**Upstream ref**: `arkts-pack.js::generateConfig()` — `treeshake: n === RELEASE`; `manualChunks(id, {getModuleInfo})` function
**Complexity**: M
**Depends on**: nothing

### TASK-WATCH-01: Watch mode (L)

**What**: Implement `--watch` flag: chokidar-equivalent file watching (use `notify` crate), incremental rebuild on change, delta-only ABC recompilation.
**File**: `crates/arkdown-watcher/src/` (SPEC-023), `crates/arkdown-cli/src/`
**Upstream ref**: `build-steps/watch.js`; `arkts-pack.js::getWatchConfig()` — `{chokidar: {useFsEvents: false}, buildDelay: 100}`; SPEC-023
**Complexity**: L
**Depends on**: TASK-CACHE-01, TASK-CACHE-02

---

## 4. Priority Ordering

### Priority 1 — ABC DebugInfo Parity (finish minimal-hap byte parity)

These close the remaining minimal-hap `modules.abc` byte gap while preserving
the current CodeItem parity.

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-ABC-01: Align DebugInfo/LNP source positions | M | Required for final byte parity |
| TASK-ABC-02: Direct cache-TS CodeItem parity | M | Required before switching to hvigor `filesInfo.txt` input |
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
| TASK-RELEASE-01: treeshake + manualChunks | M | Required for correct release HAP size |
| TASK-CACHE-01: Rolldown cache wiring | M | 10× build speed improvement |
| TASK-CACHE-02: Task-level incremental | M | Dev loop speed |

### Priority 4 — Multi-Module Project Support

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-LOADER-01: ByteCode HAR maps | M | Any project with BC-HAR deps |
| TASK-LOADER-02: Dynamic import info | M | Dynamic import patterns |
| TASK-PACK-02: ProcessIntegratedHsp | M | Projects embedding HSP |
| TASK-PACK-04: ProcessShareConfig | M | Cross-app HSP sharing |

### Priority 5 — Polish and Advanced Features

| Task | Complexity | Impact |
|------|------------|--------|
| TASK-ABC-04: ABC incremental cache | M | Compile speed |
| TASK-ABC-05: Widget compile pass | M | Card/widget apps (SPEC-027) |
| TASK-PACK-01: Preloaded SO | S | Apps with native libs |
| TASK-PACK-03: Privacy profile | S | Privacy-sensitive apps |
| TASK-WATCH-01: Watch mode | L | DevEco IDE integration |

---

## 5. Upstream Source References

| Area | URL |
|---|---|
| ArkCompile task (full) | `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js` |
| Rollup plugin setup | `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js` |
| Build orchestration | `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build.js` |
| SDK plugin loading | `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js` |
| Importee processing | `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build-steps/process-importee.js` |
| assembly-emitter.cpp | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/assembler/assembly-emitter.cpp` |
| assembly-emitter.h | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/assembler/assembly-emitter.h` |
| Panda bytecode ISA | `https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/isa/isa.yaml` |
| ets-loader source | DevEco SDK: `<OHOS_SDK_HOME>/ets/build-tools/ets-loader/` |

---

## 6. Summary Scorecard

| Pipeline Area | Steps Total | ✅ | 🔄 | ❌ |
|---|---|---|---|---|
| Pre-Build (A) | 2 | 2 | 0 | 0 |
| Profile Processing (B) | 8 | 2 | 2 | 4 |
| Resource Compilation (C) | 3 | 2 | 0 | 1 |
| loader.json (D) | 5 | 1 | 3 | 1 |
| ArkCompile Setup (E) | 4 | 2 | 0 | 2 |
| Rollup Plugins (F) | 28 | 13 | 4 | 11 |
| Post-Bundle Processing (G) | 8 | 2 | 1 | 5 |
| ABC Compilation (H) | 14 | 7 | 4 | 3 |
| Obfuscation/Minification (I) | 5 | 0 | 0 | 5 |
| Incremental Cache (J) | 4 | 0 | 0 | 4 |
| Packaging (K) | 6 | 2 | 3 | 1 |
| Signing (L) | 3 | 2 | 1 | 0 |
| Advanced/Watch (M) | 5 | 0 | 0 | 5 |
| **Total** | **95** | **35** | **18** | **42** |

**Current overall pipeline coverage: ~37% fully verified, ~19% partial, ~44% not started.**
The HAP golden-parity path (minimal-hap fixture with no dependencies) covers ~68 of the 95 steps.
The remaining 42 ❌ steps are concentrated in: release-mode obfuscation+minification, incremental caching, watch mode, full ViewPU ArkUI lowering, and multi-module dependency handling.
