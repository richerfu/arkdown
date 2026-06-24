# TEST-002: arkdown-config

**Crate**: `arkdown-config`

---

## Test File Layout

```
crates/arkdown-config/src/
├── tests/
│   ├── build_profile.rs      ← build-profile.json5 parsing
│   ├── oh_package.rs         ← oh-package.json5 parsing
│   ├── module_json.rs        ← module.json5 parsing
│   ├── sdk_discovery.rs      ← OHOS_SDK_HOME resolution
│   └── merge.rs              ← product × module × target merging
```

---

## build_profile.rs

### TC-001: parses standard build-profile.json5 successfully

- **Input**: valid `build-profile.json5` with one product and one module
- **Expected**: `ArkdownConfig` with `product.name == "default"`, `modules.len() == 1`

### TC-002: `compatibleSdkVersion "5.0.0(12)"` → `compile_api_version = 12`

- **Input**: `"compatibleSdkVersion": "5.0.0(12)"`
- **Expected**: `sdk_config.compile_api_version == 12`

### TC-003: `compatibleSdkVersion "4.1.0(11)"` → 11

- **Expected**: `compile_api_version == 11`

### TC-004: missing `bundleName` returns error

- **Input**: product without `bundleName` field
- **Expected**: `Err(ArkdownError::ConfigError { field: "bundleName", ... })`

### TC-005: JSON5 comments allowed

- **Input**: `build-profile.json5` with `// comment` lines
- **Expected**: parses without error

### TC-006: multiple products selectable by name

- **Input**: two products `"default"` and `"release"`
- **Expected**: `from_project(..., product_name: Some("release"))` returns the release product

---

## oh_package.rs

### TC-007: `file:` dependency parsed as local path

- **Input**: `{ "dependencies": { "my-har": "file:../my-har" } }`
- **Expected**: dependency entry has `is_file_ref = true`, `path = "../my-har"`

### TC-008: ohpm dependency parsed as version spec

- **Input**: `{ "dependencies": { "@ohos/lib": "^1.0.0" } }`
- **Expected**: dependency entry has `is_file_ref = false`

### TC-009: missing `name` field does not error (optional)

- **Input**: `{}` (minimal oh-package.json5)
- **Expected**: `OhPackageJson5 { name: None, ... }` — no error

---

## module_json.rs

### TC-010: parses abilities array correctly

- **Input**: module.json5 with one ability `{ name: "EntryAbility", srcEntry: "./ets/entryability/EntryAbility.ets" }`
- **Expected**: `module_json.module.abilities[0].src_entry == "./ets/entryability/EntryAbility.ets"`

### TC-011: `pages: "$profile:main_pages"` parsed as profile reference string

- **Expected**: `module_json.module.pages == Some("$profile:main_pages")`

---

## sdk_discovery.rs

### TC-012: `OHOS_SDK_HOME` env var used when set

- **Input**: `OHOS_SDK_HOME=/custom/sdk`
- **Expected**: `sdk_config.es2abc_path` starts with `/custom/sdk`

### TC-013: missing `OHOS_SDK_HOME` and no default location → error

- **Input**: env var unset, no standard location exists
- **Expected**: `Err(ArkdownError::SdkNotFound { ... })`

### TC-014: platform-correct binary name selected

- **Condition**: `#[cfg(target_os = "macos")]`
- **Expected**: path contains `build-mac`; `#[cfg(target_os = "linux")]` → `build-linux`

---

## merge.rs

### TC-015: `buildOption` from product applied to module config

- **Input**: product with `strictMode.caseSensitiveCheck: true`
- **Expected**: all `ModuleConfig.build_option.strict_mode.case_sensitive_check == true`

### TC-016: `--module entry` scopes output to single module

- **Input**: two modules `entry` and `feature`; `module_name = Some("entry")`
- **Expected**: `ArkdownConfig.modules.len() == 1`, name `"entry"`
