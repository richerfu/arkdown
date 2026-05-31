# SPEC-015: Full OHOS Resolve Plugin

**Crate**: `arkdown-plugin-ohos-resolve`
**Priority**: P1
**Blocked by**: SPEC-003 (resolver library), SPEC-014 (module meta, for pkgNameToPkgBriefInfo)
**Blocks**: —

---

## Summary

Wraps the `OhosResolver` library (SPEC-003) as an `ArkdownPlugin`. This plugin handles the
`resolve_id` hook for all OHOS import specifiers. Also implements the `generate_bundle` hook
for the post-build import path rewriting (`process-importee` equivalent).

Replaces: `resolve/index.js`, `import-resolver.js`, `resolveImportSpecifiers.js`,
`build-steps/process-importee.js`, and `@rollup/plugin-node-resolve`.

---

## resolve_id Hook

```rust
async fn resolve_id(&self, ctx: &ResolveContext, specifier: &str, importer: Option<&str>)
    -> PluginResult<Option<ResolvedId>>
{
    let importer_path = importer.map(Path::new);
    let resolved = self.resolver.resolve(specifier, importer_path.unwrap_or(&self.project_root))?;

    match resolved {
        OhosResolvedModule::ExternalBundle { ohm_url } |
        OhosResolvedModule::ExternalHsp { .. } => {
            Ok(Some(ResolvedId { id: ohm_url, external: true, .. }))
        }
        OhosResolvedModule::SdkDeclaration { .. } => {
            // SDK declarations are type-only — mark as external to skip bundling
            Ok(Some(ResolvedId { id: ..., external: true, module_type: ModuleType::Declaration, .. }))
        }
        OhosResolvedModule::BytecodeHar { ohm_url, .. } => {
            Ok(Some(ResolvedId { id: ohm_url, external: true, module_type: ModuleType::BytecodeHar, .. }))
        }
        OhosResolvedModule::LocalSource { path, module_type, .. } => {
            Ok(Some(ResolvedId { id: path.to_string_lossy().into(), external: false, module_type }))
        }
        _ => Ok(Some(...))
    }
}
```

## generate_bundle Hook (process-importee equivalent)

After bundling completes, rewrite import paths in the bundle output for:

1. **ByteCode HAR entries**: Replace the source reference with the pre-compiled ABC path reference
2. **Bundled dependencies**: When `is_bundled_dependencies` is true, include HAR source in bundle output

```rust
async fn generate_bundle(&self, ctx: &mut GenerateBundleContext, bundle: &mut ChunkGraph,
    is_write: bool) -> PluginResult<()>
{
    for chunk in bundle.chunks.iter_mut() {
        self.rewrite_bytecode_har_imports(chunk, &ctx.byte_code_har_info)?;
    }
    Ok(())
}
```

## Post-Bundle Importee Processing (`importee.rs`)

The `process_importees` function is a standalone post-bundle pass (not a plugin hook) that
ports hvigor's `build-steps/process-importee.js`. It is called from `arkdown-cli` after
bundling completes.

### `ProcessImporteeConfig`

```rust
pub struct ProcessImporteeConfig {
    pub bundle_name: String,       // e.g. "com.example.app"
    pub module_name: String,       // e.g. "entry"
    pub module_version: String,    // version string (may be empty)
    pub project_root: PathBuf,
    pub module_src_main: PathBuf,  // absolute path to {module}/src/main
    pub output_dir: PathBuf,       // absolute path to {module}/build/default/intermediates
}
```

### `ProcessImporteeResult`

```rust
pub struct ProcessImporteeResult {
    pub non_entry_importees: Vec<String>,  // OHM URL format
    pub compiled_importees: Vec<String>,   // relative paths from src/main
}
```

### Algorithm

1. Build a `name → &ArkChunk` map from the `ChunkGraph`.
2. Identify entry chunks (`is_entry || is_dynamic_entry`).
3. BFS from entry chunks through `imports`/`dynamic_imports` to collect all reachable chunks.
4. **Non-entry importees**: reachable chunks that are NOT entry chunks, each mapped to an OHM URL
   `@bundle:{bundle_name}/{module_name}/{path_without_ext}` via their `facade_module_id`.
5. **Compiled importees**: all `facade_module_id` values that are local source files (not
   external specifiers), each converted to a path relative to `module_src_main`.
6. Write the cache file: `{output_dir}/importee/default/{module_name}/importee.json`.

### External specifier filtering

Modules with IDs starting with `@ohos:`, `@kit.`, `@shared:`, `@bundle:`,
`@system.`, or `@hms.` are excluded from both importee lists. Scoped
`@ohos/*` package imports are not automatically external: they first resolve
through `oh_modules` / `pkgNameToPkgBriefInfo`, and only fall back to SDK
declarations when no package is present.

### loader.json integration

Two new fields are added to `LoaderJson` (in `arkdown-profile`):
- `nonEntryImportees: Vec<String>` — OHM URL list, serialized only when non-empty
- `compiledImportees: Vec<String>` — source path list, serialized only when non-empty

These are populated by the CLI build command after calling `process_importees` and the
loader.json is re-written with the updated values.

## Case-Sensitive Check

When `config.case_sensitive_check` is true, validate that the resolved path matches the
filesystem case exactly (using `std::fs::read_dir` to check the actual filename):

```rust
fn check_case_sensitive(specifier: &str, resolved: &Path) -> Result<(), ArkdownError> {
    let actual_filename = resolved.parent()
        .and_then(|p| std::fs::read_dir(p).ok())
        .and_then(|mut d| d.find(|e| {
            e.as_ref().ok()?.path() == resolved
        }));
    // compare actual_filename with resolved.file_name()
}
```

---

## Files to Create

```
crates/arkdown-plugin-ohos-resolve/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── plugin.rs            (OhosResolvePlugin: ArkdownPlugin impl)
    ├── importee.rs          (process-importee: post-bundle rewriting)
    └── case_check.rs        (case-sensitivity validation)
```

---

## Acceptance Criteria

- All specifier forms from SPEC-003 resolve correctly through the plugin interface
- ByteCode HAR modules appear as `external: true` in `ModuleGraph`
- SDK declarations skipped — not included in bundle output
- Case-sensitivity check warns on mismatched case on macOS/Windows

---

## Design References

- `meta/design/module-resolution.md` — full resolver algorithm
- `meta/design/plugins.md` — `resolve/index` + `import-resolver` + `process-importee` analysis

---

## Rolldown Alignment

| arkdown concept                                               | rolldown equivalent                                  | note                                                                                                                        |
| ------------------------------------------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `OhosResolvePlugin::resolve_id`                               | `Plugin::resolve_id` returning `HookResolveIdOutput` | Identical hook; arkdown maps `OhosResolvedModule` → `ResolvedId` here                                                       |
| `OhosResolvePlugin::generate_bundle`                          | `Plugin::generate_bundle`                            | Same hook; used for post-bundle import path rewriting (process-importee equivalent)                                         |
| `OhosResolvePlugin::case_check`                               | no direct rolldown equivalent                        | The case-sensitivity check is integrated into the resolve hook; rolldown does not have this                                 |
| `ExternalBundle` → `ResolvedId { external: true }`            | `HookResolveIdOutput { external: true }`             | Identical semantics                                                                                                         |
| `SdkDeclaration` → `external: true, module_type: Declaration` | rolldown has no `Declaration` type                   | arkdown adds `ModuleType::Declaration` to signal "skip loading"; rolldown would use `external: true` alone                  |
| `BytecodeHar` → `external: true, module_type: BytecodeHar`    | no rolldown equivalent                               | OHOS-specific: ABC path carried in the ID string for use by packager                                                        |
| `generate_bundle` / `rewrite_bytecode_har_imports`            | rolldown chunk rendering                             | In rolldown, cross-chunk imports are rewritten during code generation; arkdown does this post-bundle for ByteCode HAR cases |
| `@rollup/plugin-node-resolve` replacement                     | rolldown built-in resolver                           | rolldown has a built-in resolver; arkdown replaces it with this OHOS-aware plugin                                           |

---

## Public API

```rust
pub struct OhosResolvePlugin {
    resolver: Arc<OhosResolver>,
    project_root: PathBuf,
}

impl OhosResolvePlugin {
    pub fn new(config: &ArkdownConfig, meta: &PkgNameToPkgBriefInfo) -> Self;
}

impl ArkdownPlugin for OhosResolvePlugin {
    fn name(&self) -> &str;
    async fn resolve_id(&self, ctx: &ResolveContext, specifier: &str, importer: Option<&str>)
        -> PluginResult<Option<ResolvedId>>;
    async fn generate_bundle(&self, ctx: &mut GenerateBundleContext, bundle: &mut ChunkGraph,
        is_write: bool) -> PluginResult<()>;
    // load, transform: no-op defaults
}
```

---

## Test Requirements

- `resolve_id("@ohos/hilog", importer)` → `ResolvedId { external: true, module_type: Declaration }`
- `resolve_id("@bundle:com.foo/mod/path", importer)` → `ResolvedId { id: "@bundle:com.foo/mod/path", external: true }`
- `resolve_id("./utils/helper", importer)` → `ResolvedId { id: abs_path, external: false, module_type: Ets }`
- `resolve_id` for a ByteCode HAR package → `ResolvedId { external: true, module_type: BytecodeHar }`
- `generate_bundle`: ByteCode HAR import in a chunk has its source path rewritten to the ABC reference
- Case-sensitivity check enabled: `resolve_id("./Utils/Helper")` when actual path is `./utils/helper` → `Err(ArkdownError::CaseMismatch)` on macOS/Windows
- Case-sensitivity check disabled: `resolve_id("./Utils/Helper")` passes through to the resolver without case validation
- Case-sensitivity check is a no-op on Linux (skip the `read_dir` call on case-sensitive filesystems)
- `resolve_id` returns `Ok(None)` for absolute filesystem IDs so rolldown can keep its entry handling; unresolved OHOS/bare specifiers surface as errors instead of being silently externalized
- Plugin name returns `"ohos-resolve"`
