use oxc_ast::ast::{
    Argument, ClassElement, Expression, FunctionBody, MethodDefinition, MethodDefinitionKind,
    Statement, UnaryOperator,
};

pub(super) fn method_body_contains_arrow(method: &MethodDefinition<'_>) -> bool {
    method
        .value
        .body
        .as_deref()
        .is_some_and(|body| statements_contain_arrow(&body.statements))
}

pub(super) fn statements_contain_arrow(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(statement_contains_arrow)
}

fn statement_contains_arrow(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::BlockStatement(block) => statements_contain_arrow(&block.body),
        Statement::ExpressionStatement(stmt) => expression_contains_arrow(&stmt.expression),
        Statement::VariableDeclaration(decl) => decl.declarations.iter().any(|declarator| {
            declarator
                .init
                .as_ref()
                .is_some_and(expression_contains_arrow)
        }),
        Statement::IfStatement(stmt) => {
            expression_contains_arrow(&stmt.test)
                || statement_contains_arrow(&stmt.consequent)
                || stmt
                    .alternate
                    .as_ref()
                    .is_some_and(statement_contains_arrow)
        }
        Statement::ReturnStatement(stmt) => stmt
            .argument
            .as_ref()
            .is_some_and(expression_contains_arrow),
        Statement::ThrowStatement(stmt) => expression_contains_arrow(&stmt.argument),
        Statement::WhileStatement(stmt) => {
            expression_contains_arrow(&stmt.test) || statement_contains_arrow(&stmt.body)
        }
        Statement::DoWhileStatement(stmt) => {
            statement_contains_arrow(&stmt.body) || expression_contains_arrow(&stmt.test)
        }
        Statement::ForStatement(stmt) => {
            stmt.init.as_ref().is_some_and(|init| match init {
                oxc_ast::ast::ForStatementInit::VariableDeclaration(decl) => {
                    decl.declarations.iter().any(|declarator| {
                        declarator
                            .init
                            .as_ref()
                            .is_some_and(expression_contains_arrow)
                    })
                }
                oxc_ast::ast::ForStatementInit::AssignmentExpression(assign) => {
                    expression_contains_arrow(&assign.right)
                }
                oxc_ast::ast::ForStatementInit::SequenceExpression(seq) => {
                    seq.expressions.iter().any(expression_contains_arrow)
                }
                expr => expression_contains_arrow(expr.to_expression()),
            }) || stmt.test.as_ref().is_some_and(expression_contains_arrow)
                || stmt.update.as_ref().is_some_and(expression_contains_arrow)
                || statement_contains_arrow(&stmt.body)
        }
        Statement::ForInStatement(stmt) => {
            expression_contains_arrow(&stmt.right) || statement_contains_arrow(&stmt.body)
        }
        Statement::ForOfStatement(stmt) => {
            expression_contains_arrow(&stmt.right) || statement_contains_arrow(&stmt.body)
        }
        Statement::TryStatement(stmt) => {
            statements_contain_arrow(&stmt.block.body)
                || stmt
                    .handler
                    .as_ref()
                    .is_some_and(|handler| statements_contain_arrow(&handler.body.body))
                || stmt
                    .finalizer
                    .as_ref()
                    .is_some_and(|finalizer| statements_contain_arrow(&finalizer.body))
        }
        Statement::SwitchStatement(stmt) => {
            expression_contains_arrow(&stmt.discriminant)
                || stmt.cases.iter().any(|case| {
                    case.test.as_ref().is_some_and(expression_contains_arrow)
                        || statements_contain_arrow(&case.consequent)
                })
        }
        _ => false,
    }
}

fn expression_contains_arrow(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ArrowFunctionExpression(_) => true,
        Expression::FunctionExpression(function) => function
            .body
            .as_ref()
            .is_some_and(|body| statements_contain_arrow(&body.statements)),
        Expression::AssignmentExpression(assign) => expression_contains_arrow(&assign.right),
        Expression::CallExpression(call) => {
            expression_contains_arrow(&call.callee)
                || call.arguments.iter().any(argument_contains_arrow)
        }
        Expression::NewExpression(new_expr) => {
            expression_contains_arrow(&new_expr.callee)
                || new_expr.arguments.iter().any(argument_contains_arrow)
        }
        Expression::ObjectExpression(object) => object.properties.iter().any(|property| {
            let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            expression_contains_arrow(&property.value)
        }),
        Expression::ArrayExpression(array) => array.elements.iter().any(|element| {
            element
                .as_expression()
                .is_some_and(expression_contains_arrow)
        }),
        Expression::SequenceExpression(seq) => {
            seq.expressions.iter().any(expression_contains_arrow)
        }
        Expression::ConditionalExpression(cond) => {
            expression_contains_arrow(&cond.test)
                || expression_contains_arrow(&cond.consequent)
                || expression_contains_arrow(&cond.alternate)
        }
        Expression::BinaryExpression(expr) => {
            expression_contains_arrow(&expr.left) || expression_contains_arrow(&expr.right)
        }
        Expression::LogicalExpression(expr) => {
            expression_contains_arrow(&expr.left) || expression_contains_arrow(&expr.right)
        }
        Expression::UnaryExpression(expr) => expression_contains_arrow(&expr.argument),
        Expression::ParenthesizedExpression(expr) => expression_contains_arrow(&expr.expression),
        Expression::ClassExpression(class) => class_body_contains_arrow(&class.body.body),
        Expression::TSAsExpression(expr) => expression_contains_arrow(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => expression_contains_arrow(&expr.expression),
        Expression::TSNonNullExpression(expr) => expression_contains_arrow(&expr.expression),
        Expression::TSTypeAssertion(expr) => expression_contains_arrow(&expr.expression),
        _ => false,
    }
}

fn class_body_contains_arrow(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| {
        let ClassElement::MethodDefinition(method) = element else {
            return false;
        };
        method.kind != MethodDefinitionKind::Constructor && method_body_contains_arrow(method)
    })
}

fn argument_contains_arrow(argument: &Argument<'_>) -> bool {
    argument
        .as_expression()
        .is_some_and(expression_contains_arrow)
}

pub(super) fn arrow_captures_from_body(body: &FunctionBody<'_>) -> ArrowCaptures {
    let mut uses_this = false;
    let mut uses_new_target = false;
    for stmt in &body.statements {
        collect_capture_from_stmt(stmt, &mut uses_this, &mut uses_new_target);
    }
    let this_slot = uses_this.then_some(0);
    let new_target_slot = uses_new_target.then_some(if uses_this { 1 } else { 0 });
    ArrowCaptures {
        this_slot,
        new_target_slot,
    }
}

fn collect_capture_from_stmt(
    stmt: &Statement<'_>,
    uses_this: &mut bool,
    uses_new_target: &mut bool,
) {
    match stmt {
        Statement::ExpressionStatement(expr) => {
            collect_capture_from_expr(&expr.expression, uses_this, uses_new_target);
        }
        Statement::ReturnStatement(ret) => {
            if let Some(expr) = &ret.argument {
                collect_capture_from_expr(expr, uses_this, uses_new_target);
            }
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                if let Some(init) = &declarator.init {
                    collect_capture_from_expr(init, uses_this, uses_new_target);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for stmt in &block.body {
                collect_capture_from_stmt(stmt, uses_this, uses_new_target);
            }
        }
        Statement::ForStatement(for_stmt) => {
            if let Some(test) = &for_stmt.test {
                collect_capture_from_expr(test, uses_this, uses_new_target);
            }
            if let Some(update) = &for_stmt.update {
                collect_capture_from_expr(update, uses_this, uses_new_target);
            }
            collect_capture_from_stmt(&for_stmt.body, uses_this, uses_new_target);
        }
        _ => {}
    }
}

fn collect_capture_from_expr(
    expr: &Expression<'_>,
    uses_this: &mut bool,
    uses_new_target: &mut bool,
) {
    match expr {
        Expression::ThisExpression(_) => *uses_this = true,
        Expression::MetaProperty(meta) if is_new_target_meta(meta) => *uses_new_target = true,
        Expression::ParenthesizedExpression(expr) => {
            collect_capture_from_expr(&expr.expression, uses_this, uses_new_target);
        }
        Expression::StaticMemberExpression(mem) => {
            collect_capture_from_expr(&mem.object, uses_this, uses_new_target);
        }
        Expression::ComputedMemberExpression(mem) => {
            collect_capture_from_expr(&mem.object, uses_this, uses_new_target);
            collect_capture_from_expr(&mem.expression, uses_this, uses_new_target);
        }
        Expression::CallExpression(call) => {
            collect_capture_from_expr(&call.callee, uses_this, uses_new_target);
            for arg in &call.arguments {
                if let Some(expr) = arg.as_expression() {
                    collect_capture_from_expr(expr, uses_this, uses_new_target);
                }
            }
        }
        Expression::AssignmentExpression(assign) => {
            collect_capture_from_expr(&assign.right, uses_this, uses_new_target);
        }
        Expression::BinaryExpression(binary) => {
            collect_capture_from_expr(&binary.left, uses_this, uses_new_target);
            collect_capture_from_expr(&binary.right, uses_this, uses_new_target);
        }
        Expression::LogicalExpression(log) => {
            collect_capture_from_expr(&log.left, uses_this, uses_new_target);
            collect_capture_from_expr(&log.right, uses_this, uses_new_target);
        }
        Expression::ConditionalExpression(cond) => {
            collect_capture_from_expr(&cond.test, uses_this, uses_new_target);
            collect_capture_from_expr(&cond.consequent, uses_this, uses_new_target);
            collect_capture_from_expr(&cond.alternate, uses_this, uses_new_target);
        }
        Expression::UnaryExpression(unary) => {
            collect_capture_from_expr(&unary.argument, uses_this, uses_new_target);
        }
        Expression::SequenceExpression(seq) => {
            for expr in &seq.expressions {
                collect_capture_from_expr(expr, uses_this, uses_new_target);
            }
        }
        Expression::NewExpression(new_expr) => {
            collect_capture_from_expr(&new_expr.callee, uses_this, uses_new_target);
            for arg in &new_expr.arguments {
                if let Some(expr) = arg.as_expression() {
                    collect_capture_from_expr(expr, uses_this, uses_new_target);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn is_new_target_meta(meta: &oxc_ast::ast::MetaProperty<'_>) -> bool {
    meta.meta.name.as_str() == "new" && meta.property.name.as_str() == "target"
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ArrowCaptures {
    pub(super) this_slot: Option<u8>,
    pub(super) new_target_slot: Option<u8>,
}

pub(super) fn is_global_this(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Identifier(id) if id.name.as_str() == "globalThis")
}

pub(super) fn is_undefined_literal(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(id) => id.name.as_str() == "undefined",
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => {
            matches!(&unary.argument, Expression::NumericLiteral(num) if num.value == 0.0)
        }
        Expression::ParenthesizedExpression(paren) => is_undefined_literal(&paren.expression),
        _ => false,
    }
}

pub(super) fn negative_integer_literal(expr: &Expression<'_>) -> Option<i32> {
    match expr {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::UnaryNegation => {
            if let Expression::NumericLiteral(num) = &unary.argument {
                if num.value.fract() == 0.0 && num.value >= 0.0 && num.value <= i32::MAX as f64 {
                    return Some(num.value as i32);
                }
            }
            None
        }
        Expression::ParenthesizedExpression(paren) => negative_integer_literal(&paren.expression),
        _ => None,
    }
}

pub(super) fn is_condition_comparison(op: oxc_ast::ast::BinaryOperator) -> bool {
    use oxc_ast::ast::BinaryOperator as Op;
    matches!(
        op,
        Op::Equality
            | Op::Inequality
            | Op::StrictEquality
            | Op::StrictInequality
            | Op::LessThan
            | Op::LessEqualThan
            | Op::GreaterThan
            | Op::GreaterEqualThan
    )
}

pub(super) fn stmts_capture_this_in_arrow(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(stmt_captures_this_in_arrow)
}

fn stmt_captures_this_in_arrow(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ExpressionStatement(es) => expr_captures_this_in_arrow(&es.expression),
        Statement::BlockStatement(block) => stmts_capture_this_in_arrow(&block.body),
        Statement::IfStatement(if_stmt) => {
            expr_captures_this_in_arrow(&if_stmt.test)
                || stmt_captures_this_in_arrow(&if_stmt.consequent)
                || if_stmt
                    .alternate
                    .as_ref()
                    .is_some_and(stmt_captures_this_in_arrow)
        }
        Statement::ReturnStatement(ret) => ret
            .argument
            .as_ref()
            .is_some_and(expr_captures_this_in_arrow),
        Statement::VariableDeclaration(decl) => decl
            .declarations
            .iter()
            .any(|d| d.init.as_ref().is_some_and(expr_captures_this_in_arrow)),
        _ => false,
    }
}

fn expr_captures_this_in_arrow(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ArrowFunctionExpression(arrow) => arrow_captures_from_body(arrow.body.as_ref())
            .this_slot
            .is_some(),
        Expression::CallExpression(call) => {
            expr_captures_this_in_arrow(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(|arg| arg.as_expression().is_some_and(expr_captures_this_in_arrow))
        }
        Expression::StaticMemberExpression(member) => expr_captures_this_in_arrow(&member.object),
        Expression::ComputedMemberExpression(member) => {
            expr_captures_this_in_arrow(&member.object)
                || expr_captures_this_in_arrow(&member.expression)
        }
        Expression::AssignmentExpression(assign) => expr_captures_this_in_arrow(&assign.right),
        Expression::BinaryExpression(bin) => {
            expr_captures_this_in_arrow(&bin.left) || expr_captures_this_in_arrow(&bin.right)
        }
        Expression::LogicalExpression(log) => {
            expr_captures_this_in_arrow(&log.left) || expr_captures_this_in_arrow(&log.right)
        }
        Expression::ConditionalExpression(cond) => {
            expr_captures_this_in_arrow(&cond.test)
                || expr_captures_this_in_arrow(&cond.consequent)
                || expr_captures_this_in_arrow(&cond.alternate)
        }
        Expression::ParenthesizedExpression(paren) => {
            expr_captures_this_in_arrow(&paren.expression)
        }
        _ => false,
    }
}
