# SPEC-002: Config Parser

**Crate**: `arkdown-config`
**Priority**: P0
**Blocked by**: SPEC-001
**Blocks**: SPEC-010 (profile), SPEC-011 (resource), SPEC-013 (CLI)

---

## Summary

Parse all OHOS project configuration files into the `ArkdownConfig` struct defined in
`arkdown-core`. This is the first step of every build — all subsequent phases read from
the resolved config.

---

## Files Consumed

| File                  | Location                  | Purpose                                                                                                    |
| --------------------- | ------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `build-profile.json5` | project root              | products, modules, signing configs, build options                                                          |
| `oh-package.json5`    | each module root          | package name, version, dependencies                                                                        |
| `module.json5`        | each module's `src/main/` | abilities, srcEntry, deviceTypes, pages                                                                    |
| `hvigorfile.ts`       | project root              | plugin registration (arkdown reads this only to validate it's a standard app project; does not execute it) |

---

## Key Requirements

### build-profile.json5 Schema

```json5
{
  app: {
    products: [{
      name: "default",
      bundleName: "com.example.myapp",
      compatibleSdkVersion: "5.0.0(12)",  // → compile_api_version = 12
      runtimeOS: "HarmonyOS",
      signingConfigs: [{
        name: "default",
        material: { certpath, keyAlias, keyPassword, profile, signAlg, storeFile, storePassword }
      }],
      buildOption: {
        strictMode: { caseSensitiveCheck: true, useNormalizedOHMUrl: true },
        arkOptions: { obfuscation: { ruleOptions: { enable: false } } }
      }
    }],
    buildModeSet: [{ name: "debug" }, { name: "release" }]
  },
  modules: [{
    name: "entry",
    srcPath: "./entry",
    targets: [{ name: "default", applyToProducts: ["default"] }]
  }]
}
```

### oh-package.json5 Schema

```json5
{
  modelVersion: "5.0.0",
  name: "entry",
  version: "1.0.0",
  dependencies: {
    "@ohos/lib": "^1.0.0",
    "my-local-har": "file:../my-local-har",
  },
  devDependencies: {},
  dynamicDependencies: {},
}
```

### SDK Path Resolution

The SDK is located via (in order):

1. `OHOS_SDK_HOME` environment variable
2. `~/Library/Huawei/Sdk/` (macOS DevEco Studio default)
3. `~/AppData/Local/Huawei/Sdk/` (Windows)
4. `/opt/huawei/sdk/` (Linux)

From the SDK path, derive:

- `es2abc`: `sdk/default/openharmony/ets/build-tools/ets-loader/bin/ark/build-{platform}/bin/es2abc`
- `restool`: `sdk/default/openharmony/toolchains/restool`
- `ets_loader_path`: `sdk/default/openharmony/ets/build-tools/ets-loader/` _(unused; ets-loader is ported to Rust — SPEC-004a)_
- `compile_api_version`: parse numeric version from `compatibleSdkVersion` (e.g., `"5.0.0(12)"` → `12`)

### Parsing Rules

- All config files use JSON5 format (use the `json5` crate)
- `compatibleSdkVersion` format: `"X.Y.Z(N)"` → extract `N` as `compile_api_version: u32`
- `products` × `modules` × `targets` defines a matrix; resolve to the selected product+module combination
  - CLI flag `--product <name>` selects the product (default: first product)
  - CLI flag `--module <name>` scopes to one module (default: all modules)
- `buildModeSet` defines available modes; CLI `--mode debug|release` selects (default: `debug`)

---

## Output Type

```rust
// From arkdown-core::config
pub struct ArkdownConfig {
    pub project_root: PathBuf,
    pub build_mode: BuildMode,         // Debug | Release
    pub output_target: OutputTarget,   // Hap | Hsp | Har | Preview | Test
    pub product: ProductConfig,
    pub modules: Vec<ModuleConfig>,
    pub sdk_config: SdkConfig,
}

pub struct ModuleConfig {
    pub name: String,
    pub src_path: PathBuf,             // absolute path to module root
    pub target_name: String,
    pub oh_package: OhPackageJson5,    // parsed oh-package.json5
    pub module_json: ModuleJson5,      // parsed src/main/module.json5
    pub build_option: BuildOption,     // merged from product + module overrides
}
```

---

## Files to Create

```
crates/arkdown-config/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── build_profile.rs    (parse build-profile.json5)
    ├── oh_package.rs       (parse oh-package.json5)
    ├── module_json.rs      (parse module.json5)
    ├── sdk.rs              (SDK path discovery and validation)
    └── resolve.rs          (merge product × module × target into ArkdownConfig)
```

---

## Acceptance Criteria

- Parses the `hmosworld` sample project (`https://gitee.com/harmonyos_samples/hmosworld`) without errors
- Extracts correct `compile_api_version` from `compatibleSdkVersion`
- Locates `es2abc` and `restool` binaries via SDK discovery
- `ArkdownConfig::from_project(root, mode, target, product_name, module_name)` is the public API
- Returns structured errors (not panics) for missing files or malformed JSON5

---

## Design References

- `meta/design/pipeline.md` — Phase 1 (Config Parsing)
- `meta/design/hvigorw-analysis.md` — config file schemas and field semantics
- `meta/design/types.md` — `ArkdownConfig`, `SdkConfig`, `ProductConfig`, `BuildOption` types
