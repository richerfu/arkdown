use rustc_hash::FxHashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingPattern, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, ForInStatement, ForOfStatement, ForStatementLeft,
    FormalParameter, Function, FunctionBody, LogicalOperator, MethodDefinitionKind, Program,
    PropertyKey, SimpleAssignmentTarget, Statement, UnaryOperator, UpdateExpression,
    YieldExpression,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::operator::UpdateOperator;

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{CatchEntry, ClassRecord, DebugInfoRecord, MethodKind, MethodRecord};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::ir::ast_analysis::{
    arrow_captures_from_body, is_condition_comparison, is_global_this, is_new_target_meta,
    is_undefined_literal, method_body_contains_arrow, negative_integer_literal,
    stmts_capture_this_in_arrow, ArrowCaptures,
};
use crate::ir::class_layout::{
    class_buffer_literal_array, class_method_num_args, class_method_record_name,
    constructor_required_param_count, is_class_buffer_method, method_key_name,
};
use crate::ir::debug_lnp::{argument_span_start, col_at, line_at, LineTracker, SourceMapper};
use crate::ir::function_lexical::collect_function_lexical_bindings;
use crate::ir::function_records::{
    collect_function_records, FunctionNameAllocator, FunctionRecord, FunctionRecordNode,
};
use crate::ir::lowering_context::LoweringContext;
use crate::ir::module_lexical::collect_module_lexical_bindings;
use crate::ir::module_record::{collect_module_record, DEFAULT_LOCAL_NAME};
use crate::ir::opcodes::Insn;
use crate::ir::record_layout::RecordLayout;

const ARG_PLACEHOLDER_BASE: u8 = 192;
const ARG_PLACEHOLDER_LIMIT: u8 = 64;
const MANDATORY_PARAM_FUNCTION_REG: u8 = ARG_PLACEHOLDER_BASE;
const MANDATORY_PARAM_NEW_TARGET_REG: u8 = ARG_PLACEHOLDER_BASE + 1;
const MANDATORY_PARAM_THIS_REG: u8 = ARG_PLACEHOLDER_BASE + 2;
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
    build_record_with_debug_and_source_map_for_target(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        package_name,
        is_commonjs,
        is_module,
        None,
        target_api_version,
        target_api_sub_version,
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
    let source_mapper = SourceMapper::from_json(source_map);
    let source_mapper = source_mapper.as_ref();
    let string_start = strings.len() as u32;
    let literal_start = literal_arrays.len();

    let allocator = Allocator::default();
    let source_type = SourceType::ts().with_module(is_module);
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
    reject_unsupported_constructs(&program, record_name)?;

    let semantic_ret = SemanticBuilder::new().build(&program);
    let scoping = if semantic_ret.errors.is_empty() {
        Some(semantic_ret.semantic.scoping())
    } else {
        None
    };

    // Collect module import/export records before lowering so bytecode can use
    // hvigor/es2abc's module-variable opcodes instead of global lookups.
    let module_record = collect_module_record(&program, is_module, scoping);
    let lexical_bindings = collect_module_lexical_bindings(&program, is_module, &module_record);
    let function_lexical_infos = collect_function_lexical_bindings(&program);
    let scan_stmts: &[Statement<'_>] = statement_scan_body(&program, is_module);
    let mut function_names = FunctionNameAllocator::new(source);
    let function_records = collect_function_records(&program, scan_stmts, &mut function_names);
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
        )
    } else {
        compile_program_body(
            &program,
            strings,
            literal_arrays,
            &lowering,
            is_module,
            source,
        )
    };
    methods.push(MethodRecord {
        name: "func_main_0".to_string(),
        bytecode: func_main.bytecode,
        num_vregs: func_main.vregs,
        num_args: 3,
        debug_info: None,
        line_ops: func_main.line_ops,
        catch_entries: func_main.catches,
        kind: MethodKind::Module,
        method_pool_entry: false,
        definefunc_order: None,
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
    );

    let record_layout = RecordLayout::new(record_name, package_name, source, is_commonjs);
    let class_name = record_layout.class_name();

    // Literal arrays produced before this point are bytecode operands.  The
    // module-record and scope-name arrays below are class field values and are
    // not emitted into libpandafile's per-record method pool.
    let bytecode_literal_end = literal_arrays.len();

    let scope_names_idx = literal_arrays.len();
    literal_arrays.push(scope_names_literal_array(scan_stmts, record_name));
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
                    param_names: vec![],
                    lexical_this: false,
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
        source_lang: 0x04,
        source_file,
        module_requests: module_record.module_requests,
        fields: module_fields,
    })
}

fn lexical_indices<'a>(names: impl IntoIterator<Item = &'a str>) -> FxHashMap<String, u8> {
    names
        .into_iter()
        .enumerate()
        .filter_map(|(idx, name)| u8::try_from(idx).ok().map(|idx| (name.to_string(), idx)))
        .collect()
}

fn scope_names_literal_array(stmts: &[Statement<'_>], record_name: &str) -> LiteralArray {
    let Some(owner_method) = first_scope_names_owner_method(stmts) else {
        // No call-arg closures in this class → no scopeNames payload.
        return LiteralArray { entries: vec![] };
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
    }
}

fn first_scope_names_owner_method(stmts: &[Statement<'_>]) -> Option<String> {
    for stmt in stmts {
        match stmt {
            Statement::ClassDeclaration(class) => {
                if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                    return Some(name);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    if let Some(name) = first_scope_names_owner_in_class(&class.body.body) {
                        return Some(name);
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::ClassDeclaration(class)) = &export.declaration {
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
        if let Some(name) = method_key_name(&method.key) {
            return Some(name);
        }
    }
    None
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
) {
    match function.node {
        FunctionRecordNode::Function(func) => {
            if func.declare || func.body.is_none() {
                return;
            }
            let param_names = function_param_names(func);
            let rest_info = function_rest_param(func);
            let compiled = if rest_info.is_some() || func.r#async {
                if let Some(src) = source {
                    compile_function_body_ext_tracked(
                        func.body.as_deref(),
                        rest_info,
                        func.r#async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        Some(src),
                        source_mapper,
                        Some((func.span.start, func.span.end)),
                    )
                    .unwrap_or_else(BodyCompileResult::empty)
                } else {
                    compile_function_body_ext(
                        func.body.as_deref(),
                        rest_info,
                        func.r#async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
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
            methods.push(MethodRecord {
                name: function.name,
                bytecode: compiled.bytecode,
                num_vregs,
                num_args,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: 1,
                    source_file: path.to_string(),
                    param_names,
                    lexical_this: func
                        .body
                        .as_deref()
                        .is_some_and(|body| stmts_capture_this_in_arrow(&body.statements)),
                }),
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                kind: MethodKind::FunctionLiteral,
                method_pool_entry: true,
                definefunc_order: Some(definefunc_order),
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
            methods.push(MethodRecord {
                name: function.name,
                bytecode: compiled.bytecode,
                num_vregs,
                num_args,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: 1,
                    source_file: path.to_string(),
                    param_names,
                    lexical_this: captures.this_slot.is_some(),
                }),
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                kind: MethodKind::Anonymous,
                method_pool_entry: true,
                definefunc_order: Some(definefunc_order),
            });
        }
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
) {
    for stmt in stmts {
        // Handle `class Foo { ... }` (ClassDeclaration)
        if let Statement::ClassDeclaration(cls) = stmt {
            let class_name = cls.id.as_ref().map(|id| id.name.as_str());
            compile_class_elements(
                class_name,
                cls.super_class.is_some(),
                &cls.body.body,
                strings,
                literal_arrays,
                lowering,
                methods,
                source,
                source_file_path,
                source_mapper,
                function_names,
            );
            continue;
        }
        if let Statement::ExportDefaultDeclaration(export) = stmt {
            if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                compile_class_elements(
                    class_name,
                    cls.super_class.is_some(),
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    lowering,
                    methods,
                    source,
                    source_file_path,
                    source_mapper,
                    function_names,
                );
            }
            continue;
        }
        if let Statement::ExportNamedDeclaration(export) = stmt {
            if let Some(Declaration::ClassDeclaration(cls)) = &export.declaration {
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                compile_class_elements(
                    class_name,
                    cls.super_class.is_some(),
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    lowering,
                    methods,
                    source,
                    source_file_path,
                    source_mapper,
                    function_names,
                );
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
                        &cls.body.body,
                        strings,
                        literal_arrays,
                        lowering,
                        methods,
                        source,
                        source_file_path,
                        source_mapper,
                        function_names,
                    );
                    continue;
                }
            }
        }
    }
}

/// Compile method bodies from a class element list and append MethodRecords.
#[allow(clippy::too_many_arguments)]
fn compile_class_elements(
    class_name: Option<&str>,
    has_super_class: bool,
    elements: &[ClassElement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    methods: &mut Vec<MethodRecord>,
    source: Option<&str>,
    source_file_path: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    function_names: &mut FunctionNameAllocator<'_>,
) {
    let class_name = class_name.unwrap_or("_");

    // Collect non-static instance field initializers for injection into the constructor.
    let instance_fields: Vec<(String, &Expression<'_>)> = elements
        .iter()
        .filter_map(|e| {
            let ClassElement::PropertyDefinition(prop) = e else {
                return None;
            };
            if prop.r#static || prop.computed {
                return None;
            }
            let name = method_key_name(&prop.key)?;
            let value = prop.value.as_ref()?;
            Some((name, value))
        })
        .collect();

    let has_constructor = elements.iter().any(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    });
    if !has_constructor {
        // Synthesize a default constructor that still runs field initializers.
        let is_default_derived_constructor = has_super_class && instance_fields.is_empty();
        let compiled = if is_default_derived_constructor {
            let (bytecode, catches) = compile_default_derived_constructor();
            let line_ops = if source.is_some() {
                default_derived_constructor_line_ops()
            } else {
                Vec::new()
            };
            BodyCompileResult::new(bytecode, line_ops, catches, 6)
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
            )
        };
        methods.push(MethodRecord {
            name: function_names.bind(0, 0, Some(class_name)),
            bytecode: compiled.bytecode,
            num_vregs: compiled.vregs,
            num_args: 4,
            debug_info: None,
            line_ops: compiled.line_ops,
            catch_entries: compiled.catches,
            kind: MethodKind::Constructor,
            method_pool_entry: true,
            definefunc_order: None,
        });
    }

    for element in elements {
        if let ClassElement::MethodDefinition(method) = element {
            let Some(key_name) = class_method_record_name(method, class_name, function_names)
            else {
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
                    )
                }
            } else if rest_info.is_some() || is_method_async {
                if let Some(src) = source {
                    compile_function_body_ext_tracked(
                        method.value.body.as_deref(),
                        rest_info,
                        is_method_async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        Some(src),
                        source_mapper,
                        Some((method.value.span.start, method.value.span.end)),
                    )
                    .unwrap_or_else(BodyCompileResult::empty)
                } else {
                    compile_function_body_ext(
                        method.value.body.as_deref(),
                        rest_info,
                        is_method_async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        Some((method.value.span.start, method.value.span.end)),
                    )
                }
            } else if let Some(src) = source {
                compile_function_body_tracked_with_end(
                    method.value.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    src,
                    source_mapper,
                    Some(method.span.end),
                    Some((method.value.span.start, method.value.span.end)),
                )
            } else {
                compile_function_body(
                    method.value.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                    Some((method.value.span.start, method.value.span.end)),
                )
            };
            let num_vregs = compiled.vregs;
            let num_args = class_method_num_args(method);
            let debug_info = source_file_path.map(|path| DebugInfoRecord {
                line_start: 1,
                source_file: path.to_string(),
                param_names: param_names.clone(),
                lexical_this: method
                    .value
                    .body
                    .as_deref()
                    .is_some_and(|body| stmts_capture_this_in_arrow(&body.statements)),
            });
            methods.push(MethodRecord {
                name: key_name,
                bytecode: compiled.bytecode,
                num_vregs,
                num_args,
                debug_info,
                line_ops: compiled.line_ops,
                catch_entries: compiled.catches,
                kind: if is_constructor {
                    MethodKind::Constructor
                } else {
                    MethodKind::Function
                },
                method_pool_entry: true,
                definefunc_order: None,
            });
        }
    }
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
        ),
        None => {
            let mut st = StringTable::default();
            BodyCompileResult::new(Insn::ReturnUndefined.encode(&mut st), vec![], vec![], 0)
        }
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
        ),
        None => {
            let mut st = StringTable::default();
            BodyCompileResult::new(Insn::ReturnUndefined.encode(&mut st), vec![], vec![], 0)
        }
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

/// Compile a function body with optional rest-parameter and async-function support.
///
/// When `rest_param` is `Some((name, n))`:
/// - Emits `copyrestargs n` at the start of the body.
/// - Stores the result to a fresh register named `name`.
///
/// When `is_async` is `true`, follows es2panda's `AsyncFunctionBuilder`:
/// `asyncfunctionenter` creates the async function object, return sites resolve
/// it, and the protected body has a catch-all cleanup that rejects it.
fn compile_function_body_ext(
    body: Option<&FunctionBody<'_>>,
    rest_param: Option<(String, u8)>,
    is_async: bool,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    compile_function_body_ext_tracked(
        body,
        rest_param,
        is_async,
        strings,
        literal_arrays,
        lowering,
        param_names,
        None,
        None,
        current_function_span,
    )
    .unwrap_or_else(BodyCompileResult::empty)
}

fn compile_function_body_ext_tracked(
    body: Option<&FunctionBody<'_>>,
    rest_param: Option<(String, u8)>,
    is_async: bool,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    source: Option<&str>,
    source_mapper: Option<&SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> Option<BodyCompileResult> {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => {
            let mut st = StringTable::default();
            let bc = Insn::ReturnUndefined.encode(&mut st);
            return Some(BodyCompileResult::new(bc, vec![], vec![], 0));
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
    Some(cg.finish_with_line_ops_and_catches())
}

fn compile_default_derived_constructor() -> (Vec<u8>, Vec<CatchEntry>) {
    // Upstream es2panda emits a GENERATED_CONSTRUCTOR as `super(...args)`.
    // See:
    // - compiler/core/function.cpp: CompileFunctionParameterDeclaration
    // - ir/expressions/callExpression.cpp: CompileSuperCallWithSpreadArgs
    // - compiler/function/functionBuilder.cpp: ImplicitReturn
    //
    // This is the API<13 path used by the fixture: copy rest args, call
    // supercallspread, validate/set `this`, then return `this`.
    let bytecode = vec![
        0x44, 0x60, // mov v0, v6
        0x44, 0x71, // mov v1, v7
        0x44, 0x82, // mov v2, v8
        0x44, 0x93, // mov v3, v9
        0xcf, 0x00, // copyrestargs 0
        0x61, 0x03, // sta v3
        0x60, 0x03, // lda v3
        0x61, 0x05, // sta v5
        0x60, 0x00, // lda v0
        0xb9, 0x00, 0x05, // supercallspread 0, v5
        0x61, 0x05, // sta v5
        0x60, 0x02, // lda v2
        0xfe, 0x07, 0x01, // throwifsupernotcorrectcall 1
        0x60, 0x05, // lda v5
        0x61, 0x02, // sta v2
        0x60, 0x02, // lda v2
        0xfe, 0x07, 0x00, // throwifsupernotcorrectcall 0
        0x64, // return
    ];
    (bytecode, vec![])
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
/// For each `(field_name, init_expr)` in `fields`, emits:
///   `lda v2; sta this_reg; emit_expr(init); stobjbyname(name, this_reg)`
/// before the normal constructor statements.
fn compile_constructor_with_fields<'src>(
    body: Option<&FunctionBody<'src>>,
    params: &[FormalParameter<'src>],
    function_span_start: u32,
    fields: &[(String, &Expression<'src>)],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    has_super_class: bool,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.derived_constructor = has_super_class;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_default_parameters(params, Some(function_span_start));
    if !fields.is_empty() {
        cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
        let this_reg = cg.alloc_reg();
        cg.push(Insn::Sta(this_reg));
        for (name, init_expr) in fields {
            cg.emit_expr(init_expr);
            let name_id = cg.intern(name);
            cg.push(Insn::StObjByName(0, name_id, this_reg));
        }
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    if has_super_class && !cg.returned {
        cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
        cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
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
    fields: &[(String, &Expression<'src>)],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
    has_super_class: bool,
    source: &'src str,
    source_mapper: Option<&'src SourceMapper>,
    current_function_span: Option<(u32, u32)>,
) -> BodyCompileResult {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    cg.derived_constructor = has_super_class;
    cg.use_function_lexical_env(current_function_span);
    cg.emit_function_lex_env_prologue();
    cg.bind_function_mandatory_lexical_names();
    cg.bind_function_param_names(param_names);
    cg.emit_default_parameters(params, Some(function_span_start));
    if !fields.is_empty() {
        cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
        let this_reg = cg.alloc_reg();
        cg.push(Insn::Sta(this_reg));
        for (name, init_expr) in fields {
            cg.emit_expr(init_expr);
            let name_id = cg.intern(name);
            cg.push(Insn::StObjByName(0, name_id, this_reg));
        }
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    if has_super_class && !cg.returned {
        cg.current_line = u32::MAX;
        cg.current_col = u32::MAX;
        cg.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
        cg.push(Insn::ThrowIfSuperNotCorrectCall(0));
        cg.set_debug_pos(function_span_end);
        cg.push(Insn::Return);
        cg.returned = true;
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
) -> BodyCompileResult {
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.source = source;
    cg.module_body = module_body;
    if module_body {
        cg.use_module_lexical_env();
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
        cg.emit_function_hoists(stmts);
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
) -> BodyCompileResult {
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    cg.function_end = function_end;
    cg.module_body = module_body;
    if module_body {
        cg.use_module_lexical_env();
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
        cg.emit_function_hoists(stmts);
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

fn reject_unsupported_constructs(program: &Program<'_>, record_name: &str) -> Result<(), AbcError> {
    let body = statement_scan_body(program, true);
    let mut scanner = UnsupportedScanner { record_name };
    scanner.scan_statements(body)
}

struct UnsupportedScanner<'a> {
    record_name: &'a str,
}

impl UnsupportedScanner<'_> {
    fn unsupported(&self, construct: &str) -> AbcError {
        AbcError::UnsupportedConstruct {
            construct: construct.to_string(),
            file: self.record_name.to_string(),
        }
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
                        ForStatementInit::PrivateFieldExpression(_) => {
                            return Err(self.unsupported("PrivateFieldExpression"));
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
                if let Some(super_class) = &class.super_class {
                    self.scan_expression(super_class)?;
                }
                self.scan_class_elements(&class.body.body)
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    self.scan_declaration(decl)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn scan_declaration(&mut self, decl: &Declaration<'_>) -> Result<(), AbcError> {
        match decl {
            Declaration::VariableDeclaration(decl) => self.scan_variable_declaration(decl),
            Declaration::FunctionDeclaration(func) => self.scan_function(func),
            Declaration::ClassDeclaration(class) => {
                if let Some(super_class) = &class.super_class {
                    self.scan_expression(super_class)?;
                }
                self.scan_class_elements(&class.body.body)
            }
            _ => Ok(()),
        }
    }

    fn scan_expression(&mut self, expr: &Expression<'_>) -> Result<(), AbcError> {
        match expr {
            Expression::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
            }
            Expression::TSAsExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSSatisfiesExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSNonNullExpression(expr) => self.scan_expression(&expr.expression),
            Expression::TSTypeAssertion(expr) => self.scan_expression(&expr.expression),
            Expression::TSInstantiationExpression(expr) => self.scan_expression(&expr.expression),
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
                oxc_ast::ast::ChainElement::PrivateFieldExpression(_) => {
                    Err(self.unsupported("PrivateFieldExpression"))
                }
                _ => Ok(()),
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
                if let Some(arg) = &expr.argument {
                    self.scan_expression(arg)?;
                }
                Ok(())
            }
            _ => Ok(()),
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
        self.scan_formal_parameters(&function.params)
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
            BindingPattern::ArrayPattern(_) | BindingPattern::ObjectPattern(_) => {
                Err(self.unsupported("DestructuringBinding"))
            }
        }
    }

    fn scan_assignment_target(&mut self, target: &AssignmentTarget<'_>) -> Result<(), AbcError> {
        match target {
            AssignmentTarget::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
            }
            AssignmentTarget::ArrayAssignmentTarget(_)
            | AssignmentTarget::ObjectAssignmentTarget(_) => {
                Err(self.unsupported("DestructuringAssignment"))
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

    fn scan_simple_assignment_target(
        &mut self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'_>,
    ) -> Result<(), AbcError> {
        match target {
            oxc_ast::ast::SimpleAssignmentTarget::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
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
            ForStatementLeft::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
            }
            ForStatementLeft::ArrayAssignmentTarget(_)
            | ForStatementLeft::ObjectAssignmentTarget(_) => {
                Err(self.unsupported("DestructuringAssignment"))
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
            PropertyKey::PrivateIdentifier(_) => Err(self.unsupported("PrivateIdentifier")),
            PropertyKey::PrivateFieldExpression(_) => {
                Err(self.unsupported("PrivateFieldExpression"))
            }
            PropertyKey::ComputedMemberExpression(expr) => {
                self.scan_expression(&expr.object)?;
                self.scan_expression(&expr.expression)
            }
            _ => Ok(()),
        }
    }

    fn scan_class_elements(&mut self, elements: &[ClassElement<'_>]) -> Result<(), AbcError> {
        for element in elements {
            match element {
                ClassElement::MethodDefinition(method) => {
                    self.scan_property_key(&method.key)?;
                    self.scan_formal_parameters(&method.value.params)?;
                    if let Some(body) = &method.value.body {
                        self.scan_statements(&body.statements)?;
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    self.scan_property_key(&property.key)?;
                    if let Some(value) = &property.value {
                        self.scan_expression(value)?;
                    }
                }
                ClassElement::AccessorProperty(property) => {
                    self.scan_property_key(&property.key)?;
                    if let Some(value) = &property.value {
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
    line_ops: Vec<u8>,
    catches: Vec<CatchEntry>,
    vregs: u16,
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
            line_ops,
            catches,
            vregs,
        }
    }

    fn empty() -> Self {
        Self::new(Vec::new(), Vec::new(), Vec::new(), 0)
    }
}

fn function_arg_placeholder(arg_index: u16) -> u8 {
    let max_index = (ARG_PLACEHOLDER_LIMIT - 1) as u16;
    let bounded = arg_index.min(max_index);
    ARG_PLACEHOLDER_BASE + bounded as u8
}

fn physicalize_argument_registers(
    bytecode: &mut [u8],
    register_operand_offsets: &[usize],
    num_vregs: u16,
) {
    for &offset in register_operand_offsets {
        let Some(byte) = bytecode.get_mut(offset) else {
            continue;
        };
        if *byte < ARG_PLACEHOLDER_BASE {
            continue;
        }
        let arg_index = (*byte - ARG_PLACEHOLDER_BASE) as u16;
        let physical = num_vregs + arg_index;
        if let Ok(reg) = u8::try_from(physical) {
            *byte = reg;
        }
    }
}

struct CodeGen<'a> {
    strings: &'a mut StringTable,
    /// Literal arrays accumulated by class definitions.
    literal_arrays: &'a mut Vec<LiteralArray>,
    lowering: &'a LoweringContext,
    /// Raw bytecode bytes accumulated so far.
    insns: Vec<u8>,
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
    /// Original source text used for line-number computation, or `None` when
    /// debug line tracking is disabled for this compilation unit.
    source: Option<&'a str>,
    source_mapper: Option<&'a SourceMapper>,
    /// Line number program accumulator.  Present only when `source` is `Some`.
    line_tracker: Option<LineTracker>,
    /// Current source line number for the instruction being emitted.
    current_line: u32,
    /// Current source column number (0-based) for the instruction being emitted.
    current_col: u32,
    function_end: Option<u32>,
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
    catch_entries: Vec<CatchEntry>,
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
            register_operand_offsets: Vec::new(),
            var_regs: FxHashMap::default(),
            next_reg: 0,
            max_next_reg: 0,
            returned: false,
            arrow_captures: ArrowCaptures::default(),
            derived_constructor: false,
            source: None,
            source_mapper: None,
            line_tracker: None,
            current_line: 1,
            current_col: 0,
            function_end: None,
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            catch_entries: Vec::new(),
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
        let tracker = self.line_tracker.take();
        self.push(Insn::NewLexEnv(self.current_lexical_names.len() as u8));
        self.line_tracker = tracker;
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
        let tracker = self.line_tracker.take();
        self.push(Insn::NewLexEnv(self.current_lexical_names.len() as u8));
        self.line_tracker = tracker;
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
            let reg = function_arg_placeholder(3 + idx as u16);
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
                let literal_start = initializer
                    .span()
                    .source_text(self.source.unwrap_or_default())
                    .strip_prefix('-')
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
        self.insns.extend_from_slice(&bytes);
    }

    /// Push the final return instruction, emitting SC(RESET) epilogue marker first.
    ///
    /// Matches libpandafile: before the return instruction, emit SET_COLUMN(RESET)
    /// to mark the epilogue boundary, as seen in all golden binary LNPs.
    fn push_with_epilogue(&mut self, insn: Insn) {
        // Emit SC(RESET=0xFFFFFFFF) to mark the epilogue start.
        // This matches the golden binary pattern: ...SC(R) PC(1)...
        if let Some(tracker) = self.line_tracker.as_mut() {
            tracker.emit_epilogue_column_reset();
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
            self.current_col = col;
        }
    }

    /// Allocate the next virtual register (capped at 254 to leave one spare).
    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_reg;
        if self.next_reg < ARG_PLACEHOLDER_BASE - 1 {
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
        physicalize_argument_registers(&mut self.insns, &self.register_operand_offsets, vregs);
        BodyCompileResult::new(self.insns, Vec::new(), self.catch_entries, vregs)
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
                self.current_line = u32::MAX;
                self.current_col = u32::MAX;
                self.push(Insn::LdUndefined);
                self.push(Insn::ReturnUndefined);
            } else if let Some(end) = self.function_end {
                self.current_line = u32::MAX;
                self.current_col = u32::MAX;
                self.push(Insn::LdUndefined);
                self.set_debug_pos(end);
                self.push(Insn::ReturnUndefined);
            } else {
                self.push_with_epilogue(Insn::LdUndefined);
                self.push_with_epilogue(Insn::ReturnUndefined);
            }
        }
        self.emit_async_cleanup();
        let vregs = self.vreg_count();
        physicalize_argument_registers(&mut self.insns, &self.register_operand_offsets, vregs);
        let line_ops = self.line_tracker.map(|t| t.finish()).unwrap_or_default();
        BodyCompileResult::new(self.insns, line_ops, self.catch_entries, vregs)
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
                if self.line_tracker.is_some() {
                    self.push(Insn::Debugger);
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
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
                let super_class = cls.super_class.as_ref();
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
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
                        let lit_arr_id = self.literal_arrays.len() as u16;
                        self.literal_arrays.push(class_buffer_literal_array(
                            class_name.unwrap_or(&name),
                            &cls.body.body,
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
                    let lit_arr_id = self.literal_arrays.len() as u16;
                    self.literal_arrays.push(class_buffer_literal_array(
                        class_name.unwrap_or(&name),
                        &cls.body.body,
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
                    let lit_arr_id = self.literal_arrays.len() as u16;
                    self.literal_arrays
                        .push(class_buffer_literal_array(class_name, &cls.body.body));
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
                        Declaration::FunctionDeclaration(_) | Declaration::TSEnumDeclaration(_) => {
                        }
                        Declaration::ClassDeclaration(cls) => {
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
                                self.emit_class_missing_properties(
                                    class_name.unwrap_or(&name),
                                    &cls.body.body,
                                    class_reg,
                                    cls.body.span.start,
                                );
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
            Statement::BreakStatement(brk) if brk.label.is_none() => {
                self.emit_control_flow_exit(true);
                if self.returned {
                    return;
                }
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                self.break_patches.push(jmp_pos);
            }
            Statement::ContinueStatement(cont) if cont.label.is_none() => {
                self.emit_control_flow_exit(false);
                if self.returned {
                    return;
                }
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                self.continue_patches.push(jmp_pos);
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
            // Everything else: no bytecode emitted.
            _ => {}
        }
    }

    // ── While loop ────────────────────────────────────────────────────────────

    fn emit_for_in_stmt(&mut self, stmt: &'a ForInStatement<'a>) {
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
        self.catch_entries.push(CatchEntry {
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
            let off = (loop_start as i32 - patch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[patch + 1] = lo;
            self.insns[patch + 2] = hi;
        }

        let jmp_pos = self.insns.len();
        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        self.push(Insn::Jmp(back_off));

        let exit_byte = self.insns.len();
        let jnez_off = (exit_byte as i32 - jnez_pos as i32) as i16;
        let [lo, hi] = jnez_off.to_le_bytes();
        self.insns[jnez_pos + 1] = lo;
        self.insns[jnez_pos + 2] = hi;

        for patch in std::mem::take(&mut self.break_patches) {
            let off = (exit_byte as i32 - patch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[patch + 1] = lo;
            self.insns[patch + 2] = hi;
        }

        self.break_patches = outer_break;
        self.continue_patches = outer_continue;
    }

    fn emit_for_of_stmt(&mut self, stmt: &'a ForOfStatement<'a>) {
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
        self.catch_entries.push(CatchEntry {
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
        self.catch_entries.push(CatchEntry {
            try_begin: body_try_begin,
            try_end: body_try_end,
            catch_begin: body_catch_begin,
            catch_end: body_catch_end,
        });

        for patch in std::mem::take(&mut self.continue_patches) {
            let off = (loop_start as i32 - patch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[patch + 1] = lo;
            self.insns[patch + 2] = hi;
        }

        let jmp_pos = self.insns.len();
        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        self.push(Insn::Jmp(back_off));

        let exit_byte = self.insns.len();
        self.patch_jump_to(done_pos, exit_byte);

        for patch in std::mem::take(&mut self.break_patches) {
            let off = (exit_byte as i32 - patch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[patch + 1] = lo;
            self.insns[patch + 2] = hi;
        }

        self.break_patches = outer_break;
        self.continue_patches = outer_continue;
    }

    fn bind_for_left(&mut self, left: &ForStatementLeft<'_>, value_reg: u8) {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                if let Some(declarator) = decl.declarations.first() {
                    if let BindingPattern::BindingIdentifier(id) = &declarator.id {
                        let name = id.name.as_str().to_string();
                        self.var_regs.insert(name.clone(), value_reg);
                        if let Some(slot) = self.current_lexical_slot(&name) {
                            self.push(Insn::Lda(value_reg));
                            self.push(Insn::StLexVar(0, slot));
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

    /// Patch a list of `jmp(0)` placeholders to the current byte position.
    fn patch_jumps_to_here(&mut self, positions: Vec<usize>) {
        let here = self.insns.len();
        for pos in positions {
            self.patch_jump_to(pos, here);
        }
    }

    /// Patch a list of `jmp(0)` placeholders to a specific byte position.
    fn patch_jumps_to(&mut self, positions: Vec<usize>, target: usize) {
        for pos in positions {
            self.patch_jump_to(pos, target);
        }
    }

    fn patch_jump_to(&mut self, pos: usize, target: usize) {
        let off = target as i32 - pos as i32;
        match self.insns[pos] {
            0x4d | 0x4f | 0x51 => {
                debug_assert!((i8::MIN as i32..=i8::MAX as i32).contains(&off));
                self.insns[pos + 1] = off as i8 as u8;
            }
            _ => {
                let [lo, hi] = (off as i16).to_le_bytes();
                self.insns[pos + 1] = lo;
                self.insns[pos + 2] = hi;
            }
        }
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
        self.catch_entries.push(CatchEntry {
            try_begin,
            try_end,
            catch_begin,
            catch_end,
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
        let loop_start = self.insns.len();
        let false_branch = self.emit_branch_if_false(&while_stmt.test);

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        self.emit_stmt(&while_stmt.body);

        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));
        let loop_exit = self.insns.len();

        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        let [lo, hi] = back_off.to_le_bytes();
        self.insns[jmp_pos + 1] = lo;
        self.insns[jmp_pos + 2] = hi;

        self.patch_jump_to(false_branch, loop_exit);

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, loop_start);
    }

    fn emit_do_while_stmt(&mut self, do_while: &'a oxc_ast::ast::DoWhileStatement<'a>) {
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
        let _ = loop_exit;
    }

    fn emit_for_stmt(&mut self, for_stmt: &'a oxc_ast::ast::ForStatement<'a>) {
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
            self.emit_expr(update);
        }

        let jmp_pos = self.insns.len();
        self.push(Insn::Jmp(0));
        let loop_exit = self.insns.len();

        let back_off = (loop_start as i32 - jmp_pos as i32) as i16;
        let [lo, hi] = back_off.to_le_bytes();
        self.insns[jmp_pos + 1] = lo;
        self.insns[jmp_pos + 2] = hi;

        if let Some(jpos) = jeqz_pos {
            self.patch_jump_to(jpos, loop_exit);
        }

        let breaks = std::mem::replace(&mut self.break_patches, outer_break);
        let continues = std::mem::replace(&mut self.continue_patches, outer_continue);
        self.patch_jumps_to_here(breaks);
        self.patch_jumps_to(continues, continue_target);
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
            self.catch_entries.push(CatchEntry {
                try_begin,
                try_end,
                catch_begin,
                catch_end: after_catch as u32,
            });
            let off = (after_catch as i32 - jmp_past_catch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[jmp_past_catch + 1] = lo;
            self.insns[jmp_past_catch + 2] = hi;
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
            self.catch_entries.push(CatchEntry {
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
            self.catch_entries.push(CatchEntry {
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
        self.catch_entries.push(CatchEntry {
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
                    unreachable!("destructuring bindings are rejected before ABC lowering");
                }
                BindingPattern::AssignmentPattern(_) => {
                    unreachable!("binding assignment patterns are rejected before ABC lowering");
                }
            }
        }
    }

    // ── Expression compiler — result always in accumulator ────────────────────

    fn emit_expr(&mut self, expr: &Expression<'_>) {
        // Update current_line and current_col from the expression's source position
        // for per-instruction LNP tracking.
        self.set_debug_pos(expr.span().start);
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
                let is_nt = is_new_target_meta(meta);
                if is_nt && self.arrow_captures.new_target_slot.is_some() {
                    if let Some((level, slot)) = self.lexical_lookup("new.target") {
                        self.push(Insn::LdLexVar(level, slot));
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                } else {
                    self.push(Insn::LdUndefined);
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
            Expression::PrivateFieldExpression(_) => {
                unreachable!("private fields are rejected before ABC lowering");
            }
            _ => {
                self.push(Insn::LdUndefined);
            }
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
            self.literal_arrays.push(LiteralArray { entries });
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
                entries.push(LiteralEntry::TaggedMethodName(record_name));
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
            PropertyKey::StringLiteral(s) => {
                let name_id = self.intern(s.value.as_str());
                self.push(Insn::LdaStr(name_id));
            }
            PropertyKey::Identifier(id) => {
                self.emit_identifier_load(id.name.as_str());
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
            _ => {
                self.push(Insn::LdUndefined);
            }
        }
        self.push(Insn::Sta(key_reg));
        key_reg
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
            self.literal_arrays.push(LiteralArray { entries });
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
    #[allow(dead_code)]
    fn emit_generator_prologue(&mut self) {
        if !self.is_generator {
            return;
        }
        let reg = self.alloc_reg();
        self.push(Insn::CreateGeneratorObj(function_arg_placeholder(0)));
        self.push(Insn::Sta(reg));
        self.gen_reg = Some(reg);
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
        self.catch_entries.push(CatchEntry {
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
        match &yield_expr.argument {
            Some(arg) => self.emit_expr(arg),
            None => self.push(Insn::LdUndefined),
        }
        if let Some(gen_reg) = self.gen_reg {
            self.push(Insn::SuspendGenerator(gen_reg));
            self.push(Insn::Lda(gen_reg));
            self.push(Insn::ResumeGenerator);
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

        // Register the class literal buffer and use its index as buf_id.
        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays.push(class_buffer_literal_array(
            class_name.unwrap_or("_"),
            elements,
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

        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays
            .push(class_buffer_literal_array(class_name, &cls.body.body));
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

    fn emit_class_missing_properties(
        &mut self,
        _class_name: &str,
        elements: &[ClassElement<'_>],
        class_reg: u8,
        class_span_start: u32,
    ) {
        let prototype_id = self.intern("prototype");
        self.set_debug_pos(class_span_start);
        self.push(Insn::Lda(class_reg));
        self.push(Insn::LdObjByName(0, prototype_id));
        let prototype_reg = self.alloc_reg();
        self.set_debug_pos(class_span_start);
        self.push(Insn::Sta(prototype_reg));

        for element in elements {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };
            if method.kind == MethodDefinitionKind::Constructor || is_class_buffer_method(method) {
                continue;
            }
            let Some(public_name) = method_key_name(&method.key) else {
                continue;
            };
            let record_name = public_name.clone();
            let method_name_id = self.intern(&record_name);
            let length = method.value.params.items.len() as u8;

            if method.kind.is_accessor() {
                let saved_next_reg = self.next_reg;
                let name_reg = self.alloc_reg();
                let undefined_reg = self.alloc_reg();
                let method_reg = self.alloc_reg();
                let public_name_id = self.intern(&public_name);
                self.set_debug_pos(method.key.span().start);
                self.push(Insn::LdaStr(public_name_id));
                self.set_debug_pos(method.key.span().start);
                self.push(Insn::Sta(name_reg));
                self.set_debug_pos(class_span_start);
                if method.kind == MethodDefinitionKind::Get {
                    self.push(Insn::LdUndefined);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Sta(undefined_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Lda(prototype_reg));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::DefineMethodShort(0, method_name_id, length));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::Sta(method_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::LdFalse);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::DefineGetterSetterByValue(
                        prototype_reg,
                        name_reg,
                        method_reg,
                        undefined_reg,
                    ));
                } else {
                    self.push(Insn::LdUndefined);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Sta(undefined_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::Lda(prototype_reg));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::DefineMethodShort(0, method_name_id, length));
                    self.set_debug_pos(method.value.span.start);
                    self.push(Insn::Sta(method_reg));
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::LdFalse);
                    self.set_debug_pos(class_span_start);
                    self.push(Insn::DefineGetterSetterByValue(
                        prototype_reg,
                        name_reg,
                        undefined_reg,
                        method_reg,
                    ));
                }
                self.next_reg = saved_next_reg;
                continue;
            }

            let target_reg = if method.r#static {
                class_reg
            } else {
                prototype_reg
            };
            self.set_debug_pos(class_span_start);
            self.push(Insn::Lda(target_reg));
            self.set_debug_pos(method.value.span.start);
            self.push(Insn::DefineMethodShort(0, method_name_id, length));
            let public_name_id = self.intern(&public_name);
            self.set_debug_pos(method.span.start);
            self.push(Insn::StOwnByName(0, public_name_id, target_reg));
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
            AssignmentTarget::PrivateFieldExpression(_) => {
                unreachable!("private fields are rejected before ABC lowering");
            }
            AssignmentTarget::ArrayAssignmentTarget(_)
            | AssignmentTarget::ObjectAssignmentTarget(_) => {
                unreachable!("destructuring assignments are rejected before ABC lowering");
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
            Expression::PrivateFieldExpression(_) => {
                unreachable!("private fields are rejected before ABC lowering");
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
            SimpleAssignmentTarget::PrivateFieldExpression(_) => {
                unreachable!("private fields are rejected before ABC lowering");
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
            Expression::PrivateFieldExpression(_) => {
                unreachable!("private fields are rejected before ABC lowering");
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
            self.push(Insn::Lda(new_this_reg));
            self.push(Insn::Sta(MANDATORY_PARAM_THIS_REG));
            self.next_reg = saved_next_reg;
            return;
        }

        let first_arg_reg = if argc == 0 {
            let reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(reg));
            reg
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
        self.push(Insn::Lda(new_this_reg));
        self.push(Insn::Sta(MANDATORY_PARAM_THIS_REG));
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
