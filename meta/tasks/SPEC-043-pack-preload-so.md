# SPEC-043: Preloaded SO File List Embedding

**Crate**: `arkdown-pack`, `arkdown-cli`
**Priority**: P2 — apps with preloaded system SO libraries
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

---

## Summary

After resource compilation, collect preloaded system SO file paths from the module's
`abilities` and HAR dependency `abilities` that are home abilities. Write the collected
paths to a preload SO file list at the intermediates path. This list is embedded in the
HAP archive so the ArkRuntime can preload the specified shared libraries at app launch.

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

The preload SO file path is `pathInfo.getPreloadSoFilePath()`:
```
<module>/build/default/intermediates/ets/default/preloadSoFileList.json
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

### 3. `PreloadSoFileList` Struct and Serialization

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct PreloadSoFileList {
    /// Absolute paths to .ets files for home abilities that need SO preloading.
    pub files: Vec<PathBuf>,
}

impl PreloadSoFileList {
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
```

### 4. Activation Condition

Preload SO collection is only active when:
- `isPreloadSoEnabled` is true in the project config (set by the SDK based on API level).
- The module is NOT an HSP.

```rust
pub fn should_collect_preload_so(
    config: &ArkdownConfig,
    module: &ModuleConfig,
) -> bool {
    config.is_preload_so_enabled && module.module_type != ModuleType::Hsp
}
```

### 5. HAP Packaging Integration

In `arkdown-pack::pack_hap()`, include `preloadSoFileList.json` in the HAP archive
when the file exists:

```rust
// In pack_hap():
let preload_so_path = intermediates_dir.join("preloadSoFileList.json");
if preload_so_path.exists() {
    zip.add_file("preloadSoFileList.json", &preload_so_path)?;
}
```

### 6. CLI Integration

In the build pipeline, after profile processing:

```rust
if should_collect_preload_so(&config, &module) {
    let preload_files = collect_preload_so_files(
        &module_json, &module_src_main_path, &har_deps
    );
    if !preload_files.is_empty() {
        let list = PreloadSoFileList {
            files: preload_files.into_iter().sorted().collect(),
        };
        let output_path = config.intermediates_dir.join("preloadSoFileList.json");
        list.write(&output_path)?;
    }
}
```

---

## Files to Change

```
crates/arkdown-pack/src/
├── preload_so.rs    — NEW: collect_preload_so_files(), is_home_ability(),
│                      PreloadSoFileList, should_collect_preload_so()
└── lib.rs           — re-export; include preloadSoFileList.json in pack_hap()

crates/arkdown-cli/src/
└── build.rs         — call collect_preload_so_files() after profile processing;
                       write preloadSoFileList.json to intermediates
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
- `should_collect_preload_so()` returns `false` when `is_preload_so_enabled = false`.
- `PreloadSoFileList::write()` produces valid JSON at the specified path.
- HAP archive includes `preloadSoFileList.json` when the file exists in intermediates.
- HAP archive does NOT include `preloadSoFileList.json` when no home abilities exist.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `addPreloadSystemSoFileList()`, `addSelfPreloadSystemSoFileList()`,
  `addDependencyPreloadSystemSoFileList()`, `allowToPreloadSystemSo()`,
  `isHomeAbility()`, `declareOutputFiles()`
- `meta/tasks/SPEC-012-packager.md` — HAP zip assembly
- `meta/design/hvigorw-analysis.md` §10 — HAP zip structure
