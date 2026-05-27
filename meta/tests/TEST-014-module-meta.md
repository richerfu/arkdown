# TEST-014: arkdown-plugin-module-meta

**Crate**: `arkdown-plugin-module-meta`
**Original source**: `hvigor-arkts-compose/dist/src/plugins/module-meta-info-plugin.js`

---

## Test File Layout

```
crates/arkdown-plugin-module-meta/src/
├── tests/
│   ├── meta_compute.rs       ← ModuleMeta field derivation per module type
│   ├── host_modules.rs       ← reverse importer map (hostModulesInfo)
│   ├── pkg_brief_info.rs     ← pkgNameToPkgBriefInfo generation
│   └── preload_so.rs         ← need_preload_so tagging from ability entries
```

---

## meta_compute.rs

### TC-001: local source file gets module_name from OHOS module

- **Input**: module at `entry/src/main/ets/pages/Index.ets`
  with `entry/oh-package.json5` containing `{ "name": "entry" }`
- **Expected**: `meta.module_name == Some("entry")`, `meta.pkg_name == Some("entry")`
- **Origin**: `load` hook walks up directory tree to find nearest `oh-package.json5`

### TC-002: SDK declaration module (`@ohos/hilog`) is not a local dependency

- **Input**: module resolved from SDK api dir
- **Expected**: `meta.is_local_dependency == false`

### TC-003: file in `oh_modules/` is not a local dependency

- **Input**: module at `entry/oh_modules/my-pkg/src/main/ets/index.ets`
- **Expected**: `meta.is_local_dependency == false`

### TC-004: file in `file:` HAR is a local dependency

- **Input**: module at `../my-har/src/main/ets/utils/Helper.ets`
  (resolved via `file:` reference in `oh-package.json5`)
- **Expected**: `meta.is_local_dependency == true`

### TC-005: `is_node_entry_file` true when file matches `main` field

- **Input**: module at `my-pkg/src/main/ets/index.ets`
  with `oh-package.json5` `{ "main": "src/main/ets/index.ets" }`
- **Expected**: `meta.is_node_entry_file == true`

### TC-006: `is_node_entry_file` false for non-entry files

- **Input**: module at `my-pkg/src/main/ets/utils/helper.ets`
- **Expected**: `meta.is_node_entry_file == false`

### TC-007: `pkg_version` extracted from `oh-package.json5`

- **Input**: `oh-package.json5` with `{ "version": "1.2.3" }`
- **Expected**: `meta.pkg_version == Some("1.2.3")`

### TC-008: `belong_module_path` is absolute path to OHOS module root

- **Input**: file at `project/entry/src/main/ets/pages/Index.ets`
  (the OHOS module root is `project/entry/`)
- **Expected**: `meta.belong_module_path == Some(PathBuf::from("project/entry"))`

---

## host_modules.rs

### TC-009: importing module populates importer's host_modules_info on the imported module

- **Setup**: module A imports module B
- **Expected after `module_parsed` for A**: `module_B.meta.host_modules_info` contains
  `HostModuleInfo { host_dependency_name: A.pkg_name, host_module_name: A.module_name }`
- **Origin**: `moduleParsed` hook walks `importedIds` and `dynamicallyImportedIds`

### TC-010: multiple importers accumulate in host_modules_info

- **Setup**: modules A and B both import module C
- **Expected**: `module_C.meta.host_modules_info` has entries for both A and B

### TC-011: dynamic import also counted as host

- **Setup**: module A has `import('./lazy')` dynamic import of module B
- **Expected**: B's `host_modules_info` contains A

### TC-012: no-import modules have empty host_modules_info

- **Setup**: module C imported by no one
- **Expected**: `module_C.meta.host_modules_info == []`

---

## pkg_brief_info.rs

### TC-013: `pkgNameToPkgBriefInfo` contains entry module

- **Setup**: project with one OHOS module `entry`
- **Expected**: `pkg_brief_info["entry"].pkg_root == "/abs/path/to/entry"`,
  `source_roots` contains `"src/main/ets"`

### TC-014: local HAR dependency appears in pkg_brief_info

- **Setup**: `entry/oh-package.json5` has `{ "dependencies": { "my-har": "file:../my-har" } }`
- **Expected**: `pkg_brief_info["my-har"].pkg_root == "/abs/path/to/my-har"`

### TC-015: ohpm package does NOT appear in pkg_brief_info

- **Setup**: dependency from `oh_modules/remote-pkg/`
- **Expected**: `pkg_brief_info` does not have key `"remote-pkg"` (only local deps listed)

### TC-016: source_roots includes generated build profile directory

- **Setup**: standard entry module with target `default`
- **Expected**: `pkg_brief_info["entry"].source_roots` includes
  `".hvigor/default/generated/"` equivalent path

---

## preload_so.rs

### TC-017: ability entry module gets need_preload_so = true

- **Setup**: ability `srcEntry = "./ets/entryability/EntryAbility.ets"`
- **Expected after `before_build_end`**: `EntryAbility.ets` module has `meta.need_preload_so == true`

### TC-018: module reachable from ability entry gets need_preload_so = true

- **Setup**: `EntryAbility.ets` → imports → `utils/Helper.ets`
- **Expected**: `Helper.ets` also has `meta.need_preload_so == true`

### TC-019: module NOT reachable from ability entry has need_preload_so = false

- **Setup**: `utils/Unreachable.ets` is not imported by any ability entry
- **Expected**: `meta.need_preload_so == false`
