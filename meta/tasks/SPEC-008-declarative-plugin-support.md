# SPEC-008 Hvigor Plugin Support on Rolldown

## Status

Complete for the declared compile-plugin scope. The implementation target is
the hvigor and OHOS plugin runtime bundled with the latest installed DevEco
Studio, currently `6.24.3`.

## Goal

Allow existing TypeScript/JavaScript hvigor configuration plugins and
`compilePluginPath` plugins to run in arkdown while keeping one compile-plugin
engine: Rolldown's `rolldown_plugin::Plugin` system.

Arkdown must not maintain a second hook scheduler, a declarative approximation,
or a subprocess plugin protocol. A compatibility adapter is allowed only for a
latest-hvigor behavior that upstream Rolldown does not expose.

## Upstream Evidence

The behavior in this specification comes from the installed IDE, not a copied
repository dependency:

- `@ohos/hvigor@6.24.3` lifecycle and node/task implementations from the
  currently installed DevEco Studio distribution.
- `OhosModuleContextImpl._loadCompilePlugin` under the installed
  `hvigor-ohos-plugin/src/plugin/context/impl/` and the installed project,
  module, and hvigorfile JSON schemas.
- `@ohos/hvigor-arkts-compose` `arkts-pack.js`, especially `getPlugins`,
  `loadCustomCompilePlugins`, `validatePlugin`, `doWrapper`, and
  `getCompilePluginContext`.
- The IDE's patched Rollup in
  `@ohos/hvigor-arkts-base/node_modules/rollup/dist/shared/rollup.js`.

The patched Rollup lifecycle is:

```text
beforeBuild (sequential, SDK-internal)
buildStart (parallel)
afterBuildStart (sequential, SDK-internal)
graph.build
beforeBuildEnd (sequential)
buildEnd (parallel)
afterBuildEnd (parallel, SDK-internal)
bundleEnd / cleanUp
```

The public custom compile-plugin validator accepts exactly:

```text
name, order,
buildStart, resolveId, load, transform,
shouldInvalidCache, moduleParsed, beforeBuildEnd, buildEnd
```

Unknown properties warn. Invalid plugin type, missing/non-string name, invalid
hook value, invalid order, invalid function-order result, and duplicate final
plugin name are hard errors.

## Two Plugin Surfaces

### Hvigor configuration plugins

`hvigorfile.ts` and imported TypeScript plugins configure nodes, tasks, OHOS
contexts, and compile-plugin paths. They are not Rolldown plugins.

The Node package evaluates them in the same V8 process used by the native addon,
following latest-hvigor ordering:

```text
hvigorconfig evaluation
configEvaluated
node creation
nodesInitialized
project/module node evaluation
nodesEvaluated
task graph resolution
taskGraphResolved
native build
buildFinished
```

Configuration effects cross into Rust as typed build options or explicit task
records. The Node runtime is required here because arbitrary TypeScript plugin
code is an intentional hvigor capability.

### Compile plugins

The latest public project entry is a custom module HvigorPlugin calling
`OhosModuleContext.loadCompilePlugin(path)`. It validates a non-empty existing
`.ts` path and runs `doCompilePluginTypeCheck` before storing the module-scoped
compile-plugin path. Arkdown mirrors that TypeScript `4.9.5` check.

The installed type declarations retain `arkOptions.compilePluginFile`, but the
installed project, module, and hvigorfile schemas do not contain the field and
a real build rejects it. Declarative uses therefore fail with
`HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE`; they are not treated as a supported
discovery mechanism. Arkdown's direct package API continues to accept
`compilePluginPath` and `hvigorPluginFile` as explicit compatibility inputs.

Every accepted path must default-export one plugin object or a flat plugin
array. These are compile-pipeline plugins.

```text
TS/JS module
  -> latest-hvigor load/default-export/validation/order rules
  -> same-process NAPI ThreadsafeFunction callbacks
  -> JsPlugin implements rolldown_plugin::Plugin
  -> PluginFactory inserts it in the Rust-native Rolldown list
  -> Rolldown owns hook scheduling and module context
```

Standard hooks map directly:

| Hvigor hook | Rolldown implementation |
| --- | --- |
| `buildStart` | `Plugin::build_start` |
| `resolveId` | `Plugin::resolve_id` |
| `load` | `Plugin::load` |
| `transform` | `Plugin::transform` |
| `moduleParsed` | `Plugin::module_parsed` |
| `buildEnd` | `Plugin::build_end` |

Arkdown's explicit JavaScript API may additionally expose a real Rolldown hook
such as `resolveDynamicImport`; the strict hvigor file loader still warns for it
because latest hvigor does not accept it in an external compile-plugin object.

## Thin Hvigor Gap Adapter

Only behavior absent from upstream Rolldown belongs in the gap adapter:

- `beforeBuildEnd`: an internal Rust Rolldown plugin registers a `pre`
  `build_end` hook. It snapshots `getModuleIds()` and `getModuleInfo()` from the
  real Rolldown `PluginContext`, then invokes custom callbacks sequentially in
  the resolved plugin order before normal `buildEnd` hooks.
- `shouldInvalidCache`: evaluated at Arkdown's cache boundary. The public input
  shape must remain latest-hvigor `{ ast?, code, id }`.
- `this.share`: build metadata is seeded as per-plugin data before Rolldown
  starts. This is context initialization, not another lifecycle hook.

The gap trait must not contain Rolldown-native hooks. SDK-only hooks such as
`beforeBuild`, `afterBuildStart`, `afterBuildEnd`, `cleanUp`, and
`shouldTransformCachedModule` are implemented by their owning Rust stages when
needed; they are not accepted as external custom compile-plugin hooks.

## Ordering

The JavaScript loader evaluates `order` against the latest IDE's full built-in
plugin name/index record. It preserves hvigor's observable details:

- default and `last` append;
- repeated `first` entries reverse through repeated `unshift`;
- function order receives `Record<string, number>` and uses the IDE's
  index-zero truthiness behavior for duplicate-name suffixing;
- a negative function result inserts at zero;
- a non-number/`NaN` result or thrown callback is a hard error;
- duplicate names in the final virtual pipeline are a hard error.

The resolved virtual list is reduced to Rust-owned built-ins plus custom
plugins. Each custom plugin then receives an absolute slot for
`PluginFactory`; hook execution still belongs to Rolldown.

## Removed Alternative Implementations

The following approaches are obsolete and must not return:

- OXC extraction of a small "declarative plugin" subset;
- a Node child-process/JSON protocol for hook execution;
- an embedded QuickJS plugin engine;
- a standalone lifecycle scheduler or parity-report object that claims to run
  Rolldown hooks itself;
- separate `arkdown-plugin-*` crates for these adapters.

Rust is preferred for built-in business logic and performance-sensitive work.
Node/V8 is retained only for executing the arbitrary JS/TS behavior that hvigor
itself exposes.

## Closed Parity Proofs

The former plugin-system blockers are covered:

1. `shouldInvalidCache` receives each retained cached module's real `id`,
   transformed `code`, and optional parsed ESTree `ast`. A `true` result causes
   an output-correct whole-module rebuild.
2. The strict latest-hvigor wrappers discard returned Promises from
   `buildStart`, `moduleParsed`, and `buildEnd`; `beforeBuildEnd` is the awaited
   sequential hook. Arkdown reproduces those wrapper semantics. Plugins passed
   through Arkdown's explicit Rolldown API keep Rolldown's native async
   semantics.
3. Compile-plugin state is scoped by package module root. A regression fixture
   specifically protects the distinction between the package root and
   Rolldown's `src/main` loader root; programmatically target-scoped legacy
   entries are filtered by effective target.
4. The native E2E runs one `OhosModuleContext.loadCompilePlugin()` TypeScript
   fixture through both the installed hvigor and Arkdown and compares hook
   events, argument counts, graph-hook activity, and AST shape.

`.mjs` configuration parity is covered: Arkdown pins the IDE's TypeScript
`4.9.5` and installs the same CommonJS transpile hook for `.ts` and `.mjs`, so
both extensions use the OHOS module mocks and imported `.mjs` plugins follow
the latest runtime rather than Node's native ESM loader.

## Verification Contract

Every plugin behavior change must include:

1. the installed-IDE owner/function used as evidence;
2. a Node runtime test for module loading, validation, ordering, or context;
3. a Rust test proving standard hooks remain Rolldown plugins and gap hooks are
   represented by an internal Rolldown adapter;
4. an end-to-end build fixture when the hook can affect output;
5. hvigor/arkdown artifact comparison for the affected stage.

Passing only a loader unit test is not enough to claim compatibility.
