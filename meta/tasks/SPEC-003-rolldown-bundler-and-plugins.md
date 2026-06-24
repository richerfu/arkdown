# SPEC-003 Rolldown Bundler And Plugins

## Goal

Replace hvigor's Rollup/webpack compile path with Rolldown and thin Rust
plugin adapters. The final bundle stage must be driven by `BundlePlan` and must
not execute Rollup, webpack, ets-loader, or hvigor-arkts-compose.

## Hvigor Inputs To Replace

Audited sources:

- `hvigor-arkts-base/dist/src/build.ts`
- `hvigor-arkts-base/dist/src/cache/rollup/**`
- `hvigor-arkts-base/dist/src/build-steps/**`
- `hvigor-arkts-compose/dist/src/arkts-pack.ts`
- `hvigor-arkts-compose/dist/src/plugins/**`
- `hvigor-ohos-plugin/src/tasks/ark-compile.ts`
- `hvigor-ohos-plugin/src/tasks/compile-node.ts`
- `hvigor-ohos-plugin/src/tasks/worker/job.ts`

## Bundle Plan Contract

`BundlePlan` must cover every observable field currently assembled by hvigor's
ArkTS compile path:

- input entries from loader/module metadata
- module root, project root, package root, output root
- build mode, target API, compatible SDK, bundle name, module name
- source roots and dependency roots
- normalized OHM URL mode
- HAR/HSP/bytecode-HAR dependency views
- supportChunks/manual chunks
- obfuscation/minify/tree-shake policy
- source map policy
- cache read/write policy and compile environment key
- code resource copy inputs
- direct/non-entry/compiled importee collection policy
- router map/share/startup/profile facts
- plugin activation order

The plan must be built by `arkdown_napi::BuildPlanBuilder` from session and
domain plans. The bundler must validate the plan before invoking Rolldown.

## Plugin Replacement Map

| hvigor/Rollup plugin | Rust replacement |
| --- | --- |
| `node-resolve/**`, `tsc-resolve.ts` | `arkdown::plugins::ohos_resolve` backed by `arkdown-resolver` |
| `add-packagejson-meta-plugin.ts` | `arkdown::plugins::module_meta` backed by dependency graph |
| `module-meta-info-plugin.ts` | `arkdown::plugins::module_meta` facts |
| `import-check-plugin.ts` | `arkdown::plugins::ohos_resolve` diagnostics using typed dependency graph |
| `check-duplicate-filenames-plugin.ts` | `arkdown::plugins::duplicate_check` |
| `router-map-plugin.ts` | `arkdown::plugins::ets_transform` consuming `arkdown-profile` router inventory |
| `use-new-ohm-url-plugin.ts` | `arkdown::plugins::ohos_resolve` and `OhmUrl` serializer |
| `wrapper-chunk-plugin.ts` | `arkdown::plugins::wrapper_chunk` render-chunk adapter with OXC parse/codegen or span patches |
| `invalid-cache-plugin.ts` | `arkdown-cache` input fingerprint set consumed by `arkdown` |
| `collect-importers-plugin.ts` | `arkdown` bundle fact collection keyed by module/source IDs |
| `copy-plugin.ts`, `delete-plugin.ts` | resource/package artifact planners, not bundler hot path |
| `terser-plugin.ts` | OXC/Rolldown minifier policy |
| `use-sensitive-check-plugin.ts` | OXC checker rule or delete if unsupported by active parity scope |
| `gc-plugin.ts`, `wait-async-plugin.ts` | delete; Rust ownership makes them unnecessary |

Plugin adapters live inside `arkdown/src/plugins/`. They must not read files in
hot hooks. They consume source code provided by Rolldown, plan data, or
source-store entries.

## Compile Node Deletion

`CompileNode` and all webpack/loader variants must be deleted from production
behavior. JS and ETS modules both flow through the Rolldown bundle stage:

```text
SourceStore
  -> Rolldown resolve/load/transform hooks
  -> OXC ArkTS/TS/JS transform
  -> Rolldown chunk graph/output
  -> importee/module metadata facts
  -> bytecode stage
```

The old Node path can remain only as an audited upstream source under
`hvigor/ts-readable`.

## Rolldown Integration Requirements

- Use workspace dependencies from the root `Cargo.toml`.
- `arkdown` may depend on `rolldown`, `rolldown_common`, and
  `rolldown_plugin` through `{ workspace = true }`.
- `arkdown` must expose a narrow `Bundler::bundle_with_plan(BundlePlan)` API.
- Plugin order must be explicit and tested.
- Hook usage must list only implemented hooks.
- Plugin outputs must be deterministic and merged by typed ID.
- Source maps must be emitted through Rolldown/OXC source-map facilities, not
  raw concatenation.
- Manual chunk decisions must be stable across thread counts.
- Cache keys must use `InputFingerprintSet`, transform options, dependency
  graph identity, and bundle plan identity.

## Importee And Metadata Outputs

The replacement for hvigor's `processImportees`, declared-file cleanup, and
bundle cache side files must produce typed artifacts:

- direct importee map
- non-entry importee map
- compiled file list
- module path map
- bytecode-HAR dependency metadata
- HSP/HAR dedup facts
- chunk/module graph facts
- source map references

Serialization must use `ArtifactPathSerializer`.

## Implementation Phases

1. Finalize `BundlePlan` fields and validation.
2. Replace all runner call sites with `Bundler::bundle_with_plan`.
3. Port resolver, OHM URL, module-meta, duplicate, import-check, HAR/HSP facts,
   router/share/startup facts, wrapper chunk, and minifier behavior to internal
   `arkdown` Rolldown adapters.
4. Remove webpack/loader compile paths from CLI production orchestration.
5. Move importee/dependency metadata generation to post-bundle fact writers.
6. Wire cache read/write through `arkdown-cache` rather than Rollup cache
   objects.
7. Delete separate `arkdown-plugin-*` crates after their final production
   callers move.
8. Add deterministic output tests for single-threaded and multi-threaded runs.

## Verification

- `cargo test -p arkdown`
- Fixture builds for `minimal-hap`, `minimal-hsp`, `minimal-har`,
  `minimal-hap-with-bc-har`, `share-config-hap`, `startup-config-hap`, and
  `release-obfuscation-hap`.
- Guardrail search:

```bash
rg -n "rollup\\(|webpack|ets-loader|hvigor-arkts-compose|@ohos/hvigor" crates
```
