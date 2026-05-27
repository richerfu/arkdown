# OHOS Module Resolution

Deep dive into how arkdown resolves module specifiers in OpenHarmony OS projects. This replaces the JavaScript resolve plugin chain (`@ohos/hvigor-arkts-compose/plugins/resolve/`, `@rollup/plugin-node-resolve`) with a pure Rust implementation.

---

## 1. Specifier Forms

OHOS projects use several distinct specifier forms. The resolver must handle all of them.

### 1.1 Relative Imports

```typescript
import { foo } from "./utils/helper";
import { bar } from "../shared/types";
```

Standard relative path resolution. Extensions tried in order: `.ets`, `.ts`, `.js`, `.ets/index`, etc.

### 1.2 `@ohos/*` SDK Kit Specifiers

```typescript
import { hilog } from "@ohos/hilog";
import { AbilityConstant } from "@ohos/ability-framework";
```

These resolve to declarations in the SDK's `api/` directory:

- Path: `sdk/default/openharmony/ets/api/@ohos.hilog.d.ts` (or `.d.ets`)
- In `arkts-pack.js`: `globalModulePaths.push(path.resolve(etsLoaderPath, '../../api'))`

When `useNativeResolver` is enabled, the Rust oxc-resolver is used. The `sdkModulesPath` array passed to the resolver plugin includes the SDK api dir.

### 1.3 `@kit/*` Kit Abstraction Specifiers

```typescript
import { window } from "@kit.ArkUI";
import { UIAbility, Want } from "@kit.AbilityKit";
import { hilog } from "@kit.PerformanceAnalysisKit";
```

Kit specifiers are umbrella imports. `@kit.ArkUI` re-exports from multiple `@ohos/*` packages. Resolution:

- Resolved via `sdk/default/openharmony/ets/kits/@kit.ArkUI.d.ts` (kit declaration)
- These kit files re-export from underlying `@ohos.*` modules

### 1.4 `@bundle:` Cross-Bundle References

```typescript
import { ComponentA } from "@bundle:com.example.myapp/hsp-lib/ets/components/ComponentA";
```

Format: `@bundle:<bundleName>/<moduleName>/<relativePath>`

- `bundleName` = `app.bundleName` from `app.json5` (e.g., `com.example.myapp`)
- `moduleName` = `module.name` from target module's `module.json5`
- `relativePath` = path under `src/main/` (without extension)

`@bundle:` references are used for cross-HSP imports and ability `srcEntry` OHM URLs. They are resolved at runtime by ArkRuntime's module registry, not at bundle time (they appear as external references in the bundle output).

### 1.5 `ohpm:` Direct Package Specifiers

Less common; some tools emit `ohpm:package-name` which maps to the ohpm package registry. Treated similarly to bare specifiers with ohpm package manager.

### 1.6 Bare Package Specifiers (ohpm)

```typescript
import { something } from "my-ohpm-package";
```

Resolution order:

1. `<module>/oh_modules/<package>/oh-package.json5` → `main` field
2. `<project>/oh_modules/<package>/oh-package.json5` → `main` field

The `oh_modules/` directory is ohpm's equivalent of `node_modules/`. It contains extracted package tarballs.

### 1.7 `file:` Local HAR References

In `oh-package.json5`:

```json5
{
  dependencies: {
    "my-local-har": "file:../local-har-module",
  },
}
```

Resolves to the local directory's `oh-package.json5` → `main` field.

---

## 2. `oh-package.json5` Dependency Types

### Source HAR

A local or remote HAR with TypeScript/ETS source:

```json5
{
  name: "my-har",
  version: "1.0.0",
  main: "./src/main/ets/index.ets",
  types: "./src/main/ets/index.d.ets",
}
```

The source files are compiled as part of the consuming module's rollup build.

### ByteCode HAR

A HAR distributed as pre-compiled `.abc` bytecode:

```json5
{
  name: "my-bytecode-har",
  metadata: {
    customTypes: [],
    declarationEntry: ["@bundle:com.vendor/my-bytecode-har/index"],
    runtimeOnly: { packages: [], sources: [] },
  },
}
```

`isByteCodeHarDependency()` returns true when the HAR's `oh-package.json5` contains a `metadata.declarationEntry` or its `main` field points to a `.abc` file.

For ByteCode HAR:

- `byteCodeHarInfo[pkgName]` in `loader.json` contains `abcPath`, `sourceMapsPath`, `compatibleSdkVersion`
- The `.abc` file is directly included in the final HAP's bytecode merge step
- Source is not compiled; only `.d.ts`/`.d.ets` declarations are used for type checking

### HSP Reference

```json5
{
  dependencies: {
    "shared-lib": "@shared:com.example.myapp/shared-module",
  },
}
```

Format: `@shared:<bundleName>/<moduleName>`. HSP modules are pre-installed on the device; the consuming HAP references them by bundle+module name at runtime.

---

## 3. oxc-resolver Integration

hvigor already uses a Huawei fork of `oxc-resolver@1.11.0-htrunk1.AllScenario.devecostudio.r2`. This is a Rust-based resolver exposed via NAPI.

### Current behavior (when `useNativeResolver` is true)

```javascript
// arkts-pack.js getResolvePlugin()
(0, index_js_1.default)({
  // OHOS node-resolve plugin
  extensions: supportExtensions,
  modulePaths: [oh_modules_path, ...resolveModulePaths],
  useNativeResolver: config.useNativeResolver && isSupportedPlatform(),
  sdkModulesPath: globalModulePaths,
  pkgNameToPkgInfo: config.pkgNameToPkgBriefInfo,
  caseSensitiveCheck: config.caseSensitiveCheck,
});
```

When `useNativeResolver` is disabled (default or unsupported platform), a JavaScript fallback is used.

### arkdown approach

arkdown's `arkdown-plugin-ohos-resolve` crate implements a full Rust resolver:

1. Start with `oxc-resolver` as the base algorithm
2. Layer OHOS-specific rules on top:
   - `@ohos/*` → SDK api path
   - `@kit/*` → SDK kits path
   - `oh_modules/` directory walk (instead of `node_modules/`)
   - `oh-package.json5` instead of `package.json` for main/types fields

### `pkgNameToPkgBriefInfo` — the OHOS package brief info map

```typescript
type PkgBriefInfo = {
  pkgRoot: string;
  pkgName: string;
  sourceRoots: string[]; // relative to pkgRoot
  originalSourceRoots: string[]; // from buildOption.sourceOption.sourceRoots
};
type PkgBriefInfoMap = Record<string, PkgBriefInfo>;
```

This map is built by `ArkCompile.setPkgNameToPkgBriefInfo()` and passed to the resolver. It enables the resolver to find source files for local HAR dependencies without traversing the full file system.

---

## 4. OHM URL Normalization (`use-new-ohm-url-plugin`)

When `useNormalizedOHMUrl` is `true` (the recommended production setting), all module identifiers are normalized to OHM URL format.

### OHM URL Format

```
@bundle:<bundleName>/<moduleName>/<filePath>
```

Example: `@bundle:com.example.myapp/entry/ets/entryability/EntryAbility`

### What `newOhmUrlPlugin` does

The `use-new-ohm-url-plugin.js` implements only the `shouldInvalidCache` hook:

```javascript
function shouldInvalidCache() {
  return this.share?.projectConfig?.useNormalizedOHMUrl !== this.cache?.get(NEW_OHM_URL_HASH);
}
```

It invalidates the build cache when the `useNormalizedOHMUrl` flag changes. The actual URL generation is done by `ohmurl-utils.js` → `generateOhmUrl()` / `generateOhmurlForSrcEntry()`.

### OHM URL generation

```typescript
// Simplified from ohmurl-utils.js
function generateOhmUrl(pkgContextInfo: PkgContextInfo): string {
  return `@bundle:${pkgContextInfo.bundleName}/${pkgContextInfo.moduleName}/${pkgContextInfo.pkgPath}`;
}

function generateOhmurlForSrcEntry(
  targetService,
  hostModuleName,
  harDep,
  srcEntry: string,
): string {
  const bundleName = targetService.getProduct().bundleName;
  const harModuleName = harDep.getModuleJsonObj().module.name;
  // srcEntry like "./ets/foo/Bar.ets" → strip "./" and extension
  const normalized = srcEntry.replace(/^\.\//, "").replace(/\.(e)?ts$/, "");
  return `@bundle:${bundleName}/${harModuleName}/${normalized}`;
}
```

### `GeneratePkgContextInfo` task

Builds `pkgContextInfos` — the authoritative mapping used for OHM URL generation:

```typescript
type PkgContextInfo = {
  packageName: string; // e.g., "my-har"
  bundleName: string; // e.g., "com.example.myapp"
  moduleName: string; // e.g., "entry"
  version: string; // package version
  pkgPath: string; // absolute path to oh_modules entry
  entryPath?: string; // resolved main file
};
```

---

## 5. Resolve Path Walk in `resolveModulePaths`

From `getResolveModules()` in `arkts-pack.js`:

```javascript
function getResolveModules(config) {
  const t = config.packageManagerType === OHPM ? OH_NODE_MODULES : NODE_MODULES;
  const modulePath = path.resolve(config.modulePath);
  const projectTopDir = path.resolve(config.projectTopDir);
  const buildGeneratedProfilePath = config.buildGeneratedProfilePath;
  config.resolveModulePaths = [
    path.resolve(modulePath, "src/main"), // module source root
    buildGeneratedProfilePath, // generated build profile dir
    path.resolve(modulePath, t), // module-level oh_modules
    path.resolve(projectTopDir, t), // project-level oh_modules
  ];
  return config.resolveModulePaths;
}
```

Resolution order for bare specifiers:

1. `<module>/src/main/` — local source files (catch `@ohos/*` resolved via SDK path alias)
2. `<module>/.hvigor/generated/` — generated build profile files
3. `<module>/oh_modules/<pkg>/` — module-level dependencies
4. `<project>/oh_modules/<pkg>/` — project-level hoisted dependencies

---

## 6. SDK Path Conventions

### ETS API path (for `@ohos/*` and `@kit/*`)

```
sdk/default/openharmony/ets/api/
```

Contains: `@ohos.hilog.d.ts`, `@ohos.ability.abilityManager.d.ts`, etc.

### Kits path (for `@kit.*`)

```
sdk/default/openharmony/ets/kits/
```

Contains: `@kit.ArkUI.d.ts`, `@kit.AbilityKit.d.ts`, etc.

### Component path (for `@Component` and ArkUI built-ins)

```
sdk/default/openharmony/ets/build-tools/ets-loader/declarations/
```

### arkdown SDK path resolution

arkdown reads `sdkPath` from config (the `etsLoaderPath`'s parent directories). The resolver maps:

- `@ohos/` → `{sdkPath}/api/`
- `@kit.` → `{sdkPath}/kits/`
- `@system.` → `{sdkPath}/api/` (legacy FA mode)

---

## 7. ByteCode HAR Resolution Details

When `useNormalizedOHMUrl` is true and a HAR dep is ByteCode type:

1. `setByteCodeHarInfoAndOtherCompileFiles()` in `GenerateLoaderJson` calls `collectByteCodeInfoAndOtherCompileEntrances()`
2. For each ByteCode HAR, `byteCodeHarInfo[pkgName]` is populated with `abcPath`
3. The ByteCode HAR's `.abc` is listed in `loader.json` → `byteCodeHarInfo`
4. The ArkCompile task's `initDefaultArkCompileConfig` passes `byteCodeHarToDependencyKeys` to the rollup config
5. During chunk rendering, ByteCode HAR imports appear as external references — no source bundling occurs

**Key invariant**: `isByteCodeHarDependency()` and `useNormalizedOHMUrl` must both be true for ByteCode HAR optimization. If `useNormalizedOHMUrl` is false, ByteCode HARs are unsupported and the build fails with `BYTECODE_HARS_%S_NOT_SUPPORTED_WHEN_USENORMALIZEDOHMURL_IS_NOT_TRUE`.

---

## 8. `process-importee.js` — Post-Build Import Rewriting

After rollup's write phase, `processImportees()` in `hvigor-arkts-base` post-processes the output to rewrite import paths for:

- ByteCode HAR entry files
- Bundled dependency entries
- Direct importee path normalization

This is needed because rollup's `chunkFileNames` uses `[name].js` and the names must match what es2abc expects.

---

## 9. arkdown Resolver Algorithm (Rust)

```
resolve(specifier, importer):
  if specifier starts with './' or '../':
    → relative resolution from importer directory
    → try extensions: [.ets, .ts, .js, .d.ets, .d.ts]
    → try index: [/index.ets, /index.ts, /index.js]

  if specifier starts with '@ohos/':
    → sdk_api_dir / '@ohos.' + specifier[7:] + '.d.ts'

  if specifier starts with '@kit.':
    → sdk_kits_dir / specifier + '.d.ts'

  if specifier starts with '@bundle:':
    → external reference; return as-is (ArkRuntime resolves at load time)

  if specifier starts with '@shared:':
    → external reference; return as-is

  bare specifier 'pkg-name':
    → walk resolveModulePaths:
        1. modulePath/src/main/
        2. buildGeneratedProfilePath/
        3. modulePath/oh_modules/pkg-name/
           → read oh-package.json5, follow 'main' field
        4. projectTopDir/oh_modules/pkg-name/
           → read oh-package.json5, follow 'main' field
    → if pkgNameToPkgBriefInfo has 'pkg-name':
        → resolve against pkgBriefInfo.sourceRoots
```

---

## Cross-Reference

- [pipeline.md](pipeline.md) — where resolution fits in the pipeline
- [hvigorw-analysis.md](hvigorw-analysis.md) — loader.json format (harNameOhmMap, hspNameOhmMap)
- [plugins.md](plugins.md) — `arkdown-plugin-ohos-resolve`, `arkdown-plugin-ohm-url`
- [chunk-model.md](chunk-model.md) — how OHM URLs appear in the chunk registry
