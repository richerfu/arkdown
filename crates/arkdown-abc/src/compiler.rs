use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::HvigorSourceMapScope;
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_index::IndexVec;
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::FxHashMap;

use crate::aot::options::{
    AotCompileOptions, AotOutput, CompileContextInfo, ScriptKind, SourceFile,
};
use crate::aot::runner::AotRunner;
use crate::cache::{
    abc_input_cache_hash, source_cache_hash, AbcProgramsCacheFile, ProgramCacheFile,
    ProgramProtoFile,
};
use crate::dependency::{
    compact_to_retained_programs, resolve_deps_relation, AbcDependencyContext,
};
use crate::emit::file::{version_for_target_api, AbcFile};
use crate::emit::record::{ClassRecord, FieldRecord, FieldType, FieldValue, FIELD_OWNER_CLASS_IDX};
use crate::error::AbcError;
use crate::ir::builder;
use crate::optimizer::{optimize_records, BytecodeOptimizerOptions};
use crate::program::ids::SourceFileId;
use crate::program_cache::{append_program_to_file, program_for_file};
use crate::widget::WidgetCompileConfig;

/// Options controlling how the [`AbcCompiler`] compiles JavaScript to Panda
/// bytecode.
pub struct AbcCompileOptions {
    /// Embed debug information (source positions, variable names) in the output.
    pub debug_info: bool,
    /// Optimisation level: 0 = none, 1 = basic, 2 = full.
    pub opt_level: u8,
    /// Optimize functions with try/catch, matching upstream
    /// `--opt-try-catch-func` defaulting to true.
    pub optimize_try_catch_func: bool,
    /// Target OpenHarmony API version (e.g. `12` for API 12 / HarmonyOS 4.x).
    pub target_api_version: u32,
    /// Optional target API stage/sub-version passed as `--target-api-sub-version`.
    pub target_api_sub_version: Option<String>,
    /// Match upstream `--enable-annotations`, effective only for API 20+.
    pub enable_annotations: bool,
    /// Merge all records into a single `.abc` output file.
    pub merge_abc: bool,
    /// Parse and lower inputs as ECMAScript modules, matching upstream
    /// `es2abc --module`.
    pub module: bool,
    /// Optional field name for non-merge `_ESModuleRecord`, matching upstream
    /// `--module-record-field-name`.
    pub module_record_field_name: Option<String>,
    /// Optional hvigor/es2abc `--npm-module-entry-list` file.
    ///
    /// Upstream `CompileNpmModuleEntryList` lowers each line into a standalone
    /// `pandasm::Record` with one `u8=false` field; it is not parsed as JS.
    pub npm_entries: Option<PathBuf>,
    /// Optional upstream active compile-context dependency roots.
    ///
    /// When present, the compiler runs the same `ResolveDepsRelations` pass as
    /// es2abc before emitting ABC. `remove_redundant_file` controls whether the
    /// resolved relation is only dumped or also applied to the Program table.
    pub dependency_context: Option<AbcDependencyContext>,
    /// Match upstream `--remove-redundant-file`: after dependency resolution,
    /// remove programs that are not reachable from compile entries.
    pub remove_redundant_file: bool,
    /// Match upstream `--dump-deps-info`: print resolved dependency files and
    /// records after BFS resolution.
    pub dump_deps_info: bool,
}

/// Per-source upstream ProgramCache output requested by `es2abc --cache-file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbcProgramCacheInput {
    /// `SourceFile::fileName` in upstream es2abc. It is part of the cache hash
    /// and is the key used by `--cache-file @modules.cache`.
    pub file_name: String,
    /// Destination `.protoBin` file for this source.
    pub cache_file: PathBuf,
}

/// A precompiled Panda ABC file accepted through upstream `--enable-abc-input`.
#[derive(Debug, Clone)]
pub struct AbcInputFile {
    pub file_name: String,
    pub record_name: String,
    pub source_file: String,
    pub package_name: String,
    pub cache_file: Option<PathBuf>,
    pub bytes: Vec<u8>,
}

/// An input source file to compile.
pub struct AbcModule {
    /// Record name — typically the OHM URL of the chunk
    /// (`@bundle:<bundleName>/<moduleName>/<relpath>`).
    pub record_name: String,
    /// JavaScript/TypeScript source passed to the bytecode compiler.
    ///
    /// Upstream `es2abc` accepts normal files directly; when `--module` is set,
    /// the input is parsed as an ECMAScript module. IIFE-wrapped CommonJS is
    /// supported only as a legacy/non-module compatibility shape.
    pub source: String,
    /// Optional source-map JSON string accepted for API compatibility.
    ///
    /// This is not used to derive ABC DebugInfo line/column coordinates.
    pub source_map: Option<String>,
    /// Optional debug source-file name stored in the ABC DebugInfo section.
    ///
    /// Upstream `es2abc` gets this from `filesInfo.txt`; source maps are not
    /// inputs to `BuildLineNumberProgram()`.
    pub source_file: Option<String>,
    /// Package name stored in the module metadata `pkgName@...` field.
    ///
    /// Upstream `es2abc` reads this from column 5 of `filesInfo.txt` and does
    /// not derive it from the record name.
    pub package_name: Option<String>,
    /// Whether this record is compiled from a CommonJS source file.
    ///
    /// Upstream `es2abc` reads this from column 3 of `filesInfo.txt` and writes
    /// it to the module metadata `isCommonjs` field.
    pub is_commonjs: bool,
    /// Whether this record belongs to an OpenHarmony shared module.
    ///
    /// Upstream `es2abc` reads this from column 6 of `filesInfo.txt`, stores it
    /// on `SourceFile::isSharedModule`, then writes `Program::IsShared()` to
    /// the module metadata `isSharedModule` field.
    pub is_shared_module: bool,
    /// Original source language string from column 7 of `filesInfo.txt`.
    ///
    /// Upstream stores this on `SourceFile::sourceLang`, then
    /// `Program::SetSourceLang()` maps `ets`/`ts`/`js` to the ABC
    /// `SOURCE_LANG` byte. Empty or unknown strings fall back to ECMAScript.
    pub origin_source_lang: String,
}

/// Output produced by a successful compilation run.
pub struct AbcArtifactResult {
    /// Path of the written `.abc` file.
    pub path: PathBuf,
    /// API version the file was compiled for.
    pub api_version: u32,
    /// Whether the file contains embedded debug information.
    pub has_debug_info: bool,
}

struct BuiltAbcFile {
    file: AbcFile,
    has_debug_info: bool,
}

/// Strip TypeScript type annotations from source before bytecode generation.
///
/// The bundler pipeline may emit TypeScript-flavoured code when the OXC parser
/// falls back to raw source for `.ets` files. This function normalises source
/// through OXC before bytecode lowering.
///
/// Normalise `source` through a TypeScript parse + Codegen round-trip.
///
/// The primary purpose is to remove non-standard ArkTS/ETS constructs that
/// would confuse the downstream `SourceType::ts()` parser used by
/// `build_record`. Standard TypeScript annotations (`): void`, `: Want`)
/// survive the round-trip unchanged — that is intentional: `build_record`
/// now accepts TypeScript source directly via `SourceType::ts()`, so we
/// no longer need to fully strip the annotations here.
///
/// Public crate wrapper for stripping TypeScript types.
pub(crate) fn strip_typescript_types_pub(source: &str, file: &str) -> Result<String, AbcError> {
    strip_typescript_types(source, file)
}

fn strip_typescript_types(source: &str, file: &str) -> Result<String, AbcError> {
    if file.ends_with(".ets") {
        return Ok(source.to_string());
    }

    let allocator = Allocator::default();
    let parse_ret = Parser::new(&allocator, source, SourceType::ts()).parse();
    if parse_ret.panicked {
        return Err(AbcError::Parse {
            file: file.to_string(),
            message: parse_ret
                .errors
                .first()
                .map(|error| error.message.to_string())
                .unwrap_or_else(|| "parser panic".to_string()),
        });
    }
    Ok(Codegen::new().build(&parse_ret.program).code)
}

fn source_file_from_record_name(record_name: &str) -> String {
    HvigorSourceMapScope::source_file_from_record_name(record_name).unwrap_or_else(|| {
        record_name
            .strip_prefix("@bundle:")
            .unwrap_or(record_name)
            .to_string()
    })
}

/// Public wrapper for compile_queue's use.
pub(crate) fn non_merge_metadata_records_pub(
    record: &mut ClassRecord,
    source_file: &str,
    is_commonjs: bool,
    is_module: bool,
    module_record_field_name: Option<&str>,
) -> Vec<ClassRecord> {
    non_merge_metadata_records(record, source_file, is_commonjs, is_module, module_record_field_name)
}

fn non_merge_metadata_records(
    record: &mut ClassRecord,
    source_file: &str,
    is_commonjs: bool,
    is_module: bool,
    module_record_field_name: Option<&str>,
) -> Vec<ClassRecord> {
    let source_lang = record.source_lang;
    let mut records = Vec::new();
    let mut module_record_idx = None;
    let mut scope_names_idx = None;
    let mut has_top_level_await = false;
    let mut is_shared_module = false;

    for field in std::mem::take(&mut record.fields) {
        match field.name.as_str() {
            "moduleRecordIdx" => {
                if let Some(FieldValue::LiteralArray(idx)) = field.value {
                    module_record_idx = Some(idx);
                }
            }
            "scopeNames" => {
                if let Some(FieldValue::LiteralArray(idx)) = field.value {
                    scope_names_idx = Some(idx);
                }
            }
            "hasTopLevelAwait" => {
                has_top_level_await =
                    matches!(field.value, Some(FieldValue::U8(value)) if value != 0);
            }
            "isSharedModule" => {
                is_shared_module = matches!(field.value, Some(FieldValue::U8(value)) if value != 0);
            }
            "isCommonjs" => {}
            _ if field.name.starts_with("pkgName@") => {}
            _ => record.fields.push(field),
        }
    }

    if is_commonjs {
        records.push(metadata_record(
            "_CommonJsRecord",
            source_lang,
            vec![u8_metadata_field("isCommonJs", 1)],
            vec![],
        ));
    }

    if let Some(scope_names_idx) = scope_names_idx {
        records.push(metadata_record(
            "_ESScopeNamesRecord",
            source_lang,
            vec![literal_metadata_field(source_file, scope_names_idx)],
            vec![scope_names_idx],
        ));
    }

    if is_module {
        if let Some(module_record_idx) = module_record_idx {
            let module_field_name = module_record_field_name.unwrap_or(source_file);
            records.push(metadata_record(
                "_ESModuleRecord",
                source_lang,
                vec![literal_metadata_field(module_field_name, module_record_idx)],
                vec![module_record_idx],
            ));
        }

        if has_top_level_await {
            records.push(metadata_record(
                "_HasTopLevelAwait",
                source_lang,
                vec![u8_metadata_field("hasTopLevelAwait", 1)],
                vec![],
            ));
        }
    }

    if is_shared_module {
        records.push(metadata_record(
            "_SharedModuleRecord",
            source_lang,
            vec![u8_metadata_field("isSharedModule", 1)],
            vec![],
        ));
    }

    records
}

fn metadata_record(
    name: impl Into<String>,
    source_lang: u8,
    fields: Vec<FieldRecord>,
    module_literal_indices: Vec<usize>,
) -> ClassRecord {
    ClassRecord {
        name: name.into(),
        methods: vec![],
        string_indices: vec![],
        literal_indices: vec![],
        module_literal_indices,
        source_code_strings: vec![],
        access_flags: 1,
        source_lang,
        metadata_attrs: Default::default(),
        conflict: false,
        params_num: 0,
        body_presence: false,
        file_location: None,
        standalone_ecmascript_input: false,
        source_file: None,
        annotations: vec![],
        module_requests: vec![],
        fields,
    }
}

fn u8_metadata_field(name: impl Into<String>, value: u8) -> FieldRecord {
    FieldRecord {
        name: name.into(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 0,
        field_type: FieldType::U8,
        metadata_field_type: None,
        access_flags: 0,
        value: Some(FieldValue::U8(value)),
        annotations: vec![],
        metadata_attrs: Default::default(),
        source: Default::default(),
    }
}

fn literal_metadata_field(name: impl Into<String>, literal_idx: usize) -> FieldRecord {
    FieldRecord {
        name: name.into(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 1,
        field_type: FieldType::U32,
        metadata_field_type: None,
        access_flags: 0,
        value: Some(FieldValue::LiteralArray(literal_idx)),
        annotations: vec![],
        metadata_attrs: Default::default(),
        source: Default::default(),
    }
}

fn abc_input_program_key(file_name: &str, record_name: &str) -> String {
    format!("{file_name}|{record_name}")
}

/// Pure-Rust ArkCompiler bytecode compiler.
///
/// Replaces the `es2abc` SDK binary described in SPEC-009. Upstream `es2abc`
/// accepts source files directly; `--module` selects ECMAScript module parsing
/// rather than an IIFE entry contract.
///
/// # Example
///
/// ```rust,ignore
/// use arkdown_abc::{AbcCompiler, AbcCompileOptions, AbcModule};
/// use std::path::Path;
///
/// let compiler = AbcCompiler::new(AbcCompileOptions {
///     debug_info: false,
///     opt_level: 0,
///     optimize_try_catch_func: true,
///     target_api_version: 12,
///     target_api_sub_version: None,
///     merge_abc: true,
///     module: true,
///     module_record_field_name: None,
///     npm_entries: None,
/// });
///
/// let result = compiler.compile(
///     vec![AbcModule {
///         record_name: "entry/main".into(),
///         source: "export const x = 1;".into(),
///         source_map: None,
///         source_file: None,
///         package_name: None,
///         is_commonjs: false,
///         is_shared_module: false,
///         origin_source_lang: String::new(),
///     }],
///     Path::new("output/modules.abc"),
/// )?;
/// println!("wrote {} bytes to {:?}", std::fs::metadata(&result.path)?.len(), result.path);
/// ```
pub struct AbcCompiler {
    options: AbcCompileOptions,
}

impl AbcCompiler {
    /// Create a new compiler with the given options.
    pub fn new(options: AbcCompileOptions) -> Self {
        AbcCompiler { options }
    }

    /// Run the widget compilation pass.
    ///
    /// This is a second, independent ABC compilation pass for ArkTS widgets
    /// (form extension abilities).  It mirrors `initWidgetDefaultArkCompileConfig`
    /// + `submitArkCompileWork` in hvigor's `ArkCompile.doTaskAction()`:
    ///
    /// - Uses a **separate cache directory** (`config.cache_dir`) so widget
    ///   incremental state does not corrupt the main module cache.
    /// - Writes the output to `config.output` (typically `widgets.abc`), not
    ///   the main `modules.abc` path.
    /// - Warnings are suppressed (`ignoreWarning: true` in hvigor).
    ///
    /// # Errors
    ///
    /// Returns [`AbcError::Io`] if the output directory cannot be created or
    /// the file cannot be written.  Returns [`AbcError::Parse`] if any widget
    /// source contains hard syntax errors.
    pub fn compile_widget(
        &self,
        config: &WidgetCompileConfig,
    ) -> Result<AbcArtifactResult, AbcError> {
        // Ensure the widget cache directory exists.
        if !config.cache_dir.as_os_str().is_empty() {
            fs_err::create_dir_all(&config.cache_dir).map_err(AbcError::Io)?;
        }

        // Build a fresh AbcFile for the widget pass — completely independent
        // from the main module's AbcFile.
        let mut abc_file = AbcFile::new();
        let mut has_debug_info = false;

        for input in &config.inputs {
            if !input.path.exists() {
                // Skip missing widget source files; hvigor sets ignoreWarning=true
                // so we mirror that leniency here.
                continue;
            }
            let source = fs_err::read_to_string(&input.path).map_err(AbcError::Io)?;
            let js_source = strip_typescript_types(&source, &input.path.to_string_lossy())?;
            let source_file = input
                .source_file
                .as_deref()
                .map(str::to_string)
                .or_else(|| {
                    config
                        .debug_info
                        .then(|| source_file_from_record_name(&input.record_name))
                });
            has_debug_info |= source_file.is_some();
            let record = builder::build_record_with_debug_and_package_for_target(
                &js_source,
                &input.record_name,
                &mut abc_file.strings,
                &mut abc_file.literal_arrays,
                source_file.as_deref(),
                None,
                false,
                // Widget source files are always compiled as ESM modules.
                true,
                config.target_api_version,
                None,
            )?;
            abc_file.add_record(record);
        }

        let bytes = abc_file
            .serialize_with_version(version_for_target_api(config.target_api_version, None));

        if let Some(parent) = config.output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
            }
        }
        std::fs::write(&config.output, &bytes).map_err(AbcError::Io)?;

        Ok(AbcArtifactResult {
            path: config.output.clone(),
            api_version: config.target_api_version,
            has_debug_info,
        })
    }

    /// Compile `modules` and write the result to `output`.
    ///
    /// Each module in `modules` becomes one [`ClassRecord`][crate::emit::record::ClassRecord].
    /// If `merge_abc` is `true`, all records are combined into a single file at
    /// `output`.  If `merge_abc` is `false`, this API follows upstream es2abc's
    /// single-output path and accepts exactly one source module.
    ///
    /// Returns [`AbcError::Io`] if the output directory cannot be created or
    /// the file cannot be written.  Returns [`AbcError::Parse`] if any module
    /// source contains hard syntax errors.
    pub fn compile(
        &self,
        modules: Vec<AbcModule>,
        output: &Path,
    ) -> Result<AbcArtifactResult, AbcError> {
        self.compile_with_program_cache(modules, output, Vec::new())
    }

    /// Compile `modules`, write ABC output, and emit upstream-compatible
    /// ProgramCache `.protoBin` files for requested source entries.
    ///
    /// `cache_inputs` is positional and must either be empty or have the same
    /// length as `modules`, matching upstream's per-`SourceFile::fileName`
    /// cache map.
    ///
    /// # SPEC-049 Phase 11b
    ///
    /// This method is now a thin facade over `AotRunner`.  It converts the
    /// legacy `AbcModule` input format to the upstream-shaped `AotCompileOptions`
    /// and delegates all compilation to the `AotRunner` orchestration pipeline.
    pub fn compile_with_program_cache(
        &self,
        modules: Vec<AbcModule>,
        output: &Path,
        cache_inputs: Vec<Option<AbcProgramCacheInput>>,
    ) -> Result<AbcArtifactResult, AbcError> {
        let aot_options =
            self.build_aot_options_from_modules(&modules, output, &cache_inputs, None)?;

        let result = AotRunner::new(aot_options).run()?;

        Ok(AbcArtifactResult {
            path: result.abc_paths.into_iter().next().unwrap_or_else(|| output.to_path_buf()),
            api_version: result.api_version,
            has_debug_info: result.has_debug_info,
        })
    }

    /// Convert `AbcModule` inputs to upstream-shaped `AotCompileOptions`.
    ///
    /// This adapter enables the legacy public API to delegate to `AotRunner`
    /// without requiring callers to construct `AotCompileOptions` directly.
    fn build_aot_options_from_modules(
        &self,
        modules: &[AbcModule],
        output: &Path,
        cache_inputs: &[Option<AbcProgramCacheInput>],
        output_proto: Option<PathBuf>,
    ) -> Result<AotCompileOptions, AbcError> {
        if !cache_inputs.is_empty() && cache_inputs.len() != modules.len() {
            return Err(AbcError::UnsupportedConstruct {
                construct: "ProgramCache input count must match source module count".to_string(),
                file: output.display().to_string(),
            });
        }

        if !self.options.merge_abc && modules.len() > 1 {
            return Err(AbcError::UnsupportedConstruct {
                construct:
                    "merge_abc=false with multiple source files requires per-input output files"
                        .to_string(),
                file: output.display().to_string(),
            });
        }

        // Keep user's opt_level as-is; effective_debug_info() in AotCompileOptions
        // handles suppression: debug_info is suppressed when opt_level > 0.
        let effective_opt_level = self.options.opt_level;

        let mut source_files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
        let mut cache_files: FxHashMap<Box<str>, PathBuf> = FxHashMap::default();

        for (idx, module) in modules.iter().enumerate() {
            let cache_input = cache_inputs.get(idx).and_then(Option::as_ref);

            // Determine script kind from module flags.
            let script_kind = if module.is_commonjs {
                ScriptKind::CommonJs
            } else if self.options.module {
                ScriptKind::Module
            } else {
                ScriptKind::Script
            };

            // Build SourceFile from AbcModule.
            // Use the record_name as the program key (always unique per module).
            // The cache key uses file_name from cache_input when available.
            // NOTE: source_file must NOT be used as the program key because
            // multiple modules can share the same source_file string (e.g. when
            // different records from the same file are compiled separately).
            let file_name_str = cache_input
                .map(|ci| ci.file_name.clone())
                .unwrap_or_else(|| module.record_name.clone());

            let mut sf = SourceFile {
                file_name: PathBuf::from(&file_name_str),
                record_name: Box::from(module.record_name.as_str()),
                script_kind,
                script_extension: crate::aot::options::ScriptExtension::from_filename(
                    &file_name_str, "ts",
                ),
                source_file: Box::from(
                    module.source_file.as_deref().unwrap_or(""),
                ),
                pkg_name: Box::from(
                    module.package_name.as_deref().unwrap_or(""),
                ),
                is_shared_module: module.is_shared_module,
                source_lang: Box::from(module.origin_source_lang.as_str()),
                is_source_mode: true,
                inline_source: Some(Arc::from(module.source.as_str())),
                hash: None,
            };

            // Register cache file mapping.
            if let Some(ci) = cache_input {
                cache_files.insert(Box::from(file_name_str.as_str()), ci.cache_file.clone());
            }

            source_files.push(sf);
        }

        // Build compile context from dependency context if present.
        let compile_context_info = if let Some(dep_ctx) = &self.options.dependency_context {
            CompileContextInfo {
                compile_entries: dep_ctx
                    .compile_entries
                    .iter()
                    .map(|s| Box::from(s.as_str()))
                    .collect(),
                hsp_pkg_names: dep_ctx
                    .external_pkg_names
                    .iter()
                    .map(|s| Box::from(s.as_str()))
                    .collect(),
                ..Default::default()
            }
        } else {
            CompileContextInfo::default()
        };

        Ok(AotCompileOptions {
            source_files,
            output: AotOutput::Path(output.to_path_buf()),
            merge_abc: self.options.merge_abc,
            debug_info: self.options.debug_info,
            enable_column: false,
            opt_level: effective_opt_level,
            optimize_try_catch_func: self.options.optimize_try_catch_func,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: self.options.target_api_version,
            target_api_sub_version: self.options.target_api_sub_version.as_deref().map(Box::from),
            cache_files,
            npm_module_entry_list: self.options.npm_entries.clone(),
            compile_context_info,
            enable_abc_input: false,
            dump_deps_info: self.options.dump_deps_info,
            remove_redundant_file: self.options.remove_redundant_file,
            output_proto,
            module_record_field_name: self.options.module_record_field_name.as_deref().map(Box::from),
            enable_annotations: self.options.enable_annotations,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None, // AbcCompileOptions does not expose use_define_semantic
        })
    }

    /// Re-emit precompiled ABC inputs through the Rust abc2program path.
    ///
    /// This mirrors upstream's ABC input flow: read Panda file, recover
    /// Program-like records, then feed them back through the normal emitter.
    pub fn compile_abc_inputs(
        &self,
        inputs: Vec<AbcInputFile>,
        output: &Path,
    ) -> Result<AbcArtifactResult, AbcError> {
        let built = self.build_abc_input_file(inputs, output)?;
        let bytes = built.file.serialize_with_version(version_for_target_api(
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
        ));
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
            }
        }
        std::fs::write(output, &bytes).map_err(AbcError::Io)?;

        Ok(AbcArtifactResult {
            path: output.to_path_buf(),
            api_version: self.options.target_api_version,
            has_debug_info: built.has_debug_info,
        })
    }

    /// Materialize `protoPanda::Program` from ABC input through Rust
    /// abc2program, matching upstream's `--outputProto` snapshot shape.
    pub fn compile_abc_input_program_proto(
        &self,
        inputs: Vec<AbcInputFile>,
        output_proto: &Path,
    ) -> Result<AbcArtifactResult, AbcError> {
        let built = self.build_abc_input_file(inputs, output_proto)?;
        let program = program_for_file(&built.file, self.options.target_api_version)?;
        ProgramProtoFile::write(output_proto, program)?;

        Ok(AbcArtifactResult {
            path: output_proto.to_path_buf(),
            api_version: self.options.target_api_version,
            has_debug_info: built.has_debug_info,
        })
    }

    fn build_abc_input_file(
        &self,
        inputs: Vec<AbcInputFile>,
        diagnostic_path: &Path,
    ) -> Result<BuiltAbcFile, AbcError> {
        let panda_file_version = version_for_target_api(
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
        );
        let mut abc_file = AbcFile::new();
        let mut record_program_keys = Vec::new();
        // ABC input records are precompiled bytecode and are never re-optimized.
        // record_needs_optimization removed per SPEC-049 Phase 11b guardrail.

        for input in &inputs {
            let hash_code = input.cache_file.as_ref().map(|_| {
                abc_input_cache_hash(
                    &input.bytes,
                    &input.file_name,
                    &input.record_name,
                    &input.source_file,
                    &input.package_name,
                    panda_file_version,
                )
            });
            if let (Some(cache_file), Some(hash_code)) = (&input.cache_file, hash_code) {
                if let Some(programs) =
                    AbcProgramsCacheFile::read_if_hash_matches(cache_file, hash_code)?
                {
                    for (program_key, program) in programs {
                        let appended_records = append_program_to_file(&mut abc_file, program)?;
                        record_program_keys.extend(std::iter::repeat_n(
                            program_key.clone(),
                            appended_records.len(),
                        ));
                    }
                    continue;
                }
            }

            let appended = crate::abc2program::append_abc_file(&mut abc_file, &input.bytes)?;
            let appended_program_keys = appended
                .iter()
                .filter_map(|&record_index| {
                    abc_file
                        .records
                        .get(record_index)
                        .map(|record| abc_input_program_key(&input.file_name, &record.name))
                })
                .collect::<Vec<_>>();
            record_program_keys.extend(appended_program_keys.iter().cloned());

            if let (Some(cache_file), Some(hash_code)) = (&input.cache_file, hash_code) {
                let mut programs = Vec::with_capacity(appended.len());
                for (record_index, program_key) in appended.iter().zip(appended_program_keys) {
                    let Some(program) = crate::program_cache::program_for_record(
                        &abc_file,
                        *record_index,
                        self.options.target_api_version,
                    )?
                    else {
                        return Err(AbcError::UnsupportedConstruct {
                            construct: format!(
                                "failed to materialize upstream ABC input ProgramCache for record index {record_index}"
                            ),
                            file: cache_file.display().to_string(),
                        });
                    };
                    programs.push((program_key, program));
                }
                AbcProgramsCacheFile::write(cache_file, hash_code, programs)
                    .map_err(AbcError::Io)?;
            }
        }

        if let Some(context) = &self.options.dependency_context {
            let resolved = resolve_deps_relation(
                &abc_file,
                &record_program_keys,
                context,
                self.options.dump_deps_info,
            );
            if self.options.remove_redundant_file {
                let keep_mask = record_program_keys
                    .iter()
                    .map(|program_key| resolved.contains_program(program_key))
                    .collect::<Vec<_>>();
                compact_to_retained_programs(&mut abc_file, &mut record_program_keys, &keep_mask);
            }
        }

        // ABC input records are precompiled bytecode and are never re-optimized.
        let should_skip_optimization = vec![false; abc_file.records.len()];
        optimize_records(
            &mut abc_file,
            BytecodeOptimizerOptions::from_compile_options(
                self.options.opt_level,
                self.options.optimize_try_catch_func,
            ),
            &should_skip_optimization,
        );

        if abc_file.records.is_empty() {
            return Err(AbcError::UnsupportedConstruct {
                construct: "ABC input did not contain any user records".to_string(),
                file: diagnostic_path.display().to_string(),
            });
        }

        Ok(BuiltAbcFile {
            file: abc_file,
            has_debug_info: false,
        })
    }

    /// Compile one upstream pandasm `Program` snapshot and write it as
    /// `protoPanda::Program`, matching `es2abc --outputProto`.
    ///
    /// Upstream `GenerateProgram()` returns immediately after this snapshot for
    /// a single Program, before `.abc` emission and cache-file emit jobs.
    /// Compile one upstream pandasm `Program` snapshot and write it as
    /// `protoPanda::Program`, matching `es2abc --outputProto`.
    ///
    /// # SPEC-049 Phase 11b
    ///
    /// Routes through `AotRunner` with `output_proto` set, which returns before
    /// final ABC emission (matching upstream `GenerateProgram` early-return for
    /// single-program `--outputProto`).
    pub fn compile_program_proto(
        &self,
        modules: Vec<AbcModule>,
        output_proto: &Path,
        cache_inputs: Vec<Option<AbcProgramCacheInput>>,
    ) -> Result<AbcArtifactResult, AbcError> {
        // Use a dummy output path; AotRunner will not write an ABC file when
        // output_proto is set and the program count is 1.
        let dummy_output = output_proto.with_extension("abc");
        let aot_options = self.build_aot_options_from_modules(
            &modules,
            &dummy_output,
            &cache_inputs,
            Some(output_proto.to_path_buf()),
        )?;

        let result = AotRunner::new(aot_options).run()?;

        Ok(AbcArtifactResult {
            path: result.proto_path.unwrap_or_else(|| output_proto.to_path_buf()),
            api_version: result.api_version,
            has_debug_info: result.has_debug_info,
        })
    }

    // build_abc_file and normalize_source_file were deleted as dead code
    // (SPEC-049 AOT refactor): all compilation now routes through AotRunner
    // via build_aot_options_from_modules.
}

// ---------------------------------------------------------------------------
// Pre-compile unsupported syntax detection
// ---------------------------------------------------------------------------

/// Check `source` for constructs that are unsupported in the ArkTS / es2abc
/// lowering pipeline.
///
/// ArkTS runs in implicit strict mode.  `es2abc` / `es2panda` rejects these
/// constructs with hard compilation errors; we must do the same to avoid
/// producing silently incorrect bytecode.
///
/// Checked constructs — aligned with `es2panda` `ETSChecker` / strict mode
/// validation (section 3.6: "对照 upstream es2panda 补 lowering；unsupported
/// 必须 hard error"):
///
/// Strict mode violations:
/// - `with` statement — forbidden in strict mode.
/// - Direct `eval()` call — ArkRuntime does not support `eval`.
/// - `arguments.callee` — forbidden in strict mode.
/// - `delete unqualifiedName` — deleting an unqualified variable name
///   (`delete x`) is a strict mode error; deleting a property (`delete o.x`)
///   is allowed.
/// - Assignment to `eval` or `arguments` — forbidden in strict mode.
/// - Duplicate formal parameters — `function f(a, a){}` is a strict error.
///
/// ArkTS-specific:
/// - Labeled function declarations — `label: function f(){}` is illegal in
///   strict mode and ArkTS; the ArkTS transpiler never emits such code and
///   es2panda rejects it.
/// - Static class member named `prototype` — `static prototype = …` is a
///   syntax error per the ES spec; es2panda rejects this as well.
///
/// The check performs a quick AST walk via `oxc_ast_visit::Visit`.
/// It does NOT perform type-level analysis (that requires the TypeScript type
/// service which is outside our scope).
pub(crate) fn check_unsupported_syntax(
    source: &str,
    record_name: &str,
) -> Result<(), crate::error::AbcError> {
    use oxc_ast::ast::{AssignmentTarget, Expression, MemberExpression, PropertyKey};
    use oxc_ast_visit::{walk, Visit};

    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, source, source_type).parse();

    // If parsing failed, let the downstream compile step report the error.
    // We only report unsupported-construct errors for valid-but-forbidden code.
    if ret.panicked || !ret.errors.is_empty() {
        return Ok(());
    }

    struct UnsupportedSyntaxVisitor<'a> {
        record_name: &'a str,
        error: Option<crate::error::AbcError>,
    }

    impl<'a, 'b> Visit<'b> for UnsupportedSyntaxVisitor<'a> {
        fn visit_with_statement(&mut self, stmt: &oxc_ast::ast::WithStatement<'b>) {
            if self.error.is_none() {
                self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                    construct: "WithStatement".to_string(),
                    file: self.record_name.to_string(),
                });
            }
            walk::walk_with_statement(self, stmt);
        }

        fn visit_call_expression(&mut self, expr: &oxc_ast::ast::CallExpression<'b>) {
            if self.error.is_none() {
                // Direct eval() call: callee is an Identifier named "eval".
                if let Expression::Identifier(id) = &expr.callee {
                    if id.name.as_str() == "eval" {
                        self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                            construct: "direct eval() call".to_string(),
                            file: self.record_name.to_string(),
                        });
                    }
                }
            }
            walk::walk_call_expression(self, expr);
        }

        fn visit_member_expression(&mut self, expr: &MemberExpression<'b>) {
            if self.error.is_none() {
                // arguments.callee — forbidden in strict mode.
                if let MemberExpression::StaticMemberExpression(static_expr) = expr {
                    if let Expression::Identifier(obj) = &static_expr.object {
                        if obj.name.as_str() == "arguments"
                            && static_expr.property.name.as_str() == "callee"
                        {
                            self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                                construct: "arguments.callee (forbidden in strict mode)"
                                    .to_string(),
                                file: self.record_name.to_string(),
                            });
                        }
                    }
                }
            }
            walk::walk_member_expression(self, expr);
        }

        fn visit_unary_expression(&mut self, expr: &oxc_ast::ast::UnaryExpression<'b>) {
            // Note: `delete identifier` is a strict-mode SyntaxError in the ES spec,
            // but es2panda COMPILES it and generates specific bytecode (global-delete
            // or false-literal for bound vars) rather than rejecting it.
            // We therefore do not hard-error here — es2abc/es2panda alignment
            // means we generate the same bytecode, not a compile error.
            walk::walk_unary_expression(self, expr);
        }

        fn visit_assignment_expression(&mut self, expr: &oxc_ast::ast::AssignmentExpression<'b>) {
            // Assignment to `eval` or `arguments` — forbidden in strict mode.
            // e.g. `eval = 1;` or `arguments = [];`
            // Matches es2panda's strict mode identifier assignment restriction.
            if self.error.is_none() {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &expr.left {
                    let name = id.name.as_str();
                    if name == "eval" || name == "arguments" {
                        self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                            construct: format!(
                                "assignment to '{}' is forbidden in strict mode \
                                 (ArkTS / es2panda strict rule: eval and arguments \
                                 cannot be used as assignment targets)",
                                name
                            ),
                            file: self.record_name.to_string(),
                        });
                    }
                }
            }
            walk::walk_assignment_expression(self, expr);
        }

        fn visit_labeled_statement(&mut self, stmt: &oxc_ast::ast::LabeledStatement<'b>) {
            // Labeled function declarations — `label: function f(){}` — are
            // forbidden in strict mode per ES2015+ and by es2panda / ArkTS.
            if self.error.is_none() {
                if matches!(stmt.body, oxc_ast::ast::Statement::FunctionDeclaration(_)) {
                    self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                        construct: "labeled function declaration (forbidden in strict mode / ArkTS; es2panda rejects this)".to_string(),
                        file: self.record_name.to_string(),
                    });
                }
            }
            walk::walk_labeled_statement(self, stmt);
        }

        fn visit_class(&mut self, class: &oxc_ast::ast::Class<'b>) {
            // Static METHOD named `prototype` is a hard Syntax Error per ES2022+
            // §15.7.1 "It is a Syntax Error if PropName of MethodDefinition is
            // 'prototype' [in a static context]."
            // Note: static PROPERTY named prototype (e.g. `static prototype = 1`)
            // is valid per the spec; only static methods are forbidden.
            // es2panda enforces this restriction.
            if self.error.is_none() {
                for elem in &class.body.body {
                    if let oxc_ast::ast::ClassElement::MethodDefinition(m) = elem {
                        if m.r#static {
                            if matches!(&m.key, PropertyKey::StaticIdentifier(id) if id.name.as_str() == "prototype")
                            {
                                self.error = Some(crate::error::AbcError::UnsupportedConstruct {
                                    construct: "static method named 'prototype' (Syntax Error per ES2022+ §15.7.1; rejected by es2panda/ArkTS)".to_string(),
                                    file: self.record_name.to_string(),
                                });
                                break;
                            }
                        }
                    }
                }
            }
            walk::walk_class(self, class);
        }
    }

    let mut visitor = UnsupportedSyntaxVisitor {
        record_name,
        error: None,
    };
    visitor.visit_program(&ret.program);

    if let Some(err) = visitor.error {
        return Err(err);
    }

    Ok(())
}
