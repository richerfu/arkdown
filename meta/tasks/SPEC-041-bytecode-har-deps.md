# SPEC-041: ByteCode HAR Dependency Map in loader.json

**Crate**: `arkdown-profile`, `arkdown-resolver`
**Priority**: P1 — any project with pre-compiled HAR dependencies
**Blocked by**: SPEC-003 (resolver), SPEC-010 (profile processor)
**Blocks**: SPEC-042 (dynamic import info), SPEC-044 (integrated HSP)

---

## Summary

Populate `loader_json.byteCodeHarInfo` and `declarationEntry` from
`oh-package.json5` dependencies that ship pre-compiled bytecode. hvigor marks a
ByteCode HAR with `metadata.byteCodeHar: true`, expects the compiled artifact at
`ets/modules.abc`, and preserves runtime-only reachability through
`metadata.declarationEntry` normalized OHM URLs. The `byteCodeHarInfo` map is
consumed by the ABC compiler to resolve cross-HAR module references without
re-compiling the dependency.

---

## Background: What hvigor Does

From `byte-code-har-utils.js::collectByteCodeHarInfo()` and
`generate-loader-json.js::setByteCodeHarInfoAndOtherCompileFiles()`:

```javascript
const collectByteCodeHarInfo = (dep, byteCodeInfo) => {
  const packageName = dep.getPackageName();
  const root = dep.getDependencyRootPath();
  const abcPath = path.join(root, "ets", "modules.abc");
  const sourceMapsPath = path.join(root, "ets", "sourceMaps.map");
  const insightIntentJsonPath = collectByteCodeHarInsightIntentJsonPath(dep);
  const pkg = dep.getPackageJsonObj();

  byteCodeInfo.set(root, {
    abcPath,
    packageName,
    compatibleSdkVersion: pkg?.compatibleSdkVersion,
    compatibleSdkVersionStage: pkg?.compatibleSdkVersionStage,
    ...(existsSync(sourceMapsPath) ? { sourceMapsPath } : {}),
    ...(insightIntentJsonPath ? { insightIntentJsonPath } : {}),
  });
};
```

A ByteCode HAR's `oh-package.json5` looks like:

```json5
{
  "name": "my-bytecode-har",
  "version": "1.0.0",
  "main": "index.d.ts",
  "metadata": {
    "byteCodeHar": true,
    "declarationEntry": [
      "@normalized:N&entry&com.example&my-bytecode-har/src/main/ets/Worker&1.0.0"
    ]
  }
}
```

`compatibleSdkVersion` and `compatibleSdkVersionStage` are read from the packaged
`oh-package.json5`. hvigor's generated package also carries metadata such as
`declarationEntry`, `extraImportees`, `globalDependencies`, and resource metadata
when those features are used.

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

Represent the ByteCode HAR subset of `oh-package.json5#metadata`:

```rust
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OhPackageMetadata {
    /// hvigor's ByteCode HAR marker.
    pub byte_code_har: bool,
    /// Compatibility fallback for older fixtures.
    pub abc_path: Option<PathBuf>,
    /// Optional source maps for the ABC file.
    pub source_maps_path: Option<PathBuf>,
    /// Optional insight intent JSON path.
    pub insight_intent_path: Option<PathBuf>,
    /// Normalized OHM URLs for worker/runtimeOnly/startup/insight-intent records.
    pub declaration_entry: Vec<String>,
}
```

### 2. `is_bytecode_har()` Detection

```rust
pub fn is_bytecode_har(pkg_dir: &Path, pkg: &OhPackageJson5) -> bool {
    pkg.metadata.byte_code_har && pkg_dir.join("ets/modules.abc").exists()
}
```

### 3. `BytecodeHarInfo` Metadata

- `metadata.byteCodeHar` plus an existing `ets/modules.abc` is the hvigor-aligned
  ByteCode HAR signal.
- `byteCodeHarInfo[pkg].abcPath` points to absolute `<pkg>/ets/modules.abc`.
- `metadata.sourceMapsPath` is serialized as absolute `sourceMapsPath` when present.
- If `metadata.sourceMapsPath` is absent, `<pkg>/ets/sourceMaps.map` is used when
  it exists.
- `metadata.insightIntentPath` is serialized as absolute `insightIntent` when present.
- `metadata.abcPath`, `main` ending in `.abc`, and root `index.abc` remain
  compatibility fallbacks for older fixtures.
- `metadata.declarationEntry` is an array of normalized OHM URLs. It must be
  copied into loader.json `declarationEntry`; it is not a declaration file path.

### 4. `generate_loader_json()` Population

In `arkdown-profile::loader_json::generate_loader_json()`:

```rust
for dep in scan_bytecode_har_deps(module_root, project_root) {
    loader_json.byte_code_har_info.insert(
        dep.package_name.clone(),
        dep.into_loader_entry().1,
    );
    loader_json.declaration_entry.extend(dep.declaration_entry);
}
```

`scan_bytecode_har_deps()` walks module and project `oh_modules/`, reads
`oh-package.json5#metadata`, and resolves all serialized paths to absolute
filesystem paths.

### 5. Exclude ByteCode HARs from `compileEntry`

Ensure that `loader_json.compile_entry` does not include source files from ByteCode
HAR dependencies. The existing entry collection in `generate_loader_json()` should
skip any dependency where `is_bytecode_har()` is true.

### 6. `declarationEntry` Reachability

hvigor has two declaration-entry paths:

- When building a ByteCode HAR, `ProcessHarArtifacts.processDeclarationEntry()`
  generates `metadata.declarationEntry` from ability `srcEntry`, startup tasks,
  insight-intent entries, workers, `runtimeOnly.sources`, and bundled dependency
  config.
- When consuming a ByteCode HAR, `GenerateLoaderJson.processDeclarationEntry()`
  copies dependency `metadata.declarationEntry` into loader.json and adds entry
  OHM URLs for `runtimeOnly.packages` that reference ByteCode HARs.

Current arkdown support copies dependency `metadata.declarationEntry` verbatim and
deduplicates it. Generation of a ByteCode HAR's own `metadata.declarationEntry`
remains a separate packaging-stage task.

### 7. `harNameOhmMap` for ByteCode HARs

ByteCode HARs also need entries in `harNameOhmMap` when building a ByteCode HAR
target. hvigor derives those URLs from `pkgContextInfo` and
`ohmurl-utils.generateOhmUrl()`. The full dependency-manager graph parity is
still tracked separately.

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

- `OhPackageJson5` with `metadata.byteCodeHar: true` and `ets/modules.abc`:
  `is_bytecode_har()` returns `true`.
- `OhPackageJson5` without `metadata`: `is_bytecode_har()` returns `false`.
- `generate_loader_json()` with a ByteCode HAR dep: dep appears in `byteCodeHarInfo`,
  NOT in `compileEntry`.
- `generate_loader_json()` with a ByteCode HAR dep that has
  `metadata.declarationEntry`: loader.json `declarationEntry` contains the same
  normalized OHM URLs, deduplicated in first-seen order.
- `generate_loader_json()` with a source HAR dep: dep appears in `compileEntry` and
  `pkgNameToPkgBriefInfo`, NOT in `byteCodeHarInfo`.
- `BytecodeHarInfo.abc_path` is an absolute path (not relative to project root).
- `BytecodeHarInfo.source_maps_path` and `insight_intent` are absolute paths when
  `metadata.sourceMapsPath` / `metadata.insightIntentPath` are specified.
- `BytecodeHarInfo.compatible_sdk_version` is empty when not specified, matching the
  current scanner's direct metadata read.
- `loader.json` serialization: `byteCodeHarInfo` field is present and non-empty when
  ByteCode HAR deps exist; field is an empty object `{}` when no BC-HAR deps.
- Round-trip: serialized `loader.json` deserializes back to the same `BytecodeHarInfo`
  values.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/generate-loader-json.js`
  — `setByteCodeHarInfoAndOtherCompileFiles()`, `processDeclarationEntry()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/utils/byte-code-har-utils.js`
  — `collectByteCodeHarInfo()`, `collectByteCodeInfoAndOtherCompileEntrances()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/project/dependency/core/default-npm-dependency.js`
  — `isByteCodeHarDependency()` reads `metadata.byteCodeHar`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/process-har-artifacts.js`
  — ByteCode HAR package `metadata.declarationEntry` generation
- `meta/tasks/SPEC-010-profile-processor.md` — `LoaderJson` schema, `byteCodeHarInfo` stub
- `meta/design/module-resolution.md` — OHM URL format for HAR dependencies
- `meta/design/hvigorw-analysis.md` §9 — `loader.json` schema reference
