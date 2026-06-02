# Repository Guidelines

## Project Structure & Module Organization

`arkdown` is a Rust workspace for building OpenHarmony artifacts (`.hap`, `.hsp`, `.har`) from ArkTS/TypeScript projects. Workspace crates live under `crates/`: shared types in `arkdown-core`, config/resolving in `arkdown-config`/`arkdown-resolver`, the bundler in `arkdown`, plugins in `arkdown-plugin-*`, packaging/signing/profile/resource support in matching crates, and the CLI in `arkdown-cli`.

Design notes and specs are in `meta/design/` and `meta/tasks/`; read the relevant spec before changing a crate. Test plans are in `meta/tests/`. Shared OHOS fixtures and assets are under `tests/fixtures/`.

## Build, Test, and Development Commands

- `cargo build`: build the full workspace.
- `cargo build -p arkdown-core`: build one crate while iterating.
- `cargo test`: run workspace tests.
- `cargo test -p arkdown-resolver`: run tests for one crate.
- `cargo test -p arkdown -- chunk`: run tests matching a name filter.
- `cargo clippy -- -D warnings`: run lints as errors.
- `cargo fmt`: format Rust code.
- `cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap`: run the CLI against the minimal fixture.

Use Cargo directly; there is no `justfile`.

## Coding Style & Naming Conventions

Rust code uses `rustfmt` defaults and four-space indentation. Crates and modules use kebab-case or snake_case to match Rust/Cargo conventions (`arkdown-plugin-json5`, `module_json.rs`). Types use `PascalCase`, functions and fields use `snake_case`, and CLI flags use kebab-case.

Prefer small crate-local helpers over cross-crate abstractions unless a pattern already exists. Keep generated artifacts and local SDK paths out of committed docs and source.

All dependency declarations must be centralized in the root `[workspace.dependencies]`. Crates under `crates/*` should reference both internal crates and third-party crates with `{ workspace = true }`; do not add direct `path`, `version`, or `git` dependency declarations inside crate manifests.

For implementation style, follow OXC/Rolldown patterns: keep behavior close to the owning type or module, prefer small state objects over long chains of free functions, and use OXC/Rolldown APIs for parsing, resolving, source maps, and code generation whenever available. Generated JavaScript/TypeScript should be produced through AST/codegen or structured serializers; use raw string assembly only when the upstream implementation does the same or no structured API exists.

## Testing Guidelines

Place unit tests near the code or in crate-level `tests/` directories, matching the existing `*_tests.rs` pattern. Async plugin tests use `#[tokio::test]`. Shared fixtures belong in `tests/fixtures/`; keep generated build output out of version control.

For changes affecting generated JSON, loader metadata, manifests, bytecode, resources, or packaging, add focused regression coverage in the owning crate and cross-check the matching `meta/tests/TEST-*.md` scenario.

## Commit & Pull Request Guidelines

History uses conventional commit style: `feat(scope): ...`, `fix(scope): ...`, and `docs(scope): ...` with scopes such as `abc`, `profile`, or `ets-transform`. Keep commits focused on one behavioral change.

Pull requests should include a short problem statement, implementation summary, tests run, and links to relevant specs or issues. Include artifact notes only when packaging, CLI behavior, or user-visible output changes.
