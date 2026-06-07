# SPEC-043: Preloaded SO File List Embedding

**Crate**: `arkdown-pack`, `arkdown-cli`
**Priority**: P2 — apps with preloaded system SO libraries
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

---

## Summary

After ArkTS compilation, hvigor uses `buildOption.preloadSystemSo` to collect source
entry files from the module's home abilities, module `srcEntry`, and source-HAR
home abilities. The ArkTS compiler/loader then emits a `preload.json` system
preload profile. Resource/package tasks consume that profile by adding
`systemPreloadHintStartupTasks` to the startup resource profile and ensuring
`module.appStartup` points at `$profile:startup_config`.

Correction: this is **not** direct native `.so` embedding under `libs/arm64-v8a/`.
`buildOption.preloadSo` is a legacy/older arkdown diagnostic path and should not be
treated as hvigor's current preload-system-SO package payload.

---

## Background: What hvigor Does

From `ark-compile.js`:

```javascript
// Activation check
allowToPreloadSystemSo() {
    return this.isPreloadSoEnabled && !this.moduleModel.isHspModule();
}

// Called in doTaskAction() when allowToPreloadSystemSo() is true
addPreloadSystemSoFileList() {
    this.addSelfPreloadSystemSoFileList();
    this.addDependencyPreloadSystemSoFileList();
}

addSelfPreloadSystemSoFileList() {
    const module = this.moduleJsonOpt?.module;
    // Add home abilities from self module
    module?.abilities?.forEach(ability => {
        if (isHomeAbility(ability)) {
            this.needPreloadSoFileSet.add(
                path.resolve(moduleSrcMainPath, ability.srcEntry)
            );
        }
    });
    // Add module srcEntry if present
    if (module?.srcEntry) {
        this.needPreloadSoFileSet.add(
            path.resolve(moduleSrcMainPath, module.srcEntry)
        );
    }
}

addDependencyPreloadSystemSoFileList() {
    this.service.getHarDependencies().forEach(dep => {
        if (dep.isByteCodeHarDependency()) return; // skip BC-HAR
        dep.getModuleJsonObj()?.module?.abilities?.forEach(ability => {
            if (isHomeAbility(ability)) {
                this.needPreloadSoFileSet.add(
                    path.resolve(dep.getDependencyRootPath(), "src/main", ability.srcEntry)
                );
            }
        });
    });
}
```

`isHomeAbility` checks if an ability has `entity.system.home` in its skills:

```javascript
function isHomeAbility(ability) {
    return ability.skills?.some(skill =>
        skill.entities?.includes('entity.system.home')
    );
}
```

The preload SO JSON path is `pathInfo.getPreloadSoFilePath()`:
```
<module>/build/default/intermediates/preload/default/preload.json
```

From `declareOutputFiles()`:
```javascript
if (this.allowToPreloadSystemSo()) {
    e.addEntry(this.pathInfo.getPreloadSoFilePath(), { isDirectory: false });
}
```

The file is written by the ArkCompile task (via `needPreloadSoFileSet`) and embedded
in the HAP by `PackageHap`.

---

## Scope

### 1. `is_home_ability()` Helper

```rust
/// Returns true if the ability has entity.system.home in its skills.
pub fn is_home_ability(ability: &AbilityConfig) -> bool {
    ability.skills.iter().any(|skill| {
        skill.entities.iter().any(|e| e == "entity.system.home")
    })
}
```

### 2. `collect_preload_so_files()` Function

```rust
/// Collect source entry paths for home abilities that should be preloaded.
/// Returns a set of absolute paths to .ets entry files.
pub fn collect_preload_so_files(
    module_json: &ModuleJson5,
    module_src_main_path: &Path,
    har_deps: &[HarDependency],
) -> HashSet<PathBuf> {
    let mut paths = HashSet::new();

    // Self module home abilities
    for ability in &module_json.module.abilities {
        if is_home_ability(ability) {
            paths.insert(module_src_main_path.join(&ability.src_entry));
        }
    }
    // Self module srcEntry
    if let Some(src_entry) = &module_json.module.src_entry {
        paths.insert(module_src_main_path.join(src_entry));
    }

    // HAR dependency home abilities (skip ByteCode HARs)
    for dep in har_deps {
        if dep.is_bytecode_har() { continue; }
        if let Some(dep_module_json) = &dep.module_json {
            for ability in &dep_module_json.module.abilities {
                if is_home_ability(ability) {
                    paths.insert(
                        dep.root_path.join("src/main").join(&ability.src_entry)
                    );
                }
            }
        }
    }

    paths
}
```

### 3. Compiler Handoff JSON

```rust
// hvigor passes `needPreloadSoFileSet` and `preloadSoFilePath` to the ArkTS
// compiler/loader. The generated file is:
// build/default/intermediates/preload/default/preload.json
//
// Its effective runtime payload is `systemPreloadHintStartupTasks`, not an
// archive entry containing the source-entry path set.
```

### 4. Activation Condition

Preload SO collection is only active when:
- `buildOption.preloadSystemSo` is true.
- The module is NOT an HSP.
- Widget compilation is not active.

```rust
pub fn should_collect_preload_so(
    config: &ArkdownConfig,
    module: &ModuleConfig,
) -> bool {
    config.product.build_option.preload_system_so && module.module_type != ModuleType::Hsp
}
```

### 5. HAP Resource/Package Integration

For HAP packaging, the important visible outputs are the generated startup profile
under resources and the module reference. `GeneratePkgModuleJson.ensureAppStartup()`
adds `module.appStartup = "$profile:startup_config"` when the preload JSON is
effective.

```rust
// After resource startup profile generation:
ensure_startup_profile_ref_in_module_json(
    &module_json_out,
    &startup_out,
)?;
```

For ByteCode HAR packaging, hvigor copies the effective preload file to
`META-INF/preload.json`.

### 6. CLI Integration

In the build pipeline, after startup/preload profile processing:

```rust
write_startup_config(&startup_out, &temp_startup_config)?;
write_startup_resource_profile(&startup_out, &res_profile_dir)?;
ensure_startup_profile_ref_in_module_json(&module_json_out, &startup_out)?;
```

---

## Files to Change

```
crates/arkdown-profile/src/
└── process_startup_config.rs — merge ByteCode HAR META-INF/preload.json tasks
crates/arkdown-cli/src/
└── build.rs         — write startup resource profile and ensure module.appStartup
crates/arkdown-pack/src/
└── lib.rs           — keep buildOption.preloadSo separate from native lib embedding
```

---

## Test Plan

- `is_home_ability()` returns `true` for an ability with `skills[].entities` containing
  `"entity.system.home"`.
- `is_home_ability()` returns `false` for an ability with no skills or other entities.
- `collect_preload_so_files()` includes the home ability `srcEntry` from the self module.
- `collect_preload_so_files()` includes `module.srcEntry` when present.
- `collect_preload_so_files()` includes home abilities from source HAR deps.
- `collect_preload_so_files()` skips ByteCode HAR deps.
- `collect_preload_so_files()` returns empty set when no home abilities exist.
- `should_collect_preload_so()` returns `false` for HSP modules.
- `should_collect_preload_so()` returns `false` when `preloadSystemSo = false`.
- HAP `resources/base/profile/startup_config.json` includes
  `systemPreloadHintStartupTasks` when ByteCode HAR preload metadata is effective.
- HAP `module.json` includes `appStartup: "$profile:startup_config"` when the
  startup profile was generated only by dependency/preload tasks.
- `buildOption.preloadSo` is not embedded as `libs/arm64-v8a/*.so`.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `addPreloadSystemSoFileList()`, `addSelfPreloadSystemSoFileList()`,
  `addDependencyPreloadSystemSoFileList()`, `allowToPreloadSystemSo()`,
  `isHomeAbility()`, `declareOutputFiles()`
- `meta/tasks/SPEC-012-packager.md` — HAP zip assembly
- `meta/design/hvigorw-analysis.md` §10 — HAP zip structure
