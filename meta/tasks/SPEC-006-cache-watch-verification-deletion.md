# SPEC-006 Cache Watch Verification And Deletion

## Goal

Finish the migration by hardening cache/watch behavior, proving deterministic
outputs, and deleting legacy hvigor/Rollup/webpack compatibility paths.

## Cache Contract

`arkdown-cache` must own cache identity from explicit fingerprints:

- project config files
- module manifests
- package manifests and lockfiles
- dependency graph identity
- SDK metadata and tool versions
- profile/router/share/startup/privacy inputs
- resource inventory and resource compiler inputs
- source file contents
- transform options
- bundle plan identity
- external command plan identity
- bytecode/cache-file/protoBin inputs
- package/sign/native inputs
- build script declarative effects

Placeholder hashes and task-name-only cache keys are forbidden.

## Watch Contract

`arkdown-watcher` must consume session inventories and dependency graph views.
It must not recursively re-run ad hoc CLI discovery logic.

Watch invalidation must identify affected typed IDs:

- project input
- module
- package/dependency node
- profile file
- resource entry
- source file
- SDK metadata file
- native source/lib
- build script file

Rebuild scheduling must reuse the same `BuildCoordinator` ordering and
dependency-wave model as a normal build.

## Determinism Contract

The final pipeline must produce deterministic outputs across thread counts:

- dependency graph topological order
- profile merge output
- loader/module metadata
- Rolldown output and source maps
- importee maps
- bytecode HAR metadata
- pack entry order
- signing inputs
- cache database commits

Hash-map iteration must never be serialized directly.

## Deletion Contract

Delete the following when the last production caller is gone:

- hvigor runtime bridge and script execution paths.
- public `HvigorScript*`, `HvigorTaskGraph*`, and hvigor task-name orchestration
  APIs.
- Rollup cache bridge paths that depend on Rollup cache object shape.
- webpack/ets-loader compile node paths.
- JS compile-plugin execution paths.
- plugin adapters that perform filesystem reads in hooks.
- duplicated dependency graph builders.
- obsolete task specs and references to old phase plans.

The deletion must happen in the same change that migrates the final production
caller. Do not leave compatibility wrappers to keep old behavior alive.

## Guardrail Searches

Run these repeatedly during the migration:

```bash
rg -n "index_vec|Arc<Mutex|Mutex<|read_to_string|fs::read|read_dir|WalkDir" crates Cargo.toml
rg -n "starts_with\\('/'\\)|contains\\(\"oh_modules\"\\)|replace\\('\\\\\\\\', \"/\"\\)|to_string_lossy" crates
rg -n "\\butils\\b|\\bhelpers\\b|\\bcommon\\b|\\bmisc\\b" crates
rg -n "rollup\\(|webpack|ets-loader|hvigor-arkts-compose|@ohos/hvigor" crates
rg -n "HvigorScript|HvigorTask|hvigorfile_runtime|Legacy.*Task|CompileNode" crates
```

Every match must be classified as:

- deleted
- test fixture
- audit-only documentation
- diagnostic-only compatibility message
- external tool command boundary

## Required Test Matrix

Minimum fixture matrix:

- `tests/fixtures/minimal-hap`
- `tests/fixtures/minimal-hsp`
- `tests/fixtures/minimal-har`
- `tests/fixtures/minimal-hap-with-bc-har`
- `tests/fixtures/release-obfuscation-hap`
- `tests/fixtures/share-config-hap`
- `tests/fixtures/startup-config-hap`

Additional fixtures must be added for:

- dynamic import metadata
- router map
- privacy/PAC
- native library packaging
- preview resource invalidation
- hot reload source invalidation
- ohosTest/unit-test variants
- unsupported dynamic `hvigorfile.ts` diagnostic
- supported declarative build script overlay

## Required Commands

Targeted commands during implementation:

```bash
cargo test -p arkdown-session
cargo test -p arkdown
cargo test -p arkdown-ets-transform
cargo test -p arkdown-profile
cargo test -p arkdown-resource
cargo test -p arkdown-pack
cargo test -p arkdown-sign
cargo test -p arkdown-ts2abc
cargo test -p arkdown-napi
```

Final commands:

```bash
cargo metadata --format-version 1 --no-deps
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

For determinism, run representative fixture builds with:

```bash
RAYON_NUM_THREADS=1 cargo run -p arkdown-napi -- build --project tests/fixtures/minimal-hap
RAYON_NUM_THREADS=8 cargo run -p arkdown-napi -- build --project tests/fixtures/minimal-hap
```

Compare artifact hashes after excluding timestamps or signing metadata that is
documented as intentionally variable.

## Completion Criteria

The migration is complete only when:

- no production code executes hvigor, Rollup, webpack, ets-loader, or
  hvigor-arkts-compose;
- all non-external build behavior is owned by Rust/OXC/Rolldown crates;
- external tools are behind typed plans and typed runners;
- manual orchestration variants cover HAP, HSP, HAR, preview, reload, unit
  test, and ohosTest modes;
- cache and watch invalidation are typed and deterministic;
- old tasks and old compatibility paths are deleted.
