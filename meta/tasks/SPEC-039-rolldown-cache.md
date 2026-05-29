# SPEC-039: Rolldown compileEnv-Keyed Msgpack Cache

**Crate**: `arkdown-cache`, `arkdown` (bundler core)
**Priority**: P1 — 10× build speed improvement
**Blocked by**: SPEC-005 (bundler), SPEC-019 (cache crate)
**Blocks**: SPEC-025 (ABC incremental cache), SPEC-040 (task-level skip)

---

## Summary

Implement the rolldown module-graph cache using the msgpack format and `compileEnv`
key, mirroring hvigor's `RollupCacheService`. On a cache hit, the entire rolldown
bundling step is skipped and previous `.js` chunk files on disk are reused. On a miss,
the cache is written after a successful rolldown run.

The `arkdown-cache` crate already has `BundleCache` (JSON-based). This SPEC adds the
msgpack-format rolldown module cache that mirrors hvigor's `cache/msgpack/` directory.

---

## Background: What hvigor Does

From `build.js`:

```javascript
async function build(config, ...) {
    readCacheWithEvent(config, storeManager);
    // ... rollup() ...
    if (config.writeRollupCache) {
        await rollupCacheService.writeCache({
            enableFileSystemCache: config.enableFileSystemCache,
            cache: bundle.cache,
            cacheDirectory: config.cacheDirectory,
            pkgName2SourceRoots: config.pkgName2SourceRoots,
            cacheKey: config.compileEnv,  // ← compileEnv key
            ...
        });
    }
}

function readCacheWithEvent(config, storeManager) {
    rollupCacheService.readCache(config, storeManager);
}
```

From `rollup-cache-service.js`:

```javascript
readCache(config, storeManager) {
    if (!config.enableFileSystemCache) return;
    const cacheDir = this.getCacheDir(config.cacheDirectory);
    // cacheDir = path.join(cacheDirectory, MSGPACK_COMPILE_CACHE_DIR_NAME)
    // MSGPACK_COMPILE_CACHE_DIR_NAME = "msgpack"
    config.cache = config.cache || { modules: [] };
    CacheHandlerFactory.getHandlerChain(cacheDir, storeManager).readCache(config);
}

async writeCache({ cache, cacheDirectory, cacheKey, ... }) {
    const cacheDir = this.getCacheDir(cacheDirectory);
    const finalCache = this.getFinalCache(cache, pkgName2SourceRoots);
    CacheHandlerFactory.getHandlerChain(cacheDir, storeManager)
        .writeCache({ cache: finalCache, cacheKey, ... });
}

getCacheDir(cacheDirectory) {
    return path.join(cacheDirectory, MSGPACK_COMPILE_CACHE_DIR_NAME);
    // → "<cachePath>/msgpack/"
}
```

`getCompileEnv()` from `arkts-pack.js`:

```javascript
function getCompileEnv(projectConfig, moduleConfig) {
    return [
        projectConfig.compileSdkVersion,
        projectConfig.compatibleSdkVersion,
        projectConfig.runtimeOS,
        projectConfig.etsLoaderPath,
        !!projectConfig.obfuscationOptions?.selfConfig?.ruleOptions?.enable,
        moduleConfig.moduleType,
        path.resolve(moduleConfig.cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME),
    ].join("#");
}
```

When `compileEnv` changes between builds (e.g., SDK version bump, obfuscation toggle),
the cache is invalidated: `CacheHandlerFactory` detects the key mismatch and discards
all cached modules.

The rollup cache format is msgpack-encoded rollup `ModuleCache` objects, one file per
module, stored at `<cachePath>/msgpack/<module_hash>.pack`.

---

## Scope

### 1. `compileEnv` Key

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileEnv {
    pub compile_sdk_version: String,
    pub compatible_sdk_version: String,
    pub runtime_os: String,           // "OpenHarmony"
    pub ets_loader_path: String,
    pub obfuscation_enabled: bool,
    pub module_type: String,          // "entry" | "har" | "hsp"
    pub msgpack_cache_path: String,   // absolute path to <cachePath>/msgpack/
}

impl CompileEnv {
    /// Produce the "#"-joined key string used as cache discriminator.
    pub fn key(&self) -> String {
        [
            &self.compile_sdk_version,
            &self.compatible_sdk_version,
            &self.runtime_os,
            &self.ets_loader_path,
            &self.obfuscation_enabled.to_string(),
            &self.module_type,
            &self.msgpack_cache_path,
        ]
        .join("#")
    }
}
```

### 2. Msgpack Module Cache Directory

Store per-module cache files at:
```
<cachePath>/msgpack/<fnv1a_hex(module_id)>.pack
```

Each `.pack` file is a msgpack-encoded `RolldownModuleCache`:

```rust
#[derive(Serialize, Deserialize)]
pub struct RolldownModuleCache {
    /// compileEnv key at write time — used to detect stale entries.
    pub compile_env_key: String,
    /// The module's transformed source code.
    pub transformed_code: String,
    /// Optional source map JSON string.
    pub source_map: Option<String>,
    /// Direct import specifiers (for dependency invalidation).
    pub imports: Vec<String>,
    /// Module metadata (pkgName, OHM URL, etc.).
    pub meta: serde_json::Value,
}
```

### 3. `RolldownCache` API

Add to `arkdown-cache`:

```rust
pub struct RolldownCache {
    cache_dir: PathBuf,
    compile_env_key: String,
    entries: HashMap<String, RolldownModuleCache>,
}

impl RolldownCache {
    /// Load all valid cache entries from disk.
    /// Entries whose compile_env_key != self.compile_env_key are discarded.
    pub fn load(cache_dir: &Path, compile_env: &CompileEnv) -> Result<Self, CacheError>;

    /// Look up a cached module by its ID.
    pub fn get(&self, module_id: &str) -> Option<&RolldownModuleCache>;

    /// Insert or update a module cache entry.
    pub fn insert(&mut self, module_id: &str, entry: RolldownModuleCache);

    /// Write all dirty entries to disk (msgpack format).
    pub fn flush(&self) -> Result<(), CacheError>;

    /// Delete all cache files in the directory (full invalidation).
    pub fn invalidate_all(&self) -> Result<(), CacheError>;
}
```

### 4. Bundler Integration

In `arkdown` (bundler core), wrap the rolldown build with cache read/write:

```rust
pub async fn bundle_with_cache(
    config: &BundleConfig,
    compile_env: &CompileEnv,
) -> Result<BundleOutput, BundleError> {
    let cache_dir = config.cache_dir.join("msgpack");
    std::fs::create_dir_all(&cache_dir)?;

    // 1. Load cache
    let mut rolldown_cache = RolldownCache::load(&cache_dir, compile_env)
        .unwrap_or_else(|_| RolldownCache::new(&cache_dir, compile_env));

    // 2. Pass cached transforms to rolldown (via ShouldTransformCachedModule hook)
    let output = rolldown::bundle(config, &rolldown_cache).await?;

    // 3. Write updated cache
    for module in output.modules() {
        rolldown_cache.insert(&module.id, RolldownModuleCache {
            compile_env_key: compile_env.key(),
            transformed_code: module.transformed_code.clone(),
            source_map: module.source_map.clone(),
            imports: module.imports.clone(),
            meta: module.meta.clone(),
        });
    }
    rolldown_cache.flush()?;

    Ok(output)
}
```

### 5. `shouldTransformCachedModule` Hook

Rolldown supports a `shouldTransformCachedModule` plugin hook. When a module's cache
entry is valid (same `compile_env_key`), return `false` to skip re-transformation:

```rust
fn should_transform_cached_module(&self, module_id: &str) -> bool {
    self.rolldown_cache.get(module_id).is_none()
}
```

### 6. `removeUncacheableModules` Equivalent

Before reading the cache, filter out modules whose `pkgName2SourceRoots` mapping has
changed (mirrors `rollupCacheService.removeUncacheableModules()`). This prevents stale
resolution from cached modules when HAR source roots change.

---

## Files to Change

```
crates/arkdown-cache/src/
├── lib.rs              — re-export RolldownCache, CompileEnv
├── rolldown_cache.rs   — NEW: RolldownCache, RolldownModuleCache
└── compile_env.rs      — NEW: CompileEnv struct and key()

crates/arkdown/src/
└── lib.rs              — bundle_with_cache(); wire RolldownCache into rolldown build

crates/arkdown-cli/src/
└── build.rs            — construct CompileEnv; call bundle_with_cache()
```

---

## Test Plan

- Cold build: `<cachePath>/msgpack/` is empty; after bundle, `.pack` files are written.
- Warm build: second bundle with identical source and same `compileEnv` reads all entries
  from cache; `shouldTransformCachedModule` returns `false` for all modules.
- `compileEnv` change: changing `compile_sdk_version` causes all `.pack` files to be
  discarded; full re-bundle occurs.
- Source change: module with changed source has a cache miss; other unchanged modules
  use cached transforms.
- `flush()` writes valid msgpack; `load()` round-trips all entries.
- `invalidate_all()` removes all `.pack` files from the directory.
- Corrupt `.pack` file: `load()` skips the corrupt entry and logs a debug message.
- `CompileEnv::key()` is deterministic for the same inputs.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/build.js`
  — `readCacheWithEvent()`, `rollupCacheService.writeCache()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/dist/src/cache/rollup/rollup-cache-service.js`
  — `RollupCacheService.readCache()`, `writeCache()`, `getCacheDir()`, `MSGPACK_COMPILE_CACHE_DIR_NAME`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
  — `getCompileEnv()`, `generateConfig()` `compileEnv` field
- `meta/tasks/SPEC-019-cache.md` — existing `arkdown-cache` crate
