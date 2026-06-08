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
use crate::aot::options::{AotCompileOptions, ScriptKind, SourceFile};
use crate::aot::program_registry::{ProgramRegistry, abc_input_program_key};
use crate::compiler::check_unsupported_syntax;
use crate::emit::file::version_for_target_api;
use crate::error::AbcError;
use crate::ir::builder;
use crate::optimizer::{optimize_records, BytecodeOptimizerOptions};
use crate::program::ids::SourceFileId;
use crate::program::model::{Program, ProgramCacheEntry, ProgramMetadata, SourceLang};
use crate::program_cache::{append_program_to_file, program_for_record};
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

        // 5. Source compile.
        let program = self.compile_source_program(&source_text)?;

        // 6. Optimize.
        let effective_debug = self.options.effective_debug_info();
        let program = optimize_program(
            program,
            self.options.opt_level,
            self.options.optimize_try_catch_func,
        );

        let need_update_cache = cache_path.is_some();
        let entry = ProgramCacheEntry {
            program,
            hash_code: cache_hash,
            need_update_cache,
            from_cache: false,
            is_source_mode: true,
            requires_post_analysis_opt: false,
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

    /// Compile a source file to a `Program`.
    fn compile_source_program(&self, raw_source_text: &str) -> Result<Program, AbcError> {
        let source = self.source;
        let file_name = source.file_name.to_string_lossy();

        // Strip TypeScript type annotations before parsing as plain JS, matching
        // upstream es2abc / old compiler.rs behavior.
        // OLD path: strip_typescript_types() was called when source_file.is_none().
        let source_text_owned;
        let source_text = if source.source_file.is_empty() {
            source_text_owned = crate::compiler::strip_typescript_types_pub(
                raw_source_text,
                file_name.as_ref(),
            )?;
            source_text_owned.as_str()
        } else {
            raw_source_text
        };

        // Pre-compile syntax check.
        check_unsupported_syntax(source_text, file_name.as_ref())?;

        // Determine module mode from script_kind.
        let is_module = source.script_kind.is_module();
        let is_commonjs = source.script_kind.is_commonjs();

        // Build the ABC file using the existing IR builder.
        // The result is a ClassRecord which we wrap in a Program.
        let mut scratch_file = crate::emit::file::AbcFile::new();

        let standalone_ecmascript_input =
            crate::ir::record_layout::is_explicit_ecmascript_source_file(
                source.source_file.as_ref(),
            );

        let source_file_str = if !source.source_file.is_empty() {
            source.source_file.as_ref().to_string()
        } else {
            // Upstream `source_file_from_record_name` fallback.
            use arkdown_core::HvigorSourceMapScope;
            HvigorSourceMapScope::source_file_from_record_name(source.record_name.as_ref())
                .unwrap_or_else(|| {
                    source
                        .record_name
                        .strip_prefix("@bundle:")
                        .unwrap_or(source.record_name.as_ref())
                        .to_string()
                })
        };

        let mut record = builder::build_record_with_debug_and_package_for_target_with_mode(
            source_text,
            source.record_name.as_ref(),
            &mut scratch_file.strings,
            &mut scratch_file.literal_arrays,
            Some(source_file_str.as_str()),
            if source.pkg_name.is_empty() {
                None
            } else {
                Some(source.pkg_name.as_ref())
            },
            is_commonjs,
            source.is_shared_module,
            source.source_lang.as_ref(),
            is_module,
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
            self.options.enable_annotations,
            self.options.effective_debug_info(),
            standalone_ecmascript_input,
        )?;

        // Non-merge mode: add upstream metadata records before the main record.
        // This matches upstream `non_merge_metadata_records` in compiler.rs.
        if !self.options.merge_abc {
            let metadata_records = crate::compiler::non_merge_metadata_records_pub(
                &mut record,
                source_file_str.as_str(),
                is_commonjs,
                is_module,
                self.options.module_record_field_name.as_deref(),
            );
            // Insert metadata records before main record in scratch_file for
            // proper literal array indexing.
            for meta in metadata_records {
                scratch_file.add_record(meta);
            }
        }

        // Annotation records (when enable_annotations is true).
        if self.options.enable_annotations {
            let ann_records = builder::annotation_records_from_source(
                source_text,
                source.record_name.as_ref(),
                Some(source_file_str.as_str()),
                self.options.target_api_version,
                self.options.enable_annotations,
                &mut scratch_file.literal_arrays,
            )?;
            for ann in ann_records {
                scratch_file.add_record(ann);
            }
        }

        // Add the main record last.
        scratch_file.add_record(record);

        // Build a Program from all accumulated records.
        // IMPORTANT: Save the local StringTable so program_to_abc_file can
        // re-intern strings and remap bytecode string indices when merging.
        let mut program = Program {
            lang: SourceLang::EcmaScript,
            metadata: ProgramMetadata {
                is_source_mode: true,
                is_generated_from_merged_abc: false,
                is_shared_module: source.is_shared_module,
                source_lang: SourceLang::from_origin_str(source.source_lang.as_ref()),
            },
            literal_array_table: scratch_file.literal_arrays.into_iter().collect(),
            strings: scratch_file.strings,
            ..Program::default()
        };
        for rec in scratch_file.records.into_iter() {
            program.add_record(rec);
        }

        Ok(program)
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

/// Apply program-level optimization matching upstream `OptimizeAndCacheProgram`.
fn optimize_program(mut program: Program, opt_level: u8, optimize_try_catch: bool) -> Program {
    if opt_level == 0 {
        return program;
    }
    // Optimization operates on the method bytecode within records.
    // We create a temporary AbcFile to run the optimizer over.
    let mut scratch = crate::emit::file::AbcFile::new();
    for record in program.record_table.iter() {
        scratch.add_record(record.clone());
    }
    let record_count = scratch.records.len();
    optimize_records(
        &mut scratch,
        BytecodeOptimizerOptions::from_compile_options(opt_level, optimize_try_catch),
        &vec![true; record_count],
    );
    // Copy optimized records back.
    for (idx, record) in program.record_table.iter_mut().enumerate() {
        if let Some(optimized) = scratch.records.get(idx) {
            *record = optimized.clone();
        }
    }
    program
}

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
}
