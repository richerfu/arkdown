use rustc_hash::FxHashSet;

use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionBody, CallExpression, Declaration,
    ExportDefaultDeclarationKind, Expression, FunctionBody, Program, Statement, StructElement,
    StructStatement,
};

use super::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::component_meta::ArkuiRuntimeNames;
use crate::transform::viewpu_emit::GeneratedViewpuName;

pub(super) struct ComponentNameCollector {
    custom_components: FxHashSet<String>,
    runtime_names: ArkuiRuntimeNames,
    names: Vec<String>,
    seen: FxHashSet<String>,
}

impl ComponentNameCollector {
    pub(super) fn new(custom_components: FxHashSet<String>) -> Self {
        Self {
            custom_components,
            runtime_names: ArkuiRuntimeNames::default(),
            names: Vec::new(),
            seen: FxHashSet::default(),
        }
    }

    pub(super) fn collect_program(mut self, program: &Program<'_>) -> Vec<String> {
        for statement in &program.body {
            self.collect_statement_components(statement);
        }
        for statement in &program.body {
            self.collect_statement_if_components(statement);
        }
        self.names
    }

    fn collect_statement_components(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement) => {
                self.collect_struct_components(statement);
            }
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
                    self.collect_struct_components(statement);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    self.collect_struct_components(statement);
                }
            }
            _ => {}
        }
    }

    fn collect_struct_components(&mut self, statement: &StructStatement<'_>) {
        for element in &statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            let Some(body) = method.value.body.as_deref() else {
                continue;
            };
            if method
                .key
                .static_name()
                .is_some_and(|name| name == GeneratedViewpuName::BUILD)
                || method.decorators.iter().any(|decorator| {
                    ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER
                })
            {
                self.collect_body_components(body);
            }
        }
    }

    fn collect_statement_if_components(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement) => {
                self.collect_struct_if_components(statement);
            }
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
                    self.collect_struct_if_components(statement);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    self.collect_struct_if_components(statement);
                }
            }
            _ => {}
        }
    }

    fn collect_struct_if_components(&mut self, statement: &StructStatement<'_>) {
        for element in &statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            let Some(body) = method.value.body.as_deref() else {
                continue;
            };
            self.collect_body_if_components(body);
        }
    }

    fn collect_body_if_components(&mut self, body: &FunctionBody<'_>) {
        for statement in &body.statements {
            self.collect_if_statement_components(statement);
        }
    }

    fn collect_if_statement_components(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_if_statement_components(statement);
                }
            }
            Statement::IfStatement(statement) => {
                self.push_if_component_name(false);
                self.collect_if_statement_components(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.collect_if_statement_components(alternate);
                }
            }
            _ => {}
        }
    }

    fn collect_body_components(&mut self, body: &FunctionBody<'_>) {
        for statement in &body.statements {
            self.collect_ui_statement_components(statement);
        }
    }

    fn collect_ui_statement_components(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_ui_statement_components(statement);
                }
            }
            Statement::ExpressionStatement(statement) => {
                self.collect_ui_expression_components(&statement.expression);
            }
            Statement::IfStatement(statement) => {
                self.push_if_component_name(true);
                self.collect_ui_statement_components(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.collect_ui_statement_components(alternate);
                }
            }
            _ => {}
        }
    }

    fn collect_ui_expression_components(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                self.collect_arkui_component_expression(component);
            }
            Expression::CallExpression(call) => {
                self.collect_call_expression_components(call);
            }
            _ => {}
        }
    }

    fn collect_arkui_component_expression(&mut self, component: &ArkUIComponentExpression<'_>) {
        if let Expression::Identifier(identifier) = &component.callee {
            self.push_component_name(identifier.name.as_str(), true);
        }
        for argument in &component.arguments {
            self.collect_argument_components(argument);
        }
        for child in &component.children {
            match child {
                ArkUIChild::Component(component) => {
                    self.collect_arkui_component_expression(component);
                }
                ArkUIChild::Expression(expression) => {
                    self.collect_ui_expression_components(expression);
                }
                ArkUIChild::Statement(statement) => {
                    self.collect_ui_statement_components(statement);
                }
            }
        }
    }

    fn collect_argument_components(&mut self, argument: &Argument<'_>) {
        let Some(expression) = argument.as_expression() else {
            return;
        };
        if let Expression::ArrowFunctionExpression(arrow) = expression {
            if let ArrowFunctionBody::FunctionBody(body) = &arrow.body {
                self.collect_body_components(body);
            }
        }
    }

    fn collect_call_expression_components(&mut self, call: &CallExpression<'_>) {
        match &call.callee {
            Expression::Identifier(identifier) => {
                if ComponentCallName::new(identifier.name.as_str()).is_component_candidate() {
                    self.push_component_name(identifier.name.as_str(), true);
                }
            }
            Expression::StaticMemberExpression(member) => {
                if let Expression::CallExpression(inner) = &member.object {
                    self.collect_call_expression_components(inner);
                }
            }
            _ => {}
        }
    }

    fn push_if_component_name(&mut self, respect_custom_components: bool) {
        let name = self.runtime_names.component_if.clone();
        self.push_component_name(&name, respect_custom_components);
    }

    fn push_component_name(&mut self, name: &str, respect_custom_components: bool) {
        if respect_custom_components && self.custom_components.contains(name) {
            return;
        }
        if self.seen.insert(name.to_string()) {
            self.names.push(name.to_string());
        }
    }
}

struct ComponentCallName<'a> {
    name: &'a str,
}

impl<'a> ComponentCallName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn is_component_candidate(&self) -> bool {
        self.name.chars().next().is_some_and(char::is_uppercase)
    }
}
