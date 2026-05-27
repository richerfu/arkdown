# SPEC-008: Duplicate Filename Check Plugin

**Crate**: `arkdown-plugin-duplicate-check`
**Priority**: P0
**Blocked by**: SPEC-005
**Blocks**: —

---

## Summary

Validates that no two source files in a module have the same base filename. Required because
`es2abc --merge-abc` uses filenames as module record names in the merged `.abc`, so duplicate
filenames cause bytecode conflicts.

Direct Rust replacement for `check-duplicate-filenames-plugin.js`.

---

## Behavior

When the module graph is complete (`before_build_end` hook), scan all module file paths. If two
modules have the same `Path::file_stem()` (filename without extension), emit a build error.

```
ERROR: Duplicate filename detected:
  src/main/ets/utils/Logger.ets
  src/main/ets/components/Logger.ets
Both resolve to record name "Logger" in the merged .abc file.
Rename one of them to avoid bytecode conflicts.
```

---

## Implementation

```rust
pub struct DuplicateCheckPlugin;

impl ArkdownPlugin for DuplicateCheckPlugin {
    fn name(&self) -> &str { "duplicate-check" }

    async fn before_build_end(&self, ctx: &mut BuildContext) -> PluginResult<()> {
        let mut seen: HashMap<String, PathBuf> = HashMap::new();
        for module in ctx.module_graph().modules.values() {
            if module.id.external || module.id.module_type == ModuleType::Declaration {
                continue;
            }
            let path = Path::new(&module.id.id);
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(existing) = seen.insert(stem.to_string(), path.to_path_buf()) {
                    return Err(ArkdownError::DuplicateFilename {
                        stem: stem.to_string(),
                        first: existing,
                        second: path.to_path_buf(),
                    });
                }
            }
        }
        Ok(())
    }
}
```

---

## Acceptance Criteria

- Build fails with clear error if two `.ets` files have the same basename
- SDK declaration files (`.d.ts`, `.d.ets`) are excluded from the check
- External modules (ByteCode HAR, `@bundle:` refs) are excluded

---

## Design References

- `meta/design/plugins.md` — `check-duplicate-filenames-plugin` entry
- `meta/design/abc-pipeline.md` — why merge-abc requires unique record names
