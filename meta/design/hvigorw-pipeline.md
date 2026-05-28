# hvigorw Build Pipeline Analysis

Complete reverse-engineering of the HarmonyOS hvigorw build tool pipeline
(DevEco Studio 6.0.0.858, conducted 2026-05-28).

---

## Key Finding: Bundler Identification

**hvigorw does NOT use rolldown.** It uses a Huawei fork of rollup 3:

```
rollup@3.19.1-htrunk1.AllScenario.devecostudio.r6
```

Located at:
```
DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/
  hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/
    node_modules/rollup/
```

Package: `@ohos/hvigor-arkts-base@6.0.6`

---

## Complete Build Pipeline

```
hvigorw assembleHap
│
├─[1] PRE-BUILD
│     ├── PreBuild
│     │     - SDK validation, clean-build check, env setup
│     └── GeneratePkgContextInfo
│           - Build OHM URL maps for all modules
│           - Produce pkgContextInfo.json
│
├─[2] PROFILE PROCESSING (parallel)
│     ├── MergeProfile
│     │     - Overlay HAR dependency profiles
│     │     - Merge module.json5 fragments
│     ├── ProcessProfile
│     │     - Set virtualMachine: "ark{N}.0.{patch}.0"
│     │     - Set compileMode: "esmodule"
│     │     - Map abilities srcEntry → OHM URLs
│     ├── ProcessRouterMap
│     │     - Collect router map entries from all modules
│     │     - Cache-invalidate on route changes
│     ├── ProcessShareConfig
│     │     - Share extension ability configs
│     └── ProcessStartupConfig
│           - Startup extension configs
│
├─[3] RESOURCE COMPILATION
│     ├── CompileResource (restool binary)
│     │     Input:  src/main/resources/
│     │     Output: resources.index (RestoolV2 binary)
│     │             ResourceTable.txt
│     │             resources/base/{element,media,profile}/
│     └── ProcessCompiledResources
│           - Handle preloaded SO libraries
│           - Prepare resource assets for packaging
│
├─[4] GENERATE loader.json
│     GenerateLoaderJson
│     Fields written:
│       compileEntry[]          - ability srcEntry paths
│       modulePathMap           - module name → absolute path
│       hspNameOhmMap           - HSP logical name → OHM URL
│       harNameOhmMap           - HAR logical name → OHM URL
│       byteCodeHarInfo         - ByteCode HAR .abc paths
│       routerMap[]             - from ProcessRouterMap
│       dynamicImportLibInfo    - dynamic import metadata
│       workers[]               - WebWorker file paths
│       packageManagerType      - "ohpm"
│       useNormalizedOHMUrl     - boolean
│       buildMode               - "debug" | "release"
│
├─[5] ROLLUP BUNDLING  ←── ArkCompile task (parallel worker pool)
│     │
│     │  rollup 3.19.1-htrunk1 pipeline:
│     │
│     │  Plugin execution order:
│     │  ① ohos-resolve           resolveId()   OHOS module resolution
│     │  ② commonjs               transform()   CJS→ESM
│     │  ③ json5                  load()        JSON/JSON5 imports
│     │  ④ module-meta-info       moduleParsed()  pkgName, OHM URLs
│     │  ⑤ ets-loader             transform()   ArkTS → TypeScript → JS
│     │  ⑥ invalid-cache          (build hooks) cache invalidation
│     │  ⑦ router-map             (build hooks) router entry collection
│     │  ⑧ har-dedup              (build hooks) HAR deduplication (HSP)
│     │  ⑨ wrapper-chunk          renderChunk() IIFE + globalThis registry
│     │
│     │  Output per chunk:
│     │  (function () {
│     │    "use strict";
│     │    /* ... bundled CJS code ... */
│     │    globalThis["chunkName_hash"] = { export1: ... };
│     │  })();
│     │
│     └── Output: *.js IIFE-wrapped chunks in loader_out/
│
├─[6] ES2ABC COMPILATION  (bytecode)
│     Input:   IIFE-wrapped .js chunks
│     Binary:  DevEco SDK es2abc
│     Process:
│       1. Parse JS with oxc (TypeScript mode)
│       2. Strip TypeScript annotations
│       3. Lower to Panda IR
│       4. Emit ABC bytecode
│       5. Merge chunks → single modules.abc
│     Parallel: Worker pool (max = MAX_ARK_COMPILE_NUM)
│     Cache:    msgpack per-function, invalidated by SDK/API version
│     Output:   ets/modules.abc  (magic: PANDA\x00\x00\x00)
│
├─[7] HAP ASSEMBLY
│     PackageHap / PackageHsp / PackageHar
│     Zip entries:
│       module.json              merged app + module sections
│       pack.info                bundle summary
│       pkgContextInfo.json      package OHM mappings
│       resources.index          binary resource table
│       ets/modules.abc          Panda bytecode
│       resources/base/...       compiled resource files
│       libs/arm64-v8a/*.so      native libraries (optional)
│
└─[8] SIGNING
      SignHap / SignHsp
      - Keystore: .p12 (PKCS#12, PBES2/AES-256-CBC for debug)
      - Profile:  .p7b (DevEco auto-generated debug cert)
      - Algorithm: SHA256withECDSA
      - Password:  DevEco AES-128-GCM encrypted (see arkdown-sign)
      Output: entry-default-signed.hap
```

---

## Task Dependency Graph (61 total tasks)

```
AssembleHap
└─ SignHap
   └─ PackageHap
      ├─ ProcessCompiledResources
      │  └─ GeneratePkgModuleJson
      │     ├─ ProcessProfile
      │     │  ├─ ProcessStartupConfig → GeneratePkgContextInfo → PreBuild
      │     │  ├─ ProcessShareConfig   → GeneratePkgContextInfo → PreBuild
      │     │  └─ ProcessRouterMap     → GeneratePkgContextInfo → PreBuild
      │     ├─ ArkCompile
      │     │  ├─ GenerateLoaderJson
      │     │  │  ├─ ProcessRouterMap
      │     │  │  ├─ ProcessShareConfig
      │     │  │  └─ ProcessStartupConfig
      │     │  ├─ CompileResource
      │     │  │  └─ ProcessResource
      │     │  │     └─ MergeProfile → PreBuild + [HAR deps]
      │     │  └─ CreateBuildProfile → PreBuild + [HSP deps]
      │     └─ ProcessModulePrivacyProfile → PreBuild
      └─ ProcessIntegratedHsp → PreBuild
```

---

## Intermediate Artifact Layout

```
entry/build/default/intermediates/
├── loader/default/
│   └── loader.json                 ← GenerateLoaderJson
├── ets/default/loader_out/default/
│   ├── *.js                        ← rollup output chunks
│   └── cache/msgpack/              ← es2abc incremental cache
├── res/default/
│   ├── module.json                 ← ProcessProfile
│   ├── resources.index             ← restool (RestoolV2 binary)
│   ├── ResourceTable.txt           ← restool (human-readable)
│   └── resources/                  ← compiled resource files
├── process_profile/default/
│   └── module.json                 ← final merged module.json
├── router_map/                     ← ProcessRouterMap
├── source_map/                     ← debug source maps
└── package/default/                ← pre-assembled HAP contents
```

---

## Plugin Inventory

### P0 — Required for any HAP build

| hvigor Plugin | npm Package | arkdown Crate | Notes |
|---------------|-------------|---------------|-------|
| `resolve/index` + `import-resolver` | custom | `arkdown-plugin-ohos-resolve` | Most complex; handles OHM URLs, ByteCode HAR, SDK paths |
| `module-meta-info-plugin` | custom | `arkdown-plugin-module-meta` | pkgName, isLocalDep, OHM identity |
| `wrapper-chunk-plugin` | custom | `arkdown-plugin-chunk-wrapper` | IIFE wrap, globalThis registry |
| `check-duplicate-filenames-plugin` | custom | `arkdown-plugin-duplicate-check` | Prevent ABC record name collisions |
| `invalid-cache-plugin` | custom | `arkdown-cache` | Incremental build cache invalidation |
| `@rollup/plugin-commonjs` | 24.1.0 | **rolldown built-in** | CJS→ESM; use rolldown's native support |
| `@rollup/plugin-json` | 6.0.0 | `arkdown-plugin-json5` | JSON/JSON5 imports |
| `common-plugin` | custom | absorbed into core | ProjectConfig context sharing |

### P1 — Required for production/release builds

| hvigor Plugin | npm Package | arkdown Crate | Notes |
|---------------|-------------|---------------|-------|
| `router-map-plugin` | custom | `arkdown-plugin-router-map` | Router map cache invalidation |
| `use-sensitive-check-plugin` | custom | `arkdown-plugin-sensitive-check` | Case-sensitive import validation |
| `har-deduplicate/` + `hsp-collect-har-files` | custom | `arkdown-plugin-har-dedup` | Two-pass HSP: collect then exclude HAR |
| `@rollup/plugin-terser` | 0.3.0 | via `oxc_minifier` | JS minification; not yet wired in arkdown |
| `importCheckPlugin` | custom | `arkdown-plugin-import-check` (TODO) | Deprecated API warnings |

### P2 — Watch mode, hot-reload, advanced features

| hvigor Plugin | arkdown Crate | Priority |
|---------------|---------------|----------|
| `collect-importers-plugin` | `arkdown-plugin-importers` (TODO) | For hot-reload dependency tracking |
| `preview-resource-watcher-plugin` | `arkdown-plugin-preview-watch` (TODO) | DevEco IDE integration |
| `add-packagejson-meta-plugin` | absorbed | Runtime version tracking |
| `build-steps/clean-ets-for-tgz` | `arkdown-plugin-ets-tgz-clean` (TODO) | HAR publishing cleanup |

### Absorbed by Rust (no plugin needed)

| hvigor JS Plugin | Rust equivalent |
|------------------|-----------------|
| `gc-plugin` | Rust `Drop` trait (deterministic) |
| `wait-async-plugin` | Tokio async runtime |
| `copy-plugin` / `delete-plugin` | Rust `std::fs` |

---

## OHOS Module Resolution Details

### Specifier forms handled by ohos-resolve

```
"@ohos/xxx"                         → SDK openharmony/ets/api/@ohos/xxx
"@kit.AbilityKit"                   → maps to @ohos:app.ability.UIAbility etc.
"@bundle:bundleName/module/path"    → cross-HAP/HSP absolute reference
"./relative/path"                   → file-relative (standard)
"oh_modules/xxx"                    → ohpm package (NOT node_modules)
"libxxx.so"                         → native library declaration
```

### Kit → ohos mapping (critical for ABC import records)

```
@kit.AbilityKit  →  @ohos:app.ability.UIAbility
                    @ohos:app.ability.AbilityConstant
                    @ohos:app.ability.Want
@kit.ArkUI       →  @ohos:web.webview  (etc.)
@kit.PerformanceAnalysisKit  →  @ohos:hilog
```

This mapping happens in the ets-loader transform step. The ABC import records
must use the `@ohos:xxx` form, not `@kit.xxx` — the ArkRuntime looks up
`@ohos:xxx` in its system module registry.

### ABC module class name format

The class name stored in modules.abc for each compiled module:

```
L&{module_name}/src/main/{path_no_ext}&;

Example:
  entry ability at ./ets/entryability/EntryAbility.ets
  → L&entry/src/main/ets/entryability/EntryAbility&;
```

This is what `ExecuteModuleBufferSecure` uses to locate the module entry point.

---

## External Binary Dependencies

| Tool | Location | Purpose | Output |
|------|----------|---------|--------|
| `restool` | `DevEco SDK/toolchains/restool` | Resource compilation | `resources.index` (RestoolV2 binary) |
| `es2abc` | `DevEco SDK/toolchains/es2abc` | JS → Panda bytecode | `modules.abc` (PANDA magic) |
| `hap-sign-tool` | `DevEco SDK/toolchains/` | HAP signing | signed `.hap` |

arkdown replaces all three with pure-Rust implementations:
- `restool` → `arkdown-resource`
- `es2abc` → `arkdown-abc`
- `hap-sign-tool` → `arkdown-sign`

---

## Critical Compatibility Requirements

| Requirement | Reason |
|-------------|--------|
| `loader.json` exact field names | `@ohos/hvigor-arkts-base` plugins parse exact keys |
| `module.json` must have `app` + `module` sections | ArkRuntime module registry |
| `module.json` must have `packageName` field | OHOS module OHM path resolution |
| `pkgContextInfo.json` must have `moduleName`, `bundleName` fields | Runtime dependency lookup |
| ABC class name `L&path&;` format | `ExecuteModuleBufferSecure` lookup |
| ABC import records use `@ohos:xxx` not `@kit.xxx` | System module registry keys |
| `resources.index` RestoolV2 magic | Binary format recognizer |
| `ets/modules.abc` path in HAP | ArkRuntime path convention (API >= 9) |
| ABC Panda magic `PANDA\x00\x00\x00` (8 bytes) | Bytecode validator |
| Adler-32 checksum at offset 0x08 | File integrity check on load |
| Annotation stubs in ABC | ArkRuntime requires L_ESModuleRecord etc. |

---

## References

- DevEco Studio: `~/.meat/ide/DevEco-Studio-6.0.0.858.app`
- hvigor-ohos-plugin: `DevEco.app/Contents/tools/hvigor/hvigor-ohos-plugin/`
- Existing analysis: `meta/design/hvigorw-analysis.md`
- Existing plugins doc: `meta/design/plugins.md`
- Existing pipeline doc: `meta/design/pipeline.md`
