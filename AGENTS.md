# Repository Guidelines

## Project Structure & Module Organization

`arkdown` is a Rust workspace for building OpenHarmony artifacts (`.hap`, `.hsp`, `.har`, `.app`) and test outputs from ArkTS/TypeScript projects. Workspace crates live under `crates/`: shared types in `arkdown-core`, typed paths in `arkdown-path`, OHOS schemas in `arkdown-ohos-schema`, startup snapshots in `arkdown-session`, resolving in `arkdown-resolver`, OXC transforms in `arkdown-ets-transform`, Rolldown bundling and built-in plugin adapters in `arkdown`, ABC compiler plans and SDK handoff in `arkdown-abc`, CMake/Ninja/native-library processing in `arkdown-native`, typed application requests/module planning/layout/pre-build/intermediate/code-resource behavior in `arkdown-build`, and packaging/signing/profile/resource/cache/watch support in matching crates. Packing validation belongs to `arkdown-pack`. The npm package and typed N-API service boundary live under `packages/arkdown`.

Separate `arkdown-plugin-*` crates are not part of the final architecture. Rollup plugin behavior migrates to internal Rolldown adapters under `crates/arkdown/src/plugins/`; non-hook business logic belongs in the owning domain crate. Delete obsolete plugin crates, compatibility wrappers, workspace members, dependency declarations, imports, tests, and docs after their final production caller moves.

Internal Hvigor compatibility layers are forbidden. Arkdown uses Hvigor only as the source-observed behavioral target: implement each verified behavior directly in its owning Rust domain type, and delete facades, adapters, aliases, wrappers, re-exports, and parallel code paths that exist only to imitate Hvigor's internal architecture or names. Do not preserve an old boundary after its final caller migrates. The sole compatibility boundary permitted by the target architecture is the public TS/JS plugin ABI in JavaScript/N-API; even there, project discovery, task orchestration, caching, SDK command construction, and artifact generation remain Rust-owned.

The current replacement plan is in `meta/tasks/`; read it before broad build-pipeline changes. Shared OHOS fixtures and assets are under `tests/fixtures/`.

Repository-level Codex skills live under `.agents/skills/` and are part of the
engineering rules for this repository. The directories actually present there
are the source of truth; do not reference or require a skill that is not in the
repository. When a task matches an available skill description, load and follow
that skill before planning or editing:

- `rust-architecture-standards`: Rust workspace, crate, Cargo, path,
  performance, OXC, and Rolldown standards.
- `arkdown-performance-memory`: parallel execution, dense IDs, memory ownership,
  and deterministic performance rules.
- `hap-verify-install`: HAP device/emulator verification installs must uninstall
  the target bundle before installing the generated package.

Hvigor parity is governed directly by the repository rules below; it does not
depend on a separate skill.

## Build, Test, and Development Commands

- `cargo build`: build the full workspace.
- `cargo build -p arkdown-core`: build one crate while iterating.
- `cargo test`: run workspace tests.
- `cargo test -p arkdown-resolver`: run tests for one crate.
- `cargo test -p arkdown -- chunk`: run tests matching a name filter.
- `cargo clippy -- -D warnings`: run lints as errors.
- `cargo fmt`: format Rust code.
- `node packages/arkdown/js/arkdown-cli.js build --project tests/fixtures/minimal-hap`: run the only supported CLI entry against the minimal fixture.

Use Cargo directly; there is no `justfile`.

## Entrypoint Boundary

The only executable command-line entry is `packages/arkdown/js/arkdown-cli.js`, published through the npm `bin` field. JavaScript owns argv parsing, help/version text, stdout/stderr, exit codes, and TS/JS plugin compatibility. Project discovery, build plans, transforms, caches, SDK commands, artifacts, checking, and cleaning remain Rust-owned behind typed N-API services.

Rust must not expose a command-line interface: do not add Rust `[[bin]]` targets, `main.rs`, Clap dependencies/derives, argv parsers, CLI install modes, or N-API functions such as `parse*CliArgs`. Do not recreate public process-monitor, runtime-report persistence, or CLI observability lifecycle facades in Rust; build diagnostics and internal structured tracing belong to the typed service implementation, while their CLI presentation and process lifecycle belong to JavaScript. The N-API boundary may expose typed service requests such as `build`, `check`, and `clean`; those are not CLI APIs. Rust application entry types and modules must use service/request terminology rather than `Command`, `cmd`, or `args`: the package owns `services`, while reusable request models belong in their owning Rust domain crate. Entrypoint changes must preserve the architecture deletion test in `packages/arkdown/__test__/runtime-architecture.test.cjs`.

## Coding Style & Naming Conventions

Rust code uses `rustfmt` defaults and four-space indentation. Crates and modules use kebab-case or snake_case to match Rust/Cargo conventions (`arkdown-ohos-schema`, `module_json.rs`). Types use `PascalCase`, functions and fields use `snake_case`, and CLI flags use kebab-case.

Prefer small crate-local helpers over cross-crate abstractions unless a pattern already exists. Keep generated artifacts and local SDK paths out of committed docs and source.

All dependency declarations must be centralized in the root `[workspace.dependencies]`. Crates under `crates/*` should reference both internal crates and third-party crates with `{ workspace = true }`; do not add direct `path`, `version`, or `git` dependency declarations inside crate manifests.

For implementation style, follow OXC/Rolldown patterns: keep behavior close to the owning type or module, prefer small state objects over long chains of free functions, and use OXC/Rolldown APIs for parsing, resolving, source maps, and code generation whenever available. Generated JavaScript/TypeScript should be produced through AST/codegen or structured serializers; use raw string assembly only when the upstream implementation does the same or no structured API exists.

Standalone functions are forbidden for production build behavior. Free functions are allowed only for pure local algorithms that do not read files, write files, cache state, inspect global config, mutate artifacts, construct commands, encode lifecycle, or execute stages. If logic needs build state, make it a method on the owning builder, resolver, compiler, processor, planner, store, writer, pass, runner, adapter, factory, snapshot, or inventory.

Minimum implementations are forbidden. Do not land placeholder behavior, stub parity, empty adapters, fake cache keys, no-op runners, TODO-only code paths, or "just enough to compile" implementations. A change must either implement the owning behavior fully for its declared scope or explicitly reject unsupported behavior with typed diagnostics and tests/fixtures that protect the rejection.

## Hvigor Parity Rules

For ArkTS 1.1, OpenHarmony source is authoritative when OXC extensions differ.
Do not port OXC-only syntax or semantics into the native tsrs checker.
tsrs (the native Rust checker) is used only for type checking and inference,
including parsing/binding needed for those queries and declaration diagnostics.
Keep its service programs in `noEmit` mode. Rust/OXC/Rolldown own transformations,
JavaScript/declaration generation and runtime annotation metadata; consume typed
checker facts where needed, never delegate build output to TypeScript emit.

arkdown's behavioral target is hvigor. For any change that affects build output,
runtime behavior, generated TypeScript/JavaScript, module resolution, bytecode,
resources, manifests, packaging, signing, cache layout, command construction, or
device startup compatibility, the implementation must be based on observed
hvigor behavior rather than guesses.

Before editing production code, identify the upstream behavior owner in the
current `developtools_ace_ets2bundle` source checkout and read that source, its
in-repository documentation, and every delegated SDK/compiler implementation
that owns the behavior. Reading current upstream source is a mandatory gate;
generated cache/output cannot substitute for it. Old hvigor decompilation trees
and old specs are not implementation evidence and must not be used to recreate
their internal architecture. If a failure only appears in a large app, compare
three artifacts only to locate and verify the source-owned difference:
original source, hvigor-generated output/cache, and arkdown-generated
output/cache. Keep the comparison focused on the failing module or stage, but do
not skip it.

Generated output, cache comparisons, test expectations, and decoded source-map
tokens locate or verify a difference; they never define implementation logic.
Before writing a parity fix, trace the current upstream
source from its public hook/task through the owning transformation, resolver,
writer, cache, or SDK handoff and implement that observed algorithm and data
semantics in the corresponding arkdown owner. Do not infer a new rule from a
fixture, a VLQ token, or a passing artifact and do not add heuristic branches
that have no upstream source owner. If the current upstream source does not
define the behavior, stop implementation and ask the user for direction rather
than inventing or preserving speculative logic.

Do not invent fallback behavior to make a sample pass. Do not hardcode project
paths, package layouts, native library declaration directories, cache paths, or
Meituan-specific locations. Paths and specifiers must come from manifests,
lockfiles, resolver APIs, hvigor task metadata, SDK metadata, or generated
module records. When hvigor delegates to a resolver or compiler service,
arkdown should use the corresponding structured Rust dependency or typed
adapter when available instead of hand-rolling string logic.

Every parity fix must either match hvigor for the declared scope or reject the
unsupported behavior with a typed diagnostic. Tests must protect the observed
parity contract, regression, or explicit rejection. Comments and test names
should cite the hvigor source file/function or generated artifact shape when
that evidence is non-obvious.

## Testing Guidelines

Place unit tests near the code or in crate-level `tests/` directories, matching the existing `*_tests.rs` pattern. Async plugin tests use `#[tokio::test]`. Shared fixtures belong in `tests/fixtures/`; keep generated build output out of version control.

For changes affecting generated JSON, loader metadata, manifests, bytecode, resources, or packaging, add focused regression coverage in the owning crate and, when applicable, the real hvigor parity fixture matrix.

Do not generate useless tests. New tests must protect real behavior, a regression, a crate boundary, a path/cache/determinism invariant, or a migration deletion contract. Do not add tests only for trivial getters, constructors, enum display output, or implementation details that do not protect behavior.

Do not add `#[cfg(test)]`, `mod tests`, test-only helper modules, or similar conditional test code inside generated production source files. Put tests in existing test locations or dedicated fixture/harness files. When deleting obsolete crates/modules, delete tests that only preserve the old boundary and replace them with coverage for the new owner.

## Commit & Pull Request Guidelines

History uses conventional commit style: `feat(scope): ...`, `fix(scope): ...`, and `docs(scope): ...` with scopes such as `abc`, `profile`, or `ets-transform`. Keep commits focused on one behavioral change.

Pull requests should include a short problem statement, implementation summary, tests run, and links to relevant specs or issues. Include artifact notes only when packaging, CLI behavior, or user-visible output changes.
