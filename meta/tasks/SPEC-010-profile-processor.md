# SPEC-010: Profile Processor (module.json5 + loader.json)

**Crate**: `arkdown-profile`
**Priority**: P0
**Blocked by**: SPEC-002
**Blocks**: SPEC-005 (needs loader.json), SPEC-012

---

## Summary

Implements two closely related tasks from hvigor:

1. **MergeProfile + ProcessProfile**: merge and transform `module.json5` into the final
   `module.json` placed in the intermediates directory
2. **GenerateLoaderJson**: produce `loader.json` — the manifest that tells the bundler
   (SPEC-005) where entry points are and how to map module identities

---

## Part A: Profile Processing

### Input: module.json5

```json5
{
  module: {
    name: "entry",
    type: "entry",
    description: "$string:module_desc",
    mainElement: "EntryAbility",
    deviceTypes: ["phone", "tablet"],
    pages: "$profile:main_pages",
    abilities: [
      {
        name: "EntryAbility",
        srcEntry: "./ets/entryability/EntryAbility.ets",
        exported: true,
        skills: [{ entities: ["entity.system.home"] }],
      },
    ],
  },
}
```

### Processing Steps (from hvigor's ProcessProfile)

1. Set `module.virtualMachine = "ark<version>"` for ArkTS stage mode (API >= 9)
2. Set `module.compileMode = "esmodule"` (or `"jsbundle"` for legacy FA model)
3. Set `module.deviceTypes` from product config
4. Collect HAR dependency abilities (`collectHarAbilities`) and merge into `module.abilities`
5. For each HAR ability `srcEntry`, generate OHM URL:
   `@bundle:<bundleName>/<harModuleName>/<relativePath>`
6. Set `module.requestPermissions` from product overlay if present

### Output: module.json (in intermediates and in HAP)

The final `module.json` placed in the HAP **must contain both `app` and `module` sections**:

```json
{
  "app": {
    "bundleName": "com.example.myapp",
    "vendor": "example",
    "versionCode": 1000000,
    "versionName": "1.0.0",
    "icon": "$media:app_icon",
    "label": "$string:app_name"
  },
  "module": {
    "name": "entry",
    "type": "entry",
    "virtualMachine": "ark12.0.1.0",
    "compileMode": "esmodule",
    "deviceTypes": ["phone", "tablet"],
    ...
  }
}
```

The `app` section is sourced from `AppScope/app.json5` (via SPEC-002 `load_app_json5()`).
The `module` section is the processed `module.json5` content with the fields below added.

**`write_module_json()` method**: merges `app` + `module` into the final JSON and writes it
to both:

- `build/default/intermediates/res/<target>/module.json` (intermediates)
- Embedded into the HAP zip at `module.json` (by SPEC-012)

Key fields added by `process_profile()`:

| Field                   | Value example   | Rule                                                               |
| ----------------------- | --------------- | ------------------------------------------------------------------ |
| `module.virtualMachine` | `"ark12.0.1.0"` | `"ark<apiVersion>.0.1.0"` for API >= 9 (stage model)               |
| `module.compileMode`    | `"esmodule"`    | `"esmodule"` for stage model (API >= 9); `"jsbundle"` for FA model |

Output path: `build/default/intermediates/res/<target>/module.json`

---

## Part B: loader.json Generation

`loader.json` is the contract between the profile processor and the bundler. The bundler
reads this file instead of directly reading `module.json5`.

### Key Fields

```json
{
  "compileEntry": ["./ets/entryability/EntryAbility.ets", "./ets/pages/Index.ets"],
  "projectRootPath": "/abs/path/to/project",
  "moduleRootPath": "/abs/path/to/entry/src/main",
  "moduleName": "entry",
  "apiVersion": 12,
  "modulePathMap": { "entry": "/abs/path/to/entry" },
  "fallbackAnBuild": false,
  "cachePath": "build/default/intermediates/ets/default/loader_out/default/cache/",
  "aceModuleBuild": "build/default/intermediates/ets/default/loader_out/default/",
  "buildMode": "debug",
  "supportChunks": false,
  "useNormalizedOHMUrl": true,
  "packageManagerType": "ohpm",
  "pkgNameToPkgBriefInfo": {
    "entry": {
      "pkgRoot": "/path/to/entry",
      "pkgName": "entry",
      "sourceRoots": ["src/main/ets", ".hvigor/default/generated/"],
      "originalSourceRoots": []
    }
  },
  "byteCodeHarInfo": {
    "my-bytecode-har": {
      "packageName": "my-bytecode-har",
      "abcPath": "/path/to/index.abc",
      "compatibleSdkVersion": "5.0.0(12)"
    }
  },
  "hspNameOhmMap": {},
  "harNameOhmMap": {
    "my-har": "@bundle:com.example/my-har/src/main/ets/Index"
  }
}
```

Field notes:

| Field             | Value                               | Change from prior spec                    |
| ----------------- | ----------------------------------- | ----------------------------------------- |
| `moduleRootPath`  | `/abs/path/to/entry/src/main`       | Now points to `src/main`, not module root |
| `moduleName`      | `"entry"`                           | Added field; OHOS module name             |
| `apiVersion`      | `12`                                | Added field; numeric API version          |
| `modulePathMap`   | `{ "entry": "/abs/path/to/entry" }` | Added field; name → absolute path map     |
| `fallbackAnBuild` | `false`                             | Added field; AN build fallback flag       |

### Entry Point Collection

Collect `compileEntry` from:

1. `module.json5#abilities[*].srcEntry`
2. `module.json5#module.srcEntry`
3. `buildOption.sourceOption.workers` array
4. Router map `pageSourceFile` entries (from `processRouterMap`)

### ByteCode HAR Info

For each dependency that is a ByteCode HAR (has `metadata.abcPath` in its `oh-package.json5`):

```rust
if let Some(abc_path) = dep.oh_package.metadata.as_ref().and_then(|m| m.abc_path.as_ref()) {
    byte_code_har_info.insert(dep.name.clone(), BytecodeHarInfo { ... });
}
```

---

## API

```rust
pub struct ProfileProcessor {
    config: Arc<ArkdownConfig>,
}

impl ProfileProcessor {
    pub fn process_profile(&self, module: &ModuleConfig) -> Result<HapManifest, ArkdownError>;
    pub fn generate_loader_json(&self, module: &ModuleConfig, manifest: &HapManifest)
        -> Result<LoaderJson, ArkdownError>;
    pub fn write_loader_json(&self, loader_json: &LoaderJson, path: &Path) -> Result<(), ArkdownError>;

    /// Write merged module.json (app + module sections combined) to the intermediates path.
    /// This is the file placed directly in the HAP zip by SPEC-012.
    pub fn write_module_json(&self, manifest: &HapManifest, path: &Path) -> Result<(), ArkdownError>;
}
```

---

## Files to Create

```
crates/arkdown-profile/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── process_profile.rs   (MergeProfile + ProcessProfile logic)
    ├── loader_json.rs       (GenerateLoaderJson: collect entries, write JSON)
    ├── router_map.rs        (router map entry collection)
    └── har_abilities.rs     (collectHarAbilities from HAR dependencies)
```

---

## Acceptance Criteria

- `process_profile()` on `MyApplication/entry` produces valid `module.json` with `virtualMachine` set
- `generate_loader_json()` includes all abilities from `module.json5` in `compileEntry`
- `loader.json` matches the format that the bundler (SPEC-005) expects
- ByteCode HAR deps appear in `byteCodeHarInfo`, not in `compileEntry`

---

## Design References

- `meta/design/hvigorw-analysis.md` — GenerateLoaderJson task, ProcessProfile task, loader.json schema
- `meta/design/pipeline.md` — Phase 11 (module.json5 merge)
- `meta/design/chunk-model.md` — entry point collection from module.json5
- `meta/design/types.md` — `HapManifest`, `ModuleJson5`, `OhPackageJson5`

---

## Rolldown Alignment

| arkdown concept                     | rolldown equivalent                               | note                                                                                                                                                                 |
| ----------------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `generate_loader_json()`            | rolldown `InputOptions::input` derivation         | Both determine bundler entry points; rolldown receives them as explicit options; arkdown derives them from `module.json5` abilities and writes them to `loader.json` |
| `LoaderJson::compileEntry`          | rolldown `input` array                            | Direct equivalent: list of entry file paths for the bundler                                                                                                          |
| `LoaderJson::pkgNameToPkgBriefInfo` | no rolldown equivalent                            | OHOS-specific: maps package names to source roots; used by the resolver to locate local HAR files                                                                    |
| `LoaderJson::byteCodeHarInfo`       | no rolldown equivalent                            | OHOS-specific: maps HAR package names to pre-compiled ABC paths                                                                                                      |
| `process_profile()`                 | no rolldown equivalent                            | Produces `module.json`; rolldown has no manifest processing concept                                                                                                  |
| `HapManifest::virtual_machine`      | no rolldown equivalent                            | OHOS-specific: `"ark3.5.0.0"` version string set in output `module.json`                                                                                             |
| `LoaderJson::supportChunks`         | rolldown `manualChunks` / chunk splitting options | Analogue: controls whether shared chunks are extracted                                                                                                               |
| `LoaderJson::aceModuleBuild`        | rolldown `OutputOptions::dir`                     | Both specify the output directory for JS chunk files                                                                                                                 |

---

## Test Requirements

- `process_profile()` on a minimal `module.json5` with one ability sets `module.virtualMachine = "ark<version>"` and `module.compileMode = "esmodule"` in the output `module.json`
- `generate_loader_json()` includes all `abilities[*].srcEntry` paths in `compileEntry`
- `generate_loader_json()` includes `module.srcEntry` in `compileEntry` when present
- Worker files from `buildOption.sourceOption.workers` appear in `compileEntry`
- ByteCode HAR dependency (with `metadata.abcPath`) appears in `byteCodeHarInfo`, not in `compileEntry`
- `pkgNameToPkgBriefInfo` includes the entry module's own package info (not just dependencies)
- Local HAR dependency (`file:../my-har`) appears in `pkgNameToPkgBriefInfo` with the correct `pkgRoot` and `sourceRoots`
- HAR abilities from dependencies are collected and merged into `module.abilities` in `process_profile()`
- HAR ability `srcEntry` is rewritten to OHM URL `"@bundle:bundleName/harModuleName/relativePath"` in the output
- `write_loader_json()` produces valid JSON that round-trips through `serde_json::from_str`
- Missing `module.json5` file returns `Err` (not panic)
- `module.json5` with `pages: "$profile:main_pages"` preserves the `$profile:` reference in the output `module.json`
- `LoaderJson::buildMode` is `"debug"` for `BuildMode::Debug` and `"release"` for `BuildMode::Release`
