---
name: rust-architecture-standards
description: Mandatory Rust workspace, crate architecture, Cargo dependency, performance, memory, path-handling, OXC-style, Rolldown-style, no-standalone-function, useful-test-only, no-cfg-test-in-generated-code, and obsolete-crate deletion standards. Use whenever Codex edits, reviews, audits, plans, or refactors Rust code, Cargo.toml files, Rust workspaces, crates, plugin systems, compiler/bundler pipelines, or docs that define Rust architecture rules. Trigger for Rust tasks mentioning crates, workspace dependencies, cargo, clippy, fmt, tests, cfg(test), OXC, Rolldown, bundler, plugin, resolver, build context, session, filesystem paths, function ownership, file layout, performance, memory, parallelization, obsolete crates, or breaking refactors.
---

# Rust Architecture Standards

## Priority

Follow user instructions, system/developer instructions, repository
`AGENTS.md`, and crate-local design docs first. Apply this skill as the
repository mandatory baseline whenever those instructions leave room. If a
repository rule is stricter, follow the stricter rule. If this skill conflicts
with a higher priority instruction, state the conflict briefly and follow the
higher priority instruction.

## Workflow

1. Read the relevant repository rules before deciding architecture.
2. Inspect the owning crate, neighboring modules, root `Cargo.toml`, and current
   dependency graph before editing or writing a plan.
3. Preserve behavior unless the user explicitly requests breaking internal API
   cleanup.
4. Treat architecture plans as implementation contracts, not suggestions.
5. When editing, implement the rule fully at the touched boundary; do not leave
   compatibility wrappers or duplicate paths unless another production caller
   still requires them.
6. Run the narrowest useful verification command, then broaden to `cargo test`,
   `cargo clippy -- -D warnings`, and `cargo fmt` when the blast radius
   justifies it.

## Cargo And Crates

- Keep third-party and internal dependency versions centralized in the root
  `[workspace.dependencies]`.
- Crate manifests under `crates/*` use `{ workspace = true }`; no direct
  `path`, `version`, or `git` entries.
- Keep crate dependencies acyclic. Use `cargo metadata` or manifest inspection
  when crate-boundary changes are involved.
- Foundation crates contain types, errors, paths, schemas, and low-level data
  structures only.
- Domain crates own one domain each: resolver, session, profile, resource,
  cache, pack, sign, ABC, transform, watcher, CLI.
- CLI crates compose stages. Domain crates do not depend on CLI crates.
- Bundler crates do not depend on concrete plugin crates.
- Do not create or keep standalone adapter crates for built-in bundler
  behavior. Built-in Rollup-to-Rolldown migrations belong inside the bundler
  crate's OXC/Rolldown-style plugin modules unless a crate owns an independent
  domain.
- Move logic to the owning domain crate or to a named
  foundation crate such as `path`, `schema`, `session`, `cache`, or
  `fingerprint`.
- Delete obsolete crates, workspace members, dependency declarations, imports,
  tests, and compatibility wrappers when their production behavior has moved
  and tests cover the replacement.

## Ownership And API Shape

- Every non-trivial operation has an owning type: builder, resolver, compiler,
  processor, planner, store, writer, pass, runner, or executor.
- Free functions are allowed only for pure local algorithms that do not read
  files, write files, cache state, inspect global config, mutate artifacts, or
  encode stage lifecycle.
- Long parameter chains carrying build state are forbidden. Use state structs,
  plans, snapshots, contexts, and stage state objects.
- Shared build data is immutable and held behind `Arc`.
- Mutable state stays inside the current builder, stage, pass, or writer.
- Cross-stage mutation goes through an explicit state object.
- Constructors accept typed contexts or narrow plans, not raw strings for
  project roots, module roots, specifiers, or artifact paths.
- Public exports are minimal. Use `pub(crate)` or private items for internal
  implementation. Crate roots do not wildcard-export implementation modules.
- `mod.rs` files declare modules and re-export narrow APIs only.

## Function And Module Layout

- Standalone functions are forbidden for production build behavior. Do not add
  free functions that own IO, lifecycle, mutation, caching, artifact writing,
  resolver policy, transform policy, command construction, or stage execution.
- Free functions are allowed only for pure local algorithms that do not read
  files, write files, cache state, inspect global config, mutate artifacts, or
  encode lifecycle. If the function needs build state, make it a method on the
  owning builder, resolver, compiler, processor, planner, store, writer, pass,
  runner, adapter, factory, snapshot, or inventory.
- Before adding or moving code, inspect the relevant OXC and Rolldown module
  layout and mirror their style where it fits: small owner modules, narrow
  `mod.rs` exports, explicit pass/adapter/runner/factory types, and tests near
  the owning logic.
- New modules must have one clear owner and one clear lifecycle role. Avoid
  parking behavior in detached `*_util.rs`, `helpers.rs`, temporary bridge
  modules, compatibility shims, or modules whose only purpose is to preserve old
  architecture names.
- File names should describe the owning concept (`resolver`, `pass`, `runner`,
  `planner`, `writer`, `adapter`, `factory`, `snapshot`, `inventory`) rather
  than an implementation accident or upstream legacy task name.
- When removing an obsolete crate or module, remove all workspace manifest
  entries, dependency declarations, re-exports, tests, fixture wiring, and
  documentation that would keep the obsolete boundary alive.

## Build Context And IO

- Startup discovery IO belongs to a session, context, inventory, or planner
  layer.
- Read project manifests, package manifests, SDK metadata, profile files,
  resource inventories, and dependency roots once per build session.
- Later stages consume snapshots, typed plans, source-store entries, or
  fingerprints.
- Plugin hooks and hot transforms do not read project/package/profile/SDK files
  or walk dependency roots.
- Source reads during bundling come from hook input or a source store.
- Cache keys are built from explicit content fingerprints and typed input
  inventories. Placeholder hashes are forbidden.
- Directory traversal results are sorted before affecting artifacts, cache keys,
  diagnostics, or tests.
- Library crates expose typed errors. Use `anyhow` in CLI and tests only.

## Tests

- Do not generate useless tests. Every new test must protect real behavior,
  a regression, a crate boundary, a path/cache/determinism invariant, or a
  migration deletion contract.
- Do not add tests just to exercise trivial getters, constructors, enum display
  output, or implementation details that do not protect behavior.
- Do not add `#[cfg(test)]`, `mod tests`, test-only helper modules, or similar
  conditional test code inside generated production source files.
- Place tests in existing integration/unit test locations that match the crate's
  established layout, or in dedicated test fixture/harness files when behavior
  crosses module/crate boundaries.
- If production code needs reusable test fixtures or builders, keep them in
  test-only files outside generated production modules and avoid leaking them
  into public APIs.
- When deleting obsolete crates/modules, update or delete tests that only
  preserve the old boundary. Replacement tests must cover the new owner and
  behavior, not the compatibility wrapper.

## Paths

- Use `Path`, `PathBuf`, or repository-specific typed path wrappers for
  filesystem paths.
- Use dedicated types for module specifiers, runtime URLs, and artifact slash
  paths. Do not mix these as `String`.
- `.to_string_lossy()` is allowed only for diagnostics and final artifact
  serialization.
- Identity, resolver matching, cache keys, package matching, and dependency
  graph keys do not use lossy path strings.
- Inline slash conversion with `.replace('\\', "/")` is forbidden outside the
  path serializer.
- String checks such as `starts_with('/')`, `contains("oh_modules")`, and path
  classification by substring are forbidden.
- Existing input paths are canonicalized only at path-resolution boundaries.
- Output paths that do not exist are lexically normalized, not canonicalized.
- Cross-platform tests cover Unix, Windows drive paths, UNC paths,
  non-UTF-8 display boundaries where supported, path separators, and component
  classification.

## Performance And Memory

- Prefer OXC/Rolldown-aligned high-performance primitives in compiler and
  bundler code.
- Use `oxc_index::IndexVec` and typed dense IDs for dense graphs, chunks,
  modules, resources, profiles, source files, compile units, pack entries, and
  bytecode tables. Do not introduce `index_vec`.
- Use `rustc_hash::FxHashMap`/`FxHashSet` for internal lookup tables.
- Never serialize nondeterministic hash-map iteration directly. Convert through
  typed IDs, sorted vectors, or semantic `IndexMap`.
- Use `Arc<str>` for stable shared snapshot strings.
- Use `Box<str>` for owned schema/config strings that are not shared.
- Use compact string types for hot short identifiers when the workspace already
  uses them or the dependency is added at the workspace root.
- Keep resource binary contents out of long-lived session snapshots. Store path,
  stat metadata, and fingerprint; stream bytes in writers.
- Avoid cloning large structs across stages. Pass typed IDs, references, plans,
  or `Arc` snapshots.
- Avoid `Arc<Mutex<...>>` collectors in plugin or bundler hot paths. Use
  per-task outputs and deterministic merge by typed ID.
- Centralize parallelism in explicit inventory builders, runners, stage
  executors, or batch APIs.
- Parallel work returns `(TypedId, Output)` or `(TypedId, Diagnostic, Output)`.
  Merge by typed ID before serialization or artifact writing.
- Final writers that define binary or archive order run sequentially or perform
  a sequential final merge.
- Verify deterministic output across single-threaded and multi-threaded runs
  when parallelism reaches artifacts.

## OXC And Rolldown Style

- Use OXC parser, AST visitors, builders, spans, allocator discipline, and
  codegen for JavaScript, TypeScript, and ArkTS parsing/transforms whenever
  available.
- Match OXC/Rolldown code organization before inventing new local structure:
  transforms belong in pass-oriented modules, bundler hook behavior belongs in
  Rolldown plugin adapter modules, and orchestration belongs in explicit
  coordinator/runner modules.
- Do not persist OXC allocator-backed AST data into sessions, caches, or
  long-lived build state.
- Persist transform output as owned code, source maps, diagnostics, spans, or
  typed metadata.
- Raw source rewriting is forbidden when an OXC AST/codegen path exists.
- If raw patches are required for upstream parity, represent them as span
  patches and apply them in deterministic order.
- Built-in plugin adapters stay thin: hook input plus context goes to a domain
  operation, then hook output returns.
- Plugin `HookUsage` lists only hooks actually implemented.
- Concrete built-in plugin construction belongs to the bundler crate's internal
  factory when the adapter is part of fixed bundler behavior. CLI/build planners
  pass typed plans; they do not assemble ad hoc plugin crates.

## Review And Planning Output

- Lead with hard blockers, crate-boundary violations, repeated IO, path
  portability failures, performance risks, and missing tests.
- For plans, specify the final crate/file ownership, dependency direction,
  execution flow, migration phase, verification command, and deletion target.
- Use mandatory language: `must`, `forbidden`, `delete`, `move`, `own`,
  `consume`, `verify`.
- Do not frame architecture requirements as optional suggestions.

## Guardrail Searches

Use these scans when relevant:

```bash
rg -n "index_vec|Arc<Mutex|Mutex<|read_to_string|fs::read|read_dir|WalkDir" crates Cargo.toml
rg -n "starts_with\\('/'\\)|contains\\(\"oh_modules\"\\)|replace\\('\\\\\\\\', \"/\"\\)|to_string_lossy" crates
rg -n "\\butils\\b|\\bhelpers\\b|\\bcommon\\b|\\bmisc\\b" crates
cargo metadata --format-version 1 --no-deps
```
