use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, ClassElement, Expression, FunctionBody, Program,
    PropertyKey, Statement,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::emit::file::LiteralArray;
use crate::emit::record::{ClassRecord, MethodRecord};
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
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let ParserReturn { program, panicked, errors, .. } =
        Parser::new(&allocator, source, source_type).parse();

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
                });
            }
        }
    }

    // Compile class method bodies found in the scanned statements.
    // Class declarations inside the IIFE (the common OHOS bundle pattern)
    // produce real bytecode for each lifecycle/method body here.
    compile_class_method_bodies(scan_stmts, strings, literal_arrays, &mut methods);

    // ArkRuntime finds modules by their OHOS class name format: "L&{module_path}&;"
    // where module_path is e.g. "entry/src/main/ets/entryability/EntryAbility".
    // This must match the record name that hvigorw uses in its merged ABC output.
    // If record_name is a bare stem (e.g. "EntryAbility"), fall back to L_GLOBAL;
    // for compatibility with single-file es2abc output.
    let class_name = if record_name.contains('/') {
        format!("L&{}&;", record_name)
    } else {
        // Bare name (legacy / test path) — use OHOS global module class name
        format!("L_{};", record_name)
    };

    // Collect module specifiers from require("@ohos:xxx") calls so that the
    // ABC serializer can emit them as standalone StringItems.  This is the first
    // step toward a proper ModuleRecord annotation; ArkRuntime can then locate
    // each import specifier by its file offset.
    let module_requests = collect_module_requests(&program);

    Ok(ClassRecord { name: class_name, methods, access_flags: 1, module_requests })
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
        if let Statement::ClassDeclaration(cls) = stmt {
            for element in &cls.body.body {
                if let ClassElement::MethodDefinition(method) = element {
                    let key_name = match &method.key {
                        PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                        PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                        _ => continue,
                    };
                    let bytecode = compile_function_body(
                        method.value.body.as_deref(),
                        strings,
                        literal_arrays,
                    );
                    let num_vregs = estimate_vregs(&bytecode);
                    let num_args = method.value.params.items.len() as u16;
                    methods.push(MethodRecord {
                        name: key_name,
                        bytecode,
                        num_vregs,
                        num_args,
                    });
                }
            }
        }
    }
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
                self.emit_class_inner(super_class);
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
                self.emit_expr(init);
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
                self.emit_class_inner(super_class);
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
    /// A stub [`LiteralArray`] with 0 entries is registered in
    /// `self.literal_arrays`; its index becomes the `buf_id` operand.  Method
    /// bodies are already compiled as separate `MethodRecord`s at the
    /// class-record level — the runtime resolves them there.
    ///
    /// After this call the class constructor is in the accumulator.
    fn emit_class_inner(&mut self, super_class: Option<&Expression<'_>>) {
        // Load the prototype (parent class) if present.
        let (proto_argc, proto_vreg) = if let Some(sc) = super_class {
            self.emit_expr(sc);
            let reg = self.alloc_reg();
            self.push(Insn::Sta(reg));
            (1u16, reg)
        } else {
            (0u16, 0u8)
        };

        // Register a stub literal array and use its index as buf_id.
        let lit_arr_id = self.literal_arrays.len() as u16;
        self.literal_arrays.push(LiteralArray { entries: Vec::new() });

        // Emit defineclasswithbuffer:
        //   slot=0, ctor_id=lit_arr_id (stub), buf_id=lit_arr_id,
        //   proto_argc, proto_vreg
        self.push(Insn::DefineClassWithBuffer(0, lit_arr_id, lit_arr_id, proto_argc, proto_vreg));
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
        let (recv_expr, method_name): (&Expression<'_>, Option<String>) =
            match &call.callee {
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

// ── Module-request collection ─────────────────────────────────────────────────

/// Collect all `require("@ohos:xxx")` specifiers from `program`.
///
/// The function unwraps a top-level IIFE wrapper (the same pattern handled by
/// `compile_program_body`) before scanning, so that require() calls inside the
/// IIFE body are found regardless of whether the source is bare or wrapped.
///
/// Returned specifiers are deduplicated and sorted for deterministic output.
fn collect_module_requests(program: &Program<'_>) -> Vec<String> {
    let stmts = extract_iife_stmts(program).unwrap_or(program.body.as_slice());
    let mut requests: Vec<String> = Vec::new();
    for stmt in stmts {
        scan_stmt_for_requires(stmt, &mut requests);
    }
    requests.sort();
    requests.dedup();
    requests
}

/// Scan a statement for `require("@ohos:xxx")` call expressions.
fn scan_stmt_for_requires(stmt: &Statement<'_>, out: &mut Vec<String>) {
    match stmt {
        Statement::ExpressionStatement(es) => {
            scan_expr_for_requires(&es.expression, out);
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                if let Some(init) = &declarator.init {
                    scan_expr_for_requires(init, out);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                scan_stmt_for_requires(s, out);
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                scan_expr_for_requires(arg, out);
            }
        }
        _ => {}
    }
}

/// Scan an expression for `require("@ohos:xxx")` patterns.
///
/// Only the direct `require("specifier")` call form is recognised; nested
/// expressions (member chains, logical operators, etc.) are not traversed
/// because the bundler always emits `require` at the top level of variable
/// initialisers.
fn scan_expr_for_requires(expr: &Expression<'_>, out: &mut Vec<String>) {
    if let Expression::CallExpression(call) = expr {
        if let Expression::Identifier(id) = &call.callee {
            if id.name.as_str() == "require" && call.arguments.len() == 1 {
                if let Some(Expression::StringLiteral(s)) = call.arguments[0].as_expression() {
                    let spec = s.value.as_str();
                    if spec.starts_with("@ohos:") && !out.iter().any(|r| r == spec) {
                        out.push(spec.to_string());
                    }
                }
            }
        }
    }
}
