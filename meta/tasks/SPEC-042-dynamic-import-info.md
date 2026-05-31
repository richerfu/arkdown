# SPEC-042: Dynamic Import Library Info in loader.json

**Crate**: `arkdown-profile`, `arkdown` (bundler core)
**Priority**: P2 — dynamic import cross-HAR patterns
**Blocked by**: SPEC-041 (ByteCode HAR deps), SPEC-005 (bundler)
**Blocks**: —

---

## Summary

Detect hvigor dynamic-import surfaces and populate
`loader.json#dynamicImportLibInfo`. Besides post-bundle `import()` resolution,
hvigor also uses `buildOption.arkOptions.runtimeOnly.packages` and dependency
`metadata.runtimeOnly` to describe packages loaded only at runtime. This map is
read by the ArkRuntime to locate the entry file for dynamically imported modules.

---

## Background: What hvigor Does

From `generate-loader-json.js::processDynamicImport()` and
`arkts-pack.js::generateConfig()`:

```javascript
const runtimeOnly = buildOption?.arkOptions?.runtimeOnly;
runtimeOnly?.packages?.forEach((pkg) => {
  this.isHarDependency(pkg)
    ? this.processHarDynamicImport(pkg, this.dynamicImportLibInfo)
    : this.processOtherDynamicImport(pkg, this.dynamicImportLibInfo);
});

// dynamicImportLibInfo shape:
{
    "my-har": {
        entryFilePath: "/abs/path/to/my-har/src/main/ets/Index.ets",
        moduleName: "my-har",
    }
}
```

The `dynamicImportLibInfo` is built from explicit runtime-only package metadata
and, in the full hvigor pipeline, from dynamic import resolution during bundling.
The resolver records the dependency's entry file path, module name, package path,
local-dependency flag, and host module info.

From `generate-loader-json.js::doTaskAction()`:

```javascript
// After bundling, read dynamicImportLibInfo from the compile config
const dynamicImportLibInfo = projectConfig.dynamicImportLibInfo ?? {};
loaderJson.dynamicImportLibInfo = dynamicImportLibInfo;
```

The distinction from static imports: static HAR imports appear in `harNameOhmMap` and
are fully bundled. Dynamic imports (`import('pkg')`) are not bundled — they are loaded
at runtime by the ArkRuntime module loader, which uses `dynamicImportLibInfo` to locate
the HAR's ABC file.

---

## Scope

### 1. `DynamicImportLibInfo` Struct

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DynamicImportLibInfo {
    /// Absolute path to the HAR's entry .ets file.
    pub entry_file_path: PathBuf,
    /// HAR module name (package name from oh-package.json5).
    pub module_name: String,
}
```

Add to `LoaderJson`:

```rust
pub struct LoaderJson {
    // ... existing fields ...
    /// Dynamic import library info: pkgName → { entryFilePath, moduleName }
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub dynamic_import_lib_info: HashMap<String, DynamicImportLibInfo>,
}
```

### 2. Dynamic Import Detection in the Bundler

After rolldown bundling completes, walk the module graph to find dynamic imports that
resolve to HAR packages:

```rust
pub fn collect_dynamic_import_lib_info(
    bundle_output: &BundleOutput,
    resolved_deps: &[ResolvedDependency],
) -> HashMap<String, DynamicImportLibInfo> {
    let mut result = HashMap::new();

    for module in bundle_output.modules() {
        for dynamic_import in &module.dynamic_imports {
            // Check if this dynamic import resolves to a HAR package
            if let Some(dep) = resolved_deps.iter().find(|d| {
                d.package_name == dynamic_import.specifier
                    || d.ohm_url == dynamic_import.resolved_id
            }) {
                if dep.is_har() && !dep.is_bytecode_har() {
                    result.insert(dep.package_name.clone(), DynamicImportLibInfo {
                        entry_file_path: dep.entry_file_path.clone(),
                        module_name: dep.package_name.clone(),
                    });
                }
            }
        }
    }

    result
}
```

### 3. Resolver Plugin Hook

The `arkdown-plugin-ohos-resolve` plugin should track dynamic import resolutions during
the `resolveId` hook and expose them via a shared context:

```rust
impl ArkdownPlugin for OhosResolvePlugin {
    async fn resolve_id(
        &self,
        ctx: &ResolveContext,
        specifier: &str,
        importer: Option<&str>,
        options: &ResolveOptions,
    ) -> PluginResult<Option<ResolveResult>> {
        let result = self.resolver.resolve(specifier, importer)?;
        // Track dynamic imports
        if options.is_dynamic_import {
            if let Some(dep) = self.resolver.is_har_package(specifier) {
                self.dynamic_import_tracker
                    .lock()
                    .insert(dep.package_name.clone(), dep.entry_file_path.clone());
            }
        }
        Ok(result)
    }
}
```

### 4. `generate_loader_json()` Population

In `arkdown-profile::loader_json::generate_loader_json()`, accept the dynamic import
info collected during bundling:

```rust
pub fn generate_loader_json(
    config: &ArkdownConfig,
    module: &ModuleConfig,
    manifest: &HapManifest,
    dynamic_import_lib_info: HashMap<String, DynamicImportLibInfo>,  // NEW
) -> Result<LoaderJson, ArkdownError> {
    // ... existing logic ...
    loader_json.dynamic_import_lib_info = dynamic_import_lib_info;
    Ok(loader_json)
}
```

### 5. CLI Integration

In the build pipeline, pass the collected dynamic import info from the bundler to the
profile processor:

```rust
let (bundle_output, dynamic_import_info) = bundle_with_dynamic_import_tracking(
    &bundle_config, &compile_env
).await?;

let loader_json = profile_processor.generate_loader_json(
    &config, &module, &manifest,
    dynamic_import_info,
)?;
```

---

## Files to Change

```
crates/arkdown-profile/src/
├── loader_json.rs          — add DynamicImportLibInfo; populate dynamic_import_lib_info
│                             in generate_loader_json()
└── lib.rs                  — re-export DynamicImportLibInfo

crates/arkdown-plugin-ohos-resolve/src/
└── lib.rs                  — track dynamic import resolutions in resolveId hook

crates/arkdown/src/
└── lib.rs                  — collect_dynamic_import_lib_info(); expose from bundle output

crates/arkdown-cli/src/
└── build.rs                — wire dynamic_import_info from bundler to profile processor
```

---

## Test Plan

- `buildOption.arkOptions.runtimeOnly.packages` containing `my-har`: `my-har`
  appears in `dynamicImportLibInfo` with correct `entryFilePath` and host module info.
- Source file with `import('my-har')` where `my-har` is a local HAR dep: `my-har`
  appears in `dynamicImportLibInfo` with correct `entryFilePath` and `moduleName`.
- Static import `import { x } from 'my-har'`: `my-har` does NOT appear in
  `dynamicImportLibInfo` (only in `harNameOhmMap`).
- Dynamic import of a non-HAR module (e.g., `import('./local-file.ets')`): not added
  to `dynamicImportLibInfo`.
- Dynamic import of a ByteCode HAR: appears in `byteCodeHarInfo`, NOT in
  `dynamicImportLibInfo`.
- `loader.json` with no dynamic imports: `dynamicImportLibInfo` is `{}` (empty object).
- `DynamicImportLibInfo.entry_file_path` is an absolute path.
- Round-trip: serialized `loader.json` with `dynamicImportLibInfo` deserializes correctly.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/generate-loader-json.js`
  — `dynamicImportLibInfo` field in `doTaskAction()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
  — `generateConfig()` `dynamicImportLibInfo` from resolver
- `meta/tasks/SPEC-031-bytecode-har-deps.md` — ByteCode HAR dependency detection
- `meta/tasks/SPEC-015-ohos-resolve-plugin.md` — `OhosResolvePlugin` hooks
- `meta/design/module-resolution.md` — HAR resolution and OHM URL format
