use rustc_hash::FxHashSet;

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, ArrowFunctionExpression, Expression, FormalParameter,
    FormalParameterKind, FormalParameters, Statement, VariableDeclarationKind,
};
use oxc_ast::AstBuilder;
use oxc_ast::NONE;
use oxc_span::SPAN;
use oxc_syntax::operator::UnaryOperator;

use crate::transform::viewpu_emit::GeneratedViewpuName;

use super::{
    ArkuiCallName, BuildArgumentEmitter, BuildContext, BuildLoweringError,
    ComponentCallStatementEmitter,
};

pub(super) struct IterationComponent<'a, 'b> {
    component: &'a ArkUIComponentExpression<'b>,
}

impl<'a, 'b> IterationComponent<'a, 'b> {
    pub(super) fn new(component: &'a ArkUIComponentExpression<'b>) -> Self {
        Self { component }
    }

    pub(super) fn is_lazy_list_item(&self) -> bool {
        !self.component.arguments.first().is_some_and(|arg| {
            matches!(
                arg.as_expression(),
                Some(Expression::BooleanLiteral(value)) if !value.value
            )
        })
    }

    pub(super) fn lazy_list_item_creation2_statement<'c>(
        &self,
        ast: AstBuilder<'c>,
        allocator: &'c Allocator,
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Result<Statement<'c>, BuildLoweringError> {
        let mut statements = OxcVec::new_in(&allocator);
        statements.push(IterationAstStatement::static_call(
            ast,
            allocator,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_create,
            ast.vec_from_iter([
                Argument::from(ast.expression_identifier(
                    SPAN,
                    ast.str(&context.runtime_names().deep_render_function),
                )),
                Argument::from(ast.expression_boolean_literal(SPAN, true)),
            ]),
        ));
        let mut has_animation = false;
        for chain in &self.component.chain_expressions {
            let name = ArkuiCallName::chain(chain)?;
            has_animation |= ComponentCallStatementEmitter::is_animation_attribute(&name);
        }
        if has_animation {
            let mut animation_segment = OxcVec::new_in(&allocator);
            for chain in &self.component.chain_expressions {
                let name = ArkuiCallName::chain(chain)?;
                if ComponentCallStatementEmitter::is_animation_attribute(&name) {
                    ComponentCallStatementEmitter::push_animation_segment(
                        &mut statements,
                        &mut animation_segment,
                        ast,
                        allocator,
                        &chain.arguments,
                        context,
                    );
                    continue;
                }
                animation_segment.push(ComponentCallStatementEmitter::attribute(
                    ast,
                    allocator,
                    &context.runtime_names().component_list_item,
                    &name,
                    &chain.arguments,
                    builder_methods,
                    context,
                ));
            }
            ComponentCallStatementEmitter::flush_animation_segment(
                &mut statements,
                &mut animation_segment,
                allocator,
            );
            return Ok(IterationAstStatement::const_arrow_function(
                ast,
                &context.runtime_names().item_creation2,
                IterationFormalParameters::render_callback(ast, context),
                statements,
            ));
        }
        for chain in &self.component.chain_expressions {
            let name = ArkuiCallName::chain(chain)?;
            statements.push(ComponentCallStatementEmitter::attribute(
                ast,
                allocator,
                &context.runtime_names().component_list_item,
                &name,
                &chain.arguments,
                builder_methods,
                context,
            ));
        }
        Ok(IterationAstStatement::const_arrow_function(
            ast,
            &context.runtime_names().item_creation2,
            IterationFormalParameters::render_callback(ast, context),
            statements,
        ))
    }
}

pub(super) struct IterationArgument;

impl IterationArgument {
    pub(super) fn arrow<'a>(argument: &'a Argument<'a>) -> Option<&'a ArrowFunctionExpression<'a>> {
        match argument.as_expression()? {
            Expression::ArrowFunctionExpression(arrow) => Some(arrow),
            Expression::ParenthesizedExpression(parenthesized) => match &parenthesized.expression {
                Expression::ArrowFunctionExpression(arrow) => Some(arrow),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn expression_with_context<'a>(
        argument: &Argument<'_>,
        context: &BuildContext,
        allocator: &'a Allocator,
    ) -> Option<Expression<'a>> {
        let argument = BuildArgumentEmitter::new()
            .with_context(context)
            .argument_with_allocator(argument, allocator);
        argument
            .as_expression()
            .map(|expression| expression.clone_in(allocator))
    }
}

pub(super) struct IterationFormalParameters;

impl IterationFormalParameters {
    pub(super) fn from_names<'a, const N: usize>(
        ast: AstBuilder<'a>,
        names: [&str; N],
    ) -> FormalParameters<'a> {
        ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec_from_iter(names.into_iter().map(|name| {
                ast.formal_parameter(
                    SPAN,
                    ast.vec(),
                    ast.binding_pattern_binding_identifier(SPAN, ast.str(name)),
                    NONE,
                    NONE,
                    false,
                    None,
                    false,
                    false,
                )
            })),
            NONE,
        )
    }

    pub(super) fn render_callback<'a>(
        ast: AstBuilder<'a>,
        context: &BuildContext,
    ) -> FormalParameters<'a> {
        Self::from_names(
            ast,
            [
                context.runtime_names().elmt_id.as_str(),
                context.runtime_names().is_initial_render.as_str(),
            ],
        )
    }

    pub(super) fn for_each_generator<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        item: Option<&FormalParameter<'_>>,
        index: Option<&FormalParameter<'_>>,
        item_binding_name: &str,
    ) -> FormalParameters<'a> {
        let mut items = OxcVec::new_in(&allocator);
        if item.is_some() {
            items.push(Self::for_each_item_parameter(ast, item_binding_name));
            if let Some(index) = index {
                items.push(index.clone_in(allocator));
            }
        }
        ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            items,
            NONE,
        )
    }

    pub(super) fn from_source<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        params: &FormalParameters<'_>,
    ) -> FormalParameters<'a> {
        let items = ast.vec_from_iter(params.items.iter().map(|param| param.clone_in(allocator)));
        let rest = params.rest.as_ref().map(|rest| rest.clone_in(allocator));
        ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            items,
            rest,
        )
    }

    fn for_each_item_parameter<'a>(
        ast: AstBuilder<'a>,
        item_binding_name: &str,
    ) -> FormalParameter<'a> {
        ast.formal_parameter(
            SPAN,
            ast.vec(),
            ast.binding_pattern_binding_identifier(SPAN, ast.str(item_binding_name)),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
        )
    }
}

pub(super) struct IterationAstStatement;

impl IterationAstStatement {
    pub(super) fn const_arrow_function<'a>(
        ast: AstBuilder<'a>,
        name: &str,
        params: FormalParameters<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        Self::const_arrow_function_with_params(ast, name, params, statements)
    }

    pub(super) fn const_arrow_function_with_params<'a>(
        ast: AstBuilder<'a>,
        name: &str,
        params: FormalParameters<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        Self::const_declaration(
            ast,
            name,
            ast.expression_arrow_function(
                SPAN,
                false,
                false,
                NONE,
                params,
                NONE,
                ast.function_body(SPAN, ast.vec(), statements),
            ),
        )
    }

    pub(super) fn item_creation_call<'a>(
        ast: AstBuilder<'a>,
        context: &BuildContext,
    ) -> Statement<'a> {
        let callee =
            ast.expression_identifier(SPAN, ast.str(&context.runtime_names().item_creation));
        let arguments =
            ast.vec_from_iter([
                Argument::from(
                    ast.expression_identifier(SPAN, ast.str(&context.runtime_names().elmt_id)),
                ),
                Argument::from(ast.expression_identifier(
                    SPAN,
                    ast.str(&context.runtime_names().is_initial_render),
                )),
            ]);
        ast.statement_expression(
            SPAN,
            ast.expression_call(SPAN, callee, NONE, arguments, false),
        )
    }

    pub(super) fn lazy_list_item_creation<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
    ) -> Statement<'a> {
        let mut statements = OxcVec::new_in(&allocator);
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().view_stack_processor,
            &context.runtime_names().start_get_access_recording_for,
            ast.vec1(Argument::from(ast.expression_identifier(
                SPAN,
                ast.str(&context.runtime_names().elmt_id),
            ))),
        ));
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_create,
            ast.vec_from_iter([
                Argument::from(ast.expression_identifier(
                    SPAN,
                    ast.str(&context.runtime_names().deep_render_function),
                )),
                Argument::from(ast.expression_boolean_literal(SPAN, true)),
            ]),
        ));
        statements.push(Self::lazy_list_item_not_initial_render(
            ast, allocator, context,
        ));
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().view_stack_processor,
            &context.runtime_names().stop_get_access_recording,
            ast.vec(),
        ));
        Self::const_arrow_function(
            ast,
            &context.runtime_names().item_creation,
            IterationFormalParameters::render_callback(ast, context),
            statements,
        )
    }

    pub(super) fn for_each_update<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
        data: Expression<'a>,
        has_item_param: bool,
        item_gen_uses_index: bool,
        key: Option<ForEachKeyExpression<'a>>,
    ) -> Statement<'a> {
        ForEachUpdateEmitter::new(ast, allocator, context).statement(
            data,
            has_item_param,
            item_gen_uses_index,
            key,
        )
    }

    pub(super) fn lazy_for_each_create<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
        data: Expression<'a>,
        has_id_func: bool,
    ) -> Statement<'a> {
        LazyForEachCreateEmitter::new(ast, allocator, context).statement(data, has_id_func)
    }

    pub(super) fn item_binding_declaration<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        item_param: &FormalParameter<'_>,
        context: &BuildContext,
    ) -> Statement<'a> {
        ItemBindingDeclarationEmitter::new(item_param, &context.runtime_names().item_binding)
            .statement(ast, allocator)
    }

    pub(super) fn member_call<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        object: Expression<'a>,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let property = ast.identifier_name(SPAN, allocator.alloc_str(method_name));
        let member = ast.member_expression_static(SPAN, object, property, false);
        let call = ast.expression_call(SPAN, Expression::from(member), NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }

    pub(super) fn const_declaration<'a>(
        ast: AstBuilder<'a>,
        name: &str,
        init: Expression<'a>,
    ) -> Statement<'a> {
        let binding = ast.binding_pattern_binding_identifier(SPAN, ast.str(name));
        let declarator = ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Const,
            binding,
            NONE,
            Some(init),
            false,
        );
        Statement::from(ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Const,
            ast.vec1(declarator),
            false,
        ))
    }

    pub(super) fn view_context<'a>(ast: AstBuilder<'a>, context: &BuildContext) -> Expression<'a> {
        if !context.use_parent_context {
            return ast.expression_this(SPAN);
        }
        let test = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let consequent = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let alternate = ast.expression_this(SPAN);
        ast.expression_parenthesized(
            SPAN,
            ast.expression_conditional(SPAN, test, consequent, alternate),
        )
    }

    fn lazy_list_item_not_initial_render<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
    ) -> Statement<'a> {
        let test = ast.expression_unary(
            SPAN,
            UnaryOperator::LogicalNot,
            ast.expression_identifier(SPAN, ast.str(&context.runtime_names().is_initial_render)),
        );
        let consequent = ast.statement_block(
            SPAN,
            ast.vec1(Self::static_call(
                ast,
                allocator,
                &context.runtime_names().component_list_item,
                &context.runtime_names().method_pop,
                ast.vec(),
            )),
        );
        ast.statement_if(SPAN, test, consequent, None)
    }

    pub(super) fn static_call<'a>(
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
        object_name: &str,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let object = ast.expression_identifier(SPAN, allocator.alloc_str(object_name));
        let property = ast.identifier_name(SPAN, allocator.alloc_str(method_name));
        let member = ast.member_expression_static(SPAN, object, property, false);
        let call = ast.expression_call(SPAN, Expression::from(member), NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }
}

pub(super) struct ForEachKeyExpression<'a> {
    pub(super) expression: Expression<'a>,
    pub(super) uses_index: bool,
}

struct ForEachUpdateEmitter<'a, 'ctx> {
    ast: AstBuilder<'a>,
    allocator: &'a Allocator,
    context: &'ctx BuildContext,
}

impl<'a, 'ctx> ForEachUpdateEmitter<'a, 'ctx> {
    fn new(ast: AstBuilder<'a>, allocator: &'a Allocator, context: &'ctx BuildContext) -> Self {
        Self {
            ast,
            allocator,
            context,
        }
    }

    fn statement(
        &self,
        data: Expression<'a>,
        has_item_param: bool,
        item_gen_uses_index: bool,
        key: Option<ForEachKeyExpression<'a>>,
    ) -> Statement<'a> {
        let arguments = self.arguments(data, has_item_param, item_gen_uses_index, key);
        IterationAstStatement::member_call(
            self.ast,
            self.allocator,
            IterationAstStatement::view_context(self.ast, self.context),
            &self.context.runtime_names().foreach_update_function,
            arguments,
        )
    }

    fn arguments(
        &self,
        data: Expression<'a>,
        has_item_param: bool,
        item_gen_uses_index: bool,
        key: Option<ForEachKeyExpression<'a>>,
    ) -> OxcVec<'a, Argument<'a>> {
        let mut arguments = OxcVec::new_in(&self.allocator);
        arguments.push(Argument::from(self.ast.expression_identifier(
            SPAN,
            self.ast.str(&self.context.runtime_names().elmt_id),
        )));
        arguments.push(Argument::from(data));
        arguments.push(Argument::from(
            self.ast.expression_identifier(
                SPAN,
                self.ast
                    .str(&self.context.runtime_names().foreach_item_gen_function),
            ),
        ));
        if !has_item_param {
            return arguments;
        }
        if let Some(key) = key {
            arguments.push(Argument::from(key.expression));
            arguments.push(Argument::from(
                self.ast
                    .expression_boolean_literal(SPAN, item_gen_uses_index),
            ));
            arguments.push(Argument::from(
                self.ast.expression_boolean_literal(SPAN, key.uses_index),
            ));
        } else if item_gen_uses_index {
            arguments.push(Argument::from(self.ast.expression_identifier(
                SPAN,
                self.ast.str(&self.context.runtime_names().undefined),
            )));
            arguments.push(Argument::from(
                self.ast.expression_boolean_literal(SPAN, true),
            ));
            arguments.push(Argument::from(
                self.ast.expression_boolean_literal(SPAN, false),
            ));
        }
        arguments
    }
}

struct LazyForEachCreateEmitter<'a, 'ctx> {
    ast: AstBuilder<'a>,
    allocator: &'a Allocator,
    context: &'ctx BuildContext,
}

impl<'a, 'ctx> LazyForEachCreateEmitter<'a, 'ctx> {
    fn new(ast: AstBuilder<'a>, allocator: &'a Allocator, context: &'ctx BuildContext) -> Self {
        Self {
            ast,
            allocator,
            context,
        }
    }

    fn statement(&self, data: Expression<'a>, has_id_func: bool) -> Statement<'a> {
        IterationAstStatement::static_call(
            self.ast,
            self.allocator,
            &self.context.runtime_names().component_lazy_for_each,
            &self.context.runtime_names().method_create,
            self.arguments(data, has_id_func),
        )
    }

    fn arguments(&self, data: Expression<'a>, has_id_func: bool) -> OxcVec<'a, Argument<'a>> {
        let mut arguments = OxcVec::new_in(&self.allocator);
        arguments.push(Argument::from(
            self.ast.expression_string_literal(
                SPAN,
                self.ast
                    .str(&self.context.runtime_names().lazy_foreach_create_version),
                None,
            ),
        ));
        arguments.push(Argument::from(IterationAstStatement::view_context(
            self.ast,
            self.context,
        )));
        arguments.push(Argument::from(data));
        arguments.push(Argument::from(
            self.ast.expression_identifier(
                SPAN,
                self.ast
                    .str(&self.context.runtime_names().lazy_foreach_item_gen_function),
            ),
        ));
        if has_id_func {
            arguments.push(Argument::from(
                self.ast.expression_identifier(
                    SPAN,
                    self.ast
                        .str(&self.context.runtime_names().lazy_foreach_item_id_func),
                ),
            ));
        }
        arguments
    }
}

struct ItemBindingDeclarationEmitter<'a> {
    parameter: &'a FormalParameter<'a>,
    item_binding_name: &'a str,
}

impl<'a> ItemBindingDeclarationEmitter<'a> {
    fn new(parameter: &'a FormalParameter<'a>, item_binding_name: &'a str) -> Self {
        Self {
            parameter,
            item_binding_name,
        }
    }

    fn statement<'b>(&self, ast: AstBuilder<'b>, allocator: &'b Allocator) -> Statement<'b> {
        let declarator = ast.variable_declarator(
            SPAN,
            VariableDeclarationKind::Const,
            self.parameter.pattern.clone_in(allocator),
            self.parameter
                .type_annotation
                .as_ref()
                .map(|ty| ty.clone_in(allocator)),
            Some(ast.expression_identifier(SPAN, ast.str(self.item_binding_name))),
            false,
        );
        Statement::from(ast.declaration_variable(
            SPAN,
            VariableDeclarationKind::Const,
            ast.vec1(declarator),
            false,
        ))
    }
}
