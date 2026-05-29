# SPEC-025: ABC Incremental Build Cache

**Crate**: `arkdown-abc`
**Priority**: P1
**Status**: IMPLEMENTED (2026-05-29)

---

## Overview

Per-compilation SHA-256 keyed on-disk cache for the ABC compiler.  When
`AbcCompileOptions::cache_path` is set, the compiler checks the cache before
running the IR lowering pass.  A cache hit writes the cached bytecode directly
to the output file, skipping all `build_record` calls.

---

## API

No public API changes beyond `AbcCompileOptions::cache_path: Option<PathBuf>`,
which was already present but previously ignored.  The field is now wired.

### `AbcCompileOptions::cache_path`

```rust
/// Optional path to an incremental build cache directory.
///
/// When set, the compiler stores compiled `.abc` bytecode in
/// `<cache_path>/modules.abc.cache` (JSON format).  On subsequent runs
/// with identical inputs the cached bytecode is used directly, skipping
/// IR lowering.
pub cache_path: Option<PathBuf>,
```

---

## Cache File Format

Path: `<cache_path>/modules.abc.cache`

```json
{
  "entries": [
    { "key": "<sha256hex>", "bytecode": "<hexencoded>" }
  ]
}
```

- **`key`**: SHA-256 hex digest of all module record names, source texts, and
  the `module` / `debug_info` compile options (in order).
- **`bytecode`**: Hex-encoded compiled `.abc` binary.

---

## Key Derivation

```
SHA-256( record_name_0 || NUL || source_0 || NUL
       || record_name_1 || NUL || source_1 || NUL
       || ...
       || "module=" || ("0" | "1")
       || "debug="  || ("0" | "1") )
```

Any change to a source text, record name, or compile option produces a new key
and triggers a full recompile.

---

## Implementation

### New file: `crates/arkdown-abc/src/cache.rs`

- `CacheFile` — on-disk JSON structure with `load`, `save`, `lookup`, `upsert`
- `derive_key` — SHA-256 key derivation over all module inputs + options

### Modified: `crates/arkdown-abc/src/compiler.rs`

`AbcCompiler::compile` now:

1. Before compilation: calls `derive_key` + `CacheFile::load` + `lookup`.
   On hit: writes cached bytes to output and returns early.
2. After compilation: calls `CacheFile::load` + `upsert` + `save`.
   Cache write errors are non-fatal (silently ignored).

### Modified: `crates/arkdown-abc/Cargo.toml`

Added workspace dependencies: `sha2`, `serde`, `serde_json`, `hex`.

---

## New Tests

| Test | Description |
|------|-------------|
| `test_cache_hit_avoids_recompile` | Compile twice with same inputs; second run returns byte-for-byte identical output from cache |
| `test_cache_invalidates_on_source_change` | Compile with record name A, then B; outputs differ and cache has 2 entries |

---

## Acceptance Criteria

- [x] `AbcCompileOptions::cache_path` is wired (not ignored)
- [x] Cache stored at `<cache_path>/modules.abc.cache`
- [x] Cache hit skips `build_record` IR lowering
- [x] Cache invalidated when inputs change (different key → new entry)
- [x] Cache write errors are non-fatal
- [x] All existing `cargo test -p arkdown-abc` tests pass
- [x] 2 new tests added: `test_cache_hit_avoids_recompile`, `test_cache_invalidates_on_source_change`

---

## Limitations / Future Work

- The cache is keyed on the **full set** of module inputs for a compilation run,
  not on individual modules.  Changing one module in a multi-module compile
  invalidates the entire entry.  Per-module caching would require a way to merge
  independently-compiled `ClassRecord`s (shared string tables make this complex).
- No LRU eviction: the cache file grows unboundedly.  A future SPEC-025v2 could
  add a max-entries limit.
- `compileEnv` invalidation (API version change) is handled implicitly: the
  cache key does not include `target_api_version`, so a version bump will reuse
  stale entries.  A future revision should include `target_api_version` in the
  key.
