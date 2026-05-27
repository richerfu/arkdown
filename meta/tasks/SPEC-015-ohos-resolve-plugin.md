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
