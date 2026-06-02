# Plugin Inventory

Complete inventory of all plugins in hvigor's JavaScript pipeline, their arkdown Rust replacements, priorities, and estimated complexity.

Priority levels:

- **P0**: Required for any HAP to build at all
- **P1**: Required for production/release builds
- **P2**: Advanced features (hot-reload, HAR dedup, preview mode)

---

## 1. `@ohos/hvigor-arkts-compose` — OHOS-Specific Plugins (Highest Priority)

### `resolve/index` + `import-resolver` + `resolveImportSpecifiers`

**Source**: `dist/src/plugins/resolve/index.js`, `import-resolver.js`, `resolveImportSpecifiers.js`

**What it does in hvigor**:
The OHOS-aware node-resolve plugin. Used instead of `@rollup/plugin-node-resolve` when `packageManagerType === "ohpm"`. Key behaviors:

- Resolves `oh_modules/` instead of `node_modules/`
- Reads `oh-package.json5` instead of `package.json` for `main`/`types` fields
- Routes `@ohos/*` to SDK api directory
- Routes `@kit.*` to SDK kits directory
- Case-sensitivity check (when `caseSensitiveCheck` is true)
- Native resolver: optionally delegates to `oxc-resolver` Rust implementation when `useNativeResolver` is true
- `pkgNameToPkgInfo` — map from package names to source root directories for local HAR resolution
- `sdkModulesPath` — list of SDK api/kits directories

**arkdown Rust crate**: `arkdown-plugin-ohos-resolve`
**Priority**: P0
**Complexity**: High
**Notes**: This is the most complex plugin. Must handle all OHOS specifier forms (see [module-resolution.md](module-resolution.md)). arkdown now delegates generic filesystem/package probing to the upstream Rust `oxc_resolver` crate and keeps only hvigor-specific semantics in local code: `oh-package.json5`, `oh_modules` / `.ohpm`, SDK fallback, ByteCode HAR, and project `file:` HAR aliases. The Rust implementation eliminates the NAPI boundary overhead entirely.

---

### `router-map-plugin`

**Source**: `dist/src/plugins/router-map-plugin.js`

**What it does in hvigor**:
Implements the `shouldInvalidCache` rollup hook. Returns `true` (cache invalid) when the router map configuration for a module has changed since the last build:

```javascript
function shouldInvalidCache(moduleInfo) {
  return (
    this.share?.projectConfig?.routerMap?.get(moduleInfo.id) !==
    this.cache?.get(ROUTER_MAP_HASH)?.get(moduleInfo.id)
  );
}
```

The actual router map collection is done in `ProcessRouterMap` task (not a rollup plugin). This plugin ensures that source files referenced in the router map are recompiled when the map changes.

**arkdown Rust crate**: `arkdown-plugin-router-map`
**Priority**: P1
**Complexity**: Low
**Notes**: The `shouldInvalidCache` hook maps to arkdown's `should_invalidate_cache` plugin hook. arkdown's native incremental cache handles this via file hash comparison, but the router map requires extra invalidation logic.

---

### `module-meta-info-plugin`

**Source**: `dist/src/plugins/module-meta-info-plugin.js`

**What it does in hvigor**:
Implements `buildStart`, `load`, `moduleParsed`, `beforeBuildEnd`, `afterBuildEnd`, `cleanUp`, `resolveDynamicImport`, `resolveId` hooks. This is the most complex plugin in the pipeline.

Key behaviors:

1. **`load` hook**: For each loaded module, determines its `pkgName`, `pkgVersion`, `moduleName`, `pkgPath`, `isLocalDependency`, `isNodeEntryFile`, and sets this as `module.meta`
2. **`moduleParsed` hook**: Builds `hostModulesInfo` (reverse dependency map) by walking `importedIds` and `dynamicallyImportedIds`
3. **`resolveId` hook**: Warns about packages listed in `dynamicDependencies` that are imported statically
4. **`beforeBuildEnd` hook**: Sets `needPreloadSo` tag on modules reachable from ability entry points (for preloaded SO optimization); stores cache
5. **`afterBuildEnd` hook**: Removes `needPreloadSo` tags after bundle write

The `localModuleInfoCache` avoids re-walking the project model on every load call. `packageInfoCache` caches `oh-package.json5` parse results.

**arkdown Rust crate**: `arkdown-plugin-module-meta`
**Priority**: P0
**Complexity**: High
**Notes**: The module metadata is consumed by many other systems (loader.json generation, ByteCode HAR lookup, OHM URL generation). Must be implemented accurately.

---

### `use-new-ohm-url-plugin`

**Source**: `dist/src/plugins/use-new-ohm-url-plugin.js`

**What it does in hvigor**:
Implements only `shouldInvalidCache`. Invalidates the build cache when `useNormalizedOHMUrl` flag changes:

```javascript
function shouldInvalidCache() {
  return this.share?.projectConfig?.useNormalizedOHMUrl !== this.cache?.get(NEW_OHM_URL_HASH);
}
```

**arkdown Rust crate**: `arkdown-plugin-ohm-url`
**Priority**: P1
**Complexity**: Low
**Notes**: In arkdown, this is absorbed into the config hash — any config change (including `useNormalizedOHMUrl`) invalidates all incremental caches. May not need a separate plugin.

---

### `har-deduplicate/har-deduplicate-util` + `hsp-collect-har-files`

**Source**: `dist/src/plugins/har-deduplicate/har-deduplicate-util.js`, `hsp-collect-har-files.js`

**What it does in hvigor**:
For HSP builds, identifies HAR source files that are duplicated across the HAP and HSP (both depend on the same HAR). The deduplication:

1. `hspCollectHarFilesPlugin` — in a separate "collect" build pass, walks the module graph and writes lists of HAR files included in the HSP to `SOURCE_HAR_DUPLICATE_FILES_INFO` and `BYTECODE_HAR_DUPLICATE_FILES_INFO` files
2. `getHarDuplicateFileSet()` — reads these files and returns a Set of file paths to exclude from the HAP compile pass
3. `isHspDeduplicateRealCompile()` — returns true when this is the real (non-collect) HSP compile
4. `isHspCollectFilesCompile()` — returns true when this is the HAR file collection pass

The build runs twice for HSP: once to collect the HAR file list, once for the real compile that excludes the collected files from the HAP.

**arkdown Rust crate**: `arkdown-plugin-har-dedup`
**Priority**: P1 (required for correct HSP builds)
**Complexity**: High
**Notes**: Requires two-pass architecture for HSP builds. The first pass is a graph analysis; the second is a normal build with exclusions applied.

---

### `collect-importers-plugin`

**Source**: `dist/src/plugins/collect-importers-plugin.js`

**What it does in hvigor**:
Builds a reverse importer graph for hot-reload/cold-reload mode. When `patchConfig.mode` is `"hotReload"` or `"coldReload"`, this plugin tracks which modules import which other modules. Used by the IDE to send targeted re-compilation requests.

**arkdown Rust crate**: `arkdown-plugin-importers`
**Priority**: P2 (hot-reload mode only)
**Complexity**: Medium
**Notes**: The reverse importer graph is a natural byproduct of the module graph build. arkdown can compute this as part of `ModuleGraph` construction.

---

### `add-packagejson-meta-plugin`

**Source**: `dist/src/plugins/add-packagejson-meta-plugin.js` (inferred from arkts-pack.js plugin list)

**What it does in hvigor**:
Embeds package.json/oh-package.json5 metadata into bundles for runtime version tracking.

**arkdown Rust crate**: `arkdown-plugin-pkg-meta`
**Priority**: P2
**Complexity**: Low

---

### `check-duplicate-filenames-plugin`

**Source**: `dist/src/plugins/check-duplicate-filenames-plugin.js`

**What it does in hvigor**:
Validates that no two source files have the same base filename (e.g., two files both named `Utils.ets` in different directories). This is required because es2abc uses filenames as module record names in the merged `.abc`, and duplicates would cause conflicts.

**arkdown Rust crate**: `arkdown-plugin-duplicate-check`
**Priority**: P0
**Complexity**: Low

---

### `importCheckPlugin` (inferred)

**Source**: Part of SDK plugins loaded from `ets-loader`

**What it does in hvigor**:
Validates that imports use allowed APIs. Checks that `@ohos/*` imports reference declared SDK APIs. Warns on deprecated APIs.

**arkdown Rust crate**: `arkdown-plugin-import-check`
**Priority**: P1
**Complexity**: Medium

---

### `use-sensitive-check-plugin`

**Source**: `dist/src/plugins/use-sensitive-check-plugin.js`

**What it does in hvigor**:
When `caseSensitiveCheck` is true (in `strictMode`), validates that import paths match the actual filesystem case exactly. Prevents issues on case-insensitive macOS/Windows filesystems producing builds that fail on case-sensitive Linux/device environments.

**arkdown Rust crate**: `arkdown-plugin-sensitive-check`
**Priority**: P1
**Complexity**: Low
**Notes**: The check is only relevant on macOS and Windows (case-insensitive FSes). On Linux, the filesystem itself enforces this.

---

### `preview-resource-watcher-plugin`

**Source**: `dist/src/plugins/preview-resource-watcher-plugin.js`

**What it does in hvigor**:
In preview mode (`config.isPreview && config.appResource`), watches resource files and triggers rebuilds when they change. Used by DevEco Studio's in-IDE previewer.

**arkdown Rust crate**: `arkdown-plugin-preview-watch`
**Priority**: P2 (preview mode only)
**Complexity**: Medium

---

### `invalid-cache-plugin`

**Source**: `dist/src/plugins/invalid-cache-plugin.js`

**What it does in hvigor**:
Implements `shouldInvalidCache`. Forces cache invalidation based on configuration changes that rollup's own cache does not detect (e.g., `compileEnv` hash changes when SDK version or obfuscation settings change).

**arkdown Rust crate**: Built into `arkdown-cache`
**Priority**: P0
**Complexity**: Low
**Notes**: Absorbed into arkdown's native incremental cache system. The `compileEnv` hash concept maps directly to arkdown's cache key derivation.

---

### `gc-plugin`

**Source**: `dist/src/plugins/gc-plugin.js`

**What it does in hvigor**:
Forces JavaScript garbage collection (`global.gc()`) between compilation phases to prevent memory pressure in the Node.js worker.

**arkdown Rust crate**: Absorbed into Rust memory model
**Priority**: Not needed
**Notes**: Rust's ownership system and `Drop` trait handle memory deterministically. No equivalent needed.

---

### `wait-async-plugin`

**Source**: `dist/src/plugins/wait-async-plugin.js`

**What it does in hvigor**:
Synchronizes asynchronous plugin results that don't fit rollup's synchronous hook model. Some OHOS SDK plugins produce results asynchronously (e.g., type checking, API validation). This plugin collects promises and waits for them before proceeding.

**arkdown Rust crate**: Absorbed into Rust async runtime
**Priority**: Not needed
**Notes**: arkdown's async plugin trait (`async fn transform`, etc.) handles async natively via Tokio.

---

### `copy-plugin` + `delete-plugin`

**Source**: `dist/src/plugins/copy-plugin.js`, `delete-plugin.js`

**What it does in hvigor**:
`copy-plugin`: Copies resource files to the build output directory (used in FA mode and watch mode).
`delete-plugin`: Cleans up stale files from previous builds (e.g., old chunk files no longer referenced).

**arkdown Rust crate**: Absorbed into `arkdown-ohos` output stage
**Priority**: P0 (delete), P2 (copy for FA mode)
**Notes**: arkdown handles file copy/delete as part of its output writing stage using native Rust fs operations.

---

### `common-plugin`

**Source**: `dist/src/plugins/common-plugin.js`

**What it does in hvigor**:
Shared plugin utilities and entry point coordination. Loads the `aceBuildJson` (loader.json), sets up the `share.projectConfig` context accessible to all plugins, and handles the initial module graph setup.

**arkdown Rust crate**: Absorbed into `arkdown-plugin` crate
**Priority**: P0
**Notes**: The `share.projectConfig` context maps to arkdown's `BuildContext` / plugin context structs.

---

## 2. `@ohos/hvigor-arkts-base` — Rollup Integration Layer

### `plugins/terser-plugin`

**Source**: `dist/src/plugins/terser-plugin.js`

**What it does in hvigor**:
JavaScript minification via terser. Used in release builds.

**arkdown replacement**: `ohos-rs/oxc` minifier
**Priority**: P1
**Complexity**: Low
**Notes**: oxc's minifier is faster than terser and already a Rust library. No separate plugin needed; integrated into the transform pipeline.

---

### `build-steps/process-importee`

**Source**: `dist/src/build-steps/process-importee.js`

**What it does in hvigor**:
Post-build step that rewrites import paths in rollup output for ByteCode HAR entries and bundled dependency entries. Runs after rollup's write phase via `processImportees()`:

```javascript
async function processImportees(config, {isByteCodeHar, bundledDependencies, ...}, modulePath, cacheDir, cache) {
    // Rewrites chunk import paths for bytecode HAR and bundled deps
}
```

Handles three cases:

1. ByteCode HAR: The HAR's ABC is referenced directly, not compiled
2. Bundled dependencies: `isBundledDependencies` mode includes HAR source in the bundle
3. Direct importee collection: Tracks which modules directly import which packages

**arkdown Rust crate**: `arkdown-plugin-ohos-resolve`
**Priority**: P1
**Complexity**: Medium
**Notes**: This post-processing logic is specific to OHOS module references and belongs in the ohos-resolve plugin's `generate_bundle` hook.

---

### `build-steps/clean-ets-for-tgz`

**Source**: `dist/src/build-steps/clean-ets-for-tgz.js`

**What it does in hvigor**:
Strips ETS-specific code (e.g., `@Component`, `@State` decorators) from source files before packaging a HAR as a TGZ for ohpm publishing. The TGZ contains type declarations, not the full ArkTS source.

**arkdown Rust crate**: `arkdown-plugin-ets-tgz-clean`
**Priority**: P2 (HAR publishing only)
**Complexity**: Medium

---

## 3. `@rollup/*` — Standard Rollup Plugins

### `@rollup/plugin-node-resolve@15.0.1`

**What it does in hvigor**:
Standard Node.js module resolution. Used when `packageManagerType === "npm"` (legacy, not ohpm). For ohpm projects, the OHOS custom resolve plugin is used instead.

**arkdown replacement**: `arkdown-plugin-ohos-resolve` (OHOS-aware)
**Priority**: P0
**Notes**: For pure npm projects (rare in OHOS context), the standard resolver is used. arkdown's resolver handles both ohpm and npm modes.

---

### `@rollup/plugin-commonjs@24.1.0`

**What it does in hvigor**:
Converts CommonJS `require()`/`module.exports` modules to ESM so rollup can process them. Most SDK helper modules and some npm packages use CJS.

**arkdown replacement**: `arkdown-plugin-commonjs`
**Priority**: P0
**Complexity**: High
**Notes**: rolldown has a built-in CJS transform. arkdown inherits this from rolldown core.

---

### `@rollup/plugin-babel@6.0.3`

**What it does in hvigor**:
Applies Babel transforms to JavaScript source files. Primarily used for modern JavaScript syntax down-compilation.

**arkdown replacement**: `ohos-rs/oxc` transformer
**Priority**: P1
**Notes**: oxc's transformer handles modern JS syntax (class fields, decorators, etc.) faster than Babel. No separate plugin needed.

---

### `@rollup/plugin-json@6.0.0`

**What it does in hvigor**:
Allows importing `.json` and `.json5` files as JavaScript modules:

```typescript
import config from "./config.json5";
// → const config = { ... };
```

**arkdown replacement**: `arkdown-plugin-json5`
**Priority**: P0
**Complexity**: Low

---

### `@rollup/plugin-terser@0.3.0`

**What it does in hvigor**:
Minification via terser (same as `plugins/terser-plugin` above; this is the rollup-standard version).

**arkdown replacement**: `ohos-rs/oxc` minifier
**Priority**: P1
**Notes**: Same as terser-plugin above.

---

## 4. Summary Table

| Plugin / Source                              | arkdown Crate                    | Priority | Complexity |
| -------------------------------------------- | -------------------------------- | -------- | ---------- |
| `resolve/index` + `import-resolver`          | `arkdown-plugin-ohos-resolve`    | P0       | High       |
| `module-meta-info-plugin`                    | `arkdown-plugin-module-meta`     | P0       | High       |
| `check-duplicate-filenames-plugin`           | `arkdown-plugin-duplicate-check` | P0       | Low        |
| `common-plugin`                              | absorbed into `arkdown-plugin`   | P0       | Low        |
| `invalid-cache-plugin`                       | `arkdown-cache` (built-in)       | P0       | Low        |
| `@rollup/plugin-commonjs`                    | rolldown built-in                | P0       | High       |
| `@rollup/plugin-json`                        | `arkdown-plugin-json5`           | P0       | Low        |
| `@rollup/plugin-node-resolve`                | `arkdown-plugin-ohos-resolve`    | P0       | —          |
| `router-map-plugin`                          | `arkdown-plugin-router-map`      | P1       | Low        |
| `use-new-ohm-url-plugin`                     | `arkdown-plugin-ohm-url`         | P1       | Low        |
| `use-sensitive-check-plugin`                 | `arkdown-plugin-sensitive-check` | P1       | Low        |
| `har-deduplicate/` + `hsp-collect-har-files` | `arkdown-plugin-har-dedup`       | P1       | High       |
| `build-steps/process-importee`               | `arkdown-plugin-ohos-resolve`    | P1       | Medium     |
| `plugins/terser-plugin`                      | `ohos-rs/oxc` minifier           | P1       | Low        |
| `@rollup/plugin-babel`                       | `ohos-rs/oxc` transformer        | P1       | Low        |
| `@rollup/plugin-terser`                      | `ohos-rs/oxc` minifier           | P1       | Low        |
| `importCheckPlugin`                          | `arkdown-plugin-import-check`    | P1       | Medium     |
| `build-steps/clean-ets-for-tgz`              | `arkdown-plugin-ets-tgz-clean`   | P2       | Medium     |
| `collect-importers-plugin`                   | `arkdown-plugin-importers`       | P2       | Medium     |
| `preview-resource-watcher-plugin`            | `arkdown-plugin-preview-watch`   | P2       | Medium     |
| `add-packagejson-meta-plugin`                | `arkdown-plugin-pkg-meta`        | P2       | Low        |
| `gc-plugin`                                  | absorbed into Rust model         | N/A      | —          |
| `wait-async-plugin`                          | absorbed into Rust async         | N/A      | —          |
| `copy-plugin` + `delete-plugin`              | `arkdown-ohos` output stage      | P0/P2    | Low        |

---

## Cross-Reference

- [pipeline.md](pipeline.md) — when each plugin runs in the pipeline
- [module-resolution.md](module-resolution.md) — resolve plugin algorithm
- [types.md](types.md) — `ArkdownPlugin` trait
- [hvigorw-analysis.md](hvigorw-analysis.md) — task context for plugins
