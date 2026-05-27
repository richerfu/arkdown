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

---

## Rolldown Alignment

| arkdown concept                          | rolldown equivalent                                          | note                                                                                              |
| ---------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| `DuplicateCheckPlugin::before_build_end` | `Plugin::buildEnd` (rollup) / `Plugin::build_end` (rolldown) | Closest rolldown equivalent; arkdown uses `before_build_end` which fires before chunk generation  |
| File stem uniqueness check               | no rolldown equivalent                                       | OHOS-specific constraint from es2abc `--merge-abc`; standard JS bundlers have no such requirement |
| `BuildContext::module_graph()`           | rolldown `PluginContext::get_module_info()`                  | rolldown exposes per-module info via an API call; arkdown exposes the full graph via the context  |

**Key divergence:** This check has no rolldown/rollup equivalent. The constraint arises from es2abc's `--merge-abc` flag which uses the JS filename as the Panda bytecode record name. Two files with the same stem (e.g., `utils/Logger.ets` and `components/Logger.ets`) would both compile to a record named `"Logger"` causing a conflict.

---

## Test Requirements

- Two modules with paths `src/main/ets/utils/Logger.ets` and `src/main/ets/components/Logger.ets` → `Err(ArkdownError::DuplicateFilename)` with both paths in the error message
- Module graph with all unique stems → `Ok(())`
- SDK declaration files (`.d.ts`, `.d.ets`, `module_type = Declaration`) are excluded from the check; two files both named `hilog.d.ts` do not trigger an error
- External modules (`external: true`) are excluded from the check
- `BytecodeHar` modules are excluded from the check
- A single module (no duplicates possible) → `Ok(())`
- Empty module graph → `Ok(())`
- Error message includes both conflicting file paths and the stem string that collides
- Plugin name returns `"duplicate-check"`
