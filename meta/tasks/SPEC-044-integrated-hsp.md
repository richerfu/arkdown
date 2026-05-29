# SPEC-044: Integrated HSP Processing

**Crate**: `arkdown-pack`
**Priority**: P2 — projects embedding remote/integrated HSP dependencies
**Blocked by**: SPEC-012 (packager), SPEC-031 (ByteCode HAR deps)
**Blocks**: —

---

## Summary

When a HAP has integrated (embedded) remote HSP dependencies, copy their bytecode and
resources into the HAP archive at the correct paths. This mirrors hvigor's
`ProcessIntegratedHsp` task (`process-integrated-hsp.js`).

An "integrated HSP" is a remote HSP that the HAP bundles directly rather than loading
at runtime from the system HSP store. This is used for HSP packages that are not
published to the system store but are distributed as part of the app.

---

## Background: What hvigor Does

From `process-integrated-hsp.js`:

```javascript
class ProcessIntegratedHsp extends AbstractTask {
    async doTaskAction() {
        const integratedHspDeps = this.dependencyManager
            .getIntegratedHspDependencies();

        for (const dep of integratedHspDeps) {
            // Copy HSP .abc to HAP's ets/ directory
            const abcSrc = dep.getAbcPath();
            const abcDest = path.join(this.hapOutputDir, "ets", dep.getModuleName() + ".abc");
            fse.copySync(abcSrc, abcDest);

            // Copy HSP resources.index to HAP's resources/ directory
            const resourcesSrc = dep.getResourcesIndexPath();
            const resourcesDest = path.join(this.hapOutputDir, "resources",
                dep.getModuleName(), "resources.index");
            if (fse.existsSync(resourcesSrc)) {
                fse.copySync(resourcesSrc, resourcesDest);
            }

            // Copy HSP module.json to HAP's module_info/ directory
            const moduleJsonSrc = dep.getModuleJsonPath();
            const moduleJsonDest = path.join(this.hapOutputDir, "module_info",
                dep.getModuleName(), "module.json");
            fse.copySync(moduleJsonSrc, moduleJsonDest);
        }
    }
}
```

An integrated HSP dependency is identified in `oh-package.json5` by the `integratedHsp`
metadata flag or by the dependency type being `integratedHsp`.

The resulting HAP zip contains:
```
ets/<hspModuleName>.abc          ← HSP bytecode
resources/<hspModuleName>/resources.index  ← HSP resources
module_info/<hspModuleName>/module.json    ← HSP module manifest
```

---

## Scope

### 1. `IntegratedHspDep` Detection

Extend `OhPackageJson5` metadata to recognize integrated HSP deps:

```rust
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OhPackageMetadata {
    // ... existing fields (abc_path, source_maps_path) ...
    /// True when this HSP is embedded in the HAP (integrated HSP).
    pub integrated_hsp: Option<bool>,
}

impl OhPackageJson5 {
    pub fn is_integrated_hsp(&self) -> bool {
        self.metadata
            .as_ref()
            .and_then(|m| m.integrated_hsp)
            .unwrap_or(false)
    }
}
```

### 2. `IntegratedHspDep` Struct

```rust
pub struct IntegratedHspDep {
    /// Module name of the HSP (e.g., "remoteHsp").
    pub module_name: String,
    /// Absolute path to the HSP's compiled .abc file.
    pub abc_path: PathBuf,
    /// Absolute path to the HSP's resources.index (optional).
    pub resources_index_path: Option<PathBuf>,
    /// Absolute path to the HSP's module.json.
    pub module_json_path: PathBuf,
}
```

### 3. `process_integrated_hsp()` Function

```rust
/// Copy integrated HSP artifacts into the HAP output directory.
pub fn process_integrated_hsp(
    hap_output_dir: &Path,
    integrated_deps: &[IntegratedHspDep],
) -> Result<(), PackError> {
    for dep in integrated_deps {
        // Copy .abc
        let abc_dest = hap_output_dir
            .join("ets")
            .join(format!("{}.abc", dep.module_name));
        std::fs::create_dir_all(abc_dest.parent().unwrap())?;
        std::fs::copy(&dep.abc_path, &abc_dest)?;

        // Copy resources.index (optional)
        if let Some(resources_src) = &dep.resources_index_path {
            if resources_src.exists() {
                let resources_dest = hap_output_dir
                    .join("resources")
                    .join(&dep.module_name)
                    .join("resources.index");
                std::fs::create_dir_all(resources_dest.parent().unwrap())?;
                std::fs::copy(resources_src, &resources_dest)?;
            }
        }

        // Copy module.json
        let module_json_dest = hap_output_dir
            .join("module_info")
            .join(&dep.module_name)
            .join("module.json");
        std::fs::create_dir_all(module_json_dest.parent().unwrap())?;
        std::fs::copy(&dep.module_json_path, &module_json_dest)?;
    }
    Ok(())
}
```

### 4. Integration with `pack_hap()`

In `arkdown-pack::pack_hap()`, call `process_integrated_hsp()` before assembling the
final zip, so the copied artifacts are included:

```rust
pub fn pack_hap(config: &PackConfig) -> Result<PathBuf, PackError> {
    // ... existing processing ...

    // Process integrated HSP deps before zipping
    if !config.integrated_hsp_deps.is_empty() {
        process_integrated_hsp(&config.hap_staging_dir, &config.integrated_hsp_deps)?;
    }

    // ... zip assembly ...
}
```

### 5. `PackConfig` Extension

Add integrated HSP deps to `PackConfig`:

```rust
pub struct PackConfig {
    // ... existing fields ...
    /// Integrated HSP dependencies to embed in this HAP.
    pub integrated_hsp_deps: Vec<IntegratedHspDep>,
}
```

### 6. CLI Integration

In the build pipeline, collect integrated HSP deps from `oh-package.json5` resolution:

```rust
let integrated_hsp_deps = resolved_deps.iter()
    .filter(|d| d.is_integrated_hsp())
    .map(|d| IntegratedHspDep {
        module_name: d.module_name.clone(),
        abc_path: d.root_path.join("ets/modules.abc"),
        resources_index_path: Some(d.root_path.join("resources.index")),
        module_json_path: d.root_path.join("module.json"),
    })
    .collect();
```

---

## Files to Change

```
crates/arkdown-config/src/
└── oh_package.rs       — add integrated_hsp to OhPackageMetadata; is_integrated_hsp()

crates/arkdown-pack/src/
├── integrated_hsp.rs   — NEW: IntegratedHspDep, process_integrated_hsp()
└── lib.rs              — integrate into pack_hap(); add integrated_hsp_deps to PackConfig

crates/arkdown-cli/src/
└── build.rs            — collect integrated HSP deps; pass to PackConfig
```

---

## Test Plan

- `is_integrated_hsp()` returns `true` when `metadata.integratedHsp = true`.
- `is_integrated_hsp()` returns `false` when `metadata` is absent.
- `process_integrated_hsp()` copies `<dep>.abc` to `hap_output/ets/<module_name>.abc`.
- `process_integrated_hsp()` copies `resources.index` to
  `hap_output/resources/<module_name>/resources.index` when it exists.
- `process_integrated_hsp()` copies `module.json` to
  `hap_output/module_info/<module_name>/module.json`.
- `process_integrated_hsp()` skips `resources.index` copy when the source file does
  not exist (no error).
- `pack_hap()` with integrated HSP deps: final HAP zip contains
  `ets/<hspName>.abc` and `module_info/<hspName>/module.json`.
- `pack_hap()` with no integrated HSP deps: HAP zip is unchanged from current behavior.
- Missing `.abc` source file: `process_integrated_hsp()` returns `Err`, not panic.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/process-integrated-hsp.js`
  — `doTaskAction()`, integrated HSP dep resolution
- `meta/tasks/SPEC-012-packager.md` — `pack_hap()`, HAP zip structure
- `meta/design/hvigorw-analysis.md` §2 — HSP dependency types
- `meta/tasks/SPEC-031-bytecode-har-deps.md` — ByteCode HAR metadata detection
