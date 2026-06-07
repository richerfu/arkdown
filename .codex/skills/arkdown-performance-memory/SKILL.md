---
name: arkdown-performance-memory
description: Mandatory arkdown performance, memory ownership, high-performance dependency, and parallelization rules. Use when Codex edits, reviews, plans, or audits arkdown performance, memory use, parallel execution, rayon, oxc_index, index_vec removal, dense IDs, source store memory, cache fingerprints, startup inventory, plugin collectors, Arc<Mutex>, repeated reads, resource streaming, deterministic output, build waves, or stage executor behavior.
---

# Arkdown Performance Memory

## Data Structures

- Remove `index_vec`; use `oxc_index::IndexVec` and typed dense IDs.
- Use typed IDs for modules, chunks, packages, dependency nodes, profile files,
  resource files, source files, SDK files, compile units, pack entries, ABC
  records, functions, literal arrays, OHPM packages, cache entries, and registry
  requests.
- Dense collections use `IndexVec<Id, T>`.
- Lookup side tables use `FxHashMap<Key, Id>`.
- Do not serialize hash-map iteration directly.
- Use `IndexMap` only when semantic order or golden artifact order requires it.

## String And Byte Ownership

- Use `Arc<str>` for long-lived shared snapshot strings.
- Use `Box<str>` for owned non-shared schema/config strings.
- Use compact string types for hot short identifiers when the dependency exists
  at the workspace root.
- Use `String` for mutation buffers, serde scratch values, diagnostics, and
  final serialization.
- Use `Arc<[u8]>` for shared raw source bytes and `Arc<str>` for validated
  source text.
- Do not store OXC allocator-backed AST data in sessions, caches, or long-lived
  build state.
- Do not store resource binary contents in `BuildSession`; stream them in
  resource and pack writers.

## Parallel Execution

Parallelism is centralized in:

- `arkdown-session::ParallelInventoryBuilder`
- `arkdown-cli::StageExecutor`
- explicit domain batch APIs

Every parallel job returns `(TypedId, Output)` or `(TypedId, Diagnostic,
Output)`. Merge by typed ID before serialization, diagnostics, cache key
construction, or artifact writing.

## Startup Parallelization

Build session startup parallelizes:

- module manifest reads/parses/fingerprints
- package manifest reads/parses/fingerprints
- profile JSON inventory
- resource root scanning
- SDK component/API metadata
- source file inventories
- dependency node parsing
- OHPM local install lookup, global cache lookup, lockfile lookup, and bounded
  registry acquisition
- content hashing when cache mode requires it

Project root selection and build-profile precedence remain sequential.
Dependency graph topological ordering has a deterministic final merge.
Offline mode schedules no network tasks. Prefer-offline resolves cache hits
before registry requests. Cache installation for the same package/version is
serialized by cache entry ID.

## Stage Parallelization

- `BuildRunner` executes modules in dependency waves.
- Modules in the same wave run in parallel only when outputs, cache keys, and
  package destinations are disjoint.
- Resource scanning and profile source processing run in parallel after
  `ModuleCompileContext` exists.
- Resource copy/compile runs by `ResourceEntryId`; duplicate destinations are
  detected before writes.
- Profile files parse in parallel and merge by dependency graph order plus
  artifact path order.
- ETS transforms run by `SourceFileId` or rolldown module ID; each task owns its
  OXC allocator.
- CompileArkTS/ABC lowering runs by `CompileUnitId` where inputs are
  independent.
- Pack entry discovery and hashing run by `PackEntryId`.

## Sequential Boundaries

Keep these sequential or give them a sequential final merge:

- CLI argument parsing
- build-profile selection
- dependency graph topological order
- loader JSON serialization
- merged profile artifact serialization
- source map concatenation
- artifact path serialization
- ABC final binary table layout
- archive writer order
- signing
- cache database commit
- cache installation for the same OHPM package/version

## Plugin Hot Path

- Plugin hooks must not call `std::fs::*`, `fs_err::*`, `WalkDir`, package
  loaders, profile loaders, or SDK loaders.
- Plugin structs must not own `Arc<Mutex<FxHashMap<...>>>`,
  `Arc<Mutex<Vec<...>>>`, or equivalent hot-path collectors.
- Plugins emit typed facts into stage-owned collectors keyed by typed IDs.
- Bundle outputs merge facts after rolldown completes and before artifact
  writing.

## Verification

- `rg "index_vec" Cargo.toml crates` returns no production dependency or source
  use after migration.
- Fixture output is byte-for-byte identical with `RAYON_NUM_THREADS=1` and
  `RAYON_NUM_THREADS=8`.
- Session read-count tests prove startup inputs are read once.
- Resource memory tests prove binary contents are streamed, not retained in
  `BuildSession`.
- Cache tests prove one module input change does not invalidate unrelated
  dependency waves.
- OHPM tests prove offline mode avoids HTTP, prefer-offline uses cache first,
  registry fetch installs into cache, and tarball extraction is safe and
  deterministic.
