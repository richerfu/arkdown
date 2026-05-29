# arkdown Task Index

Prioritized implementation tasks for the arkdown bundler.

- **P0** — Required for any HAP to compile at all. Nothing ships without these.
- **P1** — Required for production/release-quality builds.
- **P2** — Advanced features (hot-reload, HAR publishing, preview mode).

Each SPEC file contains three standard sections:

- **Public API** — the exact Rust API surface to implement
- **Rolldown Alignment** — maps arkdown types/hooks to rolldown equivalents; notes intentional OHOS divergences
- **Test Requirements** — concrete, specific test cases for the crate

---

## P0 — Minimum Viable HAP Build

| ID        | Title                                   | Crate                            | Blocked By         |
| --------- | --------------------------------------- | -------------------------------- | ------------------ |
| SPEC-001  | Workspace + Core Types                  | `arkdown-core`                   | —                  |
| SPEC-002  | Config Parser                           | `arkdown-config`                 | SPEC-001           |
| SPEC-003  | OHOS Module Resolver                    | `arkdown-resolver`               | SPEC-001           |
| SPEC-004  | Module Loader + Plugin Pipeline         | `arkdown-loader`                 | SPEC-001, 003      |
| SPEC-005  | Bundler Core (rolldown integration)     | `arkdown`                        | SPEC-001, 003, 004 |
| SPEC-006  | Chunk Wrapper Plugin (IIFE)             | `arkdown-plugin-chunk-wrapper`   | SPEC-005           |
| SPEC-007  | JSON5 Import Plugin                     | `arkdown-plugin-json5`           | SPEC-005           |
| SPEC-008  | Duplicate Filename Check Plugin         | `arkdown-plugin-duplicate-check` | SPEC-005           |
| SPEC-009  | ArkCompiler Bytecode Compiler (Rust)    | `arkdown-abc`                    | SPEC-005, 006      |
| SPEC-010  | Profile Processor (module.json5)        | `arkdown-profile`                | SPEC-002           |
| SPEC-011  | Resource Compiler (Rust, ports restool) | `arkdown-resource`               | SPEC-002           |
| SPEC-012  | HAP/HSP/HAR Packager                    | `arkdown-pack`                   | SPEC-009, 010, 011 |
| SPEC-013  | CLI (`arkdown build`)                   | `arkdown-cli`                    | SPEC-012           |
| SPEC-004a | ArkTS Transform (pure Rust ets-loader)  | `arkdown-plugin-ets-transform`   | SPEC-001, 004      |

## P1 — Production Builds

| ID       | Title                         | Crate                            | Blocked By    |
| -------- | ----------------------------- | -------------------------------- | ------------- |
| SPEC-014 | Module Meta Plugin            | `arkdown-plugin-module-meta`     | SPEC-005      |
| SPEC-015 | Full OHOS Resolve Plugin      | `arkdown-plugin-ohos-resolve`    | SPEC-003, 014 |
| SPEC-016 | HAR Deduplication Plugin      | `arkdown-plugin-har-dedup`       | SPEC-005, 014 |
| SPEC-017 | Router Map Plugin             | `arkdown-plugin-router-map`      | SPEC-005      |
| SPEC-018 | Case-Sensitive Check Plugin   | `arkdown-plugin-sensitive-check` | SPEC-005      |
| SPEC-019 | Incremental Build Cache       | `arkdown-cache`                  | SPEC-005      |
| SPEC-020 | Obfuscation Pipeline          | `arkdown-obfuscate`              | SPEC-009      |
| SPEC-021 | HAP Signing                   | `arkdown-sign`                   | SPEC-012      |
| SPEC-022 | hvigorw Compatibility Wrapper | `arkdown-cli` (extension)        | SPEC-013      |

## P1 — Production Builds (continued)

| ID       | Title                         | Crate           | Blocked By |
| -------- | ----------------------------- | --------------- | ---------- |
| SPEC-044 | Integrated HSP Processing     | `arkdown-pack`  | SPEC-012   |

## P2 — Advanced Features

### IDE Integration (watch, hot-reload, preview)

| ID       | Title                            | Crate                          | Blocked By |
| -------- | -------------------------------- | ------------------------------ | ---------- |
| SPEC-023 | Watch Mode                       | `arkdown-watcher`              | SPEC-013   |
| SPEC-024 | Hot-Reload Importer Graph Plugin | `arkdown-plugin-importers`     | SPEC-005   |

### ArkTS Compiler Correctness

| ID       | Title                              | Crate                          | Blocked By        |
| -------- | ---------------------------------- | ------------------------------ | ----------------- |
| SPEC-025 | ABC Incremental Cache (per-fn SHA) | `arkdown-abc`, `arkdown-cache` | SPEC-009, SPEC-039 |
| SPEC-026 | Widget Compilation Pass            | `arkdown-abc`, `arkdown-cli`   | SPEC-009          |
| SPEC-027 | API Transform Pass (apiTransform)  | `arkdown-plugin-ets-transform` | SPEC-004a         |
| SPEC-028 | ABC Structural Parity Fixes        | `arkdown-abc`                  | SPEC-009          |
| SPEC-029 | ViewPU Build() Body Lowering       | `arkdown-plugin-ets-transform` | SPEC-004a         |
| SPEC-030 | @Builder and @Styles Decorators    | `arkdown-plugin-ets-transform` | SPEC-004a         |

### Release Build Quality

| ID       | Title                               | Crate                                  | Blocked By          |
| -------- | ----------------------------------- | -------------------------------------- | ------------------- |
| SPEC-034 | Release Tree-shake and ManualChunks | `arkdown`, `arkdown-cli`               | SPEC-005            |
| SPEC-038 | Obfuscation Pipeline Wiring         | `arkdown-obfuscate`, `arkdown-cli`     | SPEC-020, SPEC-009  |
| SPEC-039 | Rolldown compileEnv-Keyed Cache     | `arkdown-cache`, `arkdown`             | SPEC-005, SPEC-019  |

### Multi-Module and Advanced Packaging

| ID       | Title                              | Crate                                     | Blocked By          |
| -------- | ---------------------------------- | ----------------------------------------- | ------------------- |
| SPEC-040 | Task-Level Incremental Skip        | `arkdown-cli`                             | SPEC-039, SPEC-038  |
| SPEC-041 | ByteCode HAR Dependency Map        | `arkdown-profile`, `arkdown-resolver`     | SPEC-003, SPEC-010  |
| SPEC-042 | Dynamic Import Library Info        | `arkdown-profile`, `arkdown`              | SPEC-041, SPEC-005  |
| SPEC-043 | Preloaded SO File List             | `arkdown-pack`, `arkdown-cli`             | SPEC-012            |
| SPEC-044 | Integrated HSP Processing          | `arkdown-pack`                            | SPEC-012            |
| SPEC-045 | Privacy Profile Embedding          | `arkdown-pack`                            | SPEC-012            |
| SPEC-046 | CrossAppSharedConfig + StartupTask | `arkdown-profile`, `arkdown-resolver`     | SPEC-010, SPEC-003  |
| SPEC-047 | Watch Mode with Incremental Rebuild | `arkdown-watcher`, `arkdown-cli`         | SPEC-039, SPEC-040  |
| SPEC-037 | ArkData Compile and ETS Checker    | `arkdown-abc`, `arkdown-cli`              | SPEC-009, SPEC-004a |

---

## Implementation Order

```
SPEC-001 (core types)
    ├── SPEC-002 (config) ──────────────────────► SPEC-010, 011
    ├── SPEC-003 (resolver)
    ├── SPEC-004 (loader) ──────────────────────► SPEC-004a (ets-transform, Rust)
    └── SPEC-005 (bundler core)
            ├── SPEC-006 (chunk wrapper)
            ├── SPEC-007 (json5 plugin)
            ├── SPEC-008 (duplicate check)
            ├── SPEC-014 (module meta)
            ├── SPEC-016 (har dedup)
            ├── SPEC-017 (router map)
            └── SPEC-018 (case check)
                    │
            SPEC-009 (abc compiler, Rust)
                    │
            SPEC-012 (packager) ─── SPEC-021 (signing)
                    │
            SPEC-013 (CLI) ─── SPEC-022 (hvigorw compat)
                    │
            SPEC-019 (cache), SPEC-023 (watch)
```

---

## Research Documents

| Document                              | Date       | Topic                                                                    |
| ------------------------------------- | ---------- | ------------------------------------------------------------------------ |
| `meta/design/rolldown-integration.md` | 2026-05-28 | Rolldown integration options (i/ii/iii) — verdict: use as reference only |
| `meta/design/hvigorw-pipeline.md`     | 2026-05-28 | Complete hvigorw build pipeline flowchart and plugin inventory           |
