# Repository Guidelines

## Project Structure & Module Organization

`arkdown` is a Rust workspace for building OpenHarmony artifacts (`.hap`, `.hsp`, `.har`) from ArkTS/TypeScript projects. Workspace crates live under `crates/`: shared types in `arkdown-core`, typed paths in `arkdown-path`, OHOS schemas in `arkdown-ohos-schema`, startup snapshots in `arkdown-session`, resolving in `arkdown-resolver`, OXC transforms in `arkdown-ets-transform`, Rolldown bundling and built-in plugin adapters in `arkdown`, and packaging/signing/profile/resource/cache/watch support in matching crates. The npm package and typed N-API service boundary live under `packages/arkdown`.

Separate `arkdown-plugin-*` crates are not part of the final architecture. Rollup plugin behavior migrates to internal Rolldown adapters under `crates/arkdown/src/plugins/`; non-hook business logic belongs in the owning domain crate. Delete obsolete plugin crates, compatibility wrappers, workspace members, dependency declarations, imports, tests, and docs after their final production caller moves.

Design notes and specs are in `meta/design/` and `meta/tasks/`; read the relevant spec before changing a crate. Test plans are in `meta/tests/`. Shared OHOS fixtures and assets are under `tests/fixtures/`.

Repository-level Codex skills live under `.agents/skills/` and are part of the
engineering rules for this repository. When a task matches a skill description,
load and follow that skill before planning or editing. These skills are
mandatory for all contributors and agents, not local preferences:

- `rust-architecture-standards`: Rust workspace, crate, Cargo, path,
  performance, OXC, and Rolldown standards.
- `arkdown-refactor-plan`: current crate-boundary and migration contract when
  broad architecture refactors are involved.
- `arkdown-build-session`: build session, context propagation, snapshots, and
  repeated-read elimination.
- `arkdown-path-portability`: cross-platform path and artifact serialization
  rules.
- `arkdown-performance-memory`: parallel execution, dense IDs, memory ownership,
  and deterministic performance rules.
- `hvigor-parity`: mandatory hvigor source/cache/output comparison workflow for
  all arkdown behavior changes. Use before editing resolver, transform,
  bundling, ABC, resource, packaging, signing, cache, CLI orchestration, or app
  runtime compatibility behavior.
- `hap-verify-install`: HAP device/emulator verification installs must uninstall
  the target bundle before installing the generated package.

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

Rust must not expose a command-line interface: do not add Rust `[[bin]]` targets, `main.rs`, Clap dependencies/derives, argv parsers, CLI install modes, or N-API functions such as `parse*CliArgs`. Do not recreate public process-monitor, runtime-report persistence, or CLI observability lifecycle facades in Rust; build diagnostics and internal structured tracing belong to the typed service implementation, while their CLI presentation and process lifecycle belong to JavaScript. The N-API boundary may expose typed service requests such as `build`, `check`, and `clean`; those are not CLI APIs. Entrypoint changes must preserve the architecture deletion test in `packages/arkdown/__test__/runtime-architecture.test.cjs`.

## Coding Style & Naming Conventions

Rust code uses `rustfmt` defaults and four-space indentation. Crates and modules use kebab-case or snake_case to match Rust/Cargo conventions (`arkdown-ohos-schema`, `module_json.rs`). Types use `PascalCase`, functions and fields use `snake_case`, and CLI flags use kebab-case.

Prefer small crate-local helpers over cross-crate abstractions unless a pattern already exists. Keep generated artifacts and local SDK paths out of committed docs and source.

All dependency declarations must be centralized in the root `[workspace.dependencies]`. Crates under `crates/*` should reference both internal crates and third-party crates with `{ workspace = true }`; do not add direct `path`, `version`, or `git` dependency declarations inside crate manifests.

For implementation style, follow OXC/Rolldown patterns: keep behavior close to the owning type or module, prefer small state objects over long chains of free functions, and use OXC/Rolldown APIs for parsing, resolving, source maps, and code generation whenever available. Generated JavaScript/TypeScript should be produced through AST/codegen or structured serializers; use raw string assembly only when the upstream implementation does the same or no structured API exists.

Standalone functions are forbidden for production build behavior. Free functions are allowed only for pure local algorithms that do not read files, write files, cache state, inspect global config, mutate artifacts, construct commands, encode lifecycle, or execute stages. If logic needs build state, make it a method on the owning builder, resolver, compiler, processor, planner, store, writer, pass, runner, adapter, factory, snapshot, or inventory.

Minimum implementations are forbidden. Do not land placeholder behavior, stub parity, empty adapters, fake cache keys, no-op runners, TODO-only code paths, or "just enough to compile" implementations. A change must either implement the owning behavior fully for its declared scope or explicitly reject unsupported behavior with typed diagnostics and tests/fixtures that protect the rejection.

## Hvigor Parity Rules

arkdown's behavioral target is hvigor. For any change that affects build output,
runtime behavior, generated TypeScript/JavaScript, module resolution, bytecode,
resources, manifests, packaging, signing, cache layout, command construction, or
device startup compatibility, the implementation must be based on observed
hvigor behavior rather than guesses.

Before editing production code, identify the upstream hvigor owner and read the
relevant source under `hvigor/`, `hvigor/ts-readable/`, SDK task code, or the
matching generated cache/output from a real hvigor build. If a failure only
appears in a large app, compare three artifacts before changing behavior:
original source, hvigor-generated output/cache, and arkdown-generated
output/cache. Keep the comparison focused on the failing module or stage, but do
not skip it.

Do not invent fallback behavior to make a sample pass. Do not hardcode project
paths, package layouts, native library declaration directories, cache paths, or
Example-specific locations. Paths and specifiers must come from manifests,
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

For changes affecting generated JSON, loader metadata, manifests, bytecode, resources, or packaging, add focused regression coverage in the owning crate and cross-check the matching `meta/tests/TEST-*.md` scenario.

Do not generate useless tests. New tests must protect real behavior, a regression, a crate boundary, a path/cache/determinism invariant, or a migration deletion contract. Do not add tests only for trivial getters, constructors, enum display output, or implementation details that do not protect behavior.

Do not add `#[cfg(test)]`, `mod tests`, test-only helper modules, or similar conditional test code inside generated production source files. Put tests in existing test locations or dedicated fixture/harness files. When deleting obsolete crates/modules, delete tests that only preserve the old boundary and replace them with coverage for the new owner.

## Commit & Pull Request Guidelines

History uses conventional commit style: `feat(scope): ...`, `fix(scope): ...`, and `docs(scope): ...` with scopes such as `abc`, `profile`, or `ets-transform`. Keep commits focused on one behavioral change.

Pull requests should include a short problem statement, implementation summary, tests run, and links to relevant specs or issues. Include artifact notes only when packaging, CLI behavior, or user-visible output changes.
