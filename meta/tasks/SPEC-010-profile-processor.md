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

### Output: module.json (in intermediates)

```json
{
  "module": {
    "name": "entry",
    "type": "entry",
    "virtualMachine": "ark3.5.0.0",
    "compileMode": "esmodule",
    "deviceTypes": ["phone", "tablet"],
    ...
  }
}
```

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
  "moduleRootPath": "/abs/path/to/entry",
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
