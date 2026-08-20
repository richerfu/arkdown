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

`arkdown build --enable-zig` is an arkdown-only compiler selection extension.
It preserves hvigor's OHOS SDK CMake/Ninja inputs while replacing the C and C++
compiler drivers with `zig cc` and `zig c++`. Without the flag, the generated
native command remains on the SDK Clang path.

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

### hvigor configuration plugin API (TypeScript/JavaScript)

The Node package follows the `@ohos/hvigor@6.24.3` custom plugin contract. The
`apply` argument is a `HvigorNode`; the OHOS context is obtained from that node:

```typescript
// hvigorfile.ts (module-level)
import { hapTasks, OhosHapContext, OhosPluginId } from "@ohos/hvigor-ohos-plugin";

const entryPlugin = {
  pluginId: "example.entry-options",
  context() {
    return { enabled: true };
  },
  apply(node): void {
    const context = node.getContext(OhosPluginId.OHOS_HAP_PLUGIN) as OhosHapContext;
    context.loadCompilePlugin("./compile-plugin.ts");
  },
};

export default {
  system: hapTasks,
  plugins: [entryPlugin],
};
```

arkdown executes imported `.ts` plugins in the same Node/V8 process, binds each
node's custom `context` callbacks before running that node's `apply(node)`, and fires
`nodesEvaluated` after every project/module hvigorfile has been evaluated.
`Target.getBuildOption()` is a deeply frozen snapshot; configuration changes use
`setBuildProfileOpt()` or `loadCompilePlugin()`, as in hvigor 6.24.3.

For current projects, `loadCompilePlugin('./path/plugin.ts')` is the supported
compile-plugin registration API. Although an internal/latest type declaration
still mentions `arkOptions.compilePluginFile`, the installed project, module,
and hvigorfile schemas reject that declarative field. Arkdown rejects it with
`HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE` instead of creating a configuration path
that real hvigor does not accept. `loadCompilePlugin` also runs the same
TypeScript 4.9.5 pre-emit diagnostics as the IDE.

Rolldown-level compile plugins (registered through `loadCompilePlugin`, or
passed directly through Arkdown's `compilePluginPath` build config):

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

Arkdown's built-ins and JavaScript adapters implement
`rolldown_plugin::Plugin`. There is no parallel `ArkdownPlugin` hook engine.
Native Rust remains preferred for built-in business logic and hot paths; the
same-process NAPI adapter exists because arbitrary TypeScript/JavaScript
execution is part of hvigor's public plugin behavior.

`beforeBuildEnd` and `shouldInvalidCache` are the only latest-hvigor external
compile-plugin hooks not supplied by upstream Rolldown. They use thin gap
adapters while all standard hook scheduling remains in Rolldown.

### Plugin hook mapping

| latest-hvigor hook    | arkdown owner                               | Notes                                      |
| --------------------- | ------------------------------------------- | ------------------------------------------ |
| `buildStart`          | Rolldown `Plugin::build_start`              | direct                                     |
| `resolveId`           | Rolldown `Plugin::resolve_id`               | direct                                     |
| `load`                | Rolldown `Plugin::load`                     | direct                                     |
| `transform`           | Rolldown `Plugin::transform`                | direct                                     |
| `moduleParsed`        | Rolldown `Plugin::module_parsed`            | direct                                     |
| `beforeBuildEnd`      | internal Rolldown `pre` `build_end` adapter | sequential, real module graph context      |
| `buildEnd`            | Rolldown `Plugin::build_end`                | direct                                     |
| `shouldInvalidCache`  | cache-boundary gap adapter                  | real retained module `{ ast?, code, id }`    |

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

## 6. Current Replacement Blockers

### Gradle Interop

Arkdown packages prebuilt `.so` files and can run project-defined custom tasks,
but complete parity for every hvigor native CMake/NDK task remains to be proven.
Until then, native libraries can be built separately and placed in the module's
ABI directory.

### Arbitrary hvigor Task Graphs

Arkdown owns a typed native build pipeline and overlays observed custom-task
behavior. Package-bound custom tasks are supported: a task registered from a
TypeScript plugin or `nodesEvaluated` hook can use `postDependencies` targeting
`PackageHap`, `PackageHsp`, or `PackageHar`, and custom-to-custom dependencies
are executed in deterministic topological order. Edges to other hvigor built-in
tasks currently produce a typed diagnostic; each missing edge is a replacement
gap, not a permanent alternate contract.

### Java Daemon

The hvigor daemon architecture is not replicated. Arkdown starts fresh each
invocation and uses on-disk caches; command/output parity must not depend on the
daemon implementation detail.

### FA Mode (Legacy Framework)

The legacy FA (Feature Ability) model with `module.json` (not `module.json5`) and the `compileMode: jsbundle` path. arkdown v1 targets stage model only (`esmodule`).

### `hvigorw buildPreviewerResource`

Preview mode (in-IDE hot-reload via DevEco Studio's previewer). Support may be added in v2.

### Integrated HSP Signing (`SignProjectRemoteHsp`, `SignModuleRemoteHsp`)

Remote HSP signing via the Huawei AppGallery integration. Deferred to v2.

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

### If you use custom compile plugins

Keep the plugin as TypeScript and register it from a module HvigorPlugin with
`OhosModuleContext.loadCompilePlugin()`. It is type-checked, loaded, and
validated with latest-hvigor rules, then wrapped as a real Rolldown plugin in
the same Node/V8 process. Arkdown's direct Node API also accepts an explicit
`compilePluginPath`. No hook rewrite is required; Rust implementations are
preferred only for Arkdown-owned built-ins and performance-sensitive
extensions.

### If you use hvigorfile.ts module-level plugins

Arkdown evaluates TypeScript module plugins and exposes the latest node/task and
OHOS context surface already covered by the runtime. Unsupported artifact-
affecting context methods fail with `ARKDOWN_UNSUPPORTED_HVIGOR_CONTEXT_API`;
those diagnostics identify remaining replacement gaps rather than silently
ignoring configuration.

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
- [types.md](types.md) — Rolldown plugin boundary and hvigor gap metadata
