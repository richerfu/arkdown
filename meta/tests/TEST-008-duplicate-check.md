# TEST-008: arkdown duplicate-check Rolldown adapter

**Crate**: `arkdown`
**Owner**: `crates/arkdown/src/plugins/duplicate_check.rs`
**Original source**: `hvigor-arkts-compose/dist/src/plugins/check-duplicate-filenames-plugin.js`

---

## Test File Layout

```
crates/arkdown/src/plugins/
└── duplicate_check.rs        ← implementation and focused unit tests
```

---

## detection.rs

### TC-001: two `.ets` files with same basename → error

- **Input graph**:
  - `src/main/ets/utils/Logger.ets`
  - `src/main/ets/components/Logger.ets`
- **Expected**: `Err(ArkdownError::DuplicateFilename { stem: "Logger", ... })`
- **Origin**: plugin collects all module file stems; second occurrence of `"Logger"` triggers error

### TC-002: error message names both conflicting paths

- **Expected**: error contains both `src/main/ets/utils/Logger.ets` and
  `src/main/ets/components/Logger.ets`

### TC-003: two files with different basenames → ok

- **Input graph**: `utils/Logger.ets` and `components/Button.ets`
- **Expected**: `Ok(())`

### TC-004: same basename, different extensions `.ets` vs `.ts` → error

- **Input**: `utils/Helper.ets` and `components/Helper.ts`
- **Expected**: `Err(ArkdownError::DuplicateFilename { stem: "Helper" })`
- **Origin**: check uses `file_stem()` (strips extension), so both are `"Helper"`

### TC-005: single-module project with all unique names → ok

- **Input**: 20 modules with unique basenames
- **Expected**: `Ok(())`

---

## exclusions.rs

### TC-006: `.d.ts` declaration files excluded from check

- **Input graph**: `utils/Logger.ets` and `@ohos/hilog.d.ts` (stem `"hilog"`)
  plus another `utils/hilog.ets`
- The `@ohos/hilog.d.ts` is `ModuleType::Declaration`
- **Expected**: `"hilog.d.ts"` is not counted; only `"hilog.ets"` checked

### TC-007: `.d.ets` files excluded

- **Input**: module with `module_type == ModuleType::Declaration`
- **Expected**: excluded from duplicate check

### TC-008: `external = true` modules excluded

- **Input**: `@bundle:com.foo/mod/path` (external) and local `path.ets`
- **Expected**: no conflict — external modules not checked

### TC-009: `ModuleType::BytecodeHar` excluded

- **Input**: ByteCode HAR stub module alongside local source with same stem
- **Expected**: no conflict reported
