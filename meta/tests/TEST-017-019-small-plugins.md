# TEST-017: arkdown-plugin-router-map

# TEST-018: arkdown-plugin-sensitive-check

# TEST-019: arkdown-cache

---

## TEST-017: arkdown-plugin-router-map

```
crates/arkdown-plugin-router-map/src/tests/
├── invalidation.rs     ← cache invalidated when router map changes
```

### TC-001: cache valid when router map hash unchanged

- **Setup**: router map hash same as stored hash
- **Expected**: `should_invalidate_cache()` returns `false`

### TC-002: cache invalid when router map content changes

- **Setup**: router map JSON changed (different hash)
- **Expected**: `should_invalidate_cache()` returns `true`

### TC-003: cache invalid on first build (no stored hash)

- **Setup**: no cached hash available
- **Expected**: returns `true`

---

## TEST-018: arkdown-plugin-sensitive-check

```
crates/arkdown-plugin-sensitive-check/src/tests/
├── case_match.rs       ← correct case passes
├── case_mismatch.rs    ← wrong case errors on macOS/Windows
└── disabled.rs         ← no-op when check disabled
```

### TC-001: correct case on macOS → no error

- **Setup**: `case_sensitive_check = true`; filesystem has `Helper.ets`; import `"./utils/Helper"`
- **Expected**: `Ok(None)` — defers after validation passes

### TC-002: wrong case on macOS → `CaseMismatch` error

- **Setup**: `case_sensitive_check = true`; filesystem has `Helper.ets`; import `"./utils/helper"` (lowercase)
- **Expected**: `Err(ArkdownError::CaseMismatch { expected: "helper.ets", actual: "Helper.ets" })`

### TC-003: disabled → no filesystem check, no error

- **Setup**: `case_sensitive_check = false`; filesystem has `Helper.ets`; import `"./utils/helper"`
- **Expected**: `Ok(None)` — plugin returns None immediately without checking FS

### TC-004: Linux → always no-op (FS is case-sensitive by itself)

- **Condition**: `#[cfg(target_os = "linux")]`
- **Expected**: plugin body skipped; no `read_dir` call

---

## TEST-019: arkdown-cache

```
crates/arkdown-cache/src/tests/
├── key_derivation.rs   ← cache key computed correctly
├── invalidation.rs     ← key changes trigger cache miss
└── storage.rs          ← read/write cache entries on disk
```

### TC-001: same content + same config → same cache key

- **Setup**: file content and config unchanged
- **Expected**: `CacheKey` identical on both builds

### TC-002: file content changed → different content_hash

- **Setup**: source file modified
- **Expected**: `cache_key.content_hash` changes

### TC-003: config changed (`useNormalizedOHMUrl` toggled) → different config_hash

- **Setup**: `use_normalized_ohm_url` changed from false to true
- **Expected**: `cache_key.config_hash` changes; all cached entries invalid

### TC-004: dependency resolved ID changed → different dependency_hash

- **Setup**: `oh-package.json5` version bump causes different resolution
- **Expected**: `cache_key.dependency_hash` changes

### TC-005: valid cache entry retrieved without re-parsing

- **Setup**: module cached from previous build; key unchanged
- **Expected**: `Module::from_cached(entry)` returned; no oxc parse call

### TC-006: cache miss when any plugin returns `should_invalidate_cache = true`

- **Setup**: plugin returns `true`; cache entry otherwise valid
- **Expected**: full load/transform pipeline runs; old entry discarded
