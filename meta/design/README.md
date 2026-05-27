# arkdown Design Documents

Architecture and design documentation for arkdown — a Rust reimplementation of the hvigor/rollup build pipeline for OpenHarmony OS (OHOS) projects.

Read these documents before working in a covered area. Keep them in sync with implementation changes.

---

## Documents

| File                                         | Description                                                                                                                                                                                            |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [pipeline.md](pipeline.md)                   | Complete end-to-end build pipeline: all phases from config parsing through HAP signing, with hvigor behavior, rolldown equivalent, and OHOS divergences for each phase                                 |
| [hvigorw-analysis.md](hvigorw-analysis.md)   | Reverse-engineering of hvigorw: full task dependency graph, task descriptions, config file schemas, intermediate artifact layout, daemon architecture, and incremental cache strategy                  |
| [module-resolution.md](module-resolution.md) | OHOS module resolution deep dive: all specifier forms (@ohos/_, @kit/_, @bundle:, bare ohpm), oh-package.json5 dependency types, oxc-resolver integration, OHM URL normalization, SDK path conventions |
| [abc-pipeline.md](abc-pipeline.md)           | es2abc compilation step: why ABC is required, all relevant CLI flags (confirmed from installed SDK), input IIFE format, source map chain, incremental cache, worker pool parallelism                   |
| [chunk-model.md](chunk-model.md)             | OHOS chunk architecture: why ArkRuntime forbids native ESM, the IIFE/globalThis registry pattern, @Entry entry points, supportChunks flag, per-module bundle layouts (HAP/HSP/HAR), HAR deduplication  |
| [types.md](types.md)                         | Canonical shared type definitions: ResolvedId, Module, ModuleGraph, ArkChunk, ChunkGraph, IifeChunk, AbcArtifact, HapManifest, OhPackageJson5, ArkdownPlugin trait, enums                              |
| [compat.md](compat.md)                       | hvigorw compatibility layer: command mapping, hvigorw wrapper script, build-profile.json5 field mapping, plugin API differences, unsupported features, migration guide                                 |
| [plugins.md](plugins.md)                     | Complete plugin inventory: all JS plugins from hvigor-arkts-compose and hvigor-arkts-base, what each does, P0/P1/P2 priority, arkdown Rust crate replacement, complexity estimate                      |

---

## Quick Reference: Key Discoveries

**The fundamental output constraint**: ArkRuntime does not run native ESM. Every output chunk is wrapped in an IIFE, and cross-chunk dependencies use `globalThis['chunkName_hash']` as a module registry. This is implemented by `wrapper-chunk-plugin` and is the first thing to implement correctly. See [chunk-model.md](chunk-model.md).

**The compilation chain**: `.ets` source → rolldown bundle (IIFE JS) → `es2abc` → `.abc` bytecode → HAP zip. There is no way to ship a HAP without going through es2abc. See [abc-pipeline.md](abc-pipeline.md).

**Module identity**: With `useNormalizedOHMUrl: true` (the production setting), all module identifiers are OHM URLs in the format `@bundle:bundleName/moduleName/filePath`. This affects the resolver, the chunk registry keys, and the HAR/HSP dependency maps. See [module-resolution.md](module-resolution.md).

**The loader.json interface**: The `GenerateLoaderJson` task produces `loader.json` which is the contract between the hvigor task pipeline and the rollup compilation pipeline. arkdown must write this file to the same path in the same format. See [hvigorw-analysis.md](hvigorw-analysis.md).

**P0 plugins** (must implement first): `wrapper-chunk-plugin`, OHOS resolve, `module-meta-info-plugin`, `check-duplicate-filenames-plugin`, JSON5 import, commonjs. See [plugins.md](plugins.md).

---

## Design Principles

1. **Ground truth is the source code**. Every claim in these documents was verified against the actual hvigor JavaScript sources.
2. **Compatibility first**. Output artifacts must be byte-for-byte compatible with what hvigor produces (same file paths, same JSON schemas, same zip structure) so DevEco Studio and the OHOS toolchain work without modification.
3. **Replace the engine, keep the contract**. arkdown replaces Node.js/rollup with Rust/rolldown but preserves all external contracts: `loader.json` format, intermediate artifact paths, es2abc invocation, restool invocation.
4. **Port, don't wrap**. `es2abc`, `restool`, and `ets-loader` are all ported to pure Rust (`arkdown-abc`, `arkdown-resource`, `arkdown-plugin-ets-transform`). No subprocess invocation, no SDK binary dependency at build time.
