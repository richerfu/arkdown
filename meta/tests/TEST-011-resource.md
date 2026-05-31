# TEST-011: arkdown-resource (Resource Compiler)

**Crate**: `arkdown-resource`
**Reference**: `https://github.com/openharmony/developtools_global_resource_tool`

---

## Test File Layout

```
crates/arkdown-resource/src/
├── tests/
│   ├── element_parsing.rs    ← string/color/float/integer JSON parsing
│   ├── qualifier_scan.rs     ← directory qualifier detection (en_US, zh_CN, etc.)
│   ├── id_assignment.rs      ← numeric resource ID assignment rules
│   ├── binary_format.rs      ← resources.index binary structure
│   ├── locale_merge.rs       ← base + locale override merging
│   └── profile_output.rs     ← profile JSON writing (router map, startup)
```

---

## element_parsing.rs

### TC-001: parse `string.json` into string resource entries

- **Input** `base/element/string.json`:
  ```json
  { "string": [{ "name": "app_name", "value": "My App" }] }
  ```
- **Expected**: `[ResourceEntry { type: String, name: "app_name", value: "My App" }]`

### TC-002: parse `color.json`

- **Input**: `{ "color": [{ "name": "primary", "value": "#FF0000" }] }`
- **Expected**: `ResourceEntry { type: Color, name: "primary", value: Color(255,0,0) }`

### TC-003: multiple entries in one file

- **Input**: `string.json` with 5 string entries
- **Expected**: 5 `ResourceEntry` instances in order

### TC-004: missing `value` field → parse error

- **Input**: `{ "string": [{ "name": "x" }] }` (no value)
- **Expected**: `Err(ResourceError::MissingField { field: "value" })`

---

## qualifier_scan.rs

### TC-005: `base/` directory has empty qualifier

- **Setup**: directory `resources/base/element/string.json`
- **Expected**: resource qualifier is `""` (default/fallback)

### TC-006: `en_US/` directory has `en_US` qualifier

- **Setup**: `resources/en_US/element/string.json`
- **Expected**: qualifier is `"en_US"`

### TC-007: `zh_CN/` directory has `zh_CN` qualifier

- **Expected**: qualifier is `"zh_CN"`

### TC-008: unrecognized qualifier directory is ignored with warning

- **Setup**: `resources/invalid_qual/`
- **Expected**: warning logged; directory skipped; no error

---

## id_assignment.rs

### TC-009: first app resource gets ID starting at 0x01000000

- **Setup**: one string resource `app_name`
- **Expected**: `app_name` assigned ID `0x01000000`

### TC-010: IDs assigned sequentially within a module

- **Setup**: three string resources
- **Expected**: IDs `0x01000000`, `0x01000001`, `0x01000002`

### TC-011: IDs stable across recompilation (same source → same IDs)

- **Setup**: compile same resource dir twice
- **Expected**: `ResourceTable.txt` identical both times

### TC-012: IDs for different resource types are independent sequences

- **Setup**: one string and one color resource
- **Expected**: both get IDs starting from `0x01000000` — or verify actual id scheme
  from `developtools_restool` source

---

## binary_format.rs

### TC-013: output `resources.index` starts with correct magic bytes

- **Expected**: first 4 bytes match Panda resource magic (from restool source)

### TC-014: all resource entries present in binary output

- **Input**: 3 string resources and 2 color resources
- **Expected**: binary contains 5 resource entries

### TC-015: string values encoded as UTF-8 in binary

- **Input**: string `"Hello 世界"`
- **Expected**: UTF-8 encoding present in binary string section

---

## locale_merge.rs

### TC-016: base resource present when no locale override

- **Setup**: `base/element/string.json` has `app_name = "My App"`, no `en_US/` override
- **Expected**: compiled resource has one value: `app_name → "My App"` for all locales

### TC-017: locale override supplements base resource

- **Setup**: `base/` has `app_name = "My App"`, `zh_CN/` has `app_name = "我的应用"`
- **Expected**: compiled resource has two values: base `"My App"`, zh_CN `"我的应用"`

### TC-018: resource only in locale dir (no base) → compiles with locale qualifier

- **Setup**: string only in `en_US/`, not in `base/`
- **Expected**: resource entry with qualifier `"en_US"`, no fallback value

---

## profile_output.rs

### TC-019: router map JSON written to `resources/base/profile/routerMap.json`

- **Input**: router map config with one route entry
- **Expected**: file created at `output/resources/base/profile/routerMap.json`

### TC-020: profile JSON content matches input

- **Input**: `{ "routerMap": [{ "name": "detail", "pageSourceFile": "..." }] }`
- **Expected**: written file contains same JSON (pretty-printed or compact)

### TC-021: no profile file written when no router map config

- **Expected**: `routerMap.json` does not exist in output
