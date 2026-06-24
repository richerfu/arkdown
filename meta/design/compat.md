# hvigorw Compatibility Layer

This document describes how arkdown provides a hvigorw-compatible interface so that existing OHOS projects can migrate with minimal changes.

---

## 1. Command Mapping

### hvigorw CLI Commands → arkdown Equivalents

| hvigorw command                        | arkdown command                             | Notes                  |
| -------------------------------------- | ------------------------------------------- | ---------------------- |
| `./hvigorw assembleHap`                | `arkdown build --target hap`                | Default product/module |
| `./hvigorw assembleHap --mode release` | `arkdown build --target hap --mode release` |                        |
| `./hvigorw assembleHsp`                | `arkdown build --target hsp`                |                        |
| `./hvigorw assembleHar`                | `arkdown build --target har`                |                        |
| `./hvigorw assembleApp`                | `arkdown build --target app`                | Builds all modules     |
| `./hvigorw clean`                      | `arkdown clean`                             | Deletes `build/`       |
| `./hvigorw test`                       | `arkdown test`                              | Runs ohosTest          |
| `./hvigorw default`                    | `arkdown build`                             | Default task           |
| `./hvigorw --version`                  | `arkdown --version`                         |                        |
| `./hvigorw -p key=value`               | `arkdown -p key=value`                      | Property override      |

### Module-Scoped Builds

hvigorw supports scoping to a specific module with `-m`:

```bash
./hvigorw assembleHap -m entry    # hvigor: module-scoped
arkdown build --target hap --module entry  # arkdown equivalent
```

### Product Override

```bash
./hvigorw assembleHap --product default
arkdown build --target hap --product default
```

---

## 2. `hvigorw` Shell Wrapper Script

arkdown provides a drop-in `hvigorw` replacement script that translates hvigor commands to arkdown:

```bash
#!/bin/bash
# ./hvigorw — drop-in replacement wrapper for arkdown
#
# Translates hvigorw arguments to arkdown arguments.
# Place this file at the project root alongside build-profile.json5.

ARKDOWN_BIN="${ARKDOWN_HOME:-$(which arkdown)}"

if [ -z "$ARKDOWN_BIN" ]; then
    echo "ERROR: arkdown not found. Install it or set ARKDOWN_HOME." >&2
    exit 1
fi

# Translate hvigorw positional commands to arkdown subcommands
CMD="$1"
shift

case "$CMD" in
    assembleHap)
        exec "$ARKDOWN_BIN" build --target hap "$@"
        ;;
    assembleHsp)
        exec "$ARKDOWN_BIN" build --target hsp "$@"
        ;;
    assembleHar)
        exec "$ARKDOWN_BIN" build --target har "$@"
        ;;
    assembleApp)
        exec "$ARKDOWN_BIN" build --target app "$@"
        ;;
    clean)
        exec "$ARKDOWN_BIN" clean "$@"
        ;;
    test)
        exec "$ARKDOWN_BIN" test "$@"
        ;;
    default|"")
        exec "$ARKDOWN_BIN" build "$@"
        ;;
    --version|-v)
        exec "$ARKDOWN_BIN" --version
        ;;
    *)
        echo "WARNING: Unknown hvigorw command '$CMD'. Passing to arkdown as-is." >&2
        exec "$ARKDOWN_BIN" "$CMD" "$@"
        ;;
esac
```

Install: replace `./hvigorw` in the project root with this script (or add a `hvigorw` alias).

---

## 3. `build-profile.json5` Field Mapping

### Direct support (pass-through)

| `build-profile.json5` field                                 | arkdown behavior                          |
| ----------------------------------------------------------- | ----------------------------------------- |
| `app.products[].name`                                       | Product name for `--product` flag         |
| `app.products[].compatibleSdkVersion`                       | Maps to `--target-api-version` for es2abc |
| `app.products[].runtimeOS`                                  | Validated; `"HarmonyOS"` supported        |
| `app.products[].signingConfig`                              | References `signingConfigs` by name       |
| `app.products[].buildOption.strictMode.caseSensitiveCheck`  | Enables case-sensitivity checks           |
| `app.products[].buildOption.strictMode.useNormalizedOHMUrl` | Enables OHM URL normalization             |
| `app.products[].buildOption.arkOptions.obfuscation`         | Obfuscation config (release mode)         |
| `app.signingConfigs[].material`                             | HAP signing material                      |
| `app.buildModeSet`                                          | Available build modes                     |
| `modules[].name`                                            | Module name                               |
| `modules[].srcPath`                                         | Module source directory                   |
| `modules[].targets`                                         | Product-to-target mapping                 |

### Target-level build options

Each target in `modules[].targets` may have its own `buildOption`:

```json5
{
  modules: [
    {
      name: "entry",
      srcPath: "./entry",
      targets: [
        {
          name: "default",
          applyToProducts: ["default"],
          buildOption: {
            sourceOption: {
              workers: ["./ets/workers/Worker.ets"],
              sourceRoots: ["./src/main/ets/generated"],
            },
          },
        },
      ],
    },
  ],
}
```

### Not supported in arkdown

| Field                              | Reason not supported                      |
| ---------------------------------- | ----------------------------------------- |
| `app.products[].compileSdkVersion` | arkdown uses `compatibleSdkVersion` only  |
| `buildOption.aotCompileMode`       | AOT is a separate step; not in arkdown v1 |
| `buildOption.apPath`               | AOT profile path; deferred                |
| `buildOption.hostPGO`              | PGO optimization; deferred                |

---

## 4. `hvigorfile.ts` Plugin API Differences

### hvigor plugin API (JavaScript)

hvigor custom compile plugins are loaded from `hvigorPluginFile` and follow this interface:

```typescript
// hvigorfile.ts (module-level)
import { moduleTasks, OhosHapContext, OhosPluginId } from "@ohos/hvigor-ohos-plugin";

const entryPlugin = {
  pluginId: OhosPluginId.OHOS_HAP_PLUGIN,
  apply(context: OhosHapContext): void {
    context.targets((target) => {
      target.getBuildProfile();
      target.getCompileArkTsOption();
    });
  },
};

export default {
  system: moduleTasks,
  plugins: [entryPlugin],
};
```

Rollup-level compile plugins (from `compilePluginPath` in build config):

```typescript
// Rollup plugin contract (from validatePlugin in arkts-pack.js)
export default {
  name: "my-plugin",
  order: "last", // 'first' | 'last' | (pluginNames: Record<string,number>) => number
  buildStart() {},
  resolveId(id, importer, options) {},
  load(id) {},
  transform(code, id) {},
  shouldInvalidCache(cached) {},
  moduleParsed(module) {},
  beforeBuildEnd(output) {},
  buildEnd(error) {},
};
```

### arkdown plugin API (Rust)

arkdown plugins implement the `ArkdownPlugin` trait (see [types.md](types.md)). The hooks map 1:1 to rollup hooks.

**Migration note**: Existing rollup-compatible JavaScript plugins can run in a compatibility shim (via NAPI), but native Rust plugins are preferred for performance.

### Plugin hook mapping

| rollup hook                   | arkdown hook                            | Notes |
| ----------------------------- | --------------------------------------- | ----- |
| `buildStart`                  | `build_start`                           |       |
| `resolveId`                   | `resolve_id`                            |       |
| `load`                        | `load`                                  |       |
| `transform`                   | `transform`                             |       |
| `moduleParsed`                | `module_parsed`                         |       |
| `beforeBuildEnd`              | `before_build_end`                      |       |
| `buildEnd`                    | `build_end`                             |       |
| `renderChunk`                 | `render_chunk`                          |       |
| `generateBundle`              | `generate_bundle`                       |       |
| `shouldInvalidCache`          | `should_invalidate_cache`               |       |
| `shouldTransformCachedModule` | (merged into `should_invalidate_cache`) |       |
| `resolveDynamicImport`        | (handled by `resolve_id`)               |       |

---

## 5. Property Overrides (`-p`)

hvigorw accepts `-p key=value` for build property overrides. arkdown supports the same:

| hvigorw property               | arkdown equivalent                              |
| ------------------------------ | ----------------------------------------------- |
| `-p ohos.buildMode=release`    | `--mode release` or `-p ohos.buildMode=release` |
| `-p ohos.arkObfuscation=true`  | `-p ohos.arkObfuscation=true`                   |
| `-p ohos.nativeResolver=true`  | (built-in in arkdown Rust resolver)             |
| `-p ohos.compileLibEntry=true` | `-p ohos.compileLibEntry=true`                  |
| `-p ohos.arkCompileMaxSize=8`  | `--jobs 8`                                      |
| `-p debugLine=true`            | `-p debugLine=true`                             |

---

## 6. Features arkdown Explicitly Will NOT Support

### Gradle Interop

hvigor has hooks into Gradle for native `.so` compilation (CMake/NDK). arkdown does not replicate this. Native code is compiled separately via CMake/Ninja and the resulting `.so` files are placed in `entry/libs/arm64-v8a/` for arkdown to package.

### Custom hvigorfile Task Registration

hvigorw supports registering arbitrary custom Gradle-like tasks in `hvigorfile.ts`. arkdown does not have a general task system — it has a fixed pipeline. Custom behavior is expressed via arkdown plugins, not custom tasks.

### Java Daemon

The hvigor daemon architecture (persistent JVM, shared class state) is not replicated. arkdown starts fresh each invocation; incremental state is managed via on-disk caches.

### FA Mode (Legacy Framework)

The legacy FA (Feature Ability) model with `module.json` (not `module.json5`) and the `compileMode: jsbundle` path. arkdown v1 targets stage model only (`esmodule`).

### `hvigorw buildPreviewerResource`

Preview mode (in-IDE hot-reload via DevEco Studio's previewer). Support may be added in v2.

### Integrated HSP Signing (`SignProjectRemoteHsp`, `SignModuleRemoteHsp`)

Remote HSP signing via the Huawei AppGallery integration. Deferred to v2.

### `@baize/wdk` Framework APIs

hvigor's internal `@baize/wdk` provides utilities like `cloneDeep`, `difference`, `uniq`. These are irrelevant to arkdown (Rust equivalents are trivial).

---

## 7. Migration Guide

### Minimal migration for a standard HAP project

1. **Install arkdown**: `cargo install arkdown` or download the binary
2. **Replace `hvigorw`**: Copy the compatibility wrapper script to `./hvigorw` (or use `arkdown` directly)
3. **No changes needed to**:
   - `build-profile.json5`
   - `oh-package.json5`
   - `module.json5`
   - `hvigorfile.ts` (for `appTasks`-only projects)
   - `.ets` source files
4. **Run**: `./hvigorw assembleHap` or `arkdown build --target hap`

### If you use custom rollup compile plugins

Custom plugins in `compilePluginPath` that follow the rollup plugin interface will work via the NAPI JavaScript plugin compatibility shim. However, for best performance, consider rewriting them as Rust `ArkdownPlugin` implementations.

### If you use hvigorfile.ts module-level plugins

hvigor module-level plugins that modify `getBuildProfile()` or `getCompileArkTsOption()` need to be expressed as arkdown config overrides or Rust plugins. There is no direct equivalent API.

### Environment variables

| hvigorw env var              | arkdown env var            |
| ---------------------------- | -------------------------- |
| `HVIGOR_DAEMON_IDLE_TIMEOUT` | Not applicable (no daemon) |
| `HVIGOR_HOME`                | `ARKDOWN_HOME`             |
| `NODE_OPTIONS`               | Not applicable             |
| `JAVA_HOME`                  | Not applicable             |

### CI/CD migration

Replace:

```yaml
- name: Build HAP
  run: ./hvigorw assembleHap --mode release
```

With:

```yaml
- name: Build HAP
  run: arkdown build --target hap --mode release
  # Or with compatibility wrapper:
  run: ./hvigorw assembleHap --mode release
```

---

## Cross-Reference

- [pipeline.md](pipeline.md) — what arkdown actually does for each command
- [hvigorw-analysis.md](hvigorw-analysis.md) — hvigor task chain that each command triggers
- [plugins.md](plugins.md) — plugin API and Rust plugin crates
- [types.md](types.md) — ArkdownPlugin trait
