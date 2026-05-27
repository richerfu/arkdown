# SPEC-014: Module Meta Plugin

**Crate**: `arkdown-plugin-module-meta`
**Priority**: P0 — metadata consumed by loader.json generation and OHM URL resolution
**Blocked by**: SPEC-005
**Blocks**: SPEC-015, SPEC-016

---

## Summary

Implements the `module-meta-info-plugin` equivalent. Annotates each module with OHOS-specific
metadata (`pkgName`, `moduleName`, `isLocalDependency`, etc.) consumed by several downstream
systems: loader.json, ByteCode HAR lookup, OHM URL generation, and hot-reload tracking.

This is the most complex built-in plugin.

---

## Hooks Implemented

### `build_start`

Initialize `pkg_info_cache: HashMap<PathBuf, OhPackageJson5>` by scanning all
`oh_modules/*/oh-package.json5` files and local module `oh-package.json5` files.

### `load` hook

For each loaded module, compute its `ModuleMeta`:

```rust
fn compute_meta(id: &str, pkg_info_cache: &HashMap<PathBuf, OhPackageJson5>) -> ModuleMeta {
    // 1. Walk up the directory tree from id to find the nearest oh-package.json5
    // 2. Extract pkgName, pkgVersion from that file
    // 3. Determine moduleName = the OHOS module (e.g. "entry") the file belongs to
    // 4. is_local_dependency = file is under a local file: HAR path (not oh_modules)
    // 5. is_node_entry_file = id matches the main/types field of its oh-package.json5
    // 6. belong_module_path = absolute path to the OHOS module root
    // 7. belong_project_path = project root
}
```

### `module_parsed` hook

Build `host_modules_info` (reverse importer map):

```rust
for imported_id in &module.imports {
    reverse_map.entry(imported_id.resolved.id.clone())
               .or_default()
               .push(HostModuleInfo {
                   host_dependency_name: module.meta.pkg_name.clone().unwrap_or_default(),
                   host_module_name: module.meta.module_name.clone().unwrap_or_default(),
               });
}
```

### `before_build_end` hook

Tag modules reachable from ability entry points with `need_preload_so = true` (for `.so`
preloading optimization). Walk the module graph from each ability `srcEntry` and mark
all reachable modules.

---

## Key Data Used By Downstream Systems

| Consumer                              | Fields Used                                    |
| ------------------------------------- | ---------------------------------------------- |
| loader.json `pkgNameToPkgBriefInfo`   | `pkg_name`, `pkg_path`, `source_roots`         |
| OHM URL generation (SPEC-003/015)     | `pkg_name`, `bundle_name`, `module_name`       |
| ByteCode HAR detection                | `is_local_dependency`, `pkg_name`              |
| `dynamicImportLibInfo` in loader.json | `host_modules_info`, `module_name`, `pkg_path` |
| SO preloading                         | `need_preload_so`                              |

---

## pkgNameToPkgBriefInfo Format

This is the primary output consumed by the resolver and loader.json:

```json
{
  "entry": {
    "pkgRoot": "/abs/path/to/entry",
    "pkgName": "entry",
    "sourceRoots": ["src/main/ets", ".hvigor/default/generated/", "src/test/ets"],
    "originalSourceRoots": []
  },
  "my-har": {
    "pkgRoot": "/abs/path/to/my-har",
    "pkgName": "my-har",
    "sourceRoots": ["src/main/ets"]
  }
}
```

Computed by walking all local dependencies (modules + local HAR file: refs) and reading their
source root configurations.

---

## Files to Create

```
crates/arkdown-plugin-module-meta/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── plugin.rs           (ModuleMetaPlugin: ArkdownPlugin impl)
    ├── meta_compute.rs     (compute_meta: walk oh-package.json5 hierarchy)
    ├── host_modules.rs     (reverse importer graph construction)
    └── pkg_brief_info.rs   (pkgNameToPkgBriefInfo generation)
```

---

## Acceptance Criteria

- Every module in `MyApplication` has non-None `meta.module_name` after `load` hook
- `pkgNameToPkgBriefInfo` includes both `entry` and any local HAR dependencies
- `host_modules_info` correctly maps HAR modules to their consuming HAP modules
- SDK declaration modules (`@ohos/*`) have `meta.is_local_dependency = false`

---

## Design References

- `meta/design/plugins.md` — `module-meta-info-plugin` detailed analysis
- `meta/design/hvigorw-analysis.md` — `pkgNameToPkgBriefInfo` schema, loader.json fields
- `meta/design/types.md` — `ModuleMeta`, `HostModuleInfo`

---

## Rolldown Alignment

| arkdown concept                                           | rolldown equivalent                    | note                                                                                                                                                            |
| --------------------------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ModuleMetaPlugin::build_start`                           | `Plugin::build_start`                  | Same hook; used to initialize the plugin's caches                                                                                                               |
| `ModuleMetaPlugin::load` (meta computation)               | `Plugin::load` + `ModuleInfo::meta`    | rolldown allows plugins to attach arbitrary `meta` to a module via `HookLoadOutput`; arkdown uses `ModuleMeta` struct on `Module`                               |
| `ModuleMetaPlugin::module_parsed` (reverse graph)         | `Plugin::module_parsed`                | Identical hook; rolldown's `ModuleInfo::importers` is a rolldown-managed list; arkdown builds its own `host_modules_info` because it needs OHOS package context |
| `ModuleMetaPlugin::before_build_end` (SO preload tagging) | no rolldown equivalent                 | OHOS-specific: marks modules reachable from ability entry points for `.so` preloading                                                                           |
| `pkg_info_cache`                                          | rolldown's internal module cache       | Both cache parsed package manifests; rolldown caches module graphs; arkdown caches `oh-package.json5` parse results                                             |
| `pkgNameToPkgBriefInfo` output                            | rolldown `ModuleInfo::meta` per module | arkdown aggregates cross-module metadata into a single map; rolldown keeps it per-module                                                                        |
| `host_modules_info` (reverse importer map)                | rolldown `ModuleInfo::importers`       | rolldown tracks importers automatically in `ModuleInfo`; arkdown builds an explicit OHOS-annotated map that includes `pkg_name` and `module_name` per importer  |

**Key divergence:** rolldown's `ModuleInfo` is read-only from plugins after the build. arkdown's `ModuleMeta` is populated by this plugin during the load hook and is mutable. This is intentional: OHOS metadata must be attached at load time because it is needed by other hooks in the same build.

---

## Test Requirements

- After `build_start`, `pkg_info_cache` contains entries for all `oh_modules/*/oh-package.json5` files in the project
- `compute_meta()` for a file under `entry/src/main/ets/` sets `module_name = "entry"` and `is_local_dependency = false`
- `compute_meta()` for a file under `oh_modules/my-har/src/main/ets/` sets `pkg_name = "my-har"` and `is_local_dependency = false` (installed ohpm package, not local file: HAR)
- `compute_meta()` for a file under a `file:../my-har` local HAR path sets `is_local_dependency = true`
- `is_node_entry_file = true` for the file that matches the `main` field of its `oh-package.json5`
- `module_parsed` hook: after processing a module that imports `my-har`, the `host_modules_info` entry for the `my-har` module contains the importing module's `pkg_name`
- `before_build_end`: modules reachable from `abilities[*].srcEntry` have `need_preload_so = true`
- `before_build_end`: modules not reachable from any ability entry point have `need_preload_so = false`
- SDK declaration modules (`module_type = Declaration`) are excluded from `pkgNameToPkgBriefInfo`
- `pkgNameToPkgBriefInfo` for the `entry` module includes `src/main/ets` and any generated source roots in `sourceRoots`
- Plugin name returns `"module-meta"`
