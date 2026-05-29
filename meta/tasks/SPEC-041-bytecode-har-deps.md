# SPEC-041: ByteCode HAR Dependency Map in loader.json

**Crate**: `arkdown-profile`, `arkdown-resolver`
**Priority**: P1 — any project with pre-compiled HAR dependencies
**Blocked by**: SPEC-003 (resolver), SPEC-010 (profile processor)
**Blocks**: SPEC-042 (dynamic import info), SPEC-044 (integrated HSP)

---

## Summary

Populate `loader_json.byteCodeHarInfo` from `oh-package.json5` dependencies that
reference pre-compiled `.abc` packages. A ByteCode HAR is a HAR whose `oh-package.json5`
has `metadata.abcPath` set, indicating it ships bytecode instead of source. The
`byteCodeHarInfo` map is consumed by the ABC compiler to resolve cross-HAR module
references without re-compiling the dependency.

---

## Background: What hvigor Does

From `generate-loader-json.js::doTaskAction()` (via `DependencyManager`):

```javascript
// byteCodeHarInfo population
const byteCodeHarDeps = this.dependencyManager.getByteCodeHarDependencies();
byteCodeHarDeps.forEach(dep => {
    const packageName = dep.getPackageName();
    const abcPath = dep.getAbcPath();           // from oh-package.json5#metadata.abcPath
    const sourceMapsPath = dep.getSourceMapsPath(); // optional
    const compatibleSdkVersion = dep.getCompatibleSdkVersion();
    const insightIntent = dep.getInsightIntentPath(); // optional

    byteCodeHarInfo[packageName] = {
        packageName,
        abcPath,
        sourceMapsPath,
        compatibleSdkVersion,
        insightIntent,
    };
});
```

A ByteCode HAR's `oh-package.json5` looks like:

```json5
{
  "name": "my-bytecode-har",
  "version": "1.0.0",
  "main": "index.d.ts",
  "metadata": {
    "abcPath": "./index.abc",
    "sourceMapsPath": "./index.abc.map",
    "insightIntentPath": "./insight_intent.json"
  }
}
```

The `compatibleSdkVersion` comes from the HAR's `build-profile.json5` or is derived
from the installed package metadata.

ByteCode HAR dependencies must NOT appear in `compileEntry` (they are pre-compiled).
They must appear in `byteCodeHarInfo` instead.

From `ark-compile.js::setByteCodeHarToDependencyKeys()`:

```javascript
// byteCodeHarToDependencyKeys: maps HAR package names to their dependency keys
// Used by the ABC compiler to resolve cross-HAR ldexternalmodulevar references
this.byteCodeHarToDependencyKeys = getByteCodeHarToDependencyKeys(this.targetService);
```

---

## Scope

### 1. `OhPackageMetadata` Extension

Extend `OhPackageJson5` in `arkdown-config` to include the `metadata` field:

```rust
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OhPackageMetadata {
    /// Path to pre-compiled ABC file (relative to package root).
    /// Presence of this field marks the package as a ByteCode HAR.
    pub abc_path: Option<PathBuf>,
    /// Optional source maps for the ABC file.
    pub source_maps_path: Option<PathBuf>,
    /// Optional insight intent JSON path.
    pub insight_intent_path: Option<PathBuf>,
}

// In OhPackageJson5:
pub metadata: Option<OhPackageMetadata>,
```

### 2. `is_bytecode_har()` Detection

```rust
impl OhPackageJson5 {
    /// Returns true if this package ships pre-compiled bytecode (ByteCode HAR).
    pub fn is_bytecode_har(&self) -> bool {
        self.metadata
            .as_ref()
            .and_then(|m| m.abc_path.as_ref())
            .is_some()
    }
}
```

### 3. `BytecodeHarInfo` Struct

Already exists in `arkdown-profile::loader_json` as a partial stub. Complete it:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BytecodeHarInfo {
    pub package_name: String,
    /// Absolute path to the pre-compiled .abc file.
    pub abc_path: PathBuf,
    /// Optional absolute path to source maps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_maps_path: Option<PathBuf>,
    /// Compatible SDK version string, e.g. "5.0.0(12)".
    pub compatible_sdk_version: String,
    /// Optional insight intent JSON path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insight_intent: Option<PathBuf>,
}
```

### 4. `generate_loader_json()` Population

In `arkdown-profile::loader_json::generate_loader_json()`:

```rust
// For each dependency in oh-package.json5 resolved by arkdown-resolver:
for dep in resolved_deps {
    let oh_pkg = load_oh_package_json5(&dep.root_path)?;
    if oh_pkg.is_bytecode_har() {
        let meta = oh_pkg.metadata.as_ref().unwrap();
        let abc_path = dep.root_path.join(meta.abc_path.as_ref().unwrap());
        let source_maps_path = meta.source_maps_path.as_ref()
            .map(|p| dep.root_path.join(p));
        let insight_intent = meta.insight_intent_path.as_ref()
            .map(|p| dep.root_path.join(p));

        loader_json.by_byte_code_har_info.insert(
            dep.package_name.clone(),
            BytecodeHarInfo {
                package_name: dep.package_name.clone(),
                abc_path,
                source_maps_path,
                compatible_sdk_version: dep.compatible_sdk_version
                    .unwrap_or_else(|| "5.0.0(12)".to_string()),
                insight_intent,
            },
        );
        // ByteCode HAR must NOT appear in compileEntry
        // (already excluded by the dep.is_bytecode_har() check in entry collection)
    }
}
```

### 5. Exclude ByteCode HARs from `compileEntry`

Ensure that `loader_json.compile_entry` does not include source files from ByteCode
HAR dependencies. The existing entry collection in `generate_loader_json()` should
skip any dependency where `is_bytecode_har()` is true.

### 6. `harNameOhmMap` for ByteCode HARs

ByteCode HARs also need entries in `harNameOhmMap` so the resolver can map package
names to OHM URLs for cross-module references:

```rust
// For each ByteCode HAR dep:
loader_json.har_name_ohm_map.insert(
    dep.package_name.clone(),
    format!(
        "@bundle:{}/{}/{}",
        config.bundle_name,
        dep.module_name,
        dep.entry_ohm_path,
    ),
);
```

---

## Files to Change

```
crates/arkdown-config/src/
└── oh_package.rs       — add OhPackageMetadata, is_bytecode_har() to OhPackageJson5

crates/arkdown-profile/src/
├── loader_json.rs      — complete BytecodeHarInfo; populate byteCodeHarInfo in
│                         generate_loader_json(); exclude BC-HARs from compileEntry
└── lib.rs              — re-export BytecodeHarInfo

crates/arkdown-resolver/src/
└── lib.rs              — expose is_bytecode_har() on resolved dependencies
```

---

## Test Plan

- `OhPackageJson5` with `metadata.abcPath` set: `is_bytecode_har()` returns `true`.
- `OhPackageJson5` without `metadata`: `is_bytecode_har()` returns `false`.
- `generate_loader_json()` with a ByteCode HAR dep: dep appears in `byteCodeHarInfo`,
  NOT in `compileEntry`.
- `generate_loader_json()` with a source HAR dep: dep appears in `compileEntry` and
  `pkgNameToPkgBriefInfo`, NOT in `byteCodeHarInfo`.
- `BytecodeHarInfo.abc_path` is an absolute path (not relative to project root).
- `BytecodeHarInfo.source_maps_path` is `None` when not specified in `oh-package.json5`.
- `BytecodeHarInfo.compatible_sdk_version` defaults to `"5.0.0(12)"` when not specified.
- `loader.json` serialization: `byteCodeHarInfo` field is present and non-empty when
  ByteCode HAR deps exist; field is an empty object `{}` when no BC-HAR deps.
- Round-trip: serialized `loader.json` deserializes back to the same `BytecodeHarInfo`
  values.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/generate-loader-json.js`
  — `doTaskAction()` `byteCodeHarInfo` population, `DependencyManager` usage
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `setByteCodeHarToDependencyKeys()`, `isByteCodeHarDependency()`
- `meta/tasks/SPEC-010-profile-processor.md` — `LoaderJson` schema, `byteCodeHarInfo` stub
- `meta/design/module-resolution.md` — OHM URL format for HAR dependencies
- `meta/design/hvigorw-analysis.md` §9 — `loader.json` schema reference
