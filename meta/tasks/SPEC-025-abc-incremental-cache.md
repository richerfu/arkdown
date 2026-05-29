# SPEC-025: ABC Incremental Cache (Per-Function SHA-256)

**Crate**: `arkdown-abc`, `arkdown-cache`
**Priority**: P2 — build speed
**Blocked by**: SPEC-009 (ABC compiler), SPEC-039 (rolldown compileEnv cache)
**Blocks**: —

---

## Summary

Wire the ABC compiler's incremental cache so that unchanged functions skip IR lowering
and bytecode emission on rebuild. The cache is keyed by `SHA-256(source_text)` per
function and stored in a msgpack file at `cache/abc/modules.abc.cache`. Cache entries
are invalidated when the `compileEnv` key changes.

This mirrors `es2abc --cache-file <path>` behavior and hvigor's
`RollupCacheService` integration in `ark-compile.js::doTaskAction()`.

---

## Background: What hvigor Does

From `ark-compile.js`:

```javascript
// doTaskAction() — after initDefaultArkCompileConfig()
e.cachePath && this.trace(e.cachePath);
fse.ensureDirSync(e.cachePath);

// trace() checks whether msgpack cache dir exists to log incremental vs. cold build
trace(cachePath) {
  const t = path.resolve(cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME);
  ohosTrace.traceIncrement(this.moduleName, "COMPILE_ARKTS", fse.existsSync(t));
}
```

`es2abc` accepts `--cache-file <path>` which reads/writes a per-function cache.
The cache key is the SHA-256 hash of the function's source text. On a cache hit,
`es2abc` skips IR lowering and emits previously cached bytecode directly.

The `compileEnv` key (from `arkts-pack.js::getCompileEnv()`) is:
```
[compileSdkVersion, compatibleSdkVersion, runtimeOS, etsLoaderPath,
 obfuscationEnabled, moduleType, cachePath].join("#")
```
When `compileEnv` changes, the entire ABC cache is invalidated.

From `SPEC-019` (`arkdown-cache`): the ABC cache file path is already plumbed as
`cache_file: Some(PathBuf)` in the CLI. The Rust ABC compiler must now actually read
and write this file.

---

## Scope

### 1. Cache File Format

Store a msgpack file at `<cachePath>/abc/modules.abc.cache`:

```rust
#[derive(Serialize, Deserialize)]
pub struct AbcCache {
    /// The compileEnv key at the time this cache was written.
    /// If it differs from the current compileEnv, the entire cache is invalid.
    pub compile_env: String,
    /// Per-function entries, keyed by SHA-256(source_text) hex string.
    pub functions: HashMap<String, AbcFunctionEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct AbcFunctionEntry {
    /// Serialized bytecode for this function's MethodRecord.
    pub bytecode: Vec<u8>,
    /// LNP body bytes (empty if debug_info disabled).
    pub lnp_body: Vec<u8>,
    /// Register count.
    pub num_vregs: u16,
    /// Number of parameters (for method signature).
    pub num_args: u16,
}
```

Use `rmp-serde` for msgpack serialization (already in workspace via `arkdown-cache`).

### 2. Cache Key Computation

```rust
/// Compute a stable cache key for a function's source text.
pub fn function_cache_key(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(source.as_bytes());
    hex::encode(hash)
}
```

The "source text" for a function is the slice of the module source corresponding to
that function's span (from the OXC AST `Span`).

### 3. AbcCompiler Integration

Extend `AbcCompileOptions`:

```rust
pub struct AbcCompileOptions {
    // ... existing fields ...
    pub cache_path: Option<PathBuf>,
    pub compile_env: Option<String>,  // NEW: for cache invalidation
}
```

In `AbcCompiler::compile()`:

1. If `cache_path` is set, attempt to load `AbcCache` from disk.
2. If `cache.compile_env != current_compile_env`, discard the cache (treat as cold).
3. For each function in the IR:
   - Compute `key = function_cache_key(function_source_slice)`.
   - If `cache.functions.contains_key(&key)`, skip IR lowering; reconstruct
     `MethodRecord` from the cached `AbcFunctionEntry`.
   - Otherwise, compile normally and insert the result into `cache.functions`.
4. After all functions are compiled, write the updated `AbcCache` back to disk.

### 4. CLI Wiring

In `crates/arkdown-cli/src/build.rs` (or equivalent):

```rust
let compile_env = compute_compile_env(&config);
let cache_path = config.cache_dir.join("abc");
std::fs::create_dir_all(&cache_path)?;

let abc_options = AbcCompileOptions {
    // ... existing fields ...
    cache_path: Some(cache_path),
    compile_env: Some(compile_env),
};
```

`compute_compile_env` mirrors `arkts-pack.js::getCompileEnv()`:

```rust
pub fn compute_compile_env(config: &ArkdownConfig) -> String {
    [
        &config.compile_sdk_version,
        &config.compatible_sdk_version,
        "OpenHarmony",                     // runtimeOS
        &config.ets_loader_path.to_string_lossy(),
        &config.obfuscation_enabled.to_string(),
        &config.module_type,               // "entry" | "har" | "hsp"
        &config.cache_dir.join("abc").to_string_lossy(),
    ]
    .join("#")
}
```

### 5. Cache Invalidation on obfuscation change

When `obfuscationEnabled` flips between builds, the `compileEnv` string changes
(it includes `obfuscationEnabled`), so the cache is automatically invalidated.
No separate logic is needed beyond the `compile_env` check.

---

## Files to Change

```
crates/arkdown-abc/src/
├── compiler.rs          — add cache_path + compile_env to AbcCompileOptions;
│                          load/write cache around function compilation loop
├── cache.rs             — AbcCache, AbcFunctionEntry, function_cache_key()
└── lib.rs               — re-export AbcCache

crates/arkdown-cli/src/
└── build.rs             — wire cache_path + compile_env into AbcCompileOptions
```

---

## Test Plan

- Cold build: `cache_path` dir is empty; after compile, `modules.abc.cache` exists.
- Warm build: second compile with identical source produces byte-identical `.abc`
  faster (verified by `tracing::debug` log: "ABC cache hit for function X").
- Source change: modifying a function's source text produces a cache miss; the new
  bytecode replaces the old entry in the cache file.
- `compile_env` change: changing `compile_sdk_version` causes all cache entries to be
  discarded; full recompile occurs.
- Corrupt cache file: if `modules.abc.cache` is not valid msgpack, `AbcCompiler`
  falls back to a cold compile without panicking.
- `cache_path: None`: no cache file is created; behavior is identical to current.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `trace()`, `doTaskAction()` cache setup, `checkAndCleanCache()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
  — `getCompileEnv()` function
- `https://gitcode.com/openharmony/arkcompiler_ets_frontend` — `es2abc --cache-file` flag
- `meta/tasks/SPEC-019-cache.md` — existing `arkdown-cache` crate, `cache_file` plumbing
- `meta/tasks/SPEC-029-rolldown-cache.md` — `compileEnv` key definition
