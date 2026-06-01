use std::collections::HashSet;

use oxc_ast::ast::{BindingPattern, Expression, FunctionBody, Program, Statement};

pub(super) fn collect_module_lexical_bindings(
    program: &Program<'_>,
    _is_module: bool,
) -> Vec<String> {
    // Collect `let`/`const` variable declarators, `class` declarations, and
    // `var X = class { … }` assignments at module scope.
    //
    // All three create lexical bindings that es2abc tracks via
    // `NewLexEnvWithName` + `stlexvar` / `ldlexvar` opcodes.
    //
    // The rolldown bundler often rewrites `class Foo extends Base { … }`
    // (ClassDeclaration) into `var Foo = class extends Base { … }`
    // (VariableDeclaration with ClassExpression init).  For `var X = class`,
    // we only treat X as lexical when it is referenced inside a top-level
    // arrow function — the pattern that hvigor's es2abc produces for @Entry
    // components (the class constructor is captured by the `registerNamedRoute`
    // arrow).  Plain `export default class Foo {}` rewrites that are not
    // captured by any arrow do not need a lexical slot.
    let stmts = program.body.as_slice();

    // First pass: collect all names referenced inside top-level arrow functions.
    let arrow_referenced = collect_top_level_arrow_refs(stmts);

    let mut names = Vec::new();

    let push_unique = |names: &mut Vec<String>, name: &str| {
        if !names.iter().any(|k: &String| k == name) {
            names.push(name.to_string());
        }
    };

    for stmt in stmts {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                        continue;
                    };
                    if decl.kind.is_lexical() {
                        push_unique(&mut names, id.name.as_str());
                        continue;
                    }
                    if matches!(
                        declarator.init.as_ref(),
                        Some(Expression::ClassExpression(_))
                    ) && arrow_referenced.contains(id.name.as_str())
                    {
                        push_unique(&mut names, id.name.as_str());
                    }
                }
            }
            Statement::ClassDeclaration(cls) => {
                if let Some(id) = &cls.id {
                    push_unique(&mut names, id.name.as_str());
                }
            }
            _ => {}
        }
    }
    names
}

/// Collect all identifier names referenced inside top-level arrow function
/// expressions (i.e. arrow functions appearing as call-expression arguments
/// at module scope).  These are the variables that need lexical slots so
/// inner arrow bodies can access them via `ldlexvar`.
fn collect_top_level_arrow_refs(stmts: &[Statement<'_>]) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for stmt in stmts {
        if let Statement::ExpressionStatement(es) = stmt {
            collect_arrow_refs_from_expr(&es.expression, &mut referenced);
        }
    }
    referenced
}

fn collect_arrow_refs_from_expr(expr: &Expression<'_>, out: &mut HashSet<String>) {
    match expr {
        Expression::CallExpression(call) => {
            for arg in &call.arguments {
                if let Some(arg_expr) = arg.as_expression() {
                    if let Expression::ArrowFunctionExpression(arrow) = arg_expr {
                        collect_idents_in_arrow_body(arrow.body.as_ref(), out);
                    } else {
                        collect_arrow_refs_from_expr(arg_expr, out);
                    }
                }
            }
            collect_arrow_refs_from_expr(&call.callee, out);
        }
        Expression::StaticMemberExpression(mem) => {
            collect_arrow_refs_from_expr(&mem.object, out);
        }
        _ => {}
    }
}

fn collect_idents_in_arrow_body(body: &FunctionBody<'_>, out: &mut HashSet<String>) {
    for stmt in &body.statements {
        collect_idents_in_stmt(stmt, out);
    }
}

fn collect_idents_in_stmt(stmt: &Statement<'_>, out: &mut HashSet<String>) {
    match stmt {
        Statement::ExpressionStatement(es) => collect_idents_in_expr(&es.expression, out),
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                collect_idents_in_expr(arg, out);
            }
        }
        Statement::VariableDeclaration(decl) => {
            for d in &decl.declarations {
                if let Some(init) = &d.init {
                    collect_idents_in_expr(init, out);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                collect_idents_in_stmt(s, out);
            }
        }
        _ => {}
    }
}

fn collect_idents_in_expr(expr: &Expression<'_>, out: &mut HashSet<String>) {
    match expr {
        Expression::Identifier(id) => {
            out.insert(id.name.as_str().to_string());
        }
        Expression::NewExpression(new_expr) => {
            collect_idents_in_expr(&new_expr.callee, out);
            for arg in &new_expr.arguments {
                if let Some(e) = arg.as_expression() {
                    collect_idents_in_expr(e, out);
                }
            }
        }
        Expression::CallExpression(call) => {
            collect_idents_in_expr(&call.callee, out);
            for arg in &call.arguments {
                if let Some(e) = arg.as_expression() {
                    collect_idents_in_expr(e, out);
                }
            }
        }
        Expression::StaticMemberExpression(mem) => {
            collect_idents_in_expr(&mem.object, out);
        }
        _ => {}
    }
}
