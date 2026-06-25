# SPEC-001 OXC/Rolldown Hvigor Replacement Architecture

## Goal

Replace hvigor's JavaScript/TypeScript build system with arkdown's Rust
pipeline. OXC and Rolldown are the required implementation ecosystem for
JavaScript, TypeScript, ArkTS parsing, transforms, bundling, source maps,
plugin hooks, and code generation. External SDK/native tools may remain
process-backed where they are true compiler, packager, signer, resource, or
native-build binaries.

## Current Hvigor Responsibility Map

| hvigor area | Observed responsibility | arkdown owner |
| --- | --- | --- |
| `hvigor/src/base/**` | CLI runtime, task graph, plugin API, lifecycle, daemon, config loader | `packages/arkdown`, `arkdown-session` |
| `hvigor-arkts-base/dist/src/build.ts` | Rollup invocation, watch, cache read/write, bundle output, importee cleanup | `arkdown`, `arkdown-cache`, `arkdown-watcher`, `arkdown::plugins::ohos_resolve` |
| `hvigor-arkts-compose/dist/src/plugins/**` | Rollup plugins and compile-plugin behavior | `arkdown` built-in Rolldown adapters, `arkdown-ets-transform`, `arkdown-resolver` |
| `hvigor-ohos-plugin/src/tasks/manager/**` | Task registration by module type, language, preview/test/reload mode | `packages/arkdown` coarse manual orchestration |
| `hvigor-ohos-plugin/src/tasks/ark-compile.ts` | ArkTS compile config, obfuscation, dependency metadata, copy code resources, worker dispatch | `packages/arkdown`, `arkdown`, `arkdown-ets-transform`, `arkdown-obfuscate`, `arkdown-resource` |
| `hvigor-ohos-plugin/src/tasks/compile-node.ts` | webpack/loader-based JS/ETS compile | delete; replace with Rolldown |
| `hvigor-ohos-plugin/src/tasks/process-*.ts` | profile, resources, startup/share/router/privacy/profile transforms | `arkdown-profile`, `arkdown-resource`, `arkdown-session` |
| `hvigor-ohos-plugin/src/tasks/package-*.ts` | HAP/HSP/HAR/app packaging | `arkdown-pack` plus allowed SDK tool runner |
| `hvigor-ohos-plugin/src/tasks/sign-*.ts` | signing and verification command building | `arkdown-sign` plus allowed SDK tool runner |
| `hvigor-ohos-plugin/src/tasks/build-native-*`, `do-native-*`, `process-libs.ts` | CMake/Ninja/native lib processing/strip/binxo | `packages/arkdown::cmd::native_build`, allowed external tools |

## Production Boundary

The final production pipeline must be:

```text
CLI args
  -> BuildSessionBuilder
  -> BuildPlanBuilder
  -> BuildCoordinator
  -> per-module dependency waves
  -> profile/resource/source/rolldown/abc/pack/sign/native stages
  -> deterministic artifact writers
```

No production stage may call hvigor's task runtime, Rollup, webpack,
ets-loader, hvigor-arkts-base, hvigor-arkts-compose, or hvigor-ohos-plugin
JavaScript.

## Crate Ownership

### `packages/arkdown`

Must own:

- CLI compatibility flags and target/mode/product/module selection.
- Coarse manual stage orchestration from typed plans.
- Dependency waves, progress, diagnostics, and lifecycle.
- External tool runners for allowed SDK/native tools.
- Custom-script compatibility diagnostics.

Must not own:

- ArkTS parser/transform internals.
- Bundle plugin implementation logic.
- Package graph construction.
- Profile/resource/schema parsing rules.

### `arkdown-session`

Must own:

- One-time startup discovery.
- Project, module, package, SDK, profile, resource, source, native, and mode
  snapshots.
- One dependency graph snapshot used by resolver, profile, bundle, HAR/HSP,
  bytecode HAR, native SO, integrated HSP, cache, and watch invalidation.
- Input fingerprints and artifact layout.

Must delete or rename hvigor-specific runtime snapshot types after equivalent
generic typed inputs exist. `HvigorScriptSnapshot` and `HvigorTaskGraphSnapshot`
must not remain final production public APIs.

### `arkdown`

Must own:

- Rolldown invocation.
- Bundle plans, output plans, manual chunk policy, source map policy, and
  Rolldown cache bridge.
- Bundler-level facts emitted by plugin adapters.

Must not depend on:

- Concrete plugin crates.
- `arkdown-session`.
- CLI types.

### `arkdown-ets-transform`

Must own:

- OXC parser setup for ArkTS/TypeScript/JavaScript inputs.
- ArkTS syntax lowering, decorator/component transforms, API transform,
  resource/syscap rewrites, lazy import handling, checker rules, source maps,
  emitted code, and typed transform facts.

Must not:

- Persist OXC allocator-backed AST nodes outside a transform call.
- Read project/package/profile/SDK files directly.
- Assemble source code with raw strings when an OXC AST/codegen path exists.

### Built-In Rolldown Adapters

Separate `arkdown-plugin-*` crates are not part of the final topology. Built-in
adapter modules inside `arkdown` must:

- Receive hook input and typed context.
- Delegate domain work to owning crates.
- Return Rolldown hook output.
- Declare only hooks actually implemented.

Adapter modules must not own session discovery, file walking, profile parsing,
SDK parsing, cache policy, or artifact writing.

## External Tool Policy

Allowed process-backed stages must be represented by typed inputs and typed
outputs:

- `packages/arkdown::cmd::build::bytecode`: `es2abc` command plans, cache-file/protoBin inputs,
  bytecode outputs, diagnostics.
- `arkdown-resource`: `restool` command plans and compiled resource outputs.
- `arkdown-pack`: package command or archive writer plans and final package
  entries.
- `arkdown-sign`: signing command plans, verification inputs, signed outputs.
- `packages/arkdown::cmd::native_build`: CMake/Ninja/strip/binxo/native library plans.

The command line can remain close to upstream hvigor behavior, but command
construction must consume typed plans. Command string assembly must not be used
as the source of build identity.

## Deletion Targets

Delete production dependencies and code paths for:

- hvigor task/runtime execution.
- Rollup JavaScript invocation.
- webpack/ets-loader compile invocation.
- JS compile-plugin shims.
- hot-path plugin filesystem reads.
- duplicated dependency graph builders.
- legacy task names as orchestration identity.
- public `Hvigor*` session/runtime types once generic replacements exist.
- separate `arkdown-plugin-*` crates after their adapters move into `arkdown`.

The `hvigor/ts-readable` tree may remain as an audit fixture only while parity
work is active. It must not be imported, executed, or required by production
code.

## Acceptance Criteria

- Building standard fixtures does not require Node-based hvigor, Rollup,
  webpack, or hvigor-arkts-compose.
- All build stages receive typed plans.
- `cargo metadata --format-version 1 --no-deps` shows acyclic crate
  dependency direction.
- `rg -n "rollup\\(|webpack|ets-loader|hvigor-arkts-compose|@ohos/hvigor" crates`
  returns no production execution path.
- Public session API has generic build-script/stage concepts instead of
  hvigor runtime concepts.
