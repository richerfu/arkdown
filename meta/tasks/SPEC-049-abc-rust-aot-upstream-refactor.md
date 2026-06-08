# SPEC-049: Rust ABC AOT Backend Upstream-Chain Refactor

**Crate**: `arkdown-abc`, `arkdown-ts2abc`, `arkdown-cli`
**Priority**: P1
**Blocked by**: SPEC-009, SPEC-025, SPEC-028
**Related**: SPEC-048
**Status**: Proposed

---

## Summary

Refactor the Rust ABC backend so its production architecture follows the
upstream `es2abc` AOT compile chain, while keeping the implementation in Rust.

The current `arkdown-abc` backend has accumulated many upstream parity fixes,
but its main compile path still lowers source directly into final `AbcFile`
records. Upstream does not work that way. Upstream builds `SourceFile` inputs,
compiles each input into `pandasm::Program`, stores programs in `programsInfo`,
optionally restores or writes ProgramCache `.protoBin` files, adds npm entry
programs, resolves dependency reachability, then emits final single or merged
ABC files through `AsmEmitter`.

This SPEC requires the Rust backend to use the same stage boundaries:

```text
Options / SourceFile
  -> CompileFileQueue
  -> ProgramCache lookup
  -> Source or ABC input Program compile
  -> ProgramRegistry
  -> npm entry Program
  -> dependency relation resolution
  -> Program optimization
  -> ProgramCache emit
  -> single / merged ABC emit
```

The refactor must not keep two production ABC compile architectures alive. When
the upstream-shaped AOT runner owns a flow, the old direct `AbcFile` flow must be
deleted.

---

## Upstream Source Map

The local upstream source tree used for this SPEC is:

```text
/Users/ranger/Desktop/project/ohos-rs/arkcompiler_ets_frontend
```

Authoritative upstream files:

| Upstream file | Role |
| --- | --- |
| `es2panda/aot/options.cpp` | CLI options, `SourceFile` collection, filelist parsing, cache manifest parsing, compile context parsing |
| `es2panda/aot/main.cpp` | AOT runner, `programsInfo`, expected program count, npm entries, deps resolution, output generation |
| `es2panda/compiler/core/compileQueue.cpp` | source/ABC compile jobs, ProgramCache lookup, optimization, ABC input split |
| `es2panda/aot/emitFiles.cpp` | ProgramCache emit jobs, single ABC emit, merged ABC emit |
| `es2panda/util/moduleHelpers.cpp` | `--npm-module-entry-list` Program construction and cache lookup |
| `es2panda/aot/resolveDepsRelation.cpp` | dependency relation collection and BFS reachability |
| `es2panda/compiler/core/emitter/emitter.cpp` | Program record, function, debug info, module metadata generation |

Rust implementation must cite these files in tests and design comments where
behavior is not obvious.

---

## Non-Negotiable Invariants

1. `arkdown-abc` owns ABC domain logic. `arkdown-ts2abc` is only a facade for
   upstream-compatible CLI arguments.
2. Source and ABC inputs compile into a Rust `Program` model before final ABC
   emission. Direct source-to-final-`AbcFile` production paths are forbidden.
3. `AbcFile` is the final binary layout accumulator only.
4. Program order is deterministic and must match upstream `programsInfo`
   iteration semantics for all golden fixtures.
5. Final ABC table layout is a sequential boundary. Compile jobs may run in
   parallel only if their outputs merge by typed ID or upstream-equivalent key.
6. Cache keys must match upstream byte-for-byte for the same input, SDK bytecode
   version, source metadata, and ABC input metadata.
7. Cache hits must skip the same compile stages that upstream skips.
8. `--outputProto`, `--npm-module-entry-list`, `--enable-abc-input`,
   `--compile-context-info`, `--remove-redundant-file`, and `--dump-deps-info`
   must be implemented as upstream-shaped AOT features, not special cases in
   final ABC serialization.
9. Existing minimal-hap byte parity must not regress.

---

## Final Rust Ownership

Target `arkdown-abc` layout:

```text
crates/arkdown-abc/src/
  lib.rs
  compiler.rs                 # public facade only
  aot/
    mod.rs
    options.rs                # AotCompileOptions, SourceFile, ScriptKind
    runner.rs                 # AotRunner, Run-equivalent orchestration
    compile_queue.rs          # CompileFileScheduler, CompileFileJob
    program_registry.rs       # ProgramRegistry, ProgramKey, expected counts
    cache_store.rs            # ProgramCacheStore, cache hash and proto I/O
    npm_entries.rs            # NpmModuleEntryCompiler
    abc_input.rs              # ABC input -> Program flow
    deps.rs                   # dependency relation and redundant removal
    output.rs                 # AotOutputGenerator
    diagnostics.rs            # upstream-shaped errors and messages
  program/
    mod.rs
    ids.rs                    # ProgramId, RecordId, FunctionId, LiteralId
    model.rs                  # Program, Record, Function, Field, LiteralArray
    dump.rs                   # normalized Program dump for oracle tests
    from_abc.rs               # ABC reader to Program conversion
    to_abc.rs                 # ProgramRegistry to AbcFile conversion
    proto.rs                  # protoPanda Program conversion
  ir/
    ...                       # source lowering now returns Program
  emit/
    ...                       # final AbcFile serializer retained
```

Target `arkdown-ts2abc` layout:

```text
crates/arkdown-ts2abc/src/
  args.rs                     # upstream CLI-compatible parsing only
  driver.rs                   # builds AotCompileRequest and calls arkdown-abc
  compile_context.rs          # parse compile-context-info into typed input
  version.rs
  error.rs
```

`arkdown-cli` must construct the same `AotCompileRequest` through a typed
CompileArkTS handoff. It must not call lower-level ABC internals.

---

## Phase 0: Oracle Harness and Fixture Freeze

### Upstream Logic

Upstream can emit or dump multiple comparison layers:

- final ABC through `AsmEmitter::EmitPrograms`;
- Program snapshots through `--outputProto`;
- normalized pandasm Program through dump flags;
- ProgramCache `.protoBin` through `--cache-file`.

### Rust Implementation

Create `crates/arkdown-abc/tests/oracle/` with two modes:

1. **Committed golden mode**
   - Reads fixtures committed under
     `crates/arkdown-abc/tests/fixtures/upstream-aot/`.
   - Does not require upstream binaries in CI.
   - Compares Rust output against committed Program dumps, cache hashes, ABC
     section hashes, and selected exact byte outputs.

2. **Local upstream mode**
   - Enabled by `ARKDOWN_UPSTREAM_ES2ABC=/path/to/es2abc`.
   - Generates fresh upstream outputs in temp dirs.
   - Compares Rust output against local upstream at runtime.
   - Skips tests when the env var is missing.

Required helper binaries/scripts:

- `tests/tools/run_upstream_es2abc.rs` or shell-free Rust test helper that
  invokes upstream with exact argv.
- `tests/tools/compare_program_dump.rs` for normalized Program diffs.
- `tests/tools/compare_abc_sections.rs` for per-section hashes and exact method
  CodeItem / DebugInfo / LNP comparison.

### Test Requirements

Oracle fixtures must cover:

| Fixture | Flags | Required comparisons |
| --- | --- | --- |
| single script source | none / `--output` | Program dump, ABC header, CodeItem |
| module source | `--module --merge-abc` | module record literal, final ABC |
| filelist source | `@filesInfo.txt --merge-abc` | SourceFile fields, Program order, ABC |
| commonjs source | `--commonjs` and filelist `commonjs` | commonjs metadata and function args |
| output proto | `--outputProto` | `.protoBin` and absence of final ABC |
| source cache | `--cache-file @modules.cache` | hash, hit/miss behavior, proto bytes |
| npm entries | `--npm-module-entry-list` | Program dump, cache, final record shape |
| ABC input | `--enable-abc-input` | `abc|record` keys, merge consistency |
| dependency pruning | `--compile-context-info --remove-redundant-file` | retained Program set and final ABC |
| debug info | `--debug-info` | DebugInfo item, LNP bytes, method source file |
| release opt | `--opt-level 2` | optimized Program dump, CodeItem |

Acceptance for Phase 0: oracle helpers exist, at least three upstream fixtures
run in both committed and local-upstream modes.

---

## Phase 1: Upstream-Shaped Options and SourceFile Model

### Upstream Logic

`Options::Parse` determines:

- `scriptKind_` from `--module`, `--commonjs`, or default script mode;
- `compilerOutput_` from `--output`, base64 output, or input basename;
- `recordName_` from `--record-name` or output basename in merge mode;
- input kind from normal file, `@filelist`, directory, or base64 input;
- `SourceFile` fields from filelist columns:
  `[filePath; recordName; moduleKind; sourceFile; pkgName; isSharedModule; sourceLang]`;
- cache mapping from `--cache-file` bare path or `@modules.cache`.

See upstream `options.cpp`:

- `CollectInputSourceFile`;
- `CollectInputAbcFile`;
- `CollectInputFilesFromFileList`;
- `ParseCacheFileOption`;
- option finalization in `Options::Parse`.

### Rust Implementation

Implement `aot/options.rs`:

```rust
pub struct AotCompileOptions {
    pub source_files: IndexVec<SourceFileId, SourceFile>,
    pub output: AotOutput,
    pub merge_abc: bool,
    pub debug_info: bool,
    pub enable_column: bool,
    pub opt_level: u8,
    pub function_threads: usize,
    pub file_threads: usize,
    pub abc_class_threads: usize,
    pub target_api_version: u32,
    pub target_api_sub_version: Option<Box<str>>,
    pub cache_files: FxHashMap<ProgramKeyInput, PathBuf>,
    pub npm_module_entry_list: Option<PathBuf>,
    pub compile_context_info: CompileContextInfo,
    pub enable_abc_input: bool,
    pub dump_deps_info: bool,
    pub remove_redundant_file: bool,
    pub output_proto: Option<PathBuf>,
    pub module_record_field_name: Option<Box<str>>,
}

pub struct SourceFile {
    pub file_name: PathBuf,
    pub record_name: Box<str>,
    pub script_kind: ScriptKind,
    pub script_extension: ScriptExtension,
    pub source_file: Box<str>,
    pub pkg_name: Box<str>,
    pub is_shared_module: bool,
    pub source_lang: Box<str>,
    pub is_source_mode: bool,
    pub inline_source: Option<Arc<str>>,
    pub hash: Option<u32>,
}
```

Rules:

- Empty filelist fields must be handled like upstream `GetStringItems`: empty
  items are dropped. Tests must document this compatibility behavior.
- Normal source input without filelist must set `is_source_mode` based on
  extension and `--enable-abc-input`.
- `--module` and `--commonjs` conflict before any input processing.
- Debug mode must force `opt_level=0`, matching upstream:
  `compilerOptions_.optLevel = isDebug ? 0 : opOptLevel`.
- `arkdown-ts2abc::ArgParser` must return an `AotCompileRequest`; it must not
  construct `AbcModule`.

### Tests

Unit tests in `arkdown-ts2abc`:

- `parse_filelist_merge_source_columns_match_upstream`.
- `parse_filelist_non_merge_uses_output_column`.
- `parse_abc_filelist_collects_pkg_name_only`.
- `parse_cache_file_bare_path_maps_single_source`.
- `parse_cache_file_manifest_maps_by_source_file_name`.
- `debug_info_forces_opt_level_zero`.
- `module_commonjs_conflict_errors_before_compile`.

Oracle tests:

- Run upstream with the same argv and assert Rust `AotCompileOptions` snapshot
  matches expected SourceFile fields.

---

## Phase 2: Program Model and ProgramRegistry

### Upstream Logic

Upstream stores compile outputs in:

```cpp
std::map<std::string, panda::es2panda::util::ProgramCache *> programsInfo;
```

For source inputs, the key is `src_->fileName`. For ABC inputs split by record,
the key is `abcFileName + "|" + recordName`. For npm entries, the key is the
npm entries list path.

### Rust Implementation

Implement `program/model.rs`:

- `Program`: equivalent to `pandasm::Program`;
- `Record`: name, fields, methods, metadata, language extension, module
  literal references;
- `Function`: full internal name, params, registers, instruction list,
  debug info, catch table, annotations;
- `LiteralArray`: typed literal values with stable indexes;
- `ProgramMetadata`: source mode, generated-from-merged-abc flag, shared flag.

Implement typed IDs:

```rust
oxc_index::define_index_type! {
    pub struct ProgramId = u32;
    pub struct RecordId = u32;
    pub struct FunctionId = u32;
    pub struct LiteralArrayId = u32;
}
```

Implement `aot/program_registry.rs`:

- `ProgramRegistry::expected_count`;
- `insert_source_program(key, ProgramCacheEntry)`;
- `insert_abc_program(key, ProgramCacheEntry)`;
- `insert_npm_program(key, ProgramCacheEntry)`;
- `check_merge_mode_consistency(merge_abc)`;
- stable iteration in upstream-equivalent key order.

`ProgramRegistry` must store insertion IDs and a sorted key index. Use
`IndexVec<ProgramId, ProgramCacheEntry>` plus `BTreeMap<ProgramKey, ProgramId>`
or deterministic sorted vector. Do not serialize `FxHashMap` iteration.

### Tests

- Source keys sort and emit in upstream `std::map` order.
- ABC keys use `file|record` and pass/fail merge consistency like upstream.
- ProgramRegistry expected count adjusts when ABC input produces N records and
  npm entries add one Program.
- Program dump is stable under repeated insertion order permutations.

---

## Phase 3: Source Compile Queue

### Upstream Logic

`CompileFileJob::Run`:

1. reads source file into buffer;
2. stores buffer in `src_->source`;
3. calls `RetrieveProgramFromCacheFiles`;
4. returns early on cache hit;
5. calls `CompileProgram`;
6. calls `OptimizeAndCacheProgram`.

### Rust Implementation

Implement `aot/compile_queue.rs`:

```rust
pub struct CompileFileScheduler<'a> {
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}

pub struct CompileFileJob<'a> {
    source_id: SourceFileId,
    source: &'a SourceFile,
}
```

Implementation logic:

- `run_all()` builds jobs from `options.source_files`.
- For phase-one determinism, execute sequentially and merge by SourceFileId.
  Later parallelism must return `(SourceFileId, CompileFileOutput)`.
- `run_one()`:
  - read file unless `inline_source` is present;
  - compute source/ABC cache hash only after reading bytes;
  - ask `ProgramCacheStore` for a cache hit;
  - if hit, return cached ProgramCacheEntry with `need_update_cache=false`;
  - if source mode, call `SourceProgramCompiler`;
  - if ABC input and merge mode, call `AbcInputProgramCompiler::compile_split`;
  - if ABC input and non-merge, call `AbcInputProgramCompiler::compile_whole`;
  - run `ProgramOptimizer` according to upstream conditions;
  - return ProgramCacheEntry with `need_update_cache=true` only when upstream
    would write cache.

### Tests

- `compile_file_job_cache_hit_skips_source_compiler`.
- `compile_file_job_cache_miss_invokes_source_compiler_once`.
- `compile_file_job_abc_input_routes_to_abc_compiler`.
- `compile_file_scheduler_merges_by_source_file_id`.
- `compile_file_scheduler_output_matches_upstream_program_order`.

---

## Phase 4: Source Program Compiler

### Upstream Logic

`es2panda::Compiler::CompileFile` parses, binds, checks, lowers, emits a
`pandasm::Program`. Program records contain:

- one main source record;
- function table entries;
- record metadata fields such as `moduleRecordIdx`, `isCommonjs`,
  `isSharedModule`, `pkgName@...`;
- debug info and source file metadata;
- module record literal arrays for ESM.

### Rust Implementation

Refactor existing `ir::builder` so it returns `Program`, not final
`ClassRecord`.

Implementation steps:

1. Introduce `SourceProgramCompiler::compile(source_file, source_text, options)`.
2. Build an OXC AST using `script_kind` and `script_extension`.
3. Run upstream-compatible TypeScript/ArkTS transform only when upstream would
   transform that source shape.
4. Build lexical/function/module scopes into owned Rust state. Do not store OXC
   allocator-backed references beyond compile.
5. Lower all functions into `Program.function_table`.
6. Lower the source record into `Program.record_table`.
7. Attach source file, package name, shared flag, source language, module kind.
8. Emit module record metadata into Program literal arrays rather than directly
   into final `AbcFile`.

Current functions that create `ClassRecord` directly must be converted to
create Program records. The final ClassRecord conversion belongs to
`program/to_abc.rs`.

### Tests

Unit:

- variable, function, class, constructor, closure, lexenv, module import/export,
  commonjs, dynamic import, JSON source.

Oracle:

- For each fixture, compare normalized Program dump against upstream
  `--dump-normalized-asm-program`.
- Compare CodeItem for every function, not just final ABC magic.
- Compare module record literal arrays field-by-field.

Negative tests:

- Unsupported constructs must match upstream diagnostics where upstream rejects.
- If Rust intentionally rejects pending constructs, tests must mark the fixture
  as known gap and must not silently emit fallback bytecode.

---

## Phase 5: ProgramCacheStore

### Upstream Logic

Source cache hash:

```text
buffer + fileName + recordName + sourcefile + pkgName + panda_file_version
```

ABC input cache hash:

```text
abcBytes + fileName + recordName + sourcefile + pkgName + panda_file_version
  + updateVersionInfo package versions when relevant
```

Npm entries cache hash:

```text
npmEntriesFileContent
```

### Rust Implementation

Implement `aot/cache_store.rs`:

- `ProgramCacheStore::source_hash`;
- `ProgramCacheStore::abc_input_hash`;
- `ProgramCacheStore::npm_entries_hash`;
- `read_source_program`;
- `write_source_program`;
- `read_abc_programs`;
- `write_abc_programs`;
- `read_npm_program`;
- `write_npm_program`.

Rules:

- Source cache hash must remove current extra `target_api_version` input unless
  an upstream version proves it is included.
- Use upstream `GetHash32String` semantics for source and npm entries. That
  means null-byte handling must match upstream behavior.
- Use upstream `GetHash32` for ABC byte buffers.
- Cache read must tolerate invalid proto and fall back to compile, matching
  upstream "old SDK cache" behavior.
- ProgramCache writes must be scheduled by output phase when upstream schedules
  emit cache jobs.

### Tests

- Compare source hash with upstream for the same fixture.
- Compare ABC input hash with upstream, including package version update info.
- Compare npm entries hash with upstream.
- Cache hit uses cached Program and does not call source lowering.
- Corrupt protoBin is ignored and recompiled.
- Cache from different sourcefile/pkgName/recordName misses.

---

## Phase 6: ABC Input Compiler

### Upstream Logic

ABC input behavior:

- `CollectInputAbcFile` creates a `SourceFile` with `isSourceMode=false`;
- merge mode splits ABC classes into per-record Programs;
- Program keys are `abcFile|recordName`;
- non-merge mode compiles whole ABC input differently;
- replace records, package version updates, bundle name updates, and
  ResourceTable filtering happen before insertion.

### Rust Implementation

Implement `aot/abc_input.rs`:

```rust
pub struct AbcInputProgramCompiler<'a> {
    options: &'a AotCompileOptions,
}
```

Implementation logic:

1. Read ABC bytes.
2. Validate magic and bytecode version.
3. Convert ABC to temporary Program records using existing `abc_reader` and
   `abc2program` support.
4. In merge mode:
   - split each user record into a separate Program;
   - skip global type according to upstream merged/unmerged rules;
   - assign key `fileName|recordName`;
   - set `is_generated_from_merged_abc=true`.
5. Apply `compileContextInfo.replaceRecords[pkgName]`.
6. Apply package version updates to static and dynamic import OHM URLs.
7. Apply bundle name update when `needModifyRecord` is true.
8. Remove duplicate ResourceTable record when package names are modified and
   upstream would filter it.
9. Return `Vec<(ProgramKey, ProgramCacheEntry)>`.

### Tests

Fixtures:

- merged ABC input;
- unmerged ABC input;
- ABC input with static imports;
- ABC input with dynamic imports;
- ABC input with ResourceTable.

Assertions:

- merge-mode consistency errors match upstream.
- `replaceRecords` removes exactly the upstream records.
- package version update changes OHM URLs and string tables.
- `abc|record` keys match upstream Program dump names.
- ABC input ProgramCache stores and restores all per-record Programs.

---

## Phase 7: Npm Module Entry Program

### Upstream Logic

`ModuleHelpers::CompileNpmModuleEntryList`:

- reads the entries file;
- if cache hit, inserts cached Program under key `entriesInfo`;
- otherwise creates one Program;
- each line becomes one record;
- record name is text before `:`;
- field name is text after `:`;
- field type is `u8`;
- field value is `false`;
- inserted Program has `needUpdateCache=true`.

### Rust Implementation

Implement `aot/npm_entries.rs`:

```rust
pub struct NpmModuleEntryCompiler<'a> {
    cache_store: &'a ProgramCacheStore,
}
```

Implementation logic:

1. Read file to string.
2. Compute npm hash.
3. If cache exists and hash matches, return ProgramCacheEntry with
   `need_update_cache=false`.
4. Otherwise parse lines using upstream `line.find(":")` semantics.
5. Build one Program with one Record per line.
6. Return ProgramCacheEntry with `need_update_cache=true`.

Do not insert npm records directly into `AbcFile`.

### Tests

- `record:field` creates one Program record and one `u8=false` field.
- line without colon matches upstream substring behavior.
- empty lines behave like upstream.
- cache hit/miss behavior.
- final merged ABC record shape matches upstream.

---

## Phase 8: Dependency Relation Resolver

### Upstream Logic

`ResolveAndGenerate` runs:

1. `ResolveDepsRelations` when compile context requires it;
2. `GenerateAbcFiles`;
3. inside `GenerateProgram`, `RemoveProgramsRedundantData` if requested.

### Rust Implementation

Implement `aot/deps.rs`:

- `DepsRelationResolver::resolve(&ProgramRegistry, &CompileContextInfo)`;
- `ResolvedDepsRelation`;
- `ProgramRegistry::retain_resolved(&ResolvedDepsRelation)`.

Rules:

- Operate on Program and record data, not final `AbcFile`.
- Static import, dynamic import, commonjs require, JSON records, and ABC input
  records must be collected through the same categories as upstream.
- `dump_deps_info` must use deterministic order and be oracle-comparable.
- `remove_redundant_file` must update expected program count and all cache
  write plans.

### Tests

- static import graph BFS.
- dynamic import graph BFS.
- commonjs require graph BFS.
- JSON/commonjs duplicate record dedup.
- ABC input records are retained or removed by upstream rules.
- `dump_deps_info` text matches upstream.
- final ABC after pruning matches upstream section hashes.

---

## Phase 9: Program Optimizer

### Upstream Logic

`OptimizeAndCacheProgram`:

- source Programs can be optimized;
- global optimization first analyzes all Programs, then optimizes later;
- cache hit Programs are not recompiled;
- opt is skipped when `optLevel == 0`.

### Rust Implementation

Refactor current `optimizer.rs` into Program-level optimizer:

```rust
pub struct ProgramOptimizer {
    options: BytecodeOptimizerOptions,
}

impl ProgramOptimizer {
    pub fn optimize_source_program(&mut self, program: &mut Program);
    pub fn analyze_for_global_optimization(&mut self, program: &Program);
    pub fn optimize_after_analysis(&mut self, registry: &mut ProgramRegistry);
}
```

Rules:

- Delete `record_needs_optimization`.
- Store optimization state on ProgramCacheEntry:
  `need_update_cache`, `from_cache`, `is_source_mode`, `requires_post_analysis_opt`.
- Debug mode must force opt 0 before scheduler starts.
- Try/catch optimization must match upstream `--opt-try-catch-func`.

### Tests

- debug source never optimized.
- release source optLevel 2 optimized.
- cache hit source is not optimized again.
- global optimization analysis and post-analysis optimize order is stable.
- try/catch fixture matches upstream CodeItem.

---

## Phase 10: AOT Output Generator and Program-to-ABC Emit

### Upstream Logic

`GenerateProgram`:

1. dump Program info if requested;
2. for single Program with no output, generate base64;
3. for single Program with outputProto and no global optimization, write proto
   and return;
4. remove redundant records if requested;
5. schedule cache emit jobs;
6. emit single or merged ABC;
7. optionally run transform library.

### Rust Implementation

Implement `aot/output.rs`:

```rust
pub struct AotOutputGenerator<'a> {
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}
```

Implementation logic:

- `generate(registry)` implements upstream `GenerateProgram`.
- `emit_cache_jobs` writes ProgramCache files for entries with
  `need_update_cache=true` and matching cache path.
- `emit_single_abc` emits one ABC per Program in non-merge mode.
- `emit_merged_abc` converts all Programs into one `AbcFile`.
- `emit_base64` serializes exactly one Program and returns stdout payload to
  caller.
- `emit_output_proto` writes proto and returns before ABC emission.

Implement `program/to_abc.rs`:

- Convert ProgramRegistry to `AbcFile` in upstream-equivalent order.
- Preserve string table, literal table, class index, method index, debug info,
  LNP, annotations, module metadata.
- Final layout is sequential.

### Tests

- base64 single source.
- outputProto early return.
- non-merge multiple outputs.
- merge ABC output.
- cache files are written before merged ABC generation when upstream does so.
- final `modules.abc` byte-for-byte matches upstream for minimal fixtures.
- section-level diff gives precise failure categories.

---

## Phase 11: Facade Migration and Deletion

### Implementation

`AbcCompiler` remains public but becomes a facade:

```rust
impl AbcCompiler {
    pub fn compile_with_program_cache(...) -> Result<AbcArtifactResult, AbcError> {
        AotRunner::new(request).run()
    }
}
```

Deletion targets:

- old direct `build_abc_file` production path;
- direct source-to-`ClassRecord` production compile;
- direct npm entry insertion into `AbcFile`;
- `record_needs_optimization`;
- ad hoc dependency pruning on final `AbcFile`;
- ABC input append shortcut as a complete compile path.

The reader/converter internals may remain if they are low-level helpers used by
the new ABC input Program compiler.

### Tests

- Public API compatibility tests for `AbcCompiler`.
- `arkdown-ts2abc` CLI facade tests.
- `arkdown-cli` build tests.
- No production code path calls deleted functions.

Guardrail:

```bash
rg -n "build_abc_file|record_needs_optimization|load_npm_entry_records" crates/arkdown-abc/src
```

After migration, matches must either be gone or test-only.

---

## Full Test Strategy

### Test Layers

1. **Unit tests**
   - Option parsing.
   - SourceFile construction.
   - ProgramRegistry ordering.
   - Cache hash calculation.
   - npm entry parser.
   - ABC input split.
   - dependency resolver.
   - Program optimizer.

2. **Program oracle tests**
   - Source -> Rust Program dump.
   - Source -> upstream normalized Program dump.
   - Diff normalized text.

3. **Cache oracle tests**
   - Compare hash values with upstream.
   - Compare `.protoBin` decode and normalized Program.
   - Verify hit/miss stage skipping.

4. **ABC structure tests**
   - Header.
   - String table.
   - Literal arrays.
   - Class records.
   - Method records.
   - CodeItem bytes.
   - DebugInfo items.
   - LineNumberProgram bytes.
   - Trailing LNP index alignment.

5. **End-to-end fixture tests**
   - minimal-hap debug.
   - minimal-hap release.
   - dynamic import.
   - ByteCode HAR.
   - widget ABC.
   - source maps/debug.
   - ABC input merge.

6. **Determinism tests**
   - `RAYON_NUM_THREADS=1`.
   - `RAYON_NUM_THREADS=8`.
   - Compare final ABC and Program dumps.

7. **Failure parity tests**
   - invalid filelist.
   - missing cache manifest.
   - unsupported flags.
   - merge/non-merge ABC mismatch.
   - invalid ABC input.
   - upstream-rejected syntax.

### Fixture Matrix

| Category | Fixtures |
| --- | --- |
| basic JS | literals, vars, calls, returns, binary/unary ops |
| functions | nested functions, closures, arrow, async, generator |
| classes | default constructor, explicit constructor, fields, static blocks, private fields |
| modules | import, export, re-export, namespace import, dynamic import, TLA |
| commonjs | require, module.exports, exports alias, CJS filelist kind |
| TypeScript | enum, namespace, import equals, declare erasure |
| ArkTS | transformed ETS cache TS, component-generated closures |
| debug | line/column, return positions, constructor positions, local variables |
| cache | source cache, ABC cache, npm cache, corrupt cache |
| ABC input | merged, unmerged, package version update, replace records |
| deps | static/dynamic dependency pruning, HSP external packages |

### Required Commands

Narrow checks:

```bash
cargo test -p arkdown-abc aot_options
cargo test -p arkdown-abc program_registry
cargo test -p arkdown-abc program_cache
cargo test -p arkdown-ts2abc
```

Oracle checks:

```bash
cargo test -p arkdown-abc upstream_aot_golden
ARKDOWN_UPSTREAM_ES2ABC=/path/to/es2abc cargo test -p arkdown-abc upstream_aot_oracle
```

Integration checks:

```bash
cargo test -p arkdown-abc
cargo test -p arkdown-cli hvigor_parity
cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap --disable-abc-cache
```

Determinism:

```bash
RAYON_NUM_THREADS=1 cargo test -p arkdown-abc upstream_aot_golden
RAYON_NUM_THREADS=8 cargo test -p arkdown-abc upstream_aot_golden
```

Workspace checks:

```bash
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo metadata --format-version 1 --no-deps
```

---

## Migration Acceptance Criteria

The refactor is complete only when all criteria pass:

1. `arkdown-abc` production compile path is `AotRunner`.
2. Source compile returns `Program` before final ABC emission.
3. ABC input compile returns Program entries keyed like upstream.
4. npm entries compile into a Program and support ProgramCache.
5. ProgramCache source, ABC input, and npm hash behavior matches upstream.
6. `--outputProto` returns before final ABC emission for the upstream-supported
   case.
7. Dependency pruning operates on ProgramRegistry.
8. Final merged ABC output remains byte-identical for existing minimal-hap
   parity fixtures.
9. New oracle fixtures compare Program dump, cache proto, CodeItem, DebugInfo,
   LNP, and final ABC.
10. Deleted old direct `AbcFile` production paths cannot be found by guardrail
    searches.

---

## Implementation Order

1. Add oracle harness and committed fixtures.
2. Add `AotCompileOptions` and `SourceFile`; migrate `arkdown-ts2abc` parsing.
3. Add Program model and ProgramRegistry.
4. Convert source lowering to return Program.
5. Add ProgramCacheStore and source cache parity.
6. Add npm entry Program compiler and cache.
7. Add ABC input Program compiler.
8. Move dependency pruning to ProgramRegistry.
9. Move optimization to Program level.
10. Add AotOutputGenerator and Program-to-ABC emitter.
11. Migrate `AbcCompiler` facade.
12. Migrate `arkdown-cli` to `AotCompileRequest`.
13. Delete old direct `AbcFile` paths.
14. Expand oracle and end-to-end tests until all acceptance criteria pass.

Each step must land with tests for that step. No step may introduce an
uncovered compatibility wrapper that keeps the previous architecture alive.
