# SPEC-000 Crate Topology And Build Flow

## Goal

Define the final crate topology for the OXC/Rolldown replacement. The design
uses coarse manual orchestration in `packages/arkdown`, Rolldown for bundling
internals, OXC for source transforms, and typed external tool plans only at SDK
and native process boundaries.

Existing crates that only exist to preserve the old plugin/task split must be
deleted after their final production callers move.

## Final Crate Set

### Foundation

| Crate | Final responsibility |
| --- | --- |
| `arkdown-core` | Shared IDs, diagnostics, build modes, target enums, artifact contracts, domain-neutral errors. No IO and no build lifecycle. |
| `arkdown-path` | `AbsPath`, `RelPath`, `ArtifactPath`, `ModuleSpecifier`, `OhmUrl`, `PathDisplay`, lexical normalization, artifact slash serialization, path classification. |
| `arkdown-ohos-schema` | Typed OHOS JSON/JSON5 schemas: app, module, build profile, package, router/share/startup/privacy/resource schemas. |

### Startup And Project Model

| Crate | Final responsibility |
| --- | --- |
| `arkdown-config` | Thin compatibility parser facade. Long term it should only parse user config into schema/domain types or be folded into `arkdown-session` if it stops owning standalone behavior. |
| `arkdown-ohpm` | OHPM lockfile/cache/registry/integrity/tarball acquisition. No profile/resource/build orchestration. |
| `arkdown-session` | `BuildSessionBuilder`, immutable snapshots, dependency graph, package graph, source store, profile/resource/native inventories, SDK tool/schema snapshots, input fingerprints, artifact layout, and plan input views. |

### Source Transform And Bundling

| Crate | Final responsibility |
| --- | --- |
| `arkdown-ets-transform` | OXC-backed ArkTS/TS/JS transforms, checker passes, source maps, codegen, transform facts, custom declarative compile-plugin subset. |
| `arkdown-resolver` | OHOS-aware module/package resolution policy used by session and Rolldown adapters. No upward walking in hot hooks. |
| `arkdown` | Rolldown integration, `BundlePlan`, `Bundler::bundle_with_plan`, built-in Rolldown plugin adapters, bundle facts, source map policy, manual chunks, bundle cache bridge. |

`arkdown` owns built-in adapters as internal modules, for example:

```text
crates/arkdown/src/plugins/json5.rs
crates/arkdown/src/plugins/duplicate_check.rs
crates/arkdown/src/plugins/module_meta.rs
crates/arkdown/src/plugins/ohos_resolve.rs
crates/arkdown/src/plugins/har_dedup.rs
crates/arkdown/src/plugins/ets_transform.rs
```

The adapters delegate real work to `arkdown-resolver`, `arkdown-session`
derived plan data, `arkdown-ets-transform`, `arkdown-profile`, or bundle fact
collectors. They must not become separate lifecycle owners.

### Domain Stages

| Crate | Final responsibility |
| --- | --- |
| `arkdown-profile` | Module/profile merge, generated module JSON, build profile, router map, startup/share/privacy/PAC, permission validation, bytecode-HAR metadata inputs. |
| `arkdown-resource` | Resource inventory consumption, restool command plans, resource compile/link outputs, preview resource variants, code-resource/rawfile copy plans. |
| `arkdown-ts2abc` | Bytecode compile plan, SDK `es2abc` command plan/runner, cache-file/protoBin inputs, optional Rust backend selection. |
| `arkdown-pack` | HAR/HSP/HAP/app package plans, archive entry inventory/order, integrated HSP, native SO/preload entries, bytecode-HAR packaging metadata. |
| `arkdown-sign` | Sign/verify plans, SDK signing command plans, signed package outputs, remote HSP signing variants. |
| `arkdown-obfuscate` | Obfuscation option planning, rule merge, keep-name facts, mapping outputs consumed by transform/bundle/bytecode stages. |
| `arkdown-cache` | Fingerprint-based cache keys, stage cache records, bundle/bytecode/resource/package cache identity. |
| `arkdown-watcher` | Watch invalidation from session inventories and dependency graph views. No ad hoc rediscovery. |

### CLI

| Crate | Final responsibility |
| --- | --- |
| `packages/arkdown` | CLI compatibility, `BuildCoordinator`, module selection, dependency waves, domain plan construction, progress/diagnostics, external tool process execution, fixture parity commands. |

`packages/arkdown` is the only crate allowed to compose all domain crates.

## Crates To Delete

Delete these crates from `Cargo.toml`, `[workspace.dependencies]`, source tree,
tests, and imports after moving the final production caller:

| Crate | Replacement |
| --- | --- |
| `arkdown-plugin-json5` | `arkdown::plugins::json5` internal Rolldown adapter plus schema/parser helpers. |
| `arkdown-plugin-duplicate-check` | `arkdown::plugins::duplicate_check` internal adapter. |
| `arkdown-plugin-module-meta` | `arkdown::plugins::module_meta` internal adapter and bundle fact writer. |
| `arkdown-plugin-ohos-resolve` | `arkdown::plugins::ohos_resolve` internal adapter backed by `arkdown-resolver` and `arkdown-path`. |
| `arkdown-plugin-har-dedup` | `arkdown::plugins::har_dedup` internal adapter backed by dependency graph and pack/profile facts. |
| `arkdown-plugin-ets-transform` | `arkdown::plugins::ets_transform` internal adapter backed by `arkdown-ets-transform`. |

These crates are not final domain owners. Keeping them as separate crates adds
crate-boundary noise without preserving meaningful architecture.

Also delete any remaining crate or module whose only production purpose is:

- hvigor task runtime compatibility
- Rollup/webpack/ets-loader execution
- JS compile-plugin execution
- old task-name-driven orchestration
- duplicated dependency graph construction
- path/string helper ownership that belongs in `arkdown-path`

## Dependency Direction

Final allowed direction:

```text
foundation:
  arkdown-core
  arkdown-path
  arkdown-ohos-schema

startup:
  arkdown-config -> foundation
  arkdown-ohpm -> foundation
  arkdown-session -> foundation + config + resolver + ohpm

source/bundle:
  arkdown-ets-transform -> foundation + path + schema
  arkdown-resolver -> foundation + path + schema
  arkdown -> foundation + path + resolver + ets-transform + profile fact types

domain:
  arkdown-profile/resource/ts2abc/pack/sign/obfuscate/cache/watcher
    -> foundation + path + schema + typed plan inputs

cli:
  packages/arkdown -> all domain crates
```

Forbidden edges:

- domain crate -> `packages/arkdown`
- domain crate -> `arkdown-session` for execution-time behavior
- `arkdown` -> `packages/arkdown`
- `arkdown` -> deleted `arkdown-plugin-*` crates
- plugin/transform hot path -> config/session discovery APIs

## Build Flow

The final build flow is:

```text
packages/arkdown
  parse args
  select target/mode/product/module
  build BuildSession
  build domain plans
  execute dependency waves:
    profile/resource preparation
    Bundler::bundle_with_plan(BundlePlan)
    bytecode/external tools
    pack
    sign
  write deterministic artifacts
```

There is no full `BuildStageGraph` in the initial design. If a DAG is added
later, it must remain static and coarse-grained; it must not model hvigor task
APIs or Rolldown plugin hooks.

## `packages/arkdown` Logic

`BuildCoordinator` should be manual and explicit:

```text
BuildCoordinator::execute()
  load_session()
  create_build_plans()
  select_module_waves()
  for wave in waves:
    run_profile_and_resource(wave)
    run_bundle(wave)
    run_bytecode(wave)
    run_package_inputs(wave)
  run_packaging()
  run_signing()
```

Tokio may be used for:

- external process execution
- bounded concurrent module waves
- bounded concurrent resource/native/package tasks
- joining stage outputs by typed ID

CPU-heavy transforms, hashing, and inventory batch work should use Rayon or the
parallel model owned by OXC/Rolldown/domain crates.

## `arkdown` Logic

`arkdown` exposes one production entry point:

```text
Bundler::bundle_with_plan(BundlePlan) -> BundleOutput
```

Internally it:

1. validates `BundlePlan`;
2. creates Rolldown input/output options;
3. creates built-in adapter set in fixed order;
4. delegates module graph, plugin lifecycle, chunk graph, render hooks, and
   generate-bundle behavior to Rolldown;
5. collects typed bundle facts;
6. serializes only final artifact paths through `ArtifactPathSerializer`;
7. returns `BundleOutput` to `packages/arkdown`.

`arkdown` must not expose a custom generic plugin runtime.

## External Tool Logic

External tools use typed plans only where the tool boundary is real:

```text
ResourceToolPlan -> ResourceToolRunner
BytecodeToolPlan -> BytecodeToolRunner
PackToolPlan -> PackToolRunner
SignToolPlan -> SignToolRunner
NativeToolPlan -> NativeToolRunner
```

Tool runners may use `tokio::process::Command`, semaphores, and `JoinSet`.
They must merge results by typed ID, not completion order.

## Migration Order

1. Add this crate topology to the active plan.
2. Move all production transform logic from `arkdown-plugin-ets-transform` to
   `arkdown-ets-transform`.
3. Move all production plugin adapters from `arkdown-plugin-*` crates into
   `arkdown/src/plugins/`.
4. Update `packages/arkdown` to construct plugin adapters through `arkdown`, not
   concrete plugin crates.
5. Remove deleted plugin crates from workspace manifests.
6. Convert CLI build flow to `BuildCoordinator`.
7. Delete hvigor task/runtime compatibility modules once unsupported script
   diagnostics are in place.
8. Run workspace metadata, tests, clippy, and fmt checks.

## Verification

Required guardrail:

```bash
rg -n "arkdown-plugin-" Cargo.toml crates
rg -n "hvigorfile_runtime|HvigorTask|CompileNode|webpack|ets-loader|@ohos/hvigor" crates
cargo metadata --format-version 1 --no-deps
```

After migration, `arkdown-plugin-*` matches must be gone from production
manifests and source. Historical mentions may remain only in this plan or
audit-only documentation.
