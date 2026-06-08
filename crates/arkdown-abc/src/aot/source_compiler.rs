//! Source program compiler — Phase 4 of SPEC-049.
//!
//! This module introduces `SourceProgramCompiler` as a separate struct
//! per the SPEC-049 Phase 4 requirement:
//!
//! > Introduce `SourceProgramCompiler::compile(source_file, source_text, options)`.
//! > Build an OXC AST using `script_kind` and `script_extension`.
//! > Run upstream-compatible TypeScript/ArkTS transform only when upstream
//! > would transform that source shape.
//! > Lower all functions into `Program.function_table`.
//! > Lower the source record into `Program.record_table`.
//! > Attach source file, package name, shared flag, source language, module kind.
//!
//! `SourceProgramCompiler` wraps the IR builder + Program construction logic
//! and is called by `CompileFileJob` in the compile queue.
//!
//! ## Upstream reference
//! - `es2panda/compiler/core/compileQueue.cpp` — `CompileFileJob::CompileProgram`
//! - `es2panda/compiler/` — `Compiler::CompileFile`

use crate::aot::options::{AotCompileOptions, SourceFile};
use crate::compiler::{non_merge_metadata_records_pub, strip_typescript_types_pub};
use crate::error::AbcError;
use crate::ir::{builder, record_layout::is_explicit_ecmascript_source_file};
use crate::program::model::{Program, SourceLang};

/// Upstream-equivalent source program compiler.
///
/// Compiles one JS/TS/ETS `SourceFile` into a `Program` (the intermediate
/// representation) ready for `ProgramRegistry` insertion.
///
/// This is the Rust equivalent of `es2panda::Compiler::CompileFile` plus
/// the `pandasm::Program` construction step in `OptimizeAndCacheProgram`.
///
/// ## Upstream reference
/// `es2panda/compiler/core/compileQueue.cpp` `CompileFileJob::CompileProgram`.
pub struct SourceProgramCompiler<'a> {
    options: &'a AotCompileOptions,
}

impl<'a> SourceProgramCompiler<'a> {
    /// Create a new source program compiler bound to the compile options.
    pub fn new(options: &'a AotCompileOptions) -> Self {
        Self { options }
    }

    /// Compile `source_file` from `source_text` into a `Program`.
    ///
    /// Steps (matching upstream `CompileProgram` → `Compiler::CompileFile`):
    ///
    /// 1. Normalize source: strip TypeScript type annotations when no explicit
    ///    source_file is provided (matching upstream TS parse/codegen normalization).
    /// 2. Pre-compile syntax check for ArkTS strict mode violations.
    /// 3. Build the OXC AST using `script_kind` and `script_extension`.
    /// 4. Lower all functions into a `ClassRecord` via `ir::builder`.
    /// 5. Add non-merge metadata records when `merge_abc = false`.
    /// 6. Add annotation records when `enable_annotations = true`.
    /// 7. Wrap records + strings + literals in a `Program`.
    ///
    /// ## Upstream reference
    /// `es2panda/compiler/core/compileQueue.cpp` `CompileFileJob::CompileProgram`.
    pub fn compile(
        &self,
        source_file: &SourceFile,
        source_text: &str,
    ) -> Result<Program, AbcError> {
        let file_name = source_file.file_name.to_string_lossy();

        // Step 1: Normalize source through TypeScript strip (OXC Codegen round-trip).
        // Upstream es2abc calls the TypeScript parser + codegen to normalize source
        // when no explicit source_file path is provided. This normalizes constructs
        // like regexp flag ordering (/bar/ig → /bar/gi).
        let source_owned;
        let source_text = if source_file.source_file.is_empty() {
            source_owned = strip_typescript_types_pub(source_text, file_name.as_ref())?;
            source_owned.as_str()
        } else {
            source_text
        };

        // Step 2: Pre-compile syntax check.
        crate::compiler::check_unsupported_syntax(source_text, file_name.as_ref())?;

        // Step 3: Determine script kind flags.
        let is_module = source_file.script_kind.is_module();
        let is_commonjs = source_file.script_kind.is_commonjs();

        // Compute source_file string for debug info.
        let source_file_str = if !source_file.source_file.is_empty() {
            source_file.source_file.as_ref().to_string()
        } else {
            use arkdown_core::HvigorSourceMapScope;
            HvigorSourceMapScope::source_file_from_record_name(
                source_file.record_name.as_ref(),
            )
            .unwrap_or_else(|| {
                source_file
                    .record_name
                    .strip_prefix("@bundle:")
                    .unwrap_or(source_file.record_name.as_ref())
                    .to_string()
            })
        };

        let standalone_ecmascript_input =
            is_explicit_ecmascript_source_file(source_file.source_file.as_ref());

        // Step 4: Build OXC AST and lower to Program via ir::builder Program API.
        //
        // `builder::build_program_with_mode` is the Program-level entry point
        // introduced in SPEC-049 Phase 4.  It owns its own StringTable and
        // LiteralArray table, matching upstream `pandasm::Program` semantics.
        let mut program = builder::build_program_with_mode(
            source_text,
            source_file.record_name.as_ref(),
            Some(source_file_str.as_str()),
            if source_file.pkg_name.is_empty() {
                None
            } else {
                Some(source_file.pkg_name.as_ref())
            },
            is_commonjs,
            source_file.is_shared_module,
            source_file.source_lang.as_ref(),
            is_module,
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
            self.options.enable_annotations,
            self.options.effective_debug_info(),
            standalone_ecmascript_input,
        )?;
        // Propagate source_lang metadata.
        program.metadata.source_lang = SourceLang::from_origin_str(source_file.source_lang.as_ref());
        program.metadata.is_shared_module = source_file.is_shared_module;

        // Step 5: Non-merge mode metadata records.
        //
        // Upstream `compiler/compiler.cpp` builds `_ESModuleRecord`, `_CommonJsRecord`,
        // `_ESScopeNamesRecord`, etc. in the non-merge compile path.  The metadata
        // fields are extracted from the main record (mutating it) and returned as
        // additional ClassRecord values to insert into the Program.
        if !self.options.merge_abc {
            let meta_records = {
                let main_record = program.record_table.iter_mut()
                    .next()
                    .expect("build_program_with_mode must produce at least one record");
                non_merge_metadata_records_pub(
                    main_record,
                    source_file_str.as_str(),
                    is_commonjs,
                    is_module,
                    self.options.module_record_field_name.as_deref(),
                )
            };
            for meta in meta_records {
                program.add_record(meta);
            }
        }

        // Step 6: Annotation records (API 20+ with --enable-annotations).
        //
        // `annotation_records_from_source` appends NEW literal arrays starting
        // at the current length of the provided Vec.  We collect the Program's
        // existing literals into a Vec, pass it to the function (so annotation
        // literal indices are correct absolute indices into the final table),
        // then rebuild the Program's literal_array_table from the extended Vec.
        if self.options.enable_annotations {
            let mut lits: Vec<crate::emit::file::LiteralArray> =
                program.literal_array_table.iter().cloned().collect();
            let ann_records = builder::annotation_records_from_source(
                source_text,
                source_file.record_name.as_ref(),
                Some(source_file_str.as_str()),
                self.options.target_api_version,
                self.options.enable_annotations,
                &mut lits,
            )?;
            // Rebuild literal_array_table with the extended literal set.
            program.literal_array_table = lits.into_iter().collect();
            for ann in ann_records {
                program.add_record(ann);
            }
        }

        tracing::debug!(
            record_name = source_file.record_name.as_ref(),
            record_count = program.record_table.len(),
            literal_count = program.literal_array_table.len(),
            is_module,
            is_commonjs,
            "SourceProgramCompiler::compile"
        );

        Ok(program)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::options::{AotCompileOptions, AotOutput, CompileContextInfo, ScriptKind};
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_options(merge_abc: bool, module: bool) -> AotCompileOptions {
        use crate::aot::options::SourceFile;
        let mut sf = SourceFile::new_source("test.ts", "ts");
        sf.script_kind = if module { ScriptKind::Module } else { ScriptKind::Script };
        sf.record_name = Box::from("test/main");
        sf.inline_source = Some(Arc::from("export const x = 1;"));

        let mut sources = IndexVec::new();
        sources.push(sf);

        AotCompileOptions {
            source_files: sources,
            output: AotOutput::Path(PathBuf::from("out.abc")),
            merge_abc,
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
    fn source_compiler_builds_program_with_records() {
        let options = make_options(true, true);
        let compiler = SourceProgramCompiler::new(&options);

        let mut sf = crate::aot::options::SourceFile::new_source("a.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/main");

        let program = compiler
            .compile(&sf, "export const x = 1;")
            .expect("source compile must succeed");

        assert!(!program.record_table.is_empty(),
            "compiled Program must have at least one record");
        assert!(program.metadata.is_source_mode,
            "source-compiled Program must have is_source_mode=true");
    }

    #[test]
    fn source_compiler_module_mode_sets_ecmascript_lang() {
        let options = make_options(true, true);
        let compiler = SourceProgramCompiler::new(&options);

        let mut sf = crate::aot::options::SourceFile::new_source("b.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/module");

        let program = compiler
            .compile(&sf, "import x from './a'; export { x };")
            .expect("module source compile must succeed");

        assert_eq!(program.lang, SourceLang::EcmaScript);
    }

    #[test]
    fn source_compiler_commonjs_mode_is_source_mode_true() {
        let options = make_options(true, false);
        let compiler = SourceProgramCompiler::new(&options);

        let mut sf = crate::aot::options::SourceFile::new_source("c.js", "js");
        sf.script_kind = ScriptKind::CommonJs;
        sf.record_name = Box::from("test/cjs");

        let program = compiler
            .compile(&sf, "exports.value = 1;")
            .expect("CommonJS source compile must succeed");

        assert!(program.metadata.is_source_mode);
    }

    #[test]
    fn source_compiler_preserves_strings_for_merge() {
        // After compile, program.strings must be non-empty for sources that
        // reference string literals — this enables string table merging in
        // program_to_abc_file.
        let options = make_options(true, true);
        let compiler = SourceProgramCompiler::new(&options);

        let mut sf = crate::aot::options::SourceFile::new_source("d.ts", "ts");
        sf.script_kind = ScriptKind::Module;
        sf.record_name = Box::from("test/strings");

        let program = compiler
            .compile(&sf, r#"const msg = "hello world";"#)
            .expect("compile with string literal must succeed");

        // The source has at least the string "hello world" — program.strings
        // must not be empty.
        assert!(!program.strings.is_empty(),
            "Program.strings must capture strings for merge remapping");
    }
}
