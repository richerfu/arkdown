# SPEC-012: HAP/HSP/HAR Packager

**Crate**: `arkdown-pack`
**Priority**: P0
**Blocked by**: SPEC-009 (abc), SPEC-010 (profile), SPEC-011 (resource)
**Blocks**: SPEC-013 (CLI), SPEC-021 (signing)

---

## Summary

Assembles compiled artifacts into the final HAP/HSP/HAR zip archive. Replaces hvigor's
`PackageHap`, `PackageHsp`, and `PackageHar` tasks. The output is an unsigned archive ready
for signing (SPEC-021).

---

## HAP Structure (zip)

```
entry.hap
├── module.json               ← from SPEC-010 write_module_json() (app + module sections merged)
├── pack.info                 ← bundle summary (see format below)
├── pkgContextInfo.json       ← package context OHM URL mappings (see format below)
├── resources.index           ← from SPEC-011 output (binary, RestoolV2 magic)
├── ets/
│   └── modules.abc           ← from SPEC-009 output (PANDA magic, API >= 9)
├── resources/
│   └── base/
│       ├── element/          ← string/color/etc resource files
│       ├── media/            ← image resources
│       └── profile/          ← profile JSON files (e.g., main_pages.json)
└── libs/
    └── arm64-v8a/
        └── *.so              ← native shared libraries (optional)
```

> **Note on `modules.abc` path**: For API >= 9 (stage model), the ABC is at `ets/modules.abc`.
> For API < 9 (FA model, legacy), it may be at the archive root as `modules.abc`.
> `module.json` must contain **both** `app` and `module` sections (see SPEC-010).

### `pack.info` Format

`pack.info` is a JSON file at the root of the HAP zip that summarizes the bundle:

```json
{
  "summary": {
    "app": {
      "bundleName": "com.example.myapp",
      "bundleType": "app",
      "version": {
        "code": 1000000,
        "name": "1.0.0"
      }
    },
    "modules": [
      {
        "mainAbility": "EntryAbility",
        "deviceTypes": ["phone", "tablet"],
        "name": "entry",
        "type": "entry",
        "abilities": [
          {
            "name": "EntryAbility",
            "label": "$string:EntryAbility_label",
            "exported": true
          }
        ]
      }
    ]
  },
  "packages": [
    {
      "deviceTypes": ["phone", "tablet"],
      "moduleType": "entry",
      "deliveryWithInstall": true,
      "name": "entry"
    }
  ]
}
```

### `pkgContextInfo.json` Format

`pkgContextInfo.json` maps package names to their OHM URL context for the runtime module registry:

```json
{
  "entry": {
    "packageName": "entry",
    "bundleName": "com.example.myapp",
    "moduleName": "entry",
    "version": "",
    "entryPath": "ets/modules.abc",
    "isSO": false
  }
}
```

### Resource File Copying

Resource files from `src/main/resources/` are copied verbatim into the HAP under the `resources/`
directory. The `resources.index` binary (compiled by SPEC-011) accompanies them. Both must be
present — the runtime uses `resources.index` for ID lookup and the `resources/` files for
actual content retrieval.

```
src/main/resources/base/element/string.json   → resources/base/element/string.json
src/main/resources/base/media/app_icon.png    → resources/base/media/app_icon.png
src/main/resources/base/profile/main_pages.json → resources/base/profile/main_pages.json
```

## HSP Structure (zip)

Same as HAP but `module.json` has `"type": "shared"`. May include source `.ets` files
in some configurations.

## HAR Structure (zip)

```
my-har.har
├── oh-package.json5          ← package manifest
├── src/main/ets/             ← original .ets source files (source HAR)
│   └── index.ets
├── src/main/resources/       ← resources
├── index.d.ts                ← generated type declarations
└── index.d.ets               ← ArkTS declaration file
```

ByteCode HAR (compiled artifact):

```
my-har.har
├── oh-package.json5
├── index.abc                 ← compiled bytecode
└── index.d.ts
```

---

## API

```rust
pub struct Packager {
    config: Arc<ArkdownConfig>,
}

pub struct PackInput {
    pub module: ModuleConfig,
    pub abc_path: PathBuf,              // ets/modules.abc from SPEC-009
    pub resources_index: PathBuf,       // resources.index from SPEC-011
    pub resources_src_dir: Option<PathBuf>, // src/main/resources/ — copied into HAP
    pub module_json: PathBuf,           // processed module.json from SPEC-010 (app+module merged)
    pub native_libs: Vec<PathBuf>,      // .so files (optional)
    pub output_path: PathBuf,           // e.g., build/default/outputs/default/entry.hap
}

impl Packager {
    pub fn pack_hap(&self, input: PackInput) -> Result<PathBuf, ArkdownError>;
    pub fn pack_hsp(&self, input: PackInput) -> Result<PathBuf, ArkdownError>;
    pub fn pack_har(&self, input: HarPackInput) -> Result<PathBuf, ArkdownError>;
}
```

Use the `zip` crate for archive creation. The archive must be stored (not compressed) for
`.abc` and `.so` files per OHOS requirements, and deflate-compressed for JSON/text files.

---

## Output Path

```
build/default/outputs/default/entry.hap
build/default/outputs/default/shared.hsp
build/default/outputs/har/my-har.har
```

---

## Acceptance Criteria

- `pack_hap()` produces a zip that DevEco Studio can install to an OHOS device/emulator
- Zip entry paths match what hvigor produces (verified by manual comparison)
- `modules.abc` placed at correct path within zip for the given API version

---

## Design References

- `meta/design/pipeline.md` — Phase 12 (HAP/HSP/HAR Packaging)
- `meta/design/chunk-model.md` — per-module bundle layout (HAP/HSP/HAR sections)
- `meta/design/hvigorw-analysis.md` — PackageHap task, zip structure

---

## Rolldown Alignment

`arkdown-pack` has no rolldown equivalent. rolldown produces JS files; it does not create zip archives. The packaging step is unique to OHOS.

| arkdown concept                                      | closest analogue                        | note                                                                                             |
| ---------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `Packager::pack_hap()`                               | webpack `ZipPlugin` / Vite build output | Conceptually similar (archive JS output); OHOS zip structure and compression rules are specific  |
| `PackInput::abc_path`                                | webpack `output.path`                   | The `.abc` file is the analogue of the JS bundle; it replaces the JS output in the final archive |
| HAP/HSP/HAR zip structure                            | npm `pack` (produces `.tgz`)            | Both create distributable archives; different format and manifest requirements                   |
| `modules.abc` placement path (API version dependent) | no equivalent                           | OHOS-specific: the path inside the zip differs between API versions                              |
| `.abc` / `.so` stored (no deflate)                   | webpack `assetModuleFilename`           | OHOS requirement: runnable files must be stored (not compressed) for mmap-based loading          |
| 4096-byte alignment for `.abc`/`.so`                 | no JS bundler equivalent                | OHOS-specific: required for page-aligned mmap; enforced via `ZipEntry` extra field padding       |

---

## Test Requirements

- `pack_hap()` produces a ZIP file with `module.json`, `pack.info`, `pkgContextInfo.json`, `resources.index`, `ets/modules.abc`, and `resources/` files at the correct paths
- `module.json` in the HAP contains both `"app"` and `"module"` top-level keys
- `pack.info` parses as valid JSON with `summary.app.bundleName` matching the project bundle name
- `pkgContextInfo.json` parses as valid JSON with the module entry mapped correctly
- `resources/base/` directory in the HAP contains copied resource files from `src/main/resources/base/`
- When `resources_src_dir` is `None`, the `resources/` directory is still absent from the HAP (no crash)
- `modules.abc` is stored (compression method 0) in the ZIP — not deflate-compressed
- `module.json` and `resources.index` are deflate-compressed in the ZIP
- ZIP entry paths inside the archive match hvigor's output exactly (verified via manual comparison with a reference HAP)
- `pack_har()` includes `oh-package.json5` and `src/main/ets/` source files for source HARs
- ByteCode HAR pack: `pack_har()` includes only `oh-package.json5`, `index.abc`, and `index.d.ts`
- `pack_hsp()` sets `"type": "shared"` in `module.json` (inherited from profile processor output)
- Native `.so` files are placed under `libs/arm64-v8a/` in the HAP ZIP
- Output path `build/default/outputs/default/entry.hap` is created by `pack_hap()`
- Empty `native_libs` list: no `libs/` directory created in the ZIP
- Missing `abc_path` input returns `Err` (not panic)
- Produced HAP can be installed on an OHOS device via `hdc install` (acceptance criterion; requires device)
