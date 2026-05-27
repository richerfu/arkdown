# SPEC-016: HAR Deduplication Plugin

**Crate**: `arkdown-plugin-har-dedup`
**Priority**: P1 — required for correct HSP builds
**Blocked by**: SPEC-005, SPEC-014
**Blocks**: —

---

## Summary

When an HSP depends on a HAR that is also a dependency of the consuming HAP, the HAR source
must be compiled into the HSP only — not duplicated in the HAP. This plugin implements the
two-pass architecture for HSP builds.

Replaces: `har-deduplicate-util.js` + `hsp-collect-har-files.js`.

---

## Two-Pass Architecture

### Pass 1: Collection Build

A first bundle pass with `is_collect_pass: true`:

- Walk the module graph and record every module file path that originates from a HAR
- Write two files:
  - `SOURCE_HAR_DUPLICATE_FILES_INFO` — list of source HAR file paths in this HSP
  - `BYTECODE_HAR_DUPLICATE_FILES_INFO` — list of ByteCode HAR references in this HSP

### Pass 2: Real Build (HAP)

A second bundle pass reads the collection output and excludes those HAR files from the HAP
compile. The excluded modules become external references pointing to the HSP's runtime identity.

---

## Implementation

```rust
pub struct HarDeduplicatePlugin {
    mode: HarDedupMode,
    collect_file_path: PathBuf,
    bytecode_collect_file_path: PathBuf,
}

pub enum HarDedupMode {
    CollectPass,
    RealBuild { excluded_files: HashSet<PathBuf> },
}

impl ArkdownPlugin for HarDeduplicatePlugin {
    fn name(&self) -> &str { "har-dedup" }

    async fn before_build_end(&self, ctx: &mut BuildContext) -> PluginResult<()> {
        match &self.mode {
            HarDedupMode::CollectPass => {
                // Write all HAR-origin module paths to collect files
                let har_files: Vec<_> = ctx.module_graph().modules.values()
                    .filter(|m| m.meta.is_local_dependency && is_har_module(m))
                    .map(|m| &m.id.id)
                    .collect();
                write_collect_file(&self.collect_file_path, &har_files)?;
            }
            HarDedupMode::RealBuild { excluded_files } => {
                // Mark excluded modules as external
                for module in ctx.module_graph_mut().modules.values_mut() {
                    if excluded_files.contains(Path::new(&module.id.id)) {
                        module.id.external = true;
                    }
                }
            }
        }
        Ok(())
    }
}
```

---

## When to Activate

Only for HSP (`output_target == OutputTarget::Hsp`) builds with local HAR dependencies.
The CLI (SPEC-013) orchestrates the two passes:

```rust
if config.output_target == OutputTarget::Hsp && has_local_har_deps {
    // Pass 1: collect
    let collect_plugin = HarDeduplicatePlugin::collect_pass(...);
    bundler.bundle_with_plugin(collect_plugin).await?;

    // Pass 2: build with exclusions
    let excluded = read_collect_file(&collect_path)?;
    let dedup_plugin = HarDeduplicatePlugin::real_build(excluded);
    bundler.bundle_with_plugin(dedup_plugin).await?;
}
```

---

## Acceptance Criteria

- HAP built after HSP does not include HAR source files already compiled into the HSP
- Single-HAP projects with no HSP dependency are unaffected
- ByteCode HAR deduplication handled separately from source HAR deduplication

---

## Design References

- `meta/design/plugins.md` — `har-deduplicate/` plugin analysis
- `meta/design/chunk-model.md` — HAR deduplication in HAR vs HAP vs HSP section
- `meta/design/hvigorw-analysis.md` — `processHarDeduplicate` in GenerateLoaderJson

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `HarDedupMode::CollectPass` | no rolldown equivalent | OHOS-specific two-pass build; rolldown does not have a graph collection pass |
| `HarDedupMode::RealBuild { excluded_files }` | rolldown `external` option | Functionally similar: marking modules as external skips bundling them; rolldown does this per-entry via the `external` input option; arkdown does it dynamically by mutating `module.id.external` in the hook |
| `before_build_end` hook (collection) | `Plugin::build_end` | Closest rolldown hook that fires after all modules are parsed |
| `before_build_end` hook (exclusion) | rolldown `external` option processing | In rolldown, externals are configured before the build; arkdown sets them at `before_build_end` time because the exclusion set is determined by the collect pass |
| `write_collect_file()` / `read_collect_file()` | no equivalent | Cross-pass communication via filesystem; rolldown is single-pass so no inter-pass file exchange needed |

**Key divergence:** rolldown is always single-pass. arkdown requires two passes for HSP builds to determine HAR deduplication. This two-pass architecture is OHOS-specific and has no rolldown equivalent.

---

## Test Requirements

- `HarDedupMode::CollectPass`: after `before_build_end`, the collect file contains all HAR-origin module paths from the module graph
- `HarDedupMode::CollectPass`: ByteCode HAR paths are written to `BYTECODE_HAR_DUPLICATE_FILES_INFO`, not `SOURCE_HAR_DUPLICATE_FILES_INFO`
- `HarDedupMode::RealBuild { excluded_files }`: modules in `excluded_files` have `id.external = true` after `before_build_end`
- `HarDedupMode::RealBuild`: modules NOT in `excluded_files` are unchanged (`external` stays false)
- Single-HAP project (no HSP): `HarDeduplicatePlugin` is never activated; build completes normally
- `OutputTarget::Hap` (not HSP): plugin is not activated even if local HAR deps exist
- The CLI correctly runs collect pass first, reads exclusion set, then runs real build with exclusions
- Produced HAP does not include any module files that were compiled into the HSP
- Plugin name returns `"har-dedup"`
- Collect file is created in the configured `cachePath` directory
