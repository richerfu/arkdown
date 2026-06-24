use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, AssignmentTarget, Expression, MemberExpression, NumberBase,
    ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    VariableDeclarationKind,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::Span;
use oxc_span::SPAN;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};
use rustc_hash::FxHashSet;

use crate::transform::viewpu_emit::{GeneratedViewpuName, ViewpuStatementAstEmitter};

use super::raw_object::RawObjectArgumentTransform;
use super::{
    ArkuiCallName, BuildContext, BuildSourceMetadata, CommonAttribute,
    ComponentCallStatementEmitter, CustomComponentInfo, LeafAttribute,
};

pub(super) struct CustomComponentStatementEmitter;

impl CustomComponentStatementEmitter {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn common_wrapped<'a>(
        allocator: &'a Allocator,
        name: &str,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        chain_attrs: &[CommonAttribute<'_>],
        builder_methods: &FxHashSet<String>,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> OxcVec<'a, Statement<'a>> {
        let ast = AstBuilder::new(allocator);
        let mut statements = OxcVec::new_in(allocator);
        let mut common_stmts = OxcVec::new_in(allocator);
        common_stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_common,
            &context.runtime_names().method_create,
        ));
        if chain_attrs
            .iter()
            .any(|attr| ComponentCallStatementEmitter::is_animation_attribute(&attr.name))
        {
            let mut animation_segment = OxcVec::new_in(allocator);
            for attr in chain_attrs {
                if ComponentCallStatementEmitter::is_animation_attribute(&attr.name) {
                    ComponentCallStatementEmitter::push_animation_ref_segment(
                        &mut common_stmts,
                        &mut animation_segment,
                        ast,
                        allocator,
                        &attr.arguments,
                        context,
                    );
                    continue;
                }
                animation_segment.push(ComponentCallStatementEmitter::attribute_refs(
                    ast,
                    allocator,
                    &context.runtime_names().component_common,
                    &attr.name,
                    &attr.arguments,
                    builder_methods,
                    context,
                ));
            }
            ComponentCallStatementEmitter::flush_animation_segment(
                &mut common_stmts,
                &mut animation_segment,
                allocator,
            );
        } else {
            for attr in chain_attrs {
                common_stmts.push(ComponentCallStatementEmitter::attribute_refs(
                    ast,
                    allocator,
                    &context.runtime_names().component_common,
                    &attr.name,
                    &attr.arguments,
                    builder_methods,
                    context,
                ));
            }
        }
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                &context.runtime_names().component_common,
                context.runtime_names(),
                common_stmts,
            ),
        );
        let custom_component_stmts = Self::custom(
            allocator,
            name,
            arguments,
            component_info,
            reuse_id,
            span,
            source_metadata,
            context,
        );
        statements.push(ast.statement_block(SPAN, custom_component_stmts));
        statements.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_common,
            &context.runtime_names().method_pop,
        ));
        statements
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn custom<'a>(
        allocator: &'a Allocator,
        name: &str,
        arguments: &[Argument<'_>],
        component_info: Option<&CustomComponentInfo>,
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> OxcVec<'a, Statement<'a>> {
        let mut statements = OxcVec::new_in(allocator);
        if component_info.is_some_and(|info| info.reusable) {
            let ast = AstBuilder::new(allocator);
            let mut recycle_stmts = OxcVec::new_in(allocator);
            recycle_stmts.push(ComponentCallStatementEmitter::empty(
                ast,
                &context.runtime_names().component_recycle,
                &context.runtime_names().method_create,
            ));
            statements.push(
                ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                    ast,
                    context.use_parent_context,
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
        let params =
            CustomComponentParamExpressions::new(allocator, arguments, component_info, context);
        statements.push(
            CustomComponentAstEmitter::new(allocator, name, &page, line, col, context)
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
        reuse_id: Option<&Argument<'_>>,
        span: Span,
        source_metadata: &BuildSourceMetadata<'_>,
        context: &BuildContext,
    ) -> Statement<'a> {
        let (line, col) = source_metadata.source_position(span.start);
        let page = source_metadata.component_page_path();
        let params =
            CustomComponentParamExpressions::new(allocator, arguments, component_info, context);
        let ast = AstBuilder::new(allocator);
        let reuse_id = reuse_id
            .and_then(|argument| argument.as_expression())
            .map(|expression| expression.clone_in(allocator))
            .unwrap_or_else(|| ast.expression_string_literal(SPAN, ast.str(name), None));
        ReusableCustomComponentAstEmitter::new(allocator, name, &page, line, col, context)
            .statement(params, reuse_id)
    }
}

struct CustomComponentAstEmitter<'a, 'b> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
    name: &'b str,
    page: &'b str,
    line: usize,
    col: usize,
    context: &'b BuildContext,
}

impl<'a, 'b> CustomComponentAstEmitter<'a, 'b> {
    fn new(
        allocator: &'a Allocator,
        name: &'b str,
        page: &'b str,
        line: usize,
        col: usize,
        context: &'b BuildContext,
    ) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
            name,
            page,
            line,
            col,
            context,
        }
    }

    fn statement(&self, params: CustomComponentParamExpressions<'a>) -> Statement<'a> {
        self.ast
            .statement_block(SPAN, self.ast.vec1(self.observe_statement(params)))
    }

    fn observe_statement(&self, params: CustomComponentParamExpressions<'a>) -> Statement<'a> {
        let callee = self.member_expression(
            self.view_context_expression(),
            &self.context.runtime_names().observe_component_creation2,
        );
        let arrow = self.observe_arrow_function(params);
        let metadata = self.object_expression([self.object_property(
            &self.context.runtime_names().name,
            self.ast
                .expression_string_literal(SPAN, self.ast.str(self.name), None),
        )]);
        self.call_statement(
            callee,
            self.ast
                .vec_from_iter([Argument::from(arrow), Argument::from(metadata)]),
        )
    }

    fn observe_arrow_function(
        &self,
        params: CustomComponentParamExpressions<'a>,
    ) -> Expression<'a> {
        let mut statements = OxcVec::new_in(self.allocator);
        statements.push(self.initial_render_if_statement(params));
        let body = self.ast.function_body(SPAN, self.ast.vec(), statements);
        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.render_callback_formal_parameters(),
            NONE,
            body,
        )
    }

    fn initial_render_if_statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
    ) -> Statement<'a> {
        let mut consequent = OxcVec::new_in(self.allocator);
        consequent.push(self.component_call_declaration(params.constructor_arguments));
        consequent.push(self.viewpu_create_statement());
        consequent.push(self.params_lambda_declaration(params.lambda));
        consequent.push(self.params_generator_assignment());

        let mut alternate = OxcVec::new_in(self.allocator);
        alternate.push(self.update_state_vars_statement(params.update));

        self.ast.statement_if(
            SPAN,
            self.ast.expression_identifier(
                SPAN,
                self.ast
                    .str(&self.context.runtime_names().is_initial_render),
            ),
            self.ast.statement_block(SPAN, consequent),
            Some(self.ast.statement_block(SPAN, alternate)),
        )
    }

    fn component_call_declaration(
        &self,
        constructor_arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let mut arguments = OxcVec::new_in(self.allocator);
        arguments.push(Argument::from(self.view_context_expression()));
        arguments.extend(constructor_arguments);
        arguments.push(Argument::from(self.ast.expression_identifier(
            SPAN,
            self.ast.str(&self.context.runtime_names().undefined),
        )));
        arguments.push(Argument::from(self.ast.expression_identifier(
            SPAN,
            self.ast.str(&self.context.runtime_names().elmt_id),
        )));
        arguments.push(Argument::from(self.empty_arrow_function()));
        arguments.push(Argument::from(self.source_info_expression()));
        let new_call = self.ast.expression_new(
            SPAN,
            self.ast
                .expression_identifier(SPAN, self.ast.str(self.name)),
            NONE,
            arguments,
        );
        self.variable_declaration(&self.context.runtime_names().component_call, new_call)
    }

    fn viewpu_create_statement(&self) -> Statement<'a> {
        self.call_statement(
            self.member_expression(
                self.ast
                    .expression_identifier(SPAN, GeneratedViewpuName::VIEWPU_BASE),
                &self.context.runtime_names().method_create,
            ),
            self.ast.vec1(Argument::from(self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().component_call),
            ))),
        )
    }

    fn params_lambda_declaration(&self, lambda: Expression<'a>) -> Statement<'a> {
        let mut statements = OxcVec::new_in(self.allocator);
        statements.push(self.ast.statement_return(SPAN, Some(lambda)));
        let body = self.ast.function_body(SPAN, self.ast.vec(), statements);
        let arrow = self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.ast.formal_parameters(
                SPAN,
                oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                self.ast.vec(),
                NONE,
            ),
            NONE,
            body,
        );
        self.variable_declaration(&self.context.runtime_names().params_lambda, arrow)
    }

    fn params_generator_assignment(&self) -> Statement<'a> {
        let left = AssignmentTarget::from(self.static_member(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().component_call),
            ),
            &self.context.runtime_names().params_generator_prefix,
        ));
        let assignment = self.ast.expression_assignment(
            SPAN,
            AssignmentOperator::Assign,
            left,
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().params_lambda),
            ),
        );
        self.ast.statement_expression(SPAN, assignment)
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
            self.ast.vec_from_iter([
                Argument::from(self.ast.expression_identifier(
                    SPAN,
                    self.ast.str(&self.context.runtime_names().elmt_id),
                )),
                Argument::from(update),
            ]),
        )
    }

    fn variable_declaration(&self, name: &str, init: Expression<'a>) -> Statement<'a> {
        let binding = self
            .ast
            .binding_pattern_binding_identifier(SPAN, self.ast.str(name));
        let declarator = self.ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Let,
            binding,
            NONE,
            Some(init),
            false,
        );
        Statement::from(self.ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Let,
            self.ast.vec1(declarator),
            false,
        ))
    }

    fn source_info_expression(&self) -> Expression<'a> {
        self.object_expression([
            self.object_property(
                GeneratedViewpuName::SOURCE_PAGE,
                self.ast
                    .expression_string_literal(SPAN, self.ast.str(self.page), None),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_LINE,
                self.ast.expression_numeric_literal(
                    SPAN,
                    self.line as f64,
                    None,
                    NumberBase::Decimal,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_COL,
                self.ast.expression_numeric_literal(
                    SPAN,
                    self.col as f64,
                    None,
                    NumberBase::Decimal,
                ),
            ),
        ])
    }

    fn empty_arrow_function(&self) -> Expression<'a> {
        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.ast.formal_parameters(
                SPAN,
                oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                self.ast.vec(),
                NONE,
            ),
            NONE,
            self.ast.function_body(SPAN, self.ast.vec(), self.ast.vec()),
        )
    }

    fn view_context_expression(&self) -> Expression<'a> {
        if !self.context.use_parent_context {
            return self.ast.expression_this(SPAN);
        }
        let test = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let consequent = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let alternate = self.ast.expression_this(SPAN);
        self.ast.expression_parenthesized(
            SPAN,
            self.ast
                .expression_conditional(SPAN, test, consequent, alternate),
        )
    }

    fn render_callback_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        self.formal_parameters([
            self.context.runtime_names().elmt_id.as_str(),
            self.context.runtime_names().is_initial_render.as_str(),
        ])
    }

    fn formal_parameters<const N: usize>(
        &self,
        names: [&str; N],
    ) -> oxc_ast::ast::FormalParameters<'a> {
        let mut params = self.ast.formal_parameters(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            self.ast.vec_with_capacity(N),
            NONE,
        );
        for name in names {
            params.items.push(
                self.ast.formal_parameter(
                    SPAN,
                    self.ast.vec(),
                    self.ast
                        .binding_pattern_binding_identifier(SPAN, self.ast.str(name)),
                    NONE,
                    NONE,
                    false,
                    None,
                    false,
                    false,
                ),
            );
        }
        params
    }

    fn call_statement(
        &self,
        callee: Expression<'a>,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.ast.statement_expression(
            SPAN,
            self.ast
                .expression_call(SPAN, callee, NONE, arguments, false),
        )
    }

    fn member_expression(&self, object: Expression<'a>, property: &str) -> Expression<'a> {
        Expression::from(self.static_member(object, property))
    }

    fn static_member(&self, object: Expression<'a>, property: &str) -> MemberExpression<'a> {
        self.ast.member_expression_static(
            SPAN,
            object,
            self.ast.identifier_name(SPAN, self.ast.str(property)),
            false,
        )
    }

    fn object_expression<I>(&self, properties: I) -> Expression<'a>
    where
        I: IntoIterator<Item = ObjectPropertyKind<'a>>,
    {
        self.ast
            .expression_object(SPAN, self.ast.vec_from_iter(properties))
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        self.ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            self.ast
                .property_key_static_identifier(SPAN, self.ast.str(name)),
            value,
            false,
            false,
            false,
        )
    }
}

struct ReusableCustomComponentAstEmitter<'a, 'b> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
    name: &'b str,
    page: &'b str,
    line: usize,
    col: usize,
    context: &'b BuildContext,
}

impl<'a, 'b> ReusableCustomComponentAstEmitter<'a, 'b> {
    fn new(
        allocator: &'a Allocator,
        name: &'b str,
        page: &'b str,
        line: usize,
        col: usize,
        context: &'b BuildContext,
    ) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
            name,
            page,
            line,
            col,
            context,
        }
    }

    fn statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        self.ast.statement_block(
            SPAN,
            self.ast
                .vec1(self.observe_recycle_statement(params, reuse_id)),
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
            self.ast
                .vec_from_iter([Argument::from(reuse_id), Argument::from(arrow)]),
        )
    }

    fn observe_arrow_function(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Expression<'a> {
        let mut statements = OxcVec::new_in(self.allocator);
        statements.push(self.static_call_statement(
            &self.context.runtime_names().view_stack_processor,
            &self.context.runtime_names().start_get_access_recording_for,
            self.ast.vec1(Argument::from(self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().elmt_id),
            ))),
        ));
        statements.push(self.initial_render_if_statement(params, reuse_id));
        statements.push(self.static_call_statement(
            &self.context.runtime_names().view_stack_processor,
            &self.context.runtime_names().stop_get_access_recording,
            self.ast.vec(),
        ));
        let body = self.ast.function_body(SPAN, self.ast.vec(), statements);
        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.recycle_formal_parameters(),
            NONE,
            body,
        )
    }

    fn initial_render_if_statement(
        &self,
        params: CustomComponentParamExpressions<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        let lambda_for_recycle = params.lambda.clone_in(self.allocator);
        let lambda_for_generator = params.lambda;
        let mut consequent = OxcVec::new_in(self.allocator);
        consequent.push(self.component_call_declaration(params.constructor_arguments));
        consequent.push(self.viewpu_create_recycle_statement(lambda_for_recycle, reuse_id));
        consequent.push(self.params_lambda_declaration(lambda_for_generator));
        consequent.push(self.params_generator_assignment());

        let mut alternate = OxcVec::new_in(self.allocator);
        alternate.push(self.update_state_vars_statement(params.update));

        self.ast.statement_if(
            SPAN,
            self.ast.expression_identifier(
                SPAN,
                self.ast
                    .str(&self.context.runtime_names().is_initial_render),
            ),
            self.ast.statement_block(SPAN, consequent),
            Some(self.ast.statement_block(SPAN, alternate)),
        )
    }

    fn component_call_declaration(
        &self,
        constructor_arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let mut arguments = OxcVec::new_in(self.allocator);
        arguments.push(Argument::from(self.view_context_expression()));
        arguments.extend(constructor_arguments);
        arguments.push(Argument::from(self.ast.expression_identifier(
            SPAN,
            self.ast.str(&self.context.runtime_names().undefined),
        )));
        arguments.push(Argument::from(self.ast.expression_identifier(
            SPAN,
            self.ast.str(&self.context.runtime_names().elmt_id),
        )));
        arguments.push(Argument::from(self.empty_arrow_function()));
        arguments.push(Argument::from(self.source_info_expression()));
        let new_call = self.ast.expression_new(
            SPAN,
            self.ast
                .expression_identifier(SPAN, self.ast.str(self.name)),
            NONE,
            arguments,
        );
        let init = self.ast.expression_conditional(
            SPAN,
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            new_call,
        );
        self.variable_declaration(&self.context.runtime_names().component_call, init)
    }

    fn viewpu_create_recycle_statement(
        &self,
        lambda: Expression<'a>,
        reuse_id: Expression<'a>,
    ) -> Statement<'a> {
        self.static_call_statement(
            GeneratedViewpuName::VIEWPU_BASE,
            &self.context.runtime_names().method_create_recycle,
            self.ast.vec_from_iter([
                Argument::from(self.ast.expression_identifier(
                    SPAN,
                    self.ast.str(&self.context.runtime_names().component_call),
                )),
                Argument::from(self.recycle_node_not_null_expression()),
                Argument::from(reuse_id),
                Argument::from(self.recycle_callback(lambda)),
            ]),
        )
    }

    fn recycle_callback(&self, lambda: Expression<'a>) -> Expression<'a> {
        let mut internal_consequent = OxcVec::new_in(self.allocator);
        internal_consequent.push(self.member_call_statement(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            &self.context.runtime_names().about_to_reuse_internal,
            self.ast.vec(),
        ));

        let mut about_to_reuse_consequent = OxcVec::new_in(self.allocator);
        about_to_reuse_consequent.push(self.member_call_statement(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            &self.context.runtime_names().about_to_reuse,
            self.ast.vec1(Argument::from(lambda)),
        ));

        let mut alternate = OxcVec::new_in(self.allocator);
        alternate.push(self.ast.statement_if(
            SPAN,
            self.method_function_test(&self.context.runtime_names().about_to_reuse),
            self.ast.statement_block(SPAN, about_to_reuse_consequent),
            None,
        ));
        alternate.push(self.member_call_statement(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            &self.context.runtime_names().rerender,
            self.ast.vec(),
        ));

        let mut statements = OxcVec::new_in(self.allocator);
        statements.push(self.ast.statement_if(
            SPAN,
            self.method_function_test(&self.context.runtime_names().about_to_reuse_internal),
            self.ast.statement_block(SPAN, internal_consequent),
            Some(self.ast.statement_block(SPAN, alternate)),
        ));

        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.empty_formal_parameters(),
            NONE,
            self.ast.function_body(SPAN, self.ast.vec(), statements),
        )
    }

    fn method_function_test(&self, method: &str) -> Expression<'a> {
        let method_expression = self.member_expression(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            method,
        );
        let typeof_method =
            self.ast
                .expression_unary(SPAN, UnaryOperator::Typeof, method_expression);
        let type_check = self.ast.expression_binary(
            SPAN,
            typeof_method,
            BinaryOperator::StrictEquality,
            self.ast.expression_string_literal(
                SPAN,
                self.ast.str(&self.context.runtime_names().function_type),
                None,
            ),
        );
        self.ast.expression_logical(
            SPAN,
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            LogicalOperator::And,
            type_check,
        )
    }

    fn recycle_node_not_null_expression(&self) -> Expression<'a> {
        self.ast.expression_binary(
            SPAN,
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().recycle_node),
            ),
            BinaryOperator::StrictInequality,
            self.ast.expression_null_literal(SPAN),
        )
    }

    fn params_lambda_declaration(&self, lambda: Expression<'a>) -> Statement<'a> {
        let mut statements = OxcVec::new_in(self.allocator);
        statements.push(self.ast.statement_return(SPAN, Some(lambda)));
        let arrow = self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.empty_formal_parameters(),
            NONE,
            self.ast.function_body(SPAN, self.ast.vec(), statements),
        );
        self.variable_declaration(&self.context.runtime_names().params_lambda, arrow)
    }

    fn params_generator_assignment(&self) -> Statement<'a> {
        let left = AssignmentTarget::from(self.static_member(
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().component_call),
            ),
            &self.context.runtime_names().params_generator_prefix,
        ));
        let assignment = self.ast.expression_assignment(
            SPAN,
            AssignmentOperator::Assign,
            left,
            self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().params_lambda),
            ),
        );
        self.ast.statement_expression(SPAN, assignment)
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
            self.ast.vec_from_iter([
                Argument::from(self.ast.expression_identifier(
                    SPAN,
                    self.ast.str(&self.context.runtime_names().elmt_id),
                )),
                Argument::from(update),
            ]),
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
                self.ast
                    .expression_identifier(SPAN, self.ast.str(object_name)),
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
        let binding = self
            .ast
            .binding_pattern_binding_identifier(SPAN, self.ast.str(name));
        let declarator = self.ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Let,
            binding,
            NONE,
            Some(init),
            false,
        );
        Statement::from(self.ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Let,
            self.ast.vec1(declarator),
            false,
        ))
    }

    fn source_info_expression(&self) -> Expression<'a> {
        self.object_expression([
            self.object_property(
                GeneratedViewpuName::SOURCE_PAGE,
                self.ast
                    .expression_string_literal(SPAN, self.ast.str(self.page), None),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_LINE,
                self.ast.expression_numeric_literal(
                    SPAN,
                    self.line as f64,
                    None,
                    NumberBase::Decimal,
                ),
            ),
            self.object_property(
                GeneratedViewpuName::SOURCE_COL,
                self.ast.expression_numeric_literal(
                    SPAN,
                    self.col as f64,
                    None,
                    NumberBase::Decimal,
                ),
            ),
        ])
    }

    fn empty_arrow_function(&self) -> Expression<'a> {
        self.ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            self.empty_formal_parameters(),
            NONE,
            self.ast.function_body(SPAN, self.ast.vec(), self.ast.vec()),
        )
    }

    fn recycle_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        let mut params = self.ast.formal_parameters(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            self.ast.vec_with_capacity(3),
            NONE,
        );
        params
            .items
            .push(self.formal_parameter(&self.context.runtime_names().elmt_id, None));
        params
            .items
            .push(self.formal_parameter(&self.context.runtime_names().is_initial_render, None));
        params.items.push(self.formal_parameter(
            &self.context.runtime_names().recycle_node,
            Some(self.ast.expression_null_literal(SPAN)),
        ));
        params
    }

    fn empty_formal_parameters(&self) -> oxc_ast::ast::FormalParameters<'a> {
        self.ast.formal_parameters(
            SPAN,
            oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
            self.ast.vec(),
            NONE,
        )
    }

    fn formal_parameter(
        &self,
        name: &str,
        initializer: Option<Expression<'a>>,
    ) -> oxc_ast::ast::FormalParameter<'a> {
        self.ast.formal_parameter(
            SPAN,
            self.ast.vec(),
            self.ast
                .binding_pattern_binding_identifier(SPAN, self.ast.str(name)),
            NONE,
            initializer,
            false,
            None,
            false,
            false,
        )
    }

    fn view_context_expression(&self) -> Expression<'a> {
        if !self.context.use_parent_context {
            return self.ast.expression_this(SPAN);
        }
        let test = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let consequent = self
            .ast
            .expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let alternate = self.ast.expression_this(SPAN);
        self.ast.expression_parenthesized(
            SPAN,
            self.ast
                .expression_conditional(SPAN, test, consequent, alternate),
        )
    }

    fn call_statement(
        &self,
        callee: Expression<'a>,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        self.ast.statement_expression(
            SPAN,
            self.ast
                .expression_call(SPAN, callee, NONE, arguments, false),
        )
    }

    fn member_expression(&self, object: Expression<'a>, property: &str) -> Expression<'a> {
        Expression::from(self.static_member(object, property))
    }

    fn static_member(&self, object: Expression<'a>, property: &str) -> MemberExpression<'a> {
        self.ast.member_expression_static(
            SPAN,
            object,
            self.ast.identifier_name(SPAN, self.ast.str(property)),
            false,
        )
    }

    fn object_expression<I>(&self, properties: I) -> Expression<'a>
    where
        I: IntoIterator<Item = ObjectPropertyKind<'a>>,
    {
        self.ast
            .expression_object(SPAN, self.ast.vec_from_iter(properties))
    }

    fn object_property(&self, name: &str, value: Expression<'a>) -> ObjectPropertyKind<'a> {
        self.ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            self.ast
                .property_key_static_identifier(SPAN, self.ast.str(name)),
            value,
            false,
            false,
            false,
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
        context: &BuildContext,
    ) -> Self {
        let ast = AstBuilder::new(allocator);
        let empty_object = || ast.expression_object(SPAN, ast.vec());
        let Some(Argument::ObjectExpression(object)) = arguments.first() else {
            if arguments.is_empty() {
                return Self {
                    constructor_arguments: ast.vec1(Argument::from(empty_object())),
                    lambda: empty_object(),
                    update: empty_object(),
                };
            }

            let constructor_arguments = ast.vec_from_iter(
                arguments
                    .iter()
                    .map(|argument| argument.clone_in(allocator)),
            );
            let lambda = Self::lambda_expression_from_arguments(allocator, arguments);
            return Self {
                constructor_arguments,
                lambda,
                update: empty_object(),
            };
        };

        let object_emitter = CustomComponentObjectEmitter::new(allocator);
        Self {
            constructor_arguments: ast.vec1(Argument::from(object_emitter.expression(
                object,
                component_info,
                ParamMode::Ctor,
                context,
            ))),
            lambda: object_emitter.expression(object, component_info, ParamMode::Lambda, context),
            update: object_emitter.expression(object, component_info, ParamMode::Update, context),
        }
    }

    fn lambda_expression_from_arguments(
        allocator: &'a Allocator,
        arguments: &[Argument<'_>],
    ) -> Expression<'a> {
        let ast = AstBuilder::new(allocator);
        let mut expressions = OxcVec::new_in(allocator);
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
            ast.expression_sequence(SPAN, expressions)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamMode {
    Ctor,
    Lambda,
    Update,
}

struct CustomComponentObjectEmitter<'a> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
}

impl<'a> CustomComponentObjectEmitter<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
        }
    }

    fn expression(
        &self,
        object: &ObjectExpression<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        context: &BuildContext,
    ) -> Expression<'a> {
        let properties = self.properties(object, component_info, mode, context);
        self.ast.expression_object(SPAN, properties)
    }

    fn properties(
        &self,
        object: &ObjectExpression<'_>,
        component_info: Option<&CustomComponentInfo>,
        mode: ParamMode,
        context: &BuildContext,
    ) -> OxcVec<'a, ObjectPropertyKind<'a>> {
        let mut properties = OxcVec::new_in(self.allocator);
        for property in &object.properties {
            if let Some(property) = self.property(property, component_info, mode, context) {
                properties.push(property);
            }
        }
        properties
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
                Some(self.ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(self.allocator),
                    value,
                    property.method,
                    property.shorthand,
                    property.computed,
                ))
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                if mode == ParamMode::Update {
                    None
                } else {
                    Some(self.ast.object_property_kind_spread_property(
                        spread.span,
                        spread.argument.clone_in(self.allocator),
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
        if matches!(mode, ParamMode::Ctor | ParamMode::Lambda) {
            if let Some(expression) = RawObjectArgumentTransform::new(context)
                .expression_with_call_args(value, self.allocator)
            {
                return expression;
            }
        }
        value.clone_in(self.allocator)
    }

    fn backing_member_expression(&self, name: &str) -> Expression<'a> {
        let backing = GeneratedViewpuName::backing_field(name);
        self.this_member_expression(&backing)
    }

    fn this_member_expression(&self, name: &str) -> Expression<'a> {
        let member = self.ast.member_expression_static(
            SPAN,
            self.ast.expression_this(SPAN),
            self.ast
                .identifier_name(SPAN, self.allocator.alloc_str(name)),
            false,
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
