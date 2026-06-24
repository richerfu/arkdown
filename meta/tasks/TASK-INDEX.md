# Active Task Index: OXC/Rolldown Hvigor Replacement

This directory is the active implementation contract for replacing hvigor's
JavaScript/TypeScript build orchestration with arkdown's Rust pipeline.

Old task specs were deleted intentionally. They were treated as historical
reference only and must not be revived as compatibility layers.

## Active Specs

0. [SPEC-000 Crate Topology And Build Flow](SPEC-000-crate-topology-and-build-flow.md)
1. [SPEC-001 Architecture Boundary](SPEC-001-oxc-rolldown-hvigor-replacement-architecture.md)
2. [SPEC-002 Session And Manual Orchestration](SPEC-002-session-stage-graph.md)
3. [SPEC-003 Rolldown Bundler And Plugins](SPEC-003-rolldown-bundler-and-plugins.md)
4. [SPEC-004 OXC ArkTS Transform](SPEC-004-oxc-arkts-transform.md)
5. [SPEC-005 External Tools And Artifacts](SPEC-005-external-tools-and-artifacts.md)
6. [SPEC-006 Cache Watch Verification And Deletion](SPEC-006-cache-watch-verification-deletion.md)
7. [SPEC-007 Final File Layout And Implementation Map](SPEC-007-final-file-layout-implementation-map.md)

## Final Direction

arkdown must replace hvigor's task runtime, Rollup/webpack compile pipeline,
SDK compile-plugin JavaScript, and hvigor plugin adapters with Rust ownership:

- `arkdown-session` owns startup discovery, immutable snapshots, dependency
  graphs, profile/resource/source inventories, SDK metadata, and fingerprints.
- `packages/arkdown` owns CLI parsing, coarse stage orchestration,
  external tool execution, and user-visible diagnostics.
- `arkdown` owns the Rolldown bundling stage and must not depend on concrete
  plugin crates or `arkdown-session`.
- `arkdown-ets-transform` owns OXC parsing, ArkTS transforms, diagnostics,
  source maps, codegen, checker behavior, and transform facts.
- Separate `arkdown-plugin-*` crates are not part of the final design; built-in
  Rolldown adapters live inside `arkdown`.
- Profile, resource, pack, sign, cache, resolver, OHPM, TS2ABC, and watcher
  crates own their domains behind typed plan APIs.

## Allowed External Tool Boundary

The following may remain process-backed until a separate Rust backend is
explicitly selected:

- SDK `es2abc` / Ark bytecode compiler.
- SDK `restool` / resource compiler.
- SDK package tools used by HAP/HSP/HAR/archive creation.
- SDK signing tools and signature verification tools.
- Native C/C++ tools: CMake, Ninja, LLVM strip/binxo, and related SDK binaries.
- Device/test runners that are inherently external.

Every other hvigor responsibility must move to OXC, Rolldown, or typed Rust
domain crates.

## Non-Negotiable Rules

- Do not execute hvigor's JavaScript task runtime in production.
- Do not execute Rollup, webpack, ets-loader, or hvigor-arkts-compose in
  production.
- Do not keep old compatibility wrappers once the final production caller is
  migrated.
- Do not read project manifests, package manifests, profiles, SDK metadata, or
  dependency roots from plugin hooks or hot transforms.
- Do not serialize artifact paths from ad hoc strings.
- Do not store OXC allocator-backed AST data in sessions, caches, or long-lived
  build state.
- Do not use untyped string paths for dependency graph identity, cache identity,
  resolver matching, or artifact serialization.

## Source Audit Basis

The current `hvigor/ts-readable` tree was audited as follows:

- `hvigor/src/base/**`: task runtime, plugin registration, lifecycle, daemon,
  logging, CLI/config loading.
- `hvigor-arkts-base/dist/src/build.ts`: Rollup execution, watch, bundle cache,
  importee processing, declared-file cleanup.
- `hvigor-arkts-compose/dist/src/plugins/**`: Rollup plugins for resolve,
  module metadata, duplicate checks, import checks, router map, OHM URL,
  wrapper chunks, cache invalidation, copy/delete, and HAR/HSP metadata.
- `hvigor-ohos-plugin/src/tasks/**`: OHOS task graph, ArkTS compile, node
  loader compile, profile/resource/pack/sign/native/test/preview/reload tasks.

This audit informs the replacement plan only. The JS implementation is not an
active implementation dependency.

## Phase Order

1. Establish hard production boundaries and delete hvigor runtime entry points.
2. Make `BuildSession` the only source of stable build inputs.
3. Replace hvigor task creation with coarse manual orchestration.
4. Replace Rollup/webpack compile with Rolldown plus arkdown plugin adapters.
5. Move ArkTS transforms, checker, and codegen to OXC-backed domain APIs.
6. Keep external tool stages process-backed behind typed runners.
7. Harden cache/watch/determinism and delete legacy names and fallback paths.

## Required Final Verification

Run these before declaring the replacement complete:

```bash
cargo metadata --format-version 1 --no-deps
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Also run fixture parity builds for HAP, HSP, HAR, bytecode-HAR dependency,
release obfuscation, share config, startup config, preview/reload-capable
fixtures, and native-resource fixtures when those stages are touched.
