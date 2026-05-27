# TEST-010: arkdown-profile

**Crate**: `arkdown-profile`

---

## Test File Layout

```
crates/arkdown-profile/src/
├── tests/
│   ├── process_profile.rs    ← module.json5 transformation
│   ├── loader_json.rs        ← loader.json generation + entry collection
│   ├── har_abilities.rs      ← HAR dependency ability merging
│   └── router_map.rs         ← router map entry collection
```

---

## process_profile.rs

### TC-001: `virtualMachine` set to `"ark<version>"` for API >= 9

- **Input**: module.json5 with `compileMode` not set; SDK API version 12
- **Expected**: output `module.json` has `"virtualMachine": "ark3.5.0.0"` (or version-appropriate value)

### TC-002: `compileMode` set to `"esmodule"` for stage model

- **Expected**: `"compileMode": "esmodule"` in output

### TC-003: `deviceTypes` from product config overrides module config

- **Input**: product with `deviceTypes: ["phone"]`, module with `deviceTypes: ["tablet"]`
- **Expected**: output uses product's `["phone"]`

### TC-004: output written to `build/default/intermediates/res/<target>/module.json`

- **Expected**: `process_profile()` writes file to correct intermediates path

---

## loader_json.rs

### TC-005: ability srcEntry included in `compileEntry`

- **Input**: module.json5 ability `{ srcEntry: "./ets/entryability/EntryAbility.ets" }`
- **Expected**: `loader_json.compile_entry` contains `"./ets/entryability/EntryAbility.ets"`

### TC-006: multiple abilities all included in `compileEntry`

- **Input**: two abilities with different `srcEntry` values
- **Expected**: both in `compileEntry`

### TC-007: worker file from `buildOption.sourceOption.workers` included

- **Input**: `"sourceOption": { "workers": ["./ets/workers/DataWorker.ets"] }`
- **Expected**: `"./ets/workers/DataWorker.ets"` in `compileEntry`

### TC-008: ByteCode HAR appears in `byteCodeHarInfo`, NOT in `compileEntry`

- **Input**: dependency with `metadata.abcPath = "index.abc"`
- **Expected**: `loader_json.byte_code_har_info["dep-name"].abc_path` is set;
  no entry in `compile_entry` for this dep

### TC-009: `pkgNameToPkgBriefInfo` contains entry module

- **Expected**: `loader_json.pkg_name_to_pkg_brief_info["entry"].pkg_root` is set

### TC-010: `useNormalizedOHMUrl` propagated from product config

- **Input**: product with `strictMode.useNormalizedOHMUrl: true`
- **Expected**: `loader_json.use_normalized_ohm_url == true`

---

## har_abilities.rs

### TC-011: HAR dependency abilities merged into module abilities

- **Setup**: HAP depends on local HAR; HAR has ability `{ srcEntry: "./ets/HarAbility.ets" }`
- **Expected**: merged `module.json` abilities include HAR's ability with OHM-URL srcEntry

### TC-012: HAR ability srcEntry converted to OHM URL

- **Input**: HAR ability `srcEntry = "./ets/HarAbility.ets"`, bundle `com.example`, HAR module `my-har`
- **Expected**: merged ability has `srcEntry = "@bundle:com.example/my-har/ets/HarAbility"`

---

## router_map.rs

### TC-013: router map entries included in `compileEntry`

- **Input**: router map JSON with `{ pageSourceFile: "./ets/pages/DetailPage.ets" }`
- **Expected**: `"./ets/pages/DetailPage.ets"` added to `compileEntry`
