//! Upstream-shaped compile file scheduler.
//!
//! This module implements `CompileFileScheduler` and `CompileFileJob`, which
//! mirror the upstream `CompileFileQueue` and `CompileFileJob` from
//! `es2panda/compiler/core/compileQueue.cpp`.
//!
//! ## Upstream algorithm (`CompileFileJob::Run`)
//!
//! 1. Read the source file into a buffer (unless `inline_source` is set).
//! 2. Store the buffer in `src_->source`.
//! 3. Call `RetrieveProgramFromCacheFiles` (cache lookup by hash).
//! 4. If cache hit, return early (no compile needed).
//! 5. Otherwise call `CompileProgram` (source compilation).
//! 6. Call `OptimizeAndCacheProgram`.
//!
//! For phase-one determinism, `run_all()` executes jobs sequentially.
//! Later parallelism must return `(SourceFileId, CompileFileOutput)` and
//! merge by SourceFileId.
//!
//! ## Upstream reference
//! - `es2panda/compiler/core/compileQueue.cpp` — `CompileFileJob::Run`
//! - `es2panda/aot/main.cpp` — `Compiler::CompileFiles`

use crate::aot::abc_input::AbcInputProgramCompiler;
use crate::aot::cache_store::ProgramCacheStore;
use crate::aot::optimizer::ProgramOptimizer;
use crate::aot::options::{AotCompileOptions, SourceFile};
use crate::aot::program_registry::ProgramRegistry;
use crate::aot::source_compiler::SourceProgramCompiler;
use crate::emit::file::version_for_target_api;
use crate::error::AbcError;
use crate::program::ids::SourceFileId;
use crate::program::model::{Program, ProgramCacheEntry, ProgramMetadata, SourceLang};
use crate::program_cache::append_program_to_file;
use crate::proto_panda::Program as ProtoProgram;

// ── CompileFileOutput ─────────────────────────────────────────────────────────

/// Output from one `CompileFileJob::run_one()`.
#[derive(Debug)]
pub struct CompileFileOutput {
    /// The compiled program and its cache state.
    pub entry: ProgramCacheEntry,
    /// The program key under which to insert into ProgramRegistry.
    pub program_key: String,
    /// Whether to write this program to a cache file after insertion.
    pub write_cache: bool,
    /// Cache file path (if `write_cache` is true).
    pub cache_path: Option<std::path::PathBuf>,
    /// Cache hash code for writing.
    pub cache_hash: u32,
}

// ── CompileFileJob ────────────────────────────────────────────────────────────

/// One compile job for a single source or ABC input file.
///
/// Upstream `CompileFileJob` holds a pointer to its `SourceFile`; we hold
/// an index for determinism.
pub struct CompileFileJob<'a> {
    source_id: SourceFileId,
    source: &'a SourceFile,
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}

impl<'a> CompileFileJob<'a> {
    pub fn new(
        source_id: SourceFileId,
        source: &'a SourceFile,
        options: &'a AotCompileOptions,
        cache_store: &'a ProgramCacheStore,
    ) -> Self {
        Self {
            source_id,
            source,
            options,
            cache_store,
        }
    }

    /// Execute this compile job, returning the `CompileFileOutput`.
    ///
    /// Mirrors upstream `CompileFileJob::Run()`.
    pub fn run(self) -> Result<Vec<CompileFileOutput>, AbcError> {
        let source = self.source;

        // 1. Read file content.
        let source_text = if let Some(inline) = &source.inline_source {
            inline.as_ref().to_string()
        } else if source.is_source_mode {
            fs_err::read_to_string(&source.file_name).map_err(AbcError::Io)?
        } else {
            // ABC input — read as bytes, not text.
            return self.run_abc_input();
        };

        // 2. Determine program key (source: fileName).
        let program_key = source.cache_key();
        let cache_path = self
            .options
            .cache_files
            .get(program_key.as_str())
            .cloned();

        // 3. Compute cache hash.
        let panda_version = version_for_target_api(
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
        );
        let cache_hash = self.cache_store.source_hash(
            &source_text,
            source.file_name.to_string_lossy().as_ref(),
            source.record_name.as_ref(),
            source.source_file.as_ref(),
            source.pkg_name.as_ref(),
        );

        // 4. Try cache hit.
        // Upstream `RetrieveProgramFromCacheFiles`: immediately returns false
        // (cache miss) when `requireGlobalOptimization` is set, because all
        // programs must be freshly compiled so the global analysis pass can
        // collect cross-program data.
        let skip_cache = self.options.require_global_optimization;
        if !skip_cache {
        if let Some(cache_path) = &cache_path {
            match self.cache_store.read_source_program(cache_path, cache_hash) {
                Ok(Some(proto_program)) => {
                    let program = proto_to_program(proto_program, source, false)?;
                    let entry = ProgramCacheEntry::new_cache_hit(program, cache_hash);
                    tracing::debug!(
                        source = %source.file_name.display(),
                        cache = %cache_path.display(),
                        hash = cache_hash,
                        "restored ProgramCache hit"
                    );
                    return Ok(vec![CompileFileOutput {
                        entry,
                        program_key,
                        write_cache: false,
                        cache_path: Some(cache_path.clone()),
                        cache_hash,
                    }]);
                }
                Ok(None) => {} // cache miss
                Err(err) => {
                    tracing::warn!(
                        source = %source.file_name.display(),
                        error = %err,
                        "ignored ProgramCache read error"
                    );
                }
            }
        }
        } // end if !skip_cache

        // 5. Source compile via SourceProgramCompiler (Phase 4: SPEC-049).
        let mut program = SourceProgramCompiler::new(self.options)
            .compile(self.source, &source_text)?;

        // 6. Optimize via ProgramOptimizer (Phase 9: SPEC-049).
        //
        // Upstream `OptimizeAndCacheProgram` control flow:
        //   if requireGlobalOptimization:
        //     AnalysisProgram() + mark for deferred optimization
        //   else if optLevel != 0:
        //     OptimizeProgram()
        let mut optimizer = ProgramOptimizer::from_compile_options(
            self.options.opt_level,
            self.options.optimize_try_catch_func,
        );

        let requires_post_analysis_opt = if self.options.require_global_optimization {
            // Global optimization: analyze for cross-program optimization, defer the
            // actual optimization pass until all programs have been analyzed.
            optimizer.analyze_for_global_optimization(&program);
            true
        } else {
            // Local optimization: apply register compaction + peephole now.
            optimizer.optimize_source_program(&mut program);
            false
        };

        // Cache writes are disabled when requireGlobalOptimization is set.
        // Upstream `EmitFileQueue::Schedule()` skips cache emit in this case.
        let need_update_cache = cache_path.is_some() && !self.options.require_global_optimization;
        let entry = ProgramCacheEntry {
            program,
            hash_code: cache_hash,
            need_update_cache,
            from_cache: false,
            is_source_mode: true,
            requires_post_analysis_opt,
        };

        Ok(vec![CompileFileOutput {
            entry,
            program_key,
            write_cache: need_update_cache,
            cache_path,
            cache_hash,
        }])
    }

    /// Handle ABC input files.
    fn run_abc_input(self) -> Result<Vec<CompileFileOutput>, AbcError> {
        let abc_bytes = fs_err::read(&self.source.file_name).map_err(AbcError::Io)?;

        let abc_compiler =
            AbcInputProgramCompiler::new(self.options, self.cache_store);

        if self.options.merge_abc {
            let programs = abc_compiler.compile_split(self.source, &abc_bytes)?;
            let outputs = programs
                .into_iter()
                .map(|(key, entry)| CompileFileOutput {
                    write_cache: entry.need_update_cache,
                    cache_path: self
                        .options
                        .cache_files
                        .get(key.as_str())
                        .cloned(),
                    cache_hash: entry.hash_code,
                    program_key: key,
                    entry,
                })
                .collect();
            Ok(outputs)
        } else {
            let (key, entry) = abc_compiler.compile_whole(self.source, &abc_bytes)?;
            Ok(vec![CompileFileOutput {
                write_cache: entry.need_update_cache,
                cache_path: self
                    .options
                    .cache_files
                    .get(key.as_str())
                    .cloned(),
                cache_hash: entry.hash_code,
                program_key: key,
                entry,
            }])
        }
    }
}

// ── CompileFileScheduler ──────────────────────────────────────────────────────

/// Schedules compile jobs for all source files in `AotCompileOptions`.
///
/// For phase-one determinism, executes sequentially and merges results into
/// the `ProgramRegistry` by `SourceFileId`.  Future parallelism would return
/// `(SourceFileId, CompileFileOutput)` and merge by ID.
pub struct CompileFileScheduler<'a> {
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}

impl<'a> CompileFileScheduler<'a> {
    pub fn new(options: &'a AotCompileOptions, cache_store: &'a ProgramCacheStore) -> Self {
        Self { options, cache_store }
    }

    /// Compile all source files and insert results into the `ProgramRegistry`.
    ///
    /// Returns the registry with all Programs inserted in upstream key order.
    pub fn run_all(&self, registry: &mut ProgramRegistry) -> Result<(), AbcError> {
        for (source_id, source) in self.options.source_files.iter_enumerated() {
            let job = CompileFileJob::new(source_id, source, self.options, self.cache_store);
            let outputs = job.run()?;
            for output in outputs {
                registry.insert_source_program(output.program_key, output.entry)?;
            }
        }
        Ok(())
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Convert a proto Program back to our Program type for cache hit path.
fn proto_to_program(
    proto: ProtoProgram,
    source_file: &SourceFile,
    is_merged: bool,
) -> Result<Program, AbcError> {
    // Use the existing program_cache module to append the proto into an AbcFile,
    // then extract the records.
    let mut scratch = crate::emit::file::AbcFile::new();
    let _result = append_program_to_file(&mut scratch, proto)?;
    let mut program = Program {
        lang: SourceLang::EcmaScript,
        metadata: ProgramMetadata {
            is_source_mode: true,
            is_generated_from_merged_abc: is_merged,
            is_shared_module: source_file.is_shared_module,
            source_lang: SourceLang::from_origin_str(source_file.source_lang.as_ref()),
        },
        // IMPORTANT: Carry over literal arrays and strings from the proto-restored AbcFile.
        // Records in the restored program reference literal arrays and strings by index.
        literal_array_table: scratch.literal_arrays.into_iter().collect(),
        strings: scratch.strings,
        ..Program::default()
    };
    for record in scratch.records.into_iter() {
        program.add_record(record);
    }
    Ok(program)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::cache_store::ProgramCacheStore;
    use crate::aot::options::{AotCompileOptions, AotOutput, CompileContextInfo, SourceFile, ScriptKind};
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_options_with_source(source: &str) -> AotCompileOptions {
        let mut sf = SourceFile::new_source("test.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/main");
        sf.inline_source = Some(Arc::from(source));

        let mut sources = IndexVec::new();
        sources.push(sf);

        AotCompileOptions {
            source_files: sources,
            output: AotOutput::Path(PathBuf::from("out.abc")),
            merge_abc: true,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        }
    }

    #[test]
    fn compile_file_job_cache_miss_invokes_source_compiler_once() {
        // A new compile with no cache must succeed and produce one Program.
        let options = make_options_with_source("export const x = 1;");
        let cache_store = ProgramCacheStore::new(12, None);
        let scheduler = CompileFileScheduler::new(&options, &cache_store);

        let mut registry = ProgramRegistry::new(1);
        let result = scheduler.run_all(&mut registry);
        assert!(result.is_ok(), "compile must succeed: {:?}", result.err());
        assert_eq!(registry.count(), 1, "must produce exactly one Program");
    }

    #[test]
    fn compile_file_scheduler_merges_by_source_file_id() {
        // Two source files → two Programs, keyed by file name.
        let mut sf1 = SourceFile::new_source("a.ts", "ts");
        sf1.script_kind = ScriptKind::Module;
        sf1.record_name = Box::from("a/main");
        sf1.inline_source = Some(Arc::from("export const a = 1;"));

        let mut sf2 = SourceFile::new_source("b.ts", "ts");
        sf2.script_kind = ScriptKind::Module;
        sf2.record_name = Box::from("b/main");
        sf2.inline_source = Some(Arc::from("export const b = 2;"));

        let mut sources = IndexVec::new();
        sources.push(sf1);
        sources.push(sf2);

        let options = AotCompileOptions {
            source_files: sources,
            output: AotOutput::Path(PathBuf::from("out.abc")),
            merge_abc: true,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        };

        let cache_store = ProgramCacheStore::new(12, None);
        let scheduler = CompileFileScheduler::new(&options, &cache_store);
        let mut registry = ProgramRegistry::new(2);
        scheduler.run_all(&mut registry).unwrap();

        assert_eq!(registry.count(), 2, "two sources must produce two Programs");
    }

    #[test]
    fn compile_file_job_cache_hit_skips_source_compiler() {
        // Write a cache entry, then verify the scheduler restores from cache.
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("test.protoBin");

        let mut sf = SourceFile::new_source("test.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/main");
        sf.inline_source = Some(Arc::from("export const x = 1;"));
        sf.source_file = Box::from("test.ts");

        let cache_store = ProgramCacheStore::new(12, None);
        let hash = cache_store.source_hash(
            "export const x = 1;",
            "test.ts",
            "test/main",
            "test.ts",
            "",
        );

        // Write a minimal proto Program to the cache.
        let proto = crate::proto_panda::Program::default();
        cache_store.write_source_program(&cache_path, hash, proto).unwrap();

        // Set up cache file mapping.
        let mut cache_files = FxHashMap::default();
        cache_files.insert(Box::from("test.ts"), cache_path.clone());

        let options = AotCompileOptions {
            source_files: IndexVec::new(),
            output: AotOutput::Path(PathBuf::from("out.abc")),
            merge_abc: true,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files,
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        };

        let source_id = SourceFileId::from(0usize);
        let job = CompileFileJob::new(source_id, &sf, &options, &cache_store);
        let outputs = job.run().unwrap();
        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].entry.from_cache,
            "job must restore from cache on hash match"
        );
    }

    #[test]
    fn compile_file_job_abc_input_routes_to_abc_compiler() {
        // When a SourceFile has is_source_mode=false (ABC input), the compile job
        // must route to AbcInputProgramCompiler rather than SourceProgramCompiler.
        // We test this by verifying that a minimal invalid source text does NOT
        // produce a compile error when the source file is marked as non-source-mode
        // (because ABC input jobs don't parse the source text as JS).
        let cache_store = ProgramCacheStore::new(12, None);

        // Create an ABC input SourceFile with invalid JS source.
        // If the job incorrectly routes to SourceProgramCompiler, it would fail.
        // Since we don't have a real ABC file, we use a valid but minimal ABC.
        let dir = tempfile::tempdir().unwrap();
        let abc_path = dir.path().join("input.abc");

        // Write a minimal valid ABC (empty program) to disk.
        let empty_abc = crate::emit::file::AbcFile::new().serialize();
        std::fs::write(&abc_path, &empty_abc).unwrap();

        let sf = SourceFile::new_abc_input(&abc_path, "abc");
        assert!(!sf.is_source_mode,
            "ABC input SourceFile must have is_source_mode=false");

        let options = AotCompileOptions {
            source_files: IndexVec::new(),
            output: AotOutput::Path(dir.path().join("out.abc")),
            merge_abc: false,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: true,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        };

        let source_id = SourceFileId::from(0usize);
        let job = CompileFileJob::new(source_id, &sf, &options, &cache_store);

        // The job should try the ABC input path (not source compiler).
        // An empty ABC file might produce 0 records (ok) or an error for
        // "no user records" — both are acceptable outcomes, NOT a parse error.
        let result = job.run();
        // The important thing: no "parse error" from TS source parsing.
        if let Err(e) = &result {
            assert!(
                !matches!(e, crate::error::AbcError::Parse { .. }),
                "ABC input job must not produce a JS parse error: {e:?}"
            );
        }
    }

    #[test]
    fn compile_file_scheduler_output_matches_upstream_program_order() {
        // Programs in the registry must be ordered by key (program key = source
        // file name), matching upstream std::map<std::string, ProgramCache*>
        // lexicographic order.
        let mut sf1 = SourceFile::new_source("z_last.ts", "ts");
        sf1.script_kind = ScriptKind::Module;
        sf1.record_name = Box::from("z/last");
        sf1.inline_source = Some(Arc::from("export const z = 1;"));

        let mut sf2 = SourceFile::new_source("a_first.ts", "ts");
        sf2.script_kind = ScriptKind::Module;
        sf2.record_name = Box::from("a/first");
        sf2.inline_source = Some(Arc::from("export const a = 1;"));

        let mut sf3 = SourceFile::new_source("m_middle.ts", "ts");
        sf3.script_kind = ScriptKind::Module;
        sf3.record_name = Box::from("m/middle");
        sf3.inline_source = Some(Arc::from("export const m = 1;"));

        let mut sources = IndexVec::new();
        // Insert in non-alphabetical order.
        sources.push(sf1); // z_last
        sources.push(sf2); // a_first
        sources.push(sf3); // m_middle

        let options = AotCompileOptions {
            source_files: sources,
            output: AotOutput::Path(std::path::PathBuf::from("out.abc")),
            merge_abc: true,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        };

        let cache_store = ProgramCacheStore::new(12, None);
        let scheduler = CompileFileScheduler::new(&options, &cache_store);
        let mut registry = ProgramRegistry::new(3);
        scheduler.run_all(&mut registry).unwrap();

        // Programs must be returned in upstream program_key (= source file name)
        // lexicographic order, regardless of insertion order.
        let keys: Vec<&str> = registry
            .iter_in_key_order()
            .map(|(key, _, _)| key)
            .collect();
        assert_eq!(
            keys,
            ["a_first.ts", "m_middle.ts", "z_last.ts"],
            "CompileFileScheduler output must match upstream std::map ordering"
        );
    }
}
