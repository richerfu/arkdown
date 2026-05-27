# SPEC-013: CLI

**Crate**: `arkdown-cli`
**Priority**: P0
**Blocked by**: SPEC-012
**Blocks**: SPEC-022 (hvigorw compat), SPEC-023 (watch)

---

## Summary

The `arkdown` command-line tool. Orchestrates all other crates into a complete build pipeline.
Provides the user-facing interface and integrates with the OHOS project structure.

---

## Commands

```
arkdown build   [--target hap|hsp|har|app] [--mode debug|release]
                [--module <name>] [--product <name>] [--project <path>]
arkdown clean   [--project <path>]
arkdown watch   [--target hap] [--module <name>]   (SPEC-023, P2)
arkdown test    [--module <name>]
arkdown --version
arkdown --help
```

### `arkdown build`

Full build pipeline for the specified target:

```
1. Config parsing (SPEC-002)
2. Profile processing → loader.json (SPEC-010)
3. Resource compilation (SPEC-011)     ┐ parallel
4. Bundling (SPEC-005)                 ┘
5. ABC compilation (SPEC-009)
6. Packaging (SPEC-012)
7. Signing (SPEC-021, if signingConfig present)
```

Steps 3 and 4 run in parallel (resources and JS bundling are independent).

### `arkdown clean`

Delete `build/` directory in the project root.

---

## Progress Output

Emit structured progress similar to hvigor:

```
arkdown: Building entry (debug/hap)
  [1/6] Parsing config...          done  (12ms)
  [2/6] Processing profiles...     done  (45ms)
  [3/6] Compiling resources...     done (120ms)
  [4/6] Bundling modules...        done (890ms)
  [5/6] Compiling ABC...           done (1.2s)
  [6/6] Packaging...               done (80ms)

Build succeeded: build/default/outputs/default/entry.hap (2.3s)
```

---

## Error Formatting

On `ArkdownError`, emit diagnostics pointing to the source file with line numbers (similar to
cargo's error format):

```
error[E001]: duplicate filename detected
  --> entry/src/main/ets/utils/Logger.ets
  |
  | first occurrence: entry/src/main/ets/utils/Logger.ets
  | conflict with:    entry/src/main/ets/components/Logger.ets
  |
  = note: es2abc uses filenames as module record names; duplicates cause bytecode conflicts
```

---

## Exit Codes

- `0` — success
- `1` — build error (diagnostic emitted to stderr)
- `2` — configuration error (invalid project structure or missing SDK)

---

## Environment Variables

- `OHOS_SDK_HOME` — SDK path override
- `ARKDOWN_LOG` — log level (`debug`, `info`, `warn`, `error`)
- `ARKDOWN_JOBS` — parallelism override (default: CPU count)

---

## Files to Create

```
crates/arkdown-cli/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── cmd/
    │   ├── build.rs    (build subcommand orchestration)
    │   └── clean.rs
    └── progress.rs     (progress display)
```

Use `clap` for argument parsing.

---

## Acceptance Criteria

- `arkdown build --target hap` on `MyApplication` produces an installable `.hap`
- `arkdown build --mode release` applies tree shaking + opt-level 2 to es2abc
- Errors print file paths with line numbers where possible
- Exit code 1 on any build failure

---

## Design References

- `meta/design/compat.md` — command mapping from hvigorw
- `meta/design/pipeline.md` — full pipeline for orchestration order

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| `arkdown build` command | rolldown `Bundler::write()` orchestration | rolldown is a library; its CLI (`rolldown` command) calls `Bundler::write()`; arkdown CLI does the same plus resource compilation, ABC compilation, and packaging |
| `--mode debug\|release` | rolldown watch mode vs production build | arkdown's mode flag maps to `BuildMode` which controls tree-shaking and obfuscation; rolldown uses `process.env.NODE_ENV` convention |
| `--target hap\|hsp\|har` | rolldown `format` option (`esm`, `cjs`, `iife`) | Different concept: arkdown target determines archive format; rolldown format determines JS module format |
| `--module <name>` | no direct rolldown equivalent | OHOS-specific: scopes the build to one OHOS module; rolldown always builds all entries |
| `--product <name>` | no direct rolldown equivalent | OHOS-specific: selects the product variant from `build-profile.json5` |
| Steps 3+4 in parallel | rolldown parallel module loading | Both run independent tasks concurrently; in arkdown it is resource compilation (step 3) and JS bundling (step 4) |
| `ARKDOWN_LOG` env var | rolldown `ROLLDOWN_LOG_LEVEL` | Same purpose: log verbosity control |
| `ARKDOWN_JOBS` env var | rolldown `ROLLDOWN_CONCURRENCY` | Both control worker parallelism |
| Cargo-style error formatting | rolldown diagnostic output | Same `error[CODE]: message\n  --> file:line:col` format |

---

## Test Requirements

- `arkdown build --target hap` on `MyApplication` exits with code 0 and produces `build/default/outputs/default/entry.hap`
- `arkdown build --mode release` applies tree shaking (verified: unused export absent from `.abc`)
- `arkdown build --module entry` builds only the `entry` module; `feature` module is not built
- `arkdown build --product default` selects the correct product from `build-profile.json5`
- `arkdown build` with a missing `module.json5` exits with code 2 and prints a config error message
- `arkdown build` with a duplicate filename exits with code 1 and prints a `duplicate-check` error with both file paths
- `arkdown clean` deletes the `build/` directory and exits with code 0
- `arkdown --version` prints the crate version string and exits with code 0
- Resource compilation (step 3) and bundling (step 4) are confirmed to run concurrently (timing test or log ordering)
- `ARKDOWN_LOG=debug` causes debug-level log lines to appear on stderr
- Progress output format: each step line contains the step number, description, and elapsed time
- Exit code 1 on any `ArkdownError` from any pipeline step
