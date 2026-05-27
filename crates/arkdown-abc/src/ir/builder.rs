use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, Expression, FunctionBody, Program, Statement,
};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

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
    let bytecode = compile_program_body(&program, strings);
    let num_vregs = estimate_vregs(&bytecode);
    methods.push(MethodRecord {
        name: "func_main_0".to_string(),
        bytecode,
        num_vregs,
        num_args: 3,
    });

    // Emit a method entry for each named top-level FunctionDeclaration.
    for stmt in &program.body {
        if let Statement::FunctionDeclaration(func) = stmt {
            if let Some(id) = &func.id {
                let func_name = id.name.as_str().to_string();
                let func_bytecode =
                    compile_function_body(func.body.as_deref(), strings);
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
    Ok(ClassRecord { name: class_name, methods })
}

// ── Byte-count heuristics ─────────────────────────────────────────────────────

/// Conservatively estimate the vreg count from bytecode length.
/// Real register allocation is a SPEC-009 follow-up.
fn estimate_vregs(bytecode: &[u8]) -> u16 {
    (bytecode.len() / 4).clamp(4, 64) as u16
}

// ── Program-level compilation ─────────────────────────────────────────────────

/// Compile the top-level program body into bytecode for the `main` method.
///
/// Detects the IIFE pattern `(function () { ... })()` and compiles the inner
/// body directly; the IIFE wrapper is transparent at the bytecode level because
/// ArkRuntime calls `main` directly.
fn compile_program_body(program: &Program<'_>, strings: &mut StringTable) -> Vec<u8> {
    // Detect a single top-level IIFE.
    if let [Statement::ExpressionStatement(es)] = program.body.as_slice() {
        if let Expression::CallExpression(call) = &es.expression {
            if call.arguments.is_empty() {
                let inner_stmts = match &call.callee {
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
                if let Some(stmts) = inner_stmts {
                    return compile_body_stmts(stmts, strings);
                }
            }
        }
    }

    compile_body_stmts(&program.body, strings)
}

/// Compile a function body's statements, or emit `returnundefined` if absent.
fn compile_function_body(body: Option<&FunctionBody<'_>>, strings: &mut StringTable) -> Vec<u8> {
    match body {
        Some(fb) => compile_body_stmts(&fb.statements, strings),
        None => {
            let mut st = StringTable::default();
            Insn::ReturnUndefined.encode(&mut st)
        }
    }
}

/// Compile a statement slice through `CodeGen` and return raw bytecode.
fn compile_body_stmts(stmts: &[Statement<'_>], strings: &mut StringTable) -> Vec<u8> {
    let mut cg = CodeGen::new(strings);
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
    fn new(strings: &'a mut StringTable) -> Self {
        CodeGen {
            strings,
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
                // TODO: emit defineclasswithbuffer once method-literal-array
                // generation is implemented (SPEC-009).
                // For now, store undefined as a placeholder so the global name
                // is at least registered.
                if let Some(id) = &cls.id {
                    let name_id = self.intern(id.name.as_str());
                    self.push(Insn::LdUndefined);
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
                self.emit_expr(&if_stmt.test);
                // Emit a placeholder jeqz offset; branch patching is TODO.
                self.push(Insn::Jeqz(0));
                self.emit_stmt(&if_stmt.consequent);
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
            _ => {
                self.push(Insn::LdUndefined);
            }
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
