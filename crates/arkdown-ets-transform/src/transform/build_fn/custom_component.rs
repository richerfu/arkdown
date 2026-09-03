use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, ArrowFunctionBody, AssignmentTarget, BindingPattern,
    Expression, FormalParameter, FormalParameters, FunctionBody, IdentifierName, MemberExpression,
    NumberBase, ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::Span;
use oxc_span::SPAN;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};
use rustc_hash::FxHashSet;

use crate::transform::viewpu_emit::{GeneratedViewpuName, ViewpuStatementAstEmitter};

use super::raw_object::RawObjectArgumentTransform;
use super::{
    ArkuiCallName, BuildContext, BuildSourceMetadata, ComponentCallStatementEmitter,
    CustomComponentGeneration, CustomComponentInfo, LeafAttribute, LeafAttributeStatementEmitter,
};

pub(super) struct CustomComponentStatementEmitter;

impl CustomComponentStatementEmitter {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn common_wrapped<'a>(
        allocator: &'a Allocator,
        name: &str,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        builder_param: Option<(&str, OxcVec<'a, Statement<'a>>)>,
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        chain_attrs: &[LeafAttribute<'_>],
        builder_methods: &FxHashSet<String>,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> OxcVec<'a, Statement<'a>> {
        let ast = &AstBuilder::new(allocator);
        let mut statements = OxcVec::new_in(&allocator);
        let mut common_stmts = OxcVec::new_in(&allocator);
        common_stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_common,
            &context.runtime_names().method_create,
        ));
        LeafAttributeStatementEmitter::new(
            &context.runtime_names().component_common,
            span,
            chain_attrs,
            builder_methods,
            context,
        )
        .without_reusable_immutable_guard()
        .push_statements(&mut common_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &context.runtime_names().component_common,
                context.runtime_names(),
                common_stmts,
            ),
        );
        let is_reusable = component_info.is_some_and(|info| info.reusable);
        let mut custom_component_stmts = Self::custom(
            allocator,
            name,
            arguments,
            component_info,
            builder_param,
            reuse_id,
            span,
            source_metadata,
            context,
        );
        // Latest process_custom_component.js closes __Common__ before the
        // enclosing reusable __Recycle__ scope. Keep the recycle pop outside
        // the common wrapper so es2abc receives the same call ordering.
        let recycle_pop = is_reusable.then(|| custom_component_stmts.pop()).flatten();
        statements.extend(custom_component_stmts);
        statements.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_common,
            &context.runtime_names().method_pop,
        ));
        if let Some(recycle_pop) = recycle_pop {
            statements.push(recycle_pop);
        }
        statements
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn custom<'a>(
        allocator: &'a Allocator,
        name: &str,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        builder_param: Option<(&str, OxcVec<'a, Statement<'a>>)>,
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> OxcVec<'a, Statement<'a>> {
        let mut statements = OxcVec::new_in(&allocator);
        if component_info.is_some_and(|info| info.reusable_v2) {
            let (line, col) = source_metadata.source_position(span.start);
            let page = source_metadata.component_page_path();
            let params = CustomComponentParamExpressions::new(
                allocator,
                arguments,
                component_info,
                builder_param,
                context,
            );
            let ast = &AstBuilder::new(allocator);
            statements.push(
                ReusableV2CustomComponentAstEmitter::new(
                    allocator, ast, name, &page, line, col, context,
                )
                .statement(params.lambda, reuse_id),
            );
            return statements;
        }
        if component_info.is_some_and(|info| info.reusable) {
            let ast = &AstBuilder::new(allocator);
            let mut recycle_stmts = OxcVec::new_in(&allocator);
            recycle_stmts.push(ComponentCallStatementEmitter::empty(
                ast,
                &context.runtime_names().component_recycle,
                &context.runtime_names().method_create,
            ));
            statements.push(
                ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                    AstBuilder::new(allocator),
                    context.use_parent_context,
                    context.global_builder_parameter.as_deref(),
                    &context.runtime_names().component_recycle,
                    context.runtime_names(),
                    recycle_stmts,
                ),
            );
            statements.push(Self::reusable(
                allocator,
                name,
                arguments,
                component_info,
                builder_param,
                reuse_id,
                span,
                source_metadata,
                context,
            ));
            statements.push(ComponentCallStatementEmitter::empty(
                ast,
                &context.runtime_names().component_recycle,
                &context.runtime_names().method_pop,
            ));
            return statements;
        }

        let (line, col) = source_metadata.source_position(span.start);
        let page = source_metadata.component_page_path();
        let params = CustomComponentParamExpressions::new(
            allocator,
            arguments,
            component_info,
            builder_param,
            context,
        );
        let runtime_base_name = component_info
            .map(CustomComponentInfo::runtime_base_name)
            .unwrap_or_else(|| context.custom_component_base_name());
        let ast = &AstBuilder::new(allocator);
        statements.push(
            CustomComponentAstEmitter {
                allocator,
                ast,
                name,
                page: &page,
                line,
                col,
                runtime_base_name,
                context,
            }
            .statement(params),
        );
        statements
    }

    #[allow(clippy::too_many_arguments)]
    fn reusable<'a>(
        allocator: &'a Allocator,
        name: &str,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        builder_param: Option<(&str, OxcVec<'a, Statement<'a>>)>,
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> Statement<'a> {
        let (line, col) = source_metadata.source_position(span.start);
        let page = source_metadata.component_page_path();
        let params = CustomComponentParamExpressions::new(
            allocator,
            arguments,
            component_info,
            builder_param,
            context,
        );
        let ast = &AstBuilder::new(allocator);
        let reuse_id = reuse_id
            .and_then(|argument| argument.as_expression())
            .map(|expression| expression.clone_in(allocator))
            .unwrap_or_else(|| {
                Expression::new_string_literal(SPAN, ast.allocator().alloc_str(name), None, ast)
            });
        let runtime_base_name = component_info
            .map(CustomComponentInfo::runtime_base_name)
            .unwrap_or_else(|| context.custom_component_base_name());
        ReusableCustomComponentAstEmitter {
            allocator,
            ast,
            name,
            page: &page,
            line,
            col,
            runtime_base_name,
            context,
        }
        .statement(params, reuse_id)
    }
}

struct ReusableV2CustomComponentAstEmitter<'a, 'ast, 'b> {
    allocator: &'a Allocator,
    ast: &'ast AstBuilder<'a>,
    name: &'b str,
    page: &'b str,
    line: usize,
    col: usize,
    context: &'b BuildContext,
}

impl<'a, 'ast, 'b> ReusableV2CustomComponentAstEmitter<'a, 'ast, 'b> {
    fn new(
        allocator: &'a Allocator,
        ast: &'ast AstBuilder<'a>,
        name: &'b str,
        page: &'b str,
        line: usize,
        col: usize,
        context: &'b BuildContext,
    ) -> Self {
        Self {
            allocator,
            ast,
            name,
            page,
            line,
            col,
            context,
        }
    }

    fn statement(&self, params: Expression<'a>, reuse_id: Option<&Argument<'_>>) -> Statement<'a> {
        let reuse_id = reuse_id
            .and_then(Argument::as_expression)
            .map(|expression| expression.clone_in(self.allocator))
            .unwrap_or_else(|| {
                Expression::new_string_literal(
                    SPAN,
                    self.ast.allocator().alloc_str(self.name),
                    None,
                    self.ast,
                )
            });
        let options = self.object_expression([
            self.object_property(
                GeneratedViewpuName::COMPONENT_CLASS,
                Expression::new_identifier(
                    SPAN,
                    self.ast.allocator().alloc_str(self.name),
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::GET_PARAMS,
                self.arrow_expression(params),
            ),
            self.object_property(
                GeneratedViewpuName::GET_REUSE_ID,
                self.arrow_expression(reuse_id),
            ),
            self.object_property(
                GeneratedViewpuName::EXTRA_INFO,
                self.source_info_expression(),
            ),
        ]);
        let callee = Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            self.view_context_expression(),
            IdentifierName::new(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(GeneratedViewpuName::REUSE_OR_CREATE_NEW_COMPONENT),
                self.ast,
            ),
            false,
            self.ast,
        ));
        let call = Expression::new_call_expression(
            SPAN,
            callee,
            None,
            OxcVec::from_value_in(options.into(), self.ast),
            false,
            self.ast,
        );
        Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(
                Statement::new_expression_statement(SPAN, call, self.ast),
                self.ast,
            ),
            self.ast,
        )
    }

    fn arrow_expression(&self, body: Expression<'a>) -> Expression<'a> {
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    SPAN,
                    oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(self.ast),
                    None,
                    self.ast,
                ),
                self.ast,
            ),
            None,
            ArrowFunctionBody::from(body),
            self.ast,
        )
    }

    fn source_info_expression(&self) -> Expression<'a> {
        self.object_expression([
            self.object_property(
                GeneratedViewpuName::SOURCE_PAGE,
                Expression::new_string_literal(
                    SPAN,
                    self.ast.allocator().alloc_str(self.page),
                    None,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_LINE,
                Expression::new_numeric_literal(
                    SPAN,
                    self.line as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_COL,
                Expression::new_numeric_literal(
                    SPAN,
                    self.col as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
        ])
    }

    fn view_context_expression(&self) -> Expression<'a> {
        if !self.context.use_parent_context {
            return Expression::new_this_expression(SPAN, self.ast);
        }
        Expression::new_parenthesized_expression(
            SPAN,
            Expression::new_conditional_expression(
                SPAN,
                Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast),
                Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast),
                Expression::new_this_expression(SPAN, self.ast),
                self.ast,
            ),
            self.ast,
        )
    }

    fn object_expression<I>(&self, properties: I) -> Expression<'a>
    where
        I: IntoIterator<Item = ObjectPropertyKind<'a>>,
    {
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in(properties, self.ast),
            self.ast,
        )
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                self.ast.allocator().alloc_str(name),
                self.ast,
            ),
            value,
            false,
            false,
            false,
            self.ast,
        )
    }
}

struct CustomComponentAstEmitter<'a, 'ast, 'b> {
    allocator: &'a Allocator,
    ast: &'ast AstBuilder<'a>,
    name: &'b str,
    page: &'b str,
    line: usize,
    col: usize,
    runtime_base_name: &'static str,
    context: &'b BuildContext,
}

impl<'a, 'ast, 'b> CustomComponentAstEmitter<'a, 'ast, 'b> {
    fn statement(&self, params: CustomComponentParamExpressions<'a>) -> Statement<'a> {
        Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(self.observe_statement(params), self.ast),
            self.ast,
        )
    }

    fn observe_statement(&self, params: CustomComponentParamExpressions<'a>) -> Statement<'a> {
        let callee = self.member_expression(
            self.view_context_expression(),
            &self.context.runtime_names().observe_component_creation2,
        );
        let arrow = self.observe_arrow_function(params);
        let metadata = self.object_expression([self.object_property(
            &self.context.runtime_names().name,
            Expression::new_string_literal(
                SPAN,
                self.ast.allocator().alloc_str(self.name),
                None,
                self.ast,
            ),
        )]);
        self.call_statement(
            callee,
            OxcVec::from_iter_in([Argument::from(arrow), Argument::from(metadata)], self.ast),
        )
    }

    fn observe_arrow_function(
        &self,
        params: CustomComponentParamExpressions<'a>,
    ) -> Expression<'a> {
        let mut statements = OxcVec::new_in(&self.allocator);
        statements.push(self.initial_render_if_statement(params));
        let body = FunctionBody::new(SPAN, OxcVec::new_in(self.ast), statements, self.ast);
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(self.render_callback_formal_parameters(), self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, self.ast)),
            self.ast,
        )
    }

    fn initial_render_if_statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
    ) -> Statement<'a> {
        let mut consequent = OxcVec::new_in(&self.allocator);
        consequent.push(self.component_call_declaration(params.constructor_arguments));
        consequent.push(self.viewpu_create_statement());
        consequent.push(self.params_lambda_declaration(params.lambda));
        consequent.push(self.params_generator_assignment());

        let mut alternate = OxcVec::new_in(&self.allocator);
        alternate.push(self.update_state_vars_statement(params.update));

        Statement::new_if_statement(
            SPAN,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().is_initial_render),
                self.ast,
            ),
            Statement::new_block_statement(SPAN, consequent, self.ast),
            Some(Statement::new_block_statement(SPAN, alternate, self.ast)),
            self.ast,
        )
    }

    fn component_call_declaration(
        &self,
        constructor_arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let mut arguments = OxcVec::new_in(&self.allocator);
        arguments.push(Argument::from(self.view_context_expression()));
        arguments.extend(constructor_arguments);
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().undefined),
            self.ast,
        )));
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().elmt_id),
            self.ast,
        )));
        arguments.push(Argument::from(self.empty_arrow_function()));
        arguments.push(Argument::from(self.source_info_expression()));
        let new_call = Expression::new_new_expression(
            SPAN,
            Expression::new_identifier(SPAN, self.ast.allocator().alloc_str(self.name), self.ast),
            None,
            arguments,
            self.ast,
        );
        self.variable_declaration(&self.context.runtime_names().component_call, new_call)
    }

    fn viewpu_create_statement(&self) -> Statement<'a> {
        self.call_statement(
            self.member_expression(
                Expression::new_identifier(SPAN, self.runtime_base_name, self.ast),
                &self.context.runtime_names().method_create,
            ),
            OxcVec::from_value_in(
                Argument::from(Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().component_call),
                    self.ast,
                )),
                self.ast,
            ),
        )
    }

    fn params_lambda_declaration(&self, lambda: Expression<'a>) -> Statement<'a> {
        let mut statements = OxcVec::new_in(&self.allocator);
        statements.push(Statement::new_return_statement(
            SPAN,
            Some(lambda),
            self.ast,
        ));
        let body = FunctionBody::new(SPAN, OxcVec::new_in(self.ast), statements, self.ast);
        let arrow = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    SPAN,
                    oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(self.ast),
                    None,
                    self.ast,
                ),
                self.ast,
            ),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, self.ast)),
            self.ast,
        );
        self.variable_declaration(&self.context.runtime_names().params_lambda, arrow)
    }

    fn params_generator_assignment(&self) -> Statement<'a> {
        let left = AssignmentTarget::from(
            self.static_member(
                Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().component_call),
                    self.ast,
                ),
                &self.context.runtime_names().params_generator_prefix,
            ),
        );
        let assignment = Expression::new_assignment_expression(
            SPAN,
            AssignmentOperator::Assign,
            left,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().params_lambda),
                self.ast,
            ),
            self.ast,
        );
        Statement::new_expression_statement(SPAN, assignment, self.ast)
    }

    fn update_state_vars_statement(&self, update: Expression<'a>) -> Statement<'a> {
        self.call_statement(
            self.member_expression(
                self.view_context_expression(),
                &self
                    .context
                    .runtime_names()
                    .update_state_vars_of_child_by_elmt_id,
            ),
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        self.ast
                            .allocator()
                            .alloc_str(&self.context.runtime_names().elmt_id),
                        self.ast,
                    )),
                    Argument::from(update),
                ],
                self.ast,
            ),
        )
    }

    fn variable_declaration(&self, name: &str, init: Expression<'a>) -> Statement<'a> {
        let binding = BindingPattern::new_binding_identifier(
            SPAN,
            self.ast.allocator().alloc_str(name),
            self.ast,
        );
        let declarator = VariableDeclarator::new(SPAN, binding, None, Some(init), false, self.ast);
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Let,
            OxcVec::from_value_in(declarator, self.ast),
            false,
            self.ast,
        )
    }

    fn source_info_expression(&self) -> Expression<'a> {
        self.object_expression([
            self.object_property(
                GeneratedViewpuName::SOURCE_PAGE,
                Expression::new_string_literal(
                    SPAN,
                    self.ast.allocator().alloc_str(self.page),
                    None,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_LINE,
                Expression::new_numeric_literal(
                    SPAN,
                    self.line as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_COL,
                Expression::new_numeric_literal(
                    SPAN,
                    self.col as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
        ])
    }

    fn empty_arrow_function(&self) -> Expression<'a> {
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    SPAN,
                    oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(self.ast),
                    None,
                    self.ast,
                ),
                self.ast,
            ),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                FunctionBody::new(
                    SPAN,
                    OxcVec::new_in(self.ast),
                    OxcVec::new_in(self.ast),
                    self.ast,
                ),
                self.ast,
            )),
            self.ast,
        )
    }

    fn view_context_expression(&self) -> Expression<'a> {
        if !self.context.use_parent_context {
            return Expression::new_this_expression(SPAN, self.ast);
        }
        let test = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast);
        let consequent = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast);
        let alternate = Expression::new_this_expression(SPAN, self.ast);
        Expression::new_parenthesized_expression(
            SPAN,
            Expression::new_conditional_expression(SPAN, test, consequent, alternate, self.ast),
            self.ast,
        )
    }

    fn render_callback_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        let mut params = self.formal_parameters([
            self.context.runtime_names().elmt_id.as_str(),
            self.context.runtime_names().is_initial_render.as_str(),
        ]);
        if let Some(parameter) = &self.context.global_builder_parameter {
            let capture_name = format!("__{parameter}__");
            params.items.push(FormalParameter::new(
                SPAN,
                OxcVec::new_in(self.ast),
                BindingPattern::new_binding_identifier(
                    SPAN,
                    self.ast.allocator().alloc_str(parameter),
                    self.ast,
                ),
                None,
                Some(ArenaBox::new_in(
                    Expression::new_identifier(
                        SPAN,
                        self.ast.allocator().alloc_str(&capture_name),
                        self.ast,
                    ),
                    self.ast,
                )),
                false,
                None,
                false,
                false,
                self.ast,
            ));
        }
        params
    }

    fn formal_parameters<const N: usize>(
        &self,
        names: [&str; N],
    ) -> oxc_ast::ast::FormalParameters<'a> {
        let mut params = FormalParameters::new(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(N, self.ast),
            None,
            self.ast,
        );
        for name in names {
            params.items.push(FormalParameter::new(
                SPAN,
                OxcVec::new_in(self.ast),
                BindingPattern::new_binding_identifier(
                    SPAN,
                    self.ast.allocator().alloc_str(name),
                    self.ast,
                ),
                None,
                None,
                false,
                None,
                false,
                false,
                self.ast,
            ));
        }
        params
    }

    fn call_statement(
        &self,
        callee: Expression<'a>,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(SPAN, callee, None, arguments, false, self.ast),
            self.ast,
        )
    }

    fn member_expression(&self, object: Expression<'a>, property: &str) -> Expression<'a> {
        Expression::from(self.static_member(object, property))
    }

    fn static_member(&self, object: Expression<'a>, property: &str) -> MemberExpression<'a> {
        MemberExpression::new_static_member_expression(
            SPAN,
            object,
            IdentifierName::new(SPAN, self.ast.allocator().alloc_str(property), self.ast),
            false,
            self.ast,
        )
    }

    fn object_expression<I>(&self, properties: I) -> Expression<'a>
    where
        I: IntoIterator<Item = ObjectPropertyKind<'a>>,
    {
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in(properties, self.ast),
            self.ast,
        )
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                self.ast.allocator().alloc_str(name),
                self.ast,
            ),
            value,
            false,
            false,
            false,
            self.ast,
        )
    }
}

struct ReusableCustomComponentAstEmitter<'a, 'ast, 'b> {
    allocator: &'a Allocator,
    ast: &'ast AstBuilder<'a>,
    name: &'b str,
    page: &'b str,
    line: usize,
    col: usize,
    runtime_base_name: &'static str,
    context: &'b BuildContext,
}

impl<'a, 'ast, 'b> ReusableCustomComponentAstEmitter<'a, 'ast, 'b> {
    fn statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(self.observe_recycle_statement(params, reuse_id), self.ast),
            self.ast,
        )
    }

    fn observe_recycle_statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        let callee = self.member_expression(
            self.view_context_expression(),
            &self
                .context
                .runtime_names()
                .observe_recycle_component_creation,
        );
        let arrow = self.observe_arrow_function(params, reuse_id.clone_in(self.allocator));
        self.call_statement(
            callee,
            OxcVec::from_iter_in([Argument::from(reuse_id), Argument::from(arrow)], self.ast),
        )
    }

    fn observe_arrow_function(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Expression<'a> {
        let mut statements = OxcVec::new_in(&self.allocator);
        statements.push(
            self.static_call_statement(
                &self.context.runtime_names().view_stack_processor,
                &self.context.runtime_names().start_get_access_recording_for,
                OxcVec::from_value_in(
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        self.ast
                            .allocator()
                            .alloc_str(&self.context.runtime_names().elmt_id),
                        self.ast,
                    )),
                    self.ast,
                ),
            ),
        );
        statements.push(self.initial_render_if_statement(params, reuse_id));
        statements.push(self.static_call_statement(
            &self.context.runtime_names().view_stack_processor,
            &self.context.runtime_names().stop_get_access_recording,
            OxcVec::new_in(self.ast),
        ));
        let body = FunctionBody::new(SPAN, OxcVec::new_in(self.ast), statements, self.ast);
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(self.recycle_formal_parameters(), self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, self.ast)),
            self.ast,
        )
    }

    fn initial_render_if_statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        let lambda_for_recycle = params.lambda.clone_in(self.allocator);
        let lambda_for_generator = params.lambda;
        let mut consequent = OxcVec::new_in(&self.allocator);
        consequent.push(self.component_call_declaration(params.constructor_arguments));
        consequent.push(self.viewpu_create_recycle_statement(lambda_for_recycle, reuse_id));
        consequent.push(self.params_lambda_declaration(lambda_for_generator));
        consequent.push(self.params_generator_assignment());

        let mut alternate = OxcVec::new_in(&self.allocator);
        alternate.push(self.update_state_vars_statement(params.update));

        Statement::new_if_statement(
            SPAN,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().is_initial_render),
                self.ast,
            ),
            Statement::new_block_statement(SPAN, consequent, self.ast),
            Some(Statement::new_block_statement(SPAN, alternate, self.ast)),
            self.ast,
        )
    }

    fn component_call_declaration(
        &self,
        constructor_arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let mut arguments = OxcVec::new_in(&self.allocator);
        arguments.push(Argument::from(self.view_context_expression()));
        arguments.extend(constructor_arguments);
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().undefined),
            self.ast,
        )));
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().elmt_id),
            self.ast,
        )));
        arguments.push(Argument::from(self.empty_arrow_function()));
        arguments.push(Argument::from(self.source_info_expression()));
        let new_call = Expression::new_new_expression(
            SPAN,
            Expression::new_identifier(SPAN, self.ast.allocator().alloc_str(self.name), self.ast),
            None,
            arguments,
            self.ast,
        );
        let init = Expression::new_conditional_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            new_call,
            self.ast,
        );
        self.variable_declaration(&self.context.runtime_names().component_call, init)
    }

    fn viewpu_create_recycle_statement(
        &self,
        lambda: Expression<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        self.static_call_statement(
            self.runtime_base_name,
            &self.context.runtime_names().method_create_recycle,
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        self.ast
                            .allocator()
                            .alloc_str(&self.context.runtime_names().component_call),
                        self.ast,
                    )),
                    Argument::from(self.recycle_node_not_null_expression()),
                    Argument::from(reuse_id),
                    Argument::from(self.recycle_callback(lambda)),
                ],
                self.ast,
            ),
        )
    }

    fn recycle_callback(&self, lambda: Expression<'a>) -> Expression<'a> {
        let mut internal_consequent = OxcVec::new_in(&self.allocator);
        internal_consequent.push(
            self.member_call_statement(
                Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().recycle_node),
                    self.ast,
                ),
                &self.context.runtime_names().about_to_reuse_internal,
                OxcVec::new_in(self.ast),
            ),
        );

        let mut about_to_reuse_consequent = OxcVec::new_in(&self.allocator);
        about_to_reuse_consequent.push(
            self.member_call_statement(
                Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().recycle_node),
                    self.ast,
                ),
                &self.context.runtime_names().about_to_reuse,
                OxcVec::from_value_in(Argument::from(lambda), self.ast),
            ),
        );

        let mut alternate = OxcVec::new_in(&self.allocator);
        alternate.push(Statement::new_if_statement(
            SPAN, // The callback is installed by ViewPU.createRecycle only for a
            // recycled node. Latest process_custom_component.js therefore
            // probes the public hook directly here, after the guarded
            // aboutToReuseInternal branch.
            self.method_function_test(&self.context.runtime_names().about_to_reuse),
            Statement::new_block_statement(SPAN, about_to_reuse_consequent, self.ast),
            None,
            self.ast,
        ));
        alternate.push(
            self.member_call_statement(
                Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().recycle_node),
                    self.ast,
                ),
                &self.context.runtime_names().rerender,
                OxcVec::new_in(self.ast),
            ),
        );

        let mut statements = OxcVec::new_in(&self.allocator);
        statements.push(Statement::new_if_statement(
            SPAN,
            self.recycle_node_method_function_test(
                &self.context.runtime_names().about_to_reuse_internal,
            ),
            Statement::new_block_statement(SPAN, internal_consequent, self.ast),
            Some(Statement::new_block_statement(SPAN, alternate, self.ast)),
            self.ast,
        ));

        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(self.empty_formal_parameters(), self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                FunctionBody::new(SPAN, OxcVec::new_in(self.ast), statements, self.ast),
                self.ast,
            )),
            self.ast,
        )
    }

    fn method_function_test(&self, method: &str) -> Expression<'a> {
        let method_expression = self.member_expression(
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            method,
        );
        let method_for_typeof = method_expression.clone_in(self.allocator);
        let typeof_method = Expression::new_unary_expression(
            SPAN,
            UnaryOperator::Typeof,
            method_for_typeof,
            self.ast,
        );
        let type_check = Expression::new_binary_expression(
            SPAN,
            typeof_method,
            BinaryOperator::StrictEquality,
            Expression::new_string_literal(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().function_type),
                None,
                self.ast,
            ),
            self.ast,
        );
        Expression::new_logical_expression(
            SPAN,
            method_expression,
            LogicalOperator::And,
            type_check,
            self.ast,
        )
    }

    fn recycle_node_method_function_test(&self, method: &str) -> Expression<'a> {
        Expression::new_logical_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            LogicalOperator::And,
            self.typeof_method_function_test(method),
            self.ast,
        )
    }

    fn typeof_method_function_test(&self, method: &str) -> Expression<'a> {
        let method_expression = self.member_expression(
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            method,
        );
        let typeof_method = Expression::new_unary_expression(
            SPAN,
            UnaryOperator::Typeof,
            method_expression,
            self.ast,
        );
        Expression::new_binary_expression(
            SPAN,
            typeof_method,
            BinaryOperator::StrictEquality,
            Expression::new_string_literal(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().function_type),
                None,
                self.ast,
            ),
            self.ast,
        )
    }

    fn recycle_node_not_null_expression(&self) -> Expression<'a> {
        Expression::new_binary_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().recycle_node),
                self.ast,
            ),
            BinaryOperator::StrictInequality,
            Expression::new_null_literal(SPAN, self.ast),
            self.ast,
        )
    }

    fn params_lambda_declaration(&self, lambda: Expression<'a>) -> Statement<'a> {
        let mut statements = OxcVec::new_in(&self.allocator);
        statements.push(Statement::new_return_statement(
            SPAN,
            Some(lambda),
            self.ast,
        ));
        let arrow = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(self.empty_formal_parameters(), self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                FunctionBody::new(SPAN, OxcVec::new_in(self.ast), statements, self.ast),
                self.ast,
            )),
            self.ast,
        );
        self.variable_declaration(&self.context.runtime_names().params_lambda, arrow)
    }

    fn params_generator_assignment(&self) -> Statement<'a> {
        let left = AssignmentTarget::from(
            self.static_member(
                Expression::new_identifier(
                    SPAN,
                    self.ast
                        .allocator()
                        .alloc_str(&self.context.runtime_names().component_call),
                    self.ast,
                ),
                &self.context.runtime_names().params_generator_prefix,
            ),
        );
        let assignment = Expression::new_assignment_expression(
            SPAN,
            AssignmentOperator::Assign,
            left,
            Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().params_lambda),
                self.ast,
            ),
            self.ast,
        );
        Statement::new_expression_statement(SPAN, assignment, self.ast)
    }

    fn update_state_vars_statement(&self, update: Expression<'a>) -> Statement<'a> {
        self.call_statement(
            self.member_expression(
                self.view_context_expression(),
                &self
                    .context
                    .runtime_names()
                    .update_state_vars_of_child_by_elmt_id,
            ),
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        self.ast
                            .allocator()
                            .alloc_str(&self.context.runtime_names().elmt_id),
                        self.ast,
                    )),
                    Argument::from(update),
                ],
                self.ast,
            ),
        )
    }

    fn static_call_statement(
        &self,
        object_name: &str,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.call_statement(
            self.member_expression(
                Expression::new_identifier(
                    SPAN,
                    self.ast.allocator().alloc_str(object_name),
                    self.ast,
                ),
                method_name,
            ),
            arguments,
        )
    }

    fn member_call_statement(
        &self,
        object: Expression<'a>,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.call_statement(self.member_expression(object, method_name), arguments)
    }

    fn variable_declaration(&self, name: &str, init: Expression<'a>) -> Statement<'a> {
        let binding = BindingPattern::new_binding_identifier(
            SPAN,
            self.ast.allocator().alloc_str(name),
            self.ast,
        );
        let declarator = VariableDeclarator::new(SPAN, binding, None, Some(init), false, self.ast);
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Let,
            OxcVec::from_value_in(declarator, self.ast),
            false,
            self.ast,
        )
    }

    fn source_info_expression(&self) -> Expression<'a> {
        self.object_expression([
            self.object_property(
                GeneratedViewpuName::SOURCE_PAGE,
                Expression::new_string_literal(
                    SPAN,
                    self.ast.allocator().alloc_str(self.page),
                    None,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_LINE,
                Expression::new_numeric_literal(
                    SPAN,
                    self.line as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_COL,
                Expression::new_numeric_literal(
                    SPAN,
                    self.col as f64,
                    None,
                    NumberBase::Decimal,
                    self.ast,
                ),
            ),
        ])
    }

    fn empty_arrow_function(&self) -> Expression<'a> {
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(self.empty_formal_parameters(), self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                FunctionBody::new(
                    SPAN,
                    OxcVec::new_in(self.ast),
                    OxcVec::new_in(self.ast),
                    self.ast,
                ),
                self.ast,
            )),
            self.ast,
        )
    }

    fn recycle_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        let mut params = FormalParameters::new(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(3, self.ast),
            None,
            self.ast,
        );
        params
            .items
            .push(self.formal_parameter(&self.context.runtime_names().elmt_id, None));
        params
            .items
            .push(self.formal_parameter(&self.context.runtime_names().is_initial_render, None));
        params.items.push(self.formal_parameter(
            &self.context.runtime_names().recycle_node,
            Some(Expression::new_null_literal(SPAN, self.ast)),
        ));
        params
    }

    fn empty_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        FormalParameters::new(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            OxcVec::new_in(self.ast),
            None,
            self.ast,
        )
    }

    fn formal_parameter(
        &self,
        name: &str,
        initializer: Option<Expression<'a>>,
    ) -> oxc_ast::ast::FormalParameter<'a> {
        FormalParameter::new(
            SPAN,
            OxcVec::new_in(self.ast),
            BindingPattern::new_binding_identifier(
                SPAN,
                self.ast.allocator().alloc_str(name),
                self.ast,
            ),
            None,
            initializer.map(|e| ArenaBox::new_in(e, self.ast)),
            false,
            None,
            false,
            false,
            self.ast,
        )
    }

    fn view_context_expression(&self) -> Expression<'a> {
        if !self.context.use_parent_context {
            return Expression::new_this_expression(SPAN, self.ast);
        }
        let test = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast);
        let consequent = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, self.ast);
        let alternate = Expression::new_this_expression(SPAN, self.ast);
        Expression::new_parenthesized_expression(
            SPAN,
            Expression::new_conditional_expression(SPAN, test, consequent, alternate, self.ast),
            self.ast,
        )
    }

    fn call_statement(
        &self,
        callee: Expression<'a>,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(SPAN, callee, None, arguments, false, self.ast),
            self.ast,
        )
    }

    fn member_expression(&self, object: Expression<'a>, property: &str) -> Expression<'a> {
        Expression::from(self.static_member(object, property))
    }

    fn static_member(&self, object: Expression<'a>, property: &str) -> MemberExpression<'a> {
        MemberExpression::new_static_member_expression(
            SPAN,
            object,
            IdentifierName::new(SPAN, self.ast.allocator().alloc_str(property), self.ast),
            false,
            self.ast,
        )
    }

    fn object_expression<I>(&self, properties: I) -> Expression<'a>
    where
        I: IntoIterator<Item = ObjectPropertyKind<'a>>,
    {
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in(properties, self.ast),
            self.ast,
        )
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                self.ast.allocator().alloc_str(name),
                self.ast,
            ),
            value,
            false,
            false,
            false,
            self.ast,
        )
    }
}

pub(super) struct CustomReuseId;

impl CustomReuseId {
    pub(super) fn from_leaf_attrs<'a>(
        attrs: &'a [LeafAttribute<'a>],
        context: &BuildContext,
    ) -> Option<&'a Argument<'a>> {
        attrs
            .iter()
            .find(|attr| context.is_reuse_id_attribute_name(&attr.name))
            .and_then(|attr| attr.arguments.first().copied())
    }

    pub(super) fn from_component_attrs<'a>(
        component: &'a ArkUIComponentExpression<'a>,
        context: &BuildContext,
    ) -> Option<&'a Argument<'a>> {
        component.chain_expressions.iter().find_map(|chain| {
            let name = ArkuiCallName::chain(chain).ok()?;
            if context.is_reuse_id_attribute_name(&name) {
                chain.arguments.first()
            } else {
                None
            }
        })
    }
}

struct CustomComponentParamExpressions<'a> {
    constructor_arguments: OxcVec<'a, Argument<'a>>,
    lambda: Expression<'a>,
    update: Expression<'a>,
}

impl<'a> CustomComponentParamExpressions<'a> {
    fn new(
        allocator: &'a Allocator,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        builder_param: Option<(&str, OxcVec<'a, Statement<'a>>)>,
        context: &BuildContext,
    ) -> Self {
        let ast = &AstBuilder::new(allocator);
        let builder_param = builder_param.map(|(name, statements)| {
            let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), statements, ast);
            let expression = Expression::new_arrow_function_expression(
                SPAN,
                false,
                None,
                ArenaBox::new_in(
                    FormalParameters::new(
                        SPAN,
                        oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                        OxcVec::new_in(ast),
                        None,
                        ast,
                    ),
                    ast,
                ),
                None,
                ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, ast)),
                ast,
            );
            (name, expression)
        });
        let object_emitter = CustomComponentObjectEmitter::new(allocator, ast);
        let Some(Argument::ObjectExpression(object)) = arguments.first() else {
            if arguments.is_empty() {
                return Self {
                    constructor_arguments: OxcVec::from_value_in(
                        Argument::from(object_emitter.empty_expression(
                            builder_param.as_ref().map(|(name, value)| (*name, value)),
                            component_info,
                            ParamMode::Ctor,
                        )),
                        ast,
                    ),
                    lambda: object_emitter.empty_expression(
                        builder_param.as_ref().map(|(name, value)| (*name, value)),
                        component_info,
                        ParamMode::Lambda,
                    ),
                    update: object_emitter.empty_expression(
                        builder_param.as_ref().map(|(name, value)| (*name, value)),
                        component_info,
                        ParamMode::Update,
                    ),
                };
            }

            let constructor_arguments = OxcVec::from_iter_in(
                arguments
                    .iter()
                    .map(|argument| argument.clone_in(allocator)),
                ast,
            );
            let lambda = Self::lambda_expression_from_arguments(allocator, arguments);
            return Self {
                constructor_arguments,
                lambda,
                update: Expression::new_object_expression(SPAN, OxcVec::new_in(ast), ast),
            };
        };

        Self {
            constructor_arguments: OxcVec::from_value_in(
                Argument::from(object_emitter.expression(
                    object,
                    component_info,
                    ParamMode::Ctor,
                    builder_param.as_ref().map(|(name, value)| (*name, value)),
                    context,
                )),
                ast,
            ),
            lambda: object_emitter.expression(
                object,
                component_info,
                ParamMode::Lambda,
                builder_param.as_ref().map(|(name, value)| (*name, value)),
                context,
            ),
            update: object_emitter.expression(
                object,
                component_info,
                ParamMode::Update,
                builder_param.as_ref().map(|(name, value)| (*name, value)),
                context,
            ),
        }
    }

    fn lambda_expression_from_arguments(
        allocator: &'a Allocator,
        arguments: &[Argument<'_>],
    ) -> Expression<'a> {
        let ast = &AstBuilder::new(allocator);
        let mut expressions = OxcVec::new_in(&allocator);
        for argument in arguments {
            match argument {
                Argument::SpreadElement(spread) => {
                    expressions.push(spread.argument.clone_in(allocator));
                }
                _ => {
                    if let Some(expression) = argument.as_expression() {
                        expressions.push(expression.clone_in(allocator));
                    }
                }
            }
        }
        if expressions.len() == 1 {
            expressions.pop().expect("single expression exists")
        } else {
            Expression::new_sequence_expression(SPAN, expressions, ast)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamMode {
    Ctor,
    Lambda,
    Update,
}

struct CustomComponentObjectEmitter<'a, 'ast> {
    allocator: &'a Allocator,
    ast: &'ast AstBuilder<'a>,
}

impl<'a, 'ast> CustomComponentObjectEmitter<'a, 'ast> {
    fn new(allocator: &'a Allocator, ast: &'ast AstBuilder<'a>) -> Self {
        Self { allocator, ast }
    }

    fn expression(
        &self,
        object: &ObjectExpression<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        builder_param: Option<(&str, &Expression<'a>)>,
        context: &BuildContext,
    ) -> Expression<'a> {
        let mut properties = self.properties(object, component_info, mode, context);
        self.push_builder_param(&mut properties, builder_param, component_info, mode);
        Expression::new_object_expression(SPAN, properties, self.ast)
    }

    fn empty_expression(
        &self,
        builder_param: Option<(&str, &Expression<'a>)>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
    ) -> Expression<'a> {
        let mut properties = OxcVec::new_in(&self.allocator);
        self.push_builder_param(&mut properties, builder_param, component_info, mode);
        Expression::new_object_expression(SPAN, properties, self.ast)
    }

    fn push_builder_param(
        &self,
        properties: &mut OxcVec<'a, ObjectPropertyKind<'a>>,
        builder_param: Option<(&str, &Expression<'a>)>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
    ) {
        let Some((name, value)) = builder_param else {
            return;
        };
        if mode == ParamMode::Update
            && !component_info.is_some_and(|info| info.update_props.contains(name))
        {
            return;
        }
        properties.push(ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                self.ast.allocator().alloc_str(name),
                self.ast,
            ),
            value.clone_in(self.allocator),
            false,
            false,
            false,
            self.ast,
        ));
    }

    fn properties(
        &self,
        object: &ObjectExpression<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        context: &BuildContext,
    ) -> OxcVec<'a, ObjectPropertyKind<'a>> {
        let mut properties = OxcVec::new_in(&self.allocator);
        let mut two_way_callbacks = OxcVec::new_in(&self.allocator);
        for property in &object.properties {
            if let Some(property) = self.property(property, component_info, mode, context) {
                properties.push(property);
            }
            if let Some(callback) = self.two_way_callback(property, component_info, mode) {
                two_way_callbacks.push(callback);
            }
        }
        properties.extend(two_way_callbacks);
        properties
    }

    fn two_way_callback(
        &self,
        property: &ObjectPropertyKind<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
    ) -> Option<ObjectPropertyKind<'a>> {
        if mode != ParamMode::Ctor {
            return None;
        }
        let info =
            component_info.filter(|info| info.generation == CustomComponentGeneration::V2)?;
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        let PropertyKey::StaticIdentifier(key) = &property.key else {
            return None;
        };
        let property_name = key.name.as_str();
        let mut event_name = String::with_capacity(property_name.len() + 1);
        event_name.push('$');
        event_name.push_str(property_name);
        if !info.param_props.contains(property_name) || !info.event_props.contains(&event_name) {
            return None;
        }
        let target = Self::double_non_null_target(&property.value)?;
        if Self::contains_optional_chain(target) {
            return None;
        }
        let target = target.clone_in(self.allocator);
        let assignment_target = match target {
            Expression::Identifier(identifier) => {
                AssignmentTarget::new_assignment_target_identifier(
                    identifier.span,
                    identifier.name,
                    self.ast,
                )
            }
            Expression::StaticMemberExpression(member) => {
                AssignmentTarget::from(MemberExpression::StaticMemberExpression(member))
            }
            _ => return None,
        };
        let value_name = self.allocator.alloc_str("value");
        let assignment = Expression::new_assignment_expression(
            property.span,
            AssignmentOperator::Assign,
            assignment_target,
            Expression::new_identifier(property.span, value_name, self.ast),
            self.ast,
        );
        let body = FunctionBody::new(
            property.span,
            OxcVec::new_in(self.ast),
            OxcVec::from_value_in(
                Statement::new_expression_statement(property.span, assignment, self.ast),
                self.ast,
            ),
            self.ast,
        );
        let mut parameters = FormalParameters::new(
            property.span,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(1, self.ast),
            None,
            self.ast,
        );
        parameters.items.push(FormalParameter::new(
            property.span,
            OxcVec::new_in(self.ast),
            BindingPattern::new_binding_identifier(property.span, value_name, self.ast),
            None,
            None,
            false,
            None,
            false,
            false,
            self.ast,
        ));
        let callback = Expression::new_arrow_function_expression(
            property.span,
            false,
            None,
            ArenaBox::new_in(parameters, self.ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, self.ast)),
            self.ast,
        );
        Some(ObjectPropertyKind::new_object_property(
            property.span,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                property.span,
                self.allocator.alloc_str(&event_name),
                self.ast,
            ),
            callback,
            false,
            false,
            false,
            self.ast,
        ))
    }

    fn double_non_null_target<'node, 'source>(
        expression: &'node Expression<'source>,
    ) -> Option<&'node Expression<'source>> {
        let Expression::TSNonNullExpression(outer) = expression else {
            return None;
        };
        let Expression::TSNonNullExpression(inner) = &outer.expression else {
            return None;
        };
        if matches!(inner.expression, Expression::TSNonNullExpression(_)) {
            return None;
        }
        matches!(
            inner.expression,
            Expression::Identifier(_) | Expression::StaticMemberExpression(_)
        )
        .then_some(&inner.expression)
    }

    fn contains_optional_chain(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::ChainExpression(_) => true,
            Expression::StaticMemberExpression(member) => {
                member.optional || Self::contains_optional_chain(&member.object)
            }
            _ => false,
        }
    }

    fn property(
        &self,
        property: &ObjectPropertyKind<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        context: &BuildContext,
    ) -> Option<ObjectPropertyKind<'a>> {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                let key_name = Self::property_key_name(&property.key);
                if mode == ParamMode::Update
                    && !key_name.as_deref().is_some_and(|name| {
                        component_info.is_some_and(|info| info.update_props.contains(name))
                    })
                {
                    return None;
                }
                let value = self.value_expression(
                    &property.value,
                    key_name.as_deref(),
                    component_info,
                    mode,
                    context,
                );
                Some(ObjectPropertyKind::new_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(self.allocator),
                    value,
                    property.method,
                    property.shorthand,
                    property.computed,
                    self.ast,
                ))
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                if mode == ParamMode::Update {
                    None
                } else {
                    Some(ObjectPropertyKind::new_spread_property(
                        spread.span,
                        spread.argument.clone_in(self.allocator),
                        self.ast,
                    ))
                }
            }
        }
    }

    fn value_expression(
        &self,
        value: &Expression<'_>,
        property_name: Option<&str>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        context: &BuildContext,
    ) -> Expression<'a> {
        if let Some(name) = Self::dollar_reference_name(value) {
            return match mode {
                ParamMode::Ctor => self.backing_member_expression(&name),
                ParamMode::Lambda => self.this_member_expression(&name),
                ParamMode::Update => value.clone_in(self.allocator),
            };
        }
        if mode == ParamMode::Ctor
            && property_name.is_some_and(|name| {
                component_info.is_some_and(|info| info.link_props.contains(name))
            })
        {
            if let Some(name) = Self::this_member_name(value) {
                return self.backing_member_expression(&name);
            }
        }
        if let Some(expression) = RawObjectArgumentTransform::new(context)
            .expression_with_call_args(value, self.allocator)
        {
            return expression;
        }
        value.clone_in(self.allocator)
    }

    fn backing_member_expression(&self, name: &str) -> Expression<'a> {
        let backing = GeneratedViewpuName::backing_field(name);
        self.this_member_expression(&backing)
    }

    fn this_member_expression(&self, name: &str) -> Expression<'a> {
        let member = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_this_expression(SPAN, self.ast),
            IdentifierName::new(SPAN, self.allocator.alloc_str(name), self.ast),
            false,
            self.ast,
        );
        Expression::from(member)
    }

    fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    fn this_member_name(value: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = value else {
            return None;
        };
        matches!(&member.object, Expression::ThisExpression(_))
            .then(|| member.property.name.to_string())
    }

    fn dollar_reference_name(value: &Expression<'_>) -> Option<String> {
        match value {
            Expression::Identifier(identifier) => {
                ParamReferenceName::new(identifier.name.as_str()).backing_property()
            }
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                ParamReferenceName::new(member.property.name.as_str()).backing_property()
            }
            _ => None,
        }
    }
}

struct ParamReferenceName<'a> {
    name: &'a str,
}

impl<'a> ParamReferenceName<'a> {
    const PREFIX: char = '$';

    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn backing_property(&self) -> Option<String> {
        self.name.strip_prefix(Self::PREFIX).map(str::to_string)
    }
}
