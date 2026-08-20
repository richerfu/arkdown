# TEST-003: arkdown-resolver

**Crate**: `arkdown-resolver`
**Note**: This is the resolver library. Plugin integration tests are in TEST-015.

---

## Test File Layout

```
crates/arkdown-resolver/src/
├── tests/
│   ├── specifier_dispatch.rs ← correct branch taken per specifier prefix
│   ├── extension_order.rs    ← file extension fallback sequence
│   ├── ohm_url.rs            ← OHM URL generation and parsing
│   └── oh_modules_layout.rs  ← oh_modules/ directory traversal
```

---

## specifier_dispatch.rs

### TC-001: `@bundle:` prefix → ExternalBundle without filesystem access

- **Input**: `"@bundle:com.foo/mod/path"`, any importer
- **Expected**: `ExternalBundle { ohm_url: "@bundle:com.foo/mod/path" }`
- No filesystem call made

### TC-002: `@shared:` prefix → ExternalHsp

- **Input**: `"@shared:com.foo/hsp/path"`
- **Expected**: `ExternalHsp { ... }`

### TC-003: `@ohos/` prefix → sdk_api_paths lookup

- **Setup**: `sdk_api_paths = ["/sdk/ets/api"]`, file `/sdk/ets/api/@ohos.hilog.d.ts` exists
- **Input**: `"@ohos/hilog"`
- **Expected**: `SdkDeclaration { path: "/sdk/ets/api/@ohos.hilog.d.ts" }`

### TC-004: `@kit.` prefix → kits lookup

- **Setup**: kits path contains `@kit.ArkUI.d.ts`
- **Input**: `"@kit.ArkUI"`
- **Expected**: `SdkDeclaration { path: ".../kits/@kit.ArkUI.d.ts" }`

### TC-005: relative `./` → file resolution

- **Input**: `"./utils/Helper"` from `src/main/ets/pages/Index.ets`
- **Expected**: `LocalSource { path: "src/main/ets/utils/Helper.ets" }`

### TC-006: bare specifier → oh_modules lookup

- **Input**: `"my-pkg"` with `oh_modules/my-pkg/oh-package.json5` present
- **Expected**: `RemotePackage { ... }`

---

## extension_order.rs

### TC-007: prefers `.ets` over `.ts` over `.js`

- **Setup**: `helper.ets`, `helper.ts`, `helper.js` all exist
- **Expected**: resolves to `helper.ets`

### TC-008: falls back through extensions if earlier ones missing

- **Setup**: only `helper.ts` exists (no `.ets`)
- **Expected**: resolves to `helper.ts`

### TC-009: index file resolution: `components/` with `index.ets`

- **Input**: `"./components"` where `components/index.ets` exists
- **Expected**: resolves to `components/index.ets`

### TC-010: no extension match → NotFound error

- **Setup**: no file matching any extension
- **Expected**: `Err(ResolveError::NotFound { ... })`

---

## ohm_url.rs

### TC-011: OHM URL format: `@bundle:bundleName/moduleName/relative-path`

- **Input**: file `entry/src/main/ets/pages/Index.ets`,
  `bundle_name = "com.example"`, `module_name = "entry"`
- **Expected**: `"@bundle:com.example/entry/ets/pages/Index"` (no extension, relative to `src/main/`)

### TC-012: OHM URL for file in root of `src/main/ets`

- **Input**: `entry/src/main/ets/entryability/EntryAbility.ets`
- **Expected**: `"@bundle:com.example/entry/ets/entryability/EntryAbility"`

### TC-013: disabled when `use_normalized_ohm_url = false`

- **Expected**: `to_ohm_url()` panics or returns path-based ID, caller must not call it when disabled

---

## oh_modules_layout.rs

### TC-014: reads `main` field from `oh-package.json5`

- **Setup**: `oh_modules/pkg/oh-package.json5` with `{ "main": "src/index.ets" }`
- **Expected**: resolved path is `oh_modules/pkg/src/index.ets`

### TC-015: `types` field used for declaration files when `main` absent

- **Setup**: `oh-package.json5` with `{ "types": "index.d.ts" }`
- **Expected**: resolved path is declaration file

### TC-016: package with `metadata.abcPath` → BytecodeHar

- **Setup**: `oh-package.json5` with `{ "metadata": { "abcPath": "index.abc" } }`
- **Expected**: `BytecodeHar { abc_path: "oh_modules/pkg/index.abc" }`

### TC-017: active package wins a same-name installed dependency

- **Setup**: the package plan contains an installed `@scope/runtime`, then the
  active module is inserted with the same package name
- **Input**: `"@scope/runtime"` from a source file in the active module
- **Expected**: resolution returns the active module's canonical `main` file

### TC-018: self subpath probes package source roots

- **Setup**: active package source root is `src/main`
- **Input**: `"@scope/runtime/ets/util/Helper"` from inside that package
- **Expected**: resolves `src/main/ets/util/Helper.ets`; the same subpath import
  from an unrelated package does not use the source-root fallback

### TC-019: importer-local `oh_modules` precedes host paths

- **Setup**: a local source package has
  `oh_modules/@scope/dependency` as a symlink to an OHPM store package; the
  dependency is absent from the host package plan
- **Expected**: resolution starts from the importing package, follows the
  symlink, reads its OHOS package entry, and returns the canonical physical file
