# arkdown Build Pipeline

This document describes the complete end-to-end build pipeline for arkdown, a Rust reimplementation of the hvigor/rollup-based build system for OpenHarmony OS (OHOS) projects.

---

## Overview

arkdown replaces hvigor's Node.js/rollup pipeline with a pure Rust implementation. The pipeline transforms `.ets`/`.ts` source files into a signed `.hap`/`.hsp`/`.har` artifact ready for deployment on ArkRuntime.

hvigor uses rollup 3 internally (via `@ohos/hvigor-arkts-compose` which depends on `rollup@3.19.1-htrunk1.AllScenario.devecostudio.r8`). arkdown replaces this entire rollup pipeline with rolldown-derived Rust stages.

---

## Phase 1: Config Parsing

**Crate**: `arkdown-config`
**Inputs**: `AppScope/app.json5`, `build-profile.json5`, `oh-package.json5`, `module.json5`, `hvigorfile.ts`
**Outputs**: `ArkdownConfig` — fully resolved build configuration struct

> **Key discovery**: `bundleName` comes from `AppScope/app.json5` (`app.bundleName`), NOT from
> `build-profile.json5` products. `signingConfigs` lives at `build-profile.json5#app.signingConfigs`
> and is referenced from products by name via `signingConfig: "default"`. Use `load_app_json5()`
> to read `AppScope/app.json5`. `targetSdkVersion` may appear alongside `compatibleSdkVersion`
> and uses the same `"X.Y.Z(N)"` format.

### hvigor behavior

hvigor reads `build-profile.json5` at the project root for products, modules, and signing configs. Each module has a `srcPath` pointing to a subdirectory containing its own `oh-package.json5` and `module.json5`. The `hvigorfile.ts` registers the `appTasks` plugin:

```typescript
// hvigorfile.ts
import { appTasks } from "@ohos/hvigor-ohos-plugin";
export default { system: appTasks, plugins: [] };
```

`build-profile.json5` example (from `MyApplication`):

```json5
{
  app: {
    products: [
      {
        name: "default",
        compatibleSdkVersion: "5.0.0(12)",
        runtimeOS: "HarmonyOS",
        buildOption: {
          strictMode: { caseSensitiveCheck: true, useNormalizedOHMUrl: true },
        },
      },
    ],
    buildModeSet: [{ name: "debug" }, { name: "release" }],
  },
  modules: [
    {
      name: "entry",
      srcPath: "./entry",
      targets: [{ name: "default", applyToProducts: ["default"] }],
    },
  ],
}
```

### rolldown equivalent

rolldown reads a `rolldown.config.js` (or `.ts`) file. arkdown uses JSON5 directly — no JavaScript config file is required.

### OHOS divergences

- `compatibleSdkVersion` drives `--target-api-version` for `es2abc`
- `useNormalizedOHMUrl` enables OHM URL normalization (see [module-resolution.md](module-resolution.md))
- `signingConfigs` reference `.p12`/`.p7b`/`.cer` material for HAP signing
- Multiple `products` × `modules` × `targets` create a matrix of build outputs

---

## Phase 2: Dependency Resolution

**Crate**: `arkdown-resolver` (wraps / replaces OHOS-specific logic from `@ohos/hvigor-arkts-compose` resolve plugins)
**Inputs**: `oh-package.json5` files across all modules and their transitive dependencies
**Outputs**: `DependencyGraph` — resolved map of package names to local paths or ohpm cache entries

### hvigor behavior

`GenerateLoaderJson` task reads `oh-package.json5` from every module and from the project root. It builds `hspNameOhmMap` and `harNameOhmMap` — mappings from dependency logical names to OHM URLs. The `DependencyManager` class traverses transitive dependencies, distinguishing:

- Local HAR (path reference via `file:../other-module`)
- Remote HAR (ohpm registry)
- HSP (shared package, referenced by bundle+module name)
- ByteCode HAR (pre-compiled `.abc` + `.d.ts` only)

### rolldown equivalent

rolldown's `SharedResolver` is built once per bundler session (bundler-level lifecycle, as per rolldown's `bundler-data-lifecycle.md`). arkdown follows the same pattern — the resolver is constructed from the dependency graph and cached across incremental builds.

### OHOS divergences

- Resolution uses `oh-package.json5` instead of `package.json`
- Bare specifiers like `@ohos/xxx` and `@kit/xxx` resolve to SDK paths (see [module-resolution.md](module-resolution.md))
- `@bundle:bundlename/modulename/path` is an absolute cross-bundle reference
- ByteCode HAR deps skip source loading entirely

---

## Phase 3: Module Loading (Parse)

**Crate**: `arkdown-loader` + `ohos-rs/oxc`
**Inputs**: `.ets`, `.ts`, `.js`, `.json5` files; OHM URL specifiers
**Outputs**: parsed AST + module metadata per file

### hvigor behavior

The rollup pipeline loads files through its built-in load hook chain. SDK plugins (loaded from `ets-loader`) include:

- `etsChecker` — TypeScript type-checking for `.ets`
- `etsTransform` — ArkTS-specific transforms (decorators like `@Component`, `@State`, `@Entry`)
- `apiTransform` — OHOS API transforms
- `createProgramPlugin` — creates the TypeScript compiler program

These are loaded dynamically from `e.etsLoaderPath` (e.g., `sdk/default/openharmony/ets/build-tools/ets-loader`).

### rolldown equivalent

rolldown's `module_loader` stage handles parallel module loading. Each module goes through: resolve → load → transform → parse → `moduleParsed` hook. arkdown follows the same hook sequence but Rust-native.

### OHOS divergences

- `.ets` files contain ArkTS syntax (struct-based components, decorator metadata). oxc must handle the ArkTS superset grammar — or a pre-transform stage must desugar before standard TS parsing.
- JSON5 is loaded natively (not via `@rollup/plugin-json`).
- `.d.ets`/`.d.ts` declaration files are loaded for type information but excluded from output chunks.

---

## Phase 4: Transform (ArkTS Desugar)

**Crate**: `arkdown-plugin-ets-transform` (pure Rust port of SDK ets-loader; see SPEC-004a)
**Inputs**: parsed ArkTS AST
**Outputs**: standard TypeScript / JavaScript AST

### hvigor behavior

The `etsTransform` SDK plugin applies decorator desugaring. `@Component` structs become class-like constructs. `@Entry` marks ability entry points. `@State`, `@Prop`, `@Link` decorators are desugared to runtime state management calls.

### rolldown equivalent

rolldown's transform stage (the `transform` plugin hook). arkdown calls these transforms via the plugin system.

### OHOS divergences

- ArkTS decorators have OHOS-specific semantics that standard Babel/oxc decorators do not model. `arkdown-plugin-ets-transform` ports all three transform passes (`etsTransform`, `etsChecker`, `apiTransform`) in pure Rust using `ohos-rs/oxc`. No subprocess invocation, no SDK dependency.
- The `useTscResolve` flag enables `tscResolve` plugin for TypeScript path mapping resolution.

---

## Phase 5: Module Graph Build

**Crate**: `arkdown` (core, rolldown-derived)
**Inputs**: transformed modules with import/export edges
**Outputs**: `ModuleGraph` — directed graph of all modules and their dependency edges

### hvigor behavior

rollup's standard module graph construction. The `moduleMetaInfoPlugin` annotates each module with:

- `pkgName`, `pkgVersion`
- `moduleName` (OHOS module name, not npm package)
- `isLocalDependency`
- `isNodeEntryFile`
- `belongModulePath`, `belongProjectPath`
- `hostModulesInfo` — reverse-dependency tracking

`collectImportersPlugin` builds a reverse importer map for hot-reload delta computation.

### rolldown equivalent

rolldown's `stages/scan` stage: `ast_scanner` walks each module's AST extracting imports/exports, `module_loader` resolves them in parallel, building the `IndexVec<ModuleIdx, NormalModule>` graph structure.

### OHOS divergences

- Module identity uses OHM URLs (`@bundle:xxx/yyy/zzz`) rather than file paths when `useNormalizedOHMUrl` is true.
- ByteCode HAR modules are external — their `.abc` file is referenced, not their source.
- `dynamicImportLibInfo` tracks cross-HAR dynamic imports for the loader JSON.

---

## Phase 6: Link + Tree-Shake

**Crate**: `arkdown` (core)
**Inputs**: `ModuleGraph`
**Outputs**: linked module graph with dead code marked, export bindings resolved

### hvigor behavior

rollup's standard linking phase. `treeshake` is enabled only in release mode (`n === RELEASE`). In debug mode, all modules are included.

```javascript
// from arkts-pack.js
treeshake: n === constants_js_1.RELEASE,
```

### rolldown equivalent

rolldown's `stages/link` stage: resolves cross-module references, computes `side_effects`, marks included statements, resolves CJS namespace objects.

### OHOS divergences

- OHOS release builds always tree-shake (removes unused exports from HAP/HSP).
- HAR builds in debug mode still include all exports for type declaration generation.

---

## Phase 7: Chunk Split

**Crate**: `arkdown` (core, code_splitting)
**Inputs**: linked module graph, entry points from `module.json5` (`srcEntry` + ability entries)
**Outputs**: `ChunkGraph` — assignment of modules to chunks

### hvigor behavior

rollup's `manualChunks` function:

```javascript
function manualChunks(e, { getModuleInfo: t }) {
  if (e.includes(constants_js_1.NODE_MODULES) || e.includes(constants_js_1.OH_NODE_MODULES))
    return "commons";
  const s = t(e);
  return s && !s.isEntry ? "vendors" : void 0;
}
```

This is only active when `supportChunks` is enabled. When disabled (the default for most modules), a single output chunk is produced per module entry.

Entry points come from `loader.json` (`compileEntry` array in `GenerateLoaderJson`), which includes:

- `module.json5#abilities[*].srcEntry` (e.g., `./ets/entryability/EntryAbility.ets`)
- `module.json5#module.srcEntry`
- Worker files listed in `buildOption.sourceOption.workers`
- `@Entry`-decorated router map source files

### rolldown equivalent

rolldown's `generate_chunks()` in `code_splitting.rs`: BitSet-based reachability assigns each module to a chunk based on which entry points can reach it. arkdown uses this same approach.

### OHOS divergences

- Each HAP module produces a distinct set of chunks (not one global bundle).
- HAR modules may produce a single merged `.abc` file.
- `supportChunks` flag (from `projectConfig`) controls whether multiple output chunks are produced or everything is inlined.

---

## Phase 8: IIFE Chunk Wrap

**Crate**: `arkdown-plugin-chunk-wrapper`
**Inputs**: rolldown output chunks (CJS format)
**Outputs**: IIFE-wrapped JS with `globalThis[name_hash]` cross-chunk registry

### hvigor behavior

The `wrapperChunkPlugin` (`wrapper-chunk-plugin.js`) implements the `renderChunk` hook:

```javascript
function renderChunk(code, chunk, options, {chunks}) {
    const s = new MagicStringReplacement(code);
    if (projectConfig.supportChunks) {
        analyzeImportAndExport(this.parse, chunk, code, s, chunks);
    }
    s.appendLeft(0, "(function () {\n");
    s.append("\n})()");
    return { code: s.toString(), map: ... };
}
```

When `supportChunks` is enabled:

- `require('chunkId')` calls are replaced with `globalThis['chunkFileName_chunkHash']`
- Each chunk exports via: `globalThis['fileName_chunkHash'] = globalThis['fileName_chunkHash'] || { export1: exports.export1, ... }`

This is required because ArkRuntime does not support native ESM `import`/`export` at the JS level. The bundled output is consumed by `es2abc` before it reaches the runtime. See [chunk-model.md](chunk-model.md) for full details.

### rolldown equivalent

rolldown's `render_chunk` plugin hook. arkdown implements `arkdown-plugin-chunk-wrapper` as a built-in Rust plugin implementing this hook.

### OHOS divergences

This is entirely OHOS-specific. Standard web bundlers do not need this IIFE wrapper.

---

## Phase 9: es2abc Compilation

**Crate**: `arkdown-abc` (pure Rust, ports `arkcompiler_ets_frontend`)
**Inputs**: IIFE-wrapped JS chunks (`.js` files in `aceModuleBuild/`)
**Outputs**: `.abc` ArkCompiler bytecode files

### hvigor behavior

`ArkCompile` task calls `submitArkCompileWork` → `run-ark.js` worker pool → invokes
`es2abc` binary (closed-source). Key behaviors:

- Merges multiple chunk records into one `.abc` per module (`--merge-abc`)
- Incremental cache keyed by input file hashes (`--cache-file` msgpack format)
- Parallel compilation: one worker thread per module
- Target API version from `compatibleSdkVersion` (`--target-api-version`)
- Debug builds include source positions (`--debug-info`)

### arkdown replacement

`arkdown-abc` reimplements this pipeline in Rust:

- **TypeScript stripping**: before IR lowering, `oxc_codegen` strips any remaining TS type
  annotations from the IIFE JS. Even post-bundle, type syntax can survive into chunk JS.
  This produces clean JavaScript for the Panda IR builder.
- Reuses `ohos-rs/oxc` to parse the stripped JS input
- Ports the IR lowering and Panda bytecode emit stages from `arkcompiler_ets_frontend`
- Parallel compilation via Rayon (one task per module)
- Same incremental cache semantics (function-level, msgpack)
- **Panda magic**: `.abc` files begin with `b"PANDA\x00\x00\x00"` (8 bytes)

See [abc-pipeline.md](abc-pipeline.md) for the complete Panda IR and bytecode format.

---

## Phase 10: Resource Compilation (restool)

**Crate**: `arkdown-resource`
**Inputs**: `src/main/resources/` directory, `module.json5`, router map JSON, startup config
**Outputs**: `resources.index`, `ResourceTable.txt`, compiled binary resource files

### hvigor behavior

`CompileResource` task builds a `resConfig.json` (pointing to resource roots) and invokes `restool` (at `sdk/default/openharmony/toolchains/restool`). The command is assembled by `restoolConfigBuilder` and executed via `executeCommand`. Post-compilation, it processes:

- Router map: writes `$profile:routerMap` reference into `module.json`
- Share config: writes `$profile:crossAppSharedConfig`
- Startup config: writes `$profile:startup_config`
- Distribution filter: writes distro filter profile JSON
- Shortcut bundleName override

### arkdown replacement

`arkdown-resource` reimplements restool in pure Rust, porting from
`https://gitcode.com/openharmony/developtools_restool`. No subprocess invocation.
All resource scanning, ID assignment, and binary serialization runs in-process.

---

## Phase 11: module.json5 Merge / Process Profile

**Crate**: `arkdown-profile`
**Inputs**: module `module.json5`, `AppScope/app.json5`, HAR dependency `module.json5` files, product config
**Outputs**: processed `module.json` placed in `intermediates/res/<target>/`

### hvigor behavior

`MergeProfile` task merges base `module.json5` with product overlays. `ProcessProfile` task:

1. Sets `module.virtualMachine` = `"ark<apiVersion>.0.1.0"` (for ArkTS stage model modules, API >= 9)
2. Sets `module.compileMode` = `"esmodule"` (stage model) or `"jsbundle"` (FA model)
3. Sets `module.deviceTypes`
4. Collects abilities from HAR dependencies (`collectHarAbilities`) and merges into `module.abilities`
5. Generates OHM URL for HAR ability `srcEntry` fields: `@bundle:bundleName/harModuleName/path`

### OHOS divergences

The `virtualMachine` field is critical — it tells the OS runtime which VM to use. For ArkTS stage
model modules with API >= 9, it is `"ark<apiVersion>.0.1.0"` (e.g., `"ark12.0.1.0"` for API 12).

**module.json must have both `app` and `module` sections.** `write_module_json()` merges:

- `app` section from `AppScope/app.json5` (bundle name, version, icon, label)
- `module` section from processed `module.json5` (with virtualMachine, compileMode added)

The merged file is placed in intermediates and also embedded directly in the HAP zip.

---

## Phase 12: HAP/HSP/HAR Packaging

**Crate**: `arkdown-pack`
**Inputs**: compiled `.abc`, compiled resources, processed `module.json`, native `.so` libraries
**Outputs**: unsigned `.hap`, `.hsp`, or `.har` archive (zip format)

### hvigor behavior

`PackageHap` task (or `PackageHsp`, `PackageHar`) assembles the output package. The HAP structure is a zip containing:

```
module.json               ← app + module sections merged (from write_module_json())
pack.info                 ← bundle summary JSON
pkgContextInfo.json       ← package context OHM URL mappings
resources.index           ← binary resource index (RestoolV2 magic)
ets/
  modules.abc             ← merged Panda bytecode (PANDA magic, API >= 9)
resources/
  base/
    element/              ← string, color, etc. resource files
    media/                ← image resources
    profile/              ← profile JSON files (e.g., main_pages.json)
libs/arm64-v8a/*.so       ← optional native libs
```

`PackageHap` extends `BasePackHapTask` which handles the zip assembly using streams. The
`ProcessCompiledResources` task runs before packaging to process any preloaded SO files.

For HAR builds, use `pack_har()` (not `pack_hap()`). Resources are optional: if
`src/main/resources/` is absent, an empty placeholder is created.

### OHOS divergences

HAP/HSP/HAR are OHOS-specific archive formats. HAR is a library archive containing `.d.ts` + `.d.ets` declarations plus either source or `.abc` bytecode. HSP is a shared package deployed once on-device and referenced by multiple HAPs.

---

## Phase 13: Signing

**Crate**: `arkdown-sign`
**Inputs**: unsigned `.hap`/`.hsp`, signing material (`.p12` keystore, `.p7b` profile, `.cer` certificate)
**Outputs**: signed `.hap`/`.hsp`

### hvigor behavior

`SignHap` task uses the Huawei signing tool (hap-sign-tool) to sign the HAP. Signing credentials come from `build-profile.json5#app.signingConfigs`.

---

## Pipeline Summary (HAP build)

```
Config Parsing
    │
    ▼
Dependency Resolution  ──── oh-package.json5 graph
    │
    ▼
PreBuild               ──── GeneratePkgContextInfo, MergeProfile
    │
    ├─► ProcessProfile (module.json5 merge + ability collection)
    │
    ├─► GenerateLoaderJson (loader.json — entry manifest for rollup)
    │       depends on: ProcessRouterMap, ProcessShareConfig, ProcessStartupConfig
    │
    ├─► CompileResource (restool invocation)
    │       depends on: ProcessResource
    │
    ▼
ArkCompile             ──── rollup (rolldown in arkdown) + wrapper-chunk-plugin
    │   depends on: GenerateLoaderJson, CompileResource, CreateBuildProfile
    │
    ├─► [rollup phases: load → transform → graph → link → chunk → renderChunk]
    │
    ├─► es2abc compilation (per-module, worker pool)
    │
    ▼
ProcessCompiledResources
    │
    ▼
PackageHap             ──── zip assembly
    │
    ▼
SignHap                ──── hap-sign-tool
    │
    ▼
AssembleHap            ──── hook task (entry point for `hvigorw assembleHap`)
```

---

## Cross-Reference

- [hvigorw-analysis.md](hvigorw-analysis.md) — full task dependency graph
- [chunk-model.md](chunk-model.md) — IIFE chunk architecture
- [abc-pipeline.md](abc-pipeline.md) — es2abc compilation details
- [module-resolution.md](module-resolution.md) — OHOS module resolution
- [plugins.md](plugins.md) — plugin inventory and Rust replacements
