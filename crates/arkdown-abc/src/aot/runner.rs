//! AotRunner — upstream-shaped AOT compile orchestration.
//!
//! `AotRunner` is the production entry point for the ABC compile chain.
//! It mirrors upstream `Run()` in `es2panda/aot/main.cpp`, orchestrating:
//!
//! 1. Options validation (source file count, merge/non-merge consistency).
//! 2. `CompileFileScheduler::run_all()` — compile all source files.
//! 3. `CheckMergeModeConsistency` — validate ABC input merge mode.
//! 4. `NpmModuleEntryCompiler::compile()` — compile npm entries if specified.
//! 5. `DepsRelationResolver::resolve()` — BFS dependency resolution.
//! 6. `ProgramRegistry::retain_resolved()` — remove unreachable programs.
//! 7. `AotOutputGenerator::generate()` — emit final ABC or proto output.
//!
//! All stages that are disabled (no npm entries, no compile context, etc.)
//! are skipped exactly as upstream skips them.
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp` — `Run()`, `ResolveAndGenerate`
//! - `es2panda/compiler/core/compileQueue.cpp` — `Compiler::CompileFiles`

use crate::aot::cache_store::ProgramCacheStore;
use crate::aot::compile_queue::CompileFileScheduler;
use crate::aot::deps::resolve_deps_relation;
use crate::aot::npm_entries::NpmModuleEntryCompiler;
use crate::aot::optimizer::ProgramOptimizer;
use crate::aot::options::AotCompileOptions;
use crate::aot::output::{AotOutputGenerator, AotOutputResult};
use crate::aot::program_registry::ProgramRegistry;
use crate::error::AbcError;

/// AOT Runner — the production entry point for the upstream-shaped compile chain.
///
/// Construct via `AotRunner::new(options)` and call `run()` to execute.
pub struct AotRunner {
    options: AotCompileOptions,
}

impl AotRunner {
    /// Create a new runner from fully-parsed `AotCompileOptions`.
    pub fn new(options: AotCompileOptions) -> Self {
        Self { options }
    }

    /// Execute the full AOT compile chain and return the output result.
    ///
    /// ## Steps (matching upstream `Run()`)
    ///
    /// 1. Build `ProgramCacheStore` for the target API version.
    /// 2. Set expected program count from source file count.
    /// 3. Run `CompileFileScheduler::run_all()`.
    /// 4. Check merge mode consistency for ABC inputs.
    /// 5. Compile npm module entries if `npm_module_entry_list` is set.
    /// 6. Resolve dependency relations if `compile_context_info` has entries.
    /// 7. Remove redundant programs if `remove_redundant_file`.
    /// 8. Generate output via `AotOutputGenerator`.
    pub fn run(self) -> Result<AotOutputResult, AbcError> {
        let options = self.options;

        // 1. Build the cache store.
        let cache_store = ProgramCacheStore::new(
            options.target_api_version,
            options.target_api_sub_version.clone(),
        );

        // 2. Initialize registry with expected count.
        let source_count = options.source_files.len();
        let mut registry = ProgramRegistry::new(source_count);

        tracing::debug!(
            source_count,
            merge_abc = options.merge_abc,
            debug_info = options.debug_info,
            opt_level = options.opt_level,
            target_api = options.target_api_version,
            "AotRunner::run() start"
        );

        // 3. Compile all source files.
        let scheduler = CompileFileScheduler::new(&options, &cache_store);
        scheduler.run_all(&mut registry)?;

        // 3b. Global optimization post-analysis pass.
        //
        // Upstream: after all programs have been compiled and analyzed, run the
        // deferred optimization pass on programs that require post-analysis opt.
        // `optimizationPendingProgs_` in upstream is equivalent to all entries
        // where `requires_post_analysis_opt = true`.
        if options.require_global_optimization {
            let mut optimizer = ProgramOptimizer::from_compile_options(
                options.opt_level,
                options.optimize_try_catch_func,
            );
            optimizer.optimize_after_analysis(&mut registry);
        }

        // 4. Check merge mode consistency for ABC inputs.
        registry.check_merge_mode_consistency(options.merge_abc)?;

        // 5. Compile npm module entries (if requested).
        if let Some(npm_path) = &options.npm_module_entry_list {
            let npm_compiler = NpmModuleEntryCompiler::new(&cache_store);
            let cache_path = options.cache_files.get("npmEntries.txt").cloned();
            let npm_entry = npm_compiler.compile(
                npm_path,
                cache_path.as_deref(),
            )?;

            let npm_key = npm_path.to_string_lossy().into_owned();
            registry.insert_npm_program(npm_key.as_str(), npm_entry)?;

            // Upstream: `Compiler::SetExpectedProgsCount(GetExpectedProgsCount() + 1)`.
            registry.add_expected(1);

            tracing::debug!(
                npm_path = %npm_path.display(),
                "compiled npm module entry list"
            );
        }

        // 6. Dependency resolution.
        if options.compile_context_info.needs_dependency_relation() {
            let resolved = resolve_deps_relation(
                &registry,
                &options.compile_context_info,
                options.dump_deps_info,
            );

            // 7. Remove redundant programs if requested.
            if options.remove_redundant_file {
                let retained = resolved.retained_keys();
                registry.retain_resolved(&retained);
            }
        }

        // 8. Validate expected count (unless remove_redundant_file changed it).
        // Upstream `GenerateAbcFiles` validates this.
        // We skip validation when remove_redundant_file is true because the
        // retained count may differ from the original expected count.
        if !options.remove_redundant_file {
            registry.validate_expected_count()?;
        }

        // 9. Generate output.
        let generator = AotOutputGenerator::new(&options, &cache_store);
        generator.generate(&registry)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::options::{AotCompileOptions, AotOutput, CompileContextInfo, ScriptKind, SourceFile};
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_options(source: &str, output: PathBuf) -> AotCompileOptions {
        let mut sf = SourceFile::new_source("test.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/main");
        sf.inline_source = Some(Arc::from(source));

        let mut sources = IndexVec::new();
        sources.push(sf);

        AotCompileOptions {
            source_files: sources,
            output: AotOutput::Path(output),
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
        }
    }

    #[test]
    fn aot_runner_compiles_simple_module_to_abc() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.abc");
        let options = make_options("export const x = 1;", output.clone());

        let runner = AotRunner::new(options);
        let result = runner.run().unwrap();

        assert_eq!(result.abc_paths.len(), 1);
        assert_eq!(result.abc_paths[0], output);
        assert_eq!(result.api_version, 12);

        let bytes = std::fs::read(&output).unwrap();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
    }

    #[test]
    fn aot_runner_compiles_multiple_modules_to_merged_abc() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("modules.abc");

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
            output: AotOutput::Path(output.clone()),
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
        };

        let runner = AotRunner::new(options);
        let result = runner.run().unwrap();

        assert_eq!(result.abc_paths.len(), 1);
        let bytes = std::fs::read(&result.abc_paths[0]).unwrap();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
    }

    #[test]
    fn aot_runner_with_debug_info_produces_debug_output() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("debug.abc");
        let mut options = make_options("export const x = 1;", output.clone());
        options.debug_info = true;
        options.opt_level = 0;

        let runner = AotRunner::new(options);
        let result = runner.run().unwrap();

        assert!(result.has_debug_info, "debug_info=true must set has_debug_info");
    }
}
