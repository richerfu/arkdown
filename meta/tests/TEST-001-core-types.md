# TEST-001: arkdown-core

**Crate**: `arkdown-core`

---

## Test File Layout

```
crates/arkdown-core/src/
├── tests/
│   ├── plugin_trait.rs       ← ArkdownPlugin default impl + dispatch
│   ├── error_display.rs      ← ArkdownError message formatting
│   └── type_sizes.rs         ← ensure hot-path types stay small
```

---

## plugin_trait.rs

### TC-001: default hook implementations return Ok(None) / Ok(())

- **Setup**: zero-impl plugin struct implementing `ArkdownPlugin`
- **Expected**: all hooks return their default (None or Ok) without panicking

### TC-002: plugin name is returned correctly

- **Setup**: plugin with `fn name() -> &str { "my-plugin" }`
- **Expected**: `plugin.name() == "my-plugin"`

### TC-003: `should_invalidate_cache` defaults to false

- **Expected**: default impl returns `false`

---

## error_display.rs

### TC-004: `ResolutionFailed` message contains specifier and importer

- **Input**: `ArkdownError::ResolutionFailed { specifier: "@ohos/foo", importer: Some("bar.ets"), reason: "not found" }`
- **Expected display**: contains `"@ohos/foo"` and `"bar.ets"`

### TC-005: `AbcCompilationFailed` message contains stderr output

- **Input**: `ArkdownError::AbcCompilationFailed { message: "unexpected token at line 5" }`
- **Expected display**: contains `"unexpected token at line 5"`

---

## type_sizes.rs

### TC-006: `ModuleType` is small (enum, no heap allocation)

- **Expected**: `std::mem::size_of::<ModuleType>() == 1`

### TC-007: `ResolvedId` does not exceed reasonable size

- **Expected**: `std::mem::size_of::<ResolvedId>() <= 64`
