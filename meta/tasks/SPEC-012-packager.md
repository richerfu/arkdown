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
├── module.json               ← from SPEC-010 output
├── resources.index           ← from SPEC-011 output
├── modules.abc               ← from SPEC-009 output (merged ABC)
├── ets/
│   └── modules.abc           ← alternative location (API version dependent)
└── libs/
    └── arm64-v8a/
        └── *.so              ← native shared libraries (optional)
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
    pub abc_path: PathBuf,            // modules.abc from SPEC-009
    pub resources_index: PathBuf,     // resources.index from SPEC-011
    pub module_json: PathBuf,         // processed module.json from SPEC-010
    pub native_libs: Vec<PathBuf>,    // .so files (optional)
    pub output_path: PathBuf,         // e.g., build/default/outputs/default/entry.hap
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
