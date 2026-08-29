# Plugin Architecture and Inventory

Arkdown has one compile-plugin engine: Rolldown.

All Rust built-ins and same-process JavaScript adapters implement
`rolldown_plugin::Plugin`. TypeScript execution is retained only at the Node/V8
compatibility boundary required by hvigor; Arkdown does not have a declarative
plugin subset, a child-process hook protocol, or a separate lifecycle runner.

The behavioral baseline is the plugin pipeline bundled with the latest
installed DevEco Studio (`@ohos/hvigor-arkts-compose@6.24.3` and its SDK
plugins), not the repository's historical hvigor copy.

## Rust/V8 Ownership Boundary

JavaScript is an ABI and plugin-authoring façade, not a build engine. The
production ownership split is:

| Behavior | Owner |
| --- | --- |
| ArkTS/TypeScript parse, transform, CommonJS emission, ESTree parse | Rust/OXC |
| module graph, standard hook scheduling, plugin contexts | Rust/Rolldown |
| project/model inventory, JSON5, schemas, targets and build options | Rust session/config crates |
| plugin shape, order, native slots, paths and extension precedence | Rust plugin/config planners |
| task catalog/graph/command plan, cache identity, CLI/native request planning | Rust runtime/session planners |
| loading user modules and invoking arbitrary user functions | Node/V8 glue |
| preserving function/object identity and adapting live Rolldown arguments | Node/V8 glue |
| Node-specific public façades (`Module`, `Buffer`, `Promise`, log4js, iconv) | Node/V8 glue |

The bridge may materialize JavaScript `Error`, warning, iterator, AST literal,
and façade objects because those are observable plugin API values. It must not
reimplement a parser, resolver, plugin scheduler, task graph, cache layout, or
build-option planner. Callback-produced plain configuration is merged by Rust
without JSON serialization so live JavaScript function identity is retained.

## External Compile Plugins

The current public hvigor entry is a module custom plugin calling
`OhosModuleContext.loadCompilePlugin(path)`. The path must exist and end in
`.ts`; before registration, Arkdown runs the same TypeScript `4.9.5`
`createProgram`/`getPreEmitDiagnostics` check and compiler options as the
installed IDE.

```typescript
const configureCompilePlugin = {
  pluginId: 'example.compile-plugin',
  apply(node) {
    node.getContext(OhosPluginId.OHOS_HAP_PLUGIN)
      .loadCompilePlugin('./compile-plugins/my-plugin.ts')
  },
}
```

Arkdown's direct package API also accepts `compilePluginPath` and
`hvigorPluginFile`. Every path is loaded by `packages/arkdown/js/runtime.js` and
must default-export one object or a flat object array. The strict latest-hvigor
surface is:

```typescript
export default {
  name: 'my-plugin',
  order: 'last', // 'first' | 'last' | (indices: Record<string, number>) => number
  buildStart() {},
  resolveId(id, importer, options) {},
  load(id) {},
  transform(code, id) {},
  shouldInvalidCache(options) {},
  moduleParsed(info) {},
  beforeBuildEnd(input) {},
  buildEnd(error) {},
}
```

Standard hooks are NAPI callbacks on `JsPlugin`, which directly implements
Rolldown's plugin trait:

| JavaScript hook | Rust/Rolldown hook |
| --- | --- |
| `buildStart` | `Plugin::build_start` |
| `resolveId` | `Plugin::resolve_id` |
| `load` | `Plugin::load` |
| `transform` | `Plugin::transform` |
| `moduleParsed` | `Plugin::module_parsed` |
| `buildEnd` | `Plugin::build_end` |

Two latest-hvigor hooks are not present in upstream Rolldown:

- `beforeBuildEnd` is supplied by an internal Rolldown plugin whose `pre`
  `build_end` callback snapshots the real module graph and invokes external
  callbacks sequentially.
- `shouldInvalidCache` is called by the owning cache stage with the exact
  public `{ ast?, code, id }` shape for every retained cached Rolldown module.
  Any `true` result invalidates the module build cache; rebuilding the complete
  module is intentionally coarser than Rollup's retained-graph update but
  preserves output correctness.

`this.share` is seeded as per-plugin context data before Rolldown starts. It is
not modeled as another build hook.

## Latest Built-in Order and Rust Ownership

Function-valued external `order` callbacks observe the full latest IDE name
record. The runtime then reduces that virtual list to Rust-owned adapters while
preserving the custom plugin's relative slot.

| Latest IDE name | Arkdown owner/equivalent |
| --- | --- |
| `commonPlugin` | bundle plan and per-build context |
| `invalidCachePlugin` | `arkdown-cache` and cache bridge |
| `checkDuplicatedFileNamesPlugin` | `plugins::duplicate_check` |
| `moduleInfoMetaPlugin` | `plugins::module_meta` |
| `oh-resolve` | `plugins::ohos_resolve` + `arkdown-resolver` |
| `routerMapPlugin` | profile/router-map fingerprints |
| `newOhmUrlPlugin` | resolver/config fingerprint policy |
| `sensitiveCheckPlugin` | `plugins::import_check` + resolver case checks |
| `commonjs` | Rolldown parser/linker behavior where applicable |
| `gcPlugin` | Rust ownership; no forced Node GC stage |
| `json` | `plugins::json5` |
| `wrapperChunkPlugin` | deleted — `render_chunk` unreachable on the scan-only path; `HvigorNativePluginSlot::WrapperChunk` kept only as a JS-plugin anchor |
| `waitAsyncPlugin` | Rust async/Rolldown hook futures |
| `collectImportersPlugin` | Rolldown module graph facts |
| `hspCollectHarFiles` | `plugins::har_dedup` (HSP collect/real passes) |
| `memoryMonitor` | optional Rust memory diagnostics |
| `watchChangedFiles` | watch invalidation/session snapshots |
| `etsChecker` | `arkdown-ets-transform` checker mode |
| `etsTransform` | `plugins::ets_transform` + `arkdown-ets-transform` |
| `apiTransform` | `arkdown-ets-transform::api_transform` |
| `buildInstrument` | coverage transform path |
| `genAbc` | native/SDK ABC stage |
| `createProgramPlugin` | preview/compiler stage policy |
| `delete` | deterministic output cleanup/minify stage |

There are no final-architecture `arkdown-plugin-*` crates. Hook adapters live
under `crates/arkdown/src/plugins/`; non-hook logic belongs to its domain crate.

## Configuration Plugins

Hvigor configuration plugins in `hvigorfile.ts` are a separate surface. The
Node runtime evaluates them with latest-hvigor node/task/lifecycle and OHOS
context adapters. Their effects become typed native build inputs or custom task
records. They do not implement Rolldown hooks unless they explicitly load a
compile plugin.

`.ts` and `.mjs` use the same synchronous CommonJS loading boundary as the
installed hvigor runtime. Rust/OXC/Rolldown reads and compiles each file to the
latest ES2016 target, preserves user imports for Node resolution, and embeds
OXC runtime helpers. JavaScript passes the generated text to `Module._compile`,
exposes plugin-facing classes/functions, and forwards callback effects to typed
Rust planners while the live mock layer supplies `@ohos/hvigor*` packages.
TypeScript `4.9.5` is retained only for latest-hvigor semantic type checking.

The installed `build-opt.d.ts` still contains a legacy
`arkOptions.compilePluginFile`, but that field is absent from the latest
project, module, and hvigorfile schemas and a real 6.24.3 build rejects it.
Arkdown therefore reports `HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE` for declarative
uses and directs projects to `OhosModuleContext.loadCompilePlugin(path)`. It
does not turn an internal/type-only field into a new public configuration API.

## Validation and Diagnostics

- Non-object export, missing/non-string name, invalid hook/order value,
  invalid function-order result, and duplicate final name are hard errors.
- Unknown properties warn exactly at the strict hvigor file-loader boundary.
- Synchronous hook exceptions append the source plugin path as latest hvigor
  does.
- Unsupported OHOS configuration APIs fail with typed diagnostics; no plugin
  behavior is silently approximated.

## Replacement Proof

The declared compile-plugin replacement scope is closed against the installed
IDE baseline:

- runtime tests cover strict loading, validation, ordering, lifecycle argument
  shapes, `this.share`, schema rejection, and TypeScript type checking;
- Rust tests prove standard callbacks are real Rolldown plugins, the two gap
  hooks stay outside the standard hook trait, and module/target filtering uses
  the package module root rather than the loader source root;
- the native E2E runs the same `OhosModuleContext.loadCompilePlugin()` fixture
  through hvigor 6.24.3 and Arkdown, comparing lifecycle events, argument
  counts, module graph callbacks, and ESTree `Program` input;
- cache tests call `shouldInvalidCache` with real retained module IDs, code,
  and parsed AST.

Unsupported OHOS context mutation APIs and arbitrary edges into hvigor's
built-in task graph remain broader orchestration gaps; they fail with typed
diagnostics and are not a second compile-plugin engine.
