use oxc_ast::ast::{Argument, CallExpression, ChainElement, Expression, FunctionBody, Statement};
use oxc_span::{GetSpan, Span};

use super::BuildContext;
use crate::transform::source_emit::original_node_text;

pub(super) fn expression_text_with_raw_object_call_args(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    let mut replacements = Vec::new();
    collect_raw_object_call_arg_replacements(expression, source_text, context, &mut replacements);
    apply_span_replacements(expression.span(), source_text, replacements)
}

fn collect_raw_object_call_arg_replacements(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    match expression {
        Expression::CallExpression(call) => {
            collect_call_raw_object_replacements(call, source_text, context, replacements);
        }
        Expression::ChainExpression(chain) => {
            if let ChainElement::CallExpression(call) = &chain.expression {
                collect_call_raw_object_replacements(call, source_text, context, replacements);
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            collect_function_body_raw_object_replacements(
                &arrow.body,
                source_text,
                context,
                replacements,
            );
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            collect_raw_object_call_arg_replacements(
                &parenthesized.expression,
                source_text,
                context,
                replacements,
            );
        }
        _ => {}
    }
}

fn collect_call_raw_object_replacements(
    call: &CallExpression<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    for argument in &call.arguments {
        let Some(expression) = argument.as_expression() else {
            continue;
        };
        if let Some(raw_object_text) = raw_object_argument_text(expression, source_text, context) {
            replacements.push((argument.span(), raw_object_text));
        } else {
            collect_raw_object_call_arg_replacements(
                expression,
                source_text,
                context,
                replacements,
            );
        }
    }
}

fn collect_function_body_raw_object_replacements(
    body: &FunctionBody<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    for statement in &body.statements {
        collect_statement_raw_object_replacements(statement, source_text, context, replacements);
    }
}

fn collect_statement_raw_object_replacements(
    statement: &Statement<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    match statement {
        Statement::ExpressionStatement(statement) => collect_raw_object_call_arg_replacements(
            &statement.expression,
            source_text,
            context,
            replacements,
        ),
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_raw_object_call_arg_replacements(
                    argument,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_statement_raw_object_replacements(
                    statement,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        Statement::IfStatement(statement) => {
            collect_statement_raw_object_replacements(
                &statement.consequent,
                source_text,
                context,
                replacements,
            );
            if let Some(alternate) = &statement.alternate {
                collect_statement_raw_object_replacements(
                    alternate,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        _ => {}
    }
}

fn raw_object_argument_text(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    if !matches!(&member.object, Expression::ThisExpression(_)) {
        return None;
    }
    let name = member.property.name.as_str();
    if !context.is_raw_object_field(name) {
        return None;
    }
    Some(format!(
        "ObservedObject.GetRawObject({})",
        original_node_text(expression, source_text)
    ))
}

pub(super) fn raw_object_argument_text_from_argument(
    argument: &Argument<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    argument
        .as_expression()
        .and_then(|expression| raw_object_argument_text(expression, source_text, context))
}

fn apply_span_replacements(
    base_span: Span,
    source_text: &str,
    mut replacements: Vec<(Span, String)>,
) -> Option<String> {
    if replacements.is_empty() {
        return None;
    }
    replacements.sort_by_key(|(span, _)| span.start);
    let mut out = String::new();
    let mut cursor = base_span.start as usize;
    for (span, replacement) in replacements {
        let start = span.start as usize;
        let end = span.end as usize;
        if start < cursor || end > base_span.end as usize {
            continue;
        }
        out.push_str(&source_text[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&source_text[cursor..base_span.end as usize]);
    Some(out.trim().to_string())
}
