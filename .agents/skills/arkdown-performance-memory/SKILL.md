---
name: arkdown-performance-memory
description: Mandatory arkdown performance, memory ownership, parallelization, deterministic merge, dense ID, source snapshot, cache fingerprint, plugin hot-path, repeated I/O, and Arc<Mutex> rules.
---

# Arkdown Performance And Memory

## Scope

Use these rules when changing startup discovery, build sessions, OXC passes,
Rolldown plugins, resource/profile processing, ABC, packing, cache, watch,
parallel execution, or long-lived build data.

## Data And Ownership

- Use `oxc_index::IndexVec` and typed dense IDs for dense module, dependency,
  source, resource, profile, compile-unit, pack-entry, and bytecode collections.
  Do not add `index_vec`.
- Use `FxHashMap`/`FxHashSet` for internal lookup and `IndexMap` only when
  semantic order requires it. Never serialize hash iteration directly.
- Use `Arc<str>` or `Arc<[u8]>` for shared immutable source data, `Box<str>`
  for non-shared schema/config strings, and `String` for mutation or final
  serialization.
- Do not keep allocator-backed OXC AST data in sessions, caches, or other
  long-lived state. Persist owned code, maps, spans, diagnostics, or typed facts.
- Do not retain resource binary contents in `BuildSession`; stream them in the
  owning writer.
- Avoid cloning large configs, manifests, source strings, or graph objects
  across stages. Pass typed IDs, references, plans, or immutable snapshots.

## I/O And Snapshots

- `arkdown-session::BuildSessionBuilder` owns startup discovery and constructs
  the immutable input snapshot. Later stages must not repeat project, package,
  profile, resource, SDK, or source discovery.
- Plugin hooks and hot transforms must not call `std::fs`, `fs_err`,
  `read_dir`, `WalkDir`, manifest loaders, or SDK loaders.
- Cache keys must use real content and environment fingerprints from the
  session. Placeholder hashes and existence-only keys are forbidden.
- Package installation and registry access are outside the arkdown build
  lifecycle. The session snapshots only already-materialized dependencies.

## Parallel Execution

- Parallel work is owned by `BuildSessionBuilder`, `BuildCoordinator`, or a
  named domain batch type. Do not create a second scheduler or Rust CLI
  executor.
- Parallelize only independent work with disjoint mutable outputs. Respect the
  dependency graph and artifact handoff order observed in upstream source.
- Each job returns `(TypedId, Output)` or `(TypedId, Diagnostic, Output)`.
  Merge by typed ID before diagnostics, serialization, cache commit, or writes.
- Each OXC task owns its allocator. Do not share mutable AST or semantic state
  between workers.
- Detect duplicate output destinations before parallel writes.
- Keep semantic-order writers sequential, or use a deterministic final merge:
  loader/profile JSON, source maps, ABC table layout, archive entries, signing,
  and cache manifest commit.

## Rolldown Plugin Hot Path

- Plugin structs must not use `Arc<Mutex<Vec<_>>>`,
  `Arc<Mutex<FxHashMap<...>>>`, or equivalent shared-write collectors.
- Hooks emit typed facts keyed by module/source ID. The owning stage merges
  facts after Rolldown completes and before artifact writing.
- A plugin adapter contains hook translation only; discovery and business
  processing belong to the session or owning domain type.

## Verification

- Compare artifacts with `RAYON_NUM_THREADS=1` and a multi-threaded setting;
  bytes, ordering, cache keys, and diagnostics must match.
- Read-count tests must prove startup-owned inputs are not re-read by stages or
  hooks.
- Incremental tests must prove a leaf change invalidates only its source-backed
  reverse dependency closure.
- Resource memory tests must prove large binary inputs are streamed.
- Use targeted scans when relevant:

```sh
rg -n 'index_vec|Arc<Mutex|Mutex<|read_to_string|fs::read|read_dir|WalkDir' crates Cargo.toml
```
