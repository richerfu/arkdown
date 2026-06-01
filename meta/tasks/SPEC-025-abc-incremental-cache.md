# SPEC-025: ABC Incremental Build Cache

**Crate**: `arkdown-abc`
**Priority**: P1
**Status**: PARTIAL / INTERNAL CACHE ONLY (updated 2026-06-01)

---

## Overview

The current implementation is a per-compilation SHA-256 keyed JSON cache used by
the Rust backend. It is useful as an internal development cache, but it is **not**
hvigor parity. hvigor passes `--cache-file @modules.cache` to es2abc; that file
maps CompileArkTS cache sources to `.protoBin` ProgramCache files, including
`npmEntries.txt -> npmEntries.protoBin`.

Full parity requires replacing the JSON cache path with upstream-compatible
`modules.cache`/protoBin ProgramCache generation and merge semantics.

---

## API

No public API changes beyond `AbcCompileOptions::cache_path: Option<PathBuf>`,
plus `AbcCompileOptions::npm_entries: Option<PathBuf>`.

### `AbcCompileOptions::cache_path`

```rust
/// Optional path to arkdown's internal JSON incremental cache directory.
///
/// When set, the compiler stores compiled `.abc` bytecode in
/// `<cache_path>/modules.abc.cache` (JSON format).  On subsequent runs
/// with identical inputs the cached bytecode is used directly, skipping
/// IR lowering. This is not hvigor's `--cache-file @modules.cache` format.
pub cache_path: Option<PathBuf>,

/// Optional hvigor/es2abc `--npm-module-entry-list` file. This is lowered as
/// upstream `CompileNpmModuleEntryList()` records and is included in the JSON
/// cache key.
pub npm_entries: Option<PathBuf>,
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

- **`key`**: SHA-256 hex digest of all module record names, source texts,
  optional `npmEntries.txt` text, and the `module` / `debug_info` compile
  options (in order).
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

Any change to a source text, record name, npm-entry list, or compile option
produces a new key and triggers a full recompile.

---

## Implementation

### New file: `crates/arkdown-abc/src/cache.rs`

- `CacheFile` — on-disk JSON structure with `load`, `save`, `lookup`, `upsert`
- `derive_key` — SHA-256 key derivation over all module inputs + options

### Modified: `crates/arkdown-abc/src/compiler.rs`

`AbcCompiler::compile` now:

1. Before compilation: calls `derive_key` + `CacheFile::load` + `lookup`.
   On hit: writes cached bytes to output and returns early.
2. Lowers `npmEntries.txt` as upstream-style records when present.
3. After compilation: calls `CacheFile::load` + `upsert` + `save`.
   Cache write errors are non-fatal (silently ignored).

### Modified: `crates/arkdown-abc/Cargo.toml`

Added workspace dependencies: `sha2`, `serde`, `serde_json`, `hex`, `tracing`.

Diagnostics must use `tracing::debug!`/`tracing::warn!`; do not add a custom
debug logging mechanism.

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
