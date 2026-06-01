use std::collections::HashSet;

use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, CallExpression, Declaration,
    ExportDefaultDeclarationKind, Expression, FunctionBody, Statement, StructElement,
    StructStatement,
};

use super::decorator_name;

pub(super) fn collect_statement_component_names(
    statement: &Statement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::StructStatement(statement) => {
            collect_struct_component_names(statement, custom_components, names, seen);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                collect_struct_component_names(statement, custom_components, names, seen);
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                collect_struct_component_names(statement, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_struct_component_names(
    statement: &StructStatement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        let Some(body) = method.value.body.as_deref() else {
            continue;
        };
        if method.key.static_name().is_some_and(|name| name == "build")
            || method
                .decorators
                .iter()
                .any(|decorator| decorator_name(decorator) == "Builder")
        {
            collect_body_component_names(body, custom_components, names, seen);
        }
    }
}

pub(super) fn collect_statement_if_component_names(
    statement: &Statement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::StructStatement(statement) => {
            collect_struct_if_component_names(statement, names, seen);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                collect_struct_if_component_names(statement, names, seen);
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                collect_struct_if_component_names(statement, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_struct_if_component_names(
    statement: &StructStatement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        let Some(body) = method.value.body.as_deref() else {
            continue;
        };
        collect_body_if_component_names(body, names, seen);
    }
}

fn collect_body_if_component_names(
    body: &FunctionBody<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for statement in &body.statements {
        collect_if_statement_component_names(statement, names, seen);
    }
}

fn collect_if_statement_component_names(
    statement: &Statement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_if_statement_component_names(statement, names, seen);
            }
        }
        Statement::IfStatement(statement) => {
            push_component_name("If", &HashSet::new(), names, seen);
            collect_if_statement_component_names(&statement.consequent, names, seen);
            if let Some(alternate) = &statement.alternate {
                collect_if_statement_component_names(alternate, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_body_component_names(
    body: &FunctionBody<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for statement in &body.statements {
        collect_ui_statement_component_names(statement, custom_components, names, seen);
    }
}

fn collect_ui_statement_component_names(
    statement: &Statement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_ui_statement_component_names(statement, custom_components, names, seen);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_ui_expression_component_names(
                &statement.expression,
                custom_components,
                names,
                seen,
            );
        }
        Statement::IfStatement(statement) => {
            push_component_name("If", custom_components, names, seen);
            collect_ui_statement_component_names(
                &statement.consequent,
                custom_components,
                names,
                seen,
            );
            if let Some(alternate) = &statement.alternate {
                collect_ui_statement_component_names(alternate, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_ui_expression_component_names(
    expression: &Expression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match expression {
        Expression::ArkUIComponentExpression(component) => {
            collect_arkui_component_expression_names(component, custom_components, names, seen);
        }
        Expression::CallExpression(call) => {
            collect_call_expression_component_names(call, custom_components, names, seen);
        }
        _ => {}
    }
}

fn collect_arkui_component_expression_names(
    component: &ArkUIComponentExpression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let Expression::Identifier(identifier) = &component.callee {
        push_component_name(identifier.name.as_str(), custom_components, names, seen);
    }
    for argument in &component.arguments {
        collect_argument_component_names(argument, custom_components, names, seen);
    }
    for child in &component.children {
        match child {
            ArkUIChild::Component(component) => {
                collect_arkui_component_expression_names(component, custom_components, names, seen);
            }
            ArkUIChild::Expression(expression) => {
                collect_ui_expression_component_names(expression, custom_components, names, seen);
            }
            ArkUIChild::Statement(statement) => {
                collect_ui_statement_component_names(statement, custom_components, names, seen);
            }
        }
    }
}

fn collect_argument_component_names(
    argument: &Argument<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(expression) = argument.as_expression() else {
        return;
    };
    if let Expression::ArrowFunctionExpression(arrow) = expression {
        collect_body_component_names(&arrow.body, custom_components, names, seen);
    }
}

fn collect_call_expression_component_names(
    call: &CallExpression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match &call.callee {
        Expression::Identifier(identifier) => {
            if identifier
                .name
                .as_str()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
            {
                push_component_name(identifier.name.as_str(), custom_components, names, seen);
            }
        }
        Expression::StaticMemberExpression(member) => {
            if let Expression::CallExpression(inner) = &member.object {
                collect_call_expression_component_names(inner, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn push_component_name(
    name: &str,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if custom_components.contains(name) {
        return;
    }
    if seen.insert(name.to_string()) {
        names.push(name.to_string());
    }
}
