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
