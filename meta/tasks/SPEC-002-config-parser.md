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
| `AppScope/app.json5`  | project root              | **bundle name** (`app.bundleName`), app-level icon/label/version                                           |
| `build-profile.json5` | project root              | products, modules, **signing configs** (at `app.signingConfigs`), build options                            |
| `oh-package.json5`    | each module root          | package name, version, dependencies                                                                        |
| `module.json5`        | each module's `src/main/` | abilities, srcEntry, deviceTypes, pages                                                                    |
| `hvigorfile.ts`       | project root              | plugin registration (arkdown reads this only to validate it's a standard app project; does not execute it) |

> **Important**: `bundleName` is read from `AppScope/app.json5` (`app.bundleName`), NOT from
> `build-profile.json5` products. The `new load_app_json5()` function handles this file.

---

## Key Requirements

### AppScope/app.json5 Schema (bundle name source)

```json5
{
  app: {
    bundleName: "com.example.myapp", // ← definitive bundle name
    vendor: "example",
    versionCode: 1000000,
    versionName: "1.0.0",
    icon: "$media:app_icon",
    label: "$string:app_name",
  },
}
```

### build-profile.json5 Schema

`signingConfigs` is at `app` level (not inside products). Products reference a signing config by
name via `signingConfig: "default"`. `targetSdkVersion` may appear alongside `compatibleSdkVersion`.

```json5
{
  app: {
    signingConfigs: [{         // ← signing configs live here, NOT in products
      name: "default",
      type: "HarmonyOS",
      material: { certpath, keyAlias, keyPassword, profile, signAlg, storeFile, storePassword }
    }],
    products: [{
      name: "default",
      signingConfig: "default",           // ← reference by name
      compatibleSdkVersion: "5.0.0(12)",  // → compile_api_version = 12
      targetSdkVersion: "5.0.0(12)",      // optional; same format as compatibleSdkVersion
      runtimeOS: "HarmonyOS",
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
- `bundleName` is read from `AppScope/app.json5` at the project root — **not** from `products`
- `signingConfigs` lives at `build-profile.json5#app.signingConfigs`; products reference them by name
- `compatibleSdkVersion` format: `"X.Y.Z(N)"` → extract `N` as `compile_api_version: u32`
- `targetSdkVersion` uses the same format; parse it the same way as `compatibleSdkVersion`
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

## Public API Additions

```rust
/// Parse AppScope/app.json5 (source of bundleName).
pub fn load_app_json5(project_root: &Path) -> Result<AppJson5, ArkdownError>;

pub struct AppJson5 {
    pub bundle_name: String,
    pub vendor: Option<String>,
    pub version_code: u32,
    pub version_name: String,
}

pub struct ProductConfig {
    pub name: String,
    pub signing_config: Option<String>,     // name reference into signingConfigs
    pub compatible_sdk_version: String,
    pub target_sdk_version: Option<String>, // same format as compatible_sdk_version
    pub runtime_os: String,
    pub build_option: BuildOption,
}

pub struct SigningConfig {
    pub name: String,
    pub material: SigningMaterial,
}
```

## Files to Create

```
crates/arkdown-config/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── app_json5.rs        (parse AppScope/app.json5 — bundle name source)
    ├── build_profile.rs    (parse build-profile.json5)
    ├── oh_package.rs       (parse oh-package.json5)
    ├── module_json.rs      (parse module.json5)
    ├── sdk.rs              (SDK path discovery and validation)
    └── resolve.rs          (merge product × module × target into ArkdownConfig)
```

---

## Acceptance Criteria

- Parses the `hmosworld` sample project (`https://github.com/susns/HmosWorld`) without errors
- Extracts correct `compile_api_version` from `compatibleSdkVersion`
- Locates `es2abc` and `restool` binaries via SDK discovery
- `ArkdownConfig::from_project(root, mode, target, product_name, module_name)` is the public API
- Returns structured errors (not panics) for missing files or malformed JSON5

---

## Design References

- `meta/design/pipeline.md` — Phase 1 (Config Parsing)
- `meta/design/hvigorw-analysis.md` — config file schemas and field semantics
- `meta/design/types.md` — `ArkdownConfig`, `SdkConfig`, `ProductConfig`, `BuildOption` types

> **Key discovery**: `AppScope/app.json5` is the canonical source of `bundleName` in real DevEco
> Studio projects. The `products` array in `build-profile.json5` does not contain `bundleName`.
> `signingConfigs` is at `build-profile.json5#app.signingConfigs`, referenced from products by name.

---

## Rolldown Alignment

| arkdown concept                                       | rolldown equivalent                                          | note                                                                                                      |
| ----------------------------------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| `ArkdownConfig`                                       | rolldown `InputOptions` + `OutputOptions`                    | arkdown merges both into a single config struct; rolldown separates build-time from emit-time options     |
| `BuildMode::Debug/Release`                            | rolldown `treeshake` option + `minify` option                | arkdown uses a single mode enum; rolldown sets these independently                                        |
| `AppJson5::bundle_name`                               | no rolldown equivalent                                       | OHOS-specific: read from `AppScope/app.json5`, not from `products`; identifies the app bundle in OHM URLs |
| `StrictModeConfig::use_normalized_ohm_url`            | no rolldown equivalent                                       | OHOS-specific: controls whether module IDs are OHM URLs or file paths                                     |
| `ModuleConfig::oh_package`                            | rolldown reads `package.json`; no `oh-package.json5` concept | OHOS uses ohpm, not npm; `oh-package.json5` replaces `package.json`                                       |
| `SdkConfig`                                           | rolldown has no SDK concept                                  | Pure arkdown addition: SDK version determines ArkTS API rewrites and signing block version                |
| `ArkdownConfig::from_project()`                       | rolldown `Bundler::new(BundlerOptions)`                      | rolldown takes options as a struct; arkdown parses from the filesystem                                    |
| `compatibleSdkVersion` parsing (`"5.0.0(12)"` → `12`) | no equivalent                                                | OHOS-specific numeric API version extraction                                                              |

**Key divergence:** rolldown accepts explicit entry points from options. arkdown derives entry points from `module.json5` abilities and the profile processor (SPEC-010) — there is no direct `entry: [...]` config field.

---

## Test Requirements

- Parse a minimal valid `build-profile.json5` with one product and one module; assert `bundle_name` and `compatible_sdk_version` are correct
- Parse a full `build-profile.json5` with multiple products and modules; assert the correct product is selected by `--product` name
- `compatibleSdkVersion` extraction: `"5.0.0(12)"` → `compile_api_version = 12`; also test `"4.1.7(11)"` → `11`
- Missing required field `bundleName` in products returns `Err` with a descriptive message (not a panic)
- Malformed JSON5 (unterminated string) returns `Err(ArkdownError::ParseError { .. })`
- SDK path discovery: mock `OHOS_SDK_HOME` env var and assert it takes precedence over default paths
- SDK path discovery: when `OHOS_SDK_HOME` is unset, falls back to `~/Library/Huawei/Sdk/` on macOS (or equivalent platform default)
- `BuildOption::strict_mode.case_sensitive_check` defaults to `false` when `strictMode` block is absent from `build-profile.json5`
- `BuildOption::strict_mode.use_normalized_ohm_url` defaults to `false` when absent
- `oh-package.json5` with `file:../my-har` local dependency parses as a `file:` reference (not treated as an ohpm package name)
- `oh-package.json5` with `dynamicDependencies` parses without error; dynamic deps do not appear in `dependencies`
- `ArkdownConfig::from_project()` on the `hmosworld` sample project returns `Ok` with correct module count
