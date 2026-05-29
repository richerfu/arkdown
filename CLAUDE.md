# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## What This Project Is

**arkdown** is a Rust bundler for OpenHarmony OS (OHOS) projects. It reimplements Huawei's `hvigorw`/hvigor build tool by oxc and rolldown. It takes `.ets`/`.ts`
source files and produces installable `.hap`/`.hsp`/`.har` artifacts.

**Parser**: Uses `ohos-rs/oxc` (`https://github.com/ohos-rs/oxc`) — a fork of OXC pinned
at the commit that `ohos-rs/oxc-ark` depends on (`rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614"`).
Always use this pinned fork for `.ets` parsing. **Do not use upstream `oxc-project/oxc` crates.io
releases** — they do not parse `.ets` syntax correctly.

Note: `ohos-rs/oxc-ark` (`https://github.com/ohos-rs/oxc-ark`) is a separate ArkTS lint/format
CLI tool (`oxk`). It is NOT used as a library dependency here; it is only a reference for which
`ohos-rs/oxc` commit to pin.

**Key design constraint**: the final artifact must contain ArkCompiler bytecode, not raw
JavaScript. The upstream `es2abc` entry accepts normal JS/TS source files; `--module`
selects ECMAScript module parsing.

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
│   ├── arkdown-plugin-chunk-wrapper/  ← legacy IIFE/globalThis registry (SPEC-006, not used in ESM pipeline)
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
| Chunk output format          | `meta/design/chunk-model.md` — ESM output format; IIFE/globalThis legacy removed    |
| ABC compiler (arkdown-abc)   | `meta/design/abc-pipeline.md` — Panda bytecode format, IR pipeline                 |
| hvigorw replacement          | `meta/design/compat.md`                                                            |
| Plugin work                  | `meta/design/plugins.md` — which JS plugins we're replacing and why                |
| ArkTS transform (ets-loader) | `meta/tasks/SPEC-004a-ets-transform.md` — decorator rules, transform output format |
| Full pipeline                | `meta/design/pipeline.md`                                                          |
| Build task details           | `meta/design/hvigorw-analysis.md`                                                  |
| Implementation spec          | `meta/tasks/SPEC-<N>-*.md` for the relevant crate                                  |

---

## Critical Design Rules

**1. Bytecode compilation is always required.** ArkRuntime executes `.abc`, not raw
JavaScript. `arkdown-abc` must match upstream `es2abc` on both final bytecode and
intermediate output files under the loader output tree.

**2. es2abc/ts2abc module inputs are normal ESM source files.** Upstream
`arkcompiler_ets_frontend` builds `SourceFile(..., options->IsModule(), ...)` and the
`ts2abc.js` wrapper only forwards bytecode-version flags to `es2abc`. The bundler outputs
clean ESM (`import`/`export` statements, no IIFE wrapper, no CJS interop helpers). The
IIFE-unwrap and CJS-wrapper compatibility paths have been removed from `arkdown-abc`; the
compiler always compiles the top-level program body directly in `--module` mode.

**3. `arkdown-abc` is a pure Rust ArkCompiler bytecode compiler.** It ports `es2abc` from
`https://github.com/openharmony/arkcompiler_ets_frontend`. No subprocess, no SDK binary
dependency. All `.abc` compilation logic is in this crate.

**4. `arkdown-resource` is a pure Rust resource compiler.** It ports `restool` from
`https://github.com/openharmony/developtools_restool`. No subprocess invocation.

**5. Use ohos-rs/oxc (pinned rev), not oxc-project/oxc crates.io.** The ohos-rs fork (pinned at
`rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614"`) supports `.ets` parsing. The upstream
crates.io releases do not. Always specify the `rev` in Cargo.toml; do not use a floating branch.
`ohos-rs/oxc-ark` is a lint/format CLI, not a parser library — do not confuse the two.

**6. OHM URLs are module identities.** With `useNormalizedOHMUrl: true`, every module
identifier in the bundle is an OHM URL: `@bundle:<bundleName>/<moduleName>/<path>`. These
appear in the `globalThis` registry keys, the `loader.json`, and cross-HAR references.
See `meta/design/module-resolution.md`.

**7. loader.json is the bundler's config.** The bundler reads `loader.json` (written by
`arkdown-profile`) instead of `build-profile.json5` or `module.json5`. The `compileEntry`
array is the definitive list of entry points. See `meta/design/hvigorw-analysis.md`.

**8. `bundleName` comes from `AppScope/app.json5`, not `products`.** In real DevEco Studio
projects, `build-profile.json5` products do NOT contain `bundleName`. Read it from
`AppScope/app.json5` via `load_app_json5()`. Signing configs live at
`build-profile.json5#app.signingConfigs`; products reference them by name.

**9. The CLI plugin order must match hvigor's compile stage.** `EtsTransformPlugin` must
run before resolution-sensitive plugins. Avoid injecting helper wrappers or rollup defaults
that hvigor does not emit. For HAR builds use `pack_har()`, not `pack_hap()`.

**10. HAP zip structure requires `pack.info` and `pkgContextInfo.json`.** Every HAP must contain:
`module.json` (with `app` + `module` sections merged), `pack.info`, `pkgContextInfo.json`,
`resources.index`, `ets/modules.abc` (PANDA magic, API >= 9), `resources/base/` (copied from
`src/main/resources/base/`). See `meta/design/hvigorw-analysis.md` section 10 and SPEC-012.

**11. TypeScript normalization is required before ABC compilation.** Use the pinned
`ohos-rs/oxc` parser/codegen path for TS/ETS syntax before IR lowering in `arkdown-abc`.
Panda `.abc` magic: `b"PANDA\x00\x00\x00"` (8 bytes).

---

## SDK Paths

`es2abc`, `restool`, and `ets-loader` are all replaced by pure Rust crates — no SDK
binaries are required at build time. `OHOS_SDK_HOME` is not needed for compilation.

```bash
# OHOS_SDK_HOME is no longer required. Set it only if debugging against the SDK directly.
export OHOS_SDK_HOME=/path/to/sdk/default/openharmony
```

| SDK binary / tool   | Rust replacement               | SPEC      |
| ------------------- | ------------------------------ | --------- |
| `es2abc`            | `arkdown-abc`                  | SPEC-009  |
| `restool`           | `arkdown-resource`             | SPEC-011  |
| `ets-loader`        | `arkdown-plugin-ets-transform` | SPEC-004a |
| `hap-sign-tool.jar` | `arkdown-sign`                 | SPEC-021  |

**No Java required.** `arkdown-sign` implements the full HAP signing pipeline in pure Rust:

- PKCS#12 keystore loading (`pkcs12` crate)
- CMS/PKCS#7 SignedData generation (`cms` + `der` crates)
- ECDSA P-256 / RSA-PSS signing (`p256`, `rsa` crates)
- ZIP alignment (4096-byte for `.abc`/`.so`) and signing block insertion
- HAP-specific chunk digest (SHA-256 with `0x5a`/`0xa5` type-prefixed chunking)

Magic constants from `developtools_hapsigner`:

- V2 magic (compatibleSdkVersion < 8): `b"HAP Sig Block 42"` (16 bytes)
- V3 magic (>= 8): `b"<hap sign block>"` (16 bytes)
- Main sig block ID: `0x20000000`, profile block ID: `0x20000002`

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
