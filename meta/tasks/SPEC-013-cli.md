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
