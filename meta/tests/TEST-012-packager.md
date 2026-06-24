# TEST-012: arkdown-pack

**Crate**: `arkdown-pack`

---

## Test File Layout

```
crates/arkdown-pack/src/
├── tests/
│   ├── hap_structure.rs      ← HAP zip entry paths match hvigor output
│   ├── har_structure.rs      ← HAR zip layout
│   └── compression.rs        ← .abc and .so stored uncompressed
```

---

## hap_structure.rs

### TC-001: HAP zip contains `module.json` at root

- **Input**: minimal `PackInput` with all required artifacts
- **Expected**: zip entry `"module.json"` exists at root level

### TC-002: HAP zip contains `resources.index` at root

- **Expected**: zip entry `"resources.index"` at root

### TC-003: HAP zip contains `modules.abc` at root

- **Expected**: zip entry `"modules.abc"` at root
  (or `"ets/modules.abc"` — verify against actual hvigor output format)

### TC-004: native `.so` libraries placed at `libs/arm64-v8a/`

- **Input**: native lib `libfoo.so`
- **Expected**: zip entry `"libs/arm64-v8a/libfoo.so"`

### TC-005: no extra files in HAP beyond expected entries

- **Expected**: zip only contains the known set of entries; no stray temp files

---

## har_structure.rs

### TC-006: source HAR contains `oh-package.json5` at root

- **Expected**: zip entry `"oh-package.json5"`

### TC-007: source HAR contains ETS source files under `src/main/ets/`

- **Input**: HAR with `src/main/ets/index.ets`
- **Expected**: zip entry `"src/main/ets/index.ets"`

---

## compression.rs

### TC-008: `.abc` files stored with `Stored` (no compression) method

- **Expected**: zip entry for `modules.abc` has `compression_method == Stored`
- **Note**: required by OHOS runtime loader

### TC-009: `.so` files stored uncompressed

- **Expected**: zip entry for `.so` has `compression_method == Stored`

### TC-010: JSON files may be deflate-compressed

- **Expected**: `module.json` may use `Deflated` method (not required to be stored)
