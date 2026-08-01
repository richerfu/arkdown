# SPEC-007 Final File Layout And Implementation Map

## Goal

Define the concrete file layout for the coarse `BuildCoordinator` +
Rolldown/OXC architecture. This document is the implementation map for moving
from the current hvigor/Rollup compatibility structure to the final crate
topology.

The final design must not keep separate `arkdown-plugin-*` crates. Rollup
plugin behavior migrates into internal Rolldown plugin adapters under
`crates/arkdown/src/plugins/`. Non-hook business logic moves to the owning
domain crate.

## Workspace Root

```text
Cargo.toml
crates/
  arkdown-core/
  arkdown-path/
  arkdown-ohos-schema/
  arkdown-config/
  arkdown-ohpm/
  arkdown-session/
  arkdown-resolver/
  arkdown-ets-transform/
  arkdown/
  arkdown-profile/
  arkdown-resource/
  arkdown-pack/
  arkdown-sign/
  arkdown-obfuscate/
  arkdown-cache/
  arkdown-watcher/
  packages/arkdown/
```

Delete from workspace after migration:

```text
crates/arkdown-plugin-json5/
crates/arkdown-plugin-duplicate-check/
crates/arkdown-plugin-module-meta/
crates/arkdown-plugin-ohos-resolve/
crates/arkdown-plugin-har-dedup/
crates/arkdown-plugin-ets-transform/
```

`Cargo.toml` must keep all third-party versions centralized under
`[workspace.dependencies]`. Crate manifests under `crates/*` must use
`{ workspace = true }`.

## `arkdown-core`

```text
crates/arkdown-core/src/
  lib.rs
  ids.rs
  diagnostics.rs
  target.rs
  artifact.rs
  error.rs
```

- `lib.rs`: narrow module declarations and re-exports only.
- `ids.rs`: typed IDs shared across crates, such as `ModuleId`, `PackageId`,
  `SourceFileId`, `ProfileFileId`, `ResourceEntryId`, `PackEntryId`,
  `ToolInvocationId`. Use `oxc_index`-compatible typed IDs.
- `diagnostics.rs`: domain-neutral diagnostic structs, severity, source span,
  stage labels, and deterministic diagnostic codes.
- `target.rs`: build mode, output target, module type, test/preview/reload mode
  enums.
- `artifact.rs`: domain-neutral artifact contracts and artifact kind enums. Do
  not serialize slash paths here.
- `error.rs`: typed shared errors. `anyhow` is forbidden here.

## `arkdown-path`

```text
crates/arkdown-path/src/
  lib.rs
  abs.rs
  rel.rs
  artifact.rs
  specifier.rs
  ohm.rs
  normalize.rs
  classify.rs
  display.rs
  serializer.rs
```

- `abs.rs`: `AbsPath` and existing-path canonicalization boundaries.
- `rel.rs`: `RelPath` and lexical relative path validation.
- `artifact.rs`: `ArtifactPath` and archive/artifact path type.
- `specifier.rs`: `ModuleSpecifier` for import/export text.
- `ohm.rs`: `OhmUrl` parsing and construction.
- `normalize.rs`: lexical normalization for output paths.
- `classify.rs`: component-based `oh_modules`, package, source root, and SDK
  path classification.
- `display.rs`: `PathDisplay` for diagnostics only.
- `serializer.rs`: `ArtifactPathSerializer`, the only slash-path serializer.

## `arkdown-ohos-schema`

```text
crates/arkdown-ohos-schema/src/
  lib.rs
  app.rs
  module.rs
  build_profile.rs
  oh_package.rs
  hvigor_config.rs
  router.rs
  startup.rs
  share.rs
  privacy.rs
  resource.rs
  json5.rs
```

- Schema files define serde-compatible structs for OHOS JSON/JSON5 inputs and
  generated outputs.
- `json5.rs` owns JSON5 parse helpers returning typed schema errors.
- No build orchestration, filesystem walking, SDK lookup, or cache behavior is
  allowed here.

## `arkdown-config`

```text
crates/arkdown-config/src/
  lib.rs
  loader.rs
  product.rs
  target_select.rs
  build_profile.rs
```

- `loader.rs`: compatibility facade for loading root project config into typed
  schema/domain values. Long term, keep this thin.
- `product.rs`: product/flavor selection helpers.
- `target_select.rs`: selected target logic for CLI flags, product, and build
  mode.
- `build_profile.rs`: build-profile parse helpers that still need compatibility
  naming.

If these modules stop owning standalone behavior, fold them into
`arkdown-session` and delete `arkdown-config`.

## `arkdown-ohpm`

```text
crates/arkdown-ohpm/src/
  lib.rs
  lockfile.rs
  package_json.rs
  cache.rs
  registry.rs
  integrity.rs
  tarball.rs
  acquire.rs
  error.rs
```

- `lockfile.rs`: parse and expose OHPM lockfile data.
- `package_json.rs`: package manifest parsing for dependency acquisition.
- `cache.rs`: global/local OHPM cache layout and lookup.
- `registry.rs`: registry metadata fetch, offline/prefer-offline policy.
- `integrity.rs`: integrity verification.
- `tarball.rs`: safe deterministic `.tgz` extraction.
- `acquire.rs`: `PackageAcquisitionPlanner` execution helpers.
- `error.rs`: typed OHPM errors.

## `arkdown-session`

```text
crates/arkdown-session/src/
  lib.rs
  builder.rs
  session.rs
  context.rs
  project/
    mod.rs
    snapshot.rs
  module/
    mod.rs
    snapshot.rs
  package/
    mod.rs
    snapshot.rs
  dependency_graph/
    mod.rs
    builder.rs
    snapshot.rs
    views.rs
  sdk/
    mod.rs
    snapshot.rs
    tools.rs
    schemas.rs
    compile_plugin.rs
  profiles/
    mod.rs
    store.rs
    inventory.rs
  resources/
    mod.rs
    inventory.rs
  source_store/
    mod.rs
    inventory.rs
    content.rs
    fingerprint.rs
  native/
    mod.rs
    inventory.rs
  build_script/
    mod.rs
    snapshot.rs
    parse.rs
    effects.rs
  artifacts/
    mod.rs
    layout.rs
  fingerprint/
    mod.rs
    input_set.rs
  plan_input/
    mod.rs
    profile.rs
    resource.rs
    bundle.rs
    bytecode.rs
    pack.rs
    sign.rs
    native.rs
    watch.rs
  paths.rs
  diagnostics.rs
```

- `builder.rs`: `BuildSessionBuilder`; owns all startup IO.
- `session.rs`: immutable `BuildSession`.
- `context.rs`: `ModuleCompileContext` and narrow views consumed by
  `packages/arkdown`.
- `project/snapshot.rs`: selected project root/config, app metadata, product,
  signing inputs.
- `module/snapshot.rs`: module roots, source/resource/profile refs, target
  data, module JSON, package metadata.
- `dependency_graph/*`: one dependency graph per build/session scope; backs
  resolver, profile, HAR/HSP, bytecode-HAR, native SO, cache, and watch.
- `sdk/*`: SDK tool/schema/compile-plugin snapshots, no repeated SDK reads.
- `profiles/*`: profile file inventory and parsed profile store.
- `resources/*`: resource root inventory; no binary contents retained.
- `source_store/*`: source inventory, optional content cache, fingerprints.
- `native/*`: C/C++ and native library inventory.
- `build_script/*`: OXC-based declarative `hvigorfile.ts`/`hvigorconfig.ts`
  parser and supported effects. This replaces public `HvigorScript*` and
  `HvigorTaskGraph*` types.
- `artifacts/layout.rs`: typed output layout.
- `fingerprint/input_set.rs`: content fingerprint set.
- `plan_input/*`: narrow input structs used by `arkdown_napi::BuildPlanBuilder`.
- `paths.rs`: session-facing `PathResolver` wrapper around `arkdown-path`.
- `diagnostics.rs`: session diagnostics.

Delete or rename existing public `hvigor_script.rs` and `hvigor_task_graph.rs`
after `build_script/*` is in place.

## `arkdown-resolver`

```text
crates/arkdown-resolver/src/
  lib.rs
  plan.rs
  resolver.rs
  package_index.rs
  exports.rs
  oh_modules.rs
  sdk.rs
  ohm.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: resolver input plan derived from session.
- `resolver.rs`: main OHOS-aware resolver.
- `package_index.rs`: package/dependency index lookup by typed IDs.
- `exports.rs`: package exports/imports rules.
- `oh_modules.rs`: component-based `oh_modules` resolution.
- `sdk.rs`: SDK declaration and SDK module resolution.
- `ohm.rs`: OHM URL construction helpers using `arkdown-path`.
- `diagnostics.rs`: import/resolution diagnostics.
- `error.rs`: typed resolver errors.

No resolver hot path may walk upward to rediscover package roots.

## `arkdown-ets-transform`

```text
crates/arkdown-ets-transform/src/
  lib.rs
  error.rs
  hvigor_format.rs
  kit_map.rs
  lazy_import.rs
  module_emit.rs
  module_name.rs
  module_rewrite.rs
  resource_transform.rs
  rolldown_plan.rs
  source.rs
  source_map.rs
  sys_resource.rs
  typescript_emit.rs
  checker/
    mod.rs
    rules.rs
  transform/
    mod.rs
    syntax.rs
    decorator.rs
    component.rs
    component_analysis.rs
    component_collection.rs
    component_field.rs
    component_meta.rs
    build_fn/
      custom_component.rs
      iteration.rs
      raw_object.rs
      styles.rs
    build_fn.rs
    viewpu_emit.rs
    viewpu_finalize.rs
    global_rewrite.rs
    source_emit.rs
    source_text.rs
  api_transform/
    mod.rs
    version_table.rs
```

- `rolldown_plan.rs`: narrow transform plan and transformer consumed by the
  Rolldown adapter.
- `source.rs`: source classification, module specifier text helpers, and line
  index support.
- `source_map.rs`: source-map builders used by the Rolldown transform output.
- `checker/*`: ArkTS checker and lint parity.
- `transform/*`: ArkTS/component/decorator/ViewPU/build-function transforms.
- `api_transform/*`: API compatibility transforms.
- `lazy_import.rs`, `module_rewrite.rs`, `typescript_emit.rs`: AST/codegen
  import and TypeScript emit support for the active transform path.
- `resource_transform.rs`, `sys_resource.rs`: resource and SDK system resource
  rewrites.

Move all production code from `arkdown-plugin-ets-transform` here. The old
plugin crate becomes a temporary migration source and must be deleted.

## `arkdown`

```text
crates/arkdown/src/
  lib.rs
  bundler.rs
  plan.rs
  output.rs
  diagnostics.rs
  compile_entry.rs
  compile_env.rs
  manual_chunks.rs
  cache_bridge.rs
  watch_config.rs
  facts.rs
  plugins/
    mod.rs
    factory.rs
    json5.rs
    duplicate_check.rs
    module_meta.rs
    ohos_resolve.rs
    har_dedup.rs
    ets_transform.rs
    wrapper_chunk.rs
    import_check.rs
    minify.rs
```

- `lib.rs`: exports `Bundler`, `BundlePlan`, `BundleOutput`, and minimal
  diagnostics.
- `bundler.rs`: `Bundler::bundle_with_plan(BundlePlan)`. Creates Rolldown
  options and delegates graph/plugin/chunk lifecycle to Rolldown.
- `plan.rs`: `BundlePlan`, bundle input/output/cache/source-map/plugin
  activation structs.
- `output.rs`: `BundleOutput`, emitted files, bundle facts, source maps.
- `diagnostics.rs`: bundle diagnostics.
- `compile_entry.rs`: compile entry construction from plan data.
- `compile_env.rs`: compile environment key used for cache identity.
- `manual_chunks.rs`: stable manual chunk policy.
- `cache_bridge.rs`: bridge between `BundlePlan`/`arkdown-cache` and Rolldown
  cache inputs/outputs. Must not depend on Rollup cache object shape.
- `watch_config.rs`: bundle watch configuration inputs.
- `facts.rs`: importee maps, module path maps, chunk facts, HAR/HSP facts.
- `plugins/mod.rs`: internal adapter module declarations.
- `plugins/factory.rs`: builds the fixed Rolldown adapter list from
  `BundlePlan`; it may add thin internal plugins only for observed hvigor hook
  boundaries absent from upstream Rolldown.
- `plugins/json5.rs`: JSON5 load/transform adapter.
- `plugins/duplicate_check.rs`: duplicate filename checks.
- `plugins/module_meta.rs`: package/module metadata facts.
- `plugins/ohos_resolve.rs`: resolve adapter backed by `arkdown-resolver`.
- `plugins/har_dedup.rs`: HAR/HSP dedup facts.
- `plugins/ets_transform.rs`: transform adapter backed by
  `arkdown-ets-transform`.
- `plugins/wrapper_chunk.rs`: migrated `wrapper-chunk-plugin` behavior using
  OXC parse/codegen or deterministic span patches.
- `plugins/import_check.rs`: import diagnostics that are naturally hook-bound.
- `plugins/minify.rs`: OXC/Rolldown minifier policy.

Do not create a separate public plugin API for users in this phase.

## Rollup Plugin Migration Map

| Upstream Rollup plugin | Final file |
| --- | --- |
| `node-resolve/**` | `crates/arkdown/src/plugins/ohos_resolve.rs` plus `arkdown-resolver` |
| `tsc-resolve.ts` | `crates/arkdown/src/plugins/ohos_resolve.rs` |
| `add-packagejson-meta-plugin.ts` | `crates/arkdown/src/plugins/module_meta.rs` |
| `module-meta-info-plugin.ts` | `crates/arkdown/src/plugins/module_meta.rs` |
| `check-duplicate-filenames-plugin.ts` | `crates/arkdown/src/plugins/duplicate_check.rs` |
| `import-check-plugin.ts` | `crates/arkdown/src/plugins/import_check.rs` |
| `router-map-plugin.ts` | `crates/arkdown/src/plugins/ets_transform.rs` plus `arkdown-profile` |
| `use-new-ohm-url-plugin.ts` | `crates/arkdown/src/plugins/ohos_resolve.rs` plus `arkdown-path::OhmUrl` |
| `wrapper-chunk-plugin.ts` | `crates/arkdown/src/plugins/wrapper_chunk.rs` |
| `terser-plugin.ts` | `crates/arkdown/src/plugins/minify.rs` |
| `collect-importers-plugin.ts` | `crates/arkdown/src/facts.rs` and post-bundle fact writer |
| `invalid-cache-plugin.ts` | `arkdown-cache` and `crates/arkdown/src/cache_bridge.rs` |
| `copy-plugin.ts` | `arkdown-resource` or `arkdown-pack` artifact plans |
| `delete-plugin.ts` | `arkdown-resource` or `arkdown-pack` cleanup plans |
| `gc-plugin.ts` | delete |
| `wait-async-plugin.ts` | delete |
| `use-sensitive-check-plugin.ts` | `arkdown-ets-transform/checker` or delete if outside parity scope |

## `arkdown-profile`

```text
crates/arkdown-profile/src/
  lib.rs
  plan.rs
  processor.rs
  module_json.rs
  build_profile.rs
  router.rs
  startup.rs
  share.rs
  privacy.rs
  pac.rs
  permissions.rs
  bytecode_har.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: `ProfilePlan`.
- `processor.rs`: orchestrates profile-domain processors only.
- `module_json.rs`: generated module JSON.
- `build_profile.rs`: generated build profile.
- `router.rs`, `startup.rs`, `share.rs`, `privacy.rs`, `pac.rs`: owning
  processors for those profile artifacts.
- `permissions.rs`: permission validation and reason checks.
- `bytecode_har.rs`: bytecode-HAR profile metadata inputs.
- `diagnostics.rs`, `error.rs`: typed diagnostics/errors.

## `arkdown-resource`

```text
crates/arkdown-resource/src/
  lib.rs
  plan.rs
  runner.rs
  command.rs
  inventory.rs
  copy.rs
  preview.rs
  output.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: `ResourcePlan`.
- `runner.rs`: resource stage runner.
- `command.rs`: `restool` command plan. No string-derived identity.
- `inventory.rs`: resource entries consumed from session.
- `copy.rs`: rawfile/code-resource copy plans.
- `preview.rs`: preview-specific resource variants.
- `output.rs`: compiled resource output inventory.

## `packages/arkdown::cmd::build::bytecode`

```text
packages/arkdown/src/cmd/build/bytecode/
  mod.rs
  plan.rs
  compiler.rs
  backend.rs
  driver.rs
  file_info.rs
  compile_context.rs
  error.rs
```

- `plan.rs`: bytecode compile plan and backend choice.
- `compiler.rs`: high-level bytecode compiler facade.
- `backend.rs`: SDK/Rust backend selection.
- `driver.rs`: SDK `es2abc` command construction and process runner.
- `file_info.rs`: file list and source metadata.
- `cache_file.rs`: cache-file/protoBin handling.
- `output.rs`: ABC output inventory.

## `arkdown-pack`

```text
crates/arkdown-pack/src/
  lib.rs
  plan.rs
  packager.rs
  entries.rs
  archive.rs
  har.rs
  hsp.rs
  hap.rs
  app.rs
  integrated_hsp.rs
  native_libs.rs
  bytecode_har.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: package plan.
- `packager.rs`: orchestrates package-domain work.
- `entries.rs`: deterministic pack entry discovery.
- `archive.rs`: archive writer or SDK package command boundary.
- `har.rs`, `hsp.rs`, `hap.rs`, `app.rs`: format-specific package logic.
- `integrated_hsp.rs`: integrated HSP collection.
- `native_libs.rs`: native SO/preload entries.
- `bytecode_har.rs`: bytecode-HAR package metadata.

## `arkdown-sign`

```text
crates/arkdown-sign/src/
  lib.rs
  plan.rs
  runner.rs
  command.rs
  verify.rs
  remote_hsp.rs
  output.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: sign plan and key/profile inputs.
- `runner.rs`: sign stage runner.
- `command.rs`: SDK signing command plan.
- `verify.rs`: verification plan/runner.
- `remote_hsp.rs`: remote HSP signing variants.
- `output.rs`: signed artifact inventory.

## `arkdown-obfuscate`

```text
crates/arkdown-obfuscate/src/
  lib.rs
  plan.rs
  rules.rs
  merge.rs
  facts.rs
  mapping.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: obfuscation plan.
- `rules.rs`: rule parsing.
- `merge.rs`: deterministic rule merge.
- `facts.rs`: facts consumed by transform/bundle/bytecode.
- `mapping.rs`: mapping output.

## `arkdown-cache`

```text
crates/arkdown-cache/src/
  lib.rs
  key.rs
  fingerprint.rs
  store.rs
  records.rs
  bundle.rs
  bytecode.rs
  resource.rs
  package.rs
  diagnostics.rs
  error.rs
```

- `key.rs`: typed cache keys.
- `fingerprint.rs`: fingerprint helpers.
- `store.rs`: cache store operations.
- `records.rs`: persisted record schemas.
- `bundle.rs`, `bytecode.rs`, `resource.rs`, `package.rs`: domain cache
  helpers.

## `arkdown-watcher`

```text
crates/arkdown-watcher/src/
  lib.rs
  plan.rs
  watcher.rs
  invalidation.rs
  event.rs
  diagnostics.rs
  error.rs
```

- `plan.rs`: watch plan from session inventories.
- `watcher.rs`: filesystem watcher integration.
- `invalidation.rs`: maps file events to typed affected IDs.
- `event.rs`: normalized watch event model.

## `packages/arkdown`

```text
packages/arkdown/src/
  main.rs
  lib.rs
  progress.rs
  cmd/
    mod.rs
    build.rs
    check.rs
    clean.rs
    dump_config.rs
    native_build.rs
    build/
      mod.rs
      args.rs
      coordinator.rs
      build_plan.rs
      module_plan.rs
      bundle_plan.rs
      plugin_factory.rs
      external_tool.rs
      resource_stage.rs
      profile_stage.rs
      bundle_stage.rs
      bytecode_stage.rs
      package_stage.rs
      sign_stage.rs
      native_stage.rs
      watch_stage.rs
      custom_config.rs
      parity_report.rs
      intermediates.rs
      generate_har_declarations.rs
      coverage_instrument.rs
```

- `cmd/build.rs`: CLI command entry and args handoff only.
- `cmd/build/args.rs`: build args and CLI compatibility flags.
- `cmd/build/coordinator.rs`: `BuildCoordinator`; explicit coarse sequencing.
- `cmd/build/build_plan.rs`: `BuildPlanBuilder`; converts session plan inputs
  into domain plans.
- `cmd/build/module_plan.rs`: module selection and dependency wave planning.
- `cmd/build/bundle_plan.rs`: CLI-side conversion to `arkdown::BundlePlan`.
- `cmd/build/plugin_factory.rs`: temporary compatibility shim only while moving
  adapters into `arkdown`; delete when `arkdown` owns adapter creation.
- `cmd/build/external_tool.rs`: shared bounded Tokio process runner for real
  external tools.
- `cmd/build/resource_stage.rs`: calls `arkdown-resource`.
- `cmd/build/profile_stage.rs`: calls `arkdown-profile`.
- `cmd/build/bundle_stage.rs`: calls `Bundler::bundle_with_plan`.
- `cmd/build/bytecode_stage.rs`: calls `cmd::build::bytecode`.
- `cmd/build/package_stage.rs`: calls `arkdown-pack`.
- `cmd/build/sign_stage.rs`: calls `arkdown-sign`.
- `cmd/build/native_stage.rs`: native C/C++/SO stage integration.
- `cmd/build/watch_stage.rs`: watch mode coordinator integration.
- `cmd/build/custom_config.rs`: JSON config compatibility only.
- `cmd/build/parity_report.rs`: diagnostics for unsupported hvigor behavior.
- `cmd/build/intermediates.rs`: intermediate artifact writer.
- `cmd/build/generate_har_declarations.rs`: HAR declaration generation.
- `cmd/build/coverage_instrument.rs`: coverage mode preparation.

Delete from CLI production path:

```text
hvigor_hooks.rs
hvigor_tasks.rs
hvigorfile_runtime.rs
pipeline.rs
stages.rs
plugins.rs
plugin_order.rs
compile_arkts_*.rs
compile_env.rs
reload.rs
preview.rs
test.rs
```

Reintroduce behavior from those files only through `BuildCoordinator`, domain
plans, or `arkdown` Rolldown adapters.

## Final Guardrails

Run these after each migration slice:

```bash
rg -n "arkdown-plugin-" Cargo.toml crates
rg -n "hvigor_hooks|hvigor_tasks|hvigorfile_runtime|CompileNode|webpack|ets-loader|@ohos/hvigor" crates
rg -n "BuildStageGraph|TaskNode|dependsOn|taskName" packages/arkdown crates/arkdown-session
cargo metadata --format-version 1 --no-deps
```

Allowed matches are only:

- this plan or audit-only docs;
- tests intentionally checking unsupported hvigor diagnostics;
- migration comments in code scheduled for deletion in the same slice.
