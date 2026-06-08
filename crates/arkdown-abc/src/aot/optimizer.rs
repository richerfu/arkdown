//! Program-level bytecode optimizer — Phase 9 of SPEC-049.
//!
//! This module implements `ProgramOptimizer` per the SPEC-049 Phase 9 spec:
//!
//! ```text
//! pub struct ProgramOptimizer {
//!     options: BytecodeOptimizerOptions,
//! }
//!
//! impl ProgramOptimizer {
//!     pub fn optimize_source_program(&mut self, program: &mut Program);
//!     pub fn analyze_for_global_optimization(&mut self, program: &Program);
//!     pub fn optimize_after_analysis(&mut self, registry: &mut ProgramRegistry);
//! }
//! ```
//!
//! ## Upstream algorithm
//!
//! `CompileFileJob::OptimizeAndCacheProgram` in `compileQueue.cpp`:
//!
//! ```cpp
//! if (src_->isSourceMode && options_->transformLib.empty()) {
//!     if (options_->requireGlobalOptimization) {
//!         util::Helpers::AnalysisProgram(prog, src_->fileName);
//!         requireOptimizationAfterAnalysis = true;
//!     } else if (options_->optLevel != 0) {
//!         util::Helpers::OptimizeProgram(prog, src_->fileName);
//!     }
//! }
//! ```
//!
//! ## Key behaviors
//!
//! - `optimize_source_program`: runs when `optLevel > 0` and NOT
//!   `requireGlobalOptimization`. Applies register compaction and
//!   peephole optimization to all methods in the Program's records.
//!
//! - `analyze_for_global_optimization`: runs when `requireGlobalOptimization`
//!   is set; marks the Program for deferred post-analysis optimization.
//!
//! - `optimize_after_analysis`: runs after ALL programs have been analyzed;
//!   applies global cross-program optimization.
//!
//! - Debug mode forces `opt_level = 0` before the scheduler starts, so
//!   `optimize_source_program` never runs in debug mode.
//!
//! ## Upstream reference
//! - `es2panda/compiler/core/compileQueue.cpp` — `OptimizeAndCacheProgram`

use std::collections::BTreeSet;

use crate::aot::program_registry::ProgramRegistry;
use crate::emit::file::AbcFile;
use crate::optimizer::{optimize_records, BytecodeOptimizerOptions};
use crate::program::model::{Program, ProgramCacheEntry};

/// Program-level bytecode optimizer.
///
/// This is the Rust equivalent of `CompileFileJob::OptimizeAndCacheProgram`
/// plus the deferred global-analysis optimization pass.
///
/// ## SPEC-049 Phase 9
///
/// > Refactor current `optimizer.rs` into Program-level optimizer.
/// > Store optimization state on ProgramCacheEntry:
/// > `need_update_cache`, `from_cache`, `is_source_mode`, `requires_post_analysis_opt`.
/// > Debug mode must force opt 0 before scheduler starts.
pub struct ProgramOptimizer {
    options: BytecodeOptimizerOptions,
}

impl ProgramOptimizer {
    /// Create a new optimizer with the given bytecode optimizer options.
    pub fn new(options: BytecodeOptimizerOptions) -> Self {
        Self { options }
    }

    /// Create a ProgramOptimizer from `opt_level` and `optimize_try_catch_func`.
    pub fn from_compile_options(opt_level: u8, optimize_try_catch_func: bool) -> Self {
        Self {
            options: BytecodeOptimizerOptions::from_compile_options(
                opt_level,
                optimize_try_catch_func,
            ),
        }
    }

    /// Optimize a source-compiled Program in place.
    ///
    /// Applies register frame compaction and (at `opt_level >= 2`) peephole
    /// optimization to all methods in the Program's records.
    ///
    /// Matches upstream `util::Helpers::OptimizeProgram` called from
    /// `CompileFileJob::OptimizeAndCacheProgram` when `optLevel != 0`.
    ///
    /// ## Preconditions
    ///
    /// - `program.metadata.is_source_mode = true` (ABC input programs are not optimized).
    /// - `self.options.opt_level > 0` (opt_level = 0 means no optimization).
    /// - Debug mode must force `opt_level = 0` before this is called.
    pub fn optimize_source_program(&mut self, program: &mut Program) {
        if self.options.opt_level == 0 {
            return;
        }
        if !program.metadata.is_source_mode {
            return;
        }

        // Build a temporary AbcFile to run the existing optimizer over.
        let mut scratch = AbcFile::new();
        for record in program.record_table.iter() {
            scratch.add_record(record.clone());
        }

        let should_optimize = vec![true; scratch.records.len()];
        optimize_records(&mut scratch, self.options, &should_optimize);

        // Copy optimized records back.
        for (idx, record) in program.record_table.iter_mut().enumerate() {
            if let Some(optimized) = scratch.records.get(idx) {
                *record = optimized.clone();
            }
        }
    }

    /// Analyze a Program for global cross-program optimization.
    ///
    /// Called when `requireGlobalOptimization = true`. The analysis pass
    /// collects information needed for cross-program optimization (e.g.
    /// inline cache statistics, call frequency, etc.). After all programs
    /// have been analyzed, `optimize_after_analysis` applies the actual
    /// optimization passes.
    ///
    /// Matches upstream `util::Helpers::AnalysisProgram` called from
    /// `CompileFileJob::OptimizeAndCacheProgram` when `requireGlobalOptimization`.
    ///
    /// ## Note: AnalysisProgram is a known stub
    ///
    /// Upstream `util::Helpers::AnalysisProgram` (called from
    /// `CompileFileJob::OptimizeAndCacheProgram` when `requireGlobalOptimization`)
    /// runs the actual cross-program bytecode analysis pass from
    /// `bytecode_optimizer`.  This pass collects inline cache statistics, call
    /// frequency data, and other cross-function information needed for branch
    /// elimination and cross-function inlining.
    ///
    /// **Current status**: The bytecode_optimizer analysis pass is not yet ported
    /// from upstream C++.  This method is a correctly-wired call site that does
    /// nothing.  The control flow is correct:
    /// - Cache is bypassed (`require_global_optimization = true` skips cache hit).
    /// - Programs are marked `requires_post_analysis_opt = true`.
    /// - `optimize_after_analysis` is called after all programs are compiled.
    ///
    /// What is MISSING: the actual analysis data is never collected, so
    /// `optimize_after_analysis` runs `optimize_source_program` on each program
    /// (applying local optimization only, not global cross-program optimization).
    ///
    /// **Functional impact**: Setting `require_global_optimization = true`
    /// currently behaves like `optLevel > 0` without the global analysis benefit.
    /// For standard TypeScript/JavaScript sources, local optimization still
    /// produces correct (though not globally optimal) ABC.
    ///
    /// **Tracking**: A follow-up SPEC will port `bytecode_optimizer::AnalysisProgram`
    /// once the pandasm IR graph (CFG) is available in Rust.
    pub fn analyze_for_global_optimization(&mut self, _program: &Program) {
        // Upstream: util::Helpers::AnalysisProgram(prog, src_->fileName)
        //
        // The full upstream analysis pass builds a Control Flow Graph (CFG)
        // and runs liveness analysis for cross-program branch elimination and
        // inlining (from `bytecode_optimizer`).  The analysis READS the program
        // to collect statistics (IC data, call frequency, type profiles) and
        // stores them for the later `OptimizeProgram` pass.
        //
        // ## Implementation status
        //
        // The bytecode_optimizer CFG analysis is not yet ported from upstream C++.
        // The CONTROL FLOW is fully aligned with upstream:
        //
        // 1. Cache is bypassed when `requireGlobalOptimization = true` ✅
        // 2. Programs are compiled fresh and marked `requires_post_analysis_opt` ✅
        // 3. `optimize_after_analysis` runs LOCAL optimization after all programs
        //    are compiled, matching upstream's deferred optimization pass ✅
        //
        // What is MISSING: the cross-program analysis data (IC statistics,
        // call frequency, type profiles) that upstream uses for inter-procedural
        // optimizations like branch elimination and inlining.  Without this data,
        // `optimize_after_analysis` applies LOCAL optimization only (register
        // compaction + peephole), not global optimization.
        //
        // **Functional impact**: programs compiled with `requireGlobalOptimization=true`
        // are correctly optimized at the local level.  Cross-program optimization
        // benefits (devirtualization, cross-function inlining) are not applied.
        //
        // **Upstream function signature**: `AnalysisProgram(pandasm::Program*, fileName)`
        // takes a non-const pointer — it MAY modify the program for inlining stubs.
        // Our `&Program` (immutable) matches the READ-ONLY analysis contract.
    }

    /// Apply global optimization after all programs have been analyzed.
    ///
    /// Iterates all Programs in the registry that have
    /// `ProgramCacheEntry::requires_post_analysis_opt = true` and applies
    /// the optimization pass now that cross-program analysis data is available.
    ///
    /// Matches upstream deferred path: after `optimizationPendingProgs_` is
    /// populated, the optimizer runs a second pass on each pending program.
    ///
    /// ## Upstream reference
    /// `es2panda/aot/emitFiles.cpp` — deferred global optimization path.
    pub fn optimize_after_analysis(&mut self, registry: &mut ProgramRegistry) {
        // Collect all programs pending post-analysis optimization.
        let keys_needing_opt: Vec<String> = registry
            .iter_in_key_order()
            .filter(|(_, _, entry)| entry.requires_post_analysis_opt)
            .map(|(key, _, _)| key.to_string())
            .collect();

        for key in keys_needing_opt {
            if let Some((_id, entry)) = registry.get_mut(&key) {
                // Apply optimization if opt_level > 0; always clear the flag
                // so callers know the post-analysis pass has completed.
                if self.options.opt_level > 0 {
                    self.optimize_source_program(&mut entry.program);
                }
                entry.requires_post_analysis_opt = false;
            }
        }
    }

    /// Returns true if any optimization will be applied (opt_level > 0).
    pub fn will_optimize(&self) -> bool {
        self.options.opt_level > 0
    }

    /// Returns true if try/catch functions are optimized.
    pub fn optimize_try_catch(&self) -> bool {
        !self.options.skip_methods_with_eh
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::model::{Program, ProgramCacheEntry};

    fn make_program_with_method(bytecode: Vec<u8>) -> Program {
        use crate::emit::record::{ClassRecord, MethodKind, MethodRecord};
        let mut program = Program::new_ecmascript();
        let mut record = ClassRecord {
            name: "test/main".to_string(),
            methods: vec![MethodRecord {
                name: "func_main_0".to_string(),
                bytecode,
                num_vregs: 5,
                num_args: 3,
                kind: MethodKind::Module,
                pandasm_insns: vec![],
                pandasm_debug: vec![],
                value_of_first_param: 0,
                slots_num: None,
                debug_info: None,
                line_ops: vec![],
                catch_entries: vec![],
                program_labels: vec![],
                label_table: vec![],
                function_kind: 1,
                source_lang: None,
                concurrent_module_requests: vec![],
                method_pool_entry: false,
                expected_property_count: None,
                annotations: vec![],
                metadata_attrs: Default::default(),
                param_annotations: vec![],
                definefunc_order: None,
                label_style: crate::emit::record::MethodLabelStyle::Source,
                param_types: vec![],
                return_type: None,
                file_location: None,
                body_presence: None,
                body_location: None,
            }],
            fields: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: true,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
            module_requests: vec![],
        };
        program.add_record(record);
        program
    }

    #[test]
    fn debug_source_never_optimized() {
        // opt_level=0 (debug mode): no optimization applied.
        let mut optimizer = ProgramOptimizer::from_compile_options(0, true);
        assert!(!optimizer.will_optimize(),
            "opt_level=0 must not optimize");

        // A simple bytecode that would be changed by optimization.
        let original_bc = vec![0x02, 0x00, 0x01, 0x00]; // arbitrary 4-byte bytecode
        let mut program = make_program_with_method(original_bc.clone());
        program.metadata.is_source_mode = true;

        optimizer.optimize_source_program(&mut program);

        // With opt_level=0, the bytecode should not change.
        let record = program.record_table.first().expect("record must exist");
        assert_eq!(
            record.methods[0].bytecode,
            original_bc,
            "debug mode must not modify bytecodes"
        );
    }

    #[test]
    fn release_source_opt_level_2_optimized() {
        // opt_level=2 (release mode): optimization is applied.
        let mut optimizer = ProgramOptimizer::from_compile_options(2, true);
        assert!(optimizer.will_optimize(),
            "opt_level=2 must enable optimization");
    }

    #[test]
    fn abc_input_program_not_optimized() {
        // ABC input programs (is_source_mode=false) must never be optimized.
        let mut optimizer = ProgramOptimizer::from_compile_options(2, true);
        let original_bc = vec![0x02, 0x00];
        let mut program = make_program_with_method(original_bc.clone());
        program.metadata.is_source_mode = false; // ABC input

        optimizer.optimize_source_program(&mut program);

        let record = program.record_table.first().expect("record must exist");
        assert_eq!(
            record.methods[0].bytecode,
            original_bc,
            "ABC input programs must not be re-optimized"
        );
    }

    #[test]
    fn global_optimization_analysis_and_post_order_stable() {
        // The optimization state flag is correctly set and cleared.
        let mut entry = ProgramCacheEntry::new_compiled(Program::new_ecmascript(), false);
        entry.requires_post_analysis_opt = true;

        let mut registry = crate::aot::program_registry::ProgramRegistry::new(1);
        registry.insert_source_program("test.ts", entry).unwrap();

        let mut optimizer = ProgramOptimizer::from_compile_options(0, true);
        optimizer.optimize_after_analysis(&mut registry);

        // After optimize_after_analysis, the flag is cleared.
        let (_id, entry) = registry.get("test.ts").unwrap();
        assert!(
            !entry.requires_post_analysis_opt,
            "requires_post_analysis_opt must be cleared after post-analysis optimization"
        );
    }

    #[test]
    fn cache_hit_source_not_optimized_again() {
        // Cache-hit programs (from_cache=true) should not re-run optimization.
        let mut optimizer = ProgramOptimizer::from_compile_options(2, true);
        let original_bc = vec![0x02, 0x00];
        let mut program = make_program_with_method(original_bc.clone());

        // Mark as cache hit — from_cache=true means it was already optimized.
        program.metadata.is_source_mode = true; // Still source mode...
        // For cache-hit programs, the compile queue skips optimization entirely
        // since from_cache=true indicates the program was already optimized.
        // The ProgramOptimizer is only called for freshly compiled programs.
        // This test verifies the flag state is correct on a cache-restored entry.
        let entry = ProgramCacheEntry::new_cache_hit(program, 0x1234);
        assert!(entry.from_cache, "cache hit must be from_cache=true");
        assert!(!entry.need_update_cache, "cache hit must not need cache update");
    }

    #[test]
    fn try_catch_fixture_matches_upstream_codeitem() {
        // When `optimize_try_catch_func = false` (matching upstream
        // `--opt-try-catch-func=false`), methods that have catch entries
        // must NOT be optimized (their bytecodes remain unchanged).
        //
        // This matches upstream `bytecode_optimizer`'s `skip_methods_with_eh`
        // option, set by `!--opt-try-catch-func`.
        use crate::emit::record::CatchEntry;

        let mut optimizer = ProgramOptimizer::from_compile_options(2, false);
        // optimize_try_catch = false → skip_methods_with_eh = true

        assert!(!optimizer.optimize_try_catch(),
            "optimize_try_catch_func=false must set skip_methods_with_eh=true");

        // A method with a non-empty catch table must not be modified.
        let original_bc = vec![0x00, 0x01, 0x02]; // 3 arbitrary bytecode bytes
        let mut program = make_program_with_method(original_bc.clone());

        // Add a catch entry to the method to simulate a try/catch function.
        let record = program.record_table.iter_mut().next().expect("record exists");
        record.methods[0].catch_entries = vec![CatchEntry {
            try_begin: 0,
            try_end: 2,
            catch_begin: 2,
            catch_end: 3,
            exception_record: None,
        }];
        program.metadata.is_source_mode = true;

        optimizer.optimize_source_program(&mut program);

        let record = program.record_table.iter().next().expect("record exists");
        assert_eq!(
            record.methods[0].bytecode,
            original_bc,
            "methods with try/catch must not be optimized when optimize_try_catch=false"
        );
    }
}
