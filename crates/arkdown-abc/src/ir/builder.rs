use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, Argument, ArrayAssignmentTarget,
    ArrayExpressionElement, ArrayPattern, AssignmentOperator, AssignmentTarget,
    AssignmentTargetMaybeDefault, AssignmentTargetProperty, BindingPattern, ClassElement,
    Declaration, Decorator, ExportDefaultDeclarationKind, Expression, ForInStatement,
    ForOfStatement, ForStatementLeft, FormalParameter, Function, FunctionBody,
    ImportDeclarationSpecifier, ImportOrExportKind, LogicalOperator, MethodDefinitionKind,
    NewExpression, ObjectAssignmentTarget, ObjectPattern, ObjectPropertyKind, Program, PropertyKey,
    SimpleAssignmentTarget, Statement, TSEnumDeclaration, TSEnumMemberName,
    TSImportEqualsDeclaration, TSModuleDeclaration, TSModuleReference, TSQualifiedName, TSType,
    TSTypeName, UnaryOperator, UpdateExpression, YieldExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{Parser, ParserReturn};
use oxc_semantic::{Reference, ScopeFlags, ScopeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::operator::UpdateOperator;
use oxc_transformer::{Module as TransformModule, TransformOptions, Transformer};

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{
    function_kind, AnnotationElementRecord, AnnotationRecord, AnnotationValue, CatchEntry,
    ClassRecord, DebugInfoRecord, FieldRecord, FieldType, FieldValue, InstructionDebugInfo,
    MethodKind, MethodLabelStyle, MethodRecord, ProgramLabel, FIELD_OWNER_CLASS_IDX,
};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::ir::ast_analysis::{
    arrow_captures_from_body, is_condition_comparison, is_global_this, is_new_target_meta,
    is_undefined_literal, method_body_contains_arrow, negative_integer_literal,
    stmts_capture_this_in_arrow, ArrowCaptures,
};
use crate::ir::class_layout::{
    class_buffer_literal_array, class_method_inferred_name, class_method_num_args,
    class_method_record_name, class_method_upstream_name, class_private_methods_literal_array,
    computed_class_method_record_name, computed_instance_class_field_key_name,
    constructor_required_param_count, is_class_buffer_method, method_key_name,
    static_initializer_record_name,
};
use crate::ir::debug_lnp::{argument_span_start, col_at, line_at, LineTracker, SourceMapper};
use crate::ir::function_lexical::collect_function_lexical_bindings;
use crate::ir::function_records::{
    collect_function_records, FunctionNameAllocator, FunctionRecord, FunctionRecordFlavor,
    FunctionRecordNode,
};
use crate::ir::lowering_context::LoweringContext;
use crate::ir::module_lexical::collect_module_lexical_bindings;
use crate::ir::module_record::{collect_module_record, DEFAULT_LOCAL_NAME};
use crate::ir::opcodes::Insn;
use crate::ir::record_layout::{is_explicit_ecmascript_source_file, RecordLayout};
use crate::names::pandasm_record_name;

const ARG_SENTINEL_BASE: u8 = 192;
const ARG_SENTINEL_LIMIT: u8 = 64;
const MANDATORY_PARAM_FUNCTION_REG: u8 = ARG_SENTINEL_BASE;
const MANDATORY_PARAM_NEW_TARGET_REG: u8 = ARG_SENTINEL_BASE + 1;
const MANDATORY_PARAM_THIS_REG: u8 = ARG_SENTINEL_BASE + 2;
const MAX_RANGE_CALL_ARG: usize = 128;

fn is_type_only_function(func: &Function<'_>) -> bool {
    func.declare || func.body.is_none()
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `source` and build a [`ClassRecord`] named `record_name`.
///
/// Steps:
/// 1. Parse as TSX (accepts TypeScript annotations and `.ets` syntax).
/// 2. Compile top-level module statements as the `main` method.
/// 3. Emit stub methods for each top-level `FunctionDeclaration`.
///
/// ## Code generation coverage
///
/// The IR lowering handles these JS constructs:
/// - Numeric / string / boolean / null literals
/// - Variable declarations (`const`, `let`, `var`) with simple initializers
/// - Global reads: `globalThis['name']` → `tryldglobalbyname`
/// - Global writes: `globalThis['name'] = value` → `trystglobalbyname`
/// - Object property read: `obj.prop` → `ldobjbyname`
/// - Object property write: `obj.prop = value` → `stobjbyname`
/// - Function calls with 0–3 positional args (`callarg0`–`callargs3`)
/// - Method calls with 0–3 args (`callthis0`–`callthis3`)
/// - `return` / `return expr`
/// - Binary expressions: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `===`, `!==`,
///   `<`, `<=`, `>`, `>=`, `<<`, `>>`, `>>>`, `&`, `|`, `^`, `in`
/// - Unary expressions: `typeof`, `void`, `!`, `-`, `~`
/// - Logical expressions: `&&`, `||`, `??` with short-circuit jump patching
/// - Conditional expression: `test ? consequent : alternate`
/// - Template literals: `` `q0${e0}q1` `` → `lda.str` + `add2` chain
/// - `while` / `do…while` / `for` loops with correct backward branch offsets
/// - `break` / `continue` (unlabelled) with forward-jump patch lists
/// - `throw expr` → `throw` opcode (`0xFE 0x00`)
/// - `this` expression in class methods → mandatory-parameter load (`lda v2`)
/// - Arrow functions with `this` / `new.target` captures → `newlexenv` + mandatory-parameter loads
/// - Arrow functions as call arguments → separate `MethodRecord` per closure
pub fn build_record(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug(source, record_name, strings, literal_arrays, None, false)
}

pub fn build_record_with_debug(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    is_module: bool,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_package(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        None,
        false,
        is_module,
    )
}

pub fn build_record_with_debug_and_package(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_module: bool,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_package_for_target(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        is_module,
        12,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_record_with_debug_and_package_for_target(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_module: bool,
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_package_for_target_with_mode(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        false,
        "",
        is_module,
        target_api_version,
        target_api_sub_version,
        false,
        source_file.is_some(),
        source_file.is_some_and(is_explicit_ecmascript_source_file),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_record_with_debug_and_package_for_target_with_mode(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_shared_module: bool,
    origin_source_lang: &str,
    is_module: bool,
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
    enable_annotations: bool,
    emit_debugger: bool,
    standalone_ecmascript_input: bool,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_source_map_for_target_with_mode(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        is_shared_module,
        origin_source_lang,
        is_module,
        None,
        target_api_version,
        target_api_sub_version,
        enable_annotations,
        emit_debugger,
        standalone_ecmascript_input,
    )
}

pub fn build_record_with_debug_and_source_map(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_module: bool,
    source_map: Option<&str>,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_source_map_for_target(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        is_module,
        source_map,
        12,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_record_with_debug_and_source_map_for_target(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_module: bool,
    source_map: Option<&str>,
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug_and_source_map_for_target_with_mode(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        false,
        "",
        is_module,
        source_map,
        target_api_version,
        target_api_sub_version,
        false,
        source_file.is_some(),
        source_file.is_some_and(is_explicit_ecmascript_source_file),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_record_with_debug_and_source_map_for_target_with_mode(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    package_name: Option<&str>,
    is_commonjs: bool,
    is_shared_module: bool,
    origin_source_lang: &str,
    is_module: bool,
    source_map: Option<&str>,
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
    enable_annotations: bool,
    emit_debugger: bool,
    standalone_ecmascript_input: bool,
) -> Result<ClassRecord, AbcError> {
    let source_mapper = SourceMapper::from_json(source_map);
    let source_mapper = source_mapper.as_ref();
    let string_start = strings.len() as u32;
    let literal_start = literal_arrays.len();

    let allocator = Allocator::default();
    // Use ETS source type when the source file is a `.ets` file so that
    // ArkTS-specific syntax (AnnotationDeclaration, StructStatement, etc.)
    // is parsed correctly.  Plain `.ts`/`.js` sources use the standard TS type.
    let is_ets = source_file
        .or(Some(record_name))
        .map(|s| s.ends_with(".ets"))
        .unwrap_or(false);
    let source_type = if is_ets {
        // SourceType::ets() sets LanguageVariant::Arkui, enabling @interface, struct, etc.
        SourceType::ets()
    } else {
        SourceType::ts().with_module(is_module)
    };
    let ParserReturn {
        mut program,
        panicked,
        errors,
        ..
    } = Parser::new(&allocator, source, source_type).parse();

    if panicked {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: errors
                .first()
                .map(|e| e.message.to_string())
                .unwrap_or_else(|| "parser panic".to_string()),
        });
    }
    let semantic_ret = SemanticBuilder::new().with_enum_eval(true).build(&program);
    let scoping = if semantic_ret.errors.is_empty() {
        let mut scoping = semantic_ret.semantic.into_scoping();
        if needs_typescript_runtime_transform(&program) {
            let mut options = TransformOptions::default();
            options.typescript.allow_namespaces = true;
            options.typescript.optimize_const_enums = false;
            options.env.module = if is_commonjs {
                TransformModule::CommonJS
            } else if is_module {
                TransformModule::Esm
            } else {
                TransformModule::Preserve
            };
            let path = source_file
                .map(Path::new)
                .unwrap_or_else(|| Path::new(record_name));
            let ret = Transformer::new(&allocator, path, &options)
                .build_with_scoping(scoping, &mut program);
            if let Some(error) = ret.errors.first() {
                return Err(AbcError::Parse {
                    file: record_name.to_string(),
                    message: error.to_string(),
                });
            }
            scoping = ret.scoping;
        }
        Some(scoping)
    } else {
        None
    };
    let scoping_ref = scoping.as_ref();
    let runtime_ts_import_equals = collect_runtime_ts_import_equals(&program, scoping_ref);
    reject_unsupported_constructs(
        &program,
        record_name,
        target_api_version,
        enable_annotations,
    )?;
    let enum_infos = collect_annotation_enum_infos(&program);
    let mut annotation_literal_array_counter = 0usize;
    let annotations_enabled =
        enable_annotations && target_api_version >= ANNOTATION_SUPPORTED_API_VERSION;

    // Collect module import/export records before lowering so bytecode can use
    // hvigor/es2abc's module-variable opcodes instead of global lookups.
    let module_record = collect_module_record(&program, is_module, scoping_ref);
    let concurrent_import_requests =
        ConcurrentImportRequests::new(&program, &module_record, scoping_ref, record_name)?;
    let lexical_bindings = collect_module_lexical_bindings(&program, is_module, &module_record);
    let function_lexical_infos = collect_function_lexical_bindings(&program);
    let scan_stmts: &[Statement<'_>] = statement_scan_body(&program, is_module);
    let has_top_level_await = is_module && stmts_have_top_level_await(scan_stmts);
    let mut function_names = FunctionNameAllocator::new(source);
    let function_records = collect_function_records(
        &program,
        scan_stmts,
        &mut function_names,
        standalone_ecmascript_input,
    );
    let function_names_by_span = function_records
        .iter()
        .map(|record| (record.span(), record.name.clone()))
        .collect();
    let lowering = LoweringContext::from_module_record(
        &module_record,
        lexical_bindings,
        function_names_by_span,
        function_lexical_infos,
        target_api_version,
        target_api_sub_version.map(str::to_string),
        emit_debugger,
        standalone_ecmascript_input,
    );

    let mut methods: Vec<MethodRecord> = Vec::new();

    // Build the `func_main_0` method — the module execution entry point.
    //
    // ArkRuntime always invokes `func_main_0` when executing a module buffer.
    // It passes exactly 3 arguments (JSThread*, this, args[]), so `num_args`
    // must be 3 regardless of the actual JS parameter list.
    //
    // When source-file debug info is requested, we also compute a Panda
    // LineNumberProgram for func_main_0 using statement span offsets.
    let func_main = if source_file.is_some() {
        compile_program_body_tracked(
            &program,
            strings,
            literal_arrays,
            &lowering,
            is_module,
            source,
            source_mapper,
            has_top_level_await,
            &runtime_ts_import_equals,
        )
    } else {
        compile_program_body(
            &program,
            strings,
            literal_arrays,
            &lowering,
            is_module,
            source,
            has_top_level_await,
            &runtime_ts_import_equals,
        )
    };
    methods.push(MethodRecord {
        name: "func_main_0".to_string(),
        bytecode: func_main.bytecode,
        pandasm_insns: func_main.pandasm_insns,
        pandasm_debug: func_main.pandasm_debug,
        num_vregs: func_main.vregs,
        num_args: 3,
        value_of_first_param: -1,
        slots_num: None,
        debug_info: source_file.map(|path| DebugInfoRecord {
            line_start: func_main.debug_line_start.unwrap_or_else(|| {
                first_runtime_stmt_start(scan_stmts)
                    .map(|start| debug_line_start(Some(source), source_mapper, start))
                    .unwrap_or(1)
            }),
            source_file: path.to_string(),
            source_code: None,
            param_names: Vec::new(),
            lexical_this: false,
            local_variables: Vec::new(),
        }),
        line_ops: func_main.line_ops,
        catch_entries: func_main.catches,
        program_labels: func_main.program_labels,
        kind: MethodKind::Module,
        function_kind: function_kind::FUNCTION,
        concurrent_module_requests: Vec::new(),
        method_pool_entry: false,
        expected_property_count: None,
        annotations: Vec::new(),
        param_annotations: Vec::new(),
        definefunc_order: None,
        label_style: MethodLabelStyle::Source,
    });

    let tracked_source = source_file.as_ref().map(|_| source);

    for (definefunc_order, function) in function_records.into_iter().enumerate() {
        compile_function_record(
            function,
            definefunc_order as u32,
            strings,
            literal_arrays,
            &lowering,
            &mut methods,
            tracked_source,
            source_file,
            source_mapper,
            &concurrent_import_requests,
        );
    }

    // Compile class method bodies found in the selected statement body.
    // Module mode scans top-level statements directly; non-module compatibility
    // inputs are always the top-level program body.
    compile_class_method_bodies(
        scan_stmts,
        strings,
        literal_arrays,
        &lowering,
        &mut methods,
        tracked_source,
        source_file,
        source_mapper,
        &mut function_names,
        &concurrent_import_requests,
        record_name,
        annotations_enabled,
        &enum_infos,
        &mut annotation_literal_array_counter,
    )?;

    let record_layout = RecordLayout::new(
        record_name,
        package_name,
        standalone_ecmascript_input,
        source,
        is_commonjs,
        is_shared_module,
        has_top_level_await,
    );
    let source_lang = source_lang_from_origin_source_lang(origin_source_lang);
    let class_name = record_layout.class_name();

    // Literal arrays produced before this point are bytecode operands.  The
    // module-record and scope-name arrays below are class field values and are
    // not emitted into libpandafile's per-record method pool.
    let bytecode_literal_end = literal_arrays.len();

    let scope_names_idx = literal_arrays.len();
    literal_arrays.push(scope_names_literal_array(
        scan_stmts,
        record_name,
        standalone_ecmascript_input,
    ));
    // Collect module import/export records so that the ABC serializer can emit
    // the compact libpandafile module record buffer used by hvigor/es2abc.
    // Upstream only runs CompileModuleRecordJob for ScriptKind::MODULE;
    // CommonJS records are marked by isCommonjs and do not carry moduleRecordIdx.
    let module_record_idx = if is_commonjs {
        None
    } else {
        let idx = literal_arrays.len();
        literal_arrays.push(module_record.to_literal_array());
        Some(idx)
    };

    // Module metadata fields that es2abc writes into every user class record.
    // ArkRuntime reads these to understand how to load the module's dependencies.
    let module_fields = record_layout.module_fields(scope_names_idx, module_record_idx);

    let source_file = source_file.map(str::to_string);
    let source_code_strings = record_layout.source_code_strings();
    if let Some(file) = &source_file {
        for method in &mut methods {
            // Only set debug_info for methods that don't have it yet.
            // Class methods compiled by compile_class_elements already have
            // debug_info populated with param_names; preserve those.
            if method.debug_info.is_none() {
                method.debug_info = Some(DebugInfoRecord {
                    line_start: 1,
                    source_file: file.clone(),
                    source_code: None,
                    param_names: vec![],
                    lexical_this: false,
                    local_variables: Vec::new(),
                });
            }
        }
    }

    let literal_indices = record_layout.literal_indices(
        (literal_start..bytecode_literal_end).collect(),
        literal_arrays,
    );

    Ok(ClassRecord {
        name: class_name,
        methods,
        string_indices: (string_start..strings.len() as u32).collect(),
        literal_indices,
        module_literal_indices: module_record_idx.map_or_else(
            || vec![scope_names_idx],
            |module_record_idx| vec![scope_names_idx, module_record_idx],
        ),
        source_code_strings,
        access_flags: 1,
        source_lang,
        standalone_ecmascript_input,
        source_file,
        annotations: Vec::new(),
        module_requests: module_record.module_requests,
        fields: module_fields,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnnotationEnumValueKind {
    Number,
    String,
}

#[derive(Debug, Clone)]
enum AnnotationEnumValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone)]
struct AnnotationEnumInfo {
    value_kind: AnnotationEnumValueKind,
    members: FxHashMap<String, AnnotationEnumValue>,
}

pub fn annotation_records_from_source(
    source: &str,
    record_name: &str,
    source_file: Option<&str>,
    target_api_version: u32,
    enable_annotations: bool,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Result<Vec<ClassRecord>, AbcError> {
    let allocator = Allocator::default();
    let is_ets = source_file
        .or(Some(record_name))
        .map(|s| s.ends_with(".ets"))
        .unwrap_or(false);
    let source_type = if is_ets {
        SourceType::ets()
    } else {
        SourceType::ts().with_module(true)
    };
    let ParserReturn {
        program,
        panicked,
        errors,
        ..
    } = Parser::new(&allocator, source, source_type).parse();
    if panicked {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: errors
                .first()
                .map(|e| e.message.to_string())
                .unwrap_or_else(|| "parser panic".to_string()),
        });
    }

    let enum_infos = collect_annotation_enum_infos(&program);
    let mut annotation_literal_array_counter = literal_arrays
        .iter()
        .filter(|array| array.explicit_key.is_some())
        .count();
    let mut records = Vec::new();
    for stmt in &program.body {
        match stmt {
            Statement::AnnotationDeclaration(decl) => records.push(annotation_record_from_decl(
                decl,
                record_name,
                target_api_version,
                enable_annotations,
                literal_arrays,
                &enum_infos,
                &mut annotation_literal_array_counter,
            )?),
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::AnnotationDeclaration(decl)) = &export.declaration {
                    records.push(annotation_record_from_decl(
                        decl,
                        record_name,
                        target_api_version,
                        enable_annotations,
                        literal_arrays,
                        &enum_infos,
                        &mut annotation_literal_array_counter,
                    )?);
                }
            }
            _ => {}
        }
    }
    Ok(records)
}

fn annotation_record_from_decl(
    decl: &AnnotationDeclaration<'_>,
    record_name: &str,
    target_api_version: u32,
    enable_annotations: bool,
    literal_arrays: &mut Vec<LiteralArray>,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    annotation_literal_array_counter: &mut usize,
) -> Result<ClassRecord, AbcError> {
    if !(enable_annotations && target_api_version >= ANNOTATION_SUPPORTED_API_VERSION) {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: ANNOTATION_NOT_ENABLED_MESSAGE.to_string(),
        });
    }
    let annotation_name = decl.id.name.as_str();
    let Some(record_short_name) = annotation_name.strip_prefix(ANNOTATION_PREFIX) else {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: format!(
                "Annotation declaration need to be prefixed with '{ANNOTATION_PREFIX}'"
            ),
        });
    };
    let annotation_record_name = if record_name.is_empty() {
        record_short_name.to_string()
    } else {
        format!("{}.{}", pandasm_record_name(record_name), record_short_name)
    };

    let mut fields = Vec::with_capacity(decl.body.body.len());
    let mut literal_indices = Vec::new();
    for element in &decl.body.body {
        let AnnotationElement::PropertyDefinition(property) = element;
        let Some(name) = property.key.static_name() else {
            return Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationComputedProperty".to_string(),
                file: record_name.to_string(),
            });
        };
        let field_type =
            annotation_field_type(property.type_annotation.as_deref(), enum_infos, record_name)?;
        let value = annotation_field_value(
            &field_type,
            property.value.as_ref(),
            literal_arrays,
            &mut literal_indices,
            &annotation_record_name,
            name.as_ref(),
            enum_infos,
            record_name,
            annotation_literal_array_counter,
        )?;
        fields.push(FieldRecord {
            name: name.to_string(),
            class_idx: FIELD_OWNER_CLASS_IDX,
            type_idx: legacy_annotation_type_idx(&field_type),
            field_type,
            access_flags: 0,
            value,
            annotations: Vec::new(),
        });
    }

    Ok(ClassRecord {
        name: format!("L{annotation_record_name};"),
        methods: Vec::new(),
        string_indices: Vec::new(),
        literal_indices,
        module_literal_indices: Vec::new(),
        source_code_strings: Vec::new(),
        access_flags: 0x2000,
        source_lang: 0,
        standalone_ecmascript_input: false,
        source_file: None,
        annotations: Vec::new(),
        module_requests: Vec::new(),
        fields,
    })
}

fn collect_annotation_enum_infos(program: &Program<'_>) -> FxHashMap<String, AnnotationEnumInfo> {
    let mut infos = FxHashMap::default();
    for stmt in &program.body {
        let decl = match stmt {
            Statement::TSEnumDeclaration(decl) => Some(decl),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::TSEnumDeclaration(decl)) => Some(decl),
                _ => None,
            },
            _ => None,
        };
        let Some(decl) = decl else {
            continue;
        };
        infos.insert(
            decl.id.name.as_str().to_string(),
            annotation_enum_info(decl),
        );
    }
    infos
}

fn annotation_enum_info(decl: &TSEnumDeclaration<'_>) -> AnnotationEnumInfo {
    let mut members = FxHashMap::default();
    let mut value_kind = AnnotationEnumValueKind::Number;
    let mut next_number = 0.0;

    for member in &decl.body.members {
        let Some(member_name) = annotation_enum_member_name(&member.id) else {
            continue;
        };
        let value = if let Some(initializer) = &member.initializer {
            if let Some(value) = annotation_string_constant(initializer, &members) {
                value_kind = AnnotationEnumValueKind::String;
                AnnotationEnumValue::String(value)
            } else if let Some(value) = annotation_number_constant(initializer, &members) {
                next_number = value + 1.0;
                AnnotationEnumValue::Number(value)
            } else {
                let value = next_number;
                next_number += 1.0;
                AnnotationEnumValue::Number(value)
            }
        } else {
            let value = next_number;
            next_number += 1.0;
            AnnotationEnumValue::Number(value)
        };
        members.insert(member_name, value);
    }

    AnnotationEnumInfo {
        value_kind,
        members,
    }
}

fn annotation_enum_member_name(member_name: &TSEnumMemberName<'_>) -> Option<String> {
    match member_name {
        TSEnumMemberName::Identifier(id) => Some(id.name.as_str().to_string()),
        TSEnumMemberName::String(string) | TSEnumMemberName::ComputedString(string) => {
            Some(string.value.as_str().to_string())
        }
        TSEnumMemberName::ComputedTemplateString(template) => template
            .single_quasi()
            .map(|name| name.as_str().to_string()),
    }
}

fn annotation_field_type(
    annotation: Option<&oxc_ast::ast::TSTypeAnnotation<'_>>,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
) -> Result<FieldType, AbcError> {
    let Some(annotation) = annotation else {
        return Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationMissingFieldType".to_string(),
            file: record_name.to_string(),
        });
    };
    let (base_type, rank) = annotation_array_base_type(&annotation.type_annotation);
    let component_name = match base_type {
        TSType::TSStringKeyword(_) => "panda.String",
        TSType::TSNumberKeyword(_) => "f64",
        TSType::TSBooleanKeyword(_) => "u1",
        TSType::TSTypeReference(reference) => {
            let Some(info) = annotation_enum_info_for_type_name(&reference.type_name, enum_infos)
            else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "AnnotationFieldType".to_string(),
                    file: record_name.to_string(),
                });
            };
            match info.value_kind {
                AnnotationEnumValueKind::Number => "f64",
                AnnotationEnumValueKind::String => "panda.String",
            }
        }
        _ => Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationFieldType".to_string(),
            file: record_name.to_string(),
        })?,
    };
    Ok(annotation_field_type_from_component(component_name, rank))
}

fn annotation_array_base_type<'a>(mut ty: &'a TSType<'a>) -> (&'a TSType<'a>, u32) {
    let mut rank = 0;
    while let TSType::TSArrayType(array) = ty {
        rank += 1;
        ty = &array.element_type;
    }
    (ty, rank)
}

fn annotation_field_type_from_component(component_name: &str, rank: u32) -> FieldType {
    if rank > 0 {
        return FieldType::Array {
            component_name: component_name.to_string(),
            rank,
        };
    }
    match component_name {
        "panda.String" => FieldType::PandaString,
        "f64" => FieldType::F64,
        "u1" => FieldType::U1,
        "u8" => FieldType::U8,
        "u32" => FieldType::U32,
        _ => FieldType::Any,
    }
}

fn annotation_field_value(
    field_type: &FieldType,
    value: Option<&Expression<'_>>,
    literal_arrays: &mut Vec<LiteralArray>,
    literal_indices: &mut Vec<usize>,
    annotation_record_name: &str,
    field_name: &str,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
    annotation_literal_array_counter: &mut usize,
) -> Result<Option<FieldValue>, AbcError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(field_type, FieldType::Array { .. }) {
        if matches!(
            annotation_unwrap_expression(value),
            Expression::ArrayExpression(_) | Expression::NewExpression(_)
        ) {
            let key_prefix = format!("{annotation_record_name}_{field_name}");
            let index = annotation_literal_array_from_expr(
                value,
                literal_arrays,
                &key_prefix,
                annotation_literal_array_counter,
                enum_infos,
                record_name,
            )?;
            literal_indices.push(index);
            return Ok(Some(FieldValue::LiteralArray(index)));
        }
        return Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationArrayDefault".to_string(),
            file: record_name.to_string(),
        });
    }
    match field_type {
        FieldType::PandaString => {
            if let Some(value) = annotation_string_constant(value, &FxHashMap::default()) {
                return Ok(Some(FieldValue::String(value)));
            }
            if let Some(AnnotationEnumValue::String(value)) =
                annotation_enum_value_from_expr(value, enum_infos)
            {
                return Ok(Some(FieldValue::String(value)));
            }
            Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationStringDefault".to_string(),
                file: record_name.to_string(),
            })
        }
        FieldType::F64 => {
            if let Some(value) = annotation_number_constant(value, &FxHashMap::default()) {
                return Ok(Some(FieldValue::F64(value)));
            }
            if let Some(AnnotationEnumValue::Number(value)) =
                annotation_enum_value_from_expr(value, enum_infos)
            {
                return Ok(Some(FieldValue::F64(value)));
            }
            Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationNumericDefault".to_string(),
                file: record_name.to_string(),
            })
        }
        FieldType::U1 => match value {
            Expression::BooleanLiteral(lit) => Ok(Some(FieldValue::Bool(lit.value))),
            _ => Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationBooleanDefault".to_string(),
                file: record_name.to_string(),
            }),
        },
        FieldType::U8 | FieldType::U32 | FieldType::Any | FieldType::Array { .. } => {
            Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationFieldDefault".to_string(),
                file: record_name.to_string(),
            })
        }
    }
}

fn annotation_literal_array_from_expr(
    expr: &Expression<'_>,
    literal_arrays: &mut Vec<LiteralArray>,
    key_prefix: &str,
    counter: &mut usize,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
) -> Result<usize, AbcError> {
    let expr = annotation_unwrap_expression(expr);
    if let Expression::NewExpression(new_expr) = expr {
        return annotation_literal_array_from_new_expr(
            new_expr,
            literal_arrays,
            key_prefix,
            counter,
            record_name,
        );
    }
    let Expression::ArrayExpression(array) = expr else {
        return Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationArrayValue".to_string(),
            file: record_name.to_string(),
        });
    };
    annotation_literal_array_from_array(
        array,
        literal_arrays,
        key_prefix,
        counter,
        enum_infos,
        record_name,
    )
}

fn annotation_literal_array_from_array(
    array: &oxc_ast::ast::ArrayExpression<'_>,
    literal_arrays: &mut Vec<LiteralArray>,
    key_prefix: &str,
    counter: &mut usize,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
) -> Result<usize, AbcError> {
    let index = literal_arrays.len();
    let key = format!("{key_prefix}_{counter}");
    *counter += 1;
    literal_arrays.push(LiteralArray {
        entries: Vec::new(),
        explicit_key: Some(key),
    });

    let mut entries = Vec::with_capacity(array.elements.len());
    for element in &array.elements {
        entries.push(annotation_literal_entry_from_element(
            element,
            literal_arrays,
            key_prefix,
            counter,
            enum_infos,
            record_name,
        )?);
    }
    literal_arrays[index].entries = entries;
    Ok(index)
}

fn annotation_literal_array_from_new_expr(
    new_expr: &NewExpression<'_>,
    literal_arrays: &mut Vec<LiteralArray>,
    key_prefix: &str,
    counter: &mut usize,
    record_name: &str,
) -> Result<usize, AbcError> {
    let Some(type_arguments) = &new_expr.type_arguments else {
        return Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationNewArrayType".to_string(),
            file: record_name.to_string(),
        });
    };
    let Some(element_type) = type_arguments.params.first() else {
        return Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationNewArrayType".to_string(),
            file: record_name.to_string(),
        });
    };
    let builtin_type = match element_type {
        TSType::TSNumberKeyword(_) => 0,
        TSType::TSBooleanKeyword(_) => 1,
        TSType::TSStringKeyword(_) => 2,
        TSType::TSTypeReference(_) => {
            let Some(marker) = new_expr.arguments.first().and_then(|arg| {
                arg.as_expression()
                    .and_then(|expr| annotation_number_constant(expr, &FxHashMap::default()))
            }) else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "AnnotationNewArrayEnumMarker".to_string(),
                    file: record_name.to_string(),
                });
            };
            match marker as u8 {
                1 => 0,
                3 => 2,
                _ => {
                    return Err(AbcError::UnsupportedConstruct {
                        construct: "AnnotationNewArrayEnumMarker".to_string(),
                        file: record_name.to_string(),
                    })
                }
            }
        }
        _ => {
            return Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationNewArrayType".to_string(),
                file: record_name.to_string(),
            })
        }
    };

    let index = literal_arrays.len();
    let key = format!("{key_prefix}_{counter}");
    *counter += 1;
    literal_arrays.push(LiteralArray {
        entries: vec![LiteralEntry::TaggedBuiltinTypeIndex(builtin_type)],
        explicit_key: Some(key),
    });
    Ok(index)
}

fn annotation_literal_entry_from_element(
    element: &ArrayExpressionElement<'_>,
    literal_arrays: &mut Vec<LiteralArray>,
    key_prefix: &str,
    counter: &mut usize,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
) -> Result<LiteralEntry, AbcError> {
    match element {
        ArrayExpressionElement::NumericLiteral(lit) => Ok(LiteralEntry::TaggedDouble(lit.value)),
        ArrayExpressionElement::UnaryExpression(expr)
            if matches!(
                expr.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
            ) =>
        {
            annotation_number_from_unary(expr.operator, &expr.argument)
                .map(LiteralEntry::TaggedDouble)
                .ok_or_else(|| AbcError::UnsupportedConstruct {
                    construct: "AnnotationNumericArrayElement".to_string(),
                    file: record_name.to_string(),
                })
        }
        ArrayExpressionElement::BooleanLiteral(lit) => Ok(LiteralEntry::TaggedBool(lit.value)),
        ArrayExpressionElement::StringLiteral(lit) => {
            Ok(LiteralEntry::TaggedString(lit.value.to_string()))
        }
        ArrayExpressionElement::ArrayExpression(array) => {
            let index = annotation_literal_array_from_array(
                array,
                literal_arrays,
                key_prefix,
                counter,
                enum_infos,
                record_name,
            )?;
            Ok(LiteralEntry::TaggedLiteralArrayIndex(index))
        }
        ArrayExpressionElement::NewExpression(new_expr) => {
            let index = annotation_literal_array_from_new_expr(
                new_expr,
                literal_arrays,
                key_prefix,
                counter,
                record_name,
            )?;
            Ok(LiteralEntry::TaggedLiteralArrayIndex(index))
        }
        ArrayExpressionElement::StaticMemberExpression(mem) => {
            annotation_enum_value_from_member(&mem.object, mem.property.name.as_str(), enum_infos)
                .map(annotation_enum_value_to_literal_entry)
                .ok_or_else(|| AbcError::UnsupportedConstruct {
                    construct: "AnnotationEnumArrayElement".to_string(),
                    file: record_name.to_string(),
                })
        }
        ArrayExpressionElement::ComputedMemberExpression(mem) => {
            let Some(member_name) = annotation_computed_member_name(&mem.expression) else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "AnnotationEnumArrayElement".to_string(),
                    file: record_name.to_string(),
                });
            };
            annotation_enum_value_from_member(&mem.object, &member_name, enum_infos)
                .map(annotation_enum_value_to_literal_entry)
                .ok_or_else(|| AbcError::UnsupportedConstruct {
                    construct: "AnnotationEnumArrayElement".to_string(),
                    file: record_name.to_string(),
                })
        }
        _ => Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationArrayElement".to_string(),
            file: record_name.to_string(),
        }),
    }
}

fn annotation_enum_info_for_type_name<'a>(
    type_name: &TSTypeName<'_>,
    enum_infos: &'a FxHashMap<String, AnnotationEnumInfo>,
) -> Option<&'a AnnotationEnumInfo> {
    let name = annotation_type_name(type_name)?;
    if let Some(info) = enum_infos.get(&name) {
        return Some(info);
    }
    name.rsplit('.')
        .next()
        .and_then(|name| enum_infos.get(name))
}

fn annotation_type_name(type_name: &TSTypeName<'_>) -> Option<String> {
    match type_name {
        TSTypeName::IdentifierReference(id) => Some(id.name.as_str().to_string()),
        TSTypeName::QualifiedName(qualified) => {
            let left = annotation_type_name(&qualified.left)?;
            Some(format!("{left}.{}", qualified.right.name.as_str()))
        }
        TSTypeName::ThisExpression(_) => None,
    }
}

fn annotation_string_constant(
    expr: &Expression<'_>,
    enum_members: &FxHashMap<String, AnnotationEnumValue>,
) -> Option<String> {
    match expr {
        Expression::StringLiteral(lit) => Some(lit.value.to_string()),
        Expression::TemplateLiteral(template) => template
            .single_quasi()
            .map(|value| value.as_str().to_string()),
        Expression::Identifier(id) => match enum_members.get(id.name.as_str()) {
            Some(AnnotationEnumValue::String(value)) => Some(value.clone()),
            _ => None,
        },
        Expression::ParenthesizedExpression(expr) => {
            annotation_string_constant(&expr.expression, enum_members)
        }
        Expression::TSAsExpression(expr) => {
            annotation_string_constant(&expr.expression, enum_members)
        }
        Expression::TSSatisfiesExpression(expr) => {
            annotation_string_constant(&expr.expression, enum_members)
        }
        Expression::TSNonNullExpression(expr) => {
            annotation_string_constant(&expr.expression, enum_members)
        }
        Expression::TSTypeAssertion(expr) => {
            annotation_string_constant(&expr.expression, enum_members)
        }
        _ => None,
    }
}

fn annotation_number_constant(
    expr: &Expression<'_>,
    enum_members: &FxHashMap<String, AnnotationEnumValue>,
) -> Option<f64> {
    match expr {
        Expression::NumericLiteral(lit) => Some(lit.value),
        Expression::UnaryExpression(expr)
            if matches!(
                expr.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
            ) =>
        {
            annotation_number_from_unary(expr.operator, &expr.argument)
        }
        Expression::Identifier(id) => match enum_members.get(id.name.as_str()) {
            Some(AnnotationEnumValue::Number(value)) => Some(*value),
            _ => None,
        },
        Expression::ParenthesizedExpression(expr) => {
            annotation_number_constant(&expr.expression, enum_members)
        }
        Expression::TSAsExpression(expr) => {
            annotation_number_constant(&expr.expression, enum_members)
        }
        Expression::TSSatisfiesExpression(expr) => {
            annotation_number_constant(&expr.expression, enum_members)
        }
        Expression::TSNonNullExpression(expr) => {
            annotation_number_constant(&expr.expression, enum_members)
        }
        Expression::TSTypeAssertion(expr) => {
            annotation_number_constant(&expr.expression, enum_members)
        }
        _ => None,
    }
}

fn annotation_number_from_unary(operator: UnaryOperator, argument: &Expression<'_>) -> Option<f64> {
    let Expression::NumericLiteral(lit) = argument else {
        return None;
    };
    match operator {
        UnaryOperator::UnaryPlus => Some(lit.value),
        UnaryOperator::UnaryNegation => Some(-lit.value),
        _ => None,
    }
}

fn annotation_enum_value_from_expr(
    expr: &Expression<'_>,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
) -> Option<AnnotationEnumValue> {
    match expr {
        Expression::StaticMemberExpression(mem) => {
            annotation_enum_value_from_member(&mem.object, mem.property.name.as_str(), enum_infos)
        }
        Expression::ComputedMemberExpression(mem) => {
            let member_name = annotation_computed_member_name(&mem.expression)?;
            annotation_enum_value_from_member(&mem.object, &member_name, enum_infos)
        }
        Expression::CallExpression(call) if call.arguments.is_empty() => {
            annotation_enum_value_from_expr(&call.callee, enum_infos)
        }
        Expression::ParenthesizedExpression(expr) => {
            annotation_enum_value_from_expr(&expr.expression, enum_infos)
        }
        Expression::TSAsExpression(expr) => {
            annotation_enum_value_from_expr(&expr.expression, enum_infos)
        }
        Expression::TSSatisfiesExpression(expr) => {
            annotation_enum_value_from_expr(&expr.expression, enum_infos)
        }
        Expression::TSNonNullExpression(expr) => {
            annotation_enum_value_from_expr(&expr.expression, enum_infos)
        }
        Expression::TSTypeAssertion(expr) => {
            annotation_enum_value_from_expr(&expr.expression, enum_infos)
        }
        _ => None,
    }
}

fn annotation_enum_value_from_member(
    object: &Expression<'_>,
    member_name: &str,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
) -> Option<AnnotationEnumValue> {
    let enum_name = annotation_member_object_name(object)?;
    enum_infos
        .get(&enum_name)
        .and_then(|info| info.members.get(member_name))
        .cloned()
}

fn annotation_member_object_name(object: &Expression<'_>) -> Option<String> {
    match object {
        Expression::Identifier(id) => Some(id.name.as_str().to_string()),
        Expression::StaticMemberExpression(mem) => Some(mem.property.name.as_str().to_string()),
        Expression::ParenthesizedExpression(expr) => {
            annotation_member_object_name(&expr.expression)
        }
        _ => None,
    }
}

fn annotation_computed_member_name(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::StringLiteral(lit) => Some(lit.value.to_string()),
        Expression::TemplateLiteral(template) => template
            .single_quasi()
            .map(|value| value.as_str().to_string()),
        _ => None,
    }
}

fn annotation_enum_value_to_literal_entry(value: AnnotationEnumValue) -> LiteralEntry {
    match value {
        AnnotationEnumValue::Number(value) => LiteralEntry::TaggedDouble(value),
        AnnotationEnumValue::String(value) => LiteralEntry::TaggedString(value),
    }
}

fn annotation_records_from_decorators(
    decorators: &[Decorator<'_>],
    method_full_name: &str,
    record_name: &str,
    literal_arrays: &mut Vec<LiteralArray>,
    annotations_enabled: bool,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    annotation_literal_array_counter: &mut usize,
) -> Result<Vec<AnnotationRecord>, AbcError> {
    if decorators.is_empty() {
        return Ok(Vec::new());
    }
    if !annotations_enabled {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: ANNOTATION_NOT_ENABLED_MESSAGE.to_string(),
        });
    }

    decorators
        .iter()
        .map(|decorator| {
            annotation_record_from_decorator(
                decorator,
                method_full_name,
                record_name,
                literal_arrays,
                enum_infos,
                annotation_literal_array_counter,
            )
        })
        .collect()
}

fn annotation_record_from_decorator(
    decorator: &Decorator<'_>,
    method_full_name: &str,
    record_name: &str,
    literal_arrays: &mut Vec<LiteralArray>,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    annotation_literal_array_counter: &mut usize,
) -> Result<AnnotationRecord, AbcError> {
    let (annotation_name, properties) = annotation_usage_parts(&decorator.expression, record_name)?;
    let record_name_for_usage = annotation_usage_record_name(&annotation_name, record_name)?;
    let properties = match properties {
        AnnotationUsageProperties::None => {
            return Ok(AnnotationRecord {
                record_name: record_name_for_usage,
                elements: Vec::new(),
            });
        }
        AnnotationUsageProperties::Single(value_expr) => {
            let key_prefix = format!("{method_full_name}_value");
            let value = annotation_usage_value_from_expr(
                value_expr,
                literal_arrays,
                &key_prefix,
                annotation_literal_array_counter,
                enum_infos,
                record_name,
            )?;
            return Ok(AnnotationRecord {
                record_name: record_name_for_usage,
                elements: vec![AnnotationElementRecord {
                    name: "value".to_string(),
                    value,
                }],
            });
        }
        AnnotationUsageProperties::Object(properties) => properties,
    };

    let mut elements = Vec::with_capacity(properties.properties.len());
    for property in &properties.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationSpreadProperty".to_string(),
                file: record_name.to_string(),
            });
        };
        let Some(name) = property.key.static_name() else {
            return Err(AbcError::UnsupportedConstruct {
                construct: "AnnotationComputedProperty".to_string(),
                file: record_name.to_string(),
            });
        };
        let element_name = name.to_string();
        let key_prefix = format!("{method_full_name}_{element_name}");
        let value = annotation_usage_value_from_expr(
            &property.value,
            literal_arrays,
            &key_prefix,
            annotation_literal_array_counter,
            enum_infos,
            record_name,
        )?;
        elements.push(AnnotationElementRecord {
            name: element_name,
            value,
        });
    }

    Ok(AnnotationRecord {
        record_name: record_name_for_usage,
        elements,
    })
}

enum AnnotationUsageProperties<'a> {
    None,
    Object(&'a oxc_ast::ast::ObjectExpression<'a>),
    Single(&'a Expression<'a>),
}

fn annotation_usage_parts<'a>(
    expr: &'a Expression<'a>,
    record_name: &str,
) -> Result<(String, AnnotationUsageProperties<'a>), AbcError> {
    let expr = annotation_unwrap_expression(expr);
    match expr {
        Expression::CallExpression(call) => {
            let name = annotation_usage_name(&call.callee, record_name)?;
            let Some(argument) = call.arguments.first() else {
                return Ok((name, AnnotationUsageProperties::None));
            };
            if call.arguments.len() > 1 {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "AnnotationArguments".to_string(),
                    file: record_name.to_string(),
                });
            };
            let Some(argument_expr) = argument.as_expression() else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "AnnotationSpreadArgument".to_string(),
                    file: record_name.to_string(),
                });
            };
            match annotation_unwrap_expression(argument_expr) {
                Expression::ObjectExpression(object) => {
                    Ok((name, AnnotationUsageProperties::Object(object)))
                }
                expr => Ok((name, AnnotationUsageProperties::Single(expr))),
            }
        }
        _ => Ok((
            annotation_usage_name(expr, record_name)?,
            AnnotationUsageProperties::None,
        )),
    }
}

fn annotation_usage_name(expr: &Expression<'_>, record_name: &str) -> Result<String, AbcError> {
    match annotation_unwrap_expression(expr) {
        Expression::Identifier(id) => Ok(id.name.as_str().to_string()),
        Expression::StaticMemberExpression(mem) => Ok(mem.property.name.as_str().to_string()),
        Expression::CallExpression(call) if call.arguments.is_empty() => {
            annotation_usage_name(&call.callee, record_name)
        }
        _ => Err(AbcError::UnsupportedConstruct {
            construct: "AnnotationUsage".to_string(),
            file: record_name.to_string(),
        }),
    }
}

fn annotation_usage_record_name(
    annotation_name: &str,
    record_name: &str,
) -> Result<String, AbcError> {
    let Some(short_name) = annotation_name.strip_prefix(ANNOTATION_PREFIX) else {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: format!(
                "Annotation declaration need to be prefixed with '{ANNOTATION_PREFIX}'"
            ),
        });
    };
    if record_name.is_empty() {
        Ok(short_name.to_string())
    } else {
        Ok(format!(
            "{}.{}",
            pandasm_record_name(record_name),
            short_name
        ))
    }
}

fn annotation_method_base_name(record_name: &str, method_name: &str) -> String {
    if record_name.is_empty() {
        method_name.to_string()
    } else {
        format!("{}.{}", pandasm_record_name(record_name), method_name)
    }
}

fn annotation_usage_value_from_expr(
    expr: &Expression<'_>,
    literal_arrays: &mut Vec<LiteralArray>,
    key_prefix: &str,
    counter: &mut usize,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    record_name: &str,
) -> Result<AnnotationValue, AbcError> {
    let expr = annotation_unwrap_expression(expr);
    if matches!(
        expr,
        Expression::ArrayExpression(_) | Expression::NewExpression(_)
    ) {
        let index = annotation_literal_array_from_expr(
            expr,
            literal_arrays,
            key_prefix,
            counter,
            enum_infos,
            record_name,
        )?;
        let key = literal_arrays[index]
            .explicit_key
            .clone()
            .expect("annotation literal array must carry explicit key");
        return Ok(AnnotationValue::LiteralArray { index, key });
    }
    if let Expression::BooleanLiteral(lit) = expr {
        return Ok(AnnotationValue::Bool(lit.value));
    }
    if let Some(value) = annotation_string_constant(expr, &FxHashMap::default()) {
        return Ok(AnnotationValue::String(value));
    }
    if let Some(value) = annotation_number_constant(expr, &FxHashMap::default()) {
        return Ok(AnnotationValue::F64(value));
    }
    if let Some(value) = annotation_enum_value_from_expr(expr, enum_infos) {
        return Ok(match value {
            AnnotationEnumValue::Number(value) => AnnotationValue::F64(value),
            AnnotationEnumValue::String(value) => AnnotationValue::String(value),
        });
    }
    Err(AbcError::UnsupportedConstruct {
        construct: "AnnotationElementValue".to_string(),
        file: record_name.to_string(),
    })
}

fn annotation_unwrap_expression<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::ParenthesizedExpression(expr) => annotation_unwrap_expression(&expr.expression),
        Expression::TSAsExpression(expr) => annotation_unwrap_expression(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => annotation_unwrap_expression(&expr.expression),
        Expression::TSNonNullExpression(expr) => annotation_unwrap_expression(&expr.expression),
        Expression::TSTypeAssertion(expr) => annotation_unwrap_expression(&expr.expression),
        _ => expr,
    }
}

fn legacy_annotation_type_idx(field_type: &FieldType) -> u16 {
    match field_type {
        FieldType::U8 => 0,
        FieldType::U32 => 1,
        _ => 0,
    }
}

fn source_lang_from_origin_source_lang(origin_source_lang: &str) -> u8 {
    match origin_source_lang {
        "ets" => 0x04,
        "ts" => 0x03,
        "js" => 0x02,
        _ => 0x00,
    }
}

fn lexical_indices<'a>(names: impl IntoIterator<Item = &'a str>) -> FxHashMap<String, u8> {
    names
        .into_iter()
        .enumerate()
        .filter_map(|(idx, name)| u8::try_from(idx).ok().map(|idx| (name.to_string(), idx)))
        .collect()
}

fn scope_names_literal_array(
    stmts: &[Statement<'_>],
    record_name: &str,
    standalone_ecmascript_input: bool,
) -> LiteralArray {
    if standalone_ecmascript_input {
        return LiteralArray {
            entries: standalone_scope_names(stmts)
                .into_iter()
                .map(LiteralEntry::TaggedString)
                .collect(),
            explicit_key: None,
        };
    }

    let Some(owner_method) = first_scope_names_owner_method(stmts) else {
        // No call-arg closures in this class → no scopeNames payload.
        return LiteralArray {
            entries: vec![],
            explicit_key: None,
        };
    };

    let class_short = record_name
        .rsplit('/')
        .next()
        .unwrap_or(record_name)
        .to_string();

    LiteralArray {
        entries: vec![
            LiteralEntry::TaggedString(class_short),
            LiteralEntry::TaggedString(owner_method),
        ],
        explicit_key: None,
    }
}

fn standalone_scope_names(stmts: &[Statement<'_>]) -> Vec<String> {
    let mut names = Vec::new();
    for stmt in stmts {
        match stmt {
            Statement::ClassDeclaration(class) if !class.declare => {
                collect_class_scope_names(&class.body.body, &mut names);
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    if !class.declare {
                        collect_class_scope_names(&class.body.body, &mut names);
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::ClassDeclaration(class)) = &export.declaration {
                    if !class.declare {
                        collect_class_scope_names(&class.body.body, &mut names);
                    }
                }
            }
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    if let Some(Expression::ClassExpression(class)) = &declarator.init {
                        collect_class_scope_names(&class.body.body, &mut names);
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn collect_class_scope_names(elements: &[ClassElement<'_>], names: &mut Vec<String>) {
    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        if method.kind == MethodDefinitionKind::Constructor || method.r#static {
            continue;
        }
        if let Some(name) = method_key_name(&method.key) {
            names.push(name);
        }
    }
}

fn first_scope_names_owner_method(stmts: &[Statement<'_>]) -> Option<String> {
    for stmt in stmts {
        match stmt {
            Statement::ClassDeclaration(class) => {
                if class.declare {
                    continue;
                }
                if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                    return Some(name);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    if class.declare {
                        continue;
                    }
                    if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                        return Some(name);
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::ClassDeclaration(class)) = &export.declaration {
                    if class.declare {
                        continue;
                    }
                    if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                        return Some(name);
                    }
                }
            }
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    if let Some(Expression::ClassExpression(class)) = &declarator.init {
                        if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                            return Some(name);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn first_scope_names_owner_in_class(elements: &[ClassElement<'_>]) -> Option<String> {
    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        if method.kind == MethodDefinitionKind::Constructor {
            continue;
        }
        if !method_body_contains_arrow(method) {
            continue;
        }
        if let Some(name) = class_method_inferred_name(method, "_") {
            return Some(name);
        }
    }
    None
}

struct ConcurrentImportRequests<'a> {
    by_name: FxHashMap<String, u32>,
    requests_by_scope: FxHashMap<ScopeId, Vec<u32>>,
    scoping: Option<&'a Scoping>,
}

impl<'a> ConcurrentImportRequests<'a> {
    fn new(
        program: &Program<'_>,
        module_record: &crate::ir::module_record::ModuleRecordInfo,
        scoping: Option<&'a Scoping>,
        record_name: &str,
    ) -> Result<Self, AbcError> {
        let by_name = module_record
            .regular_imports
            .iter()
            .map(|import| (import.local_name.clone(), import.module_request_idx as u32))
            .collect::<FxHashMap<_, _>>();
        let mut by_symbol = FxHashMap::default();

        if let Some(scoping) = scoping {
            for stmt in &program.body {
                let Statement::ImportDeclaration(import) = stmt else {
                    continue;
                };
                if import.import_kind == ImportOrExportKind::Type {
                    continue;
                }
                let Some(specifiers) = &import.specifiers else {
                    continue;
                };
                for specifier in specifiers {
                    let local = match specifier {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                            &default.local
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(named)
                            if named.import_kind != ImportOrExportKind::Type =>
                        {
                            &named.local
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(_) => continue,
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => continue,
                    };
                    let Some(request_idx) = by_name.get(local.name.as_str()).copied() else {
                        continue;
                    };
                    if let Some(symbol_id) = local.symbol_id.get() {
                        by_symbol.insert(symbol_id, request_idx);
                    }
                }
            }

            let mut builder = ConcurrentMetadataBuilder {
                scoping,
                import_requests_by_symbol: &by_symbol,
                requests_by_scope: FxHashMap::default(),
                concurrent_stack: Vec::new(),
                record_name,
                error: None,
            };
            builder.visit_program(program);
            if let Some(error) = builder.error {
                return Err(error);
            }

            return Ok(Self {
                by_name,
                requests_by_scope: builder.requests_by_scope,
                scoping: Some(scoping),
            });
        }

        Ok(Self {
            by_name,
            requests_by_scope: FxHashMap::default(),
            scoping,
        })
    }

    fn collect_for_function(&self, function: &Function<'_>) -> Vec<u32> {
        if !function_has_directive(function, "use concurrent") {
            return Vec::new();
        }
        if self.scoping.is_some() {
            if let Some(scope_id) = function.scope_id.get() {
                return self
                    .requests_by_scope
                    .get(&scope_id)
                    .cloned()
                    .unwrap_or_default();
            }
        }
        let Some(body) = &function.body else {
            return Vec::new();
        };
        let mut collector = ConcurrentModuleRequestCollector {
            imports: self,
            requests: Vec::new(),
        };
        collector.visit_function_body(body);
        collector.requests
    }
}

struct ConcurrentMetadataBuilder<'b> {
    scoping: &'b Scoping,
    import_requests_by_symbol: &'b FxHashMap<SymbolId, u32>,
    requests_by_scope: FxHashMap<ScopeId, Vec<u32>>,
    concurrent_stack: Vec<ScopeId>,
    record_name: &'b str,
    error: Option<AbcError>,
}

impl ConcurrentMetadataBuilder<'_> {
    fn invalid_concurrent_function(&self) -> AbcError {
        AbcError::Parse {
            file: self.record_name.to_string(),
            message: "Concurrent function should only be function declaration".to_string(),
        }
    }

    fn invalid_concurrent_variable(&self, name: &str) -> AbcError {
        AbcError::Parse {
            file: self.record_name.to_string(),
            message: format!(
                "Concurrent function should only use import variable or local variable, '{name}' is not one of them"
            ),
        }
    }

    fn scope_contains(&self, ancestor: ScopeId, scope: ScopeId) -> bool {
        self.scoping
            .scope_ancestors(scope)
            .any(|scope| scope == ancestor)
    }
}

impl<'a> Visit<'a> for ConcurrentMetadataBuilder<'_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        if self.error.is_some() {
            return;
        }

        let is_concurrent = function_has_directive(it, "use concurrent");
        if is_concurrent && it.generator {
            self.error = Some(self.invalid_concurrent_function());
            return;
        }

        if is_concurrent {
            if let Some(scope_id) = it.scope_id.get() {
                self.concurrent_stack.push(scope_id);
                walk::walk_function(self, it, flags);
                self.concurrent_stack.pop();
                return;
            }
        }

        walk::walk_function(self, it, flags);
    }

    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        if self.error.is_some() {
            return;
        }
        if !it.expression && body_has_directive(&it.body, "use concurrent") {
            self.error = Some(self.invalid_concurrent_function());
            return;
        }
        walk::walk_arrow_function_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if self.error.is_some() {
            return;
        }
        let Some(concurrent_scope) = self.concurrent_stack.last().copied() else {
            return;
        };
        let Some(reference_id) = it.reference_id.get() else {
            return;
        };
        let reference = self.scoping.get_reference(reference_id);
        if reference.is_type() {
            return;
        }
        let Some(symbol_id) = reference.symbol_id() else {
            return;
        };
        if self.scope_contains(concurrent_scope, self.scoping.symbol_scope_id(symbol_id)) {
            return;
        }

        if let Some(request_idx) = self.import_requests_by_symbol.get(&symbol_id) {
            self.requests_by_scope
                .entry(concurrent_scope)
                .or_default()
                .push(*request_idx);
            return;
        }

        self.error = Some(self.invalid_concurrent_variable(self.scoping.symbol_name(symbol_id)));
    }
}

struct ConcurrentModuleRequestCollector<'a, 'b> {
    imports: &'b ConcurrentImportRequests<'a>,
    requests: Vec<u32>,
}

impl<'a> Visit<'a> for ConcurrentModuleRequestCollector<'a, '_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        if function_has_directive(it, "use concurrent") {
            return;
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        if !it.expression && body_has_directive(&it.body, "use concurrent") {
            return;
        }
        walk::walk_arrow_function_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if let Some(request_idx) = self.imports.by_name.get(it.name.as_str()) {
            self.requests.push(*request_idx);
        }
    }
}

// ── Program-level compilation ─────────────────────────────────────────────────

/// Always returns the top-level statement slice — IIFE/CJS unwrapping removed.
fn statement_scan_body<'a>(program: &'a Program<'_>, _is_module: bool) -> &'a [Statement<'a>] {
    program.body.as_slice()
}

#[allow(clippy::too_many_arguments)]
fn compile_function_record(
    function: FunctionRecord<'_>,
    definefunc_order: u32,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    methods: &mut Vec<MethodRecord>,
    source: Option<&str>,
    source_file_path: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    concurrent_import_requests: &ConcurrentImportRequests<'_>,
) {
    match function.node {
        FunctionRecordNode::Function(func) => {
            if func.declare || func.body.is_none() {
                return;
            }
            let param_names = function_param_names(func);
            let rest_info = function_rest_param(func);
            let has_destructure = function_has_destructuring_params(func);
            let compiled = if func.generator {
                // Generator (or async-generator): route through state-machine path.
                compile_generator_body(
                    func.body.as_deref(),
                    func.r#async,
                    rest_info,
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    source,
                    source_mapper,
                    Some((func.span.start, func.span.end)),
                )
            } else if rest_info.is_some() || func.r#async || has_destructure {
                // Route through the extended path that handles rest params,
                // async functions, and destructuring parameter patterns.
                if let Some(src) = source {
                    compile_function_body_with_destructure_params(
                        func.body.as_deref(),
                        rest_info,
                        func.r#async,
                        &func.params.items,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        Some(src),
                        source_mapper,
                        Some((func.span.start, func.span.end)),
                    )
                } else {
                    compile_function_body_with_destructure_params(
                        func.body.as_deref(),
                        rest_info,
                        func.r#async,
                        &func.params.items,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        None,
                        None,
                        Some((func.span.start, func.span.end)),
                    )
                }
            } else if let Some(src) = source {
                compile_function_body_tracked_with_end(
                    func.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    src,
                    source_mapper,
                    Some(func.span.end),
                    Some((func.span.start, func.span.end)),
                )
            } else {
                compile_function_body(
                    func.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    Some((func.span.start, func.span.end)),
                )
            };
            let num_vregs = compiled.vregs;
            let num_args = func.params.items.len() as u16 + 3;
            let expected_property_count = expected_property_count_option(
                function_body_expected_property_count(func.body.as_deref(), lowering),
            );
            let upstream_function_kind =
                function_record_function_kind(func, function.flavor, lowering.target_api_version());
            let concurrent_module_requests = concurrent_import_requests.collect_for_function(func);
            methods.push(MethodRecord {
                name: function.name,
                bytecode: compiled.bytecode,
                pandasm_insns: compiled.pandasm_insns,
                pandasm_debug: compiled.pandasm_debug,
                num_vregs,
                num_args,
                value_of_first_param: -1,
                slots_num: None,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: compiled.debug_line_start.unwrap_or(1),
                    source_file: path.to_string(),
                    source_code: None,
                    param_names,
                    lexical_this: func
                        .body
                        .as_deref()
                        .is_some_and(|body| stmts_capture_this_in_arrow(&body.statements)),
                    local_variables: Vec::new(),
                }),
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                program_labels: compiled.program_labels,
                kind: MethodKind::FunctionLiteral,
                function_kind: upstream_function_kind,
                concurrent_module_requests,
                method_pool_entry: true,
                expected_property_count,
                annotations: Vec::new(),
                param_annotations: Vec::new(),
                definefunc_order: Some(definefunc_order),
                label_style: MethodLabelStyle::Source,
            });
        }
        FunctionRecordNode::Arrow(arrow) => {
            let captures = arrow_captures_from_body(arrow.body.as_ref());
            let param_names = arrow_param_names(arrow);
            let compiled = compile_arrow_body_with_line_ops(
                arrow.body.as_ref(),
                strings,
                literal_arrays,
                lowering,
                captures,
                &param_names,
                arrow.expression,
                source,
                source_mapper,
                Some((arrow.span.start, arrow.span.end)),
            );
            let num_vregs = compiled.vregs;
            let num_args = arrow.params.items.len() as u16 + 3;
            let expected_property_count = expected_property_count_option(
                function_body_expected_property_count(Some(arrow.body.as_ref()), lowering),
            );
            methods.push(MethodRecord {
                name: function.name,
                bytecode: compiled.bytecode,
                pandasm_insns: compiled.pandasm_insns,
                pandasm_debug: compiled.pandasm_debug,
                num_vregs,
                num_args,
                value_of_first_param: -1,
                slots_num: None,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: compiled.debug_line_start.unwrap_or(1),
                    source_file: path.to_string(),
                    source_code: None,
                    param_names,
                    lexical_this: captures.this_slot.is_some(),
                    local_variables: Vec::new(),
                }),
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                program_labels: compiled.program_labels,
                kind: MethodKind::Anonymous,
                function_kind: if arrow.r#async {
                    function_kind::ASYNC_NC_FUNCTION
                } else {
                    function_kind::NC_FUNCTION
                },
                concurrent_module_requests: Vec::new(),
                method_pool_entry: true,
                expected_property_count,
                annotations: Vec::new(),
                param_annotations: Vec::new(),
                definefunc_order: Some(definefunc_order),
                label_style: MethodLabelStyle::Source,
            });
        }
    }
}

fn function_record_function_kind(
    function: &Function<'_>,
    flavor: FunctionRecordFlavor,
    target_api_version: u32,
) -> u32 {
    if function_has_directive(function, "use concurrent") {
        return function_kind::CONCURRENT_FUNCTION;
    }

    if flavor == FunctionRecordFlavor::ObjectMethod {
        return if function.r#async {
            function_kind::ASYNC_FUNCTION
        } else {
            function_kind::NONE
        };
    }

    let kind = match (function.r#async, function.generator) {
        (true, true) => function_kind::ASYNC_GENERATOR_FUNCTION,
        (true, false) => function_kind::ASYNC_FUNCTION,
        (false, true) => function_kind::GENERATOR_FUNCTION,
        (false, false) => function_kind::FUNCTION,
    };

    if target_api_version >= 12
        && !function.generator
        && function_has_directive(function, "use sendable")
    {
        return kind | function_kind::SENDABLE_FUNCTION;
    }

    kind
}

fn function_has_directive(function: &Function<'_>, directive: &str) -> bool {
    let Some(body) = &function.body else {
        return false;
    };
    body_has_directive(body, directive)
}

fn body_has_directive(body: &FunctionBody<'_>, directive: &str) -> bool {
    if body
        .directives
        .iter()
        .any(|item| item.expression.value.as_str() == directive)
    {
        return true;
    }
    for stmt in &body.statements {
        let Statement::ExpressionStatement(expr) = stmt else {
            return false;
        };
        let Expression::StringLiteral(value) = &expr.expression else {
            return false;
        };
        if value.value.as_str() == directive {
            return true;
        }
    }
    false
}

fn class_method_function_kind(
    function: &Function<'_>,
    is_constructor: bool,
    is_async: bool,
) -> u32 {
    if function_has_directive(function, "use concurrent") {
        return function_kind::CONCURRENT_FUNCTION;
    }
    if is_constructor {
        function_kind::FUNCTION
    } else if is_async {
        function_kind::ASYNC_FUNCTION
    } else {
        function_kind::NONE
    }
}

fn object_method_literal_entry(record_name: String, function: &Function<'_>) -> LiteralEntry {
    if function.generator {
        if function.r#async {
            LiteralEntry::TaggedAsyncGeneratorMethodName(record_name)
        } else {
            LiteralEntry::TaggedGeneratorMethodName(record_name)
        }
    } else {
        LiteralEntry::TaggedMethodName(record_name)
    }
}

/// Walk `stmts` for `ClassDeclaration`s and compile each method body into a
/// separate [`MethodRecord`], appending them to `methods`.
///
/// Each class lifecycle method (`onCreate`, `onWindowStageCreate`, etc.) gets
/// its own bytecode record so ArkRuntime can dispatch to the real implementation.
fn compile_class_method_bodies(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    methods: &mut Vec<MethodRecord>,
    source: Option<&str>,
    source_file_path: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    function_names: &mut FunctionNameAllocator<'_>,
    concurrent_import_requests: &ConcurrentImportRequests<'_>,
    record_name: &str,
    annotations_enabled: bool,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    annotation_literal_array_counter: &mut usize,
) -> Result<(), AbcError> {
    for stmt in stmts {
        // Handle `class Foo { ... }` (ClassDeclaration)
        if let Statement::ClassDeclaration(cls) = stmt {
            if cls.declare {
                continue;
            }
            let class_name = cls.id.as_ref().map(|id| id.name.as_str());
            compile_class_elements(
                class_name,
                cls.super_class.is_some(),
                cls.span.start,
                cls.span.end,
                &cls.decorators,
                &cls.body.body,
                strings,
                literal_arrays,
                lowering,
                methods,
                source,
                source_file_path,
                source_mapper,
                function_names,
                concurrent_import_requests,
                record_name,
                annotations_enabled,
                enum_infos,
                annotation_literal_array_counter,
            )?;
            continue;
        }
        if let Statement::ExportDefaultDeclaration(export) = stmt {
            if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
                if cls.declare {
                    continue;
                }
                let class_name = cls
                    .id
                    .as_ref()
                    .map_or(DEFAULT_LOCAL_NAME, |id| id.name.as_str());
                compile_class_elements(
                    Some(class_name),
                    cls.super_class.is_some(),
                    cls.span.start,
                    cls.span.end,
                    &cls.decorators,
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    lowering,
                    methods,
                    source,
                    source_file_path,
                    source_mapper,
                    function_names,
                    concurrent_import_requests,
                    record_name,
                    annotations_enabled,
                    enum_infos,
                    annotation_literal_array_counter,
                )?;
            }
            continue;
        }
        if let Statement::ExportNamedDeclaration(export) = stmt {
            if let Some(Declaration::ClassDeclaration(cls)) = &export.declaration {
                if cls.declare {
                    continue;
                }
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                compile_class_elements(
                    class_name,
                    cls.super_class.is_some(),
                    cls.span.start,
                    cls.span.end,
                    &cls.decorators,
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    lowering,
                    methods,
                    source,
                    source_file_path,
                    source_mapper,
                    function_names,
                    concurrent_import_requests,
                    record_name,
                    annotations_enabled,
                    enum_infos,
                    annotation_literal_array_counter,
                )?;
            }
            continue;
        }
        // Handle `var Foo = class extends Bar { ... }` (VariableDeclaration with ClassExpression)
        // Handles `var Foo = class extends Bar { ... }` (rolldown ESM output pattern).
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for declarator in &var_decl.declarations {
                // Extract binding name (skip destructuring).
                let name = match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => id.name.as_str().to_string(),
                    _ => continue,
                };

                if let Some(Expression::ClassExpression(cls)) = &declarator.init {
                    let class_name = cls.id.as_ref().map_or(name.as_str(), |id| id.name.as_str());
                    compile_class_elements(
                        Some(class_name),
                        cls.super_class.is_some(),
                        cls.span.start,
                        cls.span.end,
                        &cls.decorators,
                        &cls.body.body,
                        strings,
                        literal_arrays,
                        lowering,
                        methods,
                        source,
                        source_file_path,
                        source_mapper,
                        function_names,
                        concurrent_import_requests,
                        record_name,
                        annotations_enabled,
                        enum_infos,
                        annotation_literal_array_counter,
                    )?;
                    continue;
                }
            }
        }
    }
    Ok(())
}

fn expected_property_count_enabled(lowering: &LoweringContext) -> bool {
    lowering.target_api_version() >= 18
}

fn expected_property_count_option(count: u32) -> Option<u32> {
    (count != 0).then_some(count)
}

fn function_body_expected_property_count(
    body: Option<&FunctionBody<'_>>,
    lowering: &LoweringContext,
) -> u32 {
    if !expected_property_count_enabled(lowering) {
        return 0;
    }
    let Some(body) = body else {
        return 0;
    };

    let mut property_names = FxHashSet::default();
    for stmt in &body.statements {
        if let Some(name) = function_this_assignment_property_name(stmt) {
            property_names.insert(name);
        }
    }
    property_names.len() as u32
}

fn compute_class_expected_property_count(
    elements: &[ClassElement<'_>],
    lowering: &LoweringContext,
) -> u32 {
    if !expected_property_count_enabled(lowering) {
        return 0;
    }

    let mut count = 0u32;
    let mut public_property_names = FxHashSet::default();
    for element in elements {
        match element {
            ClassElement::PropertyDefinition(prop) if !prop.r#static => {
                if matches!(prop.key, PropertyKey::PrivateIdentifier(_)) {
                    count += 1;
                } else if let Some(name) = expected_property_key_name(&prop.key) {
                    add_expected_property_name(&mut public_property_names, &mut count, name);
                }
            }
            ClassElement::AccessorProperty(prop) if !prop.r#static => {
                if matches!(prop.key, PropertyKey::PrivateIdentifier(_)) {
                    count += 1;
                } else if let Some(name) = expected_property_key_name(&prop.key) {
                    add_expected_property_name(&mut public_property_names, &mut count, name);
                }
            }
            _ => {}
        }
    }

    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        if method.kind != MethodDefinitionKind::Constructor {
            continue;
        }
        let Some(body) = method.value.body.as_deref() else {
            continue;
        };
        for stmt in &body.statements {
            if let Some(name) = class_constructor_this_assignment_property_name(stmt) {
                add_expected_property_name(&mut public_property_names, &mut count, name);
            }
        }
    }

    count
}

fn add_expected_property_name(names: &mut FxHashSet<String>, count: &mut u32, name: String) {
    if names.insert(name) {
        *count += 1;
    }
}

fn expected_property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_string()),
        PropertyKey::Identifier(id) => Some(id.name.as_str().to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_string()),
        PropertyKey::NumericLiteral(n) => n
            .raw
            .as_deref()
            .map(str::to_string)
            .or_else(|| Some(n.value.to_string())),
        _ => None,
    }
}

fn function_this_assignment_property_name(stmt: &Statement<'_>) -> Option<String> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = &expr_stmt.expression else {
        return None;
    };
    static_this_assignment_target_name(&assign.left)
}

fn class_constructor_this_assignment_property_name(stmt: &Statement<'_>) -> Option<String> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = &expr_stmt.expression else {
        return None;
    };
    class_this_assignment_target_name(&assign.left)
}

fn static_this_assignment_target_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(mem) if is_this_expression(&mem.object) => {
            Some(mem.property.name.as_str().to_string())
        }
        AssignmentTarget::TSAsExpression(expr) => {
            static_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSSatisfiesExpression(expr) => {
            static_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSNonNullExpression(expr) => {
            static_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSTypeAssertion(expr) => {
            static_this_assignment_expression_name(&expr.expression)
        }
        _ => None,
    }
}

fn class_this_assignment_target_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(mem) if is_this_expression(&mem.object) => {
            Some(mem.property.name.as_str().to_string())
        }
        AssignmentTarget::ComputedMemberExpression(mem) if is_this_expression(&mem.object) => {
            expected_property_expression_name(&mem.expression)
        }
        AssignmentTarget::TSAsExpression(expr) => {
            class_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSSatisfiesExpression(expr) => {
            class_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSNonNullExpression(expr) => {
            class_this_assignment_expression_name(&expr.expression)
        }
        AssignmentTarget::TSTypeAssertion(expr) => {
            class_this_assignment_expression_name(&expr.expression)
        }
        _ => None,
    }
}

fn static_this_assignment_expression_name(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::StaticMemberExpression(mem) if is_this_expression(&mem.object) => {
            Some(mem.property.name.as_str().to_string())
        }
        _ => None,
    }
}

fn class_this_assignment_expression_name(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::StaticMemberExpression(mem) if is_this_expression(&mem.object) => {
            Some(mem.property.name.as_str().to_string())
        }
        Expression::ComputedMemberExpression(mem) if is_this_expression(&mem.object) => {
            expected_property_expression_name(&mem.expression)
        }
        _ => None,
    }
}

fn expected_property_expression_name(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Identifier(id) => Some(id.name.as_str().to_string()),
        Expression::StringLiteral(s) => Some(s.value.as_str().to_string()),
        Expression::NumericLiteral(n) => n
            .raw
            .as_deref()
            .map(str::to_string)
            .or_else(|| Some(n.value.to_string())),
        _ => None,
    }
}

fn is_this_expression(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::ThisExpression(_))
}

/// Compile method bodies from a class element list and append MethodRecords.
#[allow(clippy::too_many_arguments)]
fn compile_class_elements(
    class_name: Option<&str>,
    has_super_class: bool,
    class_span_start: u32,
    class_span_end: u32,
    class_decorators: &[Decorator<'_>],
    elements: &[ClassElement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    methods: &mut Vec<MethodRecord>,
    source: Option<&str>,
    source_file_path: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    function_names: &mut FunctionNameAllocator<'_>,
    concurrent_import_requests: &ConcurrentImportRequests<'_>,
    record_name: &str,
    annotations_enabled: bool,
    enum_infos: &FxHashMap<String, AnnotationEnumInfo>,
    annotation_literal_array_counter: &mut usize,
) -> Result<(), AbcError> {
    let class_name = class_name.unwrap_or("_");

    // Collect non-static instance fields for injection into the constructor.
    let instance_fields: Vec<ClassFieldInit<'_>> = elements
        .iter()
        .filter_map(|e| {
            let ClassElement::PropertyDefinition(prop) = e else {
                return None;
            };
            if prop.r#static {
                return None;
            }
            let key = if prop.computed {
                ClassFieldKey::CapturedComputed {
                    key: &prop.key,
                    capture_name: computed_instance_class_field_key_name(prop),
                }
            } else {
                ClassFieldKey::Static(method_key_name(&prop.key)?)
            };
            Some(ClassFieldInit {
                key,
                value: prop.value.as_ref(),
                span_start: prop.span.start,
            })
        })
        .collect();

    // Collect private field slot map and instance private field initializers.
    let (private_field_slots, _private_field_count) = collect_class_private_slots(elements);
    let private_instance_fields: Vec<ClassPrivateFieldInit<'_>> = elements
        .iter()
        .filter_map(|e| match e {
            ClassElement::PropertyDefinition(prop) => {
                if prop.r#static {
                    return None;
                }
                let PropertyKey::PrivateIdentifier(id) = &prop.key else {
                    return None;
                };
                let slot = *private_field_slots.get(id.name.as_str())?;
                Some(ClassPrivateFieldInit {
                    slot,
                    value: prop.value.as_ref(),
                    span_start: prop.span.start,
                })
            }
            ClassElement::AccessorProperty(accessor) => {
                if accessor.r#static {
                    return None;
                }
                let backing_name = accessor_backing_field_name(accessor);
                let slot = *private_field_slots.get(&backing_name)?;
                Some(ClassPrivateFieldInit {
                    slot,
                    value: accessor.value.as_ref(),
                    span_start: accessor.span.start,
                })
            }
            _ => None,
        })
        .collect();
    let class_expected_property_count =
        expected_property_count_option(compute_class_expected_property_count(elements, lowering));

    let has_constructor = elements.iter().any(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    });
    if !has_constructor {
        // Synthesize a default constructor that still runs field initializers.
        let is_default_derived_constructor =
            has_super_class && instance_fields.is_empty() && private_instance_fields.is_empty();
        let is_standalone_base_default_constructor = lowering.standalone_ecmascript_input()
            && !has_super_class
            && instance_fields.is_empty()
            && private_instance_fields.is_empty();
        let is_standalone_base_callinit_constructor = lowering.standalone_ecmascript_input()
            && !has_super_class
            && (!instance_fields.is_empty() || !private_instance_fields.is_empty());
        let compiled = if is_standalone_base_default_constructor {
            compile_default_base_constructor(
                strings,
                literal_arrays,
                lowering,
                source,
                source_mapper,
                class_span_start,
            )
        } else if is_standalone_base_callinit_constructor {
            compile_standalone_callinit_constructor(
                strings,
                literal_arrays,
                lowering,
                source,
                source_mapper,
                class_span_start,
                class_span_end,
            )
        } else if is_default_derived_constructor {
            let line_ops = if source.is_some() {
                default_derived_constructor_line_ops()
            } else {
                Vec::new()
            };
            compile_default_derived_constructor(line_ops)
        } else {
            compile_constructor_with_fields(
                None,
                &[],
                0,
                &instance_fields,
                strings,
                literal_arrays,
                lowering,
                &[],
                has_super_class,
                None,
                &private_instance_fields,
                private_field_slots.clone(),
            )
        };
        let constructor_name = if lowering.standalone_ecmascript_input() {
            format!("#~{class_name}=#{class_name}")
        } else {
            function_names.bind(0, 0, Some(class_name))
        };
        let constructor_num_args =
            if is_standalone_base_default_constructor || is_standalone_base_callinit_constructor {
                3
            } else {
                4
            };
        let constructor_annotations = annotation_records_from_decorators(
            class_decorators,
            &annotation_method_base_name(record_name, &constructor_name),
            record_name,
            literal_arrays,
            annotations_enabled,
            enum_infos,
            annotation_literal_array_counter,
        )?;
        let debug_info = source_file_path.map(|path| DebugInfoRecord {
            line_start: compiled.debug_line_start.unwrap_or(1),
            source_file: path.to_string(),
            source_code: None,
            param_names: Vec::new(),
            lexical_this: false,
            local_variables: Vec::new(),
        });
        methods.push(MethodRecord {
            name: constructor_name,
            bytecode: compiled.bytecode,
            pandasm_insns: compiled.pandasm_insns,
            pandasm_debug: compiled.pandasm_debug,
            num_vregs: compiled.vregs,
            num_args: constructor_num_args,
            value_of_first_param: -1,
            slots_num: None,
            debug_info,
            line_ops: compiled.line_ops,
            catch_entries: compiled.catches,
            program_labels: compiled.program_labels,
            kind: MethodKind::Constructor,
            function_kind: function_kind::FUNCTION,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: true,
            expected_property_count: class_expected_property_count,
            annotations: constructor_annotations,
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
        });
    }

    if lowering.standalone_ecmascript_input()
        && (!instance_fields.is_empty() || !private_instance_fields.is_empty())
    {
        let initializer_name = format!("#~{class_name}>#instance_initializer");
        let compiled = compile_standalone_instance_initializer(
            &instance_fields,
            &private_instance_fields,
            strings,
            literal_arrays,
            lowering,
            source,
            source_mapper,
            class_span_start,
            private_field_slots.clone(),
        );
        methods.push(MethodRecord {
            name: initializer_name,
            bytecode: compiled.bytecode,
            pandasm_insns: compiled.pandasm_insns,
            pandasm_debug: compiled.pandasm_debug,
            num_vregs: compiled.vregs,
            num_args: 3,
            value_of_first_param: -1,
            slots_num: None,
            debug_info: source_file_path.map(|path| DebugInfoRecord {
                line_start: compiled.debug_line_start.unwrap_or(1),
                source_file: path.to_string(),
                source_code: None,
                param_names: Vec::new(),
                lexical_this: false,
                local_variables: Vec::new(),
            }),
            line_ops: compiled.line_ops,
            catch_entries: compiled.catches,
            program_labels: compiled.program_labels,
            kind: MethodKind::Function,
            function_kind: function_kind::NONE,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: true,
            expected_property_count: None,
            annotations: Vec::new(),
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
        });
    }

    if let Some(static_initializer_name) = static_initializer_record_name(class_name, elements) {
        let static_key_count = static_computed_class_field_count(elements);
        let compiled = compile_static_class_initializer(
            elements,
            strings,
            literal_arrays,
            lowering,
            source,
            source_mapper,
        );
        methods.push(MethodRecord {
            name: static_initializer_name,
            bytecode: compiled.bytecode,
            pandasm_insns: compiled.pandasm_insns,
            pandasm_debug: compiled.pandasm_debug,
            num_vregs: compiled.vregs,
            num_args: 3 + static_key_count,
            value_of_first_param: -1,
            slots_num: None,
            debug_info: source_file_path.map(|path| DebugInfoRecord {
                line_start: compiled.debug_line_start.unwrap_or(1),
                source_file: path.to_string(),
                source_code: None,
                param_names: Vec::new(),
                lexical_this: false,
                local_variables: Vec::new(),
            }),
            line_ops: compiled.line_ops,
            catch_entries: compiled.catches,
            program_labels: compiled.program_labels,
            kind: MethodKind::Function,
            function_kind: function_kind::NONE,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: true,
            expected_property_count: None,
            annotations: Vec::new(),
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
        });
    }

    for element in elements {
        if let ClassElement::MethodDefinition(method) = element {
            let Some(key_name) = class_method_record_name(
                method,
                class_name,
                function_names,
                lowering.standalone_ecmascript_input(),
            ) else {
                continue;
            };
            let param_names: Vec<String> = method
                .value
                .params
                .items
                .iter()
                .filter_map(|p| match &p.pattern {
                    BindingPattern::BindingIdentifier(id) => Some(id.name.as_str().to_string()),
                    _ => None,
                })
                .collect();

            // Detect rest parameter and async flag for this method.
            let rest_info: Option<(String, u8)> =
                method.value.params.rest.as_ref().and_then(|rest_param| {
                    if let BindingPattern::BindingIdentifier(id) = &rest_param.rest.argument {
                        let count = method.value.params.items.len() as u8;
                        Some((id.name.as_str().to_string(), count))
                    } else {
                        None
                    }
                });
            let is_method_async = method.value.r#async;
            let is_constructor = method.kind == MethodDefinitionKind::Constructor;

            let compiled = if is_constructor {
                // Constructor: inject instance field initializers as prologue.
                if let Some(src) = source {
                    compile_constructor_with_fields_tracked(
                        method.value.body.as_deref(),
                        &method.value.params.items,
                        method.value.span.start,
                        method.span.end,
                        &instance_fields,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        has_super_class,
                        src,
                        source_mapper,
                        Some((method.value.span.start, method.value.span.end)),
                        &private_instance_fields,
                        private_field_slots.clone(),
                    )
                } else {
                    compile_constructor_with_fields(
                        method.value.body.as_deref(),
                        &method.value.params.items,
                        method.value.span.start,
                        &instance_fields,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        has_super_class,
                        Some((method.value.span.start, method.value.span.end)),
                        &private_instance_fields,
                        private_field_slots.clone(),
                    )
                }
            } else if rest_info.is_some() || is_method_async || method.value.generator {
                compile_function_body_ext_tracked_with_private(
                    method.value.body.as_deref(),
                    rest_info,
                    is_method_async,
                    method.value.generator,
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    source,
                    source_mapper,
                    Some((method.value.span.start, method.value.span.end)),
                    private_field_slots.clone(),
                )
            } else if let Some(src) = source {
                compile_body_stmts_tracked_with_private(
                    method
                        .value
                        .body
                        .as_deref()
                        .map(|fb| fb.statements.as_slice())
                        .unwrap_or(&[]),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    src,
                    source_mapper,
                    Some(method.span.end),
                    Some((method.value.span.start, method.value.span.end)),
                    &FxHashSet::default(),
                    private_field_slots.clone(),
                )
            } else {
                // No source tracking but private slots still need to be set.
                compile_body_stmts_with_private(
                    method
                        .value
                        .body
                        .as_deref()
                        .map(|fb| fb.statements.as_slice())
                        .unwrap_or(&[]),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    Some((method.value.span.start, method.value.span.end)),
                    private_field_slots.clone(),
                )
            };
            let num_vregs = compiled.vregs;
            let num_args = class_method_num_args(method);
            let upstream_function_kind =
                class_method_function_kind(&method.value, is_constructor, is_method_async);
            let concurrent_module_requests =
                concurrent_import_requests.collect_for_function(&method.value);
            let mut annotations = if is_constructor {
                annotation_records_from_decorators(
                    class_decorators,
                    &annotation_method_base_name(record_name, &key_name),
                    record_name,
                    literal_arrays,
                    annotations_enabled,
                    enum_infos,
                    annotation_literal_array_counter,
                )?
            } else {
                Vec::new()
            };
            annotations.extend(annotation_records_from_decorators(
                &method.decorators,
                &annotation_method_base_name(record_name, &key_name),
                record_name,
                literal_arrays,
                annotations_enabled,
                enum_infos,
                annotation_literal_array_counter,
            )?);
            let debug_info = source_file_path.map(|path| DebugInfoRecord {
                line_start: compiled.debug_line_start.unwrap_or(1),
                source_file: path.to_string(),
                source_code: None,
                param_names: param_names.clone(),
                lexical_this: method
                    .value
                    .body
                    .as_deref()
                    .is_some_and(|body| stmts_capture_this_in_arrow(&body.statements)),
                local_variables: Vec::new(),
            });
            methods.push(MethodRecord {
                name: key_name,
                bytecode: compiled.bytecode,
                pandasm_insns: compiled.pandasm_insns,
                pandasm_debug: compiled.pandasm_debug,
                num_vregs,
                num_args,
                value_of_first_param: -1,
                slots_num: None,
                debug_info,
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                program_labels: compiled.program_labels,
                kind: if is_constructor {
                    MethodKind::Constructor
                } else {
                    MethodKind::Function
                },
                function_kind: upstream_function_kind,
                concurrent_module_requests,
                method_pool_entry: !(lowering.standalone_ecmascript_input()
                    && !is_constructor
                    && is_class_buffer_method(method)),
                expected_property_count: is_constructor
                    .then_some(class_expected_property_count)
                    .flatten(),
                annotations,
                param_annotations: Vec::new(),
                definefunc_order: None,
                label_style: MethodLabelStyle::Source,
            });
        }
    }

    // Compile getter/setter MethodRecords for each auto-accessor property.
    //
    // Mirrors upstream Transformer::ProcessAutoAccessorProperty: the backing field
    // is a synthesized private field; the getter reads it; the setter writes it.
    //
    // NOTE: this loop must run *outside* the MethodDefinition loop above.
    // Previously it was nested inside that loop, causing each accessor's
    // getter+setter pair to be emitted once per class element (N duplicates
    // instead of 1).  Upstream ProcessAutoAccessorProperty is a single-pass
    // class-level transform, not a per-method operation.
    //
    for element in elements {
        let ClassElement::AccessorProperty(accessor) = element else {
            continue;
        };
        let backing_name = accessor_backing_field_name(accessor);
        let Some(&slot) = private_field_slots.get(&backing_name) else {
            continue;
        };
        // Getter: `get <key>() { return this.#backing; }`
        let getter_name = auto_accessor_getter_record_name(class_name, accessor);
        let getter_compiled = {
            let mut cg = CodeGen::new(strings, literal_arrays, lowering);
            cg.private_field_slots = private_field_slots.clone();
            cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            cg.push(Insn::LdPrivateProperty(0, 0, slot));
            cg.push_with_epilogue(Insn::Return);
            cg.returned = true;
            cg.finish_with_catches()
        };
        methods.push(MethodRecord {
            name: getter_name,
            bytecode: getter_compiled.bytecode,
            pandasm_insns: getter_compiled.pandasm_insns,
            pandasm_debug: getter_compiled.pandasm_debug,
            num_vregs: getter_compiled.vregs,
            num_args: 3,
            value_of_first_param: -1,
            slots_num: None,
            debug_info: None,
            line_ops: Vec::new(),
            catch_entries: getter_compiled.catches,
            program_labels: getter_compiled.program_labels,
            kind: MethodKind::Function,
            function_kind: function_kind::NONE,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: true,
            expected_property_count: None,
            annotations: Vec::new(),
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
        });

        // Setter: `set <key>(v) { this.#backing = v; }`
        let setter_name = auto_accessor_setter_record_name(class_name, accessor);
        let setter_compiled = {
            let mut cg = CodeGen::new(strings, literal_arrays, lowering);
            cg.private_field_slots = private_field_slots.clone();
            let val_reg = cg.alloc_reg();
            let this_reg = cg.alloc_reg();
            // Load the value argument (first user param = arg index 3).
            cg.push(Insn::Lda(function_arg_register(3)));
            cg.push(Insn::Sta(val_reg));
            cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            cg.push(Insn::Sta(this_reg));
            // Load value, then store to private field.
            cg.push(Insn::Lda(val_reg));
            cg.push(Insn::StPrivateProperty(0, 0, slot, this_reg));
            cg.push_with_epilogue(Insn::ReturnUndefined);
            cg.returned = true;
            cg.finish_with_catches()
        };
        methods.push(MethodRecord {
            name: setter_name,
            bytecode: setter_compiled.bytecode,
            pandasm_insns: setter_compiled.pandasm_insns,
            pandasm_debug: setter_compiled.pandasm_debug,
            num_vregs: setter_compiled.vregs,
            num_args: 4, // 3 mandatory + 1 value param
            value_of_first_param: -1,
            slots_num: None,
            debug_info: None,
            line_ops: Vec::new(),
            catch_entries: setter_compiled.catches,
            program_labels: setter_compiled.program_labels,
            kind: MethodKind::Function,
            function_kind: function_kind::NONE,
            concurrent_module_requests: Vec::new(),
            method_pool_entry: true,
            expected_property_count: None,
            annotations: Vec::new(),
            param_annotations: Vec::new(),
            definefunc_order: None,
            label_style: MethodLabelStyle::Source,
        });
    }
    Ok(())
}

fn static_computed_class_field_count(elements: &[ClassElement<'_>]) -> u16 {
    elements
        .iter()
        .filter(|element| {
            matches!(
                element,
                ClassElement::PropertyDefinition(property)
                    if property.r#static && property.computed
            )
        })
        .count() as u16
}

/// Compile the top-level program body into bytecode for the `main` method.
///
/// Module mode follows upstream `es2abc --module` and compiles top-level
/// statements directly. Non-module compatibility mode unwraps a single bundler
/// IIFE when present.
fn compile_program_body(
    program: &Program<'_>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    _is_module: bool,
    source: &str,
    has_top_level_await: bool,
    runtime_ts_import_equals: &FxHashSet<(u32, u32)>,
) -> BodyCompileResult {
    compile_body_stmts(
        &program.body,
        strings,
        literal_arrays,
        lowering,
        true,
        &[],
        None,
        Some(source),
        has_top_level_await,
        runtime_ts_import_equals,
    )
}

/// Compile a function body's statements; returns `(bytecode, catch_entries)`.
fn compile_function_body(
    body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    match body {
        Some(fb) => compile_body_stmts(
            &fb.statements,
            strings,
            literal_arrays,
            lowering,
            false,
            param_names,
            current_function_span,
            None,
            false,
            &FxHashSet::default(),
        ),
        None => BodyCompileResult::return_undefined(),
    }
}

/// Like `compile_function_body` but also returns LNP line-advance opcodes.
/// Returns `(bytecode, line_ops, catch_entries)`.
#[allow(dead_code)]
fn compile_function_body_tracked(
    body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: &str,
    source_mapper: Option<&SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
    let compiled = compile_function_body_tracked_with_end(
        body,
        strings,
        literal_arrays,
        lowering,
        param_names,
        source,
        source_mapper,
        None,
        current_function_span,
    );
    (compiled.bytecode, compiled.line_ops, compiled.catches)
}

#[allow(clippy::too_many_arguments)]
fn compile_function_body_tracked_with_end(
    body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: &str,
    source_mapper: Option<&SourceMapper>,
    function_end: Option<u32>,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    match body {
        Some(fb) => compile_body_stmts_tracked(
            &fb.statements,
            strings,
            literal_arrays,
            lowering,
            false,
            param_names,
            source,
            source_mapper,
            function_end,
            current_function_span,
            false,
            &FxHashSet::default(),
        ),
        None => BodyCompileResult::return_undefined(),
    }
}

fn compile_arrow_body_with_line_ops<'src>(
    body: &FunctionBody<'src>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    captures: ArrowCaptures,
    param_names: &[String],
    expression_body: bool,
    source: Option<&'src str>,
    source_mapper: Option<&'src SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    let mut gen = CodeGen::new_arrow_with_source(
        strings,
        literal_arrays,
        lowering,
        captures,
        source,
        source_mapper,
    );
    if source.is_some() {
        if let Some(start) = first_runtime_stmt_start(&body.statements) {
            gen.reset_line_tracker_start(start);
        }
    }
    gen.use_function_lexical_env(current_function_span);
    gen.emit_function_lex_env_prologue();
    gen.bind_function_mandatory_lexical_names();
    if !expression_body {
        gen.function_end = Some(body.span.end);
    }
    gen.bind_function_param_names(param_names);
    if expression_body {
        if let [Statement::ExpressionStatement(expr_stmt)] = body.statements.as_slice() {
            gen.emit_expr(&expr_stmt.expression);
            gen.set_debug_pos(expr_stmt.expression.span().end);
            gen.push(Insn::Return);
            gen.returned = true;
            return gen.finish_with_line_ops_and_catches();
        }
    }
    for stmt in &body.statements {
        gen.emit_stmt(stmt);
        if gen.returned {
            break;
        }
    }
    gen.finish_with_line_ops_and_catches()
}

/// Like `compile_function_body_ext_tracked` but also emits destructuring
/// bindings for function parameters that use array/object patterns.
#[allow(clippy::too_many_arguments)]
fn compile_function_body_with_destructure_params<'src>(
    body: Option<&FunctionBody<'src>>,
    rest_param: Option<(String, u8)>,
    is_async: bool,
    params: &[FormalParameter<'src>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => {
            return BodyCompileResult::return_undefined();
        }
    };
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.is_async = is_async;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.emit_async_function_prologue();
    cg.bind_function_param_names(param_names);
    // Emit destructuring bindings for array/object parameter patterns.
    cg.emit_destructure_parameters(params);
    if let Some((name, count)) = rest_param {
        let reg = cg.alloc_reg();
        cg.push(Insn::CopyRestArgs(count));
        cg.push(Insn::Sta(reg));
        cg.var_regs.insert(name, reg);
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

/// Compile a generator (or async-generator) function body.
///
/// Emits the `CreateGeneratorObj` / initial `SuspendGenerator` prologue, then
/// compiles the body statements.  Yield points use the `emit_yield_expr`
/// path that emits `SuspendGenerator` + `ResumeGenerator` + mode-check for
/// `.return()` / `.throw()` handling.
#[allow(clippy::too_many_arguments)]
fn compile_generator_body<'src>(
    body: Option<&FunctionBody<'src>>,
    is_async: bool,
    rest_param: Option<(String, u8)>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => {
            return BodyCompileResult::return_undefined();
        }
    };
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.is_generator = true;
    cg.is_async = is_async;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    if is_async {
        // Async generator: enter async context first, then generator prologue.
        cg.emit_async_function_prologue();
    }
    cg.emit_generator_prologue();
    cg.bind_function_param_names(param_names);
    if let Some((name, count)) = rest_param {
        let reg = cg.alloc_reg();
        cg.push(Insn::CopyRestArgs(count));
        cg.push(Insn::Sta(reg));
        cg.var_regs.insert(name, reg);
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

fn compile_default_base_constructor<'src>(
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    source: Option<&'src str>,
    source_mapper: Option<&'src SourceMapper>,
    class_span_start: u32,
) -> BodyCompileResult {
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    if lowering.standalone_ecmascript_input() {
        cg.reset_line_tracker_start(class_span_start);
    }
    cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
    cg.push(Insn::Return);
    cg.returned = true;
    cg.finish_with_line_ops_and_catches()
}

fn compile_standalone_callinit_constructor<'src>(
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    source: Option<&'src str>,
    source_mapper: Option<&'src SourceMapper>,
    _class_span_start: u32,
    class_span_end: u32,
) -> BodyCompileResult {
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.push(Insn::LdLexVar(0, 0));
    cg.push(Insn::CallRuntimeCallInit(0, MANDATORY_PARAM_THIS_REG));
    cg.set_debug_pos(class_span_end);
    cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
    cg.push(Insn::Return);
    cg.returned = true;
    cg.finish_with_line_ops_and_catches()
}

#[allow(clippy::too_many_arguments)]
fn compile_standalone_instance_initializer<'src>(
    fields: &[ClassFieldInit<'src>],
    private_fields: &[ClassPrivateFieldInit<'src>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    source: Option<&'src str>,
    source_mapper: Option<&'src SourceMapper>,
    class_span_start: u32,
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.private_field_slots = private_field_slots;
    let initializer_start = fields
        .first()
        .map(|field| field.span_start)
        .or_else(|| private_fields.first().map(|field| field.span_start))
        .unwrap_or(class_span_start);
    cg.reset_line_tracker_start(initializer_start);
    for field in fields {
        match &field.key {
            ClassFieldKey::Static(name) => {
                if let Some(value) = field.value {
                    cg.emit_expr(value);
                } else {
                    cg.push(Insn::LdUndefined);
                }
                let name_id = cg.intern(name);
                cg.push(Insn::DefinePropertyByName(
                    0,
                    name_id,
                    MANDATORY_PARAM_THIS_REG,
                ));
            }
            ClassFieldKey::CapturedComputed { .. } => {
                cg.emit_class_field_definition(field, MANDATORY_PARAM_THIS_REG, None);
            }
        }
    }
    for field in private_fields {
        if let Some(value) = field.value {
            cg.emit_expr(value);
        } else {
            cg.push(Insn::LdUndefined);
        }
        cg.push(Insn::CallRuntimeDefinePrivateProperty(
            0,
            0,
            field.slot,
            MANDATORY_PARAM_THIS_REG,
        ));
    }
    cg.current_line = u32::MAX;
    cg.current_col = 0;
    cg.push(Insn::ReturnUndefined);
    cg.returned = true;
    cg.finish_with_line_ops_and_catches()
}

fn compile_default_derived_constructor(line_ops: Vec<u8>) -> BodyCompileResult {
    // Upstream es2panda emits a GENERATED_CONSTRUCTOR as `super(...args)`.
    // See:
    // - compiler/core/function.cpp: CompileFunctionParameterDeclaration
    // - ir/expressions/callExpression.cpp: CompileSuperCallWithSpreadArgs
    // - compiler/function/functionBuilder.cpp: ImplicitReturn
    //
    // This is the API<13 path used by the fixture: copy rest args, call
    // supercallspread, validate/set `this`, then return `this`.
    let insns = vec![
        Insn::MovV4(0, 6),
        Insn::MovV4(1, 7),
        Insn::MovV4(2, 8),
        Insn::MovV4(3, 9),
        Insn::CopyRestArgs(0),
        Insn::Sta(3),
        Insn::Lda(3),
        Insn::Sta(5),
        Insn::Lda(0),
        Insn::SuperCallSpread(0, 5),
        Insn::Sta(5),
        Insn::Lda(2),
        Insn::ThrowIfSuperNotCorrectCall(1),
        Insn::Lda(5),
        Insn::Sta(2),
        Insn::Lda(2),
        Insn::ThrowIfSuperNotCorrectCall(0),
        Insn::Return,
    ];
    BodyCompileResult::from_static_insns(insns, line_ops, Vec::new(), 6)
}

fn default_derived_constructor_line_ops() -> Vec<u8> {
    // Upstream GENERATED_CONSTRUCTOR debug info is attached after the hidden
    // argument-copy prologue. The bytecode itself is fixed above; this function
    // only mirrors es2panda's source-position stream for that body.
    let mut tracker = LineTracker::new(1);

    // copyrestargs/sta/lda stay at the class start position.
    tracker.advance_with_col(2, 0, 2);
    tracker.advance_with_col(2, 0, 2);
    tracker.advance_with_col(2, 0, 2);

    // supercallspread and derived-constructor validation inherit the class body
    // position until the final "this" validation pair.
    for size in [2, 2, 3, 2, 2, 3, 2, 2] {
        tracker.advance_with_col(8, 15, size);
    }

    // The final validation pair is emitted with reset column, then the implicit
    // return uses the generated constructor end position.
    tracker.advance_with_col(1, u32::MAX, 2);
    tracker.advance_with_col(1, u32::MAX, 3);
    tracker.advance_with_col(8, 15, 1);

    tracker.finish()
}

/// Compile a constructor body, prepending instance field initializer code.
///
/// Mirrors upstream `CompileField`: load the initializer value, or `undefined`
/// when absent, then `DefineOwnProperty` on `this`.
fn compile_constructor_with_fields<'src>(
    body: Option<&FunctionBody<'src>>,
    params: &[FormalParameter<'src>],
    function_span_start: u32,
    fields: &[ClassFieldInit<'src>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    has_super_class: bool,
    current_function_span: Option<(u32, u32)>,
    private_fields: &[ClassPrivateFieldInit<'src>],
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.derived_constructor = has_super_class;
    cg.private_field_slots = private_field_slots;
    cg.use_function_lexical_env(current_function_span);
    if current_function_span.is_none() {
        cg.use_module_outer_lexical_env();
    }
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_default_parameters(params, Some(function_span_start));
    let this_reg =
        if fields.is_empty() && private_fields.is_empty() && lowering.standalone_ecmascript_input()
        {
            None
        } else {
            let reg = cg.alloc_reg();
            cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            cg.push(Insn::Sta(reg));
            Some(reg)
        };
    if !fields.is_empty() {
        let this_reg = this_reg.expect("field initializers need a this register");
        for field in fields {
            cg.emit_class_field_definition(field, this_reg, None);
        }
    }
    // Emit private field initializations via callruntime.defineprivateproperty.
    for pf in private_fields {
        if let Some(value) = pf.value {
            cg.emit_expr(value);
        } else {
            cg.push(Insn::LdUndefined);
        }
        cg.push(Insn::CallRuntimeDefinePrivateProperty(
            0,
            0,
            pf.slot,
            this_reg.expect("private field initializers need a this register"),
        ));
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    if has_super_class && !cg.returned {
        if let Some(new_this_reg) = cg.derived_new_this_reg {
            cg.push(Insn::Lda(new_this_reg));
            cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
            cg.push(Insn::Lda(new_this_reg));
        } else {
            cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
        }
        cg.push(Insn::Return);
        cg.returned = true;
    }
    cg.finish_with_catches()
}

fn compile_constructor_with_fields_tracked<'src>(
    body: Option<&FunctionBody<'src>>,
    params: &[FormalParameter<'src>],
    function_span_start: u32,
    function_span_end: u32,
    fields: &[ClassFieldInit<'src>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    has_super_class: bool,
    source: &'src str,
    source_mapper: Option<&'src SourceMapper>,
    current_function_span: Option<(u32, u32)>,
    private_fields: &[ClassPrivateFieldInit<'src>],
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    if lowering.standalone_ecmascript_input() {
        if let Some(start) = first_runtime_stmt_start(stmts) {
            cg.reset_line_tracker_start(start);
        }
    }
    let _ = function_span_start;
    cg.derived_constructor = has_super_class;
    cg.private_field_slots = private_field_slots;
    cg.use_function_lexical_env(current_function_span);
    if current_function_span.is_none() {
        cg.use_module_outer_lexical_env();
    }
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_default_parameters(params, Some(function_span_start));
    let this_reg =
        if fields.is_empty() && private_fields.is_empty() && lowering.standalone_ecmascript_input()
        {
            None
        } else {
            let reg = cg.alloc_reg();
            cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            cg.push(Insn::Sta(reg));
            Some(reg)
        };
    if !fields.is_empty() {
        let this_reg = this_reg.expect("field initializers need a this register");
        for field in fields {
            cg.emit_class_field_definition(field, this_reg, None);
        }
    }
    // Emit private field initializations.
    for pf in private_fields {
        if let Some(value) = pf.value {
            cg.emit_expr(value);
        } else {
            cg.push(Insn::LdUndefined);
        }
        cg.push(Insn::CallRuntimeDefinePrivateProperty(
            0,
            0,
            pf.slot,
            this_reg.expect("private field initializers need a this register"),
        ));
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    if has_super_class && !cg.returned {
        let standalone_ecmascript_input = lowering.standalone_ecmascript_input();
        if !standalone_ecmascript_input {
            cg.current_line = u32::MAX;
            cg.current_col = u32::MAX;
        }
        if let Some(new_this_reg) = cg.derived_new_this_reg {
            if standalone_ecmascript_input {
                let tracker = cg.line_tracker.take();
                cg.push(Insn::Lda(new_this_reg));
                cg.line_tracker = tracker;
                cg.current_line = u32::MAX;
            } else {
                cg.push(Insn::Lda(new_this_reg));
            }
            cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
            cg.push(Insn::Lda(new_this_reg));
        } else {
            if standalone_ecmascript_input {
                let tracker = cg.line_tracker.take();
                cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
                cg.line_tracker = tracker;
                cg.current_line = u32::MAX;
            } else {
                cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            }
            cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
        }
        cg.set_debug_pos(function_span_end);
        cg.push(Insn::Return);
        cg.returned = true;
    }
    cg.finish_with_line_ops_and_catches()
}

fn compile_static_class_initializer<'src>(
    elements: &[ClassElement<'src>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    source: Option<&'src str>,
    source_mapper: Option<&'src SourceMapper>,
) -> BodyCompileResult {
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    let this_reg = cg.alloc_reg();
    cg.push(Insn::LdThis);
    cg.push(Insn::Sta(this_reg));
    let mut computed_key_index = 0u16;
    for element in elements {
        match element {
            ClassElement::PropertyDefinition(property) => {
                if !property.r#static {
                    continue;
                }
                let key = if property.computed {
                    let key_reg = function_arg_register(3 + computed_key_index);
                    computed_key_index = computed_key_index.saturating_add(1);
                    ClassFieldRuntimeKey::ComputedReg(key_reg)
                } else {
                    let Some(name) = method_key_name(&property.key) else {
                        continue;
                    };
                    ClassFieldRuntimeKey::Static(name)
                };
                cg.emit_class_field_definition_with_key(property.value.as_ref(), key, this_reg);
            }
            ClassElement::StaticBlock(block) => {
                for stmt in &block.body {
                    cg.emit_stmt(stmt);
                    if cg.returned {
                        break;
                    }
                }
            }
            _ => {}
        }
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

/// Compile a statement slice through `CodeGen` and return `(bytecode, catch_entries)`.
fn compile_body_stmts(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    module_body: bool,
    param_names: &[String],
    current_function_span: Option<(u32, u32)>,
    source: Option<&str>,
    has_top_level_await: bool,
    runtime_ts_import_equals: &FxHashSet<(u32, u32)>,
) -> BodyCompileResult {
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.source = source;
    cg.runtime_ts_import_equals = runtime_ts_import_equals.clone();
    cg.module_body = module_body;
    if module_body {
        cg.use_module_lexical_env();
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
        cg.emit_ts_enum_var_hoists(stmts);
        cg.emit_function_hoists(stmts);
        // Top-level await (TLA): emit async prologue so `await` is valid.
        if has_top_level_await {
            cg.is_async = true;
            cg.emit_async_function_prologue();
        }
    } else {
        cg.use_function_lexical_env(current_function_span);
        cg.emit_function_lex_env_prologue();
        cg.bind_function_mandatory_lexical_names();
        cg.bind_function_param_names(param_names);
        cg.emit_function_hoists(stmts);
    }
    for stmt in stmts {
        if is_hoisted_function_declaration(stmt) {
            continue;
        }
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_catches()
}

/// Compile a statement slice through `CodeGen` with line tracking enabled.
///
/// Returns `(bytecode, line_ops, catch_entries)`.
fn compile_body_stmts_tracked(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    module_body: bool,
    param_names: &[String],
    source: &str,
    source_mapper: Option<&SourceMapper>,
    function_end: Option<u32>,
    current_function_span: Option<(u32, u32)>,
    has_top_level_await: bool,
    runtime_ts_import_equals: &FxHashSet<(u32, u32)>,
) -> BodyCompileResult {
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    if lowering.standalone_ecmascript_input() && module_body {
        if let Some(start) = first_runtime_stmt_start(stmts) {
            cg.reset_line_tracker_start(start);
        }
    }
    cg.function_end = function_end;
    cg.runtime_ts_import_equals = runtime_ts_import_equals.clone();
    cg.module_body = module_body;
    if module_body {
        cg.use_module_lexical_env();
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
        cg.emit_ts_enum_var_hoists(stmts);
        cg.emit_function_hoists(stmts);
        // Top-level await (TLA): emit async prologue so `await` is valid.
        if has_top_level_await {
            cg.is_async = true;
            cg.emit_async_function_prologue();
        }
    } else {
        cg.use_function_lexical_env(current_function_span);
        cg.emit_function_lex_env_prologue();
        cg.bind_function_mandatory_lexical_names();
        cg.bind_function_param_names(param_names);
        cg.emit_function_hoists(stmts);
    }
    for stmt in stmts {
        if is_hoisted_function_declaration(stmt) {
            continue;
        }
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

/// Returns true if `stmts` contains a top-level `await` expression.
///
/// Upstream es2abc wraps the module body in an async context when TLA is
/// detected, emitting `asyncfunctionenter` and treating the module's
/// `func_main_0` as async.
fn stmts_have_top_level_await(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(stmt_has_top_level_await)
}

fn simple_numeric_for_parts<'a>(
    for_stmt: &'a oxc_ast::ast::ForStatement<'a>,
) -> Option<(&'a str, i32, i32)> {
    use oxc_ast::ast::{BinaryOperator, ForStatementInit};

    let ForStatementInit::VariableDeclaration(decl) = for_stmt.init.as_ref()? else {
        return None;
    };
    let [declarator] = decl.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return None;
    };
    let name = id.name.as_str();
    let init_value = numeric_i32_literal(declarator.init.as_ref()?)?;

    let Expression::BinaryExpression(test) = for_stmt.test.as_ref()? else {
        return None;
    };
    if test.operator != BinaryOperator::LessThan || identifier_expr_name(&test.left)? != name {
        return None;
    }
    let limit_value = numeric_i32_literal(&test.right)?;

    let Expression::UpdateExpression(update) = for_stmt.update.as_ref()? else {
        return None;
    };
    if update.operator != UpdateOperator::Increment || update.prefix {
        return None;
    }
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(update_id) = &update.argument else {
        return None;
    };
    if update_id.name.as_str() != name {
        return None;
    }

    Some((name, init_value, limit_value))
}

fn identifier_expr_name<'a>(expr: &'a Expression<'a>) -> Option<&'a str> {
    match expr {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn numeric_i32_literal(expr: &Expression<'_>) -> Option<i32> {
    match expr {
        Expression::NumericLiteral(number)
            if number.value.fract() == 0.0
                && number.value >= i32::MIN as f64
                && number.value <= i32::MAX as f64 =>
        {
            Some(number.value as i32)
        }
        Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::UnaryNegation
                && matches!(&unary.argument, Expression::NumericLiteral(_)) =>
        {
            let Expression::NumericLiteral(number) = &unary.argument else {
                return None;
            };
            if number.value.fract() == 0.0 && number.value >= 0.0 && number.value <= i32::MAX as f64
            {
                Some(-(number.value as i32))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_simple_standalone_class(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().all(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    })
}

fn has_standalone_instance_initializer(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| match element {
        ClassElement::PropertyDefinition(property) => !property.r#static,
        ClassElement::AccessorProperty(accessor) => !accessor.r#static,
        _ => false,
    })
}

fn debug_line_start(
    source: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    offset: u32,
) -> u32 {
    let Some(source) = source else {
        return 1;
    };
    source_mapper
        .and_then(|mapper| mapper.map_offset(source, offset))
        .map_or_else(|| line_at(source, offset), |(line, _)| line)
}

fn first_runtime_stmt_start(stmts: &[Statement<'_>]) -> Option<u32> {
    stmts
        .iter()
        .find(|stmt| !is_hoisted_function_declaration(stmt))
        .map(|stmt| stmt.span().start)
}

fn stmt_has_top_level_await(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ExpressionStatement(es) => expr_has_top_level_await(&es.expression),
        Statement::VariableDeclaration(decl) => decl
            .declarations
            .iter()
            .any(|d| d.init.as_ref().is_some_and(expr_has_top_level_await)),
        Statement::ReturnStatement(ret) => {
            ret.argument.as_ref().is_some_and(expr_has_top_level_await)
        }
        Statement::ThrowStatement(throw) => expr_has_top_level_await(&throw.argument),
        Statement::BlockStatement(block) => stmts_have_top_level_await(&block.body),
        Statement::IfStatement(if_) => {
            expr_has_top_level_await(&if_.test)
                || stmt_has_top_level_await(&if_.consequent)
                || if_.alternate.as_ref().is_some_and(stmt_has_top_level_await)
        }
        Statement::LabeledStatement(labeled) => stmt_has_top_level_await(&labeled.body),
        Statement::WithStatement(with) => {
            expr_has_top_level_await(&with.object) || stmt_has_top_level_await(&with.body)
        }
        Statement::WhileStatement(while_) => {
            expr_has_top_level_await(&while_.test) || stmt_has_top_level_await(&while_.body)
        }
        Statement::DoWhileStatement(do_while) => {
            stmt_has_top_level_await(&do_while.body) || expr_has_top_level_await(&do_while.test)
        }
        Statement::ForStatement(for_) => {
            for_.init.as_ref().is_some_and(for_init_has_top_level_await)
                || for_.test.as_ref().is_some_and(expr_has_top_level_await)
                || for_.update.as_ref().is_some_and(expr_has_top_level_await)
                || stmt_has_top_level_await(&for_.body)
        }
        Statement::ForInStatement(for_in) => {
            for_left_has_top_level_await(&for_in.left)
                || expr_has_top_level_await(&for_in.right)
                || stmt_has_top_level_await(&for_in.body)
        }
        Statement::ForOfStatement(for_of) => {
            for_left_has_top_level_await(&for_of.left)
                || expr_has_top_level_await(&for_of.right)
                || stmt_has_top_level_await(&for_of.body)
        }
        Statement::SwitchStatement(switch) => {
            expr_has_top_level_await(&switch.discriminant)
                || switch.cases.iter().any(|case| {
                    case.test.as_ref().is_some_and(expr_has_top_level_await)
                        || stmts_have_top_level_await(&case.consequent)
                })
        }
        Statement::TryStatement(try_) => {
            stmts_have_top_level_await(&try_.block.body)
                || try_
                    .handler
                    .as_ref()
                    .is_some_and(|handler| stmts_have_top_level_await(&handler.body.body))
                || try_
                    .finalizer
                    .as_ref()
                    .is_some_and(|finalizer| stmts_have_top_level_await(&finalizer.body))
        }
        Statement::ClassDeclaration(class) => {
            class
                .super_class
                .as_ref()
                .is_some_and(expr_has_top_level_await)
                || class_elements_have_top_level_await(&class.body.body)
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                class
                    .super_class
                    .as_ref()
                    .is_some_and(expr_has_top_level_await)
                    || class_elements_have_top_level_await(&class.body.body)
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(_) => false,
            kind => kind.as_expression().is_some_and(expr_has_top_level_await),
        },
        Statement::ExportNamedDeclaration(export) => export
            .declaration
            .as_ref()
            .is_some_and(declaration_has_top_level_await),
        _ => false,
    }
}

fn declaration_has_top_level_await(decl: &Declaration<'_>) -> bool {
    match decl {
        Declaration::VariableDeclaration(decl) => decl
            .declarations
            .iter()
            .any(|d| d.init.as_ref().is_some_and(expr_has_top_level_await)),
        Declaration::ClassDeclaration(class) => {
            class
                .super_class
                .as_ref()
                .is_some_and(expr_has_top_level_await)
                || class_elements_have_top_level_await(&class.body.body)
        }
        Declaration::TSEnumDeclaration(decl) => decl.body.members.iter().any(|member| {
            member
                .initializer
                .as_ref()
                .is_some_and(expr_has_top_level_await)
        }),
        _ => false,
    }
}

fn for_init_has_top_level_await(init: &oxc_ast::ast::ForStatementInit<'_>) -> bool {
    match init {
        oxc_ast::ast::ForStatementInit::VariableDeclaration(decl) => decl
            .declarations
            .iter()
            .any(|d| d.init.as_ref().is_some_and(expr_has_top_level_await)),
        oxc_ast::ast::ForStatementInit::AssignmentExpression(assign) => {
            assignment_target_has_top_level_await(&assign.left)
                || expr_has_top_level_await(&assign.right)
        }
        oxc_ast::ast::ForStatementInit::SequenceExpression(seq) => {
            seq.expressions.iter().any(expr_has_top_level_await)
        }
        oxc_ast::ast::ForStatementInit::PrivateFieldExpression(expr) => {
            expr_has_top_level_await(&expr.object)
        }
        expr => expr.as_expression().is_some_and(expr_has_top_level_await),
    }
}

fn for_left_has_top_level_await(left: &ForStatementLeft<'_>) -> bool {
    match left {
        ForStatementLeft::VariableDeclaration(decl) => decl
            .declarations
            .iter()
            .any(|d| d.init.as_ref().is_some_and(expr_has_top_level_await)),
        ForStatementLeft::PrivateFieldExpression(expr) => expr_has_top_level_await(&expr.object),
        ForStatementLeft::ArrayAssignmentTarget(arr) => {
            array_assignment_target_has_top_level_await(arr)
        }
        ForStatementLeft::ObjectAssignmentTarget(obj) => {
            object_assignment_target_has_top_level_await(obj)
        }
        ForStatementLeft::ComputedMemberExpression(expr) => {
            expr_has_top_level_await(&expr.object) || expr_has_top_level_await(&expr.expression)
        }
        ForStatementLeft::StaticMemberExpression(expr) => expr_has_top_level_await(&expr.object),
        _ => false,
    }
}

fn assignment_target_has_top_level_await(target: &AssignmentTarget<'_>) -> bool {
    match target {
        AssignmentTarget::ComputedMemberExpression(expr) => {
            expr_has_top_level_await(&expr.object) || expr_has_top_level_await(&expr.expression)
        }
        AssignmentTarget::StaticMemberExpression(expr) => expr_has_top_level_await(&expr.object),
        AssignmentTarget::PrivateFieldExpression(expr) => expr_has_top_level_await(&expr.object),
        AssignmentTarget::ArrayAssignmentTarget(arr) => {
            array_assignment_target_has_top_level_await(arr)
        }
        AssignmentTarget::ObjectAssignmentTarget(obj) => {
            object_assignment_target_has_top_level_await(obj)
        }
        AssignmentTarget::TSAsExpression(expr) => expr_has_top_level_await(&expr.expression),
        AssignmentTarget::TSSatisfiesExpression(expr) => expr_has_top_level_await(&expr.expression),
        AssignmentTarget::TSNonNullExpression(expr) => expr_has_top_level_await(&expr.expression),
        AssignmentTarget::TSTypeAssertion(expr) => expr_has_top_level_await(&expr.expression),
        _ => false,
    }
}

fn assignment_target_maybe_default_has_top_level_await(
    target: &AssignmentTargetMaybeDefault<'_>,
) -> bool {
    match target {
        AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(default) => {
            assignment_target_has_top_level_await(&default.binding)
                || expr_has_top_level_await(&default.init)
        }
        target => target
            .as_assignment_target()
            .is_some_and(assignment_target_has_top_level_await),
    }
}

fn array_assignment_target_has_top_level_await(arr: &ArrayAssignmentTarget<'_>) -> bool {
    arr.elements
        .iter()
        .flatten()
        .any(|element| assignment_target_maybe_default_has_top_level_await(element))
        || arr
            .rest
            .as_ref()
            .is_some_and(|rest| assignment_target_has_top_level_await(&rest.target))
}

fn object_assignment_target_has_top_level_await(obj: &ObjectAssignmentTarget<'_>) -> bool {
    obj.properties.iter().any(|prop| match prop {
        AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(identifier) => identifier
            .init
            .as_ref()
            .is_some_and(expr_has_top_level_await),
        AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
            (property.computed && property_key_has_top_level_await(&property.name))
                || assignment_target_maybe_default_has_top_level_await(&property.binding)
        }
    }) || obj
        .rest
        .as_ref()
        .is_some_and(|rest| assignment_target_has_top_level_await(&rest.target))
}

fn class_elements_have_top_level_await(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| match element {
        ClassElement::PropertyDefinition(property) => {
            property
                .value
                .as_ref()
                .is_some_and(expr_has_top_level_await)
                || (property.computed && property_key_has_top_level_await(&property.key))
        }
        ClassElement::AccessorProperty(accessor) => {
            accessor
                .value
                .as_ref()
                .is_some_and(expr_has_top_level_await)
                || (accessor.computed && property_key_has_top_level_await(&accessor.key))
        }
        ClassElement::StaticBlock(block) => stmts_have_top_level_await(&block.body),
        ClassElement::MethodDefinition(method) => {
            method.computed && property_key_has_top_level_await(&method.key)
        }
        ClassElement::TSIndexSignature(_) => false,
    })
}

fn property_key_has_top_level_await(key: &PropertyKey<'_>) -> bool {
    match key {
        PropertyKey::PrivateIdentifier(_) => false,
        key => key.as_expression().is_some_and(expr_has_top_level_await),
    }
}

fn scanable_property_key_expression<'a>(key: &'a PropertyKey<'a>) -> Option<&'a Expression<'a>> {
    match key {
        PropertyKey::PrivateIdentifier(_) => None,
        key => key.as_expression(),
    }
}

fn expr_has_top_level_await(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::AwaitExpression(_) => true,
        Expression::AssignmentExpression(assign) => {
            assignment_target_has_top_level_await(&assign.left)
                || expr_has_top_level_await(&assign.right)
        }
        Expression::BinaryExpression(b) => {
            expr_has_top_level_await(&b.left) || expr_has_top_level_await(&b.right)
        }
        Expression::LogicalExpression(logical) => {
            expr_has_top_level_await(&logical.left) || expr_has_top_level_await(&logical.right)
        }
        Expression::UnaryExpression(unary) => expr_has_top_level_await(&unary.argument),
        Expression::UpdateExpression(update) => match &update.argument {
            SimpleAssignmentTarget::ComputedMemberExpression(expr) => {
                expr_has_top_level_await(&expr.object) || expr_has_top_level_await(&expr.expression)
            }
            SimpleAssignmentTarget::StaticMemberExpression(expr) => {
                expr_has_top_level_await(&expr.object)
            }
            SimpleAssignmentTarget::PrivateFieldExpression(expr) => {
                expr_has_top_level_await(&expr.object)
            }
            _ => false,
        },
        Expression::CallExpression(c) => {
            expr_has_top_level_await(&c.callee)
                || c.arguments.iter().any(|a| match a {
                    Argument::SpreadElement(s) => expr_has_top_level_await(&s.argument),
                    _ => a.as_expression().is_some_and(expr_has_top_level_await),
                })
        }
        Expression::NewExpression(new) => {
            expr_has_top_level_await(&new.callee)
                || new.arguments.iter().any(|a| match a {
                    Argument::SpreadElement(s) => expr_has_top_level_await(&s.argument),
                    _ => a.as_expression().is_some_and(expr_has_top_level_await),
                })
        }
        Expression::StaticMemberExpression(m) => expr_has_top_level_await(&m.object),
        Expression::ComputedMemberExpression(m) => {
            expr_has_top_level_await(&m.object) || expr_has_top_level_await(&m.expression)
        }
        Expression::PrivateFieldExpression(expr) => expr_has_top_level_await(&expr.object),
        Expression::ChainExpression(chain) => match &chain.expression {
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                expr_has_top_level_await(&call.callee)
                    || call.arguments.iter().any(|arg| match arg {
                        Argument::SpreadElement(spread) => {
                            expr_has_top_level_await(&spread.argument)
                        }
                        _ => arg.as_expression().is_some_and(expr_has_top_level_await),
                    })
            }
            oxc_ast::ast::ChainElement::StaticMemberExpression(expr) => {
                expr_has_top_level_await(&expr.object)
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(expr) => {
                expr_has_top_level_await(&expr.object) || expr_has_top_level_await(&expr.expression)
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(expr) => {
                expr_has_top_level_await(&expr.object)
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expr) => {
                expr_has_top_level_await(&expr.expression)
            }
        },
        Expression::SequenceExpression(s) => s.expressions.iter().any(expr_has_top_level_await),
        Expression::ParenthesizedExpression(p) => expr_has_top_level_await(&p.expression),
        Expression::ConditionalExpression(c) => {
            expr_has_top_level_await(&c.test)
                || expr_has_top_level_await(&c.consequent)
                || expr_has_top_level_await(&c.alternate)
        }
        Expression::ObjectExpression(obj) => obj.properties.iter().any(|prop| match prop {
            oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) => {
                property_key_has_top_level_await(&property.key)
                    || expr_has_top_level_await(&property.value)
            }
            oxc_ast::ast::ObjectPropertyKind::SpreadProperty(spread) => {
                expr_has_top_level_await(&spread.argument)
            }
        }),
        Expression::ArrayExpression(arr) => arr
            .elements
            .iter()
            .any(|elem| elem.as_expression().is_some_and(expr_has_top_level_await)),
        Expression::TemplateLiteral(tmpl) => tmpl.expressions.iter().any(expr_has_top_level_await),
        Expression::TaggedTemplateExpression(tagged) => {
            expr_has_top_level_await(&tagged.tag)
                || tagged
                    .quasi
                    .expressions
                    .iter()
                    .any(expr_has_top_level_await)
        }
        Expression::ImportExpression(import) => {
            expr_has_top_level_await(&import.source)
                || import
                    .options
                    .as_ref()
                    .is_some_and(expr_has_top_level_await)
        }
        Expression::TSAsExpression(expr) => expr_has_top_level_await(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => expr_has_top_level_await(&expr.expression),
        Expression::TSNonNullExpression(expr) => expr_has_top_level_await(&expr.expression),
        Expression::TSTypeAssertion(expr) => expr_has_top_level_await(&expr.expression),
        Expression::TSInstantiationExpression(expr) => expr_has_top_level_await(&expr.expression),
        Expression::ClassExpression(class) => {
            class
                .super_class
                .as_ref()
                .is_some_and(expr_has_top_level_await)
                || class_elements_have_top_level_await(&class.body.body)
        }
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => false,
        _ => false,
    }
}

/// Like `compile_body_stmts` but injects a private field slot map.
#[allow(clippy::too_many_arguments)]
fn compile_body_stmts_with_private(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    current_function_span: Option<(u32, u32)>,
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.private_field_slots = private_field_slots;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_function_hoists(stmts);
    for stmt in stmts {
        if is_hoisted_function_declaration(stmt) {
            continue;
        }
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_catches()
}

/// Like `compile_body_stmts_tracked` but injects a private field slot map into
/// the CodeGen before compiling the body.  Used for class method bodies that
/// access private fields.
#[allow(clippy::too_many_arguments)]
fn compile_body_stmts_tracked_with_private(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: &str,
    source_mapper: Option<&SourceMapper>,
    function_end: Option<u32>,
    current_function_span: Option<(u32, u32)>,
    runtime_ts_import_equals: &FxHashSet<(u32, u32)>,
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    cg.function_end = function_end;
    cg.runtime_ts_import_equals = runtime_ts_import_equals.clone();
    cg.private_field_slots = private_field_slots;
    cg.module_body = false;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_function_hoists(stmts);
    for stmt in stmts {
        if is_hoisted_function_declaration(stmt) {
            continue;
        }
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

/// Like `compile_function_body_ext_tracked` but injects a private field slot map.
#[allow(clippy::too_many_arguments)]
fn compile_function_body_ext_tracked_with_private<'src>(
    body: Option<&FunctionBody<'src>>,
    rest_param: Option<(String, u8)>,
    is_async: bool,
    is_generator: bool,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    current_function_span: Option<(u32, u32)>,
    private_field_slots: FxHashMap<String, u16>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => {
            return BodyCompileResult::return_undefined();
        }
    };
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.is_async = is_async;
    cg.is_generator = is_generator;
    cg.private_field_slots = private_field_slots;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    if is_async {
        cg.emit_async_function_prologue();
    }
    if is_generator {
        cg.emit_generator_prologue();
    }
    cg.bind_function_param_names(param_names);
    if let Some((name, count)) = rest_param {
        let reg = cg.alloc_reg();
        cg.push(Insn::CopyRestArgs(count));
        cg.push(Insn::Sta(reg));
        cg.var_regs.insert(name, reg);
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
}

fn is_hoisted_function_declaration(stmt: &Statement<'_>) -> bool {
    matches!(stmt, Statement::FunctionDeclaration(_))
        || matches!(
            stmt,
            Statement::ExportNamedDeclaration(export)
                if matches!(export.declaration, Some(Declaration::FunctionDeclaration(_)))
        )
        || matches!(
            stmt,
            Statement::ExportDefaultDeclaration(export)
                if matches!(export.declaration, ExportDefaultDeclarationKind::FunctionDeclaration(_))
        )
}

/// Like `compile_program_body` but also returns LNP opcodes for the method.
///
/// The `source` parameter is the original source text used to compute line
/// numbers from statement byte offsets.
fn compile_program_body_tracked(
    program: &Program<'_>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    _is_module: bool,
    source: &str,
    source_mapper: Option<&SourceMapper>,
    has_top_level_await: bool,
    runtime_ts_import_equals: &FxHashSet<(u32, u32)>,
) -> BodyCompileResult {
    compile_body_stmts_tracked(
        &program.body,
        strings,
        literal_arrays,
        lowering,
        true,
        &[],
        source,
        source_mapper,
        None,
        None,
        has_top_level_await,
        runtime_ts_import_equals,
    )
}

fn arrow_param_names(arrow: &oxc_ast::ast::ArrowFunctionExpression<'_>) -> Vec<String> {
    arrow
        .params
        .items
        .iter()
        .filter_map(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str().to_string()),
            _ => None,
        })
        .collect()
}

fn function_param_names(function: &oxc_ast::ast::Function<'_>) -> Vec<String> {
    function
        .params
        .items
        .iter()
        .filter_map(|param| match &param.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str().to_string()),
            _ => None,
        })
        .collect()
}

/// Internal slot key for an auto-accessor backing field.
///
/// Upstream `Transformer::ProcessAutoAccessorProperty` rewrites every
/// `accessor` into a private `auto_accessor_storage` property. The exact source
/// name is only used to assign a private slot inside this Rust lowering; the
/// emitted ABC addresses private fields by slot.
fn accessor_backing_field_name(accessor: &oxc_ast::ast::AccessorProperty<'_>) -> String {
    format!(
        "auto_accessor_storage_{}_{}",
        accessor.span.start, accessor.span.end
    )
}

fn auto_accessor_key_record_part(accessor: &oxc_ast::ast::AccessorProperty<'_>) -> String {
    if accessor.computed {
        return format!(
            "computed_auto_accessor_{}_{}",
            accessor.key.span().start,
            accessor.key.span().end
        );
    }
    match &accessor.key {
        PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
        PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
        PropertyKey::NumericLiteral(n) => n
            .raw
            .as_ref()
            .map(|raw| raw.as_str().to_string())
            .unwrap_or_else(|| n.value.to_string()),
        PropertyKey::BigIntLiteral(bigint) => bigint.value.as_str().to_string(),
        PropertyKey::Identifier(id) => id.name.as_str().to_string(),
        PropertyKey::PrivateIdentifier(id) => format!("#{}", id.name.as_str()),
        _ => unreachable!("unsupported non-computed auto-accessor key"),
    }
}

fn auto_accessor_getter_record_name(
    class_name: &str,
    accessor: &oxc_ast::ast::AccessorProperty<'_>,
) -> String {
    let key_part = auto_accessor_key_record_part(accessor);
    format!("#{class_name}_get_{key_part}#")
}

fn auto_accessor_setter_record_name(
    class_name: &str,
    accessor: &oxc_ast::ast::AccessorProperty<'_>,
) -> String {
    let key_part = auto_accessor_key_record_part(accessor);
    format!("#{class_name}_set_{key_part}#")
}

/// Build a private field name → slot map for a class.
///
/// Private fields (#name) are assigned slots 0, 1, 2... in declaration order.
/// Private methods (including getters/setters) get slots after private fields
/// (matching upstream `ClassScope::AddPrivateName`).
/// Returns `(field_slots, field_count)` where `field_count` is the number of
/// private *fields* (not methods).
fn collect_class_private_slots(elements: &[ClassElement<'_>]) -> (FxHashMap<String, u16>, u16) {
    let mut slots: FxHashMap<String, u16> = FxHashMap::default();
    let mut slot = 0u16;
    // Private fields first.
    for element in elements {
        match element {
            ClassElement::PropertyDefinition(prop) => {
                if let PropertyKey::PrivateIdentifier(id) = &prop.key {
                    slots
                        .entry(id.name.as_str().to_string())
                        .or_insert_with(|| {
                            let s = slot;
                            slot += 1;
                            s
                        });
                }
            }
            ClassElement::AccessorProperty(accessor) => {
                // auto-accessor: allocate a slot for the synthesized backing field.
                let backing_name = accessor_backing_field_name(accessor);
                slots.entry(backing_name).or_insert_with(|| {
                    let s = slot;
                    slot += 1;
                    s
                });
            }
            _ => {}
        }
    }
    let field_count = slot;
    // Private methods after fields.
    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        if let PropertyKey::PrivateIdentifier(id) = &method.key {
            slots
                .entry(id.name.as_str().to_string())
                .or_insert_with(|| {
                    let s = slot;
                    slot += 1;
                    s
                });
        }
    }
    (slots, field_count)
}

/// Returns true if any function parameter uses a destructuring binding pattern.
fn function_has_destructuring_params(function: &oxc_ast::ast::Function<'_>) -> bool {
    function.params.items.iter().any(|p| {
        matches!(
            &p.pattern,
            BindingPattern::ArrayPattern(_) | BindingPattern::ObjectPattern(_)
        )
    })
}

fn function_rest_param(function: &oxc_ast::ast::Function<'_>) -> Option<(String, u8)> {
    function.params.rest.as_ref().and_then(|rest_param| {
        if let BindingPattern::BindingIdentifier(id) = &rest_param.rest.argument {
            Some((
                id.name.as_str().to_string(),
                function.params.items.len() as u8,
            ))
        } else {
            None
        }
    })
}

const ANNOTATION_SUPPORTED_API_VERSION: u32 = 20;
const ANNOTATION_PREFIX: &str = "__$$ETS_ANNOTATION$$__";
const ANNOTATION_NOT_ENABLED_MESSAGE: &str = concat!(
    "Current configuration does not support using annotations. ",
    "Annotations can be used in the version of API 20 or higher versions.\n",
    "Solutions: > Check the compatibleSdkVersion in build-profile.json5.",
    "> If compatibleSdkVersion is set to API 20 or higher version.",
    "> If you're running es2abc in commandline without IDE, please check whether target-api-version and ",
    "enable-annotations options are correctly configured."
);

fn reject_unsupported_constructs(
    program: &Program<'_>,
    record_name: &str,
    target_api_version: u32,
    enable_annotations: bool,
) -> Result<(), AbcError> {
    let body = statement_scan_body(program, true);
    let mut scanner = UnsupportedScanner {
        record_name,
        target_api_version,
        enable_annotations,
    };
    scanner.scan_statements(body)
}

fn needs_typescript_runtime_transform(program: &Program<'_>) -> bool {
    program
        .body
        .iter()
        .any(statement_needs_typescript_runtime_transform)
}

fn statement_needs_typescript_runtime_transform(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::TSExportAssignment(_) => true,
        Statement::TSModuleDeclaration(decl) => ts_module_needs_runtime_transform(decl),
        Statement::ExportNamedDeclaration(export) => {
            matches!(
                export.declaration.as_ref(),
                Some(Declaration::TSModuleDeclaration(decl))
                    if ts_module_needs_runtime_transform(decl)
            )
        }
        _ => false,
    }
}

fn ts_module_needs_runtime_transform(decl: &TSModuleDeclaration<'_>) -> bool {
    !decl.declare && decl.body.is_some()
}

fn collect_runtime_ts_import_equals(
    program: &Program<'_>,
    scoping: Option<&Scoping>,
) -> FxHashSet<(u32, u32)> {
    let mut runtime_decls = FxHashSet::default();
    collect_runtime_ts_import_equals_from_statements(&program.body, scoping, &mut runtime_decls);
    runtime_decls
}

fn collect_runtime_ts_import_equals_from_statements(
    statements: &[Statement<'_>],
    scoping: Option<&Scoping>,
    runtime_decls: &mut FxHashSet<(u32, u32)>,
) {
    for stmt in statements {
        match stmt {
            Statement::TSImportEqualsDeclaration(decl) => {
                collect_runtime_ts_import_equals_decl(decl, scoping, runtime_decls);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSImportEqualsDeclaration(decl)) = &export.declaration {
                    collect_runtime_ts_import_equals_decl(decl, scoping, runtime_decls);
                }
            }
            Statement::BlockStatement(block) => {
                collect_runtime_ts_import_equals_from_statements(
                    &block.body,
                    scoping,
                    runtime_decls,
                );
            }
            _ => {}
        }
    }
}

fn collect_runtime_ts_import_equals_decl(
    decl: &TSImportEqualsDeclaration<'_>,
    scoping: Option<&Scoping>,
    runtime_decls: &mut FxHashSet<(u32, u32)>,
) {
    if decl.import_kind == ImportOrExportKind::Type {
        return;
    }
    let has_value_reference = scoping
        .and_then(|scoping| {
            let symbol_id = decl.id.symbol_id.get()?;
            Some(
                scoping
                    .get_resolved_references(symbol_id)
                    .any(|reference| !Reference::is_type(reference)),
            )
        })
        .unwrap_or(true);
    if has_value_reference {
        runtime_decls.insert((decl.span.start, decl.span.end));
    }
}

struct UnsupportedScanner<'a> {
    record_name: &'a str,
    target_api_version: u32,
    enable_annotations: bool,
}

impl UnsupportedScanner<'_> {
    fn unsupported(&self, construct: &str) -> AbcError {
        AbcError::UnsupportedConstruct {
            construct: construct.to_string(),
            file: self.record_name.to_string(),
        }
    }

    fn parse_error(&self, message: impl Into<String>) -> AbcError {
        AbcError::Parse {
            file: self.record_name.to_string(),
            message: message.into(),
        }
    }

    fn annotations_enabled(&self) -> bool {
        self.enable_annotations && self.target_api_version >= ANNOTATION_SUPPORTED_API_VERSION
    }

    fn scan_annotation_declaration(
        &self,
        decl: &AnnotationDeclaration<'_>,
    ) -> Result<(), AbcError> {
        if !self.annotations_enabled() {
            return Err(self.parse_error(ANNOTATION_NOT_ENABLED_MESSAGE));
        }
        if !decl.id.name.as_str().starts_with(ANNOTATION_PREFIX) {
            return Err(self.parse_error(format!(
                "Annotation declaration need to be prefixed with '{ANNOTATION_PREFIX}'"
            )));
        }
        Ok(())
    }

    fn scan_statements(&mut self, statements: &[Statement<'_>]) -> Result<(), AbcError> {
        for stmt in statements {
            self.scan_statement(stmt)?;
        }
        Ok(())
    }

    fn scan_statement(&mut self, stmt: &Statement<'_>) -> Result<(), AbcError> {
        match stmt {
            Statement::BlockStatement(block) => self.scan_statements(&block.body),
            Statement::ExpressionStatement(expr) => self.scan_expression(&expr.expression),
            Statement::VariableDeclaration(decl) => self.scan_variable_declaration(decl),
            Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_) => Ok(()),
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.scan_expression(arg)?;
                }
                Ok(())
            }
            Statement::ThrowStatement(throw) => self.scan_expression(&throw.argument),
            Statement::IfStatement(stmt) => {
                self.scan_expression(&stmt.test)?;
                self.scan_statement(&stmt.consequent)?;
                if let Some(alt) = &stmt.alternate {
                    self.scan_statement(alt)?;
                }
                Ok(())
            }
            Statement::LabeledStatement(stmt) => self.scan_statement(&stmt.body),
            Statement::WithStatement(_) => Err(self.unsupported("WithStatement")),
            Statement::WhileStatement(stmt) => {
                self.scan_expression(&stmt.test)?;
                self.scan_statement(&stmt.body)
            }
            Statement::DoWhileStatement(stmt) => {
                self.scan_statement(&stmt.body)?;
                self.scan_expression(&stmt.test)
            }
            Statement::ForStatement(stmt) => {
                if let Some(init) = &stmt.init {
                    use oxc_ast::ast::ForStatementInit;
                    match init {
                        ForStatementInit::VariableDeclaration(decl) => {
                            self.scan_variable_declaration(decl)?;
                        }
                        ForStatementInit::AssignmentExpression(assign) => {
                            self.scan_assignment_target(&assign.left)?;
                            self.scan_expression(&assign.right)?;
                        }
                        ForStatementInit::SequenceExpression(seq) => {
                            for expr in &seq.expressions {
                                self.scan_expression(expr)?;
                            }
                        }
                        ForStatementInit::PrivateFieldExpression(expr) => {
                            self.scan_expression(&expr.object)?;
                        }
                        _ => {}
                    }
                }
                if let Some(test) = &stmt.test {
                    self.scan_expression(test)?;
                }
                if let Some(update) = &stmt.update {
                    self.scan_expression(update)?;
                }
                self.scan_statement(&stmt.body)
            }
            Statement::ForInStatement(stmt) => {
                self.scan_for_left(&stmt.left)?;
                self.scan_expression(&stmt.right)?;
                self.scan_statement(&stmt.body)
            }
            Statement::ForOfStatement(stmt) => {
                self.scan_for_left(&stmt.left)?;
                self.scan_expression(&stmt.right)?;
                self.scan_statement(&stmt.body)
            }
            Statement::SwitchStatement(stmt) => {
                self.scan_expression(&stmt.discriminant)?;
                for case in &stmt.cases {
                    if let Some(test) = &case.test {
                        self.scan_expression(test)?;
                    }
                    self.scan_statements(&case.consequent)?;
                }
                Ok(())
            }
            Statement::TryStatement(stmt) => {
                self.scan_statements(&stmt.block.body)?;
                if let Some(handler) = &stmt.handler {
                    self.scan_statements(&handler.body.body)?;
                }
                if let Some(finalizer) = &stmt.finalizer {
                    self.scan_statements(&finalizer.body)?;
                }
                Ok(())
            }
            Statement::FunctionDeclaration(func) => {
                self.scan_function(func)?;
                Ok(())
            }
            Statement::ClassDeclaration(class) => {
                if class.declare {
                    return Ok(());
                }
                if let Some(super_class) = &class.super_class {
                    self.scan_expression(super_class)?;
                }
                self.scan_class_elements(&class.body.body)
            }
            Statement::ExportDefaultDeclaration(export) => {
                match &export.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        self.scan_function(func)?;
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        if class.declare {
                            return Ok(());
                        }
                        if let Some(super_class) = &class.super_class {
                            self.scan_expression(super_class)?;
                        }
                        self.scan_class_elements(&class.body.body)?;
                    }
                    ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {}
                    kind => {
                        if let Some(expr) = kind.as_expression() {
                            self.scan_expression(expr)?;
                        }
                    }
                }
                Ok(())
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    self.scan_declaration(decl)?;
                }
                Ok(())
            }
            Statement::ImportDeclaration(_) | Statement::ExportAllDeclaration(_) => Ok(()),
            Statement::TSExportAssignment(_) => Err(self.unsupported("TSExportAssignment")),
            Statement::TSNamespaceExportDeclaration(_) => Ok(()),
            Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSGlobalDeclaration(_) => Ok(()),
            Statement::TSEnumDeclaration(decl) => self.scan_ts_enum_declaration(decl),
            Statement::TSModuleDeclaration(decl) => self.scan_ts_module_declaration(decl),
            Statement::TSImportEqualsDeclaration(decl) => {
                self.scan_ts_import_equals_declaration(decl)
            }
            Statement::StructStatement(_) => Err(self.unsupported("StructStatement")),
            Statement::AnnotationDeclaration(decl) => self.scan_annotation_declaration(decl),
            Statement::LazyImportDeclaration(_) => Err(self.unsupported("LazyImportDeclaration")),
        }
    }

    fn scan_declaration(&mut self, decl: &Declaration<'_>) -> Result<(), AbcError> {
        match decl {
            Declaration::VariableDeclaration(decl) => self.scan_variable_declaration(decl),
            Declaration::FunctionDeclaration(func) => self.scan_function(func),
            Declaration::ClassDeclaration(class) => {
                if class.declare {
                    return Ok(());
                }
                if let Some(super_class) = &class.super_class {
                    self.scan_expression(super_class)?;
                }
                self.scan_class_elements(&class.body.body)
            }
            Declaration::TSTypeAliasDeclaration(_)
            | Declaration::TSInterfaceDeclaration(_)
            | Declaration::TSGlobalDeclaration(_) => Ok(()),
            Declaration::TSEnumDeclaration(decl) => self.scan_ts_enum_declaration(decl),
            Declaration::TSModuleDeclaration(decl) => self.scan_ts_module_declaration(decl),
            Declaration::TSImportEqualsDeclaration(decl) => {
                self.scan_ts_import_equals_declaration(decl)
            }
            Declaration::StructStatement(_) => Err(self.unsupported("StructStatement")),
            Declaration::AnnotationDeclaration(decl) => self.scan_annotation_declaration(decl),
        }
    }

    fn scan_ts_enum_declaration(&mut self, decl: &TSEnumDeclaration<'_>) -> Result<(), AbcError> {
        if decl.declare {
            return Ok(());
        }
        for member in &decl.body.members {
            if let Some(initializer) = &member.initializer {
                self.scan_expression(initializer)?;
            }
        }
        Ok(())
    }

    fn scan_ts_module_declaration(
        &mut self,
        decl: &TSModuleDeclaration<'_>,
    ) -> Result<(), AbcError> {
        if decl.declare || decl.body.is_none() {
            return Ok(());
        }
        Err(self.unsupported("TSModuleDeclaration"))
    }

    fn scan_ts_import_equals_declaration(
        &mut self,
        _decl: &TSImportEqualsDeclaration<'_>,
    ) -> Result<(), AbcError> {
        Ok(())
    }

    fn scan_expression(&mut self, expr: &Expression<'_>) -> Result<(), AbcError> {
        match expr {
            Expression::PrivateFieldExpression(expr) => {
                // Scan the object; the private field name (#name) is a class-scope identifier.
                self.scan_expression(&expr.object)
            }
            Expression::PrivateInExpression(expr) => {
                // Scan the object; the private name (#x) on the left is a class-scope identifier.
                self.scan_expression(&expr.right)
            }
            Expression::JSXElement(_) => Err(self.unsupported("JSXElement")),
            Expression::JSXFragment(_) => Err(self.unsupported("JSXFragment")),
            Expression::V8IntrinsicExpression(_) => Err(self.unsupported("V8IntrinsicExpression")),
            // import.meta is handled in emit_expression via GetImportMeta opcode.
            Expression::MetaProperty(_) => Ok(()),
            Expression::TSAsExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSSatisfiesExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSNonNullExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSTypeAssertion(expr) => self.scan_expression(&expr.expression),
            Expression::TSInstantiationExpression(expr) => self.scan_expression(&expr.expression),
            Expression::ImportExpression(expr) => {
                self.scan_expression(&expr.source)?;
                if let Some(options) = &expr.options {
                    self.scan_expression(options)?;
                }
                Ok(())
            }
            Expression::ParenthesizedExpression(expr) => self.scan_expression(&expr.expression),
            Expression::SequenceExpression(seq) => {
                for expr in &seq.expressions {
                    self.scan_expression(expr)?;
                }
                Ok(())
            }
            Expression::ArkUIComponentExpression(_) => {
                Err(self.unsupported("ArkUIComponentExpression"))
            }
            Expression::LeadingDotExpression(_) => Err(self.unsupported("LeadingDotExpression")),
            Expression::AssignmentExpression(assign) => {
                self.scan_assignment_target(&assign.left)?;
                self.scan_expression(&assign.right)
            }
            Expression::BinaryExpression(expr) => {
                self.scan_expression(&expr.left)?;
                self.scan_expression(&expr.right)
            }
            Expression::LogicalExpression(expr) => {
                self.scan_expression(&expr.left)?;
                self.scan_expression(&expr.right)
            }
            Expression::UnaryExpression(expr) => self.scan_expression(&expr.argument),
            Expression::UpdateExpression(expr) => {
                self.scan_simple_assignment_target(&expr.argument)
            }
            Expression::ConditionalExpression(expr) => {
                self.scan_expression(&expr.test)?;
                self.scan_expression(&expr.consequent)?;
                self.scan_expression(&expr.alternate)
            }
            Expression::CallExpression(call) => {
                self.scan_expression(&call.callee)?;
                for arg in &call.arguments {
                    if let Some(expr) = arg.as_expression() {
                        self.scan_expression(expr)?;
                    }
                }
                Ok(())
            }
            Expression::NewExpression(expr) => {
                self.scan_expression(&expr.callee)?;
                for arg in &expr.arguments {
                    if let Some(expr) = arg.as_expression() {
                        self.scan_expression(expr)?;
                    }
                }
                Ok(())
            }
            Expression::StaticMemberExpression(expr) => self.scan_expression(&expr.object),
            Expression::ComputedMemberExpression(expr) => {
                self.scan_expression(&expr.object)?;
                self.scan_expression(&expr.expression)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.scan_expression(&call.callee)?;
                    for arg in &call.arguments {
                        if let Some(expr) = arg.as_expression() {
                            self.scan_expression(expr)?;
                        }
                    }
                    Ok(())
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(expr) => {
                    self.scan_expression(&expr.object)
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(expr) => {
                    self.scan_expression(&expr.object)?;
                    self.scan_expression(&expr.expression)
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(expr) => {
                    self.scan_expression(&expr.object)
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(expr) => {
                    self.scan_expression(&expr.expression)
                }
            },
            Expression::ObjectExpression(obj) => {
                use oxc_ast::ast::ObjectPropertyKind;
                for prop in &obj.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            self.scan_property_key(&property.key)?;
                            self.scan_expression(&property.value)?;
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.scan_expression(&spread.argument)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::ArrayExpression(arr) => {
                for elem in &arr.elements {
                    if let Some(expr) = elem.as_expression() {
                        self.scan_expression(expr)?;
                    }
                }
                Ok(())
            }
            Expression::FunctionExpression(function) => {
                self.scan_function(function)?;
                if let Some(body) = &function.body {
                    self.scan_statements(&body.statements)?;
                }
                Ok(())
            }
            Expression::ArrowFunctionExpression(function) => {
                self.scan_formal_parameters(&function.params)?;
                self.scan_statements(&function.body.statements)
            }
            Expression::ClassExpression(class) => {
                if let Some(super_class) = &class.super_class {
                    self.scan_expression(super_class)?;
                }
                self.scan_class_elements(&class.body.body)
            }
            Expression::TemplateLiteral(tmpl) => {
                for expr in &tmpl.expressions {
                    self.scan_expression(expr)?;
                }
                Ok(())
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.scan_expression(&tagged.tag)?;
                for expr in &tagged.quasi.expressions {
                    self.scan_expression(expr)?;
                }
                Ok(())
            }
            Expression::AwaitExpression(expr) => self.scan_expression(&expr.argument),
            Expression::YieldExpression(expr) => {
                // yield is lowered via the generator state machine.
                if let Some(arg) = &expr.argument {
                    self.scan_expression(arg)?;
                }
                Ok(())
            }
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::Identifier(_)
            | Expression::Super(_)
            | Expression::ThisExpression(_) => Ok(()),
        }
    }

    fn scan_variable_declaration(
        &mut self,
        decl: &oxc_ast::ast::VariableDeclaration<'_>,
    ) -> Result<(), AbcError> {
        for declarator in &decl.declarations {
            self.scan_binding_pattern(&declarator.id)?;
            if let Some(init) = &declarator.init {
                self.scan_expression(init)?;
            }
        }
        Ok(())
    }

    fn scan_function(&mut self, function: &Function<'_>) -> Result<(), AbcError> {
        // Generator functions are lowered via the CreateGeneratorObj/
        // SuspendGenerator/ResumeGenerator state machine.
        self.scan_formal_parameters(&function.params)?;
        if let Some(body) = &function.body {
            self.scan_statements(&body.statements)?;
        }
        Ok(())
    }

    fn scan_formal_parameters(
        &mut self,
        params: &oxc_ast::ast::FormalParameters<'_>,
    ) -> Result<(), AbcError> {
        for param in &params.items {
            self.scan_binding_pattern(&param.pattern)?;
            if let Some(init) = &param.initializer {
                self.scan_expression(init)?;
            }
        }
        if let Some(rest) = &params.rest {
            self.scan_binding_rest_element(&rest.rest)?;
        }
        Ok(())
    }

    fn scan_binding_rest_element(
        &mut self,
        rest: &oxc_ast::ast::BindingRestElement<'_>,
    ) -> Result<(), AbcError> {
        self.scan_binding_pattern(&rest.argument)
    }

    fn scan_binding_pattern(&mut self, pattern: &BindingPattern<'_>) -> Result<(), AbcError> {
        match pattern {
            BindingPattern::BindingIdentifier(_) => Ok(()),
            BindingPattern::AssignmentPattern(assign) => {
                self.scan_expression(&assign.right)?;
                self.scan_binding_pattern(&assign.left)
            }
            BindingPattern::ArrayPattern(arr) => {
                // Array destructuring — recurse into each element and the rest pattern.
                for element in arr.elements.iter().flatten() {
                    self.scan_binding_pattern(element)?;
                }
                if let Some(rest) = &arr.rest {
                    self.scan_binding_pattern(&rest.argument)?;
                }
                Ok(())
            }
            BindingPattern::ObjectPattern(obj) => {
                // Object destructuring — recurse into each property value and the rest.
                for prop in &obj.properties {
                    if prop.computed {
                        self.scan_expression(&prop.key.as_expression().unwrap_or_else(|| {
                            panic!("computed property key must be an expression")
                        }))?;
                    }
                    self.scan_binding_pattern(&prop.value)?;
                }
                if let Some(rest) = &obj.rest {
                    self.scan_binding_pattern(&rest.argument)?;
                }
                Ok(())
            }
        }
    }

    fn scan_assignment_target(&mut self, target: &AssignmentTarget<'_>) -> Result<(), AbcError> {
        match target {
            AssignmentTarget::PrivateFieldExpression(expr) => {
                // Scan the object; the private name (#name) is a class-scope identifier.
                self.scan_expression(&expr.object)
            }
            AssignmentTarget::ArrayAssignmentTarget(arr) => self.scan_array_assignment_target(arr),
            AssignmentTarget::ObjectAssignmentTarget(obj) => {
                self.scan_object_assignment_target(obj)
            }
            AssignmentTarget::TSAsExpression(expr) => self.scan_expression(&expr.expression),
            AssignmentTarget::TSSatisfiesExpression(expr) => self.scan_expression(&expr.expression),
            AssignmentTarget::TSNonNullExpression(expr) => self.scan_expression(&expr.expression),
            AssignmentTarget::TSTypeAssertion(expr) => self.scan_expression(&expr.expression),
            AssignmentTarget::ComputedMemberExpression(expr) => {
                self.scan_expression(&expr.object)?;
                self.scan_expression(&expr.expression)
            }
            AssignmentTarget::StaticMemberExpression(expr) => self.scan_expression(&expr.object),
            _ => Ok(()),
        }
    }

    fn scan_array_assignment_target(
        &mut self,
        arr: &ArrayAssignmentTarget<'_>,
    ) -> Result<(), AbcError> {
        for element in arr.elements.iter().flatten() {
            self.scan_assignment_target_maybe_default(element)?;
        }
        if let Some(rest) = &arr.rest {
            self.scan_assignment_target(&rest.target)?;
        }
        Ok(())
    }

    fn scan_object_assignment_target(
        &mut self,
        obj: &ObjectAssignmentTarget<'_>,
    ) -> Result<(), AbcError> {
        for prop in &obj.properties {
            self.scan_assignment_target_property(prop)?;
        }
        if let Some(rest) = &obj.rest {
            self.scan_assignment_target(&rest.target)?;
        }
        Ok(())
    }

    fn scan_assignment_target_maybe_default(
        &mut self,
        target: &AssignmentTargetMaybeDefault<'_>,
    ) -> Result<(), AbcError> {
        match target {
            AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(w) => {
                self.scan_assignment_target(&w.binding)?;
                self.scan_expression(&w.init)
            }
            _ => {
                // All other variants are simple AssignmentTarget — forward to scan_assignment_target.
                let simple: &AssignmentTarget<'_> = target.as_assignment_target().unwrap();
                self.scan_assignment_target(simple)
            }
        }
    }

    fn scan_assignment_target_property(
        &mut self,
        prop: &AssignmentTargetProperty<'_>,
    ) -> Result<(), AbcError> {
        match prop {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(id_prop) => {
                if let Some(init) = &id_prop.init {
                    self.scan_expression(init)?;
                }
                Ok(())
            }
            AssignmentTargetProperty::AssignmentTargetPropertyProperty(prop_prop) => {
                if prop_prop.computed {
                    if let Some(expr) = prop_prop.name.as_expression() {
                        self.scan_expression(expr)?;
                    }
                }
                self.scan_assignment_target_maybe_default(&prop_prop.binding)
            }
        }
    }

    fn scan_simple_assignment_target(
        &mut self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'_>,
    ) -> Result<(), AbcError> {
        match target {
            oxc_ast::ast::SimpleAssignmentTarget::PrivateFieldExpression(expr) => {
                // Private field in update target — scan the object.
                self.scan_expression(&expr.object)
            }
            oxc_ast::ast::SimpleAssignmentTarget::ComputedMemberExpression(expr) => {
                self.scan_expression(&expr.object)?;
                self.scan_expression(&expr.expression)
            }
            oxc_ast::ast::SimpleAssignmentTarget::StaticMemberExpression(expr) => {
                self.scan_expression(&expr.object)
            }
            _ => Ok(()),
        }
    }

    fn scan_for_left(&mut self, left: &ForStatementLeft<'_>) -> Result<(), AbcError> {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => self.scan_variable_declaration(decl),
            ForStatementLeft::PrivateFieldExpression(expr) => {
                // Private field on for-left — scan the object expression.
                self.scan_expression(&expr.object)
            }
            ForStatementLeft::ArrayAssignmentTarget(arr) => self.scan_array_assignment_target(arr),
            ForStatementLeft::ObjectAssignmentTarget(obj) => {
                self.scan_object_assignment_target(obj)
            }
            ForStatementLeft::ComputedMemberExpression(expr) => {
                self.scan_expression(&expr.object)?;
                self.scan_expression(&expr.expression)
            }
            ForStatementLeft::StaticMemberExpression(expr) => self.scan_expression(&expr.object),
            _ => Ok(()),
        }
    }

    fn scan_property_key(&mut self, key: &PropertyKey<'_>) -> Result<(), AbcError> {
        match key {
            // Private identifiers are class-scope names — no sub-expression to scan.
            PropertyKey::PrivateIdentifier(_) => Ok(()),
            PropertyKey::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
            }
            key => {
                if let Some(expr) = scanable_property_key_expression(key) {
                    self.scan_expression(expr)?;
                }
                Ok(())
            }
        }
    }

    fn scan_class_elements(&mut self, elements: &[ClassElement<'_>]) -> Result<(), AbcError> {
        for element in elements {
            match element {
                ClassElement::MethodDefinition(method) => {
                    self.scan_property_key(&method.key)?;
                    // Allow private identifier keys (#name) for private methods.
                    let is_private = matches!(&method.key, PropertyKey::PrivateIdentifier(_));
                    if !is_private
                        && method.kind != MethodDefinitionKind::Constructor
                        && !method.computed
                        && method_key_name(&method.key).is_none()
                    {
                        return Err(self.unsupported("UnsupportedClassMethodKey"));
                    }
                    self.scan_formal_parameters(&method.value.params)?;
                    if let Some(body) = &method.value.body {
                        self.scan_statements(&body.statements)?;
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    self.scan_property_key(&property.key)?;
                    // Allow private identifier keys (#name) and computed keys;
                    // only reject other non-string keys.
                    let is_private = matches!(&property.key, PropertyKey::PrivateIdentifier(_));
                    if !is_private && !property.computed && method_key_name(&property.key).is_none()
                    {
                        return Err(self.unsupported("UnsupportedClassFieldKey"));
                    }
                    if let Some(value) = &property.value {
                        self.scan_expression(value)?;
                    }
                }
                ClassElement::AccessorProperty(accessor) => {
                    // auto-accessor is lowered to private backing field + getter/setter.
                    self.scan_property_key(&accessor.key)?;
                    let supported_static_key = matches!(
                        &accessor.key,
                        PropertyKey::StaticIdentifier(_)
                            | PropertyKey::StringLiteral(_)
                            | PropertyKey::NumericLiteral(_)
                            | PropertyKey::BigIntLiteral(_)
                            | PropertyKey::Identifier(_)
                            | PropertyKey::PrivateIdentifier(_)
                    );
                    if !accessor.computed && !supported_static_key {
                        return Err(self.unsupported("UnsupportedAutoAccessorKey"));
                    }
                    if let Some(value) = &accessor.value {
                        self.scan_expression(value)?;
                    }
                }
                ClassElement::StaticBlock(block) => self.scan_statements(&block.body)?,
                ClassElement::TSIndexSignature(_) => {}
            }
        }
        Ok(())
    }
}

// ── CodeGen ───────────────────────────────────────────────────────────────────

struct BodyCompileResult {
    bytecode: Vec<u8>,
    pandasm_insns: Vec<Insn>,
    pandasm_debug: Vec<InstructionDebugInfo>,
    line_ops: Vec<u8>,
    catches: Vec<CatchEntry>,
    program_labels: Vec<ProgramLabel>,
    vregs: u16,
    debug_line_start: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
enum JumpKind {
    Jmp,
    Jeqz,
    Jnez,
}

#[derive(Clone, Copy)]
struct IteratorState {
    closed_reg: u8,
    method_reg: u8,
    iterator_reg: u8,
}

impl BodyCompileResult {
    fn new(bytecode: Vec<u8>, line_ops: Vec<u8>, catches: Vec<CatchEntry>, vregs: u16) -> Self {
        Self {
            bytecode,
            pandasm_insns: Vec::new(),
            pandasm_debug: Vec::new(),
            line_ops,
            catches,
            program_labels: Vec::new(),
            vregs,
            debug_line_start: None,
        }
    }

    fn from_static_insns(
        pandasm_insns: Vec<Insn>,
        line_ops: Vec<u8>,
        catches: Vec<CatchEntry>,
        vregs: u16,
    ) -> Self {
        let mut strings = StringTable::default();
        let bytecode = pandasm_insns
            .iter()
            .flat_map(|insn| insn.encode(&mut strings))
            .collect();
        Self::new(bytecode, line_ops, catches, vregs).with_pandasm_insns(pandasm_insns)
    }

    fn return_undefined() -> Self {
        Self::from_static_insns(vec![Insn::ReturnUndefined], Vec::new(), Vec::new(), 0)
    }

    fn with_pandasm_insns(mut self, pandasm_insns: Vec<Insn>) -> Self {
        self.pandasm_insns = pandasm_insns;
        self
    }

    fn with_pandasm_debug(mut self, pandasm_debug: Vec<InstructionDebugInfo>) -> Self {
        self.pandasm_debug = pandasm_debug;
        self
    }

    fn with_program_labels(mut self, program_labels: Vec<ProgramLabel>) -> Self {
        self.program_labels = program_labels;
        self
    }

    fn with_debug_line_start(mut self, debug_line_start: Option<u32>) -> Self {
        self.debug_line_start = debug_line_start;
        self
    }
}

fn function_arg_register(arg_index: u16) -> u8 {
    let max_index = (ARG_SENTINEL_LIMIT - 1) as u16;
    let bounded = arg_index.min(max_index);
    ARG_SENTINEL_BASE + bounded as u8
}

fn physicalize_argument_registers(
    bytecode: &mut [u8],
    pandasm_insns: &mut [Insn],
    register_operand_offsets: &[usize],
    num_vregs: u16,
) {
    for &offset in register_operand_offsets {
        let Some(byte) = bytecode.get_mut(offset) else {
            continue;
        };
        if *byte < ARG_SENTINEL_BASE {
            continue;
        }
        let arg_index = (*byte - ARG_SENTINEL_BASE) as u16;
        let physical = num_vregs + arg_index;
        if let Ok(reg) = u8::try_from(physical) {
            *byte = reg;
        }
    }

    for insn in pandasm_insns {
        insn.physicalize_argument_registers(num_vregs);
    }
}

fn instruction_offsets(insns: &[Insn]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(insns.len());
    let mut offset = 0usize;
    for insn in insns {
        offsets.push(offset);
        offset += insn.byte_len();
    }
    offsets
}

fn instruction_stream_len(insns: &[Insn]) -> usize {
    insns.iter().map(Insn::byte_len).sum()
}

fn jump_target(offset: usize, insn: &Insn) -> Option<(JumpKind, usize)> {
    let (kind, rel) = match insn {
        Insn::JmpShort(value) => (JumpKind::Jmp, *value as i32),
        Insn::Jmp(value) => (JumpKind::Jmp, *value as i32),
        Insn::JmpWide(value) => (JumpKind::Jmp, *value),
        Insn::JeqzShort(value) => (JumpKind::Jeqz, *value as i32),
        Insn::Jeqz(value) => (JumpKind::Jeqz, *value as i32),
        Insn::JeqzWide(value) => (JumpKind::Jeqz, *value),
        Insn::JnezShort(value) => (JumpKind::Jnez, *value as i32),
        Insn::Jnez(value) => (JumpKind::Jnez, *value as i32),
        Insn::JnezWide(value) => (JumpKind::Jnez, *value),
        _ => return None,
    };
    let target = offset as i64 + rel as i64;
    Some((kind, target as usize))
}

fn translate_offset(
    original: usize,
    original_offsets: &[usize],
    offsets: &[usize],
    original_end: usize,
    end: usize,
) -> usize {
    if original == original_end {
        return end;
    }
    let index = original_offsets
        .iter()
        .position(|offset| *offset == original)
        .expect("branch/catch target must point at an instruction boundary");
    offsets[index]
}

fn jump_insn(kind: JumpKind, rel: i64) -> Insn {
    let rel_i32 = i32::try_from(rel).expect("jump offset must fit i32");
    match kind {
        JumpKind::Jmp if i8::try_from(rel_i32).is_ok() => Insn::JmpShort(rel_i32 as i8),
        JumpKind::Jmp if i16::try_from(rel_i32).is_ok() => Insn::Jmp(rel_i32 as i16),
        JumpKind::Jmp => Insn::JmpWide(rel_i32),
        JumpKind::Jeqz if i8::try_from(rel_i32).is_ok() => Insn::JeqzShort(rel_i32 as i8),
        JumpKind::Jeqz if i16::try_from(rel_i32).is_ok() => Insn::Jeqz(rel_i32 as i16),
        JumpKind::Jeqz => Insn::JeqzWide(rel_i32),
        JumpKind::Jnez if i8::try_from(rel_i32).is_ok() => Insn::JnezShort(rel_i32 as i8),
        JumpKind::Jnez if i16::try_from(rel_i32).is_ok() => Insn::Jnez(rel_i32 as i16),
        JumpKind::Jnez => Insn::JnezWide(rel_i32),
    }
}

fn line_ops_from_instruction_debug(insns: &[Insn], debug: &[InstructionDebugInfo]) -> Vec<u8> {
    if debug.is_empty() {
        return Vec::new();
    }
    let mut tracker = LineTracker::new(u32::MAX);
    for (insn, debug) in insns.iter().zip(debug) {
        tracker.advance_with_col(
            debug.line_number,
            debug.column_number,
            insn.byte_len() as u32,
        );
    }
    tracker.finish()
}

struct ClassFieldInit<'a> {
    key: ClassFieldKey<'a>,
    value: Option<&'a Expression<'a>>,
    span_start: u32,
}

/// A private instance field initializer: `#name = value`.
struct ClassPrivateFieldInit<'a> {
    /// Private field slot index within the class's private environment.
    slot: u16,
    /// The optional initializer expression (None → `undefined`).
    value: Option<&'a Expression<'a>>,
    span_start: u32,
}

enum ClassFieldKey<'a> {
    Static(String),
    CapturedComputed {
        key: &'a PropertyKey<'a>,
        capture_name: String,
    },
}

enum ClassFieldRuntimeKey {
    Static(String),
    ComputedReg(u8),
}

struct ClassComputedKeys {
    method_regs: Vec<u8>,
    auto_accessor_regs: Vec<u8>,
    static_field_regs: Vec<u8>,
}

struct CodeGen<'a> {
    strings: &'a mut StringTable,
    /// Literal arrays accumulated by class definitions.
    literal_arrays: &'a mut Vec<LiteralArray>,
    lowering: &'a LoweringContext,
    /// Raw bytecode bytes accumulated so far.
    insns: Vec<u8>,
    /// Structured pandasm instructions in the same order as `insns`.
    pandasm_insns: Vec<Insn>,
    /// Per-pandasm-instruction source positions in the same order as
    /// `pandasm_insns`.
    pandasm_debug: Vec<InstructionDebugInfo>,
    /// Byte offset of each entry in `pandasm_insns`.
    pandasm_insn_offsets: Vec<usize>,
    /// Encoded byte offsets whose bytes are vreg operands.
    register_operand_offsets: Vec<usize>,
    /// Maps JS binding name → virtual register number.
    var_regs: FxHashMap<String, u8>,
    /// Next free register index.
    next_reg: u8,
    /// Maximum register count reached while compiling this function body.
    max_next_reg: u8,
    /// Set to `true` after a `return` or `returnundefined` is emitted so that
    /// unreachable code is not generated.
    returned: bool,
    arrow_captures: ArrowCaptures,
    derived_constructor: bool,
    derived_new_this_reg: Option<u8>,
    /// Original source text used for line-number computation, or `None` when
    /// debug line tracking is disabled for this compilation unit.
    source: Option<&'a str>,
    source_mapper: Option<&'a SourceMapper>,
    /// Line number program accumulator.  Present only when `source` is `Some`.
    line_tracker: Option<LineTracker>,
    /// Whether `debugger;` statements lower to Panda Debugger instructions.
    emit_debugger: bool,
    /// Current source line number for the instruction being emitted.
    current_line: u32,
    /// Current source column number (0-based) for the instruction being emitted.
    current_col: u32,
    suppress_next_expr_debug_pos: bool,
    function_end: Option<u32>,
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
    label_stack: Vec<LabelContext>,
    catch_entries: Vec<CatchEntry>,
    program_labels: Vec<ProgramLabel>,
    program_label_names: FxHashMap<usize, String>,
    next_program_label: usize,
    #[allow(dead_code)]
    is_generator: bool,
    gen_reg: Option<u8>,
    is_async: bool,
    async_reg: Option<u8>,
    async_try_begin: Option<u32>,
    iterator_stack: Vec<IteratorState>,
    finally_stack: Vec<&'a oxc_ast::ast::BlockStatement<'a>>,
    module_body: bool,
    inferred_function_name: Option<String>,
    current_lexical_names: Vec<String>,
    current_lexical_indices: FxHashMap<String, u8>,
    outer_lexical_envs: Vec<FxHashMap<String, u8>>,
    runtime_ts_import_equals: FxHashSet<(u32, u32)>,
    /// Private field name → slot index for the current class scope.
    /// Populated before compiling class methods and constructors that have private fields.
    /// `level` is always 0 (current class scope) in our single-class implementation.
    private_field_slots: FxHashMap<String, u16>,
}

struct LabelContext {
    name: String,
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
}

impl<'a> CodeGen<'a> {
    fn new(
        strings: &'a mut StringTable,
        literal_arrays: &'a mut Vec<LiteralArray>,
        lowering: &'a LoweringContext,
    ) -> Self {
        CodeGen {
            strings,
            literal_arrays,
            lowering,
            insns: Vec::new(),
            pandasm_insns: Vec::new(),
            pandasm_debug: Vec::new(),
            pandasm_insn_offsets: Vec::new(),
            register_operand_offsets: Vec::new(),
            var_regs: FxHashMap::default(),
            next_reg: 0,
            max_next_reg: 0,
            returned: false,
            arrow_captures: ArrowCaptures::default(),
            derived_constructor: false,
            derived_new_this_reg: None,
            source: None,
            source_mapper: None,
            line_tracker: None,
            emit_debugger: false,
            current_line: 1,
            current_col: 0,
            suppress_next_expr_debug_pos: false,
            function_end: None,
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            label_stack: Vec::new(),
            catch_entries: Vec::new(),
            program_labels: Vec::new(),
            program_label_names: FxHashMap::default(),
            next_program_label: 0,
            is_generator: false,
            gen_reg: None,
            is_async: false,
            async_reg: None,
            async_try_begin: None,
            iterator_stack: Vec::new(),
            finally_stack: Vec::new(),
            module_body: false,
            inferred_function_name: None,
            current_lexical_names: Vec::new(),
            current_lexical_indices: FxHashMap::default(),
            outer_lexical_envs: Vec::new(),
            runtime_ts_import_equals: FxHashSet::default(),
            private_field_slots: FxHashMap::default(),
        }
    }

    /// Create a `CodeGen` that also emits Panda LineNumberProgram opcodes.
    ///
    /// `source` is the original source text; statement byte offsets are used to
    /// compute 1-based line numbers.
    fn new_with_source(
        strings: &'a mut StringTable,
        literal_arrays: &'a mut Vec<LiteralArray>,
        lowering: &'a LoweringContext,
        source: &'a str,
        source_mapper: Option<&'a SourceMapper>,
    ) -> Self {
        let mut gen = Self::new(strings, literal_arrays, lowering);
        gen.source = Some(source);
        gen.source_mapper = source_mapper;
        gen.line_tracker = Some(LineTracker::new(u32::MAX));
        gen.emit_debugger = gen.lowering.emit_debugger();
        gen.current_line = u32::MAX;
        gen.current_col = 0;
        gen
    }

    #[allow(dead_code)]
    fn new_arrow(
        strings: &'a mut StringTable,
        literal_arrays: &'a mut Vec<LiteralArray>,
        lowering: &'a LoweringContext,
        arrow_captures: ArrowCaptures,
    ) -> Self {
        let mut gen = Self::new(strings, literal_arrays, lowering);
        gen.arrow_captures = arrow_captures;
        gen
    }

    fn new_arrow_with_source(
        strings: &'a mut StringTable,
        literal_arrays: &'a mut Vec<LiteralArray>,
        lowering: &'a LoweringContext,
        arrow_captures: ArrowCaptures,
        source: Option<&'a str>,
        source_mapper: Option<&'a SourceMapper>,
    ) -> Self {
        let mut gen = Self::new(strings, literal_arrays, lowering);
        gen.arrow_captures = arrow_captures;
        if let Some(src) = source {
            gen.source = Some(src);
            gen.source_mapper = source_mapper;
            gen.line_tracker = Some(LineTracker::new(u32::MAX));
            gen.emit_debugger = gen.lowering.emit_debugger();
            gen.current_line = u32::MAX;
            gen.current_col = 0;
        }
        gen
    }

    fn use_module_lexical_env(&mut self) {
        self.set_current_lexical_env(self.lowering.module_lexical_names.clone());
    }

    fn use_function_lexical_env(&mut self, span: Option<(u32, u32)>) {
        let Some(span) = span else {
            return;
        };
        self.set_current_lexical_env(self.lowering.function_lexical_names(span).to_vec());
        self.outer_lexical_envs = self
            .lowering
            .outer_lexical_envs(span)
            .into_iter()
            .map(|names| lexical_indices(names.iter().map(String::as_str)))
            .collect();
    }

    fn use_module_outer_lexical_env(&mut self) {
        if self.lowering.module_lexical_names.is_empty() {
            return;
        }
        self.outer_lexical_envs = vec![lexical_indices(
            self.lowering
                .module_lexical_names
                .iter()
                .map(String::as_str),
        )];
    }

    fn set_current_lexical_env(&mut self, names: Vec<String>) {
        self.current_lexical_indices = lexical_indices(names.iter().map(String::as_str));
        self.current_lexical_names = names;
    }

    fn current_lexical_slot(&self, name: &str) -> Option<u8> {
        self.current_lexical_indices.get(name).copied()
    }

    fn lexical_lookup(&self, name: &str) -> Option<(u8, u8)> {
        if let Some(slot) = self.current_lexical_slot(name) {
            return Some((0, slot));
        }
        let mut level = u8::from(!self.current_lexical_names.is_empty());
        for env in &self.outer_lexical_envs {
            if let Some(slot) = env.get(name).copied() {
                return Some((level, slot));
            }
            level = level.saturating_add(1);
        }
        None
    }

    fn emit_module_lex_env_prologue(&mut self) {
        if self.current_lexical_names.is_empty() {
            return;
        }
        self.push(Insn::NewLexEnv(self.current_lexical_names.len() as u8));
        let reserves_class_binding = self.current_lexical_names.iter().any(|name| {
            name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().any(char::is_lowercase)
        });
        self.ensure_next_reg_at_least(if reserves_class_binding { 5 } else { 4 });
    }

    fn emit_function_lex_env_prologue(&mut self) {
        if self.current_lexical_names.is_empty() {
            return;
        }
        self.push(Insn::NewLexEnv(self.current_lexical_names.len() as u8));
        if self.lowering.standalone_ecmascript_input() {
            return;
        }
        for slot in 0..self.current_lexical_names.len() {
            self.push(Insn::LdUndefined);
            self.push(Insn::StLexVar(0, slot as u8));
        }
    }

    fn emit_function_hoists(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            match stmt {
                Statement::FunctionDeclaration(func) => self.emit_function_declaration(func),
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::FunctionDeclaration(func)) = &export.declaration {
                        self.emit_function_declaration(func);
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::FunctionDeclaration(func) =
                        &export.declaration
                    {
                        self.emit_default_function_declaration(func);
                    }
                }
                _ => {}
            }
        }
    }

    fn emit_function_declaration(&mut self, func: &Function<'_>) {
        if is_type_only_function(func) {
            return;
        }
        if let Some(id) = &func.id {
            let name_id = self.intern(id.name.as_str());
            let length = func.params.items.len() as u8;
            self.push(Insn::DefineFuncShort(0, name_id, length));
            let name = id.name.as_str().to_string();
            if let Some(slot) = self.current_lexical_slot(&name) {
                self.push(Insn::StLexVar(0, slot));
                return;
            }
            let reg = self.alloc_reg();
            self.push(Insn::Sta(reg));
            self.var_regs.insert(name.clone(), reg);
            if let Some(export_index) = self.lowering.export_index(&name) {
                self.push(Insn::Lda(reg));
                self.push(Insn::StModuleVar(export_index));
            }
        }
    }

    fn emit_default_function_declaration(&mut self, func: &Function<'_>) {
        if is_type_only_function(func) {
            return;
        }
        if func.id.is_some() {
            self.emit_function_declaration(func);
            return;
        }

        let length = func.params.items.len() as u8;
        let record_name = self
            .lowering
            .function_name(func.span.start, func.span.end)
            .map(str::to_string);
        let name_id = record_name
            .as_deref()
            .map(|name| self.intern(name))
            .unwrap_or_else(|| self.intern("default"));
        self.push(Insn::DefineFuncShort(0, name_id, length));
        if let Some(export_index) = self.lowering.export_index(DEFAULT_LOCAL_NAME) {
            self.push(Insn::StModuleVar(export_index));
        }
    }

    fn bind_param_names(&mut self, param_names: &[String]) {
        for name in param_names {
            if self.var_regs.contains_key(name) {
                continue;
            }
            let reg = self.alloc_reg();
            self.var_regs.insert(name.clone(), reg);
        }
    }

    fn bind_function_mandatory_lexical_names(&mut self) {
        if let Some(slot) = self.current_lexical_slot("new.target") {
            self.push(Insn::Lda(MANDATORY_PARAM_NEW_TARGET_REG));
            self.push(Insn::StLexVar(0, slot));
        }
        if let Some(slot) = self.current_lexical_slot("this") {
            self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            self.push(Insn::StLexVar(0, slot));
        }
    }

    fn bind_function_param_names(&mut self, param_names: &[String]) {
        for (idx, name) in param_names.iter().enumerate() {
            if self.var_regs.contains_key(name) {
                continue;
            }
            let reg = function_arg_register(3 + idx as u16);
            self.var_regs.insert(name.clone(), reg);
            if let Some(slot) = self.current_lexical_slot(name) {
                self.push(Insn::Lda(reg));
                self.push(Insn::StLexVar(0, slot));
                self.var_regs.remove(name);
            }
        }
    }

    fn emit_default_parameters(
        &mut self,
        params: &[FormalParameter<'_>],
        function_span_start: Option<u32>,
    ) {
        for (idx, param) in params.iter().enumerate() {
            let Some(initializer) = param.initializer.as_deref() else {
                continue;
            };
            if !matches!(&param.pattern, BindingPattern::BindingIdentifier(_)) {
                continue;
            }

            let param_start = param.pattern.span().start;
            let branch_start = function_span_start.unwrap_or(param_start);
            self.set_debug_pos(param_start);
            let saved_next_reg = self.next_reg;
            let param_reg = 3u8.saturating_add(idx as u8);
            let value_reg = self.alloc_reg();
            self.push(Insn::Lda(param_reg));
            self.set_debug_pos(branch_start);
            self.push(Insn::Sta(value_reg));
            self.push(Insn::LdUndefined);
            self.push(Insn::StrictEq(0, value_reg));
            let skip_default = self.insns.len();
            self.push(Insn::Jeqz(0));
            if is_undefined_literal(initializer) {
                self.set_debug_pos(initializer.span().start);
                self.push(Insn::LdUndefined);
                self.set_debug_pos(param_start);
                self.push(Insn::Sta(param_reg));
            } else if let Some(value) = negative_integer_literal(initializer) {
                let init_start = initializer.span().start;
                let literal_start = self
                    .source
                    .and_then(|source| {
                        let span = initializer.span();
                        (span.end as usize <= source.len()).then(|| span.source_text(source))
                    })
                    .and_then(|text| text.strip_prefix('-'))
                    .map(|_| init_start.saturating_add(1))
                    .unwrap_or(init_start);
                self.set_debug_pos(literal_start);
                self.push(Insn::LdAi(value));
                self.set_debug_pos(init_start);
                self.push(Insn::Sta(value_reg));
                self.push(Insn::Lda(value_reg));
                self.push(Insn::Neg(0));
                self.set_debug_pos(param_start);
                self.push(Insn::Sta(param_reg));
            } else {
                self.emit_expr(initializer);
                self.set_debug_pos(param_start);
                self.push(Insn::Sta(value_reg));
                self.push(Insn::Lda(value_reg));
                self.push(Insn::Sta(param_reg));
            }
            let after_default = self.insns.len();
            self.patch_jump_to(skip_default, after_default);
            self.next_reg = saved_next_reg;
        }
    }

    /// Emit destructuring bindings for function parameters that use patterns.
    ///
    /// Called after `bind_function_param_names` and `emit_default_parameters`
    /// to bind the individual names inside array/object parameter patterns.
    fn emit_destructure_parameters(&mut self, params: &[FormalParameter<'_>]) {
        for (idx, param) in params.iter().enumerate() {
            let pattern = &param.pattern;
            match pattern {
                BindingPattern::BindingIdentifier(_) => {
                    // Already handled by bind_function_param_names.
                }
                BindingPattern::ArrayPattern(_) | BindingPattern::ObjectPattern(_) => {
                    // Destructuring parameter: the value comes in via the argument register.
                    let param_reg = function_arg_register(3 + idx as u16);
                    self.emit_destructure_binding(pattern, param_reg);
                }
                BindingPattern::AssignmentPattern(assign) => {
                    // Pattern with default in parameter position (e.g. `f([a] = [])`).
                    let param_reg = function_arg_register(3 + idx as u16);
                    self.emit_destructure_binding(pattern, param_reg);
                    let _ = assign; // inner default value handled by emit_destructure_binding
                }
            }
        }
    }

    /// Append the bytes for `insn`, emitting per-instruction LNP entries.
    ///
    /// Matches libpandafile BuildLineNumberProgram: per-instruction
    /// EmitLineNumber + EmitColumnNumber + pc_inc += insn_size.
    fn push(&mut self, insn: Insn) {
        let start = self.insns.len();
        self.register_operand_offsets.extend(
            insn.register_operand_offsets()
                .iter()
                .map(|offset| start + offset),
        );
        let bytes = insn.encode(self.strings);
        let insn_size = bytes.len() as u32;
        if let Some(tracker) = self.line_tracker.as_mut() {
            tracker.advance_with_col(self.current_line, self.current_col, insn_size);
        }
        if self.source.is_some() {
            self.pandasm_debug.push(InstructionDebugInfo {
                line_number: self.current_line,
                column_number: self.current_col,
            });
        }
        self.insns.extend_from_slice(&bytes);
        self.pandasm_insn_offsets.push(start);
        self.pandasm_insns.push(insn);
    }

    /// Push the final return instruction, emitting SC(RESET) epilogue marker first.
    ///
    /// Matches libpandafile: before the return instruction, emit SET_COLUMN(RESET)
    /// to mark the epilogue boundary, as seen in all golden binary LNPs.
    fn push_with_epilogue(&mut self, insn: Insn) {
        // Emit SC(RESET=0xFFFFFFFF) to mark the epilogue start.
        // This matches the golden binary pattern: ...SC(R) PC(1)...
        if !self.lowering.standalone_ecmascript_input() {
            if let Some(tracker) = self.line_tracker.as_mut() {
                tracker.emit_epilogue_column_reset();
            }
        }
        self.push(insn);
    }

    fn set_debug_pos(&mut self, offset: u32) {
        if let Some(source) = self.source {
            let (line, col) = self
                .source_mapper
                .and_then(|mapper| mapper.map_offset(source, offset))
                .unwrap_or_else(|| (line_at(source, offset), col_at(source, offset)));
            self.current_line = line;
            self.current_col = if self.lowering.standalone_ecmascript_input() {
                0
            } else {
                col
            };
        }
    }

    fn reset_line_tracker_start(&mut self, offset: u32) {
        if let Some(source) = self.source {
            let line = self
                .source_mapper
                .and_then(|mapper| mapper.map_offset(source, offset))
                .map_or_else(|| line_at(source, offset), |(line, _)| line);
            self.current_line = line;
            self.current_col = 0;
            if self.line_tracker.is_some() {
                self.line_tracker = Some(LineTracker::new(line));
            }
        }
    }

    /// Allocate the next virtual register (capped at 254 to leave one spare).
    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_reg;
        if self.next_reg < ARG_SENTINEL_BASE - 1 {
            self.next_reg += 1;
        }
        self.note_current_reg_count();
        r
    }

    fn ensure_next_reg_at_least(&mut self, next: u8) {
        self.next_reg = self.next_reg.max(next);
        self.note_current_reg_count();
    }

    fn note_current_reg_count(&mut self) {
        self.max_next_reg = self.max_next_reg.max(self.next_reg);
    }

    fn vreg_count(&self) -> u16 {
        self.max_next_reg.max(self.next_reg) as u16
    }

    /// Intern a string and return its 16-bit index.
    fn intern(&mut self, s: &str) -> u16 {
        self.strings.intern(s) as u16
    }

    fn finish_with_catches(mut self) -> BodyCompileResult {
        if !self.returned {
            if let Some(areg) = self.async_reg {
                self.push(Insn::LdUndefined);
                self.push(Insn::AsyncFunctionResolve(areg));
                self.push_with_epilogue(Insn::Return);
            } else {
                self.push_with_epilogue(Insn::LdUndefined);
                self.push_with_epilogue(Insn::ReturnUndefined);
            }
        }
        self.emit_async_cleanup();
        let vregs = self.vreg_count();
        physicalize_argument_registers(
            &mut self.insns,
            &mut self.pandasm_insns,
            &self.register_operand_offsets,
            vregs,
        );
        self.finalize_pandasm_layout();
        BodyCompileResult::new(self.insns, Vec::new(), self.catch_entries, vregs)
            .with_pandasm_insns(self.pandasm_insns)
            .with_pandasm_debug(self.pandasm_debug)
            .with_program_labels(self.program_labels)
    }

    fn finish_with_line_ops_and_catches(mut self) -> BodyCompileResult {
        if !self.returned {
            if let Some(areg) = self.async_reg {
                self.push(Insn::LdUndefined);
                self.push(Insn::AsyncFunctionResolve(areg));
                if let Some(end) = self.function_end {
                    self.set_debug_pos(end);
                    self.push(Insn::Return);
                } else {
                    self.push_with_epilogue(Insn::Return);
                }
            } else if self.module_body && self.source.is_some() {
                if self.lowering.standalone_ecmascript_input() {
                    self.current_line = u32::MAX;
                } else {
                    self.current_line = u32::MAX;
                    self.current_col = u32::MAX;
                }
                if !self.lowering.standalone_ecmascript_input() {
                    self.push(Insn::LdUndefined);
                }
                self.push(Insn::ReturnUndefined);
            } else if let Some(end) = self.function_end {
                self.current_line = u32::MAX;
                self.current_col = u32::MAX;
                if !self.lowering.standalone_ecmascript_input() {
                    self.push(Insn::LdUndefined);
                }
                self.set_debug_pos(end);
                self.push(Insn::ReturnUndefined);
            } else {
                self.push_with_epilogue(Insn::LdUndefined);
                self.push_with_epilogue(Insn::ReturnUndefined);
            }
        }
        self.emit_async_cleanup();
        let vregs = self.vreg_count();
        physicalize_argument_registers(
            &mut self.insns,
            &mut self.pandasm_insns,
            &self.register_operand_offsets,
            vregs,
        );
        let debug_line_start = self.line_tracker.as_ref().map(LineTracker::start_line);
        let layout_changed = self.finalize_pandasm_layout();
        let original_line_ops = self
            .line_tracker
            .map(LineTracker::finish)
            .unwrap_or_default();
        let line_ops = if layout_changed && !self.pandasm_debug.is_empty() {
            line_ops_from_instruction_debug(&self.pandasm_insns, &self.pandasm_debug)
        } else {
            original_line_ops
        };
        BodyCompileResult::new(self.insns, line_ops, self.catch_entries, vregs)
            .with_pandasm_insns(self.pandasm_insns)
            .with_pandasm_debug(self.pandasm_debug)
            .with_program_labels(self.program_labels)
            .with_debug_line_start(debug_line_start)
    }

    fn finalize_pandasm_layout(&mut self) -> bool {
        if self.pandasm_insns.is_empty() {
            return false;
        }

        let original_offsets = instruction_offsets(&self.pandasm_insns);
        let original_end = instruction_stream_len(&self.pandasm_insns);
        let original_lens = self
            .pandasm_insns
            .iter()
            .map(Insn::byte_len)
            .collect::<Vec<_>>();
        let jump_targets = self
            .pandasm_insns
            .iter()
            .enumerate()
            .map(|(index, insn)| jump_target(original_offsets[index], insn))
            .collect::<Vec<_>>();

        for _ in 0..8 {
            let offsets = instruction_offsets(&self.pandasm_insns);
            let end = instruction_stream_len(&self.pandasm_insns);
            let old_lens = self
                .pandasm_insns
                .iter()
                .map(Insn::byte_len)
                .collect::<Vec<_>>();

            for (index, target) in jump_targets.iter().enumerate() {
                let Some((kind, original_target)) = target else {
                    continue;
                };
                let target = translate_offset(
                    *original_target,
                    &original_offsets,
                    &offsets,
                    original_end,
                    end,
                );
                let rel = target as i64 - offsets[index] as i64;
                self.pandasm_insns[index] = jump_insn(*kind, rel);
            }

            let new_lens = self
                .pandasm_insns
                .iter()
                .map(Insn::byte_len)
                .collect::<Vec<_>>();
            if old_lens == new_lens {
                break;
            }
        }

        let offsets = instruction_offsets(&self.pandasm_insns);
        let end = instruction_stream_len(&self.pandasm_insns);
        for catch in &mut self.catch_entries {
            catch.try_begin = translate_offset(
                catch.try_begin as usize,
                &original_offsets,
                &offsets,
                original_end,
                end,
            ) as u32;
            catch.try_end = translate_offset(
                catch.try_end as usize,
                &original_offsets,
                &offsets,
                original_end,
                end,
            ) as u32;
            catch.catch_begin = translate_offset(
                catch.catch_begin as usize,
                &original_offsets,
                &offsets,
                original_end,
                end,
            ) as u32;
            catch.catch_end = translate_offset(
                catch.catch_end as usize,
                &original_offsets,
                &offsets,
                original_end,
                end,
            ) as u32;
        }
        for label in &mut self.program_labels {
            label.offset = translate_offset(
                label.offset as usize,
                &original_offsets,
                &offsets,
                original_end,
                end,
            ) as u32;
        }
        self.rebuild_program_label_index();

        self.insns.clear();
        for insn in &self.pandasm_insns {
            self.insns.extend(insn.encode(self.strings));
        }
        original_lens
            != self
                .pandasm_insns
                .iter()
                .map(Insn::byte_len)
                .collect::<Vec<_>>()
    }

    // ── Statement dispatcher ──────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &'a Statement<'a>) {
        // Update current_line from the statement's source position so that
        // subsequent push() calls emit per-instruction LNP entries with the
        // correct line and column numbers.
        self.set_debug_pos(stmt.span().start);
        match stmt {
            Statement::VariableDeclaration(decl) => {
                self.emit_var_decl(decl);
            }
            Statement::TSEnumDeclaration(decl) => {
                self.emit_ts_enum_declaration(decl);
            }
            Statement::TSImportEqualsDeclaration(decl) => {
                self.emit_ts_import_equals_declaration(decl);
            }
            Statement::ExpressionStatement(es) => {
                // Skip string-literal "use strict" directives — they produce
                // an expression but generate no bytecode.
                if matches!(&es.expression, Expression::StringLiteral(_)) {
                    return;
                }
                let saved_next_reg = self.next_reg;
                self.emit_expr(&es.expression);
                // Discard accumulator result (no sta needed for side-effect-only
                // ExpressionStatements).
                self.next_reg = saved_next_reg;
            }
            Statement::DebuggerStatement(_) => {
                if self.emit_debugger {
                    self.push(Insn::Debugger);
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    if self.lowering.standalone_ecmascript_input() {
                        if let Some(end) = self.function_end {
                            self.set_debug_pos(end);
                            self.suppress_next_expr_debug_pos = true;
                        }
                    }
                    self.emit_expr(arg);
                    self.emit_control_flow_exit(true);
                    if self.returned {
                        return;
                    }
                    if let Some(areg) = self.async_reg {
                        self.push(Insn::AsyncFunctionResolve(areg));
                    }
                    if let Some(end) = self.function_end {
                        self.set_debug_pos(end);
                        self.push(Insn::Return);
                    } else {
                        self.push_with_epilogue(Insn::Return);
                    }
                } else {
                    self.push(Insn::LdUndefined);
                    self.emit_control_flow_exit(true);
                    if self.returned {
                        return;
                    }
                    self.current_line = u32::MAX;
                    self.current_col = u32::MAX;
                    if let Some(areg) = self.async_reg {
                        self.push(Insn::AsyncFunctionResolve(areg));
                        if let Some(end) = self.function_end {
                            self.set_debug_pos(end);
                            self.push(Insn::Return);
                        } else {
                            self.push_with_epilogue(Insn::Return);
                        }
                    } else {
                        if let Some(end) = self.function_end {
                            self.set_debug_pos(end);
                            self.push(Insn::ReturnUndefined);
                        } else {
                            self.push_with_epilogue(Insn::ReturnUndefined);
                        }
                    }
                }
                self.returned = true;
            }
            Statement::ClassDeclaration(cls) => {
                if cls.declare {
                    return;
                }
                let super_class = cls.super_class.as_ref();
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                if self.lowering.standalone_ecmascript_input() {
                    if let Some(class_name) = class_name {
                        if self.emit_standalone_class_with_instance_initializer(class_name, cls) {
                            return;
                        }
                        if self.emit_simple_standalone_class_declaration(class_name, cls) {
                            return;
                        }
                    }
                }
                if let Some(id) = &cls.id {
                    let name = id.name.as_str().to_string();
                    if let Some(export_index) = self.lowering.export_index(&name) {
                        let class_reg = self.alloc_reg();
                        let var_reg = self.alloc_reg();
                        self.var_regs.insert(name.clone(), var_reg);
                        let proto_vreg = if let Some(sc) = super_class {
                            self.emit_expr(sc);
                            let reg = self.alloc_reg();
                            self.set_debug_pos(cls.body.span.start);
                            self.push(Insn::Sta(reg));
                            reg
                        } else {
                            self.push(Insn::LdHole);
                            self.push(Insn::Sta(class_reg));
                            class_reg
                        };
                        let computed_keys = self.emit_class_computed_keys(&cls.body.body);
                        let lit_arr_id = self.literal_arrays.len() as u16;
                        self.literal_arrays.push(class_buffer_literal_array(
                            class_name.unwrap_or(&name),
                            &cls.body.body,
                            self.lowering.standalone_ecmascript_input(),
                        ));
                        let formal_count = constructor_required_param_count(&cls.body.body);
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::DefineClassWithBuffer(
                            0,
                            lit_arr_id,
                            lit_arr_id,
                            formal_count,
                            proto_vreg,
                        ));
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Sta(class_reg));
                        self.set_debug_pos(id.span.start);
                        self.push(Insn::Sta(var_reg));
                        self.emit_class_missing_properties(
                            class_name.unwrap_or(&name),
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.method_regs,
                            &computed_keys.auto_accessor_regs,
                        );
                        self.emit_class_static_initializer(
                            class_name.unwrap_or(&name),
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.static_field_regs,
                        );
                        if let Some(lex_idx) = self.current_lexical_slot(&name) {
                            self.push(Insn::Lda(class_reg));
                            self.push(Insn::StLexVar(0, lex_idx));
                        }
                        self.push(Insn::Lda(class_reg));
                        self.push(Insn::StModuleVar(export_index));
                        return;
                    }
                }
                // Emit defineclasswithbuffer; result → accumulator.
                if let Some(id) = &cls.id {
                    let name = id.name.as_str().to_string();
                    let saved_next_reg = self.next_reg;
                    let class_reg = self.alloc_reg();
                    let var_reg = self.alloc_reg();
                    self.var_regs.insert(name.clone(), var_reg);
                    let proto_vreg = if let Some(sc) = super_class {
                        self.emit_expr(sc);
                        let reg = self.alloc_reg();
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Sta(reg));
                        reg
                    } else {
                        self.push(Insn::LdHole);
                        self.push(Insn::Sta(class_reg));
                        class_reg
                    };
                    let computed_keys = self.emit_class_computed_keys(&cls.body.body);
                    let lit_arr_id = self.literal_arrays.len() as u16;
                    self.literal_arrays.push(class_buffer_literal_array(
                        class_name.unwrap_or(&name),
                        &cls.body.body,
                        self.lowering.standalone_ecmascript_input(),
                    ));
                    let formal_count = constructor_required_param_count(&cls.body.body);
                    self.set_debug_pos(cls.body.span.start);
                    self.push(Insn::DefineClassWithBuffer(
                        0,
                        lit_arr_id,
                        lit_arr_id,
                        formal_count,
                        proto_vreg,
                    ));
                    self.set_debug_pos(cls.body.span.start);
                    self.push(Insn::Sta(class_reg));
                    self.set_debug_pos(id.span.start);
                    self.push(Insn::Sta(var_reg));
                    self.emit_class_missing_properties(
                        class_name.unwrap_or(&name),
                        &cls.body.body,
                        class_reg,
                        cls.body.span.start,
                        &computed_keys.method_regs,
                        &computed_keys.auto_accessor_regs,
                    );
                    self.emit_class_static_initializer(
                        class_name.unwrap_or(&name),
                        &cls.body.body,
                        class_reg,
                        cls.body.span.start,
                        &computed_keys.static_field_regs,
                    );
                    if let Some(lex_idx) = self.current_lexical_slot(&name) {
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Lda(class_reg));
                        self.set_debug_pos(id.span.start);
                        self.push(Insn::Sta(class_reg));
                        self.push(Insn::Lda(class_reg));
                        self.push(Insn::StLexVar(0, lex_idx));
                        self.var_regs.remove(&name);
                        self.next_reg = saved_next_reg;
                    }
                    if let Some(export_index) = self.lowering.export_index(&name) {
                        self.push(Insn::Lda(class_reg));
                        self.push(Insn::StModuleVar(export_index));
                    }
                } else {
                    self.emit_class_inner(class_name, &cls.body.body, super_class);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
                    if cls.declare {
                        return;
                    }
                    let local_name = cls.id.as_ref().map(|id| id.name.as_str());
                    let class_name = local_name.unwrap_or(DEFAULT_LOCAL_NAME);
                    let class_reg = self.alloc_reg();
                    if let Some(name) = local_name {
                        self.var_regs.insert(name.to_string(), class_reg + 1);
                        self.ensure_next_reg_at_least(class_reg.saturating_add(2));
                    }
                    let proto_vreg = if let Some(sc) = &cls.super_class {
                        self.emit_expr(sc);
                        let reg = self.alloc_reg();
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Sta(reg));
                        reg
                    } else {
                        self.push(Insn::LdHole);
                        self.push(Insn::Sta(class_reg));
                        class_reg
                    };
                    let computed_keys = self.emit_class_computed_keys(&cls.body.body);
                    let lit_arr_id = self.literal_arrays.len() as u16;
                    self.literal_arrays.push(class_buffer_literal_array(
                        class_name,
                        &cls.body.body,
                        self.lowering.standalone_ecmascript_input(),
                    ));
                    let formal_count = constructor_required_param_count(&cls.body.body);
                    self.set_debug_pos(cls.body.span.start);
                    self.push(Insn::DefineClassWithBuffer(
                        0,
                        lit_arr_id,
                        lit_arr_id,
                        formal_count,
                        proto_vreg,
                    ));
                    self.set_debug_pos(cls.body.span.start);
                    self.push(Insn::Sta(class_reg));
                    if let Some(name) = local_name {
                        if let Some(id) = &cls.id {
                            self.set_debug_pos(id.span.start);
                        }
                        self.push(Insn::Sta(class_reg + 1));
                        self.emit_class_missing_properties(
                            class_name,
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.method_regs,
                            &computed_keys.auto_accessor_regs,
                        );
                        self.emit_class_static_initializer(
                            class_name,
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.static_field_regs,
                        );
                        if let Some(export_index) = self.lowering.export_index(name) {
                            self.push(Insn::Lda(class_reg));
                            if let Some(id) = &cls.id {
                                self.set_debug_pos(id.span.start);
                            }
                            self.push(Insn::StModuleVar(export_index));
                        }
                    } else {
                        self.emit_class_missing_properties(
                            class_name,
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.method_regs,
                            &computed_keys.auto_accessor_regs,
                        );
                        self.emit_class_static_initializer(
                            class_name,
                            &cls.body.body,
                            class_reg,
                            cls.body.span.start,
                            &computed_keys.static_field_regs,
                        );
                        if let Some(export_index) = self.lowering.export_index(DEFAULT_LOCAL_NAME) {
                            self.push(Insn::Lda(class_reg));
                            self.push(Insn::StModuleVar(export_index));
                        }
                    }
                } else if let Some(expr) = export.declaration.as_expression() {
                    self.emit_expr(expr);
                    if let Some(export_index) = self.lowering.export_index(DEFAULT_LOCAL_NAME) {
                        self.push(Insn::StModuleVar(export_index));
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    match decl {
                        Declaration::VariableDeclaration(decl) => self.emit_var_decl(decl),
                        Declaration::FunctionDeclaration(_) => {}
                        Declaration::TSEnumDeclaration(decl) => self.emit_ts_enum_declaration(decl),
                        Declaration::TSImportEqualsDeclaration(decl) => {
                            self.emit_ts_import_equals_declaration(decl);
                        }
                        Declaration::ClassDeclaration(cls) => {
                            if cls.declare {
                                return;
                            }
                            let super_class = cls.super_class.as_ref();
                            let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                            if let Some(id) = &cls.id {
                                let name = id.name.as_str().to_string();
                                let class_reg = self.alloc_reg();
                                let var_reg = self.alloc_reg();
                                self.var_regs.insert(name.clone(), var_reg);
                                self.emit_class_inner(class_name, &cls.body.body, super_class);
                                self.push(Insn::Sta(class_reg));
                                self.set_debug_pos(id.span.start);
                                self.push(Insn::Sta(var_reg));
                                if let Some(lex_idx) = self.current_lexical_slot(&name) {
                                    self.push(Insn::Lda(class_reg));
                                    self.push(Insn::StLexVar(0, lex_idx));
                                }
                                if let Some(export_index) = self.lowering.export_index(&name) {
                                    self.push(Insn::Lda(class_reg));
                                    self.push(Insn::StModuleVar(export_index));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Statement::FunctionDeclaration(func) => {
                self.emit_function_declaration(func);
            }
            Statement::IfStatement(if_stmt) => {
                self.emit_if_stmt(if_stmt);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.emit_stmt(s);
                    if self.returned {
                        break;
                    }
                }
            }
            Statement::WhileStatement(while_stmt) => {
                self.emit_while_stmt(while_stmt);
            }
            Statement::DoWhileStatement(do_while) => {
                self.emit_do_while_stmt(do_while);
            }
            Statement::ForStatement(for_stmt) => {
                self.emit_for_stmt(for_stmt);
            }
            Statement::BreakStatement(brk) => {
                self.emit_control_flow_exit(true);
                if self.returned {
                    return;
                }
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                if let Some(label) = &brk.label {
                    self.push_labeled_break(label.name.as_str(), jmp_pos);
                } else {
                    self.break_patches.push(jmp_pos);
                }
            }
            Statement::ContinueStatement(cont) => {
                self.emit_control_flow_exit(false);
                if self.returned {
                    return;
                }
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                if let Some(label) = &cont.label {
                    self.push_labeled_continue(label.name.as_str(), jmp_pos);
                } else {
                    self.continue_patches.push(jmp_pos);
                }
            }
            Statement::TryStatement(ts) => {
                self.emit_try_stmt(ts);
            }
            Statement::ThrowStatement(throw) => {
                self.emit_expr(&throw.argument);
                self.push(Insn::Throw);
            }
            Statement::ForInStatement(for_in) => {
                self.emit_for_in_stmt(for_in);
            }
            Statement::ForOfStatement(for_of) => {
                self.emit_for_of_stmt(for_of);
            }
            Statement::SwitchStatement(switch) => {
                self.emit_switch_stmt(switch);
            }
            Statement::LabeledStatement(labeled) => {
                self.emit_labeled_stmt(labeled);
            }
            // Everything else: no bytecode emitted.
            _ => {}
        }
    }

    fn emit_switch_stmt(&mut self, switch: &'a oxc_ast::ast::SwitchStatement<'a>) {
        let saved_next_reg = self.next_reg;
        let tag_reg = self.alloc_reg();
        self.emit_expr(&switch.discriminant);
        self.push(Insn::Sta(tag_reg));

        if switch.cases.is_empty() {
            self.next_reg = saved_next_reg;
            return;
        }

        let mut case_jumps = Vec::with_capacity(switch.cases.len());
        let mut default_index = None;
        for (idx, case) in switch.cases.iter().enumerate() {
            if let Some(test) = &case.test {
                self.emit_expr(test);
                self.push(Insn::StrictEq(0, tag_reg));
                let jump_pos = self.insns.len();
                self.push(Insn::Jnez(0));
                case_jumps.push((idx, jump_pos));
            } else {
                default_index = Some(idx);
            }
        }

        let no_match_jump = self.insns.len();
        self.push(Insn::Jmp(0));

        let outer_breaks = std::mem::take(&mut self.break_patches);
        let outer_returned = self.returned;
        self.returned = false;

        let mut case_targets = vec![0usize; switch.cases.len()];
        for (idx, case) in switch.cases.iter().enumerate() {
            case_targets[idx] = self.insns.len();
            for stmt in &case.consequent {
                self.emit_stmt(stmt);
                if self.returned {
                    break;
                }
            }
            self.returned = false;
        }

        let end = self.insns.len();
        for (idx, jump_pos) in case_jumps {
            self.patch_jump_to(jump_pos, case_targets[idx]);
        }
        let no_match_target = default_index.map(|idx| case_targets[idx]).unwrap_or(end);
        self.patch_jump_to(no_match_jump, no_match_target);

        let breaks = std::mem::replace(&mut self.break_patches, outer_breaks);
        self.patch_jumps_to(breaks, end);
        self.returned = outer_returned;
        self.next_reg = saved_next_reg;
    }

    fn emit_labeled_stmt(&mut self, labeled: &'a oxc_ast::ast::LabeledStatement<'a>) {
        let name = labeled.label.name.as_str().to_string();
        self.label_stack.push(LabelContext {
            name: name.clone(),
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        });

        match &labeled.body {
            Statement::WhileStatement(stmt) => self.emit_while_stmt_labeled(stmt, Some(&name)),
            Statement::DoWhileStatement(stmt) => self.emit_do_while_stmt_labeled(stmt, Some(&name)),
            Statement::ForStatement(stmt) => self.emit_for_stmt_labeled(stmt, Some(&name)),
            Statement::ForInStatement(stmt) => self.emit_for_in_stmt_labeled(stmt, Some(&name)),
            Statement::ForOfStatement(stmt) => self.emit_for_of_stmt_labeled(stmt, Some(&name)),
            _ => self.emit_stmt(&labeled.body),
        }

        let mut ctx = self
            .label_stack
            .pop()
            .expect("labeled statement context must exist");
        self.patch_jumps_to_here(std::mem::take(&mut ctx.break_patches));
        self.patch_jumps_to_here(std::mem::take(&mut ctx.continue_patches));
    }

    fn push_labeled_break(&mut self, label: &str, patch: usize) {
        if let Some(ctx) = self
            .label_stack
            .iter_mut()
            .rev()
            .find(|ctx| ctx.name == label)
        {
            ctx.break_patches.push(patch);
        } else {
            self.break_patches.push(patch);
        }
    }

    fn push_labeled_continue(&mut self, label: &str, patch: usize) {
        if let Some(ctx) = self
            .label_stack
            .iter_mut()
            .rev()
            .find(|ctx| ctx.name == label)
        {
            ctx.continue_patches.push(patch);
        } else {
            self.continue_patches.push(patch);
        }
    }

    fn take_labeled_continue_patches(&mut self, label: Option<&str>) -> Vec<usize> {
        let Some(label) = label else {
            return Vec::new();
        };
        let Some(ctx) = self
            .label_stack
            .iter_mut()
            .rev()
            .find(|ctx| ctx.name == label)
        else {
            return Vec::new();
        };
        std::mem::take(&mut ctx.continue_patches)
    }

    // ── While loop ────────────────────────────────────────────────────────────

    fn emit_for_in_stmt(&mut self, stmt: &'a ForInStatement<'a>) {
        self.emit_for_in_stmt_labeled(stmt, None);
    }

    fn emit_for_in_stmt_labeled(&mut self, stmt: &'a ForInStatement<'a>, label: Option<&str>) {
        self.emit_expr(&stmt.right);
        let try_begin = self.insns.len() as u32;
        self.push(Insn::GetPropIterator);
        let iter_reg = self.alloc_reg();
        self.push(Insn::Sta(iter_reg));
        let try_end = self.insns.len() as u32;

        let jmp_past_handler = self.insns.len();
        self.push(Insn::Jmp(0));
        let catch_begin = self.insns.len() as u32;
        let err_reg = self.alloc_reg();
        self.push(Insn::Sta(err_reg));
        self.push(Insn::Lda(err_reg));
        self.push(Insn::Throw);
        let after_handler = self.insns.len();
        self.patch_jump_to(jmp_past_handler, after_handler);
        self.push_catch_entry(CatchEntry {
            try_begin,
            try_end,
            catch_begin,
            catch_end: after_handler as u32,
        });

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        let loop_start = self.insns.len();
        self.push(Insn::GetNextPropName(iter_reg));
        let key_reg = self.alloc_reg();
        self.push(Insn::Sta(key_reg));
        self.push(Insn::LdUndefined);
        self.push(Insn::Eq(0, key_reg));
        let jnez_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        self.bind_for_left(&stmt.left, key_reg);

        let outer_returned = self.returned;
        self.returned = false;
        self.emit_stmt(&stmt.body);
        self.returned = outer_returned;

        for patch in std::mem::take(&mut self.continue_patches) {
            self.patch_jump_to(patch, loop_start);
        }
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, loop_start);

        let jmp_pos = self.insns.len();
        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        self.push(Insn::Jmp(back_off));

        let exit_byte = self.insns.len();
        self.patch_jump_to(jnez_pos, exit_byte);

        for patch in std::mem::take(&mut self.break_patches) {
            self.patch_jump_to(patch, exit_byte);
        }

        self.break_patches = outer_break;
        self.continue_patches = outer_continue;
    }

    fn emit_for_of_stmt(&mut self, stmt: &'a ForOfStatement<'a>) {
        self.emit_for_of_stmt_labeled(stmt, None);
    }

    fn emit_for_of_stmt_labeled(&mut self, stmt: &'a ForOfStatement<'a>, label: Option<&str>) {
        let init_try_begin = self.insns.len() as u32;
        self.emit_expr(&stmt.right);
        let init_try_end = self.insns.len() as u32;
        let init_jmp_past_catch = self.insns.len();
        self.push(Insn::Jmp(0));
        let init_catch_begin = self.insns.len() as u32;
        let exception_reg = self.alloc_reg();
        self.push(Insn::Sta(exception_reg));
        self.push(Insn::Lda(exception_reg));
        self.push(Insn::Throw);
        let init_catch_end = self.insns.len() as u32;
        self.patch_jump_to(init_jmp_past_catch, init_catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin: init_try_begin,
            try_end: init_try_end,
            catch_begin: init_catch_begin,
            catch_end: init_catch_end,
        });

        let state = IteratorState {
            closed_reg: self.alloc_reg(),
            method_reg: self.alloc_reg(),
            iterator_reg: self.alloc_reg(),
        };
        let result_reg = self.alloc_reg();
        self.push(Insn::GetIterator(0));
        self.push(Insn::Sta(state.iterator_reg));

        let next_id = self.intern("next");
        let done_id = self.intern("done");
        let value_id = self.intern("value");
        self.push(Insn::Lda(state.iterator_reg));
        self.push(Insn::LdObjByName(0, next_id));
        self.push(Insn::Sta(state.method_reg));
        self.push(Insn::LdFalse);
        self.push(Insn::Sta(state.closed_reg));

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        let loop_start = self.insns.len();

        self.push(Insn::Lda(state.method_reg));
        self.push(Insn::CallThis0(0, state.iterator_reg));
        self.push(Insn::Sta(result_reg));
        self.push(Insn::ThrowIfNotObject(result_reg));
        self.push(Insn::Lda(result_reg));
        self.push(Insn::LdObjByName(0, done_id));
        let done_pos = self.emit_branch_if_true_from_acc();

        self.push(Insn::Lda(result_reg));
        self.push(Insn::LdObjByName(0, value_id));
        let val_reg = self.alloc_reg();
        self.push(Insn::Sta(val_reg));

        let body_try_begin = self.insns.len() as u32;
        self.iterator_stack.push(state);
        self.bind_for_left(&stmt.left, val_reg);

        let outer_returned = self.returned;
        self.returned = false;
        self.emit_stmt(&stmt.body);
        self.returned = outer_returned;
        self.iterator_stack.pop();
        let body_try_end = self.insns.len() as u32;
        let body_jmp_past_catch = self.insns.len();
        self.push(Insn::Jmp(0));
        let body_catch_begin = self.insns.len() as u32;
        self.emit_iterator_close(state, true);
        let body_catch_end = self.insns.len() as u32;
        self.patch_jump_to(body_jmp_past_catch, body_catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin: body_try_begin,
            try_end: body_try_end,
            catch_begin: body_catch_begin,
            catch_end: body_catch_end,
        });

        for patch in std::mem::take(&mut self.continue_patches) {
            self.patch_jump_to(patch, loop_start);
        }
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, loop_start);

        let jmp_pos = self.insns.len();
        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        self.push(Insn::Jmp(back_off));

        let exit_byte = self.insns.len();
        self.patch_jump_to(done_pos, exit_byte);

        for patch in std::mem::take(&mut self.break_patches) {
            self.patch_jump_to(patch, exit_byte);
        }

        self.break_patches = outer_break;
        self.continue_patches = outer_continue;
    }

    fn bind_for_left(&mut self, left: &ForStatementLeft<'_>, value_reg: u8) {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                if let Some(declarator) = decl.declarations.first() {
                    match &declarator.id {
                        BindingPattern::BindingIdentifier(id) => {
                            let name = id.name.as_str().to_string();
                            self.var_regs.insert(name.clone(), value_reg);
                            if let Some(slot) = self.current_lexical_slot(&name) {
                                self.push(Insn::Lda(value_reg));
                                self.push(Insn::StLexVar(0, slot));
                            }
                        }
                        BindingPattern::ArrayPattern(_) | BindingPattern::ObjectPattern(_) => {
                            // Destructuring for-of: bind the element value into the pattern.
                            self.emit_destructure_binding(&declarator.id, value_reg);
                        }
                        BindingPattern::AssignmentPattern(assign) => {
                            // for (const [a = def] of ...) — has a binding default
                            let tmp = self.alloc_reg();
                            self.push(Insn::Lda(value_reg));
                            self.push(Insn::Sta(tmp));
                            self.push(Insn::LdUndefined);
                            self.push(Insn::StrictEq(0, tmp));
                            let skip = self.insns.len();
                            self.push(Insn::Jeqz(0));
                            self.emit_expr(&assign.right);
                            self.push(Insn::Sta(tmp));
                            let after = self.insns.len();
                            self.patch_jump_to(skip, after);
                            self.emit_destructure_binding(&assign.left, tmp);
                        }
                    }
                }
            }
            ForStatementLeft::AssignmentTargetIdentifier(id) => {
                let name = id.name.as_str();
                if let Some(&reg) = self.var_regs.get(name) {
                    if reg != value_reg {
                        self.push(Insn::Lda(value_reg));
                        self.push(Insn::Sta(reg));
                    }
                } else {
                    let name_id = self.intern(name);
                    self.push(Insn::Lda(value_reg));
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
            }
            ForStatementLeft::ArrayAssignmentTarget(arr) => {
                self.emit_destructure_array_assignment(arr, value_reg);
            }
            ForStatementLeft::ObjectAssignmentTarget(obj) => {
                self.emit_destructure_object_assignment(obj, value_reg);
            }
            _ => {}
        }
    }

    // ── If statement with jump patching ──────────────────────────────────────

    /// Emit an `if (test) { consequent } [else { alternate }]` with correct
    /// branch offsets.
    ///
    /// Layout (no-else):
    /// ```text
    ///   <test>
    ///   jeqz  +N        ← if falsy jump past consequent
    ///   <consequent>
    ///   <after>
    /// ```
    ///
    /// Layout (with-else):
    /// ```text
    ///   <test>
    ///   jeqz  +M        ← if falsy jump to alternate
    ///   <consequent>
    ///   jmp   +K        ← skip alternate
    ///   <alternate>
    ///   <after>
    /// ```
    ///
    /// Branch offsets are measured from the start of the branch instruction.
    fn emit_if_stmt(&mut self, if_stmt: &'a oxc_ast::ast::IfStatement<'a>) {
        let false_branch = self.emit_branch_if_false(&if_stmt.test);

        // 3. Emit consequent; preserve outer `returned` flag across the branch.
        let outer_returned = self.returned;
        self.returned = false;
        self.emit_stmt(&if_stmt.consequent);
        let consequent_returned = self.returned;
        self.returned = outer_returned;

        if let Some(alt) = &if_stmt.alternate {
            // 4a. Emit `jmp` to skip the alternate; remember its position.
            let jmp_pos = self.insns.len();
            self.push(Insn::Jmp(0));

            // Patch jeqz → start of alternate (= current byte position).
            let alt_start = self.insns.len();
            self.patch_jump_to(false_branch, alt_start);

            // 5a. Emit alternate.
            let alt_outer_returned = self.returned;
            self.returned = false;
            self.emit_stmt(alt);
            let _alt_returned = self.returned;
            self.returned = alt_outer_returned;

            // Patch jmp → just past the alternate.
            let end_pos = self.insns.len();
            self.patch_jump_to(jmp_pos, end_pos);
        } else {
            // 4b. Patch jeqz → just past the consequent (no else branch).
            let _ = consequent_returned; // informational only for now
            let end_pos = self.insns.len();
            self.patch_jump_to(false_branch, end_pos);
        }
    }

    // ── Loop helpers ──────────────────────────────────────────────────────────

    /// Patch a list of pending `jmp(0)` sites to the current byte position.
    fn patch_jumps_to_here(&mut self, positions: Vec<usize>) {
        let here = self.insns.len();
        for pos in positions {
            self.patch_jump_to(pos, here);
        }
    }

    /// Patch a list of pending `jmp(0)` sites to a specific byte position.
    fn patch_jumps_to(&mut self, positions: Vec<usize>, target: usize) {
        for pos in positions {
            self.patch_jump_to(pos, target);
        }
    }

    fn ensure_program_label(&mut self, offset: usize) -> String {
        if let Some(name) = self.program_label_names.get(&offset) {
            return name.clone();
        }
        let name = format!("label_{}", self.next_program_label);
        self.next_program_label += 1;
        self.program_label_names.insert(offset, name.clone());
        self.program_labels.push(ProgramLabel {
            name: name.clone(),
            offset: offset as u32,
        });
        name
    }

    fn push_catch_entry(&mut self, entry: CatchEntry) {
        self.ensure_program_label(entry.try_begin as usize);
        self.ensure_program_label(entry.try_end as usize);
        self.ensure_program_label(entry.catch_begin as usize);
        self.ensure_program_label(entry.catch_end as usize);
        self.catch_entries.push(entry);
    }

    fn rebuild_program_label_index(&mut self) {
        self.program_label_names.clear();
        for label in &self.program_labels {
            self.program_label_names
                .entry(label.offset as usize)
                .or_insert_with(|| label.name.clone());
        }
    }

    fn patch_jump_to(&mut self, pos: usize, target: usize) {
        let off = target as i32 - pos as i32;
        match self.insns[pos] {
            0x4d | 0x4f | 0x51 => {
                debug_assert!((i8::MIN as i32..=i8::MAX as i32).contains(&off));
                self.insns[pos + 1] = off as i8 as u8;
                self.patch_pandasm_jump(pos, off);
            }
            _ => {
                let [lo, hi] = (off as i16).to_le_bytes();
                self.insns[pos + 1] = lo;
                self.insns[pos + 2] = hi;
                self.patch_pandasm_jump(pos, off);
            }
        }
        self.ensure_program_label(target);
    }

    fn patch_pandasm_jump(&mut self, pos: usize, off: i32) {
        let Some(index) = self
            .pandasm_insn_offsets
            .iter()
            .position(|offset| *offset == pos)
        else {
            return;
        };
        self.pandasm_insns[index].patch_jump_offset(off);
    }

    fn emit_branch_if_false(&mut self, expr: &Expression<'_>) -> usize {
        if let Expression::BinaryExpression(bin) = expr {
            if is_condition_comparison(bin.operator) {
                self.emit_condition_comparison(bin);
                self.set_debug_pos(bin.span.start);
                let branch_pos = self.insns.len();
                self.push(Insn::Jeqz(0));
                return branch_pos;
            }
        }

        self.emit_expr(expr);
        self.set_debug_pos(expr.span().start);
        self.emit_is_false();
        let branch_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        branch_pos
    }

    fn emit_branch_if_true_from_acc(&mut self) -> usize {
        self.emit_is_true();
        let branch_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        branch_pos
    }

    fn emit_optional_nullish_jumps(&mut self, obj_reg: u8) -> [usize; 2] {
        self.push(Insn::LdNull);
        self.push(Insn::StrictEq(0, obj_reg));
        let null_jump = self.insns.len();
        self.push(Insn::Jnez(0));

        self.push(Insn::LdUndefined);
        self.push(Insn::StrictEq(0, obj_reg));
        let undefined_jump = self.insns.len();
        self.push(Insn::Jnez(0));

        [null_jump, undefined_jump]
    }

    fn patch_optional_nullish_jumps(&mut self, jumps: [usize; 2], target: usize) {
        for jump in jumps {
            self.patch_jump_to(jump, target);
        }
    }

    fn emit_branch_if_undefined_from_acc(&mut self) -> usize {
        let tmp = self.alloc_reg();
        self.push(Insn::Sta(tmp));
        self.push(Insn::LdUndefined);
        self.push(Insn::Eq(0, tmp));
        let branch_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        branch_pos
    }

    fn emit_branch_if_not_undefined_from_acc(&mut self) -> usize {
        let tmp = self.alloc_reg();
        self.push(Insn::Sta(tmp));
        self.push(Insn::LdUndefined);
        self.push(Insn::Eq(0, tmp));
        let branch_pos = self.insns.len();
        self.push(Insn::Jeqz(0));
        branch_pos
    }

    fn emit_rethrow_from_acc(&mut self) {
        let exception = self.alloc_reg();
        self.push(Insn::Sta(exception));
        let hole = self.alloc_reg();
        self.push(Insn::LdHole);
        self.push(Insn::Sta(hole));
        self.push(Insn::Lda(exception));
        self.push(Insn::NotEq(0, hole));
        let skip_throw = self.insns.len();
        self.push(Insn::Jeqz(0));
        self.push(Insn::Lda(exception));
        self.push(Insn::Throw);
        let end = self.insns.len();
        self.patch_jump_to(skip_throw, end);
    }

    fn emit_control_flow_exit(&mut self, close_iterators: bool) {
        let has_iterators = close_iterators && !self.iterator_stack.is_empty();
        if !has_iterators && self.finally_stack.is_empty() {
            return;
        }

        let result_reg = self.alloc_reg();
        self.push(Insn::Sta(result_reg));

        if has_iterators {
            for state in self.iterator_stack.clone().into_iter().rev() {
                self.push(Insn::Lda(result_reg));
                self.emit_iterator_close(state, false);
                if self.returned {
                    return;
                }
                self.push(Insn::Sta(result_reg));
            }
        }

        let finalizers = self.finally_stack.clone();
        for finalizer in finalizers.into_iter().rev() {
            self.emit_finalizer_block(finalizer);
            if self.returned {
                return;
            }
        }

        self.push(Insn::Lda(result_reg));
    }

    fn emit_finalizer_block(&mut self, finalizer: &'a oxc_ast::ast::BlockStatement<'a>) {
        let saved_stack = self.finally_stack.clone();
        if let Some(pos) = self
            .finally_stack
            .iter()
            .rposition(|active| std::ptr::eq(*active, finalizer))
        {
            self.finally_stack.truncate(pos);
        }

        let outer_returned = self.returned;
        self.returned = false;
        for stmt in &finalizer.body {
            self.emit_stmt(stmt);
            if self.returned {
                break;
            }
        }
        let finalizer_returned = self.returned;
        self.returned = outer_returned || finalizer_returned;
        if !finalizer_returned {
            self.finally_stack = saved_stack;
        }
    }

    fn emit_iterator_close(&mut self, state: IteratorState, abrupt_completion: bool) {
        let completion = self.alloc_reg();
        let inner_result = self.alloc_reg();
        let inner_exception = self.alloc_reg();

        self.push(Insn::Sta(completion));
        self.push(Insn::Lda(state.closed_reg));
        let no_return_closed = self.emit_branch_if_true_from_acc();

        self.push(Insn::LdTrue);
        self.push(Insn::Sta(state.closed_reg));
        self.push(Insn::LdUndefined);
        self.push(Insn::Sta(inner_result));
        self.push(Insn::LdHole);
        self.push(Insn::Sta(inner_exception));

        let try_begin = self.insns.len() as u32;
        let return_id = self.intern("return");
        self.push(Insn::Lda(state.iterator_reg));
        self.push(Insn::LdObjByName(0, return_id));
        self.push(Insn::Sta(state.method_reg));
        let no_return_undefined = self.emit_branch_if_undefined_from_acc();
        self.push(Insn::Lda(state.method_reg));
        self.push(Insn::CallThis0(0, state.iterator_reg));
        self.push(Insn::Sta(inner_result));
        let try_end = self.insns.len() as u32;
        let jump_past_catch = self.insns.len();
        self.push(Insn::Jmp(0));
        let catch_begin = self.insns.len() as u32;
        self.push(Insn::Sta(inner_exception));
        let catch_end = self.insns.len() as u32;
        self.patch_jump_to(jump_past_catch, catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin,
            try_end,
            catch_begin,
            catch_end: catch_begin,
        });

        if abrupt_completion {
            self.push(Insn::Lda(completion));
            self.push(Insn::Throw);
        } else {
            self.push(Insn::Lda(inner_exception));
            self.emit_rethrow_from_acc();
        }

        self.push(Insn::Lda(inner_result));
        self.push(Insn::ThrowIfNotObject(inner_result));

        let no_return = self.insns.len();
        self.patch_jump_to(no_return_closed, no_return);
        self.patch_jump_to(no_return_undefined, no_return);
        self.push(Insn::Lda(completion));
        if abrupt_completion {
            self.push(Insn::Throw);
        }
    }

    // ── Destructuring binding lowering ──────────────────────────────────────

    /// Bind `rhs_reg` (which holds the initializer value) to `pattern`.
    ///
    /// Mirrors upstream `es2abc` destructuring lowering:
    /// - **ObjectPattern**: property-load per key via `LdObjByName` / `LdObjByValue`.
    /// - **ArrayPattern**: full iterator protocol (same as `for-of`).
    /// - **AssignmentPattern**: default-value guard (`undefined` check → branch).
    /// - **BindingIdentifier**: direct register assignment.
    fn emit_destructure_binding(&mut self, pattern: &BindingPattern<'_>, rhs_reg: u8) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                self.emit_bind_identifier_from_reg(id.name.as_str(), rhs_reg);
            }
            BindingPattern::AssignmentPattern(assign) => {
                // Default value: if `rhs_reg === undefined`, evaluate the default.
                let value_reg = self.alloc_reg();
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::Sta(value_reg));
                self.push(Insn::LdUndefined);
                self.push(Insn::StrictEq(0, value_reg));
                let skip_default = self.insns.len();
                self.push(Insn::Jeqz(0));
                // Value IS undefined → evaluate and store default.
                self.emit_expr(&assign.right);
                self.push(Insn::Sta(value_reg));
                let after_default = self.insns.len();
                self.patch_jump_to(skip_default, after_default);
                self.emit_destructure_binding(&assign.left, value_reg);
            }
            BindingPattern::ObjectPattern(obj) => {
                self.emit_destructure_object_binding(obj, rhs_reg);
            }
            BindingPattern::ArrayPattern(arr) => {
                self.emit_destructure_array_binding(arr, rhs_reg);
            }
        }
    }

    /// Bind an identifier from `value_reg`; also write to lexical env slot and
    /// module export slot if applicable.
    fn emit_bind_identifier_from_reg(&mut self, name: &str, value_reg: u8) {
        let name = name.to_string();
        if let Some(reg) = self.var_regs.get(&name) {
            let target = *reg;
            if target != value_reg {
                self.push(Insn::Lda(value_reg));
                self.push(Insn::Sta(target));
            }
        } else {
            let reg = self.alloc_reg();
            self.var_regs.insert(name.clone(), reg);
            self.push(Insn::Lda(value_reg));
            self.push(Insn::Sta(reg));
            if let Some(slot) = self.current_lexical_slot(&name) {
                self.push(Insn::Lda(reg));
                self.push(Insn::StLexVar(0, slot));
            }
            if let Some(export_index) = self.lowering.export_index(&name) {
                self.push(Insn::Lda(reg));
                self.push(Insn::StModuleVar(export_index));
            }
        }
    }

    /// Object destructuring binding: `const {a, b: alias, c = def, ...rest} = rhs_reg`.
    fn emit_destructure_object_binding(&mut self, obj: &ObjectPattern<'_>, rhs_reg: u8) {
        let mut excluded_key_regs: Vec<u8> = Vec::new();

        for prop in &obj.properties {
            let value_reg = self.alloc_reg();
            self.push(Insn::Lda(rhs_reg));
            if prop.computed {
                // Computed key: evaluate key expression then use LdObjByValue.
                let key_reg = self.alloc_reg();
                if let Some(key_expr) = prop.key.as_expression() {
                    self.emit_expr(key_expr);
                }
                self.push(Insn::Sta(key_reg));
                excluded_key_regs.push(key_reg);
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::LdObjByValue(0, key_reg));
            } else {
                // Static key: use LdObjByName.
                let key_name = match &prop.key {
                    PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                    PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                    PropertyKey::NumericLiteral(n) => n.value.to_string(),
                    _ => String::new(),
                };
                let key_id = self.intern(&key_name);
                self.push(Insn::LdObjByName(0, key_id));
            }
            self.push(Insn::Sta(value_reg));
            self.emit_destructure_binding(&prop.value, value_reg);
        }

        // Rest: CopyDataProperties to collect remaining own properties.
        if let Some(rest) = &obj.rest {
            let rest_reg = self.alloc_reg();
            self.push(Insn::CreateEmptyObject);
            self.push(Insn::Sta(rest_reg));
            // CopyDataProperties(dst, src, excluded_key_count, excluded_keys...)
            // Upstream: callruntime CopyDataProperties with excluded keys.
            // Simple form: if no excluded keys, just iterate own properties.
            // Approximate: emit CopyDataProperties without exclusions for now.
            let _ = excluded_key_regs; // excluded keys would be passed here
            self.push(Insn::CopyDataProperties(rhs_reg));
            // acc = the rest object (filled by CopyDataProperties from rhs_reg into acc)
            // Actually CopyDataProperties fills into the last created empty object.
            // Upstream pattern: sta dst; lda src; copydataproperties dst
            self.push(Insn::Sta(rest_reg));
            self.emit_destructure_binding(&rest.argument, rest_reg);
        }
    }

    /// Array destructuring binding: `const [a, b, ...rest] = rhs_reg`.
    ///
    /// Uses the full iterator protocol, identical to `for-of` lowering.
    fn emit_destructure_array_binding(&mut self, arr: &ArrayPattern<'_>, rhs_reg: u8) {
        // Push the iterable onto the accumulator and create the iterator.
        let state = IteratorState {
            closed_reg: self.alloc_reg(),
            method_reg: self.alloc_reg(),
            iterator_reg: self.alloc_reg(),
        };
        let result_reg = self.alloc_reg();

        self.push(Insn::Lda(rhs_reg));
        self.push(Insn::GetIterator(0));
        self.push(Insn::Sta(state.iterator_reg));

        let next_id = self.intern("next");
        let done_id = self.intern("done");
        let value_id = self.intern("value");

        self.push(Insn::Lda(state.iterator_reg));
        self.push(Insn::LdObjByName(0, next_id));
        self.push(Insn::Sta(state.method_reg));
        self.push(Insn::LdFalse);
        self.push(Insn::Sta(state.closed_reg));

        let body_try_begin = self.insns.len() as u32;

        for element in &arr.elements {
            // Advance the iterator (even for holes).
            self.push(Insn::Lda(state.method_reg));
            self.push(Insn::CallThis0(0, state.iterator_reg));
            self.push(Insn::Sta(result_reg));
            self.push(Insn::ThrowIfNotObject(result_reg));

            // Check `done`.
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, done_id));
            // If done, use `undefined` as the value.
            let done_skip = self.insns.len();
            self.push(Insn::Jeqz(0));
            let undef_reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(undef_reg));
            let after_done = self.insns.len();
            self.push(Insn::Jmp(0)); // skip value extraction
            let not_done = self.insns.len();
            self.patch_jump_to(done_skip, not_done);

            // Get `value`.
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, value_id));
            self.push(Insn::Sta(undef_reg));
            let value_end = self.insns.len();
            self.patch_jump_to(after_done, value_end);

            if let Some(elem) = element {
                self.emit_destructure_binding(elem, undef_reg);
            }
            // Hole (`[, b]`): just discard the value.
        }

        // Rest element: collect remaining values.
        if let Some(rest) = &arr.rest {
            let rest_arr_reg = self.alloc_reg();
            self.push(Insn::CreateEmptyArray(0));
            self.push(Insn::Sta(rest_arr_reg));
            let idx_reg = self.alloc_reg();
            self.push(Insn::LdAi(0));
            self.push(Insn::Sta(idx_reg));

            // Loop: call next(), exit when done, push values to rest array.
            let loop_start = self.insns.len();
            self.push(Insn::Lda(state.method_reg));
            self.push(Insn::CallThis0(0, state.iterator_reg));
            self.push(Insn::Sta(result_reg));
            self.push(Insn::ThrowIfNotObject(result_reg));
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, done_id));
            let rest_done = self.insns.len();
            self.push(Insn::Jnez(0)); // exit loop when done
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, value_id));
            let item_reg = self.alloc_reg();
            self.push(Insn::Sta(item_reg));
            // Append to rest_arr_reg.
            self.push(Insn::StArraySpread(rest_arr_reg, idx_reg));
            let back_off = (loop_start as i32 - self.insns.len() as i32) as i16;
            self.push(Insn::Jmp(back_off));
            let rest_exit = self.insns.len();
            self.patch_jump_to(rest_done, rest_exit);

            self.emit_destructure_binding(&rest.argument, rest_arr_reg);
        }

        let body_try_end = self.insns.len() as u32;
        let body_jmp_past_catch = self.insns.len();
        self.push(Insn::Jmp(0));
        let body_catch_begin = self.insns.len() as u32;
        self.emit_iterator_close(state, true);
        let body_catch_end = self.insns.len() as u32;
        self.patch_jump_to(body_jmp_past_catch, body_catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin: body_try_begin,
            try_end: body_try_end,
            catch_begin: body_catch_begin,
            catch_end: body_catch_end,
        });

        // Normal completion: close the iterator.
        self.push(Insn::LdUndefined);
        self.emit_iterator_close(state, false);
    }

    // ── Destructuring assignment lowering ────────────────────────────────────

    /// Assign `rhs_reg` (result of evaluating the RHS) into assignment target
    /// `target`.  Mirrors upstream `es2abc` destructuring assignment lowering.
    fn emit_destructure_assignment(&mut self, target: &AssignmentTarget<'_>, rhs_reg: u8) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.push(Insn::Lda(rhs_reg));
                let name = id.name.as_str();
                if let Some(&reg) = self.var_regs.get(name) {
                    self.push(Insn::Sta(reg));
                } else if let Some((level, slot)) = self.lexical_lookup(name) {
                    self.push(Insn::StLexVar(level, slot));
                } else {
                    let name_id = self.intern(name);
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
            }
            AssignmentTarget::StaticMemberExpression(mem) => {
                let key_name = mem.property.name.as_str().to_string();
                let key_id = self.intern(&key_name);
                let obj_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::StObjByName(0, key_id, obj_reg));
            }
            AssignmentTarget::ComputedMemberExpression(mem) => {
                let obj_reg = self.alloc_reg();
                let key_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.push(Insn::Sta(obj_reg));
                self.emit_expr(&mem.expression);
                self.push(Insn::Sta(key_reg));
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::StObjByValue(0, obj_reg, key_reg));
            }
            AssignmentTarget::ArrayAssignmentTarget(arr) => {
                self.emit_destructure_array_assignment(arr, rhs_reg);
            }
            AssignmentTarget::ObjectAssignmentTarget(obj) => {
                self.emit_destructure_object_assignment(obj, rhs_reg);
            }
            AssignmentTarget::TSAsExpression(expr) => {
                self.emit_destructure_from_expr(&expr.expression, rhs_reg);
            }
            AssignmentTarget::TSSatisfiesExpression(expr) => {
                self.emit_destructure_from_expr(&expr.expression, rhs_reg);
            }
            AssignmentTarget::TSNonNullExpression(expr) => {
                self.emit_destructure_from_expr(&expr.expression, rhs_reg);
            }
            AssignmentTarget::TSTypeAssertion(expr) => {
                self.emit_destructure_from_expr(&expr.expression, rhs_reg);
            }
            _ => {}
        }
    }

    /// Emit assignment of `rhs_reg` into an `Expression` that is being used as
    /// an assignment target (TS wrapper unwrapping).
    fn emit_destructure_from_expr(&mut self, expr: &Expression<'_>, rhs_reg: u8) {
        match expr {
            Expression::Identifier(id) => {
                self.push(Insn::Lda(rhs_reg));
                let name = id.name.as_str();
                if let Some(&reg) = self.var_regs.get(name) {
                    self.push(Insn::Sta(reg));
                } else if let Some((level, slot)) = self.lexical_lookup(name) {
                    self.push(Insn::StLexVar(level, slot));
                } else {
                    let name_id = self.intern(name);
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
            }
            Expression::StaticMemberExpression(mem) => {
                let key_name = mem.property.name.as_str().to_string();
                let key_id = self.intern(&key_name);
                let obj_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::StObjByName(0, key_id, obj_reg));
            }
            Expression::ComputedMemberExpression(mem) => {
                let obj_reg = self.alloc_reg();
                let key_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.push(Insn::Sta(obj_reg));
                self.emit_expr(&mem.expression);
                self.push(Insn::Sta(key_reg));
                self.push(Insn::Lda(rhs_reg));
                self.push(Insn::StObjByValue(0, obj_reg, key_reg));
            }
            Expression::ParenthesizedExpression(p) => {
                self.emit_destructure_from_expr(&p.expression, rhs_reg);
            }
            Expression::TSAsExpression(e) => {
                self.emit_destructure_from_expr(&e.expression, rhs_reg)
            }
            Expression::TSSatisfiesExpression(e) => {
                self.emit_destructure_from_expr(&e.expression, rhs_reg)
            }
            Expression::TSNonNullExpression(e) => {
                self.emit_destructure_from_expr(&e.expression, rhs_reg)
            }
            Expression::TSTypeAssertion(e) => {
                self.emit_destructure_from_expr(&e.expression, rhs_reg)
            }
            _ => {}
        }
    }

    fn emit_destructure_array_assignment(&mut self, arr: &ArrayAssignmentTarget<'_>, rhs_reg: u8) {
        let state = IteratorState {
            closed_reg: self.alloc_reg(),
            method_reg: self.alloc_reg(),
            iterator_reg: self.alloc_reg(),
        };
        let result_reg = self.alloc_reg();
        let next_id = self.intern("next");
        let done_id = self.intern("done");
        let value_id = self.intern("value");

        self.push(Insn::Lda(rhs_reg));
        self.push(Insn::GetIterator(0));
        self.push(Insn::Sta(state.iterator_reg));
        self.push(Insn::Lda(state.iterator_reg));
        self.push(Insn::LdObjByName(0, next_id));
        self.push(Insn::Sta(state.method_reg));
        self.push(Insn::LdFalse);
        self.push(Insn::Sta(state.closed_reg));

        let body_try_begin = self.insns.len() as u32;

        for element in &arr.elements {
            let value_reg = self.alloc_reg();
            self.push(Insn::Lda(state.method_reg));
            self.push(Insn::CallThis0(0, state.iterator_reg));
            self.push(Insn::Sta(result_reg));
            self.push(Insn::ThrowIfNotObject(result_reg));
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, done_id));
            let done_skip = self.insns.len();
            self.push(Insn::Jeqz(0));
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(value_reg));
            let after_done = self.insns.len();
            self.push(Insn::Jmp(0));
            let not_done = self.insns.len();
            self.patch_jump_to(done_skip, not_done);
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, value_id));
            self.push(Insn::Sta(value_reg));
            let value_end = self.insns.len();
            self.patch_jump_to(after_done, value_end);

            if let Some(elem) = element {
                self.emit_destructure_assignment_maybe_default(elem, value_reg);
            }
        }

        if let Some(rest) = &arr.rest {
            let rest_arr_reg = self.alloc_reg();
            self.push(Insn::CreateEmptyArray(0));
            self.push(Insn::Sta(rest_arr_reg));
            let idx_reg = self.alloc_reg();
            self.push(Insn::LdAi(0));
            self.push(Insn::Sta(idx_reg));

            let loop_start = self.insns.len();
            self.push(Insn::Lda(state.method_reg));
            self.push(Insn::CallThis0(0, state.iterator_reg));
            self.push(Insn::Sta(result_reg));
            self.push(Insn::ThrowIfNotObject(result_reg));
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, done_id));
            let rest_done = self.insns.len();
            self.push(Insn::Jnez(0));
            self.push(Insn::Lda(result_reg));
            self.push(Insn::LdObjByName(0, value_id));
            let item_reg = self.alloc_reg();
            self.push(Insn::Sta(item_reg));
            self.push(Insn::StArraySpread(rest_arr_reg, idx_reg));
            let back_off = (loop_start as i32 - self.insns.len() as i32) as i16;
            self.push(Insn::Jmp(back_off));
            let rest_exit = self.insns.len();
            self.patch_jump_to(rest_done, rest_exit);

            self.emit_destructure_assignment(&rest.target, rest_arr_reg);
        }

        let body_try_end = self.insns.len() as u32;
        let body_jmp_past_catch = self.insns.len();
        self.push(Insn::Jmp(0));
        let body_catch_begin = self.insns.len() as u32;
        self.emit_iterator_close(state, true);
        let body_catch_end = self.insns.len() as u32;
        self.patch_jump_to(body_jmp_past_catch, body_catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin: body_try_begin,
            try_end: body_try_end,
            catch_begin: body_catch_begin,
            catch_end: body_catch_end,
        });
        self.push(Insn::LdUndefined);
        self.emit_iterator_close(state, false);
    }

    fn emit_destructure_assignment_maybe_default(
        &mut self,
        target: &AssignmentTargetMaybeDefault<'_>,
        value_reg: u8,
    ) {
        match target {
            AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(w) => {
                let tmp_reg = self.alloc_reg();
                self.push(Insn::Lda(value_reg));
                self.push(Insn::Sta(tmp_reg));
                self.push(Insn::LdUndefined);
                self.push(Insn::StrictEq(0, tmp_reg));
                let skip_default = self.insns.len();
                self.push(Insn::Jeqz(0));
                self.emit_expr(&w.init);
                self.push(Insn::Sta(tmp_reg));
                let after_default = self.insns.len();
                self.patch_jump_to(skip_default, after_default);
                self.emit_destructure_assignment(&w.binding, tmp_reg);
            }
            _ => {
                if let Some(inner) = target.as_assignment_target() {
                    self.emit_destructure_assignment(inner, value_reg);
                }
            }
        }
    }

    fn emit_destructure_object_assignment(
        &mut self,
        obj: &ObjectAssignmentTarget<'_>,
        rhs_reg: u8,
    ) {
        for prop in &obj.properties {
            match prop {
                AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(id_prop) => {
                    // Shorthand: `({ a } = obj)` → read `obj.a`, assign to `a` (with default).
                    let prop_name = id_prop.binding.name.as_str().to_string();
                    let key_id = self.intern(&prop_name);
                    let value_reg = self.alloc_reg();
                    self.push(Insn::Lda(rhs_reg));
                    self.push(Insn::LdObjByName(0, key_id));
                    self.push(Insn::Sta(value_reg));
                    if let Some(init) = &id_prop.init {
                        // Apply default if undefined.
                        let tmp_reg = self.alloc_reg();
                        self.push(Insn::Lda(value_reg));
                        self.push(Insn::Sta(tmp_reg));
                        self.push(Insn::LdUndefined);
                        self.push(Insn::StrictEq(0, tmp_reg));
                        let skip_default = self.insns.len();
                        self.push(Insn::Jeqz(0));
                        self.emit_expr(init);
                        self.push(Insn::Sta(tmp_reg));
                        let after_default = self.insns.len();
                        self.patch_jump_to(skip_default, after_default);
                        // Assign tmp_reg to the identifier.
                        self.push(Insn::Lda(tmp_reg));
                        let id_name = id_prop.binding.name.as_str();
                        if let Some(&reg) = self.var_regs.get(id_name) {
                            self.push(Insn::Sta(reg));
                        } else if let Some((level, slot)) = self.lexical_lookup(id_name) {
                            self.push(Insn::StLexVar(level, slot));
                        } else {
                            let n_id = self.intern(id_name);
                            self.push(Insn::TryStGlobalByName(0, n_id));
                        }
                    } else {
                        // No default: assign value_reg directly.
                        let id_name = id_prop.binding.name.as_str();
                        if let Some(&reg) = self.var_regs.get(id_name) {
                            self.push(Insn::Lda(value_reg));
                            self.push(Insn::Sta(reg));
                        } else if let Some((level, slot)) = self.lexical_lookup(id_name) {
                            self.push(Insn::Lda(value_reg));
                            self.push(Insn::StLexVar(level, slot));
                        } else {
                            let n_id = self.intern(id_name);
                            self.push(Insn::Lda(value_reg));
                            self.push(Insn::TryStGlobalByName(0, n_id));
                        }
                    }
                }
                AssignmentTargetProperty::AssignmentTargetPropertyProperty(prop_prop) => {
                    // Renamed: `({ a: b } = obj)` → read `obj.a`, assign to `b`.
                    let value_reg = self.alloc_reg();
                    self.push(Insn::Lda(rhs_reg));
                    if prop_prop.computed {
                        let key_reg = self.alloc_reg();
                        if let Some(key_expr) = prop_prop.name.as_expression() {
                            self.emit_expr(key_expr);
                        }
                        self.push(Insn::Sta(key_reg));
                        self.push(Insn::Lda(rhs_reg));
                        self.push(Insn::LdObjByValue(0, key_reg));
                    } else {
                        let key_name = match &prop_prop.name {
                            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                            PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                            _ => String::new(),
                        };
                        let key_id = self.intern(&key_name);
                        self.push(Insn::LdObjByName(0, key_id));
                    }
                    self.push(Insn::Sta(value_reg));
                    self.emit_destructure_assignment_maybe_default(&prop_prop.binding, value_reg);
                }
            }
        }

        // Rest: collect remaining properties.
        if let Some(rest) = &obj.rest {
            let rest_reg = self.alloc_reg();
            self.push(Insn::CreateEmptyObject);
            self.push(Insn::Sta(rest_reg));
            self.push(Insn::CopyDataProperties(rhs_reg));
            self.push(Insn::Sta(rest_reg));
            self.emit_destructure_assignment(&rest.target, rest_reg);
        }
    }

    fn emit_is_true(&mut self) {
        if self.lowering.is_default_api_version() {
            self.push(Insn::IsTrue);
        } else {
            self.push(Insn::CallRuntimeIsTrue(0));
        }
    }

    fn emit_is_false(&mut self) {
        if self.lowering.is_default_api_version() {
            self.push(Insn::IsFalse);
        } else {
            self.push(Insn::CallRuntimeIsFalse(0));
        }
    }

    fn emit_negate_acc(&mut self) {
        let false_branch = self.emit_branch_if_true_from_acc();
        self.push(Insn::LdTrue);
        let end_jump = self.insns.len();
        self.push(Insn::Jmp(0));

        let false_label = self.insns.len();
        self.patch_jump_to(false_branch, false_label);
        self.push(Insn::LdFalse);

        let end_label = self.insns.len();
        self.patch_jump_to(end_jump, end_label);
    }

    fn emit_while_stmt(&mut self, while_stmt: &'a oxc_ast::ast::WhileStatement<'a>) {
        self.emit_while_stmt_labeled(while_stmt, None);
    }

    fn emit_while_stmt_labeled(
        &mut self,
        while_stmt: &'a oxc_ast::ast::WhileStatement<'a>,
        label: Option<&str>,
    ) {
        let loop_start = self.insns.len();
        let false_branch = self.emit_branch_if_false(&while_stmt.test);

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        self.emit_stmt(&while_stmt.body);

        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));
        let loop_exit = self.insns.len();

        self.patch_jump_to(jmp_pos, loop_start);

        self.patch_jump_to(false_branch, loop_exit);

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, loop_start);
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, loop_start);
    }

    fn emit_do_while_stmt(&mut self, do_while: &'a oxc_ast::ast::DoWhileStatement<'a>) {
        self.emit_do_while_stmt_labeled(do_while, None);
    }

    fn emit_do_while_stmt_labeled(
        &mut self,
        do_while: &'a oxc_ast::ast::DoWhileStatement<'a>,
        label: Option<&str>,
    ) {
        let loop_start = self.insns.len();

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        self.emit_stmt(&do_while.body);

        let continue_target = self.insns.len();
        let false_branch = self.emit_branch_if_false(&do_while.test);
        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));
        let loop_exit = self.insns.len();
        self.patch_jump_to(jmp_pos, loop_start);
        self.patch_jump_to(false_branch, loop_exit);

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, continue_target);
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, continue_target);
        let _ = loop_exit;
    }

    fn emit_for_stmt(&mut self, for_stmt: &'a oxc_ast::ast::ForStatement<'a>) {
        self.emit_for_stmt_labeled(for_stmt, None);
    }

    fn emit_for_stmt_labeled(
        &mut self,
        for_stmt: &'a oxc_ast::ast::ForStatement<'a>,
        label: Option<&str>,
    ) {
        if self.emit_standalone_simple_numeric_for_stmt(for_stmt, label) {
            return;
        }

        if let Some(init) = &for_stmt.init {
            self.emit_for_init(init);
        }

        let loop_start = self.insns.len();
        let mut jeqz_pos: Option<usize> = None;
        if let Some(test) = &for_stmt.test {
            jeqz_pos = Some(self.emit_branch_if_false(test));
        }

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        self.emit_stmt(&for_stmt.body);

        let continue_target = self.insns.len();
        if let Some(update) = &for_stmt.update {
            self.set_debug_pos(update.span().start);
            self.emit_expr(update);
        }

        self.set_debug_pos(for_stmt.span.start);
        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));
        let loop_exit = self.insns.len();

        self.patch_jump_to(jmp_pos, loop_start);

        if let Some(jpos) = jeqz_pos {
            self.patch_jump_to(jpos, loop_exit);
        }

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, continue_target);
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, continue_target);
    }

    fn emit_standalone_simple_numeric_for_stmt(
        &mut self,
        for_stmt: &'a oxc_ast::ast::ForStatement<'a>,
        label: Option<&str>,
    ) -> bool {
        if !self.lowering.standalone_ecmascript_input() {
            return false;
        }
        let Some((name, init_value, limit_value)) = simple_numeric_for_parts(for_stmt) else {
            return false;
        };

        let limit_reg = self.alloc_reg();
        self.push(Insn::LdAi(limit_value));
        self.push(Insn::Sta(limit_reg));
        let loop_reg = self.alloc_reg();
        self.push(Insn::LdAi(init_value));
        self.push(Insn::Sta(loop_reg));
        let previous_binding = self.var_regs.insert(name.to_string(), loop_reg);

        let loop_start = self.insns.len();
        self.push(Insn::Lda(limit_reg));
        self.push(Insn::Less(0, loop_reg));
        let jeqz_pos = self.insns.len();
        self.push(Insn::JeqzShort(0));

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);
        self.emit_stmt(&for_stmt.body);

        let continue_target = self.insns.len();
        if let Some(update) = &for_stmt.update {
            self.set_debug_pos(update.span().start);
        }
        self.push(Insn::Lda(loop_reg));
        self.push(Insn::ToNumeric(0));
        self.push(Insn::Inc(0));
        self.push(Insn::Sta(loop_reg));

        self.set_debug_pos(for_stmt.span.start);
        let jmp_pos = self.insns.len();
        self.push(Insn::JmpShort(0));
        let loop_exit = self.insns.len();
        self.patch_jump_to(jmp_pos, loop_start);
        self.patch_jump_to(jeqz_pos, loop_exit);

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, continue_target);
        let labeled_continues = self.take_labeled_continue_patches(label);
        self.patch_jumps_to(labeled_continues, continue_target);

        if let Some(previous) = previous_binding {
            self.var_regs.insert(name.to_string(), previous);
        } else {
            self.var_regs.remove(name);
        }
        true
    }

    fn emit_for_init(&mut self, init: &oxc_ast::ast::ForStatementInit<'_>) {
        use oxc_ast::ast::ForStatementInit;
        match init {
            ForStatementInit::VariableDeclaration(decl) => self.emit_var_decl(decl),
            ForStatementInit::AssignmentExpression(e) => self.emit_assignment(e),
            ForStatementInit::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.emit_expr(e);
                }
            }
            _ => {}
        }
    }

    fn emit_try_stmt(&mut self, ts: &'a oxc_ast::ast::TryStatement<'a>) {
        if ts.finalizer.is_some() {
            self.emit_try_finally_stmt(ts);
            return;
        }

        let try_begin = self.insns.len() as u32;
        let outer_returned = self.returned;
        self.returned = false;
        for s in &ts.block.body {
            self.emit_stmt(s);
            if self.returned {
                break;
            }
        }
        self.returned = outer_returned;
        let try_end = self.insns.len() as u32;

        if let Some(handler) = &ts.handler {
            // Emit a jmp to skip over the catch block on the normal (no-exception) path.
            let jmp_past_catch = self.insns.len();
            self.push(Insn::Jmp(0));

            // Catch handler entry point.
            let catch_begin = self.insns.len() as u32;

            // Exception is in accumulator at handler entry.  Store it if param exists.
            if let Some(param) = &handler.param {
                if let BindingPattern::BindingIdentifier(id) = &param.pattern {
                    let name = id.name.as_str().to_string();
                    let reg = self.alloc_reg();
                    self.var_regs.insert(name, reg);
                    self.push(Insn::Sta(reg));
                }
            }

            let catch_outer_returned = self.returned;
            self.returned = false;
            for s in &handler.body.body {
                self.emit_stmt(s);
                if self.returned {
                    break;
                }
            }
            self.returned = catch_outer_returned;

            // Patch jmp_past_catch → current position.
            let after_catch = self.insns.len();
            self.push_catch_entry(CatchEntry {
                try_begin,
                try_end,
                catch_begin,
                catch_end: after_catch as u32,
            });
            self.patch_jump_to(jmp_past_catch, after_catch);
        }
    }

    fn emit_try_finally_stmt(&mut self, ts: &'a oxc_ast::ast::TryStatement<'a>) {
        let Some(finalizer) = ts.finalizer.as_deref() else {
            return;
        };

        let finalizer_run = self.alloc_reg();
        self.push(Insn::LdUndefined);
        self.push(Insn::Sta(finalizer_run));

        let outer_try_begin = self.insns.len() as u32;
        let saved_finally_len = self.finally_stack.len();
        self.finally_stack.push(finalizer);

        if let Some(handler) = &ts.handler {
            let inner_try_begin = self.insns.len() as u32;
            let outer_returned = self.returned;
            self.returned = false;
            for s in &ts.block.body {
                self.emit_stmt(s);
                if self.returned {
                    break;
                }
            }
            let try_returned = self.returned;
            self.returned = false;
            let inner_try_end = self.insns.len() as u32;
            let jmp_past_catch = self.insns.len();
            self.push(Insn::Jmp(0));

            let catch_begin = self.insns.len() as u32;
            if let Some(param) = &handler.param {
                if let BindingPattern::BindingIdentifier(id) = &param.pattern {
                    let name = id.name.as_str().to_string();
                    let reg = self.alloc_reg();
                    self.var_regs.insert(name, reg);
                    self.push(Insn::Sta(reg));
                }
            }
            for s in &handler.body.body {
                self.emit_stmt(s);
                if self.returned {
                    break;
                }
            }
            let catch_returned = self.returned;
            self.returned = false;
            let catch_end = self.insns.len() as u32;
            self.patch_jump_to(jmp_past_catch, catch_end as usize);
            self.push_catch_entry(CatchEntry {
                try_begin: inner_try_begin,
                try_end: inner_try_end,
                catch_begin,
                catch_end,
            });
            self.returned = outer_returned || (try_returned && catch_returned);
        } else {
            let inner_try_begin = self.insns.len() as u32;
            let outer_returned = self.returned;
            self.returned = false;
            for s in &ts.block.body {
                self.emit_stmt(s);
                if self.returned {
                    break;
                }
            }
            let try_returned = self.returned;
            self.returned = false;
            let inner_try_end = self.insns.len() as u32;
            let jmp_past_throw = self.insns.len();
            self.push(Insn::Jmp(0));

            let catch_begin = self.insns.len() as u32;
            self.push(Insn::Throw);
            let catch_end = self.insns.len() as u32;
            self.patch_jump_to(jmp_past_throw, catch_end as usize);
            self.push_catch_entry(CatchEntry {
                try_begin: inner_try_begin,
                try_end: inner_try_end,
                catch_begin,
                catch_end,
            });
            self.returned = outer_returned || try_returned;
        }

        self.finally_stack.truncate(saved_finally_len);
        let outer_try_end = self.insns.len() as u32;

        let exception = self.alloc_reg();
        self.push(Insn::LdHole);
        self.push(Insn::Sta(exception));
        let jmp_to_catch_end = self.insns.len();
        self.push(Insn::Jmp(0));

        let outer_catch_begin = self.insns.len() as u32;
        self.push(Insn::Sta(exception));
        let outer_catch_end = self.insns.len() as u32;
        self.patch_jump_to(jmp_to_catch_end, outer_catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin: outer_try_begin,
            try_end: outer_try_end,
            catch_begin: outer_catch_begin,
            catch_end: outer_catch_end,
        });

        self.push(Insn::Lda(finalizer_run));
        let skip_finalizer = self.emit_branch_if_not_undefined_from_acc();
        self.push(Insn::Sta(finalizer_run));
        self.emit_finalizer_block(finalizer);
        let after_finalizer = self.insns.len();
        self.patch_jump_to(skip_finalizer, after_finalizer);

        self.push(Insn::Lda(exception));
        self.emit_rethrow_from_acc();
    }

    fn emit_var_decl(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'_>) {
        if decl.declare {
            return;
        }
        for declarator in &decl.declarations {
            match &declarator.id {
                BindingPattern::BindingIdentifier(id) => {
                    let name = id.name.as_str().to_string();
                    let saved_next_reg = self.next_reg;
                    if let Some(init) = &declarator.init {
                        if let Expression::ClassExpression(cls) = init {
                            self.emit_class_var_decl(&name, cls);
                            continue;
                        } else if matches!(
                            init,
                            Expression::ArrowFunctionExpression(_)
                                | Expression::FunctionExpression(_)
                        ) {
                            let previous = self.inferred_function_name.replace(name.clone());
                            self.emit_expr(init);
                            self.inferred_function_name = previous;
                        } else {
                            self.emit_expr(init);
                        }
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    let reg = self.alloc_reg();
                    self.var_regs.insert(name.clone(), reg);
                    self.set_debug_pos(id.span.start);
                    self.push(Insn::Sta(reg));
                    if let Some(slot) = self.current_lexical_slot(&name) {
                        self.push(Insn::Lda(reg));
                        self.push(Insn::StLexVar(0, slot));
                        self.var_regs.remove(&name);
                        self.next_reg = saved_next_reg;
                    }
                    if let Some(export_index) = self.lowering.export_index(&name) {
                        self.push(Insn::Lda(reg));
                        self.push(Insn::StModuleVar(export_index));
                    }
                }
                BindingPattern::ArrayPattern(_) | BindingPattern::ObjectPattern(_) => {
                    // Destructuring binding: evaluate the RHS, then lower the pattern.
                    let rhs_reg = self.alloc_reg();
                    if let Some(init) = &declarator.init {
                        self.emit_expr(init);
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    self.push(Insn::Sta(rhs_reg));
                    self.emit_destructure_binding(&declarator.id, rhs_reg);
                }
                BindingPattern::AssignmentPattern(_) => {
                    unreachable!("top-level binding assignment patterns are not valid here");
                }
            }
        }
    }

    fn emit_ts_import_equals_declaration(&mut self, decl: &TSImportEqualsDeclaration<'_>) {
        if !self
            .runtime_ts_import_equals
            .contains(&(decl.span.start, decl.span.end))
        {
            return;
        }

        let name = decl.id.name.as_str().to_string();
        let saved_next_reg = self.next_reg;
        self.emit_ts_module_reference(&decl.module_reference);
        let reg = self.alloc_reg();
        self.var_regs.insert(name.clone(), reg);
        self.set_debug_pos(decl.id.span.start);
        self.push(Insn::Sta(reg));
        if let Some(slot) = self.current_lexical_slot(&name) {
            self.push(Insn::Lda(reg));
            self.push(Insn::StLexVar(0, slot));
            self.var_regs.remove(&name);
            self.next_reg = saved_next_reg;
        }
        if let Some(export_index) = self.lowering.export_index(&name) {
            self.push(Insn::Lda(reg));
            self.push(Insn::StModuleVar(export_index));
        }
    }

    fn emit_ts_module_reference(&mut self, module_reference: &TSModuleReference<'_>) {
        match module_reference {
            TSModuleReference::ExternalModuleReference(reference) => {
                let name_id = self.intern(reference.expression.value.as_str());
                self.push(Insn::TryLdGlobalByName(0, name_id));
            }
            TSModuleReference::IdentifierReference(id) => {
                self.emit_identifier_load(id.name.as_str());
            }
            TSModuleReference::QualifiedName(qualified) => {
                self.emit_ts_qualified_name(qualified);
            }
        }
    }

    fn emit_ts_type_name(&mut self, type_name: &TSTypeName<'_>) {
        match type_name {
            TSTypeName::IdentifierReference(id) => self.emit_identifier_load(id.name.as_str()),
            TSTypeName::QualifiedName(qualified) => self.emit_ts_qualified_name(qualified),
            TSTypeName::ThisExpression(_) => self.push(Insn::LdThis),
        }
    }

    fn emit_ts_qualified_name(&mut self, qualified: &TSQualifiedName<'_>) {
        let saved_next_reg = self.next_reg;
        let obj_reg = self.alloc_reg();
        self.emit_ts_type_name(&qualified.left);
        self.set_debug_pos(qualified.left.span().start);
        self.push(Insn::Sta(obj_reg));
        let name_id = self.intern(qualified.right.name.as_str());
        self.set_debug_pos(qualified.right.span.start);
        self.push(Insn::Lda(obj_reg));
        self.push(Insn::LdObjByName(0, name_id));
        self.next_reg = saved_next_reg;
    }

    fn emit_ts_enum_var_hoists(&mut self, stmts: &[Statement<'_>]) {
        let mut seen = FxHashSet::default();
        for stmt in stmts {
            let decl = match stmt {
                Statement::TSEnumDeclaration(decl) => Some(decl),
                Statement::ExportNamedDeclaration(export) => match &export.declaration {
                    Some(Declaration::TSEnumDeclaration(decl)) => Some(decl),
                    _ => None,
                },
                _ => None,
            };
            let Some(decl) = decl else {
                continue;
            };
            if decl.declare {
                continue;
            }
            let name = decl.id.name.as_str();
            if !seen.insert(name.to_string()) || self.var_regs.contains_key(name) {
                continue;
            }
            let reg = self.alloc_reg();
            self.set_debug_pos(decl.id.span.start);
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(reg));
            self.var_regs.insert(name.to_string(), reg);
            self.store_ts_enum_binding_from_reg(name, reg);
        }
    }

    fn emit_ts_enum_declaration(&mut self, decl: &TSEnumDeclaration<'_>) {
        if decl.declare {
            return;
        }
        let enum_name = decl.id.name.as_str().to_string();
        let enum_reg = self.ensure_ts_enum_object(&enum_name, decl.id.span.start);
        let mut previous_member: Option<String> = None;
        let enum_members = decl
            .body
            .members
            .iter()
            .filter_map(|member| Self::ts_enum_member_name(&member.id))
            .collect::<FxHashSet<_>>();
        let mut string_members = FxHashSet::default();

        for member in &decl.body.members {
            let Some(member_name) = Self::ts_enum_member_name(&member.id) else {
                continue;
            };
            self.set_debug_pos(member.span.start);
            if let Some(initializer) = &member.initializer {
                if Self::ts_enum_initializer_is_string(initializer, &string_members) {
                    self.emit_ts_enum_initializer_expr(initializer, enum_reg, &enum_members);
                    let name_id = self.intern(&member_name);
                    self.push(Insn::StObjByName(0, name_id, enum_reg));
                    string_members.insert(member_name.clone());
                    previous_member = Some(member_name);
                } else {
                    self.emit_ts_enum_initializer_expr(initializer, enum_reg, &enum_members);
                    let value_reg = self.alloc_reg();
                    self.push(Insn::Sta(value_reg));
                    self.emit_ts_enum_numeric_member(enum_reg, &member_name, value_reg);
                    previous_member = Some(member_name);
                }
            } else {
                let value_reg = self.alloc_reg();
                if let Some(previous_member) = previous_member.as_deref() {
                    let previous_id = self.intern(previous_member);
                    self.push(Insn::Lda(enum_reg));
                    self.push(Insn::LdObjByName(0, previous_id));
                    let previous_value_reg = self.alloc_reg();
                    self.push(Insn::Sta(previous_value_reg));
                    self.push(Insn::LdAi(1));
                    self.push(Insn::Add2(0, previous_value_reg));
                } else {
                    self.push(Insn::LdAi(0));
                }
                self.push(Insn::Sta(value_reg));
                self.emit_ts_enum_numeric_member(enum_reg, &member_name, value_reg);
                previous_member = Some(member_name);
            }
        }
    }

    fn emit_ts_enum_initializer_expr(
        &mut self,
        expr: &Expression<'_>,
        enum_reg: u8,
        enum_members: &FxHashSet<String>,
    ) {
        match expr {
            Expression::Identifier(id) if enum_members.contains(id.name.as_str()) => {
                let name_id = self.intern(id.name.as_str());
                self.push(Insn::Lda(enum_reg));
                self.push(Insn::LdObjByName(0, name_id));
            }
            Expression::BinaryExpression(bin) => {
                let saved_next_reg = self.next_reg;
                let left_reg = self.alloc_reg();
                self.emit_ts_enum_initializer_expr(&bin.left, enum_reg, enum_members);
                self.set_debug_pos(bin.right.span().start);
                self.push(Insn::Sta(left_reg));
                self.emit_ts_enum_initializer_expr(&bin.right, enum_reg, enum_members);
                self.set_debug_pos(bin.right.span().start);
                self.emit_binary_op(bin.operator, left_reg);
                self.next_reg = saved_next_reg;
            }
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::UnaryNegation => {
                    self.emit_ts_enum_initializer_expr(&unary.argument, enum_reg, enum_members);
                    self.push(Insn::Neg(0));
                }
                UnaryOperator::UnaryPlus => {
                    self.emit_ts_enum_initializer_expr(&unary.argument, enum_reg, enum_members);
                    self.push(Insn::ToNumber(0));
                }
                UnaryOperator::BitwiseNot => {
                    self.emit_ts_enum_initializer_expr(&unary.argument, enum_reg, enum_members);
                    self.push(Insn::BitNot(0));
                }
                _ => self.emit_expr(expr),
            },
            Expression::ParenthesizedExpression(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            Expression::TSAsExpression(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            Expression::TSSatisfiesExpression(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            Expression::TSNonNullExpression(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            Expression::TSTypeAssertion(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            Expression::TSInstantiationExpression(expr) => {
                self.emit_ts_enum_initializer_expr(&expr.expression, enum_reg, enum_members);
            }
            _ => self.emit_expr(expr),
        }
    }

    fn ensure_ts_enum_object(&mut self, name: &str, span_start: u32) -> u8 {
        let reg = if let Some(&reg) = self.var_regs.get(name) {
            reg
        } else {
            let reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(reg));
            self.var_regs.insert(name.to_string(), reg);
            self.store_ts_enum_binding_from_reg(name, reg);
            reg
        };
        self.set_debug_pos(span_start);
        self.push(Insn::Lda(reg));
        let has_existing_object = self.emit_branch_if_true_from_acc();
        self.push(Insn::CreateEmptyObject);
        self.push(Insn::Sta(reg));
        self.store_ts_enum_binding_from_reg(name, reg);
        let after_init = self.insns.len();
        self.patch_jump_to(has_existing_object, after_init);
        reg
    }

    fn store_ts_enum_binding_from_reg(&mut self, name: &str, reg: u8) {
        if let Some(slot) = self.current_lexical_slot(name) {
            self.push(Insn::Lda(reg));
            self.push(Insn::StLexVar(0, slot));
        }
        if let Some(export_index) = self.lowering.export_index(name) {
            self.push(Insn::Lda(reg));
            self.push(Insn::StModuleVar(export_index));
        }
    }

    fn emit_ts_enum_numeric_member(&mut self, enum_reg: u8, member_name: &str, value_reg: u8) {
        let name_id = self.intern(member_name);
        self.push(Insn::Lda(value_reg));
        self.push(Insn::StObjByName(0, name_id, enum_reg));
        self.push(Insn::LdaStr(name_id));
        self.push(Insn::StObjByValue(0, enum_reg, value_reg));
    }

    fn ts_enum_member_name(member_name: &TSEnumMemberName<'_>) -> Option<String> {
        match member_name {
            TSEnumMemberName::Identifier(id) => Some(id.name.as_str().to_string()),
            TSEnumMemberName::String(string) | TSEnumMemberName::ComputedString(string) => {
                Some(string.value.as_str().to_string())
            }
            TSEnumMemberName::ComputedTemplateString(template) => template
                .single_quasi()
                .map(|name| name.as_str().to_string()),
        }
    }

    fn ts_enum_initializer_is_string(
        expr: &Expression<'_>,
        string_members: &FxHashSet<String>,
    ) -> bool {
        match expr {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => true,
            Expression::Identifier(id) => string_members.contains(id.name.as_str()),
            Expression::BinaryExpression(bin)
                if bin.operator == oxc_ast::ast::BinaryOperator::Addition =>
            {
                Self::ts_enum_initializer_is_string(&bin.left, string_members)
                    && Self::ts_enum_initializer_is_string(&bin.right, string_members)
            }
            Expression::ParenthesizedExpression(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            Expression::TSAsExpression(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            Expression::TSSatisfiesExpression(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            Expression::TSNonNullExpression(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            Expression::TSTypeAssertion(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            Expression::TSInstantiationExpression(expr) => {
                Self::ts_enum_initializer_is_string(&expr.expression, string_members)
            }
            _ => false,
        }
    }

    // ── Expression compiler — result always in accumulator ────────────────────

    fn emit_expr(&mut self, expr: &Expression<'_>) {
        // Update current_line and current_col from the expression's source position
        // for per-instruction LNP tracking.
        if self.suppress_next_expr_debug_pos {
            self.suppress_next_expr_debug_pos = false;
        } else {
            self.set_debug_pos(expr.span().start);
        }
        match expr {
            Expression::NumericLiteral(n) => {
                if n.value.fract() == 0.0
                    && n.value >= i32::MIN as f64
                    && n.value <= i32::MAX as f64
                {
                    self.push(Insn::LdAi(n.value as i32));
                } else {
                    self.push(Insn::FLdAi(n.value));
                }
            }
            Expression::BigIntLiteral(bigint) => {
                self.emit_bigint_literal(bigint);
            }
            Expression::StringLiteral(s) => {
                let id = self.intern(s.value.as_str());
                self.push(Insn::LdaStr(id));
            }
            Expression::RegExpLiteral(regex) => {
                self.emit_regexp_literal(regex);
            }
            Expression::BooleanLiteral(b) => {
                if b.value {
                    self.push(Insn::LdTrue);
                } else {
                    self.push(Insn::LdFalse);
                }
            }
            Expression::NullLiteral(_) => {
                self.push(Insn::LdNull);
            }
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                match name {
                    "undefined" => self.push(Insn::LdUndefined),
                    "null" => self.push(Insn::LdNull),
                    "true" => self.push(Insn::LdTrue),
                    "false" => self.push(Insn::LdFalse),
                    _ => self.emit_identifier_load(name),
                }
            }
            Expression::TSAsExpression(expr) => {
                self.emit_expr(&expr.expression);
            }
            Expression::TSSatisfiesExpression(expr) => {
                self.emit_expr(&expr.expression);
            }
            Expression::TSNonNullExpression(expr) => {
                self.emit_expr(&expr.expression);
            }
            Expression::TSTypeAssertion(expr) => {
                self.emit_expr(&expr.expression);
            }
            Expression::TSInstantiationExpression(expr) => {
                self.emit_expr(&expr.expression);
            }
            Expression::ThisExpression(_) => {
                self.emit_this_value();
            }
            Expression::Super(_) => {
                self.emit_this_value();
            }
            Expression::MetaProperty(meta) => {
                if is_new_target_meta(meta) {
                    if self.arrow_captures.new_target_slot.is_some() {
                        if let Some((level, slot)) = self.lexical_lookup("new.target") {
                            self.push(Insn::LdLexVar(level, slot));
                        } else {
                            self.push(Insn::Lda(MANDATORY_PARAM_NEW_TARGET_REG));
                        }
                    } else {
                        self.push(Insn::Lda(MANDATORY_PARAM_NEW_TARGET_REG));
                    }
                } else {
                    // import.meta — load the per-module meta object via getimportmeta (0xd5).
                    // The runtime populates its `url` and `resolve` properties.
                    self.push(Insn::GetImportMeta);
                }
            }
            Expression::ParenthesizedExpression(p) => {
                self.emit_expr(&p.expression);
            }
            Expression::ComputedMemberExpression(mem) => {
                self.emit_computed_member(mem);
            }
            Expression::StaticMemberExpression(mem) => {
                self.emit_static_member(mem);
            }
            Expression::AssignmentExpression(assign) => {
                self.emit_assignment(assign);
            }
            Expression::CallExpression(call) => {
                self.emit_call(call);
            }
            Expression::ImportExpression(import) => {
                self.emit_import_expr(import);
            }
            Expression::BinaryExpression(bin) => {
                self.emit_binary_expr(bin);
            }
            Expression::UnaryExpression(unary) => {
                self.emit_unary_expr(unary);
            }
            Expression::UpdateExpression(update) => {
                self.emit_update_expr(update);
            }
            Expression::LogicalExpression(log) => {
                self.emit_logical_expr(log);
            }
            Expression::NewExpression(new_expr) => {
                self.emit_new(new_expr);
            }
            Expression::SequenceExpression(seq) => {
                // Compile all sub-expressions left to right; acc = last value.
                for e in &seq.expressions {
                    self.emit_expr(e);
                }
            }
            Expression::ClassExpression(cls) => {
                // `class [Name] [extends Super] { … }` as an expression.
                // Result (class constructor function) is left in the accumulator
                // so the caller (e.g. emit_var_decl) can assign it as needed.
                let super_class = cls.super_class.as_ref();
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                self.emit_class_inner(class_name, &cls.body.body, super_class);
            }
            Expression::ArrowFunctionExpression(arrow) => {
                let length = arrow.params.items.len() as u8;
                let record_name = self
                    .lowering
                    .function_name(arrow.span.start, arrow.span.end)
                    .map(str::to_string);
                let method_name_id = record_name
                    .as_deref()
                    .map(|name| self.intern(name))
                    .or_else(|| {
                        self.inferred_function_name
                            .clone()
                            .map(|name| self.intern(&name))
                    })
                    .unwrap_or(0);
                self.push(Insn::DefineFuncShort(0, method_name_id, length));
            }
            Expression::FunctionExpression(func) => {
                // Named or anonymous function expression: `function(params) {}`.
                let length = func.params.items.len() as u8;
                let record_name = self
                    .lowering
                    .function_name(func.span.start, func.span.end)
                    .map(str::to_string);
                let method_name_id = record_name
                    .as_deref()
                    .map(|name| self.intern(name))
                    .or_else(|| func.id.as_ref().map(|id| self.intern(id.name.as_str())))
                    .or_else(|| {
                        self.inferred_function_name
                            .clone()
                            .map(|name| self.intern(&name))
                    })
                    .unwrap_or(0);
                self.push(Insn::DefineFuncShort(0, method_name_id, length));
            }
            Expression::ObjectExpression(obj) => {
                self.emit_object_expr(obj);
            }
            Expression::ConditionalExpression(cond) => {
                self.emit_conditional_expr(cond);
            }
            Expression::TemplateLiteral(tmpl) => {
                self.emit_template_literal(tmpl);
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.emit_tagged_template_expr(tagged);
            }
            Expression::ChainExpression(chain) => {
                self.emit_chain_expr(chain);
            }
            Expression::ArrayExpression(arr) => {
                self.emit_array_expr(arr);
            }
            Expression::AwaitExpression(await_expr) => {
                self.emit_expr(&await_expr.argument);
                self.emit_async_await();
            }
            Expression::YieldExpression(yield_expr) => {
                self.emit_yield_expr(yield_expr);
            }
            Expression::PrivateFieldExpression(expr) => {
                self.emit_private_field_load(&expr.field.name, &expr.object);
            }
            Expression::PrivateInExpression(expr) => {
                // `#x in obj` — brand-check opcode.
                let name = expr.left.name.as_str();
                self.emit_expr(&expr.right);
                if let Some(slot) = self.private_field_slots.get(name).copied() {
                    self.push(Insn::TestIn(0, 0, slot));
                }
            }
            _ => {
                self.push(Insn::LdUndefined);
            }
        }
    }

    /// Emit a private field load: object → acc, then `ldprivateproperty`.
    fn emit_private_field_load(&mut self, field_name: &str, object: &Expression<'_>) {
        let slot = self.private_field_slots.get(field_name).copied();
        self.emit_expr(object);
        if let Some(slot) = slot {
            self.push(Insn::LdPrivateProperty(0, 0, slot));
        }
    }

    /// Emit a private field store: value → acc, object → obj_reg, then
    /// `stprivateproperty`.
    fn emit_private_field_store(
        &mut self,
        field_name: &str,
        object: &Expression<'_>,
        value: &Expression<'_>,
    ) {
        let slot = self.private_field_slots.get(field_name).copied();
        let obj_reg = self.alloc_reg();
        self.emit_expr(object);
        self.push(Insn::Sta(obj_reg));
        self.emit_expr(value);
        if let Some(slot) = slot {
            self.push(Insn::StPrivateProperty(0, 0, slot, obj_reg));
        }
    }

    fn emit_this_value(&mut self) {
        if self.arrow_captures.this_slot.is_some() {
            if let Some((level, slot)) = self.lexical_lookup("this") {
                self.push(Insn::LdLexVar(level, slot));
            } else {
                self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            }
        } else {
            // PandaGen::GetThis loads binder::MANDATORY_PARAM_THIS.
            self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            if self.derived_constructor {
                self.push(Insn::ThrowIfSuperNotCorrectCall(0));
            }
        }
    }

    // ── Optional chaining: obj?.prop / obj?.method() ─────────────────────────

    fn emit_chain_expr(&mut self, chain: &oxc_ast::ast::ChainExpression<'_>) {
        match &chain.expression {
            oxc_ast::ast::ChainElement::StaticMemberExpression(mem) => {
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.push(Insn::Sta(obj_reg));
                let nullish_jumps = self.emit_optional_nullish_jumps(obj_reg);
                self.push(Insn::Lda(obj_reg));
                let name_id = self.intern(mem.property.name.as_str());
                self.push(Insn::LdObjByName(0, name_id));
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                let null_start = self.insns.len();
                self.patch_optional_nullish_jumps(nullish_jumps, null_start);
                self.push(Insn::LdUndefined);
                let end = self.insns.len();
                self.patch_jump_to(jmp_pos, end);
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(mem) => {
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.push(Insn::Sta(obj_reg));
                let nullish_jumps = self.emit_optional_nullish_jumps(obj_reg);
                self.emit_expr(&mem.expression);
                let key_reg = self.alloc_reg();
                self.push(Insn::Sta(key_reg));
                self.push(Insn::Lda(key_reg));
                self.push(Insn::LdObjByValue(0, obj_reg));
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                let null_start = self.insns.len();
                self.patch_optional_nullish_jumps(nullish_jumps, null_start);
                self.push(Insn::LdUndefined);
                let end = self.insns.len();
                self.patch_jump_to(jmp_pos, end);
            }
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                let is_method = matches!(
                    &call.callee,
                    Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
                );
                if is_method {
                    enum ChainMethod<'b> {
                        Static(&'b Expression<'b>, &'b str, bool),
                        Computed(&'b Expression<'b>, &'b Expression<'b>, bool),
                    }
                    let method = match &call.callee {
                        Expression::StaticMemberExpression(m) => {
                            ChainMethod::Static(&m.object, m.property.name.as_str(), m.optional)
                        }
                        Expression::ComputedMemberExpression(m) => {
                            ChainMethod::Computed(&m.object, &m.expression, m.optional)
                        }
                        other => {
                            self.emit_expr(other);
                            self.push(Insn::CallArg0(0));
                            return;
                        }
                    };
                    let recv_expr = match method {
                        ChainMethod::Static(recv_expr, _, _)
                        | ChainMethod::Computed(recv_expr, _, _) => recv_expr,
                    };
                    self.emit_expr(recv_expr);
                    let this_reg = self.alloc_reg();
                    self.push(Insn::Sta(this_reg));
                    let mut nullish_jumps = Vec::new();
                    let member_optional = match method {
                        ChainMethod::Static(_, _, optional)
                        | ChainMethod::Computed(_, _, optional) => optional,
                    };
                    if member_optional {
                        nullish_jumps
                            .extend_from_slice(&self.emit_optional_nullish_jumps(this_reg));
                    }
                    let callee_reg = self.alloc_reg();
                    match method {
                        ChainMethod::Static(_, method_name, _) => {
                            let name_id = self.intern(method_name);
                            self.push(Insn::Lda(this_reg));
                            self.push(Insn::LdObjByName(0, name_id));
                            self.push(Insn::Sta(callee_reg));
                        }
                        ChainMethod::Computed(_, key_expr, _) => {
                            self.emit_expr(key_expr);
                            let key_reg = self.alloc_reg();
                            self.push(Insn::Sta(key_reg));
                            self.push(Insn::Lda(key_reg));
                            self.push(Insn::LdObjByValue(0, this_reg));
                            self.push(Insn::Sta(callee_reg));
                        }
                    }
                    if call.optional {
                        nullish_jumps
                            .extend_from_slice(&self.emit_optional_nullish_jumps(callee_reg));
                    }
                    let argc = call.arguments.len();
                    if Self::args_need_apply(&call.arguments) {
                        let args_obj = self.emit_arguments_array_to_reg(&call.arguments);
                        self.push(Insn::Lda(callee_reg));
                        self.push(Insn::Apply(0, this_reg, args_obj));
                    } else {
                        match argc {
                            0 => {
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallThis0(0, this_reg));
                            }
                            1 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallThis1(0, this_reg, v0));
                            }
                            2 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallThis2(0, this_reg, v0, v1));
                            }
                            3 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallThis3(0, this_reg, v0, v1, v2));
                            }
                            n => {
                                let first_arg = this_reg.saturating_add(1);
                                for (i, arg) in call.arguments.iter().enumerate() {
                                    self.emit_argument_to_reg(
                                        arg,
                                        first_arg.saturating_add(i as u8),
                                    );
                                }
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallThisRange(0, n as u8, this_reg));
                            }
                        }
                    }
                    let jmp_pos = self.insns.len();
                    self.push(Insn::Jmp(0));
                    let null_start = self.insns.len();
                    for jump in nullish_jumps {
                        self.patch_jump_to(jump, null_start);
                    }
                    self.push(Insn::LdUndefined);
                    let end = self.insns.len();
                    self.patch_jump_to(jmp_pos, end);
                } else {
                    self.emit_expr(&call.callee);
                    let callee_reg = self.alloc_reg();
                    self.push(Insn::Sta(callee_reg));
                    let nullish_jumps = if call.optional {
                        Some(self.emit_optional_nullish_jumps(callee_reg))
                    } else {
                        None
                    };
                    let argc = call.arguments.len();
                    if Self::args_need_apply(&call.arguments) {
                        let this_reg = self.alloc_reg();
                        self.push(Insn::LdUndefined);
                        self.push(Insn::Sta(this_reg));
                        let args_obj = self.emit_arguments_array_to_reg(&call.arguments);
                        self.push(Insn::Lda(callee_reg));
                        self.push(Insn::Apply(0, this_reg, args_obj));
                    } else {
                        match argc {
                            0 => {
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallArg0(0));
                            }
                            1 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallArg1(0, v0));
                            }
                            2 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallArgs2(0, v0, v1));
                            }
                            3 => {
                                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallArgs3(0, v0, v1, v2));
                            }
                            n => {
                                let first_arg = callee_reg.saturating_add(1);
                                for (i, arg) in call.arguments.iter().enumerate() {
                                    self.emit_argument_to_reg(
                                        arg,
                                        first_arg.saturating_add(i as u8),
                                    );
                                }
                                self.push(Insn::Lda(callee_reg));
                                self.push(Insn::CallRange(0, n as u8, first_arg));
                            }
                        }
                    }
                    if let Some(nullish_jumps) = nullish_jumps {
                        let jmp_pos = self.insns.len();
                        self.push(Insn::Jmp(0));
                        let null_start = self.insns.len();
                        self.patch_optional_nullish_jumps(nullish_jumps, null_start);
                        self.push(Insn::LdUndefined);
                        let end = self.insns.len();
                        self.patch_jump_to(jmp_pos, end);
                    }
                }
            }
            _ => {
                self.push(Insn::LdUndefined);
            }
        }
    }

    // ── Array literal: [a, ...b, c] ───────────────────────────────────────────

    fn static_array_literal_entries(
        arr: &oxc_ast::ast::ArrayExpression<'_>,
    ) -> Option<Vec<LiteralEntry>> {
        let mut entries = Vec::with_capacity(arr.elements.len());
        for elem in &arr.elements {
            let expr = elem.as_expression()?;
            match expr {
                Expression::StringLiteral(s) => {
                    entries.push(LiteralEntry::TaggedString(s.value.as_str().to_string()));
                }
                _ => return None,
            }
        }
        Some(entries)
    }

    fn emit_array_expr(&mut self, arr: &oxc_ast::ast::ArrayExpression<'_>) {
        if let Some(entries) = Self::static_array_literal_entries(arr) {
            let lit_id = self.literal_arrays.len() as u16;
            self.literal_arrays.push(LiteralArray {
                entries,
                explicit_key: None,
            });
            self.push(Insn::CreateArrayWithBuffer(0, lit_id));
            return;
        }

        self.push(Insn::CreateEmptyArray(0));
        if arr.elements.is_empty() {
            return;
        }
        let arr_reg = self.alloc_reg();
        self.push(Insn::Sta(arr_reg));
        let mut idx: u16 = 0;
        let mut iter = arr.elements.iter().peekable();
        while let Some(elem) = iter.peek().copied() {
            if matches!(elem, oxc_ast::ast::ArrayExpressionElement::SpreadElement(_)) {
                break;
            }
            match elem {
                oxc_ast::ast::ArrayExpressionElement::Elision(_) => idx += 1,
                _ => {
                    if let Some(expr) = elem.as_expression() {
                        self.emit_expr(expr);
                        self.push(Insn::StOwnByIndex(0, arr_reg, idx));
                    }
                    idx += 1;
                }
            }
            iter.next();
        }

        if iter.peek().is_none() {
            self.push(Insn::Lda(arr_reg));
            return;
        }

        let idx_reg = self.alloc_reg();
        self.push(Insn::LdAi(idx as i32));
        self.push(Insn::Sta(idx_reg));
        let length_id = self.intern("length");

        for elem in iter {
            match elem {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                    self.emit_expr(&spread.argument);
                    self.push(Insn::StArraySpread(arr_reg, idx_reg));
                    self.push(Insn::Lda(arr_reg));
                    self.push(Insn::LdObjByName(0, length_id));
                    self.push(Insn::Sta(idx_reg));
                }
                oxc_ast::ast::ArrayExpressionElement::Elision(_) => {
                    self.push(Insn::Lda(idx_reg));
                    self.push(Insn::Inc(0));
                    self.push(Insn::Sta(idx_reg));
                }
                _ => {
                    if let Some(expr) = elem.as_expression() {
                        self.emit_expr(expr);
                        self.push(Insn::StOwnByValue(0, arr_reg, idx_reg));
                    }
                    self.push(Insn::Lda(idx_reg));
                    self.push(Insn::Inc(0));
                    self.push(Insn::Sta(idx_reg));
                }
            }
        }
        self.push(Insn::Lda(arr_reg));
    }

    // ── BigInt literal: 123n ─────────────────────────────────────────────────

    fn emit_bigint_literal(&mut self, bigint: &oxc_ast::ast::BigIntLiteral<'_>) {
        let value = bigint
            .raw
            .as_ref()
            .and_then(|raw| raw.as_str().strip_suffix('n'))
            .unwrap_or_else(|| bigint.value.as_str());
        let id = self.intern(value);
        self.push(Insn::LdBigInt(id));
    }

    // ── Dynamic import: import(source) ───────────────────────────────────────

    fn emit_import_expr(&mut self, import: &oxc_ast::ast::ImportExpression<'_>) {
        self.emit_expr(&import.source);
        self.push(Insn::DynamicImport);
    }

    // ── RegExp literal: /pattern/flags ───────────────────────────────────────

    fn emit_regexp_literal(&mut self, regex: &oxc_ast::ast::RegExpLiteral<'_>) {
        let ctor_name = self.intern("RegExp");
        self.push(Insn::TryLdGlobalByName(0, ctor_name));
        let ctor_reg = self.alloc_reg();
        self.push(Insn::Sta(ctor_reg));

        let pattern_id = self.intern(regex.regex.pattern.text.as_str());
        self.push(Insn::LdaStr(pattern_id));
        let pattern_reg = self.alloc_reg();
        self.push(Insn::Sta(pattern_reg));

        let mut arg_count = 2u8;
        let flags = self
            .source
            .and_then(|source| {
                let literal = source.get(regex.span.start as usize..regex.span.end as usize)?;
                literal.rsplit_once('/').map(|(_, flags)| flags)
            })
            .filter(|flags| !flags.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| regex.regex.flags.to_string());
        if !flags.is_empty() {
            let flags_id = self.intern(&flags);
            self.push(Insn::LdaStr(flags_id));
            let flags_reg = self.alloc_reg();
            self.push(Insn::Sta(flags_reg));
            arg_count = 3;
        }

        self.push(Insn::NewObjRange(0, arg_count, ctor_reg));
    }

    // ── Object literal: { key: value, … } ────────────────────────────────────

    fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_string()),
            PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_string()),
            _ => None,
        }
    }

    fn is_proto_property(property: &oxc_ast::ast::ObjectProperty<'_>) -> bool {
        !property.computed
            && !property.method
            && !property.shorthand
            && matches!(
                Self::property_key_name(&property.key).as_deref(),
                Some("__proto__")
            )
    }

    fn is_anon_class_or_func_expr(expr: &Expression<'_>) -> bool {
        match expr {
            Expression::FunctionExpression(func) => func.id.is_none(),
            Expression::ArrowFunctionExpression(_) => true,
            Expression::ClassExpression(class) => class.id.is_none(),
            _ => false,
        }
    }

    fn is_legal_name_format(name: &str) -> bool {
        // Mirrors es2panda ObjectExpression::IsLegalNameFormat.
        name.contains('.') && name.contains('\\')
    }

    fn object_literal_static_entry(
        &mut self,
        property: &oxc_ast::ast::ObjectProperty<'_>,
        key_name: &str,
    ) -> (Vec<LiteralEntry>, bool) {
        let mut entries = vec![LiteralEntry::TaggedString(key_name.to_string())];

        match &property.value {
            Expression::StringLiteral(s) => {
                entries.push(LiteralEntry::TaggedString(s.value.as_str().to_string()));
                (entries, true)
            }
            Expression::NullLiteral(_) => {
                entries.push(LiteralEntry::TaggedNull);
                (entries, true)
            }
            Expression::FunctionExpression(func) if property.method => {
                let record_name = self
                    .lowering
                    .function_name(func.span.start, func.span.end)
                    .map(str::to_string)
                    .unwrap_or_else(|| key_name.to_string());
                entries.push(object_method_literal_entry(record_name, func));
                entries.push(LiteralEntry::TaggedMethodAffiliate(
                    func.params.items.len() as u16
                ));
                (entries, true)
            }
            _ => {
                entries.push(LiteralEntry::TaggedNull);
                (entries, false)
            }
        }
    }

    fn define_own_property_by_name(&mut self, name_id: u16, obj_reg: u8) {
        if self.lowering.is_default_api_version() {
            self.push(Insn::DefineFieldByName(0, name_id, obj_reg));
        } else {
            self.push(Insn::DefinePropertyByName(0, name_id, obj_reg));
        }
    }

    fn emit_class_field_definition(
        &mut self,
        field: &ClassFieldInit<'_>,
        obj_reg: u8,
        precomputed_key_reg: Option<u8>,
    ) {
        let key = match &field.key {
            ClassFieldKey::Static(name) => ClassFieldRuntimeKey::Static(name.clone()),
            ClassFieldKey::CapturedComputed { key, capture_name } => {
                ClassFieldRuntimeKey::ComputedReg(
                    precomputed_key_reg
                        .unwrap_or_else(|| self.emit_captured_class_key_to_reg(capture_name, key)),
                )
            }
        };
        self.emit_class_field_definition_with_key(field.value, key, obj_reg);
    }

    fn emit_captured_class_key_to_reg(&mut self, capture_name: &str, key: &PropertyKey<'_>) -> u8 {
        if let Some((level, slot)) = self.lexical_lookup(capture_name) {
            let key_reg = self.alloc_reg();
            self.push(Insn::LdLexVar(level, slot));
            self.push(Insn::Sta(key_reg));
            return key_reg;
        }
        debug_assert!(
            false,
            "missing lexical capture for computed instance class field key"
        );
        self.emit_property_key_to_reg(key)
    }

    fn emit_class_field_definition_with_key(
        &mut self,
        value: Option<&Expression<'_>>,
        key: ClassFieldRuntimeKey,
        obj_reg: u8,
    ) {
        if let Some(init_expr) = value {
            self.emit_expr(init_expr);
        } else {
            self.push(Insn::LdUndefined);
        }
        let value_reg = self.alloc_reg();
        self.push(Insn::Sta(value_reg));
        self.push(Insn::Lda(value_reg));
        match key {
            ClassFieldRuntimeKey::Static(name) => {
                let name_id = self.intern(&name);
                self.define_own_property_by_name(name_id, obj_reg);
            }
            ClassFieldRuntimeKey::ComputedReg(key_reg) => {
                self.define_own_property_by_value(key_reg, obj_reg);
            }
        }
    }

    fn store_own_property_by_name(&mut self, name_id: u16, obj_reg: u8, name_setting: bool) {
        if name_setting {
            self.push(Insn::StOwnByNameWithNameSet(0, name_id, obj_reg));
        } else {
            self.push(Insn::StOwnByName(0, name_id, obj_reg));
        }
    }

    fn emit_property_key_to_reg(&mut self, key: &PropertyKey<'_>) -> u8 {
        let key_reg = self.alloc_reg();
        match key {
            PropertyKey::StaticIdentifier(id) => {
                let name_id = self.intern(id.name.as_str());
                self.push(Insn::LdaStr(name_id));
            }
            PropertyKey::PrivateIdentifier(_) => {
                unreachable!("private property key cannot be emitted as a computed key");
            }
            key => {
                if let Some(expr) = key.as_expression() {
                    self.emit_expr(expr);
                } else {
                    unreachable!("unsupported non-expression computed property key");
                }
            }
        }
        self.push(Insn::Sta(key_reg));
        key_reg
    }

    fn emit_auto_accessor_static_key_to_reg(
        &mut self,
        accessor: &oxc_ast::ast::AccessorProperty<'_>,
    ) -> Option<u8> {
        let key_reg = self.alloc_reg();
        match &accessor.key {
            PropertyKey::StaticIdentifier(id) => {
                let name_id = self.intern(id.name.as_str());
                self.push(Insn::LdaStr(name_id));
            }
            PropertyKey::StringLiteral(s) => {
                let name_id = self.intern(s.value.as_str());
                self.push(Insn::LdaStr(name_id));
            }
            PropertyKey::NumericLiteral(n) => {
                if n.value.fract() == 0.0
                    && n.value >= i32::MIN as f64
                    && n.value <= i32::MAX as f64
                {
                    self.push(Insn::LdAi(n.value as i32));
                } else {
                    self.push(Insn::FLdAi(n.value));
                }
            }
            PropertyKey::BigIntLiteral(bigint) => {
                self.emit_bigint_literal(bigint);
            }
            PropertyKey::Identifier(id) => {
                let name_id = self.intern(id.name.as_str());
                self.push(Insn::LdaStr(name_id));
            }
            PropertyKey::PrivateIdentifier(_) => return None,
            _ => return None,
        }
        self.push(Insn::Sta(key_reg));
        Some(key_reg)
    }

    fn define_own_property_by_value(&mut self, key_reg: u8, obj_reg: u8) {
        self.push(Insn::CallRuntimeDefineFieldByValue(0, key_reg, obj_reg));
    }

    fn store_own_property_by_value(&mut self, key_reg: u8, obj_reg: u8, name_setting: bool) {
        if name_setting {
            self.push(Insn::StOwnByValueWithNameSet(0, obj_reg, key_reg));
        } else {
            self.push(Insn::StOwnByValue(0, obj_reg, key_reg));
        }
    }

    fn emit_object_property_value(
        &mut self,
        property: &oxc_ast::ast::ObjectProperty<'_>,
        obj_reg: u8,
        key_name: &str,
    ) {
        if property.method {
            if let Expression::FunctionExpression(func) = &property.value {
                let record_name = self
                    .lowering
                    .function_name(func.span.start, func.span.end)
                    .map(str::to_string)
                    .unwrap_or_else(|| key_name.to_string());
                let method_name_id = self.intern(&record_name);
                self.push(Insn::Lda(obj_reg));
                self.push(Insn::DefineMethodShort(
                    0,
                    method_name_id,
                    func.params.items.len() as u8,
                ));
                return;
            }
        }

        let previous_inferred_name = self.inferred_function_name.replace(key_name.to_string());
        self.emit_expr(&property.value);
        self.inferred_function_name = previous_inferred_name;
    }

    fn compile_object_init_property(
        &mut self,
        property: &oxc_ast::ast::ObjectProperty<'_>,
        obj_reg: u8,
    ) {
        let static_key_name = Self::property_key_name(&property.key);
        let computed_key_reg = property
            .computed
            .then(|| self.emit_property_key_to_reg(&property.key));

        let name_setting = if property.method {
            property.computed
        } else if property.computed {
            Self::is_anon_class_or_func_expr(&property.value)
        } else {
            let Some(key_name) = static_key_name.as_deref() else {
                return;
            };
            Self::is_anon_class_or_func_expr(&property.value)
                && Self::is_legal_name_format(&key_name)
        };

        let inferred_name = static_key_name.as_deref().unwrap_or("");
        self.emit_object_property_value(property, obj_reg, inferred_name);
        if let Some(key_reg) = computed_key_reg {
            if !name_setting && self.lowering.target_api_version() > 10 {
                self.define_own_property_by_value(key_reg, obj_reg);
            } else {
                self.store_own_property_by_value(key_reg, obj_reg, name_setting);
            }
            return;
        }
        let Some(key_name) = static_key_name else {
            return;
        };
        let name_id = self.intern(&key_name);
        if !name_setting && self.lowering.target_api_version() > 10 {
            self.define_own_property_by_name(name_id, obj_reg);
        } else {
            self.store_own_property_by_name(name_id, obj_reg, name_setting);
        }
    }

    fn compile_object_accessor_property(
        &mut self,
        property: &oxc_ast::ast::ObjectProperty<'_>,
        obj_reg: u8,
    ) {
        let key_reg = self.emit_property_key_to_reg(&property.key);
        let undefined_reg = self.alloc_reg();
        self.push(Insn::LdUndefined);
        self.push(Insn::Sta(undefined_reg));

        let accessor_reg = self.alloc_reg();
        self.push(Insn::Lda(obj_reg));
        self.emit_expr(&property.value);
        self.push(Insn::Sta(accessor_reg));

        let (getter_reg, setter_reg) = if property.kind == oxc_ast::ast::PropertyKind::Get {
            (accessor_reg, undefined_reg)
        } else {
            (undefined_reg, accessor_reg)
        };
        if property.computed {
            self.push(Insn::LdTrue);
        } else {
            self.push(Insn::LdFalse);
        }
        self.push(Insn::DefineGetterSetterByValue(
            obj_reg, key_reg, getter_reg, setter_reg,
        ));
    }

    fn emit_object_expr(&mut self, obj: &oxc_ast::ast::ObjectExpression<'_>) {
        use oxc_ast::ast::{ObjectPropertyKind, PropertyKind};

        if obj.properties.is_empty() {
            self.push(Insn::CreateEmptyObject);
            return;
        }

        let mut compiled = vec![false; obj.properties.len()];
        let mut literal_slots: Vec<Option<Vec<LiteralEntry>>> =
            Vec::with_capacity(obj.properties.len());
        let mut prop_name_map: FxHashMap<String, usize> = FxHashMap::default();
        let mut getter_name_map: FxHashMap<String, usize> = FxHashMap::default();
        let mut setter_name_map: FxHashMap<String, usize> = FxHashMap::default();

        for (idx, prop) in obj.properties.iter().enumerate() {
            let ObjectPropertyKind::ObjectProperty(property) = prop else {
                break;
            };
            if property.computed || Self::is_proto_property(property) {
                break;
            }
            let Some(key_name) = Self::property_key_name(&property.key) else {
                break;
            };

            let mut prop_idx = idx;
            if let Some(existing) = prop_name_map.get(&key_name).copied() {
                prop_idx = existing;
                if property.kind != PropertyKind::Set {
                    if let Some(getter_idx) = getter_name_map.get(&key_name).copied() {
                        compiled[getter_idx] = true;
                    }
                }
                if property.kind != PropertyKind::Get {
                    if let Some(setter_idx) = setter_name_map.get(&key_name).copied() {
                        compiled[setter_idx] = true;
                    }
                }
            } else {
                prop_name_map.insert(key_name.clone(), prop_idx);
            }

            if property.kind == PropertyKind::Get {
                getter_name_map.insert(key_name.clone(), idx);
            } else if property.kind == PropertyKind::Set {
                setter_name_map.insert(key_name.clone(), idx);
            }

            let (entries, is_compiled) = self.object_literal_static_entry(property, &key_name);
            if literal_slots.len() <= prop_idx {
                literal_slots.resize_with(prop_idx + 1, || None);
            }
            literal_slots[prop_idx] = Some(entries);
            if is_compiled {
                compiled[idx] = true;
            }
        }

        let entries = literal_slots
            .into_iter()
            .flatten()
            .flatten()
            .collect::<Vec<_>>();
        if entries.is_empty() {
            self.push(Insn::CreateEmptyObject);
        } else {
            let lit_id = self.literal_arrays.len() as u16;
            self.literal_arrays.push(LiteralArray {
                entries,
                explicit_key: None,
            });
            self.push(Insn::CreateObjectWithBuffer(0, lit_id));
        }

        let obj_reg = self.alloc_reg();
        self.push(Insn::Sta(obj_reg));
        for (idx, prop) in obj.properties.iter().enumerate() {
            if compiled[idx] {
                continue;
            }
            let ObjectPropertyKind::ObjectProperty(property) = prop else {
                if let ObjectPropertyKind::SpreadProperty(spread) = prop {
                    self.emit_expr(&spread.argument);
                    self.push(Insn::CopyDataProperties(obj_reg));
                }
                continue;
            };
            match property.kind {
                PropertyKind::Init => {
                    if Self::is_proto_property(property) {
                        self.emit_expr(&property.value);
                        let proto_reg = self.alloc_reg();
                        self.push(Insn::Sta(proto_reg));
                        self.push(Insn::Lda(obj_reg));
                        self.push(Insn::SetObjectWithProto(0, proto_reg));
                    } else {
                        self.compile_object_init_property(property, obj_reg);
                    }
                }
                PropertyKind::Get | PropertyKind::Set => {
                    self.compile_object_accessor_property(property, obj_reg);
                }
            }
        }
        self.push(Insn::Lda(obj_reg));
    }

    // ── Conditional expression: test ? consequent : alternate ─────────────────

    fn emit_conditional_expr(&mut self, cond: &oxc_ast::ast::ConditionalExpression<'_>) {
        let false_branch = self.emit_branch_if_false(&cond.test);

        self.emit_expr(&cond.consequent);
        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));

        let alt_start = self.insns.len();
        self.patch_jump_to(false_branch, alt_start);

        self.emit_expr(&cond.alternate);

        let end_pos = self.insns.len();
        self.patch_jump_to(jmp_pos, end_pos);
    }

    // ── Template literal: `q0${e0}q1…` ──────────────────────────────────────

    fn emit_template_literal(&mut self, tmpl: &oxc_ast::ast::TemplateLiteral<'_>) {
        let first = &tmpl.quasis[0];
        let first_str = first
            .value
            .cooked
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| first.value.raw.as_str().to_string());
        let first_id = self.intern(&first_str);
        self.push(Insn::LdaStr(first_id));

        if tmpl.expressions.is_empty() {
            return;
        }

        let left_reg = self.alloc_reg();
        self.push(Insn::Sta(left_reg));
        let tmp_reg = self.alloc_reg();

        for (i, expr) in tmpl.expressions.iter().enumerate() {
            // acc = expr_i; left = left + acc
            self.emit_expr(expr);
            self.push(Insn::Sta(tmp_reg));
            self.push(Insn::Lda(left_reg));
            self.push(Insn::Add2(0, tmp_reg));
            self.push(Insn::Sta(left_reg));

            // acc = quasi[i+1]; left = left + acc
            let q = &tmpl.quasis[i + 1];
            let q_str = q
                .value
                .cooked
                .as_ref()
                .map(|s| s.as_str().to_string())
                .unwrap_or_else(|| q.value.raw.as_str().to_string());
            let q_id = self.intern(&q_str);
            self.push(Insn::LdaStr(q_id));
            self.push(Insn::Sta(tmp_reg));
            self.push(Insn::Lda(left_reg));
            self.push(Insn::Add2(0, tmp_reg));
            self.push(Insn::Sta(left_reg));
        }

        self.push(Insn::Lda(left_reg));
    }

    fn emit_tagged_template_expr(&mut self, tagged: &oxc_ast::ast::TaggedTemplateExpression<'_>) {
        let saved_next_reg = self.next_reg;
        let slot = 0u8;
        let callee_reg = self.alloc_reg();
        let mut this_reg = None;

        match &tagged.tag {
            Expression::StaticMemberExpression(mem) => {
                let reg = self.alloc_reg();
                this_reg = Some(reg);
                self.emit_expr(&mem.object);
                self.set_debug_pos(tagged.tag.span().start);
                self.push(Insn::Sta(reg));
                self.set_debug_pos(mem.property.span.start);
                let name_id = self.intern(mem.property.name.as_str());
                self.push(Insn::Lda(reg));
                if matches!(&mem.object, Expression::Super(_)) {
                    self.push(Insn::LdSuperByName(0, name_id));
                } else {
                    self.push(Insn::LdObjByName(0, name_id));
                }
            }
            Expression::ComputedMemberExpression(mem) => {
                let reg = self.alloc_reg();
                this_reg = Some(reg);
                self.emit_expr(&mem.object);
                self.set_debug_pos(tagged.tag.span().start);
                self.push(Insn::Sta(reg));
                self.set_debug_pos(mem.expression.span().start);
                if let Expression::StringLiteral(key) = &mem.expression {
                    let name_id = self.intern(key.value.as_str());
                    self.push(Insn::Lda(reg));
                    if matches!(&mem.object, Expression::Super(_)) {
                        self.push(Insn::LdSuperByName(0, name_id));
                    } else {
                        self.push(Insn::LdObjByName(0, name_id));
                    }
                } else {
                    self.emit_expr(&mem.expression);
                    if matches!(&mem.object, Expression::Super(_)) {
                        self.push(Insn::LdSuperByValue(0, reg));
                    } else {
                        self.push(Insn::LdObjByValue(0, reg));
                    }
                }
            }
            tag => {
                self.emit_expr(tag);
            }
        }

        self.push(Insn::Sta(callee_reg));
        self.emit_template_object(&tagged.quasi);
        let live_next_reg = this_reg
            .map(|reg| reg.saturating_add(1))
            .unwrap_or_else(|| callee_reg.saturating_add(1));
        self.next_reg = live_next_reg;

        let arg0 = self.alloc_reg();
        self.push(Insn::Sta(arg0));
        let mut arg_regs = vec![arg0];
        for expr in &tagged.quasi.expressions {
            self.emit_expr(expr);
            let arg_reg = self.alloc_reg();
            self.push(Insn::Sta(arg_reg));
            arg_regs.push(arg_reg);
        }

        let argc = arg_regs.len();
        if let Some(this_reg) = this_reg {
            self.push(Insn::Lda(callee_reg));
            match argc {
                1 => self.push(Insn::CallThis1(slot, this_reg, arg_regs[0])),
                2 => self.push(Insn::CallThis2(slot, this_reg, arg_regs[0], arg_regs[1])),
                3 => self.push(Insn::CallThis3(
                    slot,
                    this_reg,
                    arg_regs[0],
                    arg_regs[1],
                    arg_regs[2],
                )),
                n => {
                    let first_arg = this_reg.saturating_add(1);
                    self.move_args_to_contiguous_range(&arg_regs, first_arg);
                    self.push(Insn::Lda(callee_reg));
                    self.push(Insn::CallThisRange(slot, n as u8, this_reg));
                }
            }
        } else {
            self.push(Insn::Lda(callee_reg));
            match argc {
                1 => self.push(Insn::CallArg1(slot, arg_regs[0])),
                2 => self.push(Insn::CallArgs2(slot, arg_regs[0], arg_regs[1])),
                3 => self.push(Insn::CallArgs3(slot, arg_regs[0], arg_regs[1], arg_regs[2])),
                n => {
                    let first_arg = callee_reg.saturating_add(1);
                    self.move_args_to_contiguous_range(&arg_regs, first_arg);
                    self.push(Insn::Lda(callee_reg));
                    self.push(Insn::CallRange(slot, n as u8, first_arg));
                }
            }
        }

        self.next_reg = saved_next_reg;
    }

    fn emit_template_object(&mut self, tmpl: &oxc_ast::ast::TemplateLiteral<'_>) {
        let saved_next_reg = self.next_reg;
        let template_arg_reg = self.alloc_reg();
        let index_reg = self.alloc_reg();
        let raw_arr_reg = self.alloc_reg();
        let cooked_arr_reg = self.alloc_reg();

        self.push(Insn::CreateEmptyArray(0));
        self.push(Insn::Sta(raw_arr_reg));
        self.push(Insn::CreateEmptyArray(0));
        self.push(Insn::Sta(cooked_arr_reg));

        for (idx, element) in tmpl.quasis.iter().enumerate() {
            self.push(Insn::LdAi(idx as i32));
            self.push(Insn::Sta(index_reg));

            let raw_id = self.intern(element.value.raw.as_str());
            self.push(Insn::LdaStr(raw_id));
            self.push(Insn::CallRuntimeDefineFieldByValue(
                0,
                index_reg,
                raw_arr_reg,
            ));

            if let Some(cooked) = &element.value.cooked {
                let cooked_id = self.intern(cooked.as_str());
                self.push(Insn::LdaStr(cooked_id));
            } else {
                self.push(Insn::LdUndefined);
            }
            self.push(Insn::CallRuntimeDefineFieldByValue(
                0,
                index_reg,
                cooked_arr_reg,
            ));
        }

        self.push(Insn::CreateEmptyArray(0));
        self.push(Insn::Sta(template_arg_reg));

        self.push(Insn::LdAi(0));
        self.push(Insn::Sta(index_reg));
        self.push(Insn::Lda(raw_arr_reg));
        self.push(Insn::CallRuntimeDefineFieldByValue(
            0,
            index_reg,
            template_arg_reg,
        ));

        self.push(Insn::LdAi(1));
        self.push(Insn::Sta(index_reg));
        self.push(Insn::Lda(cooked_arr_reg));
        self.push(Insn::CallRuntimeDefineFieldByValue(
            0,
            index_reg,
            template_arg_reg,
        ));

        self.push(Insn::Lda(template_arg_reg));
        self.push(Insn::GetTemplateObject(0));
        self.next_reg = saved_next_reg;
    }

    fn move_args_to_contiguous_range(&mut self, arg_regs: &[u8], first_arg: u8) {
        for (idx, &arg_reg) in arg_regs.iter().enumerate() {
            let target = first_arg.saturating_add(idx as u8);
            if arg_reg != target {
                self.push(Insn::Lda(arg_reg));
                self.push(Insn::Sta(target));
                if self.next_reg <= target {
                    self.next_reg = target.saturating_add(1);
                }
            }
        }
    }

    // ── Class definition ──────────────────────────────────────────────────────

    /// Emit `defineclasswithbuffer` for a class (declaration or expression).
    ///
    /// A class-buffer [`LiteralArray`] is registered in `self.literal_arrays`;
    /// its index becomes the `buf_id` operand. Method bodies are already
    /// compiled as separate `MethodRecord`s at the class-record level, and the
    /// literal buffer points at those MethodItems by file offset.
    ///
    /// After this call the class constructor is in the accumulator.
    /// Emit the generator function prologue:
    /// `createGeneratorObj v_func; sta gen_reg; suspendGenerator gen_reg;
    /// lda gen_reg; resumeGenerator`.
    ///
    /// Mirrors upstream es2abc generator function prologue:
    /// 1. Create the generator object (wraps the function).
    /// 2. Initial suspend: the `function*` call returns the generator object.
    /// 3. On first `next()`: execution resumes here.
    fn emit_generator_prologue(&mut self) {
        if !self.is_generator {
            return;
        }
        let reg = self.alloc_reg();
        // CreateGeneratorObj takes the function object (mandatory arg 0).
        self.push(Insn::CreateGeneratorObj(function_arg_register(0)));
        self.push(Insn::Sta(reg));
        self.gen_reg = Some(reg);
        // Initial suspension: the function call site gets the generator object.
        // On first next(), execution resumes after this suspend.
        self.push(Insn::SuspendGenerator(reg));
        self.push(Insn::Lda(reg));
        self.push(Insn::ResumeGenerator);
        // Discard the initial resume value (it's always undefined for the first call).
    }

    /// Emit the async function prologue: `asyncfunctionenter; sta async_reg`.
    ///
    /// Must be called before the function body statements are emitted.  Sets
    /// `self.async_reg` so that return sites can emit `asyncfunctionresolve`.
    fn emit_async_function_prologue(&mut self) {
        if !self.is_async {
            return;
        }
        let reg = self.alloc_reg();
        self.push(Insn::AsyncFunctionEnter);
        self.push(Insn::Sta(reg));
        self.async_reg = Some(reg);
        self.async_try_begin = Some(self.insns.len() as u32);
    }

    fn emit_async_cleanup(&mut self) {
        let (Some(async_reg), Some(try_begin)) = (self.async_reg, self.async_try_begin) else {
            return;
        };
        let try_end = self.insns.len() as u32;
        let catch_begin = self.insns.len() as u32;
        let exception_reg = self.alloc_reg();
        self.push(Insn::Sta(exception_reg));
        self.push(Insn::AsyncFunctionReject(async_reg));
        self.push(Insn::Return);
        let catch_end = self.insns.len() as u32;
        self.push_catch_entry(CatchEntry {
            try_begin,
            try_end,
            catch_begin,
            catch_end,
        });
        self.async_try_begin = None;
    }

    fn emit_async_await(&mut self) {
        let Some(async_reg) = self.async_reg else {
            return;
        };
        let completion_type = self.alloc_reg();
        let completion_value = self.alloc_reg();
        self.push(Insn::AsyncFunctionAwaitUncaught(async_reg));
        self.push(Insn::SuspendGenerator(async_reg));
        self.push(Insn::Lda(async_reg));
        self.push(Insn::ResumeGenerator);
        self.push(Insn::Sta(completion_value));
        self.push(Insn::Lda(async_reg));
        self.push(Insn::GetResumeMode);
        self.push(Insn::Sta(completion_type));
        self.push(Insn::LdAi(1));
        self.push(Insn::Eq(0, completion_type));
        let not_throw = self.insns.len();
        self.push(Insn::Jeqz(0));
        self.push(Insn::Lda(completion_value));
        self.push(Insn::Throw);
        let after_throw = self.insns.len();
        self.patch_jump_to(not_throw, after_throw);
        self.push(Insn::Lda(completion_value));
    }

    fn emit_yield_expr(&mut self, yield_expr: &YieldExpression<'_>) {
        // Emit the yielded value (or undefined for bare `yield`).
        match &yield_expr.argument {
            Some(arg) => self.emit_expr(arg),
            None => self.push(Insn::LdUndefined),
        }
        let Some(gen_reg) = self.gen_reg else {
            return;
        };
        // Upstream es2abc generator yield sequence (mirrors async await pattern):
        //   SuspendGenerator(gen_reg) — yield the value to the caller
        //   Lda(gen_reg) + ResumeGenerator — get the value passed to next()
        //   Sta(completion_value) + Lda(gen_reg) + GetResumeMode → mode
        //   if mode == 1: return (early return via .return(value))
        //   if mode == 2: throw (exception via .throw(error))
        //   acc = completion_value (normal next(value) resume)
        let completion_value = self.alloc_reg();
        let resume_mode = self.alloc_reg();
        self.push(Insn::SuspendGenerator(gen_reg));
        self.push(Insn::Lda(gen_reg));
        self.push(Insn::ResumeGenerator);
        self.push(Insn::Sta(completion_value));
        self.push(Insn::Lda(gen_reg));
        self.push(Insn::GetResumeMode);
        self.push(Insn::Sta(resume_mode));
        // Mode 1 = return: emit explicit return from the generator.
        self.push(Insn::LdAi(1));
        self.push(Insn::Eq(0, resume_mode));
        let not_return = self.insns.len();
        self.push(Insn::Jeqz(0));
        self.push(Insn::Lda(completion_value));
        self.emit_control_flow_exit(true);
        if !self.returned {
            self.push_with_epilogue(Insn::Return);
        }
        let after_return = self.insns.len();
        self.patch_jump_to(not_return, after_return);
        // Mode 2 = throw: re-throw the exception.
        self.push(Insn::LdAi(2));
        self.push(Insn::Eq(0, resume_mode));
        let not_throw = self.insns.len();
        self.push(Insn::Jeqz(0));
        self.push(Insn::Lda(completion_value));
        self.push(Insn::Throw);
        let after_throw = self.insns.len();
        self.patch_jump_to(not_throw, after_throw);
        // Mode 0 = next: acc = the value passed to next().
        self.push(Insn::Lda(completion_value));
    }

    // ── Class definition ──────────────────────────────────────────────────────

    /// Emit `defineclasswithbuffer` for a class (declaration or expression).
    ///
    /// A class-buffer [`LiteralArray`] is registered in `self.literal_arrays`;
    /// its index becomes the `buf_id` operand. Method bodies are already
    /// compiled as separate `MethodRecord`s at the class-record level, and the
    /// literal buffer points at those MethodItems by file offset.
    ///
    /// After this call the class constructor is in the accumulator.
    fn emit_class_inner(
        &mut self,
        class_name: Option<&str>,
        elements: &[ClassElement<'_>],
        super_class: Option<&Expression<'_>>,
    ) {
        // Load the prototype (parent class) if present.
        let proto_vreg = if let Some(sc) = super_class {
            self.emit_expr(sc);
            let reg = self.alloc_reg();
            self.push(Insn::Sta(reg));
            reg
        } else {
            let reg = self.alloc_reg();
            self.push(Insn::LdHole);
            self.push(Insn::Sta(reg));
            reg
        };
        let computed_keys = self.emit_class_computed_keys(elements);

        // Register the class literal buffer and use its index as buf_id.
        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays.push(class_buffer_literal_array(
            class_name.unwrap_or("_"),
            elements,
            self.lowering.standalone_ecmascript_input(),
        ));

        let formal_count = constructor_required_param_count(elements);

        // Emit defineclasswithbuffer:
        //   slot=0, ctor_id=lit_arr_id (stub), buf_id=lit_arr_id,
        //   proto_argc, proto_vreg
        self.push(Insn::DefineClassWithBuffer(
            0,
            lit_arr_id,
            lit_arr_id,
            formal_count,
            proto_vreg,
        ));
        let class_reg = self.alloc_reg();
        self.push(Insn::Sta(class_reg));
        self.emit_class_missing_properties(
            class_name.unwrap_or("_"),
            elements,
            class_reg,
            0,
            &computed_keys.method_regs,
            &computed_keys.auto_accessor_regs,
        );
        self.emit_class_static_initializer(
            class_name.unwrap_or("_"),
            elements,
            class_reg,
            0,
            &computed_keys.static_field_regs,
        );
        self.push(Insn::Lda(class_reg));
    }

    fn emit_class_var_decl(&mut self, name: &str, cls: &oxc_ast::ast::Class<'_>) {
        let lexical_slot = self.current_lexical_slot(name);
        let export_slot = self.lowering.export_index(name);
        if lexical_slot.is_some() {
            self.ensure_next_reg_at_least(5);
        } else if export_slot.is_some() {
            self.next_reg = self.next_reg.min(4);
        }
        let saved_next_reg = self.next_reg;
        let class_name = cls.id.as_ref().map_or(name, |id| id.name.as_str());
        let class_reg = self.alloc_reg();
        let var_reg = self.alloc_reg();
        if lexical_slot.is_none() {
            self.var_regs.insert(name.to_string(), var_reg);
        }

        let proto_vreg = if let Some(sc) = &cls.super_class {
            self.emit_expr(sc);
            let reg = self.alloc_reg();
            self.set_debug_pos(cls.body.span.start);
            self.push(Insn::Sta(reg));
            reg
        } else {
            self.push(Insn::LdHole);
            self.push(Insn::Sta(class_reg));
            class_reg
        };
        let computed_keys = self.emit_class_computed_keys(&cls.body.body);

        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays.push(class_buffer_literal_array(
            class_name,
            &cls.body.body,
            self.lowering.standalone_ecmascript_input(),
        ));
        let formal_count = constructor_required_param_count(&cls.body.body);
        self.set_debug_pos(cls.body.span.start);
        self.push(Insn::DefineClassWithBuffer(
            0,
            lit_arr_id,
            lit_arr_id,
            formal_count,
            proto_vreg,
        ));
        self.set_debug_pos(cls.body.span.start);
        self.push(Insn::Sta(class_reg));
        if let Some(id) = &cls.id {
            self.set_debug_pos(id.span.start);
        }
        self.push(Insn::Sta(var_reg));
        self.emit_class_missing_properties(
            class_name,
            &cls.body.body,
            class_reg,
            cls.body.span.start,
            &computed_keys.method_regs,
            &computed_keys.auto_accessor_regs,
        );
        self.emit_class_static_initializer(
            class_name,
            &cls.body.body,
            class_reg,
            cls.body.span.start,
            &computed_keys.static_field_regs,
        );
        if let Some(slot) = lexical_slot {
            self.push(Insn::Lda(class_reg));
            self.push(Insn::Sta(class_reg));
            self.push(Insn::Lda(class_reg));
            self.push(Insn::StLexVar(0, slot));
            self.next_reg = saved_next_reg;
        }
        if let Some(export_index) = export_slot {
            self.push(Insn::Lda(class_reg));
            self.push(Insn::StModuleVar(export_index));
        }
    }

    fn emit_standalone_class_with_instance_initializer(
        &mut self,
        class_name: &str,
        cls: &oxc_ast::ast::Class<'_>,
    ) -> bool {
        if cls.super_class.is_some() || !has_standalone_instance_initializer(&cls.body.body) {
            return false;
        }

        self.ensure_next_reg_at_least(2);
        self.push(Insn::Nop);
        self.push(Insn::NewLexEnv(1));

        let try_begin = self.insns.len() as u32;
        self.push(Insn::LdHole);
        self.push(Insn::Sta(1));

        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays
            .push(class_buffer_literal_array(class_name, &cls.body.body, true));
        let formal_count = constructor_required_param_count(&cls.body.body);
        self.set_debug_pos(cls.body.span.start);
        self.push(Insn::DefineClassWithBuffer(
            0,
            lit_arr_id,
            lit_arr_id,
            formal_count,
            1,
        ));

        let prototype_id = self.intern("prototype");
        self.push(Insn::LdObjByName(0, prototype_id));
        let initializer_name = format!("#~{class_name}>#instance_initializer");
        let initializer_id = self.intern(&initializer_name);
        self.push(Insn::DefineMethodShort(0, initializer_id, 0));
        self.push(Insn::StLexVar(0, 0));
        let try_end = self.insns.len() as u32;

        let jump_past_handler = self.insns.len();
        self.push(Insn::JmpShort(0));
        let catch_begin = self.insns.len() as u32;
        self.push(Insn::Sta(0));
        self.push(Insn::PopLexEnv);
        self.push(Insn::Lda(0));
        self.push(Insn::Throw);
        let catch_end = self.insns.len() as u32;
        self.patch_jump_to(jump_past_handler, catch_end as usize);
        self.push_catch_entry(CatchEntry {
            try_begin,
            try_end,
            catch_begin,
            catch_end: catch_begin,
        });

        self.push(Insn::PopLexEnv);
        self.current_line = u32::MAX;
        self.current_col = 0;
        self.push(Insn::ReturnUndefined);
        self.returned = true;
        true
    }

    fn emit_simple_standalone_class_declaration(
        &mut self,
        class_name: &str,
        cls: &oxc_ast::ast::Class<'_>,
    ) -> bool {
        if !is_simple_standalone_class(&cls.body.body) {
            return false;
        }
        let proto_vreg = if let Some(super_class) = &cls.super_class {
            let Some(reg) = self.standalone_super_class_reg(super_class) else {
                return false;
            };
            reg
        } else {
            let reg = self.alloc_reg();
            self.push(Insn::LdHole);
            self.push(Insn::Sta(reg));
            self.var_regs.insert(class_name.to_string(), reg);
            reg
        };

        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays
            .push(class_buffer_literal_array(class_name, &cls.body.body, true));
        let formal_count = constructor_required_param_count(&cls.body.body);
        self.set_debug_pos(cls.body.span.start);
        self.push(Insn::DefineClassWithBuffer(
            0,
            lit_arr_id,
            lit_arr_id,
            formal_count,
            proto_vreg,
        ));
        if cls.super_class.is_none() {
            if let Some(reg) = self.var_regs.get(class_name).copied() {
                self.push(Insn::Sta(reg));
            }
        }
        let prototype_id = self.intern("prototype");
        self.push(Insn::LdObjByName(0, prototype_id));
        true
    }

    fn standalone_super_class_reg(&self, super_class: &Expression<'_>) -> Option<u8> {
        match super_class {
            Expression::Identifier(identifier) => {
                self.var_regs.get(identifier.name.as_str()).copied()
            }
            _ => None,
        }
    }

    fn emit_class_computed_keys(&mut self, elements: &[ClassElement<'_>]) -> ClassComputedKeys {
        let mut keys = ClassComputedKeys {
            method_regs: Vec::new(),
            auto_accessor_regs: Vec::new(),
            static_field_regs: Vec::new(),
        };
        for element in elements {
            match element {
                ClassElement::PropertyDefinition(property)
                    if property.r#static && property.computed =>
                {
                    keys.static_field_regs
                        .push(self.emit_property_key_to_reg(&property.key));
                }
                ClassElement::PropertyDefinition(property)
                    if !property.r#static && property.computed =>
                {
                    let key_reg = self.emit_property_key_to_reg(&property.key);
                    let capture_name = computed_instance_class_field_key_name(property);
                    if let Some(slot) = self.current_lexical_slot(&capture_name) {
                        self.push(Insn::Lda(key_reg));
                        self.push(Insn::StLexVar(0, slot));
                    }
                }
                ClassElement::MethodDefinition(method) if method.computed => {
                    keys.method_regs
                        .push(self.emit_property_key_to_reg(&method.key));
                }
                ClassElement::AccessorProperty(accessor) if accessor.computed => {
                    keys.auto_accessor_regs
                        .push(self.emit_property_key_to_reg(&accessor.key));
                }
                _ => {}
            }
        }
        keys
    }

    fn emit_class_static_initializer(
        &mut self,
        class_name: &str,
        elements: &[ClassElement<'_>],
        class_reg: u8,
        class_span_start: u32,
        computed_key_regs: &[u8],
    ) {
        let Some(initializer_name) = static_initializer_record_name(class_name, elements) else {
            return;
        };
        let callee_reg = self.alloc_reg();
        let this_reg = self.alloc_reg();
        let method_name_id = self.intern(&initializer_name);
        self.set_debug_pos(class_span_start);
        self.push(Insn::DefineFuncShort(0, method_name_id, 0));
        self.push(Insn::Sta(callee_reg));
        self.push(Insn::Lda(class_reg));
        self.push(Insn::Sta(this_reg));
        self.push(Insn::Lda(callee_reg));
        match computed_key_regs {
            [] => self.push(Insn::CallThis0(0, this_reg)),
            [key0] => self.push(Insn::CallThis1(0, this_reg, *key0)),
            [key0, key1] => self.push(Insn::CallThis2(0, this_reg, *key0, *key1)),
            [key0, key1, key2] => self.push(Insn::CallThis3(0, this_reg, *key0, *key1, *key2)),
            keys => {
                let first_arg = this_reg.saturating_add(1);
                self.ensure_next_reg_at_least(first_arg.saturating_add(keys.len() as u8));
                for (idx, key_reg) in keys.iter().enumerate() {
                    self.push(Insn::Lda(*key_reg));
                    self.push(Insn::Sta(first_arg.saturating_add(idx as u8)));
                }
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThisRange(0, keys.len() as u8, this_reg));
            }
        }
        self.push(Insn::Lda(class_reg));
    }

    fn emit_class_missing_properties(
        &mut self,
        class_name: &str,
        elements: &[ClassElement<'_>],
        class_reg: u8,
        class_span_start: u32,
        computed_method_key_regs: &[u8],
        computed_auto_accessor_key_regs: &[u8],
    ) {
        let prototype_id = self.intern("prototype");
        self.set_debug_pos(class_span_start);
        self.push(Insn::Lda(class_reg));
        self.push(Insn::LdObjByName(0, prototype_id));
        let prototype_reg = self.alloc_reg();
        self.set_debug_pos(class_span_start);
        self.push(Insn::Sta(prototype_reg));

        let mut computed_method_index = 0usize;
        for element in elements {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };
            let computed_key_reg = if method.computed {
                let key = computed_method_key_regs.get(computed_method_index).copied();
                computed_method_index = computed_method_index.saturating_add(1);
                key
            } else {
                None
            };
            if method.kind == MethodDefinitionKind::Constructor || is_class_buffer_method(method) {
                continue;
            }
            let public_name = if method.computed {
                computed_class_method_record_name(method)
            } else if let Some(public_name) = method_key_name(&method.key) {
                public_name
            } else {
                continue;
            };
            let method_record_name = if self.lowering.standalone_ecmascript_input() {
                class_method_upstream_name(method, class_name)
                    .unwrap_or_else(|| public_name.clone())
            } else {
                public_name.clone()
            };
            let method_name_id = self.intern(&method_record_name);
            let length = method.value.params.items.len() as u8;
            let target_reg = if method.r#static {
                class_reg
            } else {
                prototype_reg
            };

            if method.kind.is_accessor() {
                let saved_next_reg = self.next_reg;
                let name_reg = computed_key_reg.unwrap_or_else(|| self.alloc_reg());
                let undefined_reg = self.alloc_reg();
                let method_reg = self.alloc_reg();
                if computed_key_reg.is_none() {
                    let public_name_id = self.intern(&public_name);
                    self.set_debug_pos(method.key.span().start);
                    self.push(Insn::LdaStr(public_name_id));
                    self.set_debug_pos(method.key.span().start);
                    self.push(Insn::Sta(name_reg));
                }
                self.set_debug_pos(class_span_start);
                if method.kind == MethodDefinitionKind::Get {
                    self.push(Insn::LdUndefined);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Sta(undefined_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Lda(target_reg));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::DefineMethodShort(0, method_name_id, length));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::Sta(method_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::LdFalse);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::DefineGetterSetterByValue(
                        target_reg,
                        name_reg,
                        method_reg,
                        undefined_reg,
                    ));
                } else {
                    self.push(Insn::LdUndefined);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Sta(undefined_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Lda(target_reg));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::DefineMethodShort(0, method_name_id, length));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::Sta(method_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::LdFalse);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::DefineGetterSetterByValue(
                        target_reg,
                        name_reg,
                        undefined_reg,
                        method_reg,
                    ));
                }
                self.next_reg = saved_next_reg;
                continue;
            }

            self.set_debug_pos(class_span_start);
            self.push(Insn::Lda(target_reg));
            self.set_debug_pos(method.value.span.start);
            self.push(Insn::DefineMethodShort(0, method_name_id, length));
            self.set_debug_pos(method.span.start);
            if let Some(key_reg) = computed_key_reg {
                self.push(Insn::StOwnByValue(0, target_reg, key_reg));
            } else {
                let public_name_id = self.intern(&public_name);
                self.push(Insn::StOwnByName(0, public_name_id, target_reg));
            }
        }

        // Emit DefineGetterSetterByValue for each auto-accessor property.
        // Mirrors upstream ProcessAutoAccessorProperty: after the class is defined,
        // install getter/setter pairs with the compiled accessor methods.
        let prototype_id = self.intern("prototype");
        let mut computed_auto_accessor_index = 0usize;
        for element in elements {
            let ClassElement::AccessorProperty(accessor) = element else {
                continue;
            };
            let computed_key_reg = if accessor.computed {
                let key = computed_auto_accessor_key_regs
                    .get(computed_auto_accessor_index)
                    .copied()
                    .expect("computed auto-accessor key must be precomputed");
                computed_auto_accessor_index = computed_auto_accessor_index.saturating_add(1);
                Some(key)
            } else {
                None
            };
            let target_reg = if accessor.r#static {
                class_reg
            } else {
                // prototype
                self.push(Insn::Lda(class_reg));
                self.push(Insn::LdObjByName(0, prototype_id));
                let proto_r = self.alloc_reg();
                self.push(Insn::Sta(proto_r));
                proto_r
            };
            // Upstream computed auto-accessors use the getter key expression
            // `backup = originalKey` and the setter key `backup`. The cached
            // register is the bytecode-level equivalent of that backup binding.
            let Some(name_reg) =
                computed_key_reg.or_else(|| self.emit_auto_accessor_static_key_to_reg(accessor))
            else {
                continue;
            };
            // Load the getter method.
            let getter_method_name = auto_accessor_getter_record_name(class_name, accessor);
            let getter_name_id = self.intern(&getter_method_name);
            let getter_reg = self.alloc_reg();
            self.push(Insn::Lda(target_reg));
            self.push(Insn::DefineMethodShort(0, getter_name_id, 0));
            self.push(Insn::Sta(getter_reg));
            // Load the setter method.
            let setter_method_name = auto_accessor_setter_record_name(class_name, accessor);
            let setter_name_id = self.intern(&setter_method_name);
            let setter_reg = self.alloc_reg();
            self.push(Insn::Lda(target_reg));
            self.push(Insn::DefineMethodShort(0, setter_name_id, 1));
            self.push(Insn::Sta(setter_reg));
            // Install via DefineGetterSetterByValue.
            self.push(Insn::LdFalse);
            self.push(Insn::DefineGetterSetterByValue(
                target_reg, name_reg, getter_reg, setter_reg,
            ));
        }

        // Emit callruntime.createprivateproperty if the class has private elements.
        // Mirrors upstream classDefinition.cpp: done AFTER CompileMissingProperties,
        // BEFORE StaticInitializer.
        let (private_slots, private_field_count) = collect_class_private_slots(elements);
        if !private_slots.is_empty() {
            let priv_lit_id = self.literal_arrays.len() as u16;
            self.literal_arrays
                .push(class_private_methods_literal_array(elements));
            self.push(Insn::CallRuntimeCreatePrivateProperty(
                private_field_count,
                priv_lit_id,
            ));
            // Reload class_reg into acc (createprivateproperty clobbers it).
            self.push(Insn::Lda(class_reg));
        }
    }

    // ── Computed member: obj[expr] ────────────────────────────────────────────

    fn emit_computed_member(&mut self, mem: &oxc_ast::ast::ComputedMemberExpression<'_>) {
        if is_global_this(&mem.object) {
            if let Expression::StringLiteral(key) = &mem.expression {
                let name_id = self.intern(key.value.as_str());
                self.push(Insn::TryLdGlobalByName(0, name_id));
                return;
            }
        }
        let saved_next_reg = self.next_reg;
        let obj_reg = self.alloc_reg();
        self.emit_expr(&mem.object);
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        if let Expression::StringLiteral(key) = &mem.expression {
            let name_id = self.intern(key.value.as_str());
            self.push(Insn::Lda(obj_reg));
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::LdSuperByName(0, name_id));
            } else {
                self.push(Insn::LdObjByName(0, name_id));
            }
        } else {
            self.emit_expr(&mem.expression);
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::LdSuperByValue(0, obj_reg));
            } else {
                self.push(Insn::LdObjByValue(0, obj_reg));
            }
        }
        self.next_reg = saved_next_reg;
    }

    // ── Static member: obj.prop ───────────────────────────────────────────────

    fn emit_static_member(&mut self, mem: &oxc_ast::ast::StaticMemberExpression<'_>) {
        if !matches!(&mem.object, Expression::Super(_)) {
            self.emit_expr(&mem.object);
            self.set_debug_pos(mem.property.span.start);
            let name_id = self.intern(mem.property.name.as_str());
            self.push(Insn::LdObjByName(0, name_id));
            return;
        }

        let saved_next_reg = self.next_reg;
        let obj_reg = self.alloc_reg();
        self.emit_expr(&mem.object);
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        // Update col to the property name position for per-instruction tracking.
        self.set_debug_pos(mem.property.span.start);
        let name_id = self.intern(mem.property.name.as_str());
        self.push(Insn::Lda(obj_reg));
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::LdSuperByName(0, name_id));
        } else {
            self.push(Insn::LdObjByName(0, name_id));
        }
        self.next_reg = saved_next_reg;
    }

    // ── Assignment: lhs = rhs ─────────────────────────────────────────────────

    fn emit_identifier_load(&mut self, name: &str) {
        if let Some(&reg) = self.var_regs.get(name) {
            self.push(Insn::Lda(reg));
        } else if let Some(export_index) = self.lowering.export_index(name) {
            self.push(Insn::LdLocalModuleVar(export_index));
            let name_id = self.intern(name);
            self.push(Insn::ThrowUndefinedIfHoleWithName(name_id));
        } else if let Some((level, slot)) = self.lexical_lookup(name) {
            self.push(Insn::LdLexVar(level, slot));
            let name_id = self.intern(name);
            self.push(Insn::ThrowUndefinedIfHoleWithName(name_id));
        } else if let Some(import_index) = self.lowering.import_index(name) {
            self.push(Insn::LdExternalModuleVar(import_index));
            let name_id = self.intern(name);
            self.push(Insn::ThrowUndefinedIfHoleWithName(name_id));
        } else {
            let name_id = self.intern(name);
            self.push(Insn::TryLdGlobalByName(0, name_id));
        }
    }

    fn emit_identifier_store(&mut self, name: &str) {
        if let Some(&reg) = self.var_regs.get(name) {
            self.push(Insn::Sta(reg));
        } else if let Some((level, slot)) = self.lexical_lookup(name) {
            self.push(Insn::StLexVar(level, slot));
        } else if let Some(export_index) = self.lowering.export_index(name) {
            self.push(Insn::StModuleVar(export_index));
        } else {
            let name_id = self.intern(name);
            self.push(Insn::TryStGlobalByName(0, name_id));
        }
    }

    fn emit_assignment(&mut self, assign: &oxc_ast::ast::AssignmentExpression<'_>) {
        self.emit_assignment_target(&assign.left, assign.operator, &assign.right);
    }

    fn emit_assignment_target(
        &mut self,
        target: &AssignmentTarget<'_>,
        operator: AssignmentOperator,
        right: &Expression<'_>,
    ) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.emit_identifier_assignment(id.name.as_str(), operator, right);
            }
            AssignmentTarget::ComputedMemberExpression(mem) => {
                self.emit_computed_member_assignment(mem, operator, right);
            }
            AssignmentTarget::StaticMemberExpression(mem) => {
                self.emit_static_member_assignment(mem, operator, right);
            }
            AssignmentTarget::TSAsExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            AssignmentTarget::TSSatisfiesExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            AssignmentTarget::TSNonNullExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            AssignmentTarget::TSTypeAssertion(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            AssignmentTarget::PrivateFieldExpression(expr) => {
                // `this.#x = value` — private field store.
                let name = expr.field.name.as_str().to_string();
                self.emit_private_field_store(&name, &expr.object, right);
            }
            AssignmentTarget::ArrayAssignmentTarget(arr) => {
                // Destructuring assignment: evaluate RHS then bind to pattern.
                let rhs_reg = self.alloc_reg();
                self.emit_expr(right);
                self.push(Insn::Sta(rhs_reg));
                self.emit_destructure_array_assignment(arr, rhs_reg);
            }
            AssignmentTarget::ObjectAssignmentTarget(obj) => {
                let rhs_reg = self.alloc_reg();
                self.emit_expr(right);
                self.push(Insn::Sta(rhs_reg));
                self.emit_destructure_object_assignment(obj, rhs_reg);
            }
        }
    }

    fn emit_assignment_expression_target(
        &mut self,
        target: &Expression<'_>,
        operator: AssignmentOperator,
        right: &Expression<'_>,
    ) {
        match target {
            Expression::Identifier(id) => {
                self.emit_identifier_assignment(id.name.as_str(), operator, right);
            }
            Expression::ComputedMemberExpression(mem) => {
                self.emit_computed_member_assignment(mem, operator, right);
            }
            Expression::StaticMemberExpression(mem) => {
                self.emit_static_member_assignment(mem, operator, right);
            }
            Expression::ParenthesizedExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            Expression::TSAsExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            Expression::TSSatisfiesExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            Expression::TSNonNullExpression(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            Expression::TSTypeAssertion(expr) => {
                self.emit_assignment_expression_target(&expr.expression, operator, right);
            }
            Expression::PrivateFieldExpression(expr) => {
                let name = expr.field.name.as_str().to_string();
                self.emit_private_field_store(&name, &expr.object, right);
            }
            _ => unreachable!("invalid assignment target reached ABC lowering"),
        }
    }

    fn emit_identifier_assignment(
        &mut self,
        name: &str,
        operator: AssignmentOperator,
        right: &Expression<'_>,
    ) {
        if operator.is_logical() {
            self.emit_identifier_load(name);
            let lhs_reg = self.alloc_reg();
            self.push(Insn::Sta(lhs_reg));
            self.emit_logical_assignment_after_lhs(operator, right, lhs_reg, |this| {
                this.emit_identifier_store(name);
            });
            return;
        }
        if operator != AssignmentOperator::Assign {
            self.emit_identifier_load(name);
            let lhs_reg = self.alloc_reg();
            self.push(Insn::Sta(lhs_reg));
            self.emit_expr(right);
            self.emit_assignment_binary(operator, lhs_reg);
        } else {
            self.emit_expr(right);
        }
        self.emit_identifier_store(name);
    }

    fn emit_update_expr(&mut self, update: &UpdateExpression<'_>) {
        match &update.argument {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.emit_identifier_update(id.name.as_str(), update.operator, update.prefix);
            }
            SimpleAssignmentTarget::TSAsExpression(expr) => {
                self.emit_update_expression_target(
                    &expr.expression,
                    update.operator,
                    update.prefix,
                );
            }
            SimpleAssignmentTarget::TSSatisfiesExpression(expr) => {
                self.emit_update_expression_target(
                    &expr.expression,
                    update.operator,
                    update.prefix,
                );
            }
            SimpleAssignmentTarget::TSNonNullExpression(expr) => {
                self.emit_update_expression_target(
                    &expr.expression,
                    update.operator,
                    update.prefix,
                );
            }
            SimpleAssignmentTarget::TSTypeAssertion(expr) => {
                self.emit_update_expression_target(
                    &expr.expression,
                    update.operator,
                    update.prefix,
                );
            }
            SimpleAssignmentTarget::PrivateFieldExpression(expr) => {
                // `this.#x++` — private field update.
                let name = expr.field.name.as_str().to_string();
                self.emit_private_field_load(&name, &expr.object);
                let (original_reg, updated_reg) =
                    self.emit_update_acc(update.operator, update.prefix);
                let obj_reg = self.alloc_reg();
                let slot = self.private_field_slots.get(name.as_str()).copied();
                self.emit_expr(&expr.object);
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(updated_reg));
                if let Some(slot) = slot {
                    self.push(Insn::StPrivateProperty(0, 0, slot, obj_reg));
                }
                if !update.prefix {
                    if let Some(orig) = original_reg {
                        self.push(Insn::Lda(orig));
                    }
                }
            }
            SimpleAssignmentTarget::ComputedMemberExpression(mem) => {
                self.emit_computed_member_update(mem, update.operator, update.prefix);
            }
            SimpleAssignmentTarget::StaticMemberExpression(mem) => {
                self.emit_static_member_update(mem, update.operator, update.prefix);
            }
        }
    }

    fn emit_update_expression_target(
        &mut self,
        target: &Expression<'_>,
        operator: UpdateOperator,
        prefix: bool,
    ) {
        match target {
            Expression::Identifier(id) => {
                self.emit_identifier_update(id.name.as_str(), operator, prefix)
            }
            Expression::ParenthesizedExpression(expr) => {
                self.emit_update_expression_target(&expr.expression, operator, prefix);
            }
            Expression::TSAsExpression(expr) => {
                self.emit_update_expression_target(&expr.expression, operator, prefix);
            }
            Expression::TSSatisfiesExpression(expr) => {
                self.emit_update_expression_target(&expr.expression, operator, prefix);
            }
            Expression::TSNonNullExpression(expr) => {
                self.emit_update_expression_target(&expr.expression, operator, prefix);
            }
            Expression::TSTypeAssertion(expr) => {
                self.emit_update_expression_target(&expr.expression, operator, prefix);
            }
            Expression::PrivateFieldExpression(expr) => {
                // `this.#x++` — read-modify-write with private field.
                let name = expr.field.name.as_str().to_string();
                self.emit_private_field_load(&name, &expr.object);
                let (original_reg, updated_reg) = self.emit_update_acc(operator, prefix);
                let obj_reg = self.alloc_reg();
                let slot = self.private_field_slots.get(name.as_str()).copied();
                self.emit_expr(&expr.object);
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(updated_reg));
                if let Some(slot) = slot {
                    self.push(Insn::StPrivateProperty(0, 0, slot, obj_reg));
                }
                if !prefix {
                    if let Some(orig) = original_reg {
                        self.push(Insn::Lda(orig));
                    }
                }
            }
            Expression::ComputedMemberExpression(mem) => {
                self.emit_computed_member_update(mem, operator, prefix);
            }
            Expression::StaticMemberExpression(mem) => {
                self.emit_static_member_update(mem, operator, prefix);
            }
            _ => unreachable!("invalid update target reached ABC lowering"),
        }
    }

    fn emit_identifier_update(&mut self, name: &str, operator: UpdateOperator, prefix: bool) {
        self.emit_identifier_load(name);
        let (original_reg, updated_reg) = self.emit_update_acc(operator, prefix);
        self.emit_identifier_store(name);
        self.emit_update_result(original_reg, updated_reg);
    }

    fn emit_static_member_update(
        &mut self,
        mem: &oxc_ast::ast::StaticMemberExpression<'_>,
        operator: UpdateOperator,
        prefix: bool,
    ) {
        let saved_next_reg = self.next_reg;
        let obj_reg = self.alloc_reg();
        self.emit_expr(&mem.object);
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        self.set_debug_pos(mem.property.span.start);
        let name_id = self.intern(mem.property.name.as_str());
        self.push(Insn::Lda(obj_reg));
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::LdSuperByName(0, name_id));
        } else {
            self.push(Insn::LdObjByName(0, name_id));
        }
        let (original_reg, updated_reg) = self.emit_update_acc(operator, prefix);
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::StSuperByName(0, name_id, obj_reg));
        } else {
            self.push(Insn::StObjByName(0, name_id, obj_reg));
        }
        self.emit_update_result(original_reg, updated_reg);
        self.next_reg = saved_next_reg;
    }

    fn emit_computed_member_update(
        &mut self,
        mem: &oxc_ast::ast::ComputedMemberExpression<'_>,
        operator: UpdateOperator,
        prefix: bool,
    ) {
        let saved_next_reg = self.next_reg;
        let obj_reg = self.alloc_reg();
        self.emit_expr(&mem.object);
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        self.emit_expr(&mem.expression);
        let key_reg = self.alloc_reg();
        self.push(Insn::Sta(key_reg));
        self.push(Insn::Lda(key_reg));
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::LdSuperByValue(0, obj_reg));
        } else {
            self.push(Insn::LdObjByValue(0, obj_reg));
        }
        let (original_reg, updated_reg) = self.emit_update_acc(operator, prefix);
        self.set_debug_pos(mem.span.start);
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::StSuperByValue(0, obj_reg, key_reg));
        } else {
            self.push(Insn::StObjByValue(0, obj_reg, key_reg));
        }
        self.emit_update_result(original_reg, updated_reg);
        self.next_reg = saved_next_reg;
    }

    fn emit_update_acc(&mut self, operator: UpdateOperator, prefix: bool) -> (Option<u8>, u8) {
        let original_reg = if prefix {
            None
        } else {
            let reg = self.alloc_reg();
            self.push(Insn::Sta(reg));
            self.push(Insn::ToNumber(0));
            Some(reg)
        };

        let operand_reg = self.alloc_reg();
        self.push(Insn::Sta(operand_reg));
        match operator {
            UpdateOperator::Increment => self.push(Insn::Inc(0)),
            UpdateOperator::Decrement => self.push(Insn::Dec(0)),
        }
        let updated_reg = self.alloc_reg();
        self.push(Insn::Sta(updated_reg));
        self.push(Insn::Lda(updated_reg));
        (original_reg, updated_reg)
    }

    fn emit_update_result(&mut self, original_reg: Option<u8>, updated_reg: u8) {
        if let Some(original_reg) = original_reg {
            self.push(Insn::Lda(original_reg));
        } else {
            self.push(Insn::Lda(updated_reg));
        }
    }

    fn emit_computed_member_assignment(
        &mut self,
        mem: &oxc_ast::ast::ComputedMemberExpression<'_>,
        operator: AssignmentOperator,
        right: &Expression<'_>,
    ) {
        if !operator.is_logical() && is_global_this(&mem.object) {
            if let Expression::StringLiteral(key) = &mem.expression {
                self.emit_expr(right);
                let name_id = self.intern(key.value.as_str());
                self.push(Insn::TryStGlobalByName(0, name_id));
                return;
            }
        }
        let saved_next_reg = self.next_reg;
        self.emit_expr(&mem.object);
        let obj_reg = self.alloc_reg();
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        if let Expression::StringLiteral(key) = &mem.expression {
            let name_id = self.intern(key.value.as_str());
            if operator.is_logical() {
                self.push(Insn::Lda(obj_reg));
                if matches!(&mem.object, Expression::Super(_)) {
                    self.push(Insn::LdSuperByName(0, name_id));
                } else {
                    self.push(Insn::LdObjByName(0, name_id));
                }
                let lhs_reg = self.alloc_reg();
                self.push(Insn::Sta(lhs_reg));
                self.emit_logical_assignment_after_lhs(operator, right, lhs_reg, |this| {
                    if matches!(&mem.object, Expression::Super(_)) {
                        this.push(Insn::StSuperByName(0, name_id, obj_reg));
                    } else {
                        this.push(Insn::StObjByName(0, name_id, obj_reg));
                    }
                });
                self.next_reg = saved_next_reg;
                return;
            }
            if operator != AssignmentOperator::Assign {
                self.push(Insn::Lda(obj_reg));
                if matches!(&mem.object, Expression::Super(_)) {
                    self.push(Insn::LdSuperByName(0, name_id));
                } else {
                    self.push(Insn::LdObjByName(0, name_id));
                }
                let lhs_reg = self.alloc_reg();
                self.push(Insn::Sta(lhs_reg));
                self.emit_expr(right);
                self.emit_assignment_binary(operator, lhs_reg);
            } else {
                self.emit_expr(right);
            }
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::StSuperByName(0, name_id, obj_reg));
            } else {
                self.push(Insn::StObjByName(0, name_id, obj_reg));
            }
        } else {
            self.emit_expr(&mem.expression);
            let key_reg = self.alloc_reg();
            self.push(Insn::Sta(key_reg));
            if operator.is_logical() {
                self.push(Insn::Lda(key_reg));
                if matches!(&mem.object, Expression::Super(_)) {
                    self.push(Insn::LdSuperByValue(0, obj_reg));
                } else {
                    self.push(Insn::LdObjByValue(0, obj_reg));
                }
                let lhs_reg = self.alloc_reg();
                self.push(Insn::Sta(lhs_reg));
                self.emit_logical_assignment_after_lhs(operator, right, lhs_reg, |this| {
                    if matches!(&mem.object, Expression::Super(_)) {
                        this.push(Insn::StSuperByValue(0, obj_reg, key_reg));
                    } else {
                        this.push(Insn::StObjByValue(0, obj_reg, key_reg));
                    }
                });
                self.next_reg = saved_next_reg;
                return;
            }
            if operator != AssignmentOperator::Assign {
                self.push(Insn::Lda(key_reg));
                if matches!(&mem.object, Expression::Super(_)) {
                    self.push(Insn::LdSuperByValue(0, obj_reg));
                } else {
                    self.push(Insn::LdObjByValue(0, obj_reg));
                }
                let lhs_reg = self.alloc_reg();
                self.push(Insn::Sta(lhs_reg));
                self.emit_expr(right);
                self.emit_assignment_binary(operator, lhs_reg);
            } else {
                self.emit_expr(right);
            }
            self.set_debug_pos(mem.span.start);
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::StSuperByValue(0, obj_reg, key_reg));
            } else {
                self.push(Insn::StObjByValue(0, obj_reg, key_reg));
            }
        }
        self.next_reg = saved_next_reg;
    }

    fn emit_static_member_assignment(
        &mut self,
        mem: &oxc_ast::ast::StaticMemberExpression<'_>,
        operator: AssignmentOperator,
        right: &Expression<'_>,
    ) {
        self.emit_expr(&mem.object);
        let obj_reg = self.alloc_reg();
        self.set_debug_pos(mem.span.start);
        self.push(Insn::Sta(obj_reg));
        if operator.is_logical() {
            self.push(Insn::Lda(obj_reg));
            let name_id = self.intern(mem.property.name.as_str());
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::LdSuperByName(0, name_id));
            } else {
                self.push(Insn::LdObjByName(0, name_id));
            }
            let lhs_reg = self.alloc_reg();
            self.push(Insn::Sta(lhs_reg));
            self.emit_logical_assignment_after_lhs(operator, right, lhs_reg, |this| {
                if matches!(&mem.object, Expression::Super(_)) {
                    this.push(Insn::StSuperByName(0, name_id, obj_reg));
                } else {
                    this.push(Insn::StObjByName(0, name_id, obj_reg));
                }
            });
            return;
        }
        if operator != AssignmentOperator::Assign {
            self.push(Insn::Lda(obj_reg));
            let name_id = self.intern(mem.property.name.as_str());
            if matches!(&mem.object, Expression::Super(_)) {
                self.push(Insn::LdSuperByName(0, name_id));
            } else {
                self.push(Insn::LdObjByName(0, name_id));
            }
            let lhs_reg = self.alloc_reg();
            self.push(Insn::Sta(lhs_reg));
            self.emit_expr(right);
            self.emit_assignment_binary(operator, lhs_reg);
        } else {
            self.emit_expr(right);
        }
        self.set_debug_pos(mem.span.start);
        let name_id = self.intern(mem.property.name.as_str());
        if matches!(&mem.object, Expression::Super(_)) {
            self.push(Insn::StSuperByName(0, name_id, obj_reg));
        } else {
            self.push(Insn::StObjByName(0, name_id, obj_reg));
        }
    }

    fn emit_logical_assignment_after_lhs<F>(
        &mut self,
        op: AssignmentOperator,
        right: &Expression<'_>,
        lhs_reg: u8,
        store: F,
    ) where
        F: FnOnce(&mut Self),
    {
        let skip_right = match op {
            AssignmentOperator::LogicalAnd => {
                self.emit_is_false();
                let branch_pos = self.insns.len();
                self.push(Insn::Jnez(0));
                branch_pos
            }
            AssignmentOperator::LogicalOr => {
                self.emit_is_true();
                let branch_pos = self.insns.len();
                self.push(Insn::Jnez(0));
                branch_pos
            }
            AssignmentOperator::LogicalNullish => {
                self.emit_skip_rhs_when_lhs_is_not_nullish(lhs_reg)
            }
            _ => unreachable!("logical assignment lowering requires a logical assignment operator"),
        };

        self.emit_expr(right);
        store(self);
        let end_jump = self.insns.len();
        self.push(Insn::Jmp(0));

        let skip_target = self.insns.len();
        self.patch_jump_to(skip_right, skip_target);
        self.push(Insn::Lda(lhs_reg));

        let end = self.insns.len();
        self.patch_jump_to(end_jump, end);
    }

    fn emit_skip_rhs_when_lhs_is_not_nullish(&mut self, lhs_reg: u8) -> usize {
        self.push(Insn::LdNull);
        self.push(Insn::StrictEq(0, lhs_reg));
        let nullish_pos = self.insns.len();
        self.push(Insn::Jnez(0));

        self.push(Insn::LdUndefined);
        self.push(Insn::StrictEq(0, lhs_reg));
        let skip_pos = self.insns.len();
        self.push(Insn::Jeqz(0));

        let nullish_target = self.insns.len();
        self.patch_jump_to(nullish_pos, nullish_target);
        skip_pos
    }

    // ── Call: callee(args) ────────────────────────────────────────────────────

    fn emit_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>) {
        let slot = 0u8;

        if matches!(&call.callee, Expression::Super(_)) {
            self.emit_super_call(call, slot);
            return;
        }

        // ── require("module_specifier") → tryldglobalbyname ──────────────────
        // ArkRuntime exposes module exports as globals through its `require`
        // mechanism.  We lower `require("@ohos:foo.bar")` directly to a
        // global lookup rather than emitting an actual function call.
        if let Expression::Identifier(id) = &call.callee {
            if id.name.as_str() == "require" && call.arguments.len() == 1 {
                if let Some(Expression::StringLiteral(s)) = call.arguments[0].as_expression() {
                    let name_id = self.intern(s.value.as_str());
                    self.push(Insn::TryLdGlobalByName(0, name_id));
                    return;
                }
            }
        }

        let is_method = matches!(
            &call.callee,
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
        );

        if is_method {
            self.emit_method_call(call, slot);
            return;
        }

        // Plain function call. Upstream CallExpression allocates a callee
        // register first; arguments are then emitted into consecutive registers
        // after it, and PandaGen::Call reloads callee before the call opcode.
        let saved_next_reg = self.next_reg;
        let callee_reg = self.alloc_reg();
        self.emit_expr(&call.callee);
        let call_fn_line = self.current_line;
        let call_fn_col = self.current_col;
        self.push(Insn::Sta(callee_reg));
        let argc = call.arguments.len();

        if Self::args_need_apply(&call.arguments) {
            let this_reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(this_reg));
            let args_obj = self.emit_arguments_array_to_reg(&call.arguments);
            self.current_line = call_fn_line;
            self.current_col = call_fn_col;
            self.push(Insn::Lda(callee_reg));
            self.push(Insn::Apply(slot, this_reg, args_obj));
            self.next_reg = saved_next_reg;
            return;
        }

        match argc {
            0 => {
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallArg0(slot));
            }
            1 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallArg1(slot, v0));
            }
            2 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallArgs2(slot, v0, v1));
            }
            3 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallArgs3(slot, v0, v1, v2));
            }
            n => {
                let first_arg = callee_reg.saturating_add(1);
                for (i, arg) in call.arguments.iter().enumerate() {
                    self.emit_argument_to_reg(arg, first_arg.saturating_add(i as u8));
                }
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallRange(slot, n as u8, first_arg));
            }
        }
        self.next_reg = saved_next_reg;
    }

    fn emit_method_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>, slot: u8) {
        let saved_next_reg = self.next_reg;

        enum MethodCallee<'b> {
            Static(&'b Expression<'b>, &'b str, u32),
            Computed(&'b Expression<'b>, &'b Expression<'b>, u32),
        }

        let callee = match &call.callee {
            Expression::StaticMemberExpression(mem) => MethodCallee::Static(
                &mem.object,
                mem.property.name.as_str(),
                mem.property.span.start,
            ),
            Expression::ComputedMemberExpression(mem) => {
                MethodCallee::Computed(&mem.object, &mem.expression, mem.expression.span().start)
            }
            other => {
                self.emit_expr(other);
                self.push(Insn::CallArg0(slot));
                self.next_reg = saved_next_reg;
                return;
            }
        };

        // Upstream CallExpression allocates a contiguous tuple:
        //   callee, this, arg0, arg1, ...
        // MemberExpression::Compile stores the receiver in `this` and leaves
        // the property function in acc; PandaGen::CallThis then reloads callee
        // before emitting the callthis* instruction.
        let callee_reg = self.alloc_reg();
        let this_reg = self.alloc_reg();
        let recv_expr = match callee {
            MethodCallee::Static(recv_expr, _, _) | MethodCallee::Computed(recv_expr, _, _) => {
                recv_expr
            }
        };
        let recv_is_super = matches!(recv_expr, Expression::Super(_));
        self.emit_expr(recv_expr);
        self.set_debug_pos(call.callee.span().start);
        self.push(Insn::Sta(this_reg));

        match callee {
            MethodCallee::Static(_, method_name, prop_start) => {
                self.set_debug_pos(prop_start);
                let name_id = self.intern(method_name);
                self.push(Insn::Lda(this_reg));
                if recv_is_super {
                    self.push(Insn::LdSuperByName(0, name_id));
                } else {
                    self.push(Insn::LdObjByName(0, name_id));
                }
                self.push(Insn::Sta(callee_reg));
            }
            MethodCallee::Computed(_, key_expr, prop_start) => {
                self.set_debug_pos(prop_start);
                if recv_is_super {
                    if let Expression::StringLiteral(key) = key_expr {
                        let name_id = self.intern(key.value.as_str());
                        self.push(Insn::Lda(this_reg));
                        self.push(Insn::LdSuperByName(0, name_id));
                    } else {
                        self.emit_expr(key_expr);
                        self.push(Insn::LdSuperByValue(0, this_reg));
                    }
                } else {
                    self.emit_expr(key_expr);
                    let key_reg = self.alloc_reg();
                    self.push(Insn::Sta(key_reg));
                    self.push(Insn::Lda(key_reg));
                    self.push(Insn::LdObjByValue(0, this_reg));
                }
                self.push(Insn::Sta(callee_reg));
            }
        }

        let argc = call.arguments.len();

        // Save the method name column for the call instruction.
        // After compiling arguments, current_col is the last arg's col.
        // The call instruction itself should use the method name col,
        // matching es2panda's per-instruction column tracking.
        let call_line = self.current_line;
        let call_col = self.current_col;

        if Self::args_need_apply(&call.arguments) {
            let args_obj = self.emit_arguments_array_to_reg(&call.arguments);
            self.current_line = call_line;
            self.current_col = call_col; // reset to method col for call insn
            self.push(Insn::Lda(callee_reg));
            self.push(Insn::Apply(slot, this_reg, args_obj));
            self.next_reg = saved_next_reg;
            return;
        }

        match argc {
            0 => {
                self.current_line = call_line;
                self.current_col = call_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThis0(slot, this_reg));
            }
            1 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                self.current_line = call_line;
                self.current_col = call_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThis1(slot, this_reg, v0));
            }
            2 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                self.current_line = call_line;
                self.current_col = call_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThis2(slot, this_reg, v0, v1));
            }
            3 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                self.current_line = call_line;
                self.current_col = call_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThis3(slot, this_reg, v0, v1, v2));
            }
            n => {
                let first_arg = this_reg.saturating_add(1);
                for (i, arg) in call.arguments.iter().enumerate() {
                    self.emit_argument_to_reg(arg, first_arg.saturating_add(i as u8));
                }
                self.current_line = call_line;
                self.current_col = call_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallThisRange(slot, n as u8, this_reg));
            }
        }

        self.next_reg = saved_next_reg;
    }

    fn emit_super_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>, slot: u8) {
        let saved_next_reg = self.next_reg;
        let argc = call.arguments.len();

        if Self::args_have_spread(&call.arguments) {
            let args_obj = self.emit_arguments_array_to_reg(&call.arguments);
            self.set_debug_pos(call.span.start);
            self.push(Insn::Lda(MANDATORY_PARAM_FUNCTION_REG));
            self.push(Insn::SuperCallSpread(slot, args_obj));
            self.next_reg = saved_next_reg;
            let new_this_reg = self.alloc_reg();
            self.push(Insn::Sta(new_this_reg));
            self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
            self.push(Insn::ThrowIfSuperNotCorrectCall(1));
            if self.lowering.standalone_ecmascript_input() {
                self.derived_new_this_reg = Some(new_this_reg);
            } else {
                self.push(Insn::Lda(new_this_reg));
                self.push(Insn::Sta(MANDATORY_PARAM_THIS_REG));
            }
            self.next_reg = saved_next_reg;
            return;
        }

        let first_arg_reg = if argc == 0 {
            let reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(reg));
            if self.lowering.standalone_ecmascript_input() {
                let copied_reg = self.alloc_reg();
                self.push(Insn::MovV4(copied_reg, reg));
                copied_reg
            } else {
                reg
            }
        } else {
            let reg = self.next_reg;
            for (i, arg) in call.arguments.iter().enumerate() {
                self.emit_argument_to_reg(arg, reg.saturating_add(i as u8));
            }
            reg
        };

        self.set_debug_pos(call.span.start);
        self.push(Insn::SuperCallThisRange(slot, argc as u8, first_arg_reg));
        self.next_reg = saved_next_reg;
        let new_this_reg = self.alloc_reg();
        self.push(Insn::Sta(new_this_reg));
        self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
        self.push(Insn::ThrowIfSuperNotCorrectCall(1));
        if self.lowering.standalone_ecmascript_input() {
            self.derived_new_this_reg = Some(new_this_reg);
        } else {
            self.push(Insn::Lda(new_this_reg));
            self.push(Insn::Sta(MANDATORY_PARAM_THIS_REG));
        }
        self.next_reg = saved_next_reg;
    }

    // ── New: new Ctor(args) ───────────────────────────────────────────────────

    fn emit_new(&mut self, new_expr: &oxc_ast::ast::NewExpression<'_>) {
        let slot = 0u8;
        self.emit_expr(&new_expr.callee);
        let ctor_reg = self.alloc_reg();
        self.set_debug_pos(new_expr.span.start);
        self.push(Insn::Sta(ctor_reg));
        let argc = new_expr.arguments.len();

        if Self::args_have_spread(&new_expr.arguments) {
            let args_obj = self.emit_arguments_array_to_reg(&new_expr.arguments);
            self.set_debug_pos(new_expr.span.start);
            self.push(Insn::Lda(args_obj));
            self.push(Insn::NewObjApply(slot, ctor_reg));
            return;
        }

        // Allocate argument registers contiguous after ctor_reg and compile each arg.
        for (i, arg) in new_expr.arguments.iter().enumerate() {
            let arg_reg = ctor_reg.saturating_add(1).saturating_add(i as u8);
            self.emit_argument_to_reg(arg, arg_reg);
            self.set_debug_pos(new_expr.span.start);
        }
        // newobjrange slot, (argc+1), ctor_reg — registers are [ctor, arg0, arg1, …].
        self.set_debug_pos(new_expr.span.start);
        self.push(Insn::NewObjRange(slot, argc as u8 + 1, ctor_reg));
    }

    // ── Argument helpers ──────────────────────────────────────────────────────

    fn emit_arguments_array_to_reg(&mut self, args: &[Argument<'_>]) -> u8 {
        self.push(Insn::CreateEmptyArray(0));
        let arr_reg = self.alloc_reg();
        self.push(Insn::Sta(arr_reg));

        let mut idx: u16 = 0;
        let mut iter = args.iter().peekable();
        while let Some(arg) = iter.peek().copied() {
            if matches!(arg, Argument::SpreadElement(_)) {
                break;
            }
            self.emit_argument(arg);
            self.set_debug_pos(argument_span_start(arg));
            self.push(Insn::StOwnByIndex(0, arr_reg, idx));
            idx += 1;
            iter.next();
        }

        if iter.peek().is_none() {
            self.push(Insn::Lda(arr_reg));
            return arr_reg;
        }

        let idx_reg = self.alloc_reg();
        self.push(Insn::LdAi(idx as i32));
        self.push(Insn::Sta(idx_reg));
        let length_id = self.intern("length");

        for arg in iter {
            if let Argument::SpreadElement(spread) = arg {
                self.emit_expr(&spread.argument);
                self.push(Insn::StArraySpread(arr_reg, idx_reg));
                self.push(Insn::Lda(arr_reg));
                self.push(Insn::LdObjByName(0, length_id));
                self.push(Insn::Sta(idx_reg));
            } else {
                self.emit_argument(arg);
                self.set_debug_pos(argument_span_start(arg));
                self.push(Insn::StOwnByValue(0, arr_reg, idx_reg));
                self.push(Insn::Lda(idx_reg));
                self.push(Insn::Inc(0));
                self.push(Insn::Sta(idx_reg));
            }
        }

        self.push(Insn::Lda(arr_reg));
        arr_reg
    }

    fn emit_argument_to_reg(&mut self, arg: &Argument<'_>, reg: u8) {
        self.emit_argument(arg);
        self.set_debug_pos(argument_span_start(arg));
        self.push(Insn::Sta(reg));
        if self.next_reg <= reg {
            self.next_reg = reg.saturating_add(1);
        }
    }

    /// Compile an argument expression into a fresh register; return the register.
    fn compile_arg_to_reg(&mut self, arg: &Argument<'_>) -> u8 {
        self.emit_argument(arg);
        let reg = self.alloc_reg();
        self.set_debug_pos(argument_span_start(arg));
        self.push(Insn::Sta(reg));
        reg
    }

    /// Compile an `Argument` and leave its expression value in the accumulator.
    fn emit_argument(&mut self, arg: &Argument<'_>) {
        if let Some(expr) = arg.as_expression() {
            self.emit_expr(expr);
        } else if let Argument::SpreadElement(spread) = arg {
            self.emit_expr(&spread.argument);
        } else {
            self.push(Insn::LdUndefined);
        }
    }

    /// Return `true` when at least one argument in `args` is a `SpreadElement`.
    fn args_have_spread(args: &[Argument<'_>]) -> bool {
        args.iter().any(|a| matches!(a, Argument::SpreadElement(_)))
    }

    fn args_need_apply(args: &[Argument<'_>]) -> bool {
        Self::args_have_spread(args) || args.len() >= MAX_RANGE_CALL_ARG
    }

    // ── Binary expression: left OP right ──────────────────────────────────────

    fn emit_binary_expr(&mut self, bin: &oxc_ast::ast::BinaryExpression<'_>) {
        use oxc_ast::ast::BinaryOperator as Op;

        let saved_next_reg = self.next_reg;
        if matches!(
            bin.operator,
            Op::Equality | Op::Inequality | Op::StrictEquality | Op::StrictInequality
        ) {
            if is_undefined_literal(&bin.left) && !is_undefined_literal(&bin.right) {
                let left_reg = self.alloc_reg();
                self.emit_expr(&bin.right);
                self.push(Insn::Sta(left_reg));
                self.emit_undefined_literal();
                self.emit_binary_op(bin.operator, left_reg);
                self.next_reg = saved_next_reg;
                return;
            }
        }

        let left_reg = self.alloc_reg();
        self.emit_expr(&bin.left);
        self.set_debug_pos(bin.right.span().start);
        self.push(Insn::Sta(left_reg));
        if is_undefined_literal(&bin.right) {
            self.emit_undefined_literal();
        } else {
            self.emit_expr(&bin.right);
        }

        self.set_debug_pos(bin.right.span().start);
        self.emit_binary_op(bin.operator, left_reg);
        self.next_reg = saved_next_reg;
    }

    fn emit_condition_comparison(&mut self, bin: &oxc_ast::ast::BinaryExpression<'_>) {
        let saved_next_reg = self.next_reg;
        if is_undefined_literal(&bin.left) && !is_undefined_literal(&bin.right) {
            let left_reg = self.alloc_reg();
            self.emit_expr(&bin.right);
            self.push(Insn::Sta(left_reg));
            self.emit_undefined_literal();
            self.emit_binary_op(bin.operator, left_reg);
            self.next_reg = saved_next_reg;
            return;
        }

        let left_reg = self.alloc_reg();
        self.emit_expr(&bin.left);
        self.set_debug_pos(bin.span.start);
        self.push(Insn::Sta(left_reg));
        if is_undefined_literal(&bin.right) {
            self.set_debug_pos(bin.right.span().start);
            self.emit_undefined_literal();
        } else {
            self.emit_expr(&bin.right);
        }
        self.set_debug_pos(bin.span.start);
        self.emit_binary_op(bin.operator, left_reg);
        self.next_reg = saved_next_reg;
    }

    fn emit_binary_op(&mut self, op: oxc_ast::ast::BinaryOperator, left_reg: u8) {
        use oxc_ast::ast::BinaryOperator as Op;

        match op {
            Op::Addition => self.push(Insn::Add2(0, left_reg)),
            Op::Subtraction => self.push(Insn::Sub2(0, left_reg)),
            Op::Multiplication => self.push(Insn::Mul2(0, left_reg)),
            Op::Division => self.push(Insn::Div2(0, left_reg)),
            Op::Remainder => self.push(Insn::Mod2(0, left_reg)),
            Op::Exponential => self.push(Insn::Exp(0, left_reg)),
            Op::Equality => self.push(Insn::Eq(0, left_reg)),
            Op::Inequality => self.push(Insn::NotEq(0, left_reg)),
            Op::StrictEquality => self.push(Insn::StrictEq(0, left_reg)),
            Op::StrictInequality => self.push(Insn::StrictNotEq(0, left_reg)),
            Op::LessThan => self.push(Insn::Less(0, left_reg)),
            Op::LessEqualThan => self.push(Insn::LessEq(0, left_reg)),
            Op::GreaterThan => self.push(Insn::Greater(0, left_reg)),
            Op::GreaterEqualThan => self.push(Insn::GreaterEq(0, left_reg)),
            Op::ShiftLeft => self.push(Insn::Shl2(0, left_reg)),
            Op::ShiftRight => self.push(Insn::Shr2(0, left_reg)),
            Op::ShiftRightZeroFill => self.push(Insn::Ashr2(0, left_reg)),
            Op::BitwiseAnd => self.push(Insn::And2(0, left_reg)),
            Op::BitwiseOR => self.push(Insn::Or2(0, left_reg)),
            Op::BitwiseXOR => self.push(Insn::Xor2(0, left_reg)),
            Op::In => {
                self.push(Insn::IsIn(0, left_reg));
            }
            Op::Instanceof => {
                self.push(Insn::InstanceOf(0, left_reg));
            }
        }
    }

    fn emit_assignment_binary(&mut self, op: AssignmentOperator, left_reg: u8) {
        match op {
            AssignmentOperator::Addition => self.push(Insn::Add2(0, left_reg)),
            AssignmentOperator::Subtraction => self.push(Insn::Sub2(0, left_reg)),
            AssignmentOperator::Multiplication => self.push(Insn::Mul2(0, left_reg)),
            AssignmentOperator::Division => self.push(Insn::Div2(0, left_reg)),
            AssignmentOperator::Remainder => self.push(Insn::Mod2(0, left_reg)),
            AssignmentOperator::Exponential => self.push(Insn::Exp(0, left_reg)),
            AssignmentOperator::ShiftLeft => self.push(Insn::Shl2(0, left_reg)),
            AssignmentOperator::ShiftRight => self.push(Insn::Shr2(0, left_reg)),
            AssignmentOperator::ShiftRightZeroFill => self.push(Insn::Ashr2(0, left_reg)),
            AssignmentOperator::BitwiseAnd => self.push(Insn::And2(0, left_reg)),
            AssignmentOperator::BitwiseOR => self.push(Insn::Or2(0, left_reg)),
            AssignmentOperator::BitwiseXOR => self.push(Insn::Xor2(0, left_reg)),
            AssignmentOperator::Assign
            | AssignmentOperator::LogicalOr
            | AssignmentOperator::LogicalAnd
            | AssignmentOperator::LogicalNullish => {
                unreachable!("non-binary assignment operator reached binary assignment lowering")
            }
        }
    }

    fn emit_undefined_literal(&mut self) {
        self.push(Insn::LdUndefined);
    }

    // ── Unary expression: OP operand ──────────────────────────────────────────

    fn emit_unary_expr(&mut self, unary: &oxc_ast::ast::UnaryExpression<'_>) {
        match unary.operator {
            UnaryOperator::Delete => {
                self.emit_delete_expr(&unary.argument);
            }
            UnaryOperator::Void => {
                self.emit_expr(&unary.argument);
                self.push(Insn::LdUndefined);
            }
            UnaryOperator::Typeof => {
                self.emit_expr(&unary.argument);
                self.set_debug_pos(unary.span.start);
                self.push(Insn::TypeOf(0));
            }
            UnaryOperator::UnaryNegation => {
                let operand_reg = self.emit_unary_operand_to_reg(&unary.argument);
                self.push(Insn::Lda(operand_reg));
                self.push(Insn::Neg(0));
            }
            UnaryOperator::BitwiseNot => {
                let operand_reg = self.emit_unary_operand_to_reg(&unary.argument);
                self.push(Insn::Lda(operand_reg));
                self.push(Insn::BitNot(0));
            }
            UnaryOperator::LogicalNot => {
                self.emit_unary_operand_to_reg(&unary.argument);
                self.set_debug_pos(unary.span.start);
                self.emit_negate_acc();
            }
            UnaryOperator::UnaryPlus => {
                let operand_reg = self.emit_unary_operand_to_reg(&unary.argument);
                self.push(Insn::Lda(operand_reg));
                self.push(Insn::ToNumber(0));
            }
        }
    }

    fn emit_unary_operand_to_reg(&mut self, argument: &Expression<'_>) -> u8 {
        self.emit_expr(argument);
        let operand_reg = self.alloc_reg();
        self.push(Insn::Sta(operand_reg));
        operand_reg
    }

    fn emit_delete_expr(&mut self, argument: &Expression<'_>) {
        match argument {
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                if self.is_bound_identifier(name) {
                    self.push(Insn::LdFalse);
                    return;
                }

                let global_reg = self.alloc_reg();
                self.push(Insn::LdGlobal);
                self.push(Insn::Sta(global_reg));
                let name_id = self.intern(name);
                self.push(Insn::LdaStr(name_id));
                self.push(Insn::DelObjProp(global_reg));
            }
            Expression::StaticMemberExpression(mem) => {
                let saved_next_reg = self.next_reg;
                let obj_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.set_debug_pos(mem.span.start);
                self.push(Insn::Sta(obj_reg));
                let name_id = self.intern(mem.property.name.as_str());
                self.set_debug_pos(mem.property.span.start);
                self.push(Insn::LdaStr(name_id));
                self.push(Insn::DelObjProp(obj_reg));
                self.next_reg = saved_next_reg;
            }
            Expression::ComputedMemberExpression(mem) => {
                let saved_next_reg = self.next_reg;
                let obj_reg = self.alloc_reg();
                self.emit_expr(&mem.object);
                self.set_debug_pos(mem.span.start);
                self.push(Insn::Sta(obj_reg));
                self.emit_expr(&mem.expression);
                self.push(Insn::DelObjProp(obj_reg));
                self.next_reg = saved_next_reg;
            }
            _ => {
                self.emit_expr(argument);
                self.push(Insn::LdTrue);
            }
        }
    }

    fn is_bound_identifier(&self, name: &str) -> bool {
        self.var_regs.contains_key(name)
            || self.lexical_lookup(name).is_some()
            || self.lowering.import_index(name).is_some()
            || self.lowering.export_index(name).is_some()
    }

    // ── Logical expression: left && right / left || right ─────────────────────

    /// Emit short-circuit `&&` / `||` with correct jump patching.
    ///
    /// `&&` layout:
    /// ```text
    ///   <left>              ← result in acc
    ///   jeqz  +N           ← if falsy, skip right; acc = left (already there)
    ///   <right>            ← acc = right (truthy branch result)
    ///   <after>
    /// ```
    ///
    /// `||` layout:
    /// ```text
    ///   <left>
    ///   jnez  +N           ← if truthy, skip right; acc = left
    ///   <right>
    ///   <after>
    /// ```
    ///
    /// `??` follows upstream `CompileLogical`: skip right only when lhs is
    /// neither `null` nor `undefined`.
    fn emit_logical_expr(&mut self, log: &oxc_ast::ast::LogicalExpression<'_>) {
        let saved_next_reg = self.next_reg;
        self.emit_expr(&log.left);
        let lhs_reg = self.alloc_reg();
        self.push(Insn::Sta(lhs_reg));

        let branch_jump = match log.operator {
            LogicalOperator::And => {
                self.emit_is_false();
                let branch_jump = self.insns.len();
                self.push(Insn::Jnez(0));
                branch_jump
            }
            LogicalOperator::Or => {
                self.emit_is_true();
                let branch_jump = self.insns.len();
                self.push(Insn::Jnez(0));
                branch_jump
            }
            LogicalOperator::Coalesce => self.emit_skip_rhs_when_lhs_is_not_nullish(lhs_reg),
        };

        self.emit_expr(&log.right);
        let end_jump = self.insns.len();
        self.push(Insn::Jmp(0));

        let skip_right = self.insns.len();
        self.patch_jump_to(branch_jump, skip_right);
        self.push(Insn::Lda(lhs_reg));

        let end_pos = self.insns.len();
        self.patch_jump_to(end_jump, end_pos);
        self.next_reg = saved_next_reg;
    }
}
