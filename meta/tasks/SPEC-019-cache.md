# SPEC-019: Incremental Build Cache

**Crate**: `arkdown-cache`
**Priority**: P1
**Status**: ✅ Wired into CLI (2026-05-29); bundle-level cache added (2026-05-29)
**Blocked by**: SPEC-005 (bundler), SPEC-009 (ABC compiler)

---

## Summary

Persistent incremental build cache that allows consecutive builds of unchanged
source trees to skip expensive ABC compilation.  Stores per-module content
hashes and config fingerprints on disk; a cache hit requires all three key
components to match.

---

## Architecture

### `BuildCache` — on-disk module transform cache

Stored at `<module>/build/default/cache/modules/<fnv1a_hex(id)>.json`.

Each JSON file is a serialised [`CachedEntry`]:
```json
{
  "id": "./ets/entryability/EntryAbility.ets",
  "key": {
    "content_hash": "a9f3c4d2e1b08765",
    "config_hash": 13764602127382145123,
    "dependency_hash": 0
  },
  "transformed_code": "",
  "source_map": null
}
```

### `CacheKey` — three-component invalidation key

| Component        | Description                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------ |
| `content_hash`   | FNV-1a 64-bit hex of the source file content. Changes when source is edited.                                       |
| `config_hash`    | FNV-1a of `build_mode:bundle_name:api_version:use_normalized_ohm_url` (+ obfuscation rule hash for release builds). Changes when build configuration changes. |
| `dependency_hash`| Hash of direct import specifier IDs. Currently always `0` (not yet implemented). Extend when cross-module caching is required. |

### `BundleCache` — bundle-output cache

Stored at `<module>/build/default/cache/bundle/bundle.json`.

Persists a single [`BundleCacheManifest`] JSON file containing the cache key
and per-chunk metadata.  On a cache hit the entire rolldown bundling step is
skipped and the previous `.js` chunk files on disk are reused directly.

```json
{
  "key": { "source_hash": 12345678901234567890, "config_hash": 9876543210987654321 },
  "chunks": [
    {
      "name": "EntryAbility",
      "path": "/…/loader_out/default/ets/EntryAbility.js",
      "is_entry": true,
      "is_dynamic_entry": false,
      "facade_module_id": "/…/src/main/ets/entryability/EntryAbility.ets",
      "exports": [],
      "imports": [],
      "dynamic_imports": [],
      "source_map": null
    }
  ],
  "source_maps_path": null
}
```

#### `BundleCacheKey`

| Component      | Description                                                               |
| -------------- | ------------------------------------------------------------------------- |
| `source_hash`  | FNV-1a over all compile-entry file contents (sorted by path).            |
| `config_hash`  | FNV-1a of `build_mode:bundle_name:api:use_normalized_ohm_url:support_chunks:module_name`. |

#### Cache validity check

`BundleCache::is_valid(&key)` returns `true` if all of:
1. `bundle.json` exists and parses successfully.
2. `manifest.key == key`.
3. Every chunk whose `path` has extension `.js` still exists on disk.

#### CLI integration

The bundle cache is checked **before** the stale `.js` cleanup so a hit can
reuse the existing files.  On a miss the manifest is saved after a successful
rolldown run.

```
Check BundleCache::is_valid()
  ├─ Hit  → reconstruct BundleOutput from manifest + disk .js files → continue
  └─ Miss → clean stale .js → run rolldown → BundleCache::save() → continue
```

### ABC compiler cache

Passed as `cache_file: Some(PathBuf)` to `compile_to_abc` (ts2abc driver).
Stored at `<module>/build/default/cache/abc/modules.abc.cache`.

The Rust ABC compiler will write incremental data to this file when that
feature is implemented.  The file is created (empty) by the CLI on first build
as a presence marker.

---

## CLI Integration (build.rs)

### Config hash computation — `compute_config_hash(config)`

```
config_hash = FNV1a(build_mode + ":" + bundle_name + ":" + api_version + ":" + use_normalized_ohm_url)
```

For release builds with obfuscation enabled, the obfuscation rule file hash
(`compute_rule_hash(&rule_files)`) is XOR'd in:

```
config_hash ^= rule_hash
config_hash *= FNV_PRIME
```

### ABC skip logic

Before calling `compile_to_abc`:
1. Load `BuildCache` from `build/default/cache/modules/`.
2. For every entry in `loader_json.compile_entry`, read the source file and
   compute a `CacheKey`.
3. If `modules_abc_path.exists()` **AND** all `cache.is_valid(entry_id, &key)`
   calls return `true` → skip ABC compilation (`continue` to next module).
4. Otherwise: compile, then `cache.insert(CachedEntry { ... })` for each entry.

### Obfuscation + cache ordering

```
Bundle .js chunks
  ↓
Obfuscation (release + enabled) — modifies .js in-place
  ↓
Cache check (ETS source content hash + config hash)
  ↓
[skip if cache hit + modules.abc exists]
  ↓
ABC compilation (with cache_file path)
  ↓
Update module BuildCache entries
  ↓
Delete .js chunks
```

The cache key is derived from the original `.ets` source (pre-obfuscation).
Obfuscation is deterministic given the same source + same rule hash; including
the rule hash in `config_hash` ensures a rule change triggers a full rebuild.

---

## Cache Invalidation Scenarios

| Change                                 | Effect                                                    |
| -------------------------------------- | --------------------------------------------------------- |
| Source file edited                     | `content_hash` changes → rebuild                          |
| Build mode switched (debug ↔ release)  | `config_hash` changes → rebuild                           |
| Bundle name or API version changed     | `config_hash` changes → rebuild                           |
| Obfuscation rules file edited          | `config_hash` changes (rule hash mixed in) → rebuild      |
| `modules.abc` deleted manually         | `modules_abc_path.exists()` false → rebuild               |
| Cache JSON files deleted               | `is_valid()` false for all entries → rebuild              |

---

## Public API

```rust
pub struct BuildCache { ... }
impl BuildCache {
    pub fn new(cache_dir: PathBuf) -> Self;
    pub fn load_from_disk(&mut self) -> Result<(), CacheError>;
    pub fn get(&self, id: &str) -> Option<&CachedEntry>;
    pub fn is_valid(&self, id: &str, key: &CacheKey) -> bool;
    pub fn insert(&mut self, entry: CachedEntry) -> Result<(), CacheError>;
    pub fn invalidate_all(&mut self) -> Result<(), CacheError>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

pub fn compute_content_hash(content: &str) -> String;  // FNV-1a hex-16
```

---

## Test Coverage

`crates/arkdown-cache/src/lib.rs` — unit tests (22 tests):
- roundtrip persist/reload
- content hash determinism
- cache miss on changed content/config/dependency hash
- `invalidate_all` clears memory and disk
- `get()` and `len()` semantics
- concurrent inserts to separate directories
- `BundleCache` save/load roundtrip
- `BundleCache::is_valid()` key match, key mismatch, missing chunk file, chunk file present
- `BundleCache::load()` returns `None` for cold cache
- `BundleCacheKey` equality/inequality

`crates/arkdown-cli/tests/cache_test.rs` — integration tests (5 tests):
- `test_first_build_writes_cache_entries` — module cache dir populated after build
- `test_first_build_writes_abc_cache_file` — ABC cache file created after build
- `test_two_builds_produce_identical_abc` — byte-identical ABC on second build
- `test_bundle_cache_manifest_written_after_first_build` — `bundle.json` written after first build
- `test_second_build_uses_bundle_cache_and_produces_identical_abc` — second build uses bundle cache, produces identical `modules.abc`

---

## TODO / Future Work

- Wire `dependency_hash` from rolldown module graph (cross-module change detection).
- Implement incremental data writing in `arkdown-abc` compiler using `cache_file`.
- Add cache pruning / TTL to prevent unbounded growth.
- Consider SHA-256 content hash (faster on large files with hardware acceleration).
- Store chunk code in `BundledChunkMeta` to avoid requiring `.js` files on disk for cache hit (currently the hit path assumes `.js` files are present).
