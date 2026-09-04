use oxc_allocator::{Allocator, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, Argument, ArrowFunctionBody, CallExpression,
    Expression, FormalParameters, FunctionBody, IdentifierName, MemberExpression, Statement,
    TSTypeAnnotation,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::SPAN;

use crate::transform::build_fn::{BuildBodyInfo, BuildLoweringError};
use crate::transform::component_meta::ArkuiRuntimeNames;
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
            .statement(&AstBuilder::new(allocator), allocator)
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

    pub(crate) fn animatable_extend_statement<'b>(
        allocator: &'b Allocator,
        exported: bool,
        component: &str,
        name: &str,
        params: &FormalParameters<'_>,
        body: Option<&FunctionBody<'_>>,
        runtime_names: &ArkuiRuntimeNames,
    ) -> Statement<'b> {
        AnimatableExtendEmitter::new(component, name, params, body, runtime_names)
            .statement(allocator, exported)
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

struct AnimatableExtendEmitter<'a> {
    component: &'a str,
    name: &'a str,
    params: &'a FormalParameters<'a>,
    body: Option<&'a FunctionBody<'a>>,
    runtime_names: &'a ArkuiRuntimeNames,
}

impl<'a> AnimatableExtendEmitter<'a> {
    fn new(
        component: &'a str,
        name: &'a str,
        params: &'a FormalParameters<'a>,
        body: Option<&'a FunctionBody<'a>>,
        runtime_names: &'a ArkuiRuntimeNames,
    ) -> Self {
        Self {
            component,
            name,
            params,
            body,
            runtime_names,
        }
    }

    fn statement<'b>(&self, allocator: &'b Allocator, exported: bool) -> Statement<'b> {
        TopLevelFunctionAstEmitter::new(exported, self.name)
            .statement_with_formal_parameters_and_body_statements(
                allocator,
                |allocator| self.formal_parameters(allocator),
                |allocator| Some(ViewpuAstBuilder::new(allocator).void_type_annotation()),
                |allocator| self.body_statements(allocator),
            )
    }

    fn formal_parameters<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> oxc_allocator::Box<'b, FormalParameters<'b>> {
        let builder = ViewpuAstBuilder::new(allocator);
        let mut params = self.params.clone_in(allocator);
        params
            .items
            .push(builder.formal_parameter(&self.runtime_names.elmt_id));
        params
            .items
            .push(builder.formal_parameter(&self.runtime_names.is_initial_render));
        params
            .items
            .push(builder.formal_parameter(GeneratedViewpuName::PARENT));
        oxc_allocator::Box::new_in(params, &allocator)
    }

    fn body_statements<'b>(&self, allocator: &'b Allocator) -> OxcVec<'b, Statement<'b>> {
        let Some(body) = self.body else {
            return OxcVec::new_in(&allocator);
        };
        let Some(first_statement) = body.statements.first() else {
            return OxcVec::new_in(&allocator);
        };

        let ast = AstBuilder::new(allocator);
        let mut attributes = OxcVec::new_in(&ast);
        ExtendBodyStatementEmitter::new(body, self.component).push_statement(
            first_statement,
            allocator,
            &mut attributes,
        );

        let callback = self.update_callback(&ast, &attributes);
        let mut create_arguments = self.named_parameter_expressions(&ast);
        create_arguments.insert(0, self.string_expression(&ast, self.name));
        create_arguments.push(callback);
        let create = self.component_call_statement(
            &ast,
            &self.runtime_names.create_animatable_property,
            create_arguments,
        );

        let mut initial = OxcVec::new_in(&ast);
        initial.push(create);
        initial.extend(attributes);

        let mut update_arguments = self.named_parameter_expressions(&ast);
        update_arguments.insert(0, self.string_expression(&ast, self.name));
        let mut update = OxcVec::new_in(&ast);
        update.push(self.component_call_statement(
            &ast,
            &self.runtime_names.update_animatable_property,
            update_arguments,
        ));

        let condition = Expression::new_identifier(
            SPAN,
            allocator.alloc_str(&self.runtime_names.is_initial_render),
            &ast,
        );
        OxcVec::from_value_in(
            Statement::new_if_statement(
                SPAN,
                condition,
                Statement::new_block_statement(SPAN, initial, &ast),
                Some(Statement::new_block_statement(SPAN, update, &ast)),
                &ast,
            ),
            &ast,
        )
    }

    fn update_callback<'b>(
        &self,
        ast: &AstBuilder<'b>,
        attributes: &OxcVec<'b, Statement<'b>>,
    ) -> Expression<'b> {
        let allocator = ast.allocator();
        let params = oxc_allocator::Box::new_in(self.params.clone_in(allocator), ast);
        let mut statements = OxcVec::new_in(ast);
        statements.push(self.runtime_call_statement(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.start_get_access_recording_for,
            OxcVec::from_value_in(
                Expression::new_identifier(
                    SPAN,
                    allocator.alloc_str(&self.runtime_names.elmt_id),
                    ast,
                ),
                ast,
            ),
        ));
        statements.push(self.runtime_call_statement(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.get_and_push_frame_node,
            OxcVec::from_iter_in(
                [
                    self.string_expression(ast, self.component),
                    Expression::new_identifier(
                        SPAN,
                        allocator.alloc_str(&self.runtime_names.elmt_id),
                        ast,
                    ),
                ],
                ast,
            ),
        ));
        statements.extend(
            attributes
                .iter()
                .map(|statement| statement.clone_in(allocator)),
        );
        statements.push(self.runtime_call_statement(
            ast,
            &self.runtime_names.view_stack_processor,
            &self.runtime_names.stop_get_access_recording,
            OxcVec::new_in(ast),
        ));
        statements.push(self.runtime_call_statement(
            ast,
            GeneratedViewpuName::PARENT,
            &self.runtime_names.finish_update_func,
            OxcVec::from_value_in(
                Expression::new_identifier(
                    SPAN,
                    allocator.alloc_str(&self.runtime_names.elmt_id),
                    ast,
                ),
                ast,
            ),
        ));
        let body = ArrowFunctionBody::new_function_body(SPAN, OxcVec::new_in(ast), statements, ast);
        Expression::new_arrow_function_expression(SPAN, false, None, params, None, body, ast)
    }

    fn named_parameter_expressions<'b>(&self, ast: &AstBuilder<'b>) -> OxcVec<'b, Expression<'b>> {
        OxcVec::from_iter_in(
            self.params.items.iter().filter_map(|parameter| {
                parameter
                    .pattern
                    .get_binding_identifier()
                    .map(|identifier| {
                        Expression::new_identifier(
                            identifier.span,
                            ast.allocator().alloc_str(identifier.name.as_str()),
                            ast,
                        )
                    })
            }),
            ast,
        )
    }

    fn component_call_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        method: &str,
        arguments: OxcVec<'b, Expression<'b>>,
    ) -> Statement<'b> {
        self.runtime_call_statement(ast, self.component, method, arguments)
    }

    fn runtime_call_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        receiver: &str,
        method: &str,
        arguments: OxcVec<'b, Expression<'b>>,
    ) -> Statement<'b> {
        let member = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(receiver), ast),
            IdentifierName::new(SPAN, ast.allocator().alloc_str(method), ast),
            false,
            ast,
        );
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(member),
            None,
            OxcVec::from_iter_in(arguments.into_iter().map(Argument::from), ast),
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, call, ast)
    }

    fn string_expression<'b>(&self, ast: &AstBuilder<'b>, value: &str) -> Expression<'b> {
        Expression::new_string_literal(SPAN, ast.allocator().alloc_str(value), None, ast)
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
        let member = MemberExpression::new_static_member_expression(
            call.span,
            Expression::new_identifier(SPAN, allocator.alloc_str(self.component), &ast),
            IdentifierName::new(call.span, allocator.alloc_str(attribute), &ast),
            false,
            &ast,
        );
        let type_arguments = call
            .type_arguments
            .as_ref()
            .map(|type_arguments| type_arguments.clone_in(allocator));
        let expression = Expression::new_call_expression(
            call.span,
            Expression::from(member),
            type_arguments,
            call.arguments.clone_in(allocator),
            call.optional,
            &ast,
        );
        Statement::new_expression_statement(call.span, expression, &ast)
    }

    fn statement<'b>(&self, statement: &Statement<'_>, allocator: &'b Allocator) -> Statement<'b> {
        match statement {
            Statement::ExpressionStatement(statement) => {
                let expression = self.expression(&statement.expression, allocator);
                Statement::new_expression_statement(
                    statement.span,
                    expression,
                    &AstBuilder::new(allocator),
                )
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
                let member = MemberExpression::new_static_member_expression(
                    identifier.span,
                    Expression::new_identifier(
                        SPAN,
                        self.allocator.alloc_str(self.component),
                        &self.ast,
                    ),
                    IdentifierName::new(identifier.span, property_name, &self.ast),
                    false,
                    &self.ast,
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
