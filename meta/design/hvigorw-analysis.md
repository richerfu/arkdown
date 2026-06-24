# hvigorw Reverse-Engineering Analysis

Detailed analysis of hvigor's build system as reverse-engineered from the minified JavaScript sources in the DevEco Studio command-line tools package.

---

## 1. Entry Point: `hvigorw assembleHap`

The `hvigorw` shell wrapper invokes the hvigor daemon, which resolves the `assembleHap` task via the `@ohos/hvigor-ohos-plugin`.

### `AssembleHap` Hook Task

```javascript
// assemble-hap.js
class AssembleHap extends AbstractModuleHookTask {
  constructor(a, e) {
    super(a, e, CommonHookTask.ASSEMBLE_HAP);
  }
  initTaskDepends(a) {
    this.skipSignHap(a)
      ? this.dependsOn(`${targetName}@${PackageHap.name}`)
      : this.dependsOn(`${targetName}@${SignHap.name}`);
  }
  skipSignHap(a) {
    return (
      a.getBuildOption().packOptions?.buildAppSkipSignHap &&
      hvigorCore.isCommandEntryTask(ASSEMBLE_APP.name)
    );
  }
}
```

`assembleHap` is a **hook task** — it has no action of its own, it simply depends on `SignHap` (or `PackageHap` if signing is skipped). The full task chain is pulled transitively.

Similarly:

- `assembleHsp` → `SignHsp` or `PackageHsp`
- `assembleHar` → `PackageHar` (no signing for HAR)

---

## 2. Full Task Dependency Graph (HAP build)

The task dependency graph for `assembleHap` on a single module with one target:

```
AssembleHap
└── SignHap
    └── PackageHap
        ├── ProcessCompiledResources
        │   └── GeneratePkgModuleJson
        │       ├── ArkCompile (CompileArkTS)
        │       │   ├── GenerateLoaderJson
        │       │   │   ├── ProcessRouterMap
        │       │   │   │   └── GeneratePkgContextInfo
        │       │   │   │       └── PreBuild
        │       │   │   ├── ProcessShareConfig
        │       │   │   │   └── GeneratePkgContextInfo
        │       │   │   ├── ProcessStartupConfig
        │       │   │   │   └── GeneratePkgContextInfo
        │       │   │   ├── CreateModuleInfo (HAP/HSP)
        │       │   │   └── [HSP deps: CompileResource@hsp]
        │       │   ├── CompileResource
        │       │   │   └── ProcessResource
        │       │   │       └── MergeProfile
        │       │   │           ├── PreBuild
        │       │   │           └── [HAR deps: MergeProfile@har]
        │       │   └── CreateBuildProfile
        │       │       ├── PreBuild
        │       │       └── [HSP deps: CreateBuildProfile@hsp]
        │       └── ProcessModulePrivacyProfile
        │           └── PreBuild
        └── ProcessIntegratedHsp
            └── PreBuild
```

For HAR targets, the graph diverges at `ProcessHarArtifacts` instead of `GeneratePkgModuleJson`.

### HAR Build Chain

```
AssembleHar → PackageHar → ProcessHarArtifacts
    ├── ProcessObfuscationFiles → CompileResource
    ├── CacheNativeLibs
    ├── ProcessOhPackageJson (or ProcessPackageJson)
    ├── HarCompileArkTS / HarBuildArkTS
    └── ProcessIntegratedHsp
```

### HSP Build Chain

```
AssembleHsp → SignHsp → PackageHsp
    ├── GeneratePkgModuleJson
    │   └── [same as HAP except ArkCompile = HspCompileArkTS]
    └── ProcessModulePrivacyProfile
```

---

## 3. Task Descriptions

| Task                       | Class                      | Purpose                                                                             |
| -------------------------- | -------------------------- | ----------------------------------------------------------------------------------- |
| `PreBuild`                 | `PreBuild`                 | Clean check (if SDK changed), emit build start event                                |
| `GeneratePkgContextInfo`   | `GeneratePkgContextInfo`   | Build `pkgContextInfos` mapping pkgName → OHM URL context                           |
| `MergeProfile`             | `MergeProfile`             | Merge base `module.json5` with product overlays and HAR/HSP dependency metadata     |
| `ProcessProfile`           | `ProcessProfile`           | Set `virtualMachine`, `compileMode`, `deviceTypes`; collect HAR abilities           |
| `ProcessRouterMap`         | `ProcessRouterMap`         | Collect router map entries from rollup output, write `intermediates/routerMap.json` |
| `ProcessShareConfig`       | `ProcessShareConfig`       | Process `crossAppSharedConfig` from HSP dependencies — **implemented**: `arkdown-profile::process_share_config` (SPEC-046) |
| `ProcessStartupConfig`     | `ProcessStartupConfig`     | Merge startup config from all HAR/HSP deps, validate task dependencies — **implemented**: `arkdown-profile::process_startup_config` (SPEC-046) |
| `CreateModuleInfo`         | `CreateModuleInfo`         | Generate module metadata JSON for HAP/HSP                                           |
| `CreateHarModuleInfo`      | `CreateHarModuleInfo`      | Generate module metadata JSON for HAR                                               |
| `GenerateLoaderJson`       | `GenerateLoaderJson`       | Write `loader.json` — the entry manifest consumed by rollup                         |
| `CompileResource`          | `CompileResource`          | Invoke `restool` to compile resources to binary format                              |
| `ArkCompile`               | `ArkCompile`               | Run rollup (bundling) + es2abc (bytecode compilation)                               |
| `GeneratePkgModuleJson`    | `GeneratePkgModuleJson`    | Generate the final `module.json` placed in the HAP                                  |
| `ProcessCompiledResources` | `ProcessCompiledResources` | Post-process compiled resources (preloaded SO handling)                             |
| `ProcessIntegratedHsp`     | `ProcessIntegratedHsp`     | Integrate remote/integrated HSP dependencies                                        |
| `PackageHap`               | `PackageHap`               | Assemble the HAP zip                                                                |
| `SignHap`                  | `SignHap`                  | Sign the HAP using hap-sign-tool                                                    |
| `CollectDebugSymbol`       | `CollectDebugSymbol`       | Collect `.so` debug symbols                                                         |

---

## 4. Config Files Consumed

### `build-profile.json5` (project root)

```json5
{
  app: {
    signingConfigs: [
      {
        name: "default",
        type: "HarmonyOS",
        material: {
          certpath: "...",
          keyAlias: "...",
          keyPassword: "...",
          profile: "...",
          signAlg: "SHA256withECDSA",
          storeFile: "...",
          storePassword: "...",
        },
      },
    ],
    products: [
      {
        name: "default",
        signingConfig: "default",
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

Key fields consumed by arkdown:

- `compatibleSdkVersion` → `--target-api-version` for es2abc
- `useNormalizedOHMUrl` → OHM URL normalization mode
- `signingConfigs.material` → signing invocation
- `buildModeSet` → debug vs release (affects tree-shake, obfuscation, opt-level)
- `modules[].srcPath` → module root directories

### `oh-package.json5` (per module and project root)

```json5
{
  modelVersion: "5.0.0",
  description: "...",
  dependencies: {
    "@ohos/lib-foo": "^1.0.0", // remote ohpm package
    "local-har": "file:../local-har", // local HAR reference
  },
  devDependencies: {
    "@ohos/hypium": "1.0.19", // test framework
  },
  overrides: {},
}
```

### `module.json5` (per module)

```json5
{
  "module": {
    "name": "entry",
    "type": "entry",                      // entry | feature | shared | har
    "mainElement": "EntryAbility",        // default ability name
    "deviceTypes": ["phone", "tablet"],
    "deliveryWithInstall": true,
    "pages": "$profile:main_pages",
    "abilities": [
      {
        "name": "EntryAbility",
        "srcEntry": "./ets/entryability/EntryAbility.ets",
        "exported": true,
        "skills": [...]
      }
    ],
    "extensionAbilities": [...]
  }
}
```

Key fields for arkdown:

- `module.type` → determines HAP (`entry`/`feature`), HSP (`shared`), or HAR (`har`) build
- `abilities[].srcEntry` → rollup entry points
- `module.srcEntry` → module-level entry (rare)
- `mainElement` → primary ability for launcher

### `hvigorfile.ts` (project/module root)

```typescript
import { appTasks } from "@ohos/hvigor-ohos-plugin";
export default {
  system: appTasks,
  plugins: [], // custom plugins extend the rollup pipeline
};
```

Module-level `hvigorfile.ts` can extend with module-specific plugins. arkdown maps this to its own plugin extension mechanism.

---

## 5. Intermediate Build Artifact Layout

hvigor places all intermediate artifacts under `build/` relative to the module directory:

```
entry/
└── build/
    └── default/                          # product name
        └── intermediates/
            ├── loader/
            │   └── default/              # target name
            │       └── default/
            │           └── loader.json   # GenerateLoaderJson output
            ├── res/
            │   └── default/
            │       ├── resConfig.json    # restool input config
            │       ├── ResourceTable.txt
            │       └── module.json       # processed module.json
            ├── ets/
            │   └── default/
            │       └── loader_out/
            │           └── default/
            │               ├── cache/    # es2abc incremental cache (msgpack)
            │               └── *.js      # rollup output chunks (pre-ABC)
            ├── patch/                    # HAP patch info
            ├── pkgContextInfo/           # OHM URL context mapping
            └── routerMap/                # collected router map entries
    └── outputs/
        └── default/
            └── entry.hap                 # final signed output
```

arkdown should produce the same layout for compatibility with DevEco Studio project views.

---

## 6. Daemon Architecture

### hvigor Daemon

hvigor runs a Java-based daemon process that:

1. Maintains a persistent JVM with pre-loaded Gradle tooling
2. Communicates with the CLI client via a local socket
3. Manages a pool of Node.js workers for the rollup compilation phase

The daemon avoids JVM startup overhead on repeated builds. arkdown eliminates the daemon entirely — the Rust binary is fast enough to start fresh each invocation, and incremental state is maintained via on-disk caches.

### Node.js Worker Pool

The ArkCompile task submits work to a worker pool via `submitArkCompileWork`:

```javascript
async function submitArkCompileWork(task, pool, log, config, callback, targetWorkers) {
  config.level = log.getLevel();
  config.buildId = globalData.buildId;
  const handle = pool.submit(task, path.resolve(__dirname, "./job.js/run"), {
    workInput: config,
    hasSideEffects: false,
    memorySensitive: true,
  });
  if (handle.getState() === TaskState.REJECT) {
    // Fallback: run on main thread
    config.isMainThread = true;
    await run(config).then(async (result) => {
      await callback(result);
    });
  }
}
```

Pool size is controlled by `ohosArkCompileMaxSize` parameter (default: `ArkPackConst.MAX_ARK_COMPILE_NUM`). Each worker runs rollup bundling + es2abc for one module.

arkdown replaces this with Rayon-based parallel module compilation using Rust threads.

---

## 7. Incremental Build Cache Strategy

hvigor uses several layers of incremental caching:

### Task-Level Incremental Caching

Each hvigor task declares `inputs` and `outputs`. If inputs haven't changed (checked via file hash or declared values), the task is skipped. Key input declarations from `ArkCompile`:

- `obfuscationHash` — hash of all obfuscation rule files
- `obfuscationEnable` — flag from `buildOption.arkOptions.obfuscation.ruleOptions.enable`
- `TSC_MAX_FLOW_DEPTH` — TypeScript compiler setting

If `obfuscationEnable` changes between builds, the entire arkts cache is cleaned:

```javascript
if (String(cacheEnable) !== String(this.getObfuscationEnable())) {
  await this.cleanCache(); // empties getTaskTempDir(targetData)
}
```

### Rollup File System Cache

`hvigor-arkts-base` uses a custom `RollupCacheService` that writes msgpack-format cache to `intermediates/ets/default/loader_out/default/cache/`. The cache key includes `compileEnv`:

```javascript
function getCompileEnv(config, merged) {
  return [
    config.compileSdkVersion,
    config.compatibleSdkVersion,
    config.runtimeOS,
    config.etsLoaderPath,
    !!obfuscationEnabled,
    merged.moduleType,
    path.resolve(merged.cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME),
  ].join("#");
}
```

Cache is invalidated when any of these change. The `invalidCachePlugin` handles per-file cache invalidation.

### es2abc Incremental Cache

`--cache-file <path>` flag passes a cache file to es2abc. This is stored in `intermediates/ets/default/loader_out/default/cache/msgpack/` (the `MSGPACK_COMPILE_CACHE_DIR_NAME` constant). Cache is invalidated on module-level changes.

### arkdown Cache Strategy

arkdown should implement:

1. Per-module content hash → skip re-transform on unchanged files
2. Rolldown's `ScanStageCache` for module graph incremental builds (mirrors hvigor's rollup cache)
3. es2abc `--cache-file` per-module cache files in the same layout for DevEco Studio compatibility

---

## 8. What arkdown Must Replicate vs. Can Simplify

### Must Replicate (compatibility-critical)

| Feature                     | Reason                                                |
| --------------------------- | ----------------------------------------------------- |
| `loader.json` format        | Consumed by SDK ets-loader plugins; format must match |
| Intermediate artifact paths | DevEco Studio watches these paths                     |
| `module.json` processing    | Must produce identical `module.json` for HAP/restool  |
| OHM URL format              | Runtime module registry uses these exact strings      |
| restool invocation args     | restool is a closed binary; args must be correct      |
| es2abc invocation flags     | Bytecode format is version-sensitive                  |
| HAP zip structure           | ArkRuntime expects specific zip layout                |
| Incremental cache location  | DevEco Studio invalidates caches by path              |

### Can Simplify

| Feature                  | Simplification                                    |
| ------------------------ | ------------------------------------------------- |
| Java daemon              | Eliminated — Rust startup is fast                 |
| Node.js worker pool      | Replaced by Rayon thread pool                     |
| hvigorfile.ts plugin API | arkdown uses its own Rust plugin trait            |
| Gradle interop           | Not supported (see [compat.md](compat.md))        |
| Custom hvigorfile tasks  | Not supported in arkdown                          |
| `invalid-cache-plugin`   | Absorbed into arkdown's native cache invalidation |
| `gc-plugin`              | No equivalent needed in Rust                      |
| `wait-async-plugin`      | Rust's async runtime handles this naturally       |

---

## 9. `loader.json` Schema

The `loader.json` file is the primary interface between `GenerateLoaderJson` (task) and the rollup/arkts-compose pipeline. Key fields written by `doTaskAction()`:

```json
{
  "workers": ["./ets/workers/Worker.ets"],
  "modulePathMap": { "entry": "/path/to/entry" },
  "compileMode": "esmodule",
  "projectRootPath": "/path/to/project",
  "nodeModulesPath": "/path/to/node_modules",
  "byteCodeHarInfo": {
    "pkgName": { "abcPath": "...", "compatibleSdkVersion": "...",
                 "sourceMapsPath": "...", "insightIntent": "..." }
  },
  "declarationEntry": ["@bundle:com.example/entry/foo"],
  "moduleName": "entry",
  "hspNameOhmMap": { "hsp-lib": "@bundle:com.example/hsp-lib/index" },
  "harNameOhmMap": { "har-lib": "@bundle:com.example/har-lib/index" },
  "packageManagerType": "ohpm",
  "compileEntry": ["/abs/path/to/EntryAbility.ets", ...],
  "otherCompileFiles": [...],
  "dynamicImportLibInfo": { "pkg": { "entryFilePath": "...", "moduleName": "..." } },
  "routerMap": [{ "name": "routeName", "pageSourceFile": "...", "buildFunction": "..." }],
  "hspResourcesMap": { "hsp-lib": "/path/to/ResourceTable.txt" },
  "anBuildMode": null,
  "apiVersion": 12,
  "fallbackAnBuild": false
}
```

arkdown must write this exact format (or a superset that the SDK plugins can consume).

### `compileEntry` population (arkdown)

`generate_loader_json` builds arkdown's internal `compile_entry` list from hvigor entry sources in order:

1. **Ability `srcEntry` paths** — from each `HapManifest.abilities[].src_entry`.
2. **ExtensionAbility `srcEntry` paths** — from each `HapManifest.extension_abilities[].src_entry`.
3. **HAR/HSP package entry** — when a source HAR/shared module has no ability/module
   `srcEntry`, read `oh-package.json5#main` (for example `Index.ets`) as the package entry.
4. **`$profile:main_pages` page files** — resolved from `src/main/resources/base/profile/<name>.json`;
   each page entry `"pages/Index"` maps to `"./ets/pages/Index.ets"`.
5. **Worker files** — every `*.ets` file found in `src/main/ets/workers/` is added as
   `"./ets/workers/<filename>.ets"` and also recorded in `LoaderJson.workers`.

Router map JSON is **not** appended to `compile_entry`. Upstream `compiler/main.js`
uses `aceBuildJson.routerMap` in `loadNavigationConfig()` for router metadata and
obfuscation entry tracking, while `projectConfig.intentEntry` comes from
`aceBuildJson.compileEntry`. arkdown mirrors that split: only `module.json5#routerMap`
causes loader `routerMap` metadata to be populated.

After all sources are collected, `compile_entry` is deduplicated preserving order.

The serialized `compileEntry` field (`hvigor_compile_entry`) remains empty for the minimal-hap
fixture, matching the golden hvigor output exactly. The internal `compile_entry` is used by
the arkdown bundler and is not written to disk.

---

## 10. Real HAP Structure (ground truth from MyApplication1)

The following zip structure was verified from a real DevEco Studio-built HAP (`entry-default-signed.hap`).
Use this as the reference when implementing `pack_hap()` in SPEC-012.

```
entry.hap
├── module.json               ← app + module sections merged; "virtualMachine": "ark12.0.1.0",
│                                "compileMode": "esmodule"
├── pack.info                 ← bundle summary (see below)
├── pkgContextInfo.json       ← package context OHM URL mappings (see below)
├── resources.index           ← binary, begins with RestoolV2 magic bytes
├── ets/
│   └── modules.abc           ← binary, begins with b"PANDA\x00\x00\x00" (8 bytes); API >= 9
├── resources/
│   └── base/
│       ├── element/          ← string.json, color.json, etc.
│       ├── media/            ← app_icon.png, etc.
│       └── profile/          ← main_pages.json, etc.
└── libs/                     ← optional; only present when native .so files exist
    └── arm64-v8a/
        └── *.so
```

### `pack.info` structure

```json
{
  "summary": {
    "app": {
      "bundleName": "com.example.myapplication",
      "bundleType": "app",
      "version": { "code": 1000000, "name": "1.0.0" }
    },
    "modules": [
      {
        "mainAbility": "EntryAbility",
        "deviceTypes": ["phone", "tablet"],
        "name": "entry",
        "type": "entry",
        "abilities": [
          { "name": "EntryAbility", "label": "$string:EntryAbility_label", "exported": true }
        ]
      }
    ]
  },
  "packages": [
    {
      "deviceTypes": ["phone", "tablet"],
      "moduleType": "entry",
      "deliveryWithInstall": true,
      "name": "entry"
    }
  ]
}
```

### `pkgContextInfo.json` structure

```json
{
  "entry": {
    "packageName": "entry",
    "bundleName": "com.example.myapplication",
    "moduleName": "entry",
    "version": "",
    "entryPath": "ets/modules.abc",
    "isSO": false
  }
}
```

### `module.json` must have both sections

```json
{
  "app": {
    "bundleName": "com.example.myapplication",
    "vendor": "example",
    "versionCode": 1000000,
    "versionName": "1.0.0",
    "icon": "$media:app_icon",
    "label": "$string:app_name"
  },
  "module": {
    "name": "entry",
    "type": "entry",
    "virtualMachine": "ark12.0.1.0",
    "compileMode": "esmodule",
    "deviceTypes": ["phone", "tablet"],
    "abilities": [...]
  }
}
```

---

## Cross-Reference

- [pipeline.md](pipeline.md) — full pipeline overview
- [abc-pipeline.md](abc-pipeline.md) — es2abc compilation details
- [module-resolution.md](module-resolution.md) — OHM URL and dependency resolution
- [chunk-model.md](chunk-model.md) — module-mode chunk/output contract and legacy wrapper background
- [compat.md](compat.md) — command mapping and migration guide
