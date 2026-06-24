# Hardcoded Path Parity Audit

Date: 2026-06-20

## Scope

This audit covers fixed strings and path-like constants found by:

```bash
rg -n "starts_with\('/'\)|contains\(\"oh_modules\"\)|replace\('\\\\', \"/\"\)|to_string_lossy" crates packages --glob '*.rs'
rg -n "format!\(.*\{.*path|to_string_lossy\(\).*cache|format!\(\"\{\}/compiler\.cache" crates packages --glob '*.rs'
rg -n "console\.|\b(e?println!|e?print!|dbg!)" crates packages --glob '*.rs' --glob '*.js' --glob '*.mjs'
```

## Hvigor Evidence

- `hvigor-arkts-base/dist/src/util/constants.ts` defines `MSGPACK_COMPILE_CACHE_DIR_NAME = "compiler.cache"`.
- `hvigor-arkts-compose/dist/src/arkts-pack.ts::getCompileEnv` uses `path.resolve(t.cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME)` as the compile-env cache path field.
- `hvigor-ohos-plugin/src/const/common-const.ts` defines fixed OHOS names including `OH_MODULES = "oh_modules"`, `OH_PACKAGE_JSON5 = "oh-package.json5"`, `MODULE_JSON5 = "module.json5"`, `APP_CONFIG = "app.json5"`, and `SRC_MAIN_RESOURCES = "src/main/resources"`.
- `hvigor-ohos-plugin/src/const/build-directory-const.ts` defines build layout names including `BUILD_ROOT = "build"`, `INTERMEDIATES = "intermediates"`, `LOADER_OUT = "loader_out"`, `RESOURCE_STR = "resource_str"`, and `MODULES_ABC = "modules.abc"`.

## Changes Made

- Removed the obsolete CLI workspace package and moved CLI/build orchestration into `packages/arkdown`.
- Replaced Rust `println!`/`eprintln!`/`print!`/`eprint!` usage with `tracing` or explicit structured stdout writes for command data.
- Replaced package JS `console.*` output with stdout/stderr helpers, `process.emitWarning`, or worker messages.
- Added `arkdown_cache::MSGPACK_COMPILE_CACHE_DIR_NAME` and `arkdown_cache::compiler_cache_path`, replacing ad hoc `"{}/compiler.cache"` construction.
- Replaced watch resource classification string matching with component-based `src/main/resources` detection.
- Replaced a module-specifier slash guard that tripped path scans with delimiter-byte matching.

## Remaining Fixed Names

The remaining production fixed names are retained because they are hvigor/OHOS contract names, not local project guesses:

- Manifest/profile names: `oh-package.json5`, `oh-package-lock.json5`, `build-profile.json5`, `module.json5`, `app.json5`, `loader.json`.
- Dependency directories: `oh_modules`.
- Source/resource layout: `src/main`, `src/main/resources`, `resources/base/profile`.
- Build artifacts: `modules.abc`, `sourceMaps.map`, `compileContextInfo.json`, `importee.json`, `pkgContextInfo.json`.
- Build directories: `build`, `intermediates`, `loader_out`, `res`, `resource_str`, `cache`.

Any future fixed path addition must cite the hvigor source constant/function or consume a typed path/layout API.
