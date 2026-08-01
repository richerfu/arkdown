# TEST-015: arkdown OHOS resolve Rolldown adapter

**Crates**: `arkdown`, `arkdown-resolver`
**Owners**: `crates/arkdown/src/plugins/ohos_resolve/`, `crates/arkdown-resolver/`
**Original source**: `hvigor-arkts-compose/dist/src/plugins/resolve/index.js`,
`import-resolver.js`, `resolveImportSpecifiers.js`, `build-steps/process-importee.js`

---

## Test File Layout

```
crates/arkdown/src/plugins/ohos_resolve/
├── mod.rs
├── plugin.rs                 ← Rolldown resolve hook
└── importee.rs               ← OHOS importee adaptation

crates/arkdown-resolver/      ← typed resolver and focused integration tests
```

---

## sdk_specifiers.rs

### TC-001: `@ohos/hilog` resolves to SDK api declaration

- **Input**: specifier `"@ohos/hilog"`, any importer
- **Expected**: `OhosResolvedModule::SdkDeclaration { specifier: "@ohos/hilog", path: <sdk>/ets/api/@ohos.hilog.d.ts }`
- **Origin**: `sdkModulesPath` includes `ets/api/`; `@ohos.hilog.d.ts` looked up there.

### TC-002: `@ohos/ability-framework` resolves correctly

- **Input**: `"@ohos/ability-framework"`
- **Expected**: `SdkDeclaration` pointing to `@ohos.ability.abilityManager.d.ts` or equivalent

### TC-003: `@kit.ArkUI` resolves to SDK kits declaration

- **Input**: `"@kit.ArkUI"`
- **Expected**: `SdkDeclaration { path: <sdk>/ets/kits/@kit.ArkUI.d.ts }`
- **Origin**: `@kit.*` resolved from `ets/kits/` directory

### TC-004: `@kit.AbilityKit` resolves correctly

- **Input**: `"@kit.AbilityKit"`
- **Expected**: `SdkDeclaration { path: <sdk>/ets/kits/@kit.AbilityKit.d.ts }`

### TC-005: unknown `@ohos/` package returns error

- **Input**: `"@ohos/does-not-exist"`
- **Expected**: `Err(ResolveError::NotFound { ... })`

---

## bundle_specifiers.rs

### TC-006: `@bundle:` reference is external (not bundled)

- **Input**: `"@bundle:com.example.myapp/hsp-lib/ets/components/ComponentA"`
- **Expected**: `OhosResolvedModule::ExternalBundle { ohm_url: "@bundle:com.example.myapp/hsp-lib/ets/components/ComponentA" }`
- **Origin**: `resolve/index.js` returns external for `@bundle:` prefix

### TC-007: `@shared:` reference is external

- **Input**: `"@shared:com.example/lib/path"`
- **Expected**: `ExternalHsp { ... }` or `ExternalBundle { ohm_url: ... }`

### TC-008: Rolldown adapter returns an external resolved ID for ExternalBundle

- **Expected**: the `rolldown_plugin::Plugin::resolve_id` result has
  `external = true` so Rolldown skips loading

---

## relative.rs

### TC-009: relative `.ets` import — exact match

- **Input**: `"./utils/Helper"` from `src/main/ets/pages/Index.ets`
- **Expected**: `LocalSource { path: src/main/ets/utils/Helper.ets }`

### TC-010: extension resolution order — `.ets` tried before `.ts`

- **Input**: `"./utils/helper"` where both `helper.ets` and `helper.ts` exist
- **Expected**: resolves to `helper.ets` (`.ets` tried first)
- **Origin**: extensions tried in order: none → `.ets` → `.ts` → `.js` → `/index.ets` → ...

### TC-011: extension resolution order — falls back to `.ts`

- **Input**: `"./utils/helper"` where only `helper.ts` exists (no `.ets`)
- **Expected**: resolves to `helper.ts`

### TC-012: index resolution — `./components` with `components/index.ets`

- **Input**: `"./components"` where `components/` directory exists with `index.ets`
- **Expected**: resolves to `components/index.ets`

### TC-013: missing file returns error

- **Input**: `"./does-not-exist"` from any importer
- **Expected**: `Err(ResolveError::NotFound { ... })`

---

## ohpm_packages.rs

### TC-014: bare specifier from `oh_modules/`

- **Input**: `"my-ohpm-package"`, with `oh_modules/my-ohpm-package/oh-package.json5` containing
  `{ "main": "src/main/ets/index.ets" }`
- **Expected**: `RemotePackage { path: oh_modules/my-ohpm-package/src/main/ets/index.ets, pkg_name: "my-ohpm-package" }`

### TC-015: scoped package `@scope/name` from `oh_modules/`

- **Input**: `"@ohos/lib"` (not an SDK package — a user ohpm package), with
  `oh_modules/@ohos/lib/oh-package.json5` present
- **Expected**: `RemotePackage { path: oh_modules/@ohos/lib/<main>, ... }`

### TC-016: module oh_modules takes priority over project oh_modules

- **Input**: `"pkg"` with both `entry/oh_modules/pkg/` and root `oh_modules/pkg/`
- **Expected**: resolves from module-level `entry/oh_modules/pkg/` (closer scope wins)

---

## local_har.rs

### TC-017: `file:` HAR resolves to HAR entry file

- **Input**: package `"my-har"` with `oh-package.json5` dependency `{ "my-har": "file:../my-har" }`,
  HAR has `oh-package.json5` with `{ "main": "src/main/ets/index.ets" }`
- **Expected**: `LocalSource { path: /abs/path/to/my-har/src/main/ets/index.ets, pkg_context: Some(...) }`

### TC-018: `file:` HAR package context contains correct pkgName

- **Input**: same as TC-017, HAR `oh-package.json5` has `{ "name": "my-har" }`
- **Expected**: `pkg_context.package_name == "my-har"`

---

## bytecode_har.rs

### TC-019: package with `metadata.abcPath` → BytecodeHar

- **Input**: `"my-bytecode-har"` with `oh-package.json5`:
  ```json5
  { metadata: { abcPath: "index.abc", declarationEntry: ["index.d.ts"] } }
  ```
- **Expected**: `BytecodeHar { abc_path: /path/to/my-bytecode-har/index.abc, declaration_path: ..., pkg_name: "my-bytecode-har" }`

### TC-020: plugin marks BytecodeHar as external

- **Expected**: `ResolvedId { external: true, module_type: ModuleType::BytecodeHar }`
  so bundler skips loading

---

## ohm_url.rs

### TC-021: OHM URL generated for local source file

- **Input**: file at `entry/src/main/ets/utils/Helper.ets`, bundle `com.example`, module `entry`
- **Expected**: `"@bundle:com.example/entry/ets/utils/Helper"` (no extension, relative to `src/main/`)

### TC-022: OHM URL generated for ohpm package entry

- **Input**: `oh_modules/my-lib/src/main/ets/index.ets`, pkg_name `my-lib`,
  bundle `com.example`, module `entry`
- **Expected**: `"@bundle:com.example/entry/oh_modules/my-lib/src/main/ets/index"`
  or the normalized OHM form as used by hvigor

### TC-023: OHM URL only generated when `use_normalized_ohm_url = true`

- **Input**: same as TC-021, but `use_normalized_ohm_url = false`
- **Expected**: `ResolvedId::id` is the absolute file path, not an OHM URL

---

## case_check.rs

### TC-024: exact case match — no error

- **Input**: `"./utils/Helper"` where filesystem has `Helper.ets` (matching case)
- **Expected**: resolves successfully

### TC-025: case mismatch — error when `case_sensitive_check = true`

- **Input**: `"./utils/helper"` where filesystem has `Helper.ets` (capital H) on macOS
- **Expected**: `Err(ArkdownError::CaseMismatch { expected: "helper.ets", actual: "Helper.ets" })`

### TC-026: case check disabled when `case_sensitive_check = false`

- **Input**: `"./utils/helper"` where filesystem has `Helper.ets`
- **Expected**: resolves without error

### TC-027: case check is no-op on Linux (always case-sensitive)

- **Condition**: `#[cfg(target_os = "linux")]`
- **Expected**: case check code path not executed
