use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, FunctionBody, ImportDeclarationSpecifier,
    ImportOrExportKind, MethodDefinition, MethodDefinitionKind, ModuleExportName, Program,
    PropertyKey, Statement,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::emit::record::{
    ClassRecord, DebugInfoRecord, FieldRecord, FieldValue, MethodRecord, FIELD_OWNER_CLASS_IDX,
};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::ir::opcodes::Insn;

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `source` and build a [`ClassRecord`] named `record_name`.
///
/// Steps:
/// 1. Parse as TSX (accepts TypeScript annotations and `.ets` syntax).
/// 2. Detect and unwrap the IIFE pattern `(function () { ... })()`, compiling
///    the inner body as the `main` method.
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
pub fn build_record(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Result<ClassRecord, AbcError> {
    build_record_with_debug(source, record_name, strings, literal_arrays, None)
}

pub fn build_record_with_debug(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    source_file: Option<&str>,
) -> Result<ClassRecord, AbcError> {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
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

    let mut methods: Vec<MethodRecord> = Vec::new();

    // Build the `func_main_0` method — the module execution entry point.
    //
    // ArkRuntime always invokes `func_main_0` when executing a module buffer.
    // It passes exactly 3 arguments (JSThread*, this, args[]), so `num_args`
    // must be 3 regardless of the actual JS parameter list.
    let bytecode = compile_program_body(&program, strings, literal_arrays);
    let num_vregs = estimate_vregs(&bytecode);
    methods.push(MethodRecord {
        name: "func_main_0".to_string(),
        bytecode,
        num_vregs,
        num_args: 3,
        debug_info: None,
    });

    // Determine which statement slice to scan for declarations.
    // If the program is a single IIFE, compile its inner body; otherwise scan
    // the top-level statements directly.
    let scan_stmts: &[Statement<'_>] =
        extract_iife_stmts(&program).unwrap_or(program.body.as_slice());

    // Emit a method entry for each named top-level FunctionDeclaration.
    for stmt in scan_stmts {
        if let Statement::FunctionDeclaration(func) = stmt {
            if let Some(id) = &func.id {
                let func_name = id.name.as_str().to_string();
                let func_bytecode =
                    compile_function_body(func.body.as_deref(), strings, literal_arrays);
                let num_vregs = estimate_vregs(&func_bytecode);
                methods.push(MethodRecord {
                    name: func_name,
                    bytecode: func_bytecode,
                    num_vregs,
                    num_args: func.params.items.len() as u16,
                    debug_info: None,
                });
            }
        }
        if let Statement::ExportNamedDeclaration(export) = stmt {
            if let Some(Declaration::FunctionDeclaration(func)) = &export.declaration {
                if let Some(id) = &func.id {
                    let func_name = id.name.as_str().to_string();
                    let func_bytecode =
                        compile_function_body(func.body.as_deref(), strings, literal_arrays);
                    let num_vregs = estimate_vregs(&func_bytecode);
                    methods.push(MethodRecord {
                        name: func_name,
                        bytecode: func_bytecode,
                        num_vregs,
                        num_args: func.params.items.len() as u16,
                        debug_info: None,
                    });
                }
            }
        }
    }

    // Compile class method bodies found in the scanned statements.
    // Class declarations inside the IIFE (the common OHOS bundle pattern)
    // produce real bytecode for each lifecycle/method body here.
    compile_class_method_bodies(scan_stmts, strings, literal_arrays, &mut methods);

    // ArkRuntime finds modules by their OHM URL class name format:
    //   "L{bundleName}/{moduleName}/{pathFromEts};"
    // e.g. "Lcom.example.myapp/entry/ets/entryability/EntryAbility;"
    //
    // This matches exactly what hvigorw and es2abc produce.
    // The record_name passed from the CLI is already:
    //   "{bundleName}/{module}/{path}"  (no src/main prefix)
    let class_name = if record_name.contains('/') {
        format!("L{};", record_name) // OHM URL format: no & wrappers
    } else {
        // Bare name (legacy / test path) — fall back to OHOS module class
        format!("L_{};", record_name)
    };

    // Collect module import/export records so that the ABC serializer can emit
    // the compact libpandafile module record buffer used by hvigor/es2abc.
    let module_record = collect_module_record(&program);
    let module_record_idx = literal_arrays.len();
    literal_arrays.push(module_record_literal_array(&module_record));
    let scope_names_idx = literal_arrays.len();
    literal_arrays.push(scope_names_literal_array(&methods));

    // The 6 module metadata fields that es2abc always writes into every user
    // class record.  ArkRuntime reads these to understand how to load the
    // module's dependencies.
    let module_name = module_name_from_record_name(record_name).unwrap_or("entry");
    let module_fields = vec![
        module_u8_field(format!("pkgName@{module_name}")),
        module_u8_field("isCommonjs"),
        module_u8_field("hasTopLevelAwait"),
        module_u8_field("isSharedModule"),
        FieldRecord {
            name: "scopeNames".to_string(),
            class_idx: FIELD_OWNER_CLASS_IDX,
            type_idx: 1,
            access_flags: 0,
            value: Some(FieldValue::LiteralArray(scope_names_idx)),
        },
        FieldRecord {
            name: "moduleRecordIdx".to_string(),
            class_idx: FIELD_OWNER_CLASS_IDX,
            type_idx: 1,
            access_flags: 0,
            value: Some(FieldValue::LiteralArray(module_record_idx)),
        },
    ];

    let source_file = source_file.map(str::to_string);
    if let Some(file) = &source_file {
        for method in &mut methods {
            method.debug_info = Some(DebugInfoRecord {
                line_start: 1,
                source_file: file.clone(),
            });
        }
    }

    Ok(ClassRecord {
        name: class_name,
        methods,
        access_flags: 1,
        source_lang: 0x04,
        source_file,
        module_requests: module_record.module_requests,
        fields: module_fields,
    })
}

fn module_name_from_record_name(record_name: &str) -> Option<&str> {
    record_name.split('/').nth(1).filter(|s| !s.is_empty())
}

fn module_u8_field(name: impl Into<String>) -> FieldRecord {
    FieldRecord {
        name: name.into(),
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: 0,
        access_flags: 0,
        value: Some(FieldValue::U8(0)),
    }
}

fn module_record_literal_array(module_record: &ModuleRecordInfo) -> LiteralArray {
    let mut entries = Vec::new();
    entries.push(LiteralEntry::RawU32(
        module_record.module_requests.len() as u32
    ));
    entries.extend(
        module_record
            .module_requests
            .iter()
            .cloned()
            .map(LiteralEntry::RawString),
    );

    entries.push(LiteralEntry::RawU32(
        module_record.regular_imports.len() as u32
    ));
    for import in &module_record.regular_imports {
        entries.push(LiteralEntry::RawString(import.local_name.clone()));
        entries.push(LiteralEntry::RawString(import.import_name.clone()));
        entries.push(LiteralEntry::RawU16(import.module_request_idx));
    }

    entries.push(LiteralEntry::RawU32(0)); // namespace imports
    entries.push(LiteralEntry::RawU32(
        module_record.local_exports.len() as u32
    ));
    for export in &module_record.local_exports {
        entries.push(LiteralEntry::RawString(export.local_name.clone()));
        entries.push(LiteralEntry::RawString(export.export_name.clone()));
    }
    entries.extend([
        LiteralEntry::RawU32(0), // indirect exports
        LiteralEntry::RawU32(0), // star exports
    ]);
    LiteralArray { entries }
}

fn scope_names_literal_array(methods: &[MethodRecord]) -> LiteralArray {
    let mut names: Vec<String> = methods.iter().map(|method| method.name.clone()).collect();
    names.sort();
    names.dedup();
    LiteralArray {
        entries: names.into_iter().map(LiteralEntry::TaggedString).collect(),
    }
}

// ── Byte-count heuristics ─────────────────────────────────────────────────────

/// Conservatively estimate the vreg count from bytecode length.
/// Real register allocation is a SPEC-009 follow-up.
fn estimate_vregs(bytecode: &[u8]) -> u16 {
    (bytecode.len() / 4).clamp(4, 64) as u16
}

// ── Program-level compilation ─────────────────────────────────────────────────

/// Return the inner statement slice if `program` consists of a single IIFE,
/// i.e. `(function () { … })()`.  Otherwise return `None`.
fn extract_iife_stmts<'a>(program: &'a Program<'_>) -> Option<&'a [Statement<'a>]> {
    if let [Statement::ExpressionStatement(es)] = program.body.as_slice() {
        if let Expression::CallExpression(call) = &es.expression {
            if call.arguments.is_empty() {
                return match &call.callee {
                    Expression::FunctionExpression(fe) => {
                        fe.body.as_deref().map(|b| b.statements.as_slice())
                    }
                    Expression::ParenthesizedExpression(p) => {
                        if let Expression::FunctionExpression(fe) = &p.expression {
                            fe.body.as_deref().map(|b| b.statements.as_slice())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
            }
        }
    }
    None
}

/// Walk `stmts` for `ClassDeclaration`s and compile each method body into a
/// separate [`MethodRecord`], appending them to `methods`.
///
/// This covers the common OHOS bundle pattern where EntryAbility and other
/// classes are declared inside the top-level IIFE: each lifecycle method
/// (`onCreate`, `onWindowStageCreate`, etc.) gets its own bytecode record so
/// ArkRuntime can dispatch to the real implementation.
fn compile_class_method_bodies(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    methods: &mut Vec<MethodRecord>,
) {
    for stmt in stmts {
        // Handle `class Foo { ... }` (ClassDeclaration)
        if let Statement::ClassDeclaration(cls) = stmt {
            let class_name = cls.id.as_ref().map(|id| id.name.as_str());
            compile_class_elements(class_name, &cls.body.body, strings, literal_arrays, methods);
            continue;
        }
        if let Statement::ExportDefaultDeclaration(export) = stmt {
            if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                compile_class_elements(
                    class_name,
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    methods,
                );
            }
            continue;
        }
        if let Statement::ExportNamedDeclaration(export) = stmt {
            if let Some(Declaration::ClassDeclaration(cls)) = &export.declaration {
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                compile_class_elements(
                    class_name,
                    &cls.body.body,
                    strings,
                    literal_arrays,
                    methods,
                );
            }
            continue;
        }
        // Handle `var Foo = class extends Bar { ... }` (VariableDeclaration with ClassExpression)
        // This is the pattern rolldown CJS output uses.
        // Also handles rolldown CJS interop helpers: `var __create = Object.create` etc.
        if let Statement::VariableDeclaration(var_decl) = stmt {
            for declarator in &var_decl.declarations {
                // Extract binding name (skip destructuring).
                let name = match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => id.name.as_str().to_string(),
                    _ => continue,
                };

                if let Some(init) = &declarator.init {
                    if let Expression::ClassExpression(cls) = init {
                        let class_name =
                            cls.id.as_ref().map_or(name.as_str(), |id| id.name.as_str());
                        compile_class_elements(
                            Some(class_name),
                            &cls.body.body,
                            strings,
                            literal_arrays,
                            methods,
                        );
                        continue;
                    }
                }

                // Rolldown CJS interop helpers: var __xxx = <arrow | fn | member>
                // Only capture names that start with double-underscore.
                if !name.starts_with("__") {
                    continue;
                }

                let bytecode = if let Some(init) = &declarator.init {
                    match init {
                        Expression::ArrowFunctionExpression(arrow) => {
                            // body is Box<FunctionBody> directly in the ohos-rs fork.
                            compile_function_body(
                                Some(arrow.body.as_ref()),
                                strings,
                                literal_arrays,
                            )
                        }
                        Expression::FunctionExpression(func) => {
                            compile_function_body(func.body.as_deref(), strings, literal_arrays)
                        }
                        _ => {
                            // Simple assignment (e.g. `var __create = Object.create`):
                            // emit a stub method that immediately returns undefined.
                            let mut st = StringTable::default();
                            Insn::ReturnUndefined.encode(&mut st)
                        }
                    }
                } else {
                    let mut st = StringTable::default();
                    Insn::ReturnUndefined.encode(&mut st)
                };

                let num_vregs = estimate_vregs(&bytecode);
                methods.push(MethodRecord {
                    name,
                    bytecode,
                    num_vregs,
                    num_args: 0,
                    debug_info: None,
                });
            }
        }
    }
}

/// Compile method bodies from a class element list and append MethodRecords.
fn compile_class_elements(
    class_name: Option<&str>,
    elements: &[ClassElement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
    methods: &mut Vec<MethodRecord>,
) {
    let class_name = class_name.unwrap_or("_");
    let has_constructor = elements.iter().any(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    });
    if !has_constructor {
        methods.push(MethodRecord {
            name: format!("#~@0=#{class_name}"),
            bytecode: Insn::ReturnUndefined.encode(&mut StringTable::default()),
            num_vregs: 0,
            num_args: 4,
            debug_info: None,
        });
    }

    let mut seen_names = HashMap::<String, u16>::new();
    for element in elements {
        if let ClassElement::MethodDefinition(method) = element {
            let Some(key_name) = class_method_record_name(method, class_name) else {
                continue;
            };
            let key_name = suffix_duplicate_method_name(key_name, &mut seen_names);
            let bytecode =
                compile_function_body(method.value.body.as_deref(), strings, literal_arrays);
            let num_vregs = estimate_vregs(&bytecode);
            let num_args = class_method_num_args(method);
            methods.push(MethodRecord {
                name: key_name,
                bytecode,
                num_vregs,
                num_args,
                debug_info: None,
            });
        }
    }
}

fn class_method_record_name(method: &MethodDefinition<'_>, class_name: &str) -> Option<String> {
    let key = method_key_name(&method.key)?;
    let name = match method.kind {
        MethodDefinitionKind::Constructor => format!("#~@0=#{class_name}"),
        _ if method.r#static => format!("#~@0<#{key}"),
        _ => format!("#~@0>#{key}"),
    };
    Some(name)
}

fn class_method_num_args(method: &MethodDefinition<'_>) -> u16 {
    method.value.params.items.len() as u16 + 3
}

fn method_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_string()),
        _ => None,
    }
}

fn suffix_duplicate_method_name(name: String, seen_names: &mut HashMap<String, u16>) -> String {
    let count = seen_names.entry(name.clone()).or_insert(0);
    let suffixed = if *count == 0 {
        name
    } else {
        format!("{name}^{count}")
    };
    *count += 1;
    suffixed
}

fn class_buffer_literal_array(class_name: &str, elements: &[ClassElement<'_>]) -> LiteralArray {
    let mut entries = Vec::new();
    let mut seen_names = HashMap::<String, u16>::new();
    let mut instance_method_count = 0u32;

    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        let Some(method_name) = class_method_record_name(method, class_name) else {
            continue;
        };
        let method_name = suffix_duplicate_method_name(method_name, &mut seen_names);

        if !is_class_buffer_method(method) {
            continue;
        }
        let Some(public_name) = method_key_name(&method.key) else {
            continue;
        };

        entries.push(LiteralEntry::TaggedString(public_name));
        entries.push(LiteralEntry::TaggedMethodName(method_name));
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

/// Compile the top-level program body into bytecode for the `main` method.
///
/// Detects the IIFE pattern `(function () { ... })()` and compiles the inner
/// body directly; the IIFE wrapper is transparent at the bytecode level because
/// ArkRuntime calls `main` directly.
fn compile_program_body(
    program: &Program<'_>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Vec<u8> {
    if let Some(stmts) = extract_iife_stmts(program) {
        return compile_body_stmts(stmts, strings, literal_arrays);
    }
    compile_body_stmts(&program.body, strings, literal_arrays)
}

/// Compile a function body's statements, or emit `returnundefined` if absent.
fn compile_function_body(
    body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Vec<u8> {
    match body {
        Some(fb) => compile_body_stmts(&fb.statements, strings, literal_arrays),
        None => {
            let mut st = StringTable::default();
            Insn::ReturnUndefined.encode(&mut st)
        }
    }
}

/// Compile a statement slice through `CodeGen` and return raw bytecode.
fn compile_body_stmts(
    stmts: &[Statement<'_>],
    strings: &mut StringTable,
    literal_arrays: &mut Vec<LiteralArray>,
) -> Vec<u8> {
    let mut cg = CodeGen::new(strings, literal_arrays);
    for stmt in stmts {
        cg.emit_stmt(stmt);
        if cg.returned {
            break;
        }
    }
    cg.finish()
}

// ── CodeGen ───────────────────────────────────────────────────────────────────

struct CodeGen<'a> {
    strings: &'a mut StringTable,
    /// Literal arrays accumulated by class definitions.
    literal_arrays: &'a mut Vec<LiteralArray>,
    /// Raw bytecode bytes accumulated so far.
    insns: Vec<u8>,
    /// Maps JS binding name → virtual register number.
    var_regs: HashMap<String, u8>,
    /// Next free register index.
    next_reg: u8,
    /// Set to `true` after a `return` or `returnundefined` is emitted so that
    /// unreachable code is not generated.
    returned: bool,
}

impl<'a> CodeGen<'a> {
    fn new(strings: &'a mut StringTable, literal_arrays: &'a mut Vec<LiteralArray>) -> Self {
        CodeGen {
            strings,
            literal_arrays,
            insns: Vec::new(),
            var_regs: HashMap::new(),
            next_reg: 0,
            returned: false,
        }
    }

    /// Append the bytes for `insn`.
    fn push(&mut self, insn: Insn) {
        let bytes = insn.encode(self.strings);
        self.insns.extend_from_slice(&bytes);
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

    /// Emit `returnundefined` if no explicit return was generated, then return
    /// the accumulated bytecode bytes.
    fn finish(mut self) -> Vec<u8> {
        if !self.returned {
            self.push(Insn::ReturnUndefined);
        }
        self.insns
    }

    // ── Statement dispatcher ──────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Statement<'_>) {
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
                self.emit_expr(&es.expression);
                // Discard accumulator result (no sta needed for side-effect-only
                // ExpressionStatements).
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.emit_expr(arg);
                    self.push(Insn::Return);
                } else {
                    self.push(Insn::ReturnUndefined);
                }
                self.returned = true;
            }
            Statement::ClassDeclaration(cls) => {
                // Emit defineclasswithbuffer; result → accumulator.
                let super_class = cls.super_class.as_ref();
                let class_name = cls.id.as_ref().map(|id| id.name.as_str());
                self.emit_class_inner(class_name, &cls.body.body, super_class);
                // Store the class constructor to a local register and,
                // if the class has a name, expose it as a global.
                if let Some(id) = &cls.id {
                    let reg = self.alloc_reg();
                    self.push(Insn::Sta(reg));
                    let name = id.name.as_str().to_string();
                    self.var_regs.insert(name.clone(), reg);
                    let name_id = self.intern(&name);
                    self.push(Insn::Lda(reg));
                    self.push(Insn::TryStGlobalByName(0, name_id));
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
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
            // Everything else: no bytecode emitted.
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
    /// `Jeqz` encodes a 16-bit signed offset measured from the **start** of the
    /// `jeqz` instruction itself.  `Jmp` likewise is measured from its own start.
    fn emit_if_stmt(&mut self, if_stmt: &oxc_ast::ast::IfStatement<'_>) {
        // 1. Emit the test expression; result in accumulator.
        self.emit_expr(&if_stmt.test);

        // 2. Emit `jeqz` with a placeholder offset; remember its byte position.
        let jeqz_pos = self.insns.len();
        self.push(Insn::Jeqz(0)); // 3 bytes: [0x50, lo, hi]

        // 3. Emit consequent; preserve outer `returned` flag across the branch.
        let outer_returned = self.returned;
        self.returned = false;
        self.emit_stmt(&if_stmt.consequent);
        let consequent_returned = self.returned;
        self.returned = outer_returned;

        if let Some(alt) = &if_stmt.alternate {
            // 4a. Emit `jmp` to skip the alternate; remember its position.
            let jmp_pos = self.insns.len();
            self.push(Insn::Jmp(0)); // 3 bytes: [0x4E, lo, hi]

            // Patch jeqz → start of alternate (= current byte position).
            let alt_start = self.insns.len();
            let jeqz_offset = (alt_start as i32 - jeqz_pos as i32) as i16;
            let [lo, hi] = jeqz_offset.to_le_bytes();
            self.insns[jeqz_pos + 1] = lo;
            self.insns[jeqz_pos + 2] = hi;

            // 5a. Emit alternate.
            let alt_outer_returned = self.returned;
            self.returned = false;
            self.emit_stmt(alt);
            let _alt_returned = self.returned;
            self.returned = alt_outer_returned;

            // Patch jmp → just past the alternate.
            let end_pos = self.insns.len();
            let jmp_offset = (end_pos as i32 - jmp_pos as i32) as i16;
            let [lo2, hi2] = jmp_offset.to_le_bytes();
            self.insns[jmp_pos + 1] = lo2;
            self.insns[jmp_pos + 2] = hi2;
        } else {
            // 4b. Patch jeqz → just past the consequent (no else branch).
            let _ = consequent_returned; // informational only for now
            let end_pos = self.insns.len();
            let jeqz_offset = (end_pos as i32 - jeqz_pos as i32) as i16;
            let [lo, hi] = jeqz_offset.to_le_bytes();
            self.insns[jeqz_pos + 1] = lo;
            self.insns[jeqz_pos + 2] = hi;
        }
    }

    fn emit_var_decl(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'_>) {
        for declarator in &decl.declarations {
            // BindingPattern is directly the enum (no .kind wrapper in this OXC fork).
            let name = match &declarator.id {
                BindingPattern::BindingIdentifier(id) => id.name.as_str().to_string(),
                _ => continue, // skip destructuring
            };

            if let Some(init) = &declarator.init {
                if let Expression::ClassExpression(cls) = init {
                    let class_name = cls.id.as_ref().map_or(name.as_str(), |id| id.name.as_str());
                    self.emit_class_inner(
                        Some(class_name),
                        &cls.body.body,
                        cls.super_class.as_ref(),
                    );
                } else {
                    self.emit_expr(init);
                }
            } else {
                self.push(Insn::LdUndefined);
            }

            let reg = self.alloc_reg();
            self.var_regs.insert(name, reg);
            self.push(Insn::Sta(reg));
        }
    }

    // ── Expression compiler — result always in accumulator ────────────────────

    fn emit_expr(&mut self, expr: &Expression<'_>) {
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
                        } else {
                            let name_id = self.intern(name);
                            self.push(Insn::TryLdGlobalByName(0, name_id));
                        }
                    }
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
            Expression::LogicalExpression(log) => {
                // Compile only the left side; proper short-circuit with branch
                // patching is a TODO.
                self.emit_expr(&log.left);
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
                // Arrow function literal: `(params) => { body }`.
                // Full nested-closure compilation is deferred to SPEC-009 v2.
                // Emit a `definefuncshort` stub so the accumulator holds a real
                // callable function object rather than `undefined`.
                let length = arrow.params.items.len() as u8;
                self.push(Insn::DefineFuncShort(0, 0, length));
            }
            Expression::FunctionExpression(func) => {
                // Named or anonymous function expression: `function(params) {}`.
                let length = func.params.items.len() as u8;
                self.push(Insn::DefineFuncShort(0, 0, length));
            }
            _ => {
                self.push(Insn::LdUndefined);
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
    fn emit_class_inner(
        &mut self,
        class_name: Option<&str>,
        elements: &[ClassElement<'_>],
        super_class: Option<&Expression<'_>>,
    ) {
        // Load the prototype (parent class) if present.
        let (proto_argc, proto_vreg) = if let Some(sc) = super_class {
            self.emit_expr(sc);
            let reg = self.alloc_reg();
            self.push(Insn::Sta(reg));
            (1u16, reg)
        } else {
            (0u16, 0u8)
        };

        // Register the class literal buffer and use its index as buf_id.
        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays.push(class_buffer_literal_array(
            class_name.unwrap_or("_"),
            elements,
        ));

        // Emit defineclasswithbuffer:
        //   slot=0, ctor_id=lit_arr_id (stub), buf_id=lit_arr_id,
        //   proto_argc, proto_vreg
        self.push(Insn::DefineClassWithBuffer(
            0, lit_arr_id, lit_arr_id, proto_argc, proto_vreg,
        ));
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
        self.emit_expr(&mem.object);
        if let Expression::StringLiteral(key) = &mem.expression {
            let name_id = self.intern(key.value.as_str());
            self.push(Insn::LdObjByName(0, name_id));
        }
        // Non-string computed keys are a TODO.
    }

    // ── Static member: obj.prop ───────────────────────────────────────────────

    fn emit_static_member(&mut self, mem: &oxc_ast::ast::StaticMemberExpression<'_>) {
        self.emit_expr(&mem.object);
        let name_id = self.intern(mem.property.name.as_str());
        self.push(Insn::LdObjByName(0, name_id));
    }

    // ── Assignment: lhs = rhs ─────────────────────────────────────────────────

    fn emit_assignment(&mut self, assign: &oxc_ast::ast::AssignmentExpression<'_>) {
        self.emit_expr(&assign.right);

        // AssignmentTarget inherits variants from SimpleAssignmentTarget via
        // @inherit, so we match the inherited variants directly.
        match &assign.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
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
                        let name_id = self.intern(key.value.as_str());
                        self.push(Insn::TryStGlobalByName(0, name_id));
                        return;
                    }
                }
                // General case: save rhs, load object, emit stobjbyname.
                let rhs_reg = self.alloc_reg();
                self.push(Insn::Sta(rhs_reg));
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(rhs_reg));
                if let Expression::StringLiteral(key) = &mem.expression {
                    let name_id = self.intern(key.value.as_str());
                    self.push(Insn::StObjByName(0, name_id, obj_reg));
                }
            }
            AssignmentTarget::StaticMemberExpression(mem) => {
                let rhs_reg = self.alloc_reg();
                self.push(Insn::Sta(rhs_reg));
                self.emit_expr(&mem.object);
                let obj_reg = self.alloc_reg();
                self.push(Insn::Sta(obj_reg));
                self.push(Insn::Lda(rhs_reg));
                let name_id = self.intern(mem.property.name.as_str());
                self.push(Insn::StObjByName(0, name_id, obj_reg));
            }
            _ => {
                // Destructuring assignments, TSAs, etc. — TODO.
            }
        }
    }

    // ── Call: callee(args) ────────────────────────────────────────────────────

    fn emit_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>) {
        let slot = 0u8;

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

        // Plain function call.
        self.emit_expr(&call.callee);
        let argc = call.arguments.len();

        match argc {
            0 => self.push(Insn::CallArg0(slot)),
            1 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                self.push(Insn::CallArg1(slot, v0));
            }
            2 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                self.push(Insn::CallArgs2(slot, v0, v1));
            }
            3 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                self.push(Insn::CallArgs3(slot, v0, v1, v2));
            }
            n => {
                // Allocate contiguous registers for all arguments.
                let v_start = self.next_reg;
                let mut regs = Vec::with_capacity(n);
                for _ in 0..n {
                    regs.push(self.alloc_reg());
                }
                for (i, arg) in call.arguments.iter().enumerate() {
                    self.emit_argument(arg);
                    self.push(Insn::Sta(regs[i]));
                }
                self.push(Insn::CallRange(slot, n as u8, v_start));
            }
        }
    }

    fn emit_method_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>, slot: u8) {
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
                return;
            }
        };

        // Load receiver → this_reg.
        self.emit_expr(recv_expr);
        let this_reg = self.alloc_reg();
        self.push(Insn::Sta(this_reg));

        // Load method function via ldobjbyname (or ldundefined as fallback).
        if let Some(ref mname) = method_name {
            let name_id = self.intern(mname);
            self.push(Insn::Lda(this_reg));
            self.push(Insn::LdObjByName(0, name_id));
        } else {
            self.push(Insn::LdUndefined);
        }

        let argc = call.arguments.len();
        match argc {
            0 => self.push(Insn::CallThis0(slot, this_reg)),
            1 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                self.push(Insn::CallThis1(slot, this_reg, v0));
            }
            2 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                self.push(Insn::CallThis2(slot, this_reg, v0, v1));
            }
            3 => {
                let v0 = self.compile_arg_to_reg(&call.arguments[0]);
                let v1 = self.compile_arg_to_reg(&call.arguments[1]);
                let v2 = self.compile_arg_to_reg(&call.arguments[2]);
                self.push(Insn::CallThis3(slot, this_reg, v0, v1, v2));
            }
            n => {
                // callthisrange: registers are [this, arg0, arg1, ...] contiguous.
                // this_reg is already allocated; allocate arg registers after it.
                let mut arg_regs = Vec::with_capacity(n);
                for _ in 0..n {
                    arg_regs.push(self.alloc_reg());
                }
                for (i, arg) in call.arguments.iter().enumerate() {
                    self.emit_argument(arg);
                    self.push(Insn::Sta(arg_regs[i]));
                }
                self.push(Insn::CallThisRange(slot, n as u8, this_reg));
            }
        }
    }

    // ── New: new Ctor(args) ───────────────────────────────────────────────────

    fn emit_new(&mut self, new_expr: &oxc_ast::ast::NewExpression<'_>) {
        let slot = 0u8;
        self.emit_expr(&new_expr.callee);
        let ctor_reg = self.alloc_reg();
        self.push(Insn::Sta(ctor_reg));
        let argc = new_expr.arguments.len() as u8;
        // newobjrange: slot, argc+1 (ctor counts as arg), v_start=ctor_reg
        self.push(Insn::NewObjRange(slot, argc + 1, ctor_reg));
    }

    // ── Argument helpers ──────────────────────────────────────────────────────

    /// Compile an argument expression into a fresh register; return the register.
    fn compile_arg_to_reg(&mut self, arg: &Argument<'_>) -> u8 {
        let reg = self.alloc_reg();
        self.emit_argument(arg);
        self.push(Insn::Sta(reg));
        reg
    }

    /// Compile an `Argument` (spread args emit `ldundefined` as a placeholder).
    ///
    /// `Argument` in this OXC fork uses `@inherit Expression`, so all
    /// Expression variants are available on `Argument`. The `as_expression()`
    /// method returns `Some(&Expression)` for non-spread arguments.
    fn emit_argument(&mut self, arg: &Argument<'_>) {
        if let Some(expr) = arg.as_expression() {
            self.emit_expr(expr);
        } else {
            // SpreadElement — not yet supported; emit undefined as placeholder.
            self.push(Insn::LdUndefined);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return `true` if `expr` is the `globalThis` identifier.
fn is_global_this(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Identifier(id) if id.name.as_str() == "globalThis")
}

// ── Module-record collection ─────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
struct ModuleRecordInfo {
    module_requests: Vec<String>,
    regular_imports: Vec<RegularImportEntry>,
    local_exports: Vec<LocalExportEntry>,
}

#[derive(Debug, Clone)]
struct RegularImportEntry {
    local_name: String,
    import_name: String,
    module_request_idx: u16,
}

#[derive(Debug, Clone)]
struct LocalExportEntry {
    local_name: String,
    export_name: String,
}

impl ModuleRecordInfo {
    fn request_index(&mut self, specifier: &str) -> u16 {
        if let Some(index) = self
            .module_requests
            .iter()
            .position(|request| request == specifier)
        {
            return index as u16;
        }
        let index = self.module_requests.len() as u16;
        self.module_requests.push(specifier.to_string());
        index
    }

    fn add_regular_import(&mut self, local_name: String, specifier: &str) {
        self.add_named_import(local_name, "default".to_string(), specifier);
    }

    fn add_named_import(&mut self, local_name: String, import_name: String, specifier: &str) {
        let module_request_idx = self.request_index(specifier);
        if self.regular_imports.iter().any(|import| {
            import.local_name == local_name && import.module_request_idx == module_request_idx
        }) {
            return;
        }
        self.regular_imports.push(RegularImportEntry {
            local_name,
            import_name,
            module_request_idx,
        });
    }

    fn add_local_export(&mut self, local_name: String, export_name: &str) {
        if self
            .local_exports
            .iter()
            .any(|export| export.local_name == local_name && export.export_name == export_name)
        {
            return;
        }
        self.local_exports.push(LocalExportEntry {
            local_name,
            export_name: export_name.to_string(),
        });
    }
}

/// Collect libpandafile module-record data from `program`.
///
/// The function unwraps a top-level IIFE wrapper before scanning. It recognises
/// the CommonJS patterns produced by the current bundler:
/// `var import_X = require("@ohos:...")` and `module.exports = Local`.
fn collect_module_record(program: &Program<'_>) -> ModuleRecordInfo {
    let stmts = extract_iife_stmts(program).unwrap_or(program.body.as_slice());
    let mut info = ModuleRecordInfo::default();
    for stmt in stmts {
        scan_stmt_for_module_record(stmt, &mut info);
    }
    info
}

/// Scan a statement for module import/export patterns.
fn scan_stmt_for_module_record(stmt: &Statement<'_>, info: &mut ModuleRecordInfo) {
    match stmt {
        Statement::ExpressionStatement(es) => {
            scan_expr_for_module_record(&es.expression, info);
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                let local_name = match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
                    _ => None,
                };
                if let Some(init) = &declarator.init {
                    if let (Some(local_name), Some(specifier)) =
                        (local_name, require_specifier(init))
                    {
                        let local_name = normalize_import_local_name(local_name, specifier);
                        info.add_regular_import(local_name, specifier);
                    } else {
                        scan_expr_for_module_record(init, info);
                    }
                }
            }
        }
        Statement::ImportDeclaration(import) => {
            if import.import_kind == ImportOrExportKind::Type {
                return;
            }
            let specifier = import.source.value.as_str();
            if !specifier.starts_with("@ohos:") {
                return;
            }
            if let Some(specifiers) = &import.specifiers {
                for spec in specifiers {
                    match spec {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                            info.add_regular_import(
                                default.local.name.as_str().to_string(),
                                specifier,
                            );
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(named) => {
                            if named.import_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            info.add_named_import(
                                named.local.name.as_str().to_string(),
                                module_export_name(&named.imported),
                                specifier,
                            );
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(ns) => {
                            info.add_regular_import(ns.local.name.as_str().to_string(), specifier);
                        }
                    }
                }
            } else {
                info.request_index(specifier);
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                scan_stmt_for_module_record(s, info);
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                scan_expr_for_module_record(arg, info);
            }
        }
        Statement::ExportDefaultDeclaration(_) => {
            info.add_local_export("default".to_string(), "default");
        }
        Statement::ExportNamedDeclaration(export) => {
            if export.export_kind == ImportOrExportKind::Type {
                return;
            }
            if export.source.is_none() {
                for spec in &export.specifiers {
                    if spec.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    info.add_local_export(
                        module_export_name(&spec.local),
                        &module_export_name(&spec.exported),
                    );
                }
                if let Some(declaration) = &export.declaration {
                    match declaration {
                        Declaration::ClassDeclaration(cls) => {
                            if let Some(id) = &cls.id {
                                info.add_local_export(
                                    id.name.as_str().to_string(),
                                    id.name.as_str(),
                                );
                            }
                        }
                        Declaration::FunctionDeclaration(func) => {
                            if let Some(id) = &func.id {
                                info.add_local_export(
                                    id.name.as_str().to_string(),
                                    id.name.as_str(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}

fn scan_expr_for_module_record(expr: &Expression<'_>, info: &mut ModuleRecordInfo) {
    if let Some(specifier) = require_specifier(expr) {
        info.request_index(specifier);
        return;
    }

    match expr {
        Expression::AssignmentExpression(assign) => {
            if is_module_exports_target(&assign.left) {
                if let Expression::Identifier(id) = &assign.right {
                    info.add_local_export(id.name.as_str().to_string(), "default");
                }
            }
            scan_expr_for_module_record(&assign.right, info);
        }
        Expression::CallExpression(call) => {
            for arg in &call.arguments {
                if let Some(expr) = arg.as_expression() {
                    scan_expr_for_module_record(expr, info);
                }
            }
        }
        Expression::ParenthesizedExpression(expr) => {
            scan_expr_for_module_record(&expr.expression, info);
        }
        Expression::SequenceExpression(seq) => {
            for expr in &seq.expressions {
                scan_expr_for_module_record(expr, info);
            }
        }
        Expression::LogicalExpression(log) => {
            scan_expr_for_module_record(&log.left, info);
            scan_expr_for_module_record(&log.right, info);
        }
        _ => {}
    }
}

fn require_specifier<'a>(expr: &'a Expression<'_>) -> Option<&'a str> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let Expression::Identifier(id) = &call.callee else {
        return None;
    };
    if id.name.as_str() != "require" || call.arguments.len() != 1 {
        return None;
    }
    let Some(Expression::StringLiteral(s)) = call.arguments[0].as_expression() else {
        return None;
    };
    let specifier = s.value.as_str();
    specifier.starts_with("@ohos:").then_some(specifier)
}

fn is_module_exports_target(target: &AssignmentTarget<'_>) -> bool {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return false;
    };
    if member.property.name.as_str() != "exports" {
        return false;
    }
    matches!(&member.object, Expression::Identifier(id) if id.name.as_str() == "module")
}

fn normalize_import_local_name(binding: &str, specifier: &str) -> String {
    if binding.starts_with("_ohos_") {
        return default_import_name(specifier);
    }
    let stripped = binding
        .strip_prefix("import_")
        .or_else(|| binding.strip_prefix("__import_"))
        .unwrap_or(binding);
    if stripped == "default" || stripped.is_empty() {
        return default_import_name(specifier);
    }
    stripped.to_string()
}

fn default_import_name(specifier: &str) -> String {
    specifier
        .rsplit(['.', ':'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(specifier)
        .to_string()
}

fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_str().to_string(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str().to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str().to_string(),
    }
}
