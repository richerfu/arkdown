# arkdown Design Documents

Architecture and design documentation for arkdown — an OXC/Rolldown replacement
for hvigor's JavaScript/TypeScript orchestration and Rollup build pipeline for
OpenHarmony OS (OHOS) projects.

Read these documents before working in a covered area. Keep them in sync with implementation changes.

---

## Documents

| File                                         | Description                                                                                                                                                                                            |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [pipeline.md](pipeline.md)                   | Complete end-to-end build pipeline: all phases from config parsing through HAP signing, with hvigor behavior, rolldown equivalent, and OHOS divergences for each phase                                 |
| [hvigorw-analysis.md](hvigorw-analysis.md)   | Reverse-engineering of hvigorw: full task dependency graph, task descriptions, config file schemas, intermediate artifact layout, daemon architecture, and incremental cache strategy                  |
| [hvigor-full-gap-analysis.md](hvigor-full-gap-analysis.md) | Current hvigor parity matrix across profile, resources, bundling, ABC, packaging, signing, and cache behavior                                                                                         |
| [module-resolution.md](module-resolution.md) | OHOS module resolution deep dive: all specifier forms (@ohos/_, @kit/_, @bundle:, bare ohpm), oh-package.json5 dependency types, oxc-resolver integration, OHM URL normalization, SDK path conventions |
| [abc-pipeline.md](abc-pipeline.md)           | es2abc compilation step: why ABC is required, module-mode input contract, DebugInfo coordinate source, incremental cache, worker pool parallelism                                                     |
| [chunk-model.md](chunk-model.md)             | Legacy chunk-wrapper background plus current module-mode chunk notes; current CompileArkTS output remains ESM for `es2abc --module`                                                                     |
| [types.md](types.md)                         | Canonical shared type definitions: ResolvedId, module/chunk models, artifacts, manifests, build enums, Rolldown plugin boundary, and hvigor gap metadata                                             |
| [compat.md](compat.md)                       | hvigorw compatibility layer: command mapping, hvigorw wrapper script, build-profile.json5 field mapping, plugin API differences, unsupported features, migration guide                                 |
| [plugins.md](plugins.md)                     | Rolldown-only plugin architecture, installed-IDE compile-plugin contract, Rust built-in ownership, TypeScript configuration boundary, and replacement proof                                          |
| [hvigor-full-gap-analysis.md](hvigor-full-gap-analysis.md) | Comprehensive mapping of all 95 hvigor pipeline steps against current arkdown status; implementation tasks TASK-ABC-01 through TASK-WATCH-01 with priority ordering and upstream refs |

---

## Quick Reference: Key Discoveries

**The fundamental output constraint**: ArkRuntime runs `.abc`, not raw JS. hvigor sends normal JS/TS files to `es2abc`; stage-model builds set module mode instead of requiring an IIFE entry wrapper. See [abc-pipeline.md](abc-pipeline.md).

**The compilation chain**: `.ets` source → ArkTS transform/bundle output → `es2abc --module` semantics → `.abc` bytecode → HAP zip. There is no way to ship a HAP without going through es2abc-equivalent bytecode generation. See [abc-pipeline.md](abc-pipeline.md).

**Current scope**: The first production target replaces hvigor's JS/TS task
logic, SDK JS compile plugins, and Rollup execution with Rust/OXC/Rolldown.
Native C/C++ upstream tools such as `es2abc`, `restool`, and signing tools can
remain SDK-backed until a Rust backend is explicitly selected and proven by
parity tests.

**Module identity**: With `useNormalizedOHMUrl: true` (the production setting), all module identifiers are OHM URLs in the format `@bundle:bundleName/moduleName/filePath`. This affects the resolver, the chunk registry keys, and the HAR/HSP dependency maps. See [module-resolution.md](module-resolution.md).

**The loader.json interface**: The `GenerateLoaderJson` task produces `loader.json` which is the contract between the hvigor task pipeline and the rollup compilation pipeline. arkdown must write this file to the same path in the same format. See [hvigorw-analysis.md](hvigorw-analysis.md).

**P0 plugins** (must implement first): OHOS resolve, `module-meta-info-plugin`, `check-duplicate-filenames-plugin`, JSON5 import, commonjs/ESM handling through rolldown. See [plugins.md](plugins.md).

---

## Design Principles

1. **Ground truth is the source code**. Every claim in these documents was verified against the actual hvigor JavaScript sources.
2. **Compatibility first**. Output artifacts must be byte-for-byte compatible with what hvigor produces (same file paths, same JSON schemas, same zip structure) so DevEco Studio and the OHOS toolchain work without modification.
3. **Replace the JS engine, keep the contract**. arkdown replaces hvigor's
   Node.js/Rollup path with Rust/OXC/Rolldown while preserving external
   contracts: `loader.json`, intermediate artifact paths, `es2abc` input shape,
   restool inputs, and packaging layout.
4. **Do not block on native rewrites**. If upstream behavior lives in C/C++,
   the initial path may call the SDK-native tool directly. Rust ports remain
   optional acceleration/parity projects, not blockers for replacing hvigor's
   JS/TS and Rollup bottlenecks.

---

## JS Layer Inventory (target: JS only for plugin system + external exposure)

The stated goal is that the JavaScript layer keeps only what is necessary to
host the hvigor plugin system and the externally-exposed API surface. Current
state (audited 2026-08-29):

**Necessary JS (retained):**
- `packages/arkdown/js/runtime.js` callback facade — hosts `hvigorfile.ts` plugin objects, dispatches
  `HvigorPlugin` lifecycle callbacks, and binds the values formally declared by
  `@ohos/hvigor/index.d.ts`. The existing Arkdown custom-task contract retains
  a narrow allowlist for task subclasses/decorators, core-node access, task
  state, and tooling-model callbacks. Filesystem behavior, task metadata and
  hooks, parameters, reports, and Worker scheduling are Rust-owned; JavaScript
  owns only live plugin/callback identity, Proxy semantics, Buffer conversion,
  and `worker_threads` message transport. Undeclared root leaks such as log4js,
  metrics, TraceManager, internal caches, DAGs, and direct/watch workers are not
  reimplemented.
- Entry/CLI/worker V8 host files required to boot the N-API bridge. All
  handwritten production JavaScript for the Node package is colocated under
  `packages/arkdown/js/`; generated N-API access files remain at the package
  root, while tests and development scripts stay in their purpose-specific
  directories.

**Known gaps (on the production critical path, awaiting a Rust port):**
- `packages/arkdown/js/ets-checker-bridge.cjs` — SDK etsChecker semantic check, runs in every build.
- `arkguard_bridge.cjs` — ArkGuard source obfuscation, release builds.
- `bytecode_arkguard_bridge.cjs` — BytecodeObfuscator, release builds.

These bridges exist solely because the corresponding SDK JS tooling has not been
ported to Rust; they are not part of the plugin system or the external API
surface and should shrink to zero as Rust implementations land.
