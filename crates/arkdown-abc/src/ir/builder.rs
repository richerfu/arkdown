use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingPattern, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, ForInStatement, ForOfStatement, ForStatementLeft,
    FormalParameter, FunctionBody, LogicalOperator, MethodDefinition, MethodDefinitionKind,
    PrivateFieldExpression, Program, PropertyKey, Statement, UnaryOperator, YieldExpression,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::{GetSpan, SourceType};

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{CatchEntry, ClassRecord, DebugInfoRecord, MethodKind, MethodRecord};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::ir::ast_analysis::{
    arrow_captures_from_body, is_condition_comparison, is_global_this, is_new_target_meta,
    is_undefined_literal, method_body_contains_arrow, negative_integer_literal,
    stmts_capture_this_in_arrow, ArrowCaptures,
};
use crate::ir::bytecode_metrics::estimate_vregs;
use crate::ir::debug_lnp::{argument_span_start, col_at, line_at, LineTracker, SourceMapper};
use crate::ir::function_records::{
    collect_function_records, FunctionNameAllocator, FunctionRecord, FunctionRecordNode,
};
use crate::ir::lowering_context::LoweringContext;
use crate::ir::module_lexical::collect_module_lexical_bindings;
use crate::ir::module_record::{collect_module_record, module_record_literal_array};
use crate::ir::opcodes::Insn;
use crate::ir::record_layout::RecordLayout;

const MANDATORY_PARAM_THIS_REG: u8 = 2;

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
    build_record_with_debug_and_source_map(
        source,
        record_name,
        strings,
        literal_arrays,
        source_file,
        is_module,
        None,
    )
}

pub fn build_record_with_debug_and_source_map(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
    is_module: bool,
    source_map: Option<&str>,
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

    // Collect module import/export records before lowering so bytecode can use
    // hvigor/es2abc's module-variable opcodes instead of global lookups.
    let module_record = collect_module_record(&program, is_module);
    let lexical_bindings = collect_module_lexical_bindings(&program, is_module);
    let lowering = LoweringContext::from_module_record(&module_record, lexical_bindings);

    let mut methods: Vec<MethodRecord> = Vec::new();

    // Build the `func_main_0` method — the module execution entry point.
    //
    // ArkRuntime always invokes `func_main_0` when executing a module buffer.
    // It passes exactly 3 arguments (JSThread*, this, args[]), so `num_args`
    // must be 3 regardless of the actual JS parameter list.
    //
    // When source-file debug info is requested, we also compute a Panda
    // LineNumberProgram for func_main_0 using statement span offsets.
    let (bytecode, func_main_line_ops, func_main_catches) = if source_file.is_some() {
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
        let (bc, catches) =
            compile_program_body(&program, strings, literal_arrays, &lowering, is_module);
        (bc, vec![], catches)
    };
    let num_vregs = estimate_vregs(&bytecode);
    let bytecode = if is_module {
        prepend_module_argument_copies(bytecode, num_vregs, 3)
    } else {
        bytecode
    };
    methods.push(MethodRecord {
        name: "func_main_0".to_string(),
        bytecode,
        num_vregs,
        num_args: 3,
        debug_info: None,
        line_ops: func_main_line_ops,
        catch_entries: func_main_catches,
        kind: MethodKind::Module,
        method_pool_entry: false,
    });

    // Determine which statement slice to scan for declarations. Upstream
    // module mode scans normal top-level statements.
    let scan_stmts: &[Statement<'_>] = statement_scan_body(&program, is_module);
    let tracked_source = source_file.as_ref().map(|_| source);
    let mut function_names = FunctionNameAllocator::new(source);

    for function in collect_function_records(&program, scan_stmts, &mut function_names) {
        compile_function_record(
            function,
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

    let record_layout = RecordLayout::new(record_name, source);
    let class_name = record_layout.class_name();

    // Literal arrays produced before this point are bytecode operands.  The
    // module-record and scope-name arrays below are class field values and are
    // not emitted into libpandafile's per-record method pool.
    let bytecode_literal_end = literal_arrays.len();

    // Collect module import/export records so that the ABC serializer can emit
    // the compact libpandafile module record buffer used by hvigor/es2abc.
    let module_record_idx = literal_arrays.len();
    literal_arrays.push(module_record_literal_array(&module_record));
    let scope_names_idx = literal_arrays.len();
    literal_arrays.push(scope_names_literal_array(scan_stmts, record_name));

    // The 6 module metadata fields that es2abc always writes into every user
    // class record.  ArkRuntime reads these to understand how to load the
    // module's dependencies.
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
        module_literal_indices: vec![scope_names_idx, module_record_idx],
        source_code_strings,
        access_flags: 1,
        source_lang: 0x04,
        source_file,
        module_requests: module_record.module_requests,
        fields: module_fields,
    })
}

fn lexical_names_literal_array(names: &[String]) -> LiteralArray {
    let mut entries = Vec::with_capacity(names.len() * 3);
    for (idx, name) in names.iter().enumerate() {
        entries.push(LiteralEntry::TaggedInteger(1));
        entries.push(LiteralEntry::TaggedString(name.clone()));
        entries.push(LiteralEntry::TaggedInteger(idx as u32));
    }
    LiteralArray { entries }
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
            let (bytecode, line_ops, catches) = if rest_info.is_some() || func.r#async {
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
                    )
                    .unwrap_or_else(|| (vec![], vec![], vec![]))
                } else {
                    let (bytecode, catches) = compile_function_body_ext(
                        func.body.as_deref(),
                        rest_info,
                        func.r#async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                    );
                    (bytecode, Vec::new(), catches)
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
                )
            } else {
                let (bytecode, catches) = compile_function_body(
                    func.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                );
                (bytecode, Vec::new(), catches)
            };
            let num_vregs = estimate_vregs(&bytecode);
            methods.push(MethodRecord {
                name: function.name,
                bytecode,
                num_vregs,
                num_args: func.params.items.len() as u16,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: 1,
                    source_file: path.to_string(),
                    param_names,
                    lexical_this: func
                        .body
                        .as_deref()
                        .is_some_and(|body| stmts_capture_this_in_arrow(&body.statements)),
                }),
                line_ops,
                catch_entries: catches,
                kind: MethodKind::Function,
                method_pool_entry: false,
            });
        }
        FunctionRecordNode::Arrow(arrow) => {
            let captures = arrow_captures_from_body(arrow.body.as_ref());
            let param_names = arrow_param_names(arrow);
            let (bytecode, line_ops, catches) = compile_arrow_body_with_line_ops(
                arrow.body.as_ref(),
                strings,
                literal_arrays,
                lowering,
                captures,
                &param_names,
                arrow.expression,
                source,
                source_mapper,
            );
            let num_vregs = estimate_vregs(&bytecode);
            let num_args = arrow.params.items.len() as u16 + 3;
            let bytecode = prepend_function_argument_copies(bytecode, num_vregs, num_args);
            methods.push(MethodRecord {
                name: function.name,
                bytecode,
                num_vregs,
                num_args,
                debug_info: source_file_path.map(|path| DebugInfoRecord {
                    line_start: 1,
                    source_file: path.to_string(),
                    param_names,
                    lexical_this: captures.this_slot.is_some(),
                }),
                line_ops,
                catch_entries: catches,
                kind: MethodKind::Anonymous,
                method_pool_entry: true,
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
        let (bytecode, line_ops, catches) = if is_default_derived_constructor {
            let (bytecode, catches) = compile_default_derived_constructor();
            let line_ops = if source.is_some() {
                default_derived_constructor_line_ops()
            } else {
                Vec::new()
            };
            (bytecode, line_ops, catches)
        } else {
            let (bytecode, catches) = compile_constructor_with_fields(
                None,
                &[],
                0,
                &instance_fields,
                strings,
                literal_arrays,
                lowering,
                &[],
                has_super_class,
            );
            (bytecode, Vec::new(), catches)
        };
        let num_vregs = if is_default_derived_constructor {
            6
        } else {
            estimate_vregs(&bytecode)
        };
        methods.push(MethodRecord {
            name: function_names.bind(0, 0, Some(class_name)),
            bytecode,
            num_vregs,
            num_args: 4,
            debug_info: None,
            line_ops,
            catch_entries: catches,
            kind: MethodKind::Constructor,
            method_pool_entry: true,
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

            let (bytecode, method_line_ops, method_catches) = if is_constructor {
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
                    )
                } else {
                    let (bc, catches) = compile_constructor_with_fields(
                        method.value.body.as_deref(),
                        &method.value.params.items,
                        method.value.span.start,
                        &instance_fields,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                        has_super_class,
                    );
                    (bc, vec![], catches)
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
                    )
                    .unwrap_or_else(|| (vec![], vec![], vec![]))
                } else {
                    let (bc, catches) = compile_function_body_ext(
                        method.value.body.as_deref(),
                        rest_info,
                        is_method_async,
                        strings,
                        literal_arrays,
                        lowering,
                        &param_names,
                    );
                    (bc, vec![], catches)
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
                )
            } else {
                let (bc, catches) = compile_function_body(
                    method.value.body.as_deref(),
                    strings,
                    literal_arrays,
                    lowering,
                    &param_names,
                );
                (bc, vec![], catches)
            };
            let num_vregs = estimate_vregs(&bytecode).max(4 + param_names.len() as u16);
            let num_args = class_method_num_args(method);
            let bytecode = prepend_function_argument_copies(bytecode, num_vregs, num_args);
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
                bytecode,
                num_vregs,
                num_args,
                debug_info,
                line_ops: method_line_ops,
                catch_entries: method_catches,
                kind: if is_constructor {
                    MethodKind::Constructor
                } else {
                    MethodKind::Function
                },
                method_pool_entry: is_class_buffer_method(method),
            });
        }
    }
}

fn class_method_record_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
    function_names: &mut FunctionNameAllocator<'_>,
) -> Option<String> {
    let name = if method.kind == MethodDefinitionKind::Constructor {
        class_name.to_string()
    } else {
        method_key_name(&method.key)?
    };
    Some(function_names.bind(method.value.span.start, method.value.span.end, Some(&name)))
}

fn class_method_num_args(method: &MethodDefinition<'_>) -> u16 {
    method.value.params.items.len() as u16 + 3
}

fn prepend_function_argument_copies(
    mut bytecode: Vec<u8>,
    num_vregs: u16,
    num_args: u16,
) -> Vec<u8> {
    if num_args == 0 {
        return bytecode;
    }

    let has_this_lexenv = bytecode.first() == Some(&0xb6) && bytecode.get(1) == Some(&1);
    let mut prologue = Vec::new();
    for dst in 0..num_args {
        if has_this_lexenv && dst == MANDATORY_PARAM_THIS_REG as u16 {
            continue;
        }
        let src = num_vregs + dst;
        if dst <= 0x0f && src <= 0x0f {
            prologue.extend_from_slice(
                &Insn::MovV4(dst as u8, src as u8).encode(&mut StringTable::default()),
            );
        } else if src <= u8::MAX as u16 {
            prologue.extend_from_slice(
                &Insn::MovV8(dst as u8, src as u8).encode(&mut StringTable::default()),
            );
        } else {
            prologue.extend_from_slice(&Insn::MovV16(dst, src).encode(&mut StringTable::default()));
        }
    }

    if has_this_lexenv {
        let lexenv: Vec<u8> = bytecode.drain(..4).collect();
        let this_src = num_vregs + MANDATORY_PARAM_THIS_REG as u16;
        let mut with_lexenv = lexenv;
        with_lexenv.append(&mut prologue);
        if this_src <= u8::MAX as u16 {
            with_lexenv
                .extend_from_slice(&Insn::Lda(this_src as u8).encode(&mut StringTable::default()));
            with_lexenv
                .extend_from_slice(&Insn::StLexVar(0, 0).encode(&mut StringTable::default()));
        }
        with_lexenv.append(&mut bytecode);
        return with_lexenv;
    }

    prologue.append(&mut bytecode);
    prologue
}

fn prepend_module_argument_copies(mut bytecode: Vec<u8>, num_vregs: u16, num_args: u16) -> Vec<u8> {
    if num_args == 0 {
        return bytecode;
    }
    let mut prologue = Vec::new();
    for dst in 0..num_args {
        let src = num_vregs + dst;
        if dst <= 0x0f && src <= 0x0f {
            prologue.extend_from_slice(
                &Insn::MovV4(dst as u8, src as u8).encode(&mut StringTable::default()),
            );
        } else if src <= u8::MAX as u16 {
            prologue.extend_from_slice(
                &Insn::MovV8(dst as u8, src as u8).encode(&mut StringTable::default()),
            );
        } else {
            prologue.extend_from_slice(&Insn::MovV16(dst, src).encode(&mut StringTable::default()));
        }
    }

    if bytecode.first() == Some(&0xb6) && bytecode.len() >= 4 {
        let mut with_lexenv: Vec<u8> = bytecode.drain(..4).collect();
        with_lexenv.append(&mut prologue);
        with_lexenv.append(&mut bytecode);
        return with_lexenv;
    }

    prologue.append(&mut bytecode);
    prologue
}

fn method_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_string()),
        _ => None,
    }
}

fn class_buffer_literal_array(_class_name: &str, elements: &[ClassElement<'_>]) -> LiteralArray {
    let mut entries = Vec::new();
    let mut instance_method_count = 0u32;

    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };

        if !is_class_buffer_method(method) {
            continue;
        }
        let Some(public_name) = method_key_name(&method.key) else {
            continue;
        };

        entries.push(LiteralEntry::TaggedString(public_name.clone()));
        entries.push(LiteralEntry::TaggedMethodName(public_name));
        entries.push(LiteralEntry::TaggedMethodAffiliate(
            method.value.params.items.len() as u16,
        ));
        instance_method_count += 1;
    }

    entries.push(LiteralEntry::TaggedInteger(instance_method_count));
    LiteralArray { entries }
}

fn is_class_buffer_method(method: &MethodDefinition<'_>) -> bool {
    if method.kind == MethodDefinitionKind::Constructor || method.kind.is_accessor() {
        return false;
    }
    if method.r#static {
        return false;
    }
    let Some(key) = method_key_name(&method.key) else {
        return false;
    };
    !matches!(key.as_str(), "initialRender" | "rerender")
}

fn constructor_required_param_count(elements: &[ClassElement<'_>]) -> u16 {
    elements
        .iter()
        .find_map(|element| {
            let ClassElement::MethodDefinition(method) = element else {
                return None;
            };
            (method.kind == MethodDefinitionKind::Constructor).then(|| {
                method
                    .value
                    .params
                    .items
                    .iter()
                    .take_while(|param| param.initializer.is_none())
                    .count() as u16
            })
        })
        .unwrap_or(0)
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
) -> (Vec<u8>, Vec<CatchEntry>) {
    compile_body_stmts(&program.body, strings, literal_arrays, lowering, true, &[])
}

/// Compile a function body's statements; returns `(bytecode, catch_entries)`.
fn compile_function_body(
    body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
) -> (Vec<u8>, Vec<CatchEntry>) {
    match body {
        Some(fb) => compile_body_stmts(
            &fb.statements,
            strings,
            literal_arrays,
            lowering,
            false,
            param_names,
        ),
        None => {
            let mut st = StringTable::default();
            (Insn::ReturnUndefined.encode(&mut st), vec![])
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
    compile_function_body_tracked_with_end(
        body,
        strings,
        literal_arrays,
        lowering,
        param_names,
        source,
        source_mapper,
        None,
    )
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
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
        ),
        None => {
            let mut st = StringTable::default();
            (Insn::ReturnUndefined.encode(&mut st), vec![], vec![])
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
    let mut gen = CodeGen::new_arrow_with_source(
        strings,
        literal_arrays,
        lowering,
        captures,
        source,
        source_mapper,
    );
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
/// When `is_async` is `true`:
/// - Emits `asyncfunctionenter; sta async_reg` as a prologue.
/// - Emits `asyncfunctionresolve 0, async_reg` before every return.
fn compile_function_body_ext(
    body: Option<&FunctionBody<'_>>,
    rest_param: Option<(String, u8)>,
    is_async: bool,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    lowering: &LoweringContext,
    param_names: &[String],
) -> (Vec<u8>, Vec<CatchEntry>) {
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
    )
    .map(|(bc, _, catches)| (bc, catches))
    .unwrap_or_else(|| (vec![], vec![]))
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
) -> Option<(Vec<u8>, Vec<u8>, Vec<CatchEntry>)> {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => {
            let mut st = StringTable::default();
            let bc = Insn::ReturnUndefined.encode(&mut st);
            return Some((bc, vec![], vec![]));
        }
    };
    let mut cg = if let Some(src) = source {
        CodeGen::new_with_source(strings, literal_arrays, lowering, src, source_mapper)
    } else {
        CodeGen::new(strings, literal_arrays, lowering)
    };
    cg.is_async = is_async;
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
    let (bc, line_ops, catches) = cg.finish_with_line_ops_and_catches();
    Some((bc, line_ops, catches))
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
) -> (Vec<u8>, Vec<CatchEntry>) {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.derived_constructor = has_super_class;
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
    let stmts: &[_] = match body {
        Some(fb) => fb.statements.as_slice(),
        None => &[],
    };
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    cg.derived_constructor = has_super_class;
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
) -> (Vec<u8>, Vec<CatchEntry>) {
    let mut cg = CodeGen::new(strings, literal_arrays, lowering);
    cg.module_body = module_body;
    if module_body {
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
    } else {
        if stmts_capture_this_in_arrow(stmts) {
            cg.emit_function_this_lex_env_prologue();
        }
        cg.bind_function_param_names(param_names);
    }
    for stmt in stmts {
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
    let mut cg = CodeGen::new_with_source(strings, literal_arrays, lowering, source, source_mapper);
    cg.function_end = function_end;
    cg.module_body = module_body;
    if module_body {
        cg.emit_module_lex_env_prologue();
        cg.bind_param_names(param_names);
    } else {
        if stmts_capture_this_in_arrow(stmts) {
            cg.emit_function_this_lex_env_prologue();
        }
        cg.bind_function_param_names(param_names);
    }
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish_with_line_ops_and_catches()
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
) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
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

// ── CodeGen ───────────────────────────────────────────────────────────────────

struct CodeGen<'a> {
    strings: &'a mut StringTable,
    /// Literal arrays accumulated by class definitions.
    literal_arrays: &'a mut Vec<LiteralArray>,
    lowering: &'a LoweringContext,
    /// Raw bytecode bytes accumulated so far.
    insns: Vec<u8>,
    /// Maps JS binding name → virtual register number.
    var_regs: HashMap<String, u8>,
    /// Next free register index.
    next_reg: u8,
    /// Set to `true` after a `return` or `returnundefined` is emitted so that
    /// unreachable code is not generated.
    returned: bool,
    arrow_captures: ArrowCaptures,
    function_this_capture_env: bool,
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
    module_body: bool,
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
            var_regs: HashMap::new(),
            next_reg: 0,
            returned: false,
            arrow_captures: ArrowCaptures::default(),
            function_this_capture_env: false,
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
            module_body: false,
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

    fn emit_module_lex_env_prologue(&mut self) {
        if self.lowering.lexical_names.is_empty() {
            return;
        }
        let literal_id = self.literal_arrays.len() as u16;
        self.literal_arrays
            .push(lexical_names_literal_array(&self.lowering.lexical_names));
        let tracker = self.line_tracker.take();
        self.push(Insn::NewLexEnvWithName(
            self.lowering.lexical_names.len() as u8,
            literal_id,
        ));
        self.line_tracker = tracker;
        let reserves_class_binding = self.lowering.lexical_names.iter().any(|name| {
            name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().any(char::is_lowercase)
        });
        self.next_reg = self
            .next_reg
            .max(if reserves_class_binding { 5 } else { 4 });
    }

    fn emit_function_this_lex_env_prologue(&mut self) {
        let literal_id = self.literal_arrays.len() as u16;
        self.literal_arrays
            .push(lexical_names_literal_array(&["this".to_string()]));
        let tracker = self.line_tracker.take();
        self.push(Insn::NewLexEnvWithName(1, literal_id));
        self.line_tracker = tracker;
        self.function_this_capture_env = true;
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

    fn bind_function_param_names(&mut self, param_names: &[String]) {
        for (idx, name) in param_names.iter().enumerate() {
            if self.var_regs.contains_key(name) {
                continue;
            }
            let reg = 3u8.saturating_add(idx as u8);
            self.var_regs.insert(name.clone(), reg);
        }

        // Upstream keeps the mandatory params in v0..v2, user params from v3,
        // and leaves one spill slot before RegScope temporaries. See
        // binder::MANDATORY_PARAMS_NUMBER and PandaGen::CopyFunctionArguments.
        let next = 4u8.saturating_add(param_names.len() as u8);
        self.next_reg = self.next_reg.max(next);
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
        if self.next_reg < 254 {
            self.next_reg += 1;
        }
        r
    }

    /// Intern a string and return its 16-bit index.
    fn intern(&mut self, s: &str) -> u16 {
        self.strings.intern(s) as u16
    }

    fn finish_with_catches(mut self) -> (Vec<u8>, Vec<CatchEntry>) {
        if !self.returned {
            if let Some(areg) = self.async_reg {
                self.push(Insn::LdUndefined);
                self.push(Insn::AsyncFunctionResolve(0, areg));
            }
            self.push_with_epilogue(Insn::LdUndefined);
            self.push_with_epilogue(Insn::ReturnUndefined);
        }
        (self.insns, self.catch_entries)
    }

    fn finish_with_line_ops_and_catches(mut self) -> (Vec<u8>, Vec<u8>, Vec<CatchEntry>) {
        if !self.returned {
            if let Some(areg) = self.async_reg {
                self.push(Insn::LdUndefined);
                self.push(Insn::AsyncFunctionResolve(0, areg));
            }
            if let Some(end) = self.function_end {
                self.current_line = u32::MAX;
                self.current_col = u32::MAX;
                self.push(Insn::LdUndefined);
                self.set_debug_pos(end);
                self.push(Insn::ReturnUndefined);
            } else if self.module_body && self.source.is_some() {
                self.current_line = u32::MAX;
                self.current_col = u32::MAX;
                self.push(Insn::LdUndefined);
                self.push(Insn::ReturnUndefined);
            } else {
                self.push_with_epilogue(Insn::LdUndefined);
                self.push_with_epilogue(Insn::ReturnUndefined);
            }
        }
        let line_ops = self.line_tracker.map(|t| t.finish()).unwrap_or_default();
        (self.insns, line_ops, self.catch_entries)
    }

    // ── Statement dispatcher ──────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Statement<'_>) {
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
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.emit_expr(arg);
                    if let Some(areg) = self.async_reg {
                        self.push(Insn::AsyncFunctionResolve(0, areg));
                    }
                    if let Some(end) = self.function_end {
                        self.set_debug_pos(end);
                        self.push(Insn::Return);
                    } else {
                        self.push_with_epilogue(Insn::Return);
                    }
                } else {
                    self.push(Insn::LdUndefined);
                    self.current_line = u32::MAX;
                    self.current_col = u32::MAX;
                    self.push(Insn::LdUndefined);
                    if let Some(areg) = self.async_reg {
                        self.push(Insn::AsyncFunctionResolve(0, areg));
                    }
                    if let Some(end) = self.function_end {
                        self.set_debug_pos(end);
                        self.push(Insn::ReturnUndefined);
                    } else {
                        self.push_with_epilogue(Insn::ReturnUndefined);
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
                            0
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
                        if let Some(lex_idx) = self.lowering.lexical_index(&name) {
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
                        0
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
                    if let Some(lex_idx) = self.lowering.lexical_index(&name) {
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Lda(class_reg));
                        self.set_debug_pos(id.span.start);
                        self.push(Insn::Sta(class_reg));
                        self.push(Insn::Lda(class_reg));
                        self.push(Insn::StLexVar(0, lex_idx));
                        self.var_regs.remove(&name);
                        self.next_reg = saved_next_reg;
                    } else {
                        let name_id = self.intern(&name);
                        self.push(Insn::Lda(class_reg));
                        self.push(Insn::TryStGlobalByName(0, name_id));
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
                    let class_name = local_name.unwrap_or("default");
                    let class_reg = self.alloc_reg();
                    if let Some(name) = local_name {
                        self.var_regs.insert(name.to_string(), class_reg + 1);
                        self.next_reg = self.next_reg.max(class_reg.saturating_add(2));
                    }
                    let proto_vreg = if let Some(sc) = &cls.super_class {
                        self.emit_expr(sc);
                        let reg = self.alloc_reg();
                        self.set_debug_pos(cls.body.span.start);
                        self.push(Insn::Sta(reg));
                        reg
                    } else {
                        0
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
                        if let Some(export_index) = self.lowering.export_index("default") {
                            self.push(Insn::Lda(class_reg));
                            self.push(Insn::StModuleVar(export_index));
                        }
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::ClassDeclaration(cls)) = &export.declaration {
                    let super_class = cls.super_class.as_ref();
                    let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                    self.emit_class_inner(class_name, &cls.body.body, super_class);
                    if let Some(id) = &cls.id {
                        let reg = self.alloc_reg();
                        self.push(Insn::Sta(reg));
                        let name = id.name.as_str().to_string();
                        self.var_regs.insert(name.clone(), reg);
                        let name_id = self.intern(&name);
                        self.push(Insn::Lda(reg));
                        self.push(Insn::TryStGlobalByName(0, name_id));
                        if let Some(export_index) = self.lowering.export_index(&name) {
                            self.push(Insn::Lda(reg));
                            self.push(Insn::StModuleVar(export_index));
                        }
                    }
                }
            }
            Statement::FunctionDeclaration(func) => {
                // TODO: use the real method index from the class literal array.
                if let Some(id) = &func.id {
                    let name_id = self.intern(id.name.as_str());
                    let length = func.params.items.len() as u8;
                    self.push(Insn::DefineFuncShort(0, 0, length));
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
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
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                self.break_patches.push(jmp_pos);
            }
            Statement::ContinueStatement(cont) if cont.label.is_none() => {
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

    fn emit_for_in_stmt(&mut self, stmt: &ForInStatement<'_>) {
        self.emit_expr(&stmt.right);
        self.push(Insn::GetPropIterator);
        let iter_reg = self.alloc_reg();
        self.push(Insn::Sta(iter_reg));

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        let loop_start = self.insns.len();
        self.push(Insn::GetNextPropName(iter_reg));
        let jeqz_pos = self.insns.len();
        self.push(Insn::Jeqz(0));

        let key_reg = self.alloc_reg();
        self.push(Insn::Sta(key_reg));
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
        let jeqz_off = (exit_byte as i32 - jeqz_pos as i32) as i16;
        let [lo, hi] = jeqz_off.to_le_bytes();
        self.insns[jeqz_pos + 1] = lo;
        self.insns[jeqz_pos + 2] = hi;

        for patch in std::mem::take(&mut self.break_patches) {
            let off = (exit_byte as i32 - patch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[patch + 1] = lo;
            self.insns[patch + 2] = hi;
        }

        self.break_patches = outer_break;
        self.continue_patches = outer_continue;
    }

    fn emit_for_of_stmt(&mut self, stmt: &ForOfStatement<'_>) {
        self.emit_expr(&stmt.right);
        self.push(Insn::GetIterator(0));
        let iter_reg = self.alloc_reg();
        self.push(Insn::Sta(iter_reg));

        let next_id = self.intern("next");
        let done_id = self.intern("done");
        let value_id = self.intern("value");
        let result_reg = self.alloc_reg();

        let outer_break = std::mem::take(&mut self.break_patches);
        let outer_continue = std::mem::take(&mut self.continue_patches);

        let loop_start = self.insns.len();

        self.push(Insn::Lda(iter_reg));
        self.push(Insn::LdObjByName(0, next_id));
        self.push(Insn::CallThis0(0, iter_reg));
        self.push(Insn::Sta(result_reg));
        self.push(Insn::Lda(result_reg));
        self.push(Insn::LdObjByName(0, done_id));
        let jnez_pos = self.insns.len();
        self.push(Insn::Jnez(0));

        self.push(Insn::Lda(result_reg));
        self.push(Insn::LdObjByName(0, value_id));
        let val_reg = self.alloc_reg();
        self.push(Insn::Sta(val_reg));
        self.bind_for_left(&stmt.left, val_reg);

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

    fn bind_for_left(&mut self, left: &ForStatementLeft<'_>, value_reg: u8) {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                if let Some(declarator) = decl.declarations.first() {
                    if let BindingPattern::BindingIdentifier(id) = &declarator.id {
                        let name = id.name.as_str().to_string();
                        self.var_regs.insert(name.clone(), value_reg);
                        if let Some(slot) = self.lowering.lexical_index(&name) {
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
    fn emit_if_stmt(&mut self, if_stmt: &oxc_ast::ast::IfStatement<'_>) {
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
        self.push(Insn::CallRuntimeIsFalse(0));
        let branch_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        branch_pos
    }

    fn emit_branch_if_true_from_acc(&mut self) -> usize {
        self.push(Insn::CallRuntimeIsTrue(0));
        let branch_pos = self.insns.len();
        self.push(Insn::Jnez(0));
        branch_pos
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

    fn emit_while_stmt(&mut self, while_stmt: &oxc_ast::ast::WhileStatement<'_>) {
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

    fn emit_do_while_stmt(&mut self, do_while: &oxc_ast::ast::DoWhileStatement<'_>) {
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

    fn emit_for_stmt(&mut self, for_stmt: &oxc_ast::ast::ForStatement<'_>) {
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

    // ── Try/catch/finally statement ───────────────────────────────────────────
    //
    // Panda exception dispatch uses a per-method CatchTable.  Each CatchEntry
    // maps a try region (byte offsets [try_begin, try_end)) to a catch handler
    // entry point (catch_begin).  At catch_begin the caught exception value is
    // already in the accumulator — no GetException opcode exists in this ISA.
    //
    // Layout emitted:
    //
    //   try_begin:
    //     <try body>
    //   try_end:           ← exclusive end of protected region
    //     [jmp after]      ← skip catch (only if handler present)
    //   catch_begin:
    //     [sta param_reg]  ← save exception (only if catch param exists)
    //     <catch body>
    //   after:
    //     <finally body>   ← inlined after both paths (if finalizer present)
    //
    // For try-finally with no catch: CatchEntry is not emitted; the try body
    // and finalizer body are emitted sequentially (simplified lowering).
    fn emit_try_stmt(&mut self, ts: &oxc_ast::ast::TryStatement<'_>) {
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

            // Record the exception table entry.
            self.catch_entries.push(CatchEntry {
                try_begin,
                try_end,
                catch_begin,
            });

            // Patch jmp_past_catch → current position.
            let after_catch = self.insns.len();
            let off = (after_catch as i32 - jmp_past_catch as i32) as i16;
            let [lo, hi] = off.to_le_bytes();
            self.insns[jmp_past_catch + 1] = lo;
            self.insns[jmp_past_catch + 2] = hi;
        }

        // Emit finalizer inline after both try and catch paths.
        if let Some(fin) = &ts.finalizer {
            let fin_outer_returned = self.returned;
            self.returned = false;
            for s in &fin.body {
                self.emit_stmt(s);
                if self.returned {
                    break;
                }
            }
            self.returned = fin_outer_returned;
        }
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
                    if let Some(slot) = self.lowering.lexical_index(&name) {
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
                BindingPattern::ArrayPattern(arr_pat) => {
                    if let Some(init) = &declarator.init {
                        self.emit_expr(init);
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    let tmp_reg = self.alloc_reg();
                    self.push(Insn::Sta(tmp_reg));
                    for (i, elem) in arr_pat.elements.iter().enumerate() {
                        let Some(pat) = elem else { continue };
                        let BindingPattern::BindingIdentifier(bid) = pat else {
                            continue;
                        };
                        let vname = bid.name.as_str().to_string();
                        let idx_str = i.to_string();
                        let name_id = self.intern(&idx_str);
                        self.push(Insn::Lda(tmp_reg));
                        self.push(Insn::LdObjByName(0, name_id));
                        let reg = self.alloc_reg();
                        self.var_regs.insert(vname.clone(), reg);
                        self.push(Insn::Sta(reg));
                        if let Some(slot) = self.lowering.lexical_index(&vname) {
                            self.push(Insn::Lda(reg));
                            self.push(Insn::StLexVar(0, slot));
                        }
                    }
                }
                BindingPattern::ObjectPattern(obj_pat) => {
                    if let Some(init) = &declarator.init {
                        self.emit_expr(init);
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    let tmp_reg = self.alloc_reg();
                    self.push(Insn::Sta(tmp_reg));
                    for prop in &obj_pat.properties {
                        let key_name = match &prop.key {
                            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                            PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                            _ => continue,
                        };
                        let BindingPattern::BindingIdentifier(bid) = &prop.value else {
                            continue;
                        };
                        let vname = bid.name.as_str().to_string();
                        let name_id = self.intern(&key_name);
                        self.push(Insn::Lda(tmp_reg));
                        self.push(Insn::LdObjByName(0, name_id));
                        let reg = self.alloc_reg();
                        self.var_regs.insert(vname.clone(), reg);
                        self.push(Insn::Sta(reg));
                        if let Some(slot) = self.lowering.lexical_index(&vname) {
                            self.push(Insn::Lda(reg));
                            self.push(Insn::StLexVar(0, slot));
                        }
                        if let Some(export_index) = self.lowering.export_index(&vname) {
                            self.push(Insn::Lda(reg));
                            self.push(Insn::StModuleVar(export_index));
                        }
                    }
                }
                _ => {}
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
            Expression::StringLiteral(s) => {
                let id = self.intern(s.value.as_str());
                self.push(Insn::LdaStr(id));
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
                    _ => {
                        if let Some(&reg) = self.var_regs.get(name) {
                            self.push(Insn::Lda(reg));
                        } else if let Some(slot) = self.lowering.lexical_index(name) {
                            self.push(Insn::LdLexVar(0, slot));
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
                }
            }
            Expression::ThisExpression(_) => {
                if let Some(slot) = self.arrow_captures.this_slot {
                    // Arrow function: `this` was captured from the enclosing
                    // scope into a lexical environment slot.
                    self.push(Insn::LdLexVar(0, slot));
                } else if self.function_this_capture_env {
                    self.push(Insn::LdLexVar(0, 0));
                } else {
                    // PandaGen::GetThis loads binder::MANDATORY_PARAM_THIS.
                    self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
                    if self.derived_constructor {
                        self.push(Insn::ThrowIfSuperNotCorrectCall(0));
                    }
                }
            }
            Expression::MetaProperty(meta) => {
                let is_nt = is_new_target_meta(meta);
                match (is_nt, self.arrow_captures.new_target_slot) {
                    (true, Some(slot)) => self.push(Insn::LdLexVar(0, slot)),
                    _ => self.push(Insn::LdUndefined),
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
            Expression::BinaryExpression(bin) => {
                self.emit_binary_expr(bin);
            }
            Expression::UnaryExpression(unary) => {
                self.emit_unary_expr(unary);
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
                let captures = arrow_captures_from_body(arrow.body.as_ref());
                if !captures.is_empty() && !self.function_this_capture_env {
                    // hvigor uses `newlexenvwithname` (0xb6) instead of plain
                    // `newlexenv` (0x09) so that ArkRuntime / debugger can map
                    // each lexical slot back to its source identifier.
                    // Build a lexical-names literal array: slot-0 = "this" when
                    // the arrow captures `this`, slot-1 = "new.target" if present.
                    let mut lex_names: Vec<String> = Vec::new();
                    if captures.this_slot.is_some() {
                        lex_names.push("this".to_string());
                    }
                    if captures.new_target_slot.is_some() {
                        lex_names.push("new.target".to_string());
                    }
                    let lit_id = self.literal_arrays.len() as u16;
                    self.literal_arrays
                        .push(lexical_names_literal_array(&lex_names));
                    self.push(Insn::NewLexEnvWithName(captures.len(), lit_id));
                    if let Some(slot) = captures.this_slot {
                        // Arrow functions capture the enclosing mandatory
                        // `this` binding into their lexical environment.
                        self.push(Insn::Lda(MANDATORY_PARAM_THIS_REG));
                        self.push(Insn::StLexVar(0, slot));
                    }
                    if let Some(slot) = captures.new_target_slot {
                        self.push(Insn::Lda(2));
                        self.push(Insn::StLexVar(0, slot));
                    }
                }
                let length = arrow.params.items.len() as u8;
                self.push(Insn::DefineFuncShort(0, 0, length));
            }
            Expression::FunctionExpression(func) => {
                // Named or anonymous function expression: `function(params) {}`.
                let length = func.params.items.len() as u8;
                self.push(Insn::DefineFuncShort(0, 0, length));
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
            Expression::ChainExpression(chain) => {
                self.emit_chain_expr(chain);
            }
            Expression::ArrayExpression(arr) => {
                self.emit_array_expr(arr);
            }
            Expression::AwaitExpression(await_expr) => {
                // Simplified lowering: evaluate the awaited expression and leave
                // the result in the accumulator.  The Panda ISA used here does not
                // expose Await/Resume/Suspend opcodes at the IR level — async
                // function semantics are handled natively by ArkRuntime's Promise
                // machinery when the function is declared `async`.  Emitting just
                // the argument value is correct for common component lifecycle
                // methods (e.g. `await storage.get(key)`) where the caller's
                // `async` attribute and the runtime's implicit Promise wrapping
                // together ensure the awaited value is unwrapped at runtime.
                self.emit_expr(&await_expr.argument);
            }
            Expression::YieldExpression(yield_expr) => {
                self.emit_yield_expr(yield_expr);
            }
            Expression::PrivateFieldExpression(priv_expr) => {
                self.emit_private_field(priv_expr);
            }
            _ => {
                self.push(Insn::LdUndefined);
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
                let null_reg = self.alloc_reg();
                self.push(Insn::LdNull);
                self.push(Insn::Sta(null_reg));
                self.push(Insn::Lda(obj_reg));
                self.push(Insn::Eq(0, null_reg));
                let jnez_pos = self.insns.len();
                self.push(Insn::Jnez(0));
                self.push(Insn::Lda(obj_reg));
                let name_id = self.intern(mem.property.name.as_str());
                self.push(Insn::LdObjByName(0, name_id));
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                let null_start = self.insns.len();
                let [lo, hi] = ((null_start as i32 - jnez_pos as i32) as i16).to_le_bytes();
                self.insns[jnez_pos + 1] = lo;
                self.insns[jnez_pos + 2] = hi;
                self.push(Insn::LdUndefined);
                let end = self.insns.len();
                let [lo2, hi2] = ((end as i32 - jmp_pos as i32) as i16).to_le_bytes();
                self.insns[jmp_pos + 1] = lo2;
                self.insns[jmp_pos + 2] = hi2;
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(mem) => {
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.push(Insn::Sta(obj_reg));
                let null_reg = self.alloc_reg();
                self.push(Insn::LdNull);
                self.push(Insn::Sta(null_reg));
                self.push(Insn::Lda(obj_reg));
                self.push(Insn::Eq(0, null_reg));
                let jnez_pos = self.insns.len();
                self.push(Insn::Jnez(0));
                self.push(Insn::Lda(obj_reg));
                if let Expression::StringLiteral(key) = &mem.expression {
                    let name_id = self.intern(key.value.as_str());
                    self.push(Insn::LdObjByName(0, name_id));
                } else {
                    self.push(Insn::LdUndefined);
                }
                let jmp_pos = self.insns.len();
                self.push(Insn::Jmp(0));
                let null_start = self.insns.len();
                let [lo, hi] = ((null_start as i32 - jnez_pos as i32) as i16).to_le_bytes();
                self.insns[jnez_pos + 1] = lo;
                self.insns[jnez_pos + 2] = hi;
                self.push(Insn::LdUndefined);
                let end = self.insns.len();
                let [lo2, hi2] = ((end as i32 - jmp_pos as i32) as i16).to_le_bytes();
                self.insns[jmp_pos + 1] = lo2;
                self.insns[jmp_pos + 2] = hi2;
            }
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                let is_method = matches!(
                    &call.callee,
                    Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
                );
                if is_method {
                    let (recv_expr, method_name): (&Expression<'_>, Option<String>) =
                        match &call.callee {
                            Expression::StaticMemberExpression(m) => {
                                (&m.object, Some(m.property.name.as_str().to_string()))
                            }
                            Expression::ComputedMemberExpression(m) => {
                                let n = if let Expression::StringLiteral(s) = &m.expression {
                                    Some(s.value.as_str().to_string())
                                } else {
                                    None
                                };
                                (&m.object, n)
                            }
                            other => {
                                self.emit_expr(other);
                                self.push(Insn::CallArg0(0));
                                return;
                            }
                        };
                    self.emit_expr(recv_expr);
                    let this_reg = self.alloc_reg();
                    self.push(Insn::Sta(this_reg));
                    let null_reg = self.alloc_reg();
                    self.push(Insn::LdNull);
                    self.push(Insn::Sta(null_reg));
                    self.push(Insn::Lda(this_reg));
                    self.push(Insn::Eq(0, null_reg));
                    let jnez_pos = self.insns.len();
                    self.push(Insn::Jnez(0));
                    if let Some(ref mname) = method_name {
                        let name_id = self.intern(mname);
                        self.push(Insn::Lda(this_reg));
                        self.push(Insn::LdObjByName(0, name_id));
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    let argc = call.arguments.len();
                    match argc {
                        0 => self.push(Insn::CallThis0(0, this_reg)),
                        1 => {
                            let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                            self.push(Insn::CallThis1(0, this_reg, v0));
                        }
                        2 => {
                            let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                            let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                            self.push(Insn::CallThis2(0, this_reg, v0, v1));
                        }
                        _ => {
                            let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                            self.push(Insn::CallThis1(0, this_reg, v0));
                        }
                    }
                    let jmp_pos = self.insns.len();
                    self.push(Insn::Jmp(0));
                    let null_start = self.insns.len();
                    let [lo, hi] = ((null_start as i32 - jnez_pos as i32) as i16).to_le_bytes();
                    self.insns[jnez_pos + 1] = lo;
                    self.insns[jnez_pos + 2] = hi;
                    self.push(Insn::LdUndefined);
                    let end = self.insns.len();
                    let [lo2, hi2] = ((end as i32 - jmp_pos as i32) as i16).to_le_bytes();
                    self.insns[jmp_pos + 1] = lo2;
                    self.insns[jmp_pos + 2] = hi2;
                } else {
                    self.emit_call(call);
                }
            }
            _ => {
                self.push(Insn::LdUndefined);
            }
        }
    }

    // ── Array literal: [a, ...b, c] ───────────────────────────────────────────

    fn emit_array_expr(&mut self, arr: &oxc_ast::ast::ArrayExpression<'_>) {
        self.push(Insn::CreateEmptyArray(0));
        if arr.elements.is_empty() {
            return;
        }
        let arr_reg = self.alloc_reg();
        self.push(Insn::Sta(arr_reg));
        let mut idx: u16 = 0;
        for elem in &arr.elements {
            match elem {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                    self.emit_expr(&spread.argument);
                    self.push(Insn::StOwnByIndex(0, arr_reg, idx));
                    idx += 1;
                }
                oxc_ast::ast::ArrayExpressionElement::Elision(_) => {
                    idx += 1;
                }
                _ => {
                    if let Some(expr) = elem.as_expression() {
                        self.emit_expr(expr);
                    } else {
                        self.push(Insn::LdUndefined);
                    }
                    self.push(Insn::StOwnByIndex(0, arr_reg, idx));
                    idx += 1;
                }
            }
        }
        self.push(Insn::Lda(arr_reg));
    }

    // ── Object literal: { key: value, … } ────────────────────────────────────

    /// Return `true` when every property of `obj` has a string-convertible key
    /// and a compile-time-constant value (currently: string literal only).
    ///
    /// Such "static" objects can be pre-built as a `LiteralArray` and loaded via
    /// `createobjectwithbuffer` (opcode `0x07`) instead of the slower
    /// `createemptyobject` + `stobjbyname` sequence.  This matches how hvigor /
    /// es2abc compiles constant object literals in stage-model code (e.g. the
    /// metadata argument of `registerNamedRoute`).
    fn is_static_object(obj: &oxc_ast::ast::ObjectExpression<'_>) -> bool {
        use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey};
        if obj.properties.is_empty() {
            // Empty object literals stay on the createemptyobject path.
            return false;
        }
        for prop in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(property) = prop else {
                return false;
            };
            // Key must be an identifier or a string literal.
            match &property.key {
                PropertyKey::StaticIdentifier(_) | PropertyKey::StringLiteral(_) => {}
                _ => return false,
            }
            // Value must be a string literal.  Numeric, boolean, and null
            // literals could also be encoded, but they require additional
            // LiteralEntry tag variants; fall through to the dynamic path for
            // those cases until they are implemented.
            match &property.value {
                Expression::StringLiteral(_) => {}
                _ => return false,
            }
        }
        true
    }

    fn emit_object_expr(&mut self, obj: &oxc_ast::ast::ObjectExpression<'_>) {
        use oxc_ast::ast::{Expression, ObjectProperty, ObjectPropertyKind, PropertyKey};

        // Fast path: all-string static object → createobjectwithbuffer.
        //
        // Build a LiteralArray with alternating TaggedString(key) /
        // TaggedString(value) entries and emit a single
        // createobjectwithbuffer instruction.  The literal-array ID is the
        // 0-based index of the newly appended array.
        if Self::is_static_object(obj) {
            let mut entries = Vec::with_capacity(obj.properties.len() * 2);
            for prop in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(property) = prop else {
                    continue;
                };
                let ObjectProperty { key, value, .. } = property.as_ref();
                let key_name = match key {
                    PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                    PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                    _ => continue,
                };
                let val_str = match value {
                    Expression::StringLiteral(s) => s.value.as_str().to_string(),
                    _ => continue,
                };
                entries.push(LiteralEntry::TaggedString(key_name));
                entries.push(LiteralEntry::TaggedString(val_str));
            }
            let lit_id = self.literal_arrays.len() as u16;
            self.literal_arrays.push(LiteralArray { entries });
            self.push(Insn::CreateObjectWithBuffer(0, lit_id));
            return;
        }

        // Slow path: dynamic properties — createemptyobject + stobjbyname.
        self.push(Insn::CreateEmptyObject);
        if obj.properties.is_empty() {
            return;
        }
        let obj_reg = self.alloc_reg();
        self.push(Insn::Sta(obj_reg));
        for prop in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(property) = prop else {
                continue;
            };
            let ObjectProperty { key, value, .. } = property.as_ref();
            // Get the string key name.
            let key_name = match key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                _ => continue,
            };
            // Evaluate the property value into the accumulator.
            self.emit_expr(value);
            // Store accumulator → obj[key].
            let name_id = self.intern(&key_name);
            self.push(Insn::StObjByName(0, name_id, obj_reg));
        }
        // Leave the object in the accumulator.
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
        self.push(Insn::CreateGeneratorObj(0));
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
    }

    fn emit_yield_expr(&mut self, yield_expr: &YieldExpression<'_>) {
        match &yield_expr.argument {
            Some(arg) => self.emit_expr(arg),
            None => self.push(Insn::LdUndefined),
        }
        if let Some(gen_reg) = self.gen_reg {
            self.push(Insn::SuspendGenerator(0, gen_reg));
            self.push(Insn::ResumeGenerator(gen_reg));
        }
    }

    // ── Private field access: obj.#field ─────────────────────────────────────

    /// Lower `obj.#field` to a `ldobjbyname` with a mangled name.
    ///
    /// Approach: mangle `#field` → `__pvt_field` and emit `ldobjbyname`.
    /// This is runtime-correct when private field names do not collide with
    /// public properties and no security boundary is required.  Full
    /// `ldprivateproperty` / `stprivateproperty` support requires a separate
    /// pass to collect all private field declarations and assign slot indices.
    fn emit_private_field(&mut self, priv_expr: &PrivateFieldExpression<'_>) {
        self.emit_expr(&priv_expr.object);
        let mangled = format!("__pvt_{}", priv_expr.field.name.as_str());
        let name_id = self.intern(&mangled);
        self.push(Insn::LdObjByName(0, name_id));
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
            0u8
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
        let lexical_slot = self.lowering.lexical_index(name);
        let export_slot = self.lowering.export_index(name);
        if lexical_slot.is_some() {
            self.next_reg = self.next_reg.max(5);
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
            0u8
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
            self.push(Insn::LdObjByName(0, name_id));
        }
        self.next_reg = saved_next_reg;
        // Non-string computed keys are a TODO.
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
        self.push(Insn::LdObjByName(0, name_id));
        self.next_reg = saved_next_reg;
    }

    // ── Assignment: lhs = rhs ─────────────────────────────────────────────────

    fn emit_assignment(&mut self, assign: &oxc_ast::ast::AssignmentExpression<'_>) {
        // AssignmentTarget inherits variants from SimpleAssignmentTarget via
        // @inherit, so we match the inherited variants directly.
        match &assign.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.emit_expr(&assign.right);
                let name = id.name.as_str();
                if let Some(&reg) = self.var_regs.get(name) {
                    self.push(Insn::Sta(reg));
                } else {
                    let name_id = self.intern(name);
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
            }
            AssignmentTarget::ComputedMemberExpression(mem) => {
                if is_global_this(&mem.object) {
                    if let Expression::StringLiteral(key) = &mem.expression {
                        self.emit_expr(&assign.right);
                        let name_id = self.intern(key.value.as_str());
                        self.push(Insn::TryStGlobalByName(0, name_id));
                        return;
                    }
                }
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.set_debug_pos(mem.span.start);
                self.push(Insn::Sta(obj_reg));
                if let Expression::StringLiteral(key) = &mem.expression {
                    self.emit_expr(&assign.right);
                    let name_id = self.intern(key.value.as_str());
                    self.push(Insn::StObjByName(0, name_id, obj_reg));
                }
            }
            AssignmentTarget::StaticMemberExpression(mem) => {
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.set_debug_pos(mem.span.start);
                self.push(Insn::Sta(obj_reg));
                if assign.operator != AssignmentOperator::Assign {
                    self.push(Insn::Lda(obj_reg));
                    let name_id = self.intern(mem.property.name.as_str());
                    self.push(Insn::LdObjByName(0, name_id));
                    let lhs_reg = self.alloc_reg();
                    self.push(Insn::Sta(lhs_reg));
                    self.emit_expr(&assign.right);
                    self.emit_assignment_binary(assign.operator, lhs_reg);
                } else {
                    self.emit_expr(&assign.right);
                }
                self.set_debug_pos(mem.span.start);
                let name_id = self.intern(mem.property.name.as_str());
                self.push(Insn::StObjByName(0, name_id, obj_reg));
            }
            _ => {
                // Destructuring assignments, TSAs, etc. — TODO.
                self.emit_expr(&assign.right);
            }
        }
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

        // When any argument is a spread element, force the range form so that
        // the spread array is forwarded as a regular positional argument
        // (`foo(...args)` → `callrange 1, args_reg`).
        if Self::args_have_spread(&call.arguments) {
            for arg in &call.arguments {
                self.emit_argument(arg);
                let reg = self.alloc_reg();
                self.set_debug_pos(argument_span_start(arg));
                self.push(Insn::Sta(reg));
            }
            self.current_line = call_fn_line;
            self.current_col = call_fn_col;
            self.push(Insn::Lda(callee_reg));
            self.push(Insn::CallRange(slot, argc as u8, callee_reg + 1));
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
                for arg in &call.arguments {
                    self.emit_argument(arg);
                    let reg = self.alloc_reg();
                    self.set_debug_pos(argument_span_start(arg));
                    self.push(Insn::Sta(reg));
                }
                self.current_line = call_fn_line;
                self.current_col = call_fn_col;
                self.push(Insn::Lda(callee_reg));
                self.push(Insn::CallRange(slot, n as u8, callee_reg + 1));
            }
        }
        self.next_reg = saved_next_reg;
    }

    fn emit_method_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>, slot: u8) {
        let saved_next_reg = self.next_reg;

        // Extract receiver and method name from the callee.
        let (recv_expr, method_name): (&Expression<'_>, Option<String>) = match &call.callee {
            Expression::StaticMemberExpression(mem) => {
                (&mem.object, Some(mem.property.name.as_str().to_string()))
            }
            Expression::ComputedMemberExpression(mem) => {
                let name = if let Expression::StringLiteral(s) = &mem.expression {
                    Some(s.value.as_str().to_string())
                } else {
                    None
                };
                (&mem.object, name)
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
        self.emit_expr(recv_expr);
        self.set_debug_pos(call.callee.span().start);
        self.push(Insn::Sta(this_reg));

        // Update col to the method name position for per-instruction tracking.
        if let Expression::StaticMemberExpression(mem) = &call.callee {
            self.set_debug_pos(mem.property.span.start);
        }

        // Load method function via ldobjbyname (or ldundefined as fallback).
        if let Some(ref mname) = method_name {
            let name_id = self.intern(mname);
            self.push(Insn::Lda(this_reg));
            self.push(Insn::LdObjByName(0, name_id));
            self.push(Insn::Sta(callee_reg));
        } else {
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(callee_reg));
        }

        let argc = call.arguments.len();

        // Save the method name column for the call instruction.
        // After compiling arguments, current_col is the last arg's col.
        // The call instruction itself should use the method name col,
        // matching es2panda's per-instruction column tracking.
        let call_line = self.current_line;
        let call_col = self.current_col;

        // When any argument is a spread element, force callthisrange so spread
        // arrays are forwarded as regular positional arguments.
        if Self::args_have_spread(&call.arguments) {
            for arg in &call.arguments {
                self.emit_argument(arg);
                let reg = self.alloc_reg();
                self.set_debug_pos(argument_span_start(arg));
                self.push(Insn::Sta(reg));
            }
            self.current_line = call_line;
            self.current_col = call_col; // reset to method col for call insn
            self.push(Insn::Lda(callee_reg));
            self.push(Insn::CallThisRange(slot, argc as u8, this_reg));
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
                for arg in &call.arguments {
                    self.emit_argument(arg);
                    let reg = self.alloc_reg();
                    self.set_debug_pos(argument_span_start(arg));
                    self.push(Insn::Sta(reg));
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
        let first_arg_reg = if argc == 0 {
            let reg = self.alloc_reg();
            self.push(Insn::LdUndefined);
            self.push(Insn::Sta(reg));
            reg
        } else {
            let reg = self.next_reg;
            for arg in &call.arguments {
                self.emit_argument(arg);
                let arg_reg = self.alloc_reg();
                self.push(Insn::Sta(arg_reg));
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
        // Allocate argument registers contiguous after ctor_reg and compile each arg.
        let mut arg_regs = Vec::with_capacity(argc);
        for _ in 0..argc {
            arg_regs.push(self.alloc_reg());
        }
        for (i, arg) in new_expr.arguments.iter().enumerate() {
            self.emit_argument(arg);
            self.set_debug_pos(new_expr.span.start);
            self.push(Insn::Sta(arg_regs[i]));
        }
        // newobjrange slot, (argc+1), ctor_reg — registers are [ctor, arg0, arg1, …].
        self.set_debug_pos(new_expr.span.start);
        self.push(Insn::NewObjRange(slot, argc as u8 + 1, ctor_reg));
    }

    // ── Argument helpers ──────────────────────────────────────────────────────

    /// Compile an argument expression into a fresh register; return the register.
    fn compile_arg_to_reg(&mut self, arg: &Argument<'_>) -> u8 {
        self.emit_argument(arg);
        let reg = self.alloc_reg();
        self.set_debug_pos(argument_span_start(arg));
        self.push(Insn::Sta(reg));
        if matches!(
            arg.as_expression(),
            Some(Expression::ObjectExpression(obj)) if Self::is_static_object(obj)
        ) {
            self.push(Insn::Lda(reg));
            self.push(Insn::Sta(reg));
        }
        reg
    }

    /// Compile an `Argument`.
    ///
    /// For plain expressions the result is left in the accumulator.
    /// For `...spread` elements the inner expression is compiled directly —
    /// i.e. `...args` emits `args` and the array is passed as a single value.
    /// This matches the simple approach for `foo(...args)` where the spread is
    /// the only argument: the array itself is forwarded via `CallRange`.
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
            // instanceof, ** etc. — emit acc for now.
            _ => {}
        }
    }

    fn emit_assignment_binary(&mut self, op: AssignmentOperator, left_reg: u8) {
        match op {
            AssignmentOperator::Addition => self.push(Insn::Add2(0, left_reg)),
            AssignmentOperator::Subtraction => self.push(Insn::Sub2(0, left_reg)),
            AssignmentOperator::Multiplication => self.push(Insn::Mul2(0, left_reg)),
            AssignmentOperator::Division => self.push(Insn::Div2(0, left_reg)),
            AssignmentOperator::Remainder => self.push(Insn::Mod2(0, left_reg)),
            AssignmentOperator::ShiftLeft => self.push(Insn::Shl2(0, left_reg)),
            AssignmentOperator::ShiftRight => self.push(Insn::Shr2(0, left_reg)),
            AssignmentOperator::ShiftRightZeroFill => self.push(Insn::Ashr2(0, left_reg)),
            AssignmentOperator::BitwiseAnd => self.push(Insn::And2(0, left_reg)),
            AssignmentOperator::BitwiseOR => self.push(Insn::Or2(0, left_reg)),
            AssignmentOperator::BitwiseXOR => self.push(Insn::Xor2(0, left_reg)),
            _ => {}
        }
    }

    fn emit_undefined_literal(&mut self) {
        self.push(Insn::LdUndefined);
    }

    // ── Unary expression: OP operand ──────────────────────────────────────────

    fn emit_unary_expr(&mut self, unary: &oxc_ast::ast::UnaryExpression<'_>) {
        match unary.operator {
            UnaryOperator::Void => {
                // `void expr` evaluates expr for side-effects, then yields undefined.
                if !matches!(&unary.argument, Expression::NumericLiteral(num) if num.value == 0.0) {
                    self.emit_expr(&unary.argument);
                }
                self.push(Insn::LdUndefined);
            }
            UnaryOperator::Typeof => {
                self.emit_expr(&unary.argument);
                self.set_debug_pos(unary.span.start);
                self.push(Insn::TypeOf(0));
            }
            UnaryOperator::UnaryNegation => {
                self.emit_expr(&unary.argument);
                self.push(Insn::Neg(0));
            }
            UnaryOperator::BitwiseNot => {
                self.emit_expr(&unary.argument);
                self.push(Insn::BitNot(0));
            }
            UnaryOperator::LogicalNot => {
                self.emit_expr(&unary.argument);
                self.set_debug_pos(unary.span.start);
                self.emit_negate_acc();
            }
            UnaryOperator::UnaryPlus => {
                // Unary `+x` — just evaluate (converts to number conceptually).
                self.emit_expr(&unary.argument);
            }
            _ => {
                self.emit_expr(&unary.argument);
            }
        }
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
    fn emit_logical_expr(&mut self, log: &oxc_ast::ast::LogicalExpression<'_>) {
        let saved_next_reg = self.next_reg;
        self.emit_expr(&log.left);
        let lhs_reg = self.alloc_reg();
        self.push(Insn::Sta(lhs_reg));

        let branch_pos = self.insns.len();
        match log.operator {
            LogicalOperator::And => {
                self.push(Insn::CallRuntimeIsFalse(0));
                self.push(Insn::Jnez(0));
            }
            LogicalOperator::Or => {
                self.push(Insn::CallRuntimeIsTrue(0));
                self.push(Insn::Jnez(0));
            }
            LogicalOperator::Coalesce => {
                // `a ?? b`: skip right only when left is not null/undefined.
                // Approximation: treat as `||` (does not handle 0/""/false correctly).
                self.push(Insn::CallRuntimeIsTrue(0));
                self.push(Insn::Jnez(0));
            }
        };
        let branch_jump = branch_pos + 3;

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
