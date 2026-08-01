# TEST-001: arkdown-core

**Crate**: `arkdown-core`

---

## Test File Layout

```
crates/arkdown-core/src/
├── tests/
│   ├── error_display.rs      ← ArkdownError message formatting
│   └── type_sizes.rs         ← ensure hot-path types stay small
```

---

## error_display.rs

### TC-001: `ResolutionFailed` message contains specifier and importer

- **Input**: `ArkdownError::ResolutionFailed { specifier: "@ohos/foo", importer: Some("bar.ets"), reason: "not found" }`
- **Expected display**: contains `"@ohos/foo"` and `"bar.ets"`

### TC-002: `AbcCompilationFailed` message contains stderr output

- **Input**: `ArkdownError::AbcCompilationFailed { message: "unexpected token at line 5" }`
- **Expected display**: contains `"unexpected token at line 5"`

---

## type_sizes.rs

### TC-003: `ModuleType` is small (enum, no heap allocation)

- **Expected**: `std::mem::size_of::<ModuleType>() == 1`

### TC-004: `ResolvedId` does not exceed reasonable size

- **Expected**: `std::mem::size_of::<ResolvedId>() <= 64`
