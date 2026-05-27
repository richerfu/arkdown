# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## What This Project Is

**arkdown** is a Rust bundler for OpenHarmony OS (OHOS) projects. It reimplements the
rolldown design and replaces Huawei's `hvigorw`/hvigor build tool. It takes `.ets`/`.ts`
source files and produces installable `.hap`/`.hsp`/`.har` artifacts.

**Parser**: Uses `ohos-rs/oxc` (`https://github.com/ohos-rs/oxc`) — a fork of OXC with
ArkTS support. Do not use upstream `oxc-project/oxc`; always use the ohos-rs fork.

**Key design constraint**: ArkRuntime does NOT execute native ESM. All output chunks must
be IIFE-wrapped and use `globalThis['name_hash']` for cross-chunk references. This is
non-negotiable.

---

## Build Commands

```bash
cargo build                          # build all crates
cargo build -p arkdown-core          # build one crate
cargo test                           # run all tests
cargo test -p arkdown-resolver       # test one crate
cargo test -p arkdown -- chunk       # run tests matching "chunk"
cargo clippy -- -D warnings          # lint
cargo fmt                            # format
```

No `justfile` yet — use cargo directly.

---

## Workspace Structure

```
arkdown/
├── CLAUDE.md                 ← this file
├── Cargo.toml                ← workspace root
├── meta/
│   ├── design/               ← architecture docs (READ BEFORE CODING)
│   └── tasks/                ← implementation specs per crate
├── crates/
│   ├── arkdown-core/         ← shared types + ArkdownPlugin trait (SPEC-001)
│   ├── arkdown-config/       ← JSON5 config parser (SPEC-002)
│   ├── arkdown-resolver/     ← OHOS module resolution library (SPEC-003)
│   ├── arkdown-loader/       ← parallel module loading + plugin pipeline (SPEC-004)
│   ├── arkdown/              ← bundler core: link, tree-shake, chunk-split (SPEC-005)
│   ├── arkdown-plugin-chunk-wrapper/  ← IIFE wrapping, globalThis registry (SPEC-006)
│   ├── arkdown-plugin-json5/          ← JSON/JSON5 import (SPEC-007)
│   ├── arkdown-plugin-duplicate-check/ ← filename uniqueness check (SPEC-008)
│   ├── arkdown-plugin-module-meta/    ← per-module OHOS metadata (SPEC-014)
│   ├── arkdown-plugin-ohos-resolve/   ← resolve plugin wrapping resolver (SPEC-015)
│   ├── arkdown-plugin-har-dedup/      ← HAR deduplication for HSP builds (SPEC-016)
│   ├── arkdown-plugin-router-map/     ← router map cache invalidation (SPEC-017)
│   ├── arkdown-plugin-sensitive-check/ ← case-sensitivity import check (SPEC-018)
│   ├── arkdown-plugin-ets-transform/  ← pure Rust ArkTS transform, ports ets-loader (SPEC-004a)
│   ├── arkdown-abc/          ← Rust ArkCompiler bytecode compiler (SPEC-009)
│   ├── arkdown-resource/     ← Rust resource compiler, ports restool (SPEC-011)
│   ├── arkdown-profile/      ← module.json5 processor + loader.json gen (SPEC-010)
│   ├── arkdown-pack/         ← HAP/HSP/HAR zip assembly (SPEC-012)
│   ├── arkdown-cache/        ← incremental build cache (SPEC-019)
│   ├── arkdown-sign/         ← HAP signing via hap-sign-tool (SPEC-021)
│   └── arkdown-cli/          ← arkdown CLI binary (SPEC-013)
```

---

## Read These Before Working in an Area

| Working on                   | Read first                                                                         |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| Any crate                    | `meta/design/README.md` (key discoveries summary)                                  |
| Type definitions             | `meta/design/types.md`                                                             |
| Module loading, resolution   | `meta/design/module-resolution.md`                                                 |
| Chunk output format          | `meta/design/chunk-model.md` — the IIFE/globalThis pattern is non-obvious          |
| ABC compiler (arkdown-abc)   | `meta/design/abc-pipeline.md` — Panda bytecode format, IR pipeline                 |
| hvigorw replacement          | `meta/design/compat.md`                                                            |
| Plugin work                  | `meta/design/plugins.md` — which JS plugins we're replacing and why                |
| ArkTS transform (ets-loader) | `meta/tasks/SPEC-004a-ets-transform.md` — decorator rules, transform output format |
| Full pipeline                | `meta/design/pipeline.md`                                                          |
| Build task details           | `meta/design/hvigorw-analysis.md`                                                  |
| Implementation spec          | `meta/tasks/SPEC-<N>-*.md` for the relevant crate                                  |

---

## Critical Design Rules

**1. IIFE wrapping is always required.** Every output chunk must be wrapped in `(function () { ... })()`.
ArkRuntime never runs raw JS. The `arkdown-plugin-chunk-wrapper` crate owns this. Never skip it.

**2. Output format is CJS, not ESM.** The bundler core (`arkdown`) must generate CJS output
(with `require()` calls). The chunk wrapper plugin converts `require()` to `globalThis[]`.
Do not generate ESM output.

**3. `arkdown-abc` is a pure Rust ArkCompiler bytecode compiler.** It ports `es2abc` from
`https://gitcode.com/openharmony/arkcompiler_ets_frontend`. No subprocess, no SDK binary
dependency. All `.abc` compilation logic is in this crate.

**4. `arkdown-resource` is a pure Rust resource compiler.** It ports `restool` from
`https://gitcode.com/openharmony/developtools_restool`. No subprocess invocation.

**5. Use ohos-rs/oxc, not oxc-project/oxc.** The ohos-rs fork adds ArkTS grammar support
(`.ets` files, `@Entry`, `@Component` struct syntax, decorator metadata). The upstream OXC
does not parse `.ets` files correctly.

**6. OHM URLs are module identities.** With `useNormalizedOHMUrl: true`, every module
identifier in the bundle is an OHM URL: `@bundle:<bundleName>/<moduleName>/<path>`. These
appear in the `globalThis` registry keys, the `loader.json`, and cross-HAR references.
See `meta/design/module-resolution.md`.

**7. loader.json is the bundler's config.** The bundler reads `loader.json` (written by
`arkdown-profile`) instead of `build-profile.json5` or `module.json5`. The `compileEntry`
array is the definitive list of entry points. See `meta/design/hvigorw-analysis.md`.

---

## SDK Paths

`es2abc`, `restool`, and `ets-loader` are all replaced by pure Rust crates — no SDK
binaries are required at build time. `OHOS_SDK_HOME` is not needed for compilation.

```bash
# OHOS_SDK_HOME is no longer required. Set it only if debugging against the SDK directly.
export OHOS_SDK_HOME=/path/to/sdk/default/openharmony
```

| SDK binary   | Rust replacement               | SPEC      |
| ------------ | ------------------------------ | --------- |
| `es2abc`     | `arkdown-abc`                  | SPEC-009  |
| `restool`    | `arkdown-resource`             | SPEC-011  |
| `ets-loader` | `arkdown-plugin-ets-transform` | SPEC-004a |

---

## Crate Conventions

- Each crate has a single `pub` API surface in `src/lib.rs`
- Error types in `src/error.rs`, using `thiserror`
- Async code uses `tokio`; CPU-bound parallelism uses `rayon`
- Plugin implementations live in their own crates; they depend on `arkdown-core` only
- No `unwrap()` or `expect()` in library code — use `?` and structured errors

---

## Dependency Policy

- `ohos-rs/oxc` (git) — parser, transformer, codegen, minifier
- `tokio` — async runtime (full features)
- `rayon` — parallel iteration
- `serde` + `serde_json` + `json5` — serialization
- `thiserror` — error types
- `index_vec` — typed index vectors (mirrors rolldown's IndexVec pattern)
- `rustc-hash` (FxHashMap) — fast hashmaps for hot paths
- `zip` — HAP/HSP/HAR archive creation
- `clap` — CLI argument parsing (CLI crate only)
- `notify` — filesystem watching (watcher crate only)

Avoid adding new dependencies without checking if an existing workspace dependency covers
the need.
