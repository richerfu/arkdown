use oxc_allocator::{Allocator, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrowFunctionBody, Expression, FormalParameters, IdentifierName, MemberExpression,
    Statement, TSTypeAnnotation,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::SPAN;

use crate::transform::build_fn::{BuildBodyInfo, BuildLoweringError};
use crate::transform::component_meta::ArkuiRuntimeNames;
use crate::transform::viewpu_emit::{
    GeneratedViewpuName, TopLevelFunctionAstEmitter, ViewpuAstBuilder,
};

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
        body_statements: OxcVec<'b, Statement<'b>>,
    ) -> Statement<'b> {
        let function_name = GeneratedViewpuName::extend_function(component, name);
        TopLevelFunctionAstEmitter::new(exported, &function_name)
            .statement_with_formal_parameters_and_body_statements(
                allocator,
                |allocator| Self::formal_parameters(params, false, allocator),
                |allocator| Some(ViewpuAstBuilder::new(allocator).void_type_annotation()),
                |_| body_statements,
            )
    }

    pub(crate) fn animatable_extend_statement<'b>(
        allocator: &'b Allocator,
        exported: bool,
        component: &str,
        name: &str,
        params: &FormalParameters<'_>,
        body_statements: OxcVec<'b, Statement<'b>>,
        runtime_names: &ArkuiRuntimeNames,
    ) -> Statement<'b> {
        AnimatableExtendEmitter::new(component, name, params, body_statements, runtime_names)
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

struct AnimatableExtendEmitter<'source, 'output> {
    component: &'source str,
    name: &'source str,
    params: &'source FormalParameters<'source>,
    body_statements: OxcVec<'output, Statement<'output>>,
    runtime_names: &'source ArkuiRuntimeNames,
}

impl<'source, 'output> AnimatableExtendEmitter<'source, 'output> {
    fn new(
        component: &'source str,
        name: &'source str,
        params: &'source FormalParameters<'source>,
        body_statements: OxcVec<'output, Statement<'output>>,
        runtime_names: &'source ArkuiRuntimeNames,
    ) -> Self {
        Self {
            component,
            name,
            params,
            body_statements,
            runtime_names,
        }
    }

    fn statement(&self, allocator: &'output Allocator, exported: bool) -> Statement<'output> {
        if self.body_statements.is_empty() {
            return TopLevelFunctionAstEmitter::new(exported, self.name)
                .statement_with_formal_parameters_and_body_statements(
                    allocator,
                    |allocator| self.formal_parameters(allocator),
                    |_| None,
                    |allocator| OxcVec::new_in(&allocator),
                );
        }
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

    fn body_statements(
        &self,
        allocator: &'output Allocator,
    ) -> OxcVec<'output, Statement<'output>> {
        let ast = AstBuilder::new(allocator);
        let attributes = OxcVec::from_iter_in(
            self.body_statements
                .iter()
                .map(|statement| statement.clone_in(allocator)),
            &ast,
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
