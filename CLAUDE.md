# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## What This Project Is

**arkdown** is a Rust build frontend for OpenHarmony OS (OHOS) projects. It replaces
hvigor's JavaScript/TypeScript orchestration and Huawei's Rollup pipeline with
OXC and Rolldown, while preserving hvigor's observable build contracts. It takes
`.ets`/`.ts` source files and produces installable `.hap`/`.hsp`/`.har` artifacts.

**Parser**: Uses `ohos-rs/oxc` (`https://github.com/ohos-rs/oxc`) — a fork of OXC pinned
in the root `Cargo.toml` (`rev = "c8d02452469e159952f0181b022c1f304e77abee"`).
Always use this pinned fork for `.ets` parsing. **Do not use upstream `oxc-project/oxc` crates.io
releases for arkdown parser/transform crates** — they do not parse `.ets` syntax correctly.

Note: `ohos-rs/oxc-ark` (`https://github.com/ohos-rs/oxc-ark`) is a separate ArkTS lint/format
CLI tool (`oxk`). It is NOT used as a library dependency here.

**Initial parity boundary**: upstream native C/C++ tools may be used directly.
The first production path focuses on replacing hvigor's JS/TS tasks, SDK JS
compile plugins, and Rollup performance bottlenecks. Native backends such as
`es2abc`, `restool`, and signing tools can remain SDK-backed until their Rust
ports are explicitly selected and proven by parity tests.

**Key design constraint**: the final artifact must contain ArkCompiler bytecode,
not raw JavaScript. The upstream `es2abc` entry accepts normal JS/TS source
files; `--module` selects ECMAScript module parsing.

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
│   └── tasks/                ← active OXC/Rolldown implementation specs
├── crates/
│   ├── arkdown-core/         ← shared IDs, diagnostics, target/artifact contracts
│   ├── arkdown-path/         ← typed paths: AbsPath, RelPath, ArtifactPath, OhmUrl
│   ├── arkdown-ohos-schema/  ← typed OHOS JSON/JSON5 schemas
│   ├── arkdown-config/       ← thin compatibility config facade
│   ├── arkdown-ohpm/         ← OHPM lockfile/cache/registry/tarball/integrity
│   ├── arkdown-session/      ← BuildSession snapshots, dependency graph, source/profile/resource inventories
│   ├── arkdown-resolver/     ← OHOS module/package resolution
│   ├── arkdown-ets-transform/ ← OXC ArkTS/TS/JS transforms, checker, source maps
│   ├── arkdown/              ← Rolldown integration + built-in plugin adapters
│   ├── arkdown-profile/      ← module/profile/router/startup/share/privacy/PAC processing
│   ├── arkdown-resource/     ← resource inventory and restool-compatible plans
│   ├── arkdown-ts2abc/       ← SDK es2abc/Rust bytecode backend planning
│   ├── arkdown-pack/         ← HAP/HSP/HAR/app packaging
│   ├── arkdown-sign/         ← signing and verification
│   ├── arkdown-obfuscate/    ← obfuscation rule planning and mapping
│   ├── arkdown-cache/        ← fingerprint-based cache keys and records
│   ├── arkdown-watcher/      ← watch invalidation from session inventories
│   └── arkdown-cli/          ← BuildCoordinator and CLI binary
```

Separate `arkdown-plugin-*` crates are obsolete in the final design. Rollup plugin
behavior must move to internal Rolldown adapters under `crates/arkdown/src/plugins/`;
business logic must move to the owning domain crate. Delete obsolete plugin crates,
workspace members, dependency declarations, imports, tests, and compatibility wrappers
after the final production caller moves.

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
| ArkTS transform (ets-loader) | `meta/tasks/SPEC-004-oxc-arkts-transform.md` |
| Full pipeline                | `meta/design/pipeline.md`                                                          |
| Build task details           | `meta/design/hvigorw-analysis.md`                                                  |
| Final crate/file layout      | `meta/tasks/SPEC-000-crate-topology-and-build-flow.md` and `meta/tasks/SPEC-007-final-file-layout-implementation-map.md` |
| Implementation spec          | active `meta/tasks/SPEC-*.md` files for the relevant owner                         |

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

**3. Native SDK backends are allowed in the initial parity path.** If upstream
behavior is implemented in C/C++, prefer invoking the corresponding SDK tool
first (`es2abc`, `restool`, signing) instead of blocking the JS/TS + Rollup
replacement on a Rust port. Rust native backends remain optional parity targets.

**4. Use ohos-rs/oxc (pinned rev), not oxc-project/oxc crates.io.** The ohos-rs fork (pinned at
`rev = "c8d02452469e159952f0181b022c1f304e77abee"`) supports `.ets` parsing. The upstream
crates.io releases do not. Always specify the `rev` in Cargo.toml; do not use a floating branch.
`ohos-rs/oxc-ark` is a lint/format CLI, not a parser library — do not confuse the two.

**5. OHM URLs are module identities.** With `useNormalizedOHMUrl: true`, every module
identifier in the bundle is an OHM URL: `@bundle:<bundleName>/<moduleName>/<path>`. These
appear in the `globalThis` registry keys, the `loader.json`, and cross-HAR references.
See `meta/design/module-resolution.md`.

**6. loader.json is the bundler's config.** The bundler reads `loader.json` (written by
`arkdown-profile`) instead of `build-profile.json5` or `module.json5`. The `compileEntry`
array is the definitive list of entry points. See `meta/design/hvigorw-analysis.md`.

**7. `bundleName` comes from `AppScope/app.json5`, not `products`.** In real DevEco Studio
projects, `build-profile.json5` products do NOT contain `bundleName`. Read it from
`AppScope/app.json5` via `load_app_json5()`. Signing configs live at
`build-profile.json5#app.signingConfigs`; products reference them by name.

**8. The CLI plugin order must match hvigor's compile stage.** `EtsTransformPlugin` must
run before resolution-sensitive plugins. Avoid injecting helper wrappers or rollup defaults
that hvigor does not emit. For HAR builds use `pack_har()`, not `pack_hap()`.

**9. HAP zip structure requires `pack.info` and `pkgContextInfo.json`.** Every HAP must contain:
`module.json` (with `app` + `module` sections merged), `pack.info`, `pkgContextInfo.json`,
`resources.index`, `ets/modules.abc` (PANDA magic, API >= 9), `resources/base/` (copied from
`src/main/resources/base/`). See `meta/design/hvigorw-analysis.md` section 10 and SPEC-012.

**10. TypeScript normalization is required before ABC compilation.** Use the pinned
`ohos-rs/oxc` parser/codegen path for TS/ETS syntax before handing files to
`es2abc` or the optional Rust ABC backend. Panda `.abc` magic:
`b"PANDA\x00\x00\x00"` (8 bytes).

**11. Minimum implementations are forbidden.** Do not land placeholder behavior,
stub parity, empty adapters, fake cache keys, no-op runners, TODO-only code paths,
or "just enough to compile" implementations. A change must either implement the
owning behavior fully for its declared scope or explicitly reject unsupported behavior
with typed diagnostics and focused coverage.

**12. Do not recreate hvigor's task runtime.** The CLI uses coarse manual
orchestration through `BuildCoordinator`. Do not introduce a full `BuildStageGraph`,
dynamic task API, string task dependencies, or user task runtime. Rolldown owns module
graph, plugin lifecycle, chunk graph, render hooks, and generate-bundle behavior
inside `Bundler::bundle_with_plan`.

**13. Rollup plugins migrate to Rolldown adapters.** Hook behavior from hvigor's
Rollup plugins belongs in `crates/arkdown/src/plugins/*`. Non-hook behavior belongs in
domain crates such as `arkdown-resolver`, `arkdown-ets-transform`, `arkdown-profile`,
`arkdown-resource`, `arkdown-pack`, or `arkdown-cache`.

**14. Stage model (ESMODULE, API >= 9) only. JSBUNDLE/FA model is explicitly out of scope.**
arkdown targets the Stage model compile path exclusively:

- `compileMode = "esmodule"` — the only supported mode. FA model (`compileMode = "jsbundle"`)
  is rejected at build time by `ProcessProfileProcessor::validate_compile_mode()`.
- `partialUpdate = true` — `ComponentInfo.partial_update` must be `true`; the ETS transform
  generates `initialRender()` (Stage model) not `render()` (FA model).
- **Do not implement JSBUNDLE-only hvigor paths.** These are permanently N/A:
  - `@rollup/plugin-terser` — JSBUNDLE release minification
  - `@rollup/plugin-babel` — JSBUNDLE class-properties transform
  - `sourcemapPathTransform` — JSBUNDLE source map path stripping
  - `generateId(statements)` — JSBUNDLE FA-model identifier injection
  - FA model `render()` method generation
- Source map output follows the ESMODULE model: `sourceMaps.map` with
  absolute-to-`ets/`-relative mapping, not the JSBUNDLE `sourcemap: true` rollup option.

---

## SDK Paths

The initial production path may use SDK-native tools for C/C++ implemented
stages. `OHOS_SDK_HOME` or configured SDK tool paths are required when selecting
the SDK `es2abc`, `restool`, or signing backend.

```bash
export OHOS_SDK_HOME=/path/to/sdk/default/openharmony
```

| Upstream component | Initial path | Rust/arkdown owner |
| ------------------ | ------------ | ------------------ |
| `ets-loader` JS plugins | Replaced by OXC transforms | `arkdown-ets-transform` + `arkdown/src/plugins/ets_transform.rs` |
| Rollup | Replaced by Rolldown | `arkdown` |
| `es2abc` | SDK backend by default | optional `arkdown-abc` backend |
| `restool` | SDK-compatible/native path allowed | `arkdown-resource` planning |
| `hap-sign-tool.jar` | SDK/native path allowed | optional `arkdown-sign` backend |

`arkdown-sign` also contains a Rust signing implementation:

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
- Built-in Rolldown plugin adapters live in `crates/arkdown/src/plugins/`; separate
  `arkdown-plugin-*` crates must not be kept in the final production topology
- No `unwrap()` or `expect()` in library code — use `?` and structured errors
- Standalone functions are forbidden for production build behavior. Free functions are
  allowed only for pure local algorithms that do not read files, write files, cache
  state, inspect global config, mutate artifacts, construct commands, encode lifecycle,
  or execute stages. If logic needs build state, make it a method on the owning builder,
  resolver, compiler, processor, planner, store, writer, pass, runner, adapter, factory,
  snapshot, or inventory.
- Follow OXC/Rolldown code organization before inventing local structure: pass-oriented
  transform modules, thin Rolldown adapter modules, explicit factories/runners, narrow
  `mod.rs` exports, and tests near the owning behavior.
- Do not generate useless tests. New tests must protect real behavior, a regression, a
  crate boundary, a path/cache/determinism invariant, or a migration deletion contract.
- Do not add `#[cfg(test)]`, `mod tests`, test-only helper modules, or similar conditional
  test code inside generated production source files. Put tests in existing test locations
  or dedicated fixture/harness files.

---

## Dependency Policy

- `ohos-rs/oxc` (git) — parser, transformer, codegen, minifier
- `tokio` — async runtime (full features)
- `rayon` — parallel iteration
- `serde` + `serde_json` + `json5` — serialization
- `thiserror` — error types
- `oxc_index` (crates.io `4.1.0`) — typed index vectors; **`index_vec` has been removed**
- `compact_str` — compact small strings for hot-path identifiers
- `rustc-hash` (FxHashMap) — fast hashmaps for hot paths
- `zip` — HAP/HSP/HAR archive creation
- `clap` — CLI argument parsing (CLI crate only)
- `notify` — filesystem watching (watcher crate only)

### Foundation crate layering

Foundation crates must not depend on session, plugin, bundler, or CLI crates:

```
arkdown-path → (no workspace deps)
arkdown-ohos-schema → (no workspace deps)
arkdown-ohpm → arkdown-path, arkdown-ohos-schema
arkdown-session → arkdown-core, arkdown-path, arkdown-ohos-schema, arkdown-ohpm
```

### Forbidden patterns

- `path_str.starts_with('/')` — use `Path::is_absolute()`
- `module_id.contains("oh_modules")` — use `arkdown_path::classify::is_inside_oh_modules()`
- inline `.replace('\\', "/")` — use `ArtifactPathSerializer::path_to_slash()`
- `index_vec::IndexVec` — use `oxc_index::IndexVec`

Avoid adding new dependencies without checking if an existing workspace dependency covers
the need.
