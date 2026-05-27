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

## Acceptance Criteria

- `OhosResolver::resolve("@ohos/hilog", importer)` returns `SdkDeclaration` pointing to SDK api dir
- `OhosResolver::resolve("./utils/helper", importer)` returns `LocalSource` with correct absolute path
- `OhosResolver::resolve("@bundle:com.foo/mod/path", importer)` returns `ExternalBundle`
- Bare specifier from `oh_modules/` resolves to the package entry file
- `file:` local HAR resolves to the HAR's source entry
- Unit tests for each specifier form

---

## Design References

- `meta/design/module-resolution.md` — complete specifier algorithm, oh_modules layout, SDK paths
- `meta/design/types.md` — `OhosResolvedModule`, `PkgContext`
- `meta/design/pipeline.md` — Phase 2 (Dependency Resolution)
