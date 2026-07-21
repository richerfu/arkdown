use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, CallExpression, Expression, FormalParameters,
    FunctionBody, Statement, TSTypeAnnotation,
};
use oxc_ast::AstBuilder;
use oxc_span::SPAN;

use crate::transform::build_fn::{BuildBodyInfo, BuildLoweringError, StylesBodyLowerer};
use crate::transform::viewpu_emit::{
    GeneratedViewpuName, ParamsInterfaceAstEmitter, ParamsInterfaceField, ParamsInterfaceFieldType,
    TopLevelFunctionAstEmitter, ViewpuAstBuilder,
};

pub(crate) struct ParamsInterfaceEmitter<'a> {
    annotation: &'a AnnotationDeclaration<'a>,
}

impl<'a> ParamsInterfaceEmitter<'a> {
    pub(crate) fn new(annotation: &'a AnnotationDeclaration<'a>) -> Self {
        Self { annotation }
    }

    pub(crate) fn statement<'b>(&self, allocator: &'b Allocator, exported: bool) -> Statement<'b> {
        let fields = self.fields();
        ParamsInterfaceAstEmitter::new(self.annotation.id.name.as_str(), fields, exported)
            .statement(AstBuilder::new(allocator), allocator)
    }

    fn fields(&self) -> Vec<ParamsInterfaceField<'a>> {
        let mut fields = Vec::new();
        for element in &self.annotation.body.body {
            let AnnotationElement::PropertyDefinition(property) = element;
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let type_annotation = property
                .type_annotation
                .as_ref()
                .map(|annotation| ParamsInterfaceFieldType::Annotation(annotation.as_ref()))
                .or(Some(ParamsInterfaceFieldType::Unknown));
            fields.push(ParamsInterfaceField::new(
                name,
                property.optional,
                type_annotation,
            ));
        }
        fields
    }
}

pub(crate) struct TopLevelFunctionEmitter;

impl TopLevelFunctionEmitter {
    pub(crate) fn decorated_with_params_statement<'b>(
        allocator: &'b Allocator,
        exported: bool,
        name: &str,
        params: &FormalParameters<'_>,
        append_parent: bool,
        return_annotation: Option<&TSTypeAnnotation<'_>>,
        body: &BuildBodyInfo<'_>,
    ) -> Result<Statement<'b>, BuildLoweringError> {
        let body_statements = body.statements(allocator)?;
        Ok(TopLevelFunctionAstEmitter::new(exported, name)
            .statement_with_formal_parameters_and_body_statements(
                allocator,
                |allocator| Self::formal_parameters(params, append_parent, allocator),
                |allocator| Self::return_type(return_annotation, allocator),
                |_| body_statements,
            ))
    }

    pub(crate) fn decorated_styles_statement<'b>(
        allocator: &'b Allocator,
        exported: bool,
        name: &str,
        params: &FormalParameters<'_>,
        return_annotation: Option<&TSTypeAnnotation<'_>>,
        body: Option<&FunctionBody<'_>>,
    ) -> Statement<'b> {
        TopLevelFunctionAstEmitter::new(exported, name)
            .statement_with_formal_parameters_and_body_statements(
                allocator,
                |allocator| Self::formal_parameters(params, false, allocator),
                |allocator| Self::return_type(return_annotation, allocator),
                |allocator| StylesBodyLowerer::new(body).statements(allocator),
            )
    }

    pub(crate) fn extend_with_body_statement<'b>(
        allocator: &'b Allocator,
        exported: bool,
        component: &str,
        name: &str,
        params: &FormalParameters<'_>,
        body: Option<&FunctionBody<'_>>,
    ) -> Statement<'b> {
        let function_name = GeneratedViewpuName::extend_function(component, name);
        TopLevelFunctionAstEmitter::new(exported, &function_name)
            .statement_with_formal_parameters_and_body_statements(
                allocator,
                |allocator| Self::formal_parameters(params, false, allocator),
                |allocator| Some(ViewpuAstBuilder::new(allocator).void_type_annotation()),
                |allocator| ExtendBodyEmitter::statements(body, component, allocator),
            )
    }

    fn formal_parameters<'b>(
        params: &FormalParameters<'_>,
        append_parent: bool,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, FormalParameters<'b>> {
        let builder = ViewpuAstBuilder::new(allocator);
        if append_parent {
            builder.clone_formal_parameters_with_parent(params)
        } else {
            builder.clone_formal_parameters(params)
        }
    }

    fn return_type<'b>(
        annotation: Option<&TSTypeAnnotation<'_>>,
        allocator: &'b Allocator,
    ) -> Option<oxc_allocator::Box<'b, TSTypeAnnotation<'b>>> {
        annotation
            .map(|annotation| ViewpuAstBuilder::new(allocator).clone_type_annotation(annotation))
    }
}

pub(crate) struct ExtendBodyEmitter;

impl ExtendBodyEmitter {
    pub(crate) fn statements<'b>(
        body: Option<&FunctionBody<'_>>,
        component: &str,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let Some(body) = body else {
            return OxcVec::new_in(&allocator);
        };
        ExtendBodyStatementEmitter::new(body, component).statements(allocator)
    }
}

struct ExtendBodyStatementEmitter<'a> {
    body: &'a FunctionBody<'a>,
    component: &'a str,
}

impl<'a> ExtendBodyStatementEmitter<'a> {
    fn new(body: &'a FunctionBody<'a>, component: &'a str) -> Self {
        Self { body, component }
    }

    fn statements<'b>(&self, allocator: &'b Allocator) -> OxcVec<'b, Statement<'b>> {
        let mut statements = OxcVec::new_in(&allocator);
        for statement in &self.body.statements {
            self.push_statement(statement, allocator, &mut statements);
        }
        statements
    }

    fn push_statement<'b>(
        &self,
        statement: &Statement<'_>,
        allocator: &'b Allocator,
        statements: &mut OxcVec<'b, Statement<'b>>,
    ) {
        if let Statement::ExpressionStatement(expression_statement) = statement {
            if let Expression::LeadingDotExpression(leading_dot) = &expression_statement.expression
            {
                if Self::is_attribute_chain(&leading_dot.expression) {
                    self.push_attribute_chain(&leading_dot.expression, allocator, statements);
                    return;
                }
            }
        }

        statements.push(self.statement(statement, allocator));
    }

    fn is_attribute_chain(expression: &Expression<'_>) -> bool {
        let Expression::CallExpression(call) = expression else {
            return false;
        };
        match &call.callee {
            Expression::Identifier(_) => true,
            Expression::StaticMemberExpression(member) => Self::is_attribute_chain(&member.object),
            _ => false,
        }
    }

    fn push_attribute_chain<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
        statements: &mut OxcVec<'b, Statement<'b>>,
    ) {
        let Expression::CallExpression(call) = expression else {
            return;
        };
        match &call.callee {
            Expression::Identifier(identifier) => {
                statements.push(self.attribute_statement(
                    identifier.name.as_str(),
                    call,
                    allocator,
                ));
            }
            Expression::StaticMemberExpression(member) => {
                self.push_attribute_chain(&member.object, allocator, statements);
                statements.push(self.attribute_statement(
                    member.property.name.as_str(),
                    call,
                    allocator,
                ));
            }
            _ => {}
        }
    }

    fn attribute_statement<'b>(
        &self,
        attribute: &str,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        let ast = AstBuilder::new(allocator);
        let member = ast.member_expression_static(
            call.span,
            ast.expression_identifier(SPAN, allocator.alloc_str(self.component)),
            ast.identifier_name(call.span, allocator.alloc_str(attribute)),
            false,
        );
        let type_arguments = call
            .type_arguments
            .as_ref()
            .map(|type_arguments| type_arguments.clone_in(allocator));
        let expression = ast.expression_call(
            call.span,
            Expression::from(member),
            type_arguments,
            call.arguments.clone_in(allocator),
            call.optional,
        );
        ast.statement_expression(call.span, expression)
    }

    fn statement<'b>(&self, statement: &Statement<'_>, allocator: &'b Allocator) -> Statement<'b> {
        match statement {
            Statement::ExpressionStatement(statement) => {
                let expression = self.expression(&statement.expression, allocator);
                AstBuilder::new(allocator).statement_expression(statement.span, expression)
            }
            _ => statement.clone_in(allocator),
        }
    }

    fn expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        match expression {
            Expression::LeadingDotExpression(expression) => {
                let mut expression = expression.expression.clone_in(allocator);
                ExtendLeadingDotPrefixer::new(allocator, self.component).prefix(&mut expression);
                expression
            }
            _ => expression.clone_in(allocator),
        }
    }
}

struct ExtendLeadingDotPrefixer<'a, 'b> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
    component: &'b str,
}

impl<'a, 'b> ExtendLeadingDotPrefixer<'a, 'b> {
    fn new(allocator: &'a Allocator, component: &'b str) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
            component,
        }
    }

    fn prefix(&self, expression: &mut Expression<'a>) -> bool {
        if let Expression::CallExpression(call) = expression {
            self.prefix_callee(&mut call.callee)
        } else {
            false
        }
    }

    fn prefix_callee(&self, callee: &mut Expression<'a>) -> bool {
        match callee {
            Expression::Identifier(identifier) => {
                let property_name = self.allocator.alloc_str(identifier.name.as_str());
                let member = self.ast.member_expression_static(
                    identifier.span,
                    self.ast
                        .expression_identifier(SPAN, self.allocator.alloc_str(self.component)),
                    self.ast.identifier_name(identifier.span, property_name),
                    false,
                );
                *callee = Expression::from(member);
                true
            }
            Expression::StaticMemberExpression(member) => self.prefix(&mut member.object),
            Expression::CallExpression(call) => self.prefix_callee(&mut call.callee),
            _ => false,
        }
    }
}
