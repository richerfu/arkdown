# SPEC-003: OHOS Module Resolver

**Crate**: `arkdown-resolver`
**Priority**: P0
**Blocked by**: SPEC-001
**Blocks**: SPEC-004, SPEC-005, SPEC-015

---

## Summary

Implement the OHOS-aware module resolver that maps import specifiers to absolute file paths or
external references. Replaces `@ohos/hvigor-arkts-compose`'s `plugins/resolve/` chain and
`@rollup/plugin-node-resolve`. Built on top of `oxc_resolver` from `ohos-rs/oxc`.

This crate is a library — it exposes a `OhosResolver` struct used by the loader and the
resolve plugin. It does NOT implement the `ArkdownPlugin` trait directly (that is SPEC-015).

---

## All Specifier Forms to Handle

See `meta/design/module-resolution.md` for full details. Summary:

| Specifier    | Example                         | Resolution                                         |
| ------------ | ------------------------------- | -------------------------------------------------- |
| Relative     | `./utils/helper`                | Standard file lookup, try `.ets`, `.ts`, `.js`     |
| `@ohos/*`    | `@ohos/hilog`                   | SDK api dir: `sdk/.../ets/api/@ohos.hilog.d.ts`    |
| `@kit.*`     | `@kit.ArkUI`                    | SDK kits dir: `sdk/.../ets/kits/@kit.ArkUI.d.ts`   |
| `@bundle:`   | `@bundle:com.foo/mod/path`      | External — not bundled (runtime-resolved)          |
| `@shared:`   | `@shared:com.foo/hsp/path`      | External — not bundled                             |
| Bare ohpm    | `my-ohpm-package`               | `oh_modules/<pkg>/oh-package.json5` → `main`       |
| `file:` HAR  | `file:../my-har`                | Local directory → resolve `src/main/ets/index.ets` |
| ByteCode HAR | package with `metadata.abcPath` | External ABC ref, skip source compilation          |

---

## Key Implementation

### `OhosResolver` struct

```rust
pub struct OhosResolver {
    config: ResolverConfig,
    oxc_resolver: OxcResolver,  // from ohos-rs/oxc (oxc_resolver crate)
}

pub struct ResolverConfig {
    pub project_root: PathBuf,
    pub sdk_api_paths: Vec<PathBuf>,      // SDK api/ and kits/ directories
    pub pkg_name_to_pkg_info: HashMap<String, PkgInfo>,  // from loader.json pkgBriefInfo
    pub use_normalized_ohm_url: bool,
    pub case_sensitive_check: bool,       // for SPEC-018
    pub oh_modules_roots: Vec<PathBuf>,   // per-module oh_modules/ + project-root oh_modules/
}

impl OhosResolver {
    pub fn new(config: ResolverConfig) -> Self;

    /// Resolve a specifier from an importer file path.
    pub fn resolve(&self, specifier: &str, importer: &Path) -> Result<OhosResolvedModule, ResolveError>;
}
```

### Resolution Algorithm

```
fn resolve(specifier, importer):
    1. if specifier starts with "@bundle:" or "@shared:" → ExternalBundle/ExternalHsp
    2. if specifier starts with "@ohos/" → SdkDeclaration (sdk_api_paths lookup)
    3. if specifier starts with "@kit." → SdkDeclaration (sdk kits lookup)
    4. if specifier starts with "./" or "../" → relative resolution via oxc_resolver
    5. bare specifier:
        a. look up in pkg_name_to_pkg_info (local HAR)
        b. look up in oh_modules/ (installed ohpm packages)
        c. if oh-package.json5 has metadata.abcPath → BytecodeHar
    6. fallback: return ResolveError::NotFound
```

### OHM URL Generation

When `use_normalized_ohm_url` is true, generate OHM URLs for resolved modules:

```
@bundle:<bundleName>/<moduleName>/<relativePath>
```

Where `relativePath` is the path relative to `src/main/` without extension.

```rust
pub fn to_ohm_url(resolved: &OhosResolvedModule, pkg_ctx: &PkgContext) -> String {
    format!("@bundle:{}/{}/{}", pkg_ctx.bundle_name, pkg_ctx.module_name, relative_path)
}
```

### Extension Resolution Order

For `.ets`/`.ts`/`.js` files, try extensions in this order (matches hvigor behavior):

1. No extension (exact match)
2. `.ets`
3. `.ts`
4. `.js`
5. `<name>/index.ets`
6. `<name>/index.ts`
7. `<name>/index.js`

---

## oh_modules/ Layout

ohpm installs packages to `oh_modules/` (not `node_modules/`). Each package has:

```
oh_modules/
└── @ohos/
    └── lib/
        ├── oh-package.json5   (main: "src/main/ets/index.ets")
        └── src/main/ets/index.ets
```

The resolver reads `oh-package.json5`'s `main` field to find the entry file.

---

## Files to Create

```
crates/arkdown-resolver/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── resolver.rs         (OhosResolver struct + resolve())
    ├── sdk.rs              (SDK api/kits path lookup)
    ├── ohm_url.rs          (OHM URL generation and parsing)
    ├── oh_modules.rs       (oh_modules/ directory traversal)
    └── error.rs            (ResolveError)
```

---

## BFS Traversal: External Module Skipping

During module loading (`load_all()` in `arkdown-loader`), the BFS walk must **skip** importing
further dependencies from external modules. The following specifier forms are treated as leaves
in the BFS graph — they are resolved but their own imports are not followed:

| Specifier prefix        | Reason to skip                                               |
| ----------------------- | ------------------------------------------------------------ |
| `@bundle:` / `@shared:` | Runtime-resolved cross-bundle references — not bundled here  |
| `@kit.*`                | SDK kit aggregation modules — declarations only, no source   |
| `@ohos/*`               | SDK API declarations — declarations only, no source          |
| `@system/*`             | Legacy SDK system APIs — declarations only                   |

Relative imports (`./`, `../`) and bare ohpm package specifiers that resolve to `LocalSource`
are always followed in BFS.

## Import Path Resolution

All relative import paths in `parse.rs` are resolved to **absolute paths** based on the importer
file's directory, not the module root. This ensures that:

- `import './utils/helper'` from `/project/entry/src/main/ets/pages/Index.ets` resolves to
  `/project/entry/src/main/ets/pages/utils/helper.ets` (not the module root)
- Paths passed to the resolver are always absolute — specifiers with `./` or `../` are
  joined with `importer.parent()` before being passed to `OxcResolver`

## Acceptance Criteria

- `OhosResolver::resolve("@ohos/hilog", importer)` returns `SdkDeclaration` pointing to SDK api dir
- `OhosResolver::resolve("./utils/helper", importer)` returns `LocalSource` with correct **absolute** path
- `OhosResolver::resolve("@bundle:com.foo/mod/path", importer)` returns `ExternalBundle`
- `@kit.*`, `@ohos/*`, `@bundle:`, `@shared:` modules are not BFS-expanded (their imports are not followed)
- Bare specifier from `oh_modules/` resolves to the package entry file
- `file:` local HAR resolves to the HAR's source entry
- Unit tests for each specifier form

---

## Design References

- `meta/design/module-resolution.md` — complete specifier algorithm, oh_modules layout, SDK paths
- `meta/design/types.md` — `OhosResolvedModule`, `PkgContext`
- `meta/design/pipeline.md` — Phase 2 (Dependency Resolution)

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `OhosResolver::resolve()` | `resolve_id` hook result (`HookResolveIdOutput`) | In rolldown a plugin returns `HookResolveIdOutput { id, external, side_effects }`; arkdown uses a richer `OhosResolvedModule` enum that the plugin layer (SPEC-015) then maps to `ResolvedId` |
| `OhosResolvedModule::ExternalBundle` | `HookResolveIdOutput { external: true }` | Same semantics: the bundler will not attempt to load the module |
| `OhosResolvedModule::LocalSource` | `HookResolveIdOutput { external: false, id: abs_path }` | Identical; rolldown uses `id` as absolute path |
| `OhosResolvedModule::SdkDeclaration` | `HookResolveIdOutput { external: true }` with `ModuleType::Declaration` | rolldown has no concept of declaration-only modules; arkdown skips loading them entirely |
| `OhosResolvedModule::BytecodeHar` | no rolldown equivalent | OHOS-specific: resolved as external (`external: true`) but carries `abc_path` for the packager |
| `ResolverConfig::pkg_name_to_pkg_info` | rolldown reads `node_modules/` + `package.json` | OHOS uses `oh_modules/` + `oh-package.json5`; the map replaces Node.js package resolution |
| `ResolverConfig::oh_modules_roots` | rolldown `resolve.modules` option (`["node_modules"]`) | Same purpose: list of directories to search for bare specifiers |
| Extension resolution order (`.ets`, `.ts`, `.js`) | rolldown `resolve.extensions` option | OHOS tries `.ets` first (arkdown-specific); rolldown default is `[".js", ".ts"]` |
| `to_ohm_url()` | no rolldown equivalent | OHOS-specific: converts resolved paths to `@bundle:bundleName/moduleName/path` strings |

---

## Test Requirements

- `resolve("@ohos/hilog", importer)` → `SdkDeclaration` pointing to SDK api dir
- `resolve("@kit.ArkUI", importer)` → `SdkDeclaration` pointing to SDK kits dir
- `resolve("@bundle:com.foo/mod/path", importer)` → `ExternalBundle { ohm_url: "@bundle:com.foo/mod/path" }`
- `resolve("@shared:com.foo/hsp/ets/index", importer)` → `ExternalHsp { .. }`
- `resolve("./utils/helper", importer)` → `LocalSource` with `.ets` extension tried first, then `.ts`, then `.js`
- `resolve("my-ohpm-pkg", importer)` → reads `oh_modules/my-ohpm-pkg/oh-package.json5` `main` field and returns `LocalSource` at that path
- `resolve("my-bytecode-har", importer)` where `oh-package.json5` has `metadata.abcPath` → `BytecodeHar`
- `resolve("file:../my-local-har", importer)` → `LocalSource` at the local HAR's `src/main/ets/index.ets`
- `resolve("nonexistent-pkg", importer)` → `Err(ResolveError::NotFound { .. })`
- OHM URL generation with `use_normalized_ohm_url = true`: `to_ohm_url()` on a local source file produces `"@bundle:com.example/entry/ets/utils/helper"` (no `.ets` extension, path relative to `src/main/`)
- Case-sensitivity check: resolving `"./Utils/Helper"` when actual file is `"./utils/helper"` returns `Err(ResolveError::CaseMismatch)` when `case_sensitive_check = true`
- Relative specifier `"../../../outside"` that escapes module root returns `Err` (no directory traversal outside project root)
