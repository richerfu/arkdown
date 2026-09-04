use rustc_hash::FxHashSet;

use oxc_allocator::{Allocator, ArenaBox, ArenaVec, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, ArrowFunctionBody, ArrowFunctionExpression, BindingPattern,
    Expression, FormalParameter, FormalParameterKind, FormalParameters, FunctionBody,
    IdentifierName, MemberExpression, Statement, VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::{GetSpan, SPAN};
use oxc_syntax::operator::UnaryOperator;

use crate::transform::viewpu_emit::GeneratedViewpuName;

use super::{
    BuildArgumentEmitter, BuildContext, BuildLoweringError, LeafAttribute,
    LeafAttributeStatementEmitter,
};

pub(super) struct IterationComponent<'a, 'b> {
    component: &'a ArkUIComponentExpression<'b>,
}

impl<'a, 'b> IterationComponent<'a, 'b> {
    pub(super) fn new(component: &'a ArkUIComponentExpression<'b>) -> Self {
        Self { component }
    }

    pub(super) fn uses_lazy_creation(&self) -> bool {
        !self.component.arguments.first().is_some_and(|arg| {
            matches!(
                arg.as_expression(),
                Some(Expression::StringLiteral(value)) if value.value == "false"
            )
        })
    }

    pub(super) fn lazy_list_item_creation2_statement<'c>(
        &self,
        ast: &AstBuilder<'c>,
        allocator: &'c Allocator,
        attributes: &[LeafAttribute<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Result<Statement<'c>, BuildLoweringError> {
        let mut statements = OxcVec::new_in(&allocator);
        statements.push(IterationAstStatement::static_call(
            ast,
            allocator,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_create,
            {
                let mut arguments = ArenaVec::from_iter_in(
                    [
                        Argument::from(Expression::new_identifier(
                            SPAN,
                            ast.allocator()
                                .alloc_str(&context.runtime_names().deep_render_function),
                            ast,
                        )),
                        Argument::from(Expression::new_boolean_literal(SPAN, true, ast)),
                    ],
                    ast,
                );
                arguments.extend(self.component.arguments.iter().map(|argument| {
                    BuildArgumentEmitter::new()
                        .with_context(context)
                        .argument_with_allocator(argument, allocator)
                }));
                arguments
            },
        ));
        // `process_component_build.js::processItemComponent` runs the same
        // attribute lowering as an ordinary ArkUI component before it wraps
        // ListItem in the lazy deep-render callback.  Feeding the already
        // expanded attributes through the shared emitter is significant for
        // global/component @Styles, stateStyles, gestures, and animation
        // boundaries; iterating the raw chain leaked calls such as
        // `ListItem.itemStyle()` into the generated module.
        LeafAttributeStatementEmitter::new(
            &context.runtime_names().component_list_item,
            self.component.callee.span(),
            attributes,
            builder_methods,
            context,
        )
        .without_reusable_immutable_guard()
        .push_statements(&mut statements, ast, allocator);
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
        let mut expression = argument.as_expression()?;
        loop {
            match expression {
                Expression::ArrowFunctionExpression(arrow) => return Some(arrow),
                Expression::ParenthesizedExpression(parenthesized) => {
                    expression = &parenthesized.expression;
                }
                _ => return None,
            }
        }
    }

    pub(super) fn expression_with_context<'a>(
        argument: &Argument<'_>,
        context: &BuildContext,
        allocator: &'a Allocator,
    ) -> Option<Expression<'a>> {
        let argument = BuildArgumentEmitter::new()
            .with_context(context)
            .with_nested_raw_object_rewrite()
            // SDK process_component_build.js::createUpdateFunctionStatement
            // reuses the original ForEach data node. Its parent still points
            // at the source `ForEach(...)` identifier call, so
            // processBuildMember does not unwrap a direct `this.items`.
            // Calls nested inside the data expression retain their original
            // parent shape and are still eligible.
            .without_top_level_raw_object_rewrite()
            .argument_with_allocator(argument, allocator);
        argument
            .as_expression()
            .map(|expression| expression.clone_in(allocator))
    }
}

pub(super) struct IterationFormalParameters;

impl IterationFormalParameters {
    pub(super) fn empty<'a>(ast: &AstBuilder<'a>) -> FormalParameters<'a> {
        FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            oxc_allocator::ArenaVec::new_in(ast),
            None,
            ast,
        )
    }

    pub(super) fn from_names<'a, const N: usize>(
        ast: &AstBuilder<'a>,
        names: [&str; N],
    ) -> FormalParameters<'a> {
        FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            oxc_allocator::ArenaVec::from_iter_in(
                names.into_iter().map(|name| {
                    FormalParameter::new(
                        SPAN,
                        oxc_allocator::ArenaVec::new_in(ast),
                        BindingPattern::new_binding_identifier(
                            SPAN,
                            ast.allocator().alloc_str(name),
                            ast,
                        ),
                        None,
                        None,
                        false,
                        None,
                        false,
                        false,
                        ast,
                    )
                }),
                ast,
            ),
            None,
            ast,
        )
    }

    pub(super) fn render_callback<'a>(
        ast: &AstBuilder<'a>,
        context: &BuildContext,
    ) -> FormalParameters<'a> {
        let mut params = Self::from_names(
            ast,
            [
                context.runtime_names().elmt_id.as_str(),
                context.runtime_names().is_initial_render.as_str(),
            ],
        );
        if let Some(parameter) = &context.global_builder_parameter {
            let capture_name = format!("__{parameter}__");
            params.items.push(FormalParameter::new(
                SPAN,
                oxc_allocator::ArenaVec::new_in(ast),
                BindingPattern::new_binding_identifier(
                    SPAN,
                    ast.allocator().alloc_str(parameter),
                    ast,
                ),
                None,
                Some(ArenaBox::new_in(
                    Expression::new_identifier(SPAN, ast.allocator().alloc_str(&capture_name), ast),
                    ast,
                )),
                false,
                None,
                false,
                false,
                ast,
            ));
        }
        params
    }

    pub(super) fn for_each_generator<'a>(
        ast: &AstBuilder<'a>,
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
        FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            items,
            None,
            ast,
        )
    }

    pub(super) fn from_source<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        params: &FormalParameters<'_>,
    ) -> FormalParameters<'a> {
        let items = oxc_allocator::ArenaVec::from_iter_in(
            params.items.iter().map(|param| param.clone_in(allocator)),
            ast,
        );
        let rest = params.rest.as_ref().map(|rest| rest.clone_in(allocator));
        FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            items,
            rest,
            ast,
        )
    }

    fn for_each_item_parameter<'a>(
        ast: &AstBuilder<'a>,
        item_binding_name: &str,
    ) -> FormalParameter<'a> {
        FormalParameter::new(
            SPAN,
            oxc_allocator::ArenaVec::new_in(ast),
            BindingPattern::new_binding_identifier(
                SPAN,
                ast.allocator().alloc_str(item_binding_name),
                ast,
            ),
            None,
            None,
            false,
            None,
            false,
            false,
            ast,
        )
    }
}

pub(super) struct IterationAstStatement;

impl IterationAstStatement {
    pub(super) fn const_arrow_function<'a>(
        ast: &AstBuilder<'a>,
        name: &str,
        params: FormalParameters<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        Self::const_arrow_function_with_params(ast, name, params, statements)
    }

    pub(super) fn const_arrow_function_with_params<'a>(
        ast: &AstBuilder<'a>,
        name: &str,
        params: FormalParameters<'a>,
        statements: OxcVec<'a, Statement<'a>>,
    ) -> Statement<'a> {
        Self::const_declaration(
            ast,
            name,
            Expression::new_arrow_function_expression(
                SPAN,
                false,
                None,
                ArenaBox::new_in(params, ast),
                None,
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                    FunctionBody::new(SPAN, ArenaVec::new_in(ast), statements, ast),
                    ast,
                )),
                ast,
            ),
        )
    }

    pub(super) fn item_creation_call<'a>(
        ast: &AstBuilder<'a>,
        context: &BuildContext,
    ) -> Statement<'a> {
        let callee = Expression::new_identifier(
            SPAN,
            ast.allocator()
                .alloc_str(&context.runtime_names().item_creation),
            ast,
        );
        let mut arguments = ArenaVec::from_iter_in(
            [
                Argument::from(Expression::new_identifier(
                    SPAN,
                    ast.allocator().alloc_str(&context.runtime_names().elmt_id),
                    ast,
                )),
                Argument::from(Expression::new_identifier(
                    SPAN,
                    ast.allocator()
                        .alloc_str(&context.runtime_names().is_initial_render),
                    ast,
                )),
            ],
            ast,
        );
        if let Some(parameter) = &context.global_builder_parameter {
            arguments.push(Argument::from(Expression::new_identifier(
                SPAN,
                ast.allocator().alloc_str(parameter),
                ast,
            )));
        }
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(SPAN, callee, None, arguments, false, ast),
            ast,
        )
    }

    pub(super) fn lazy_list_item_creation<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        component: &ArkUIComponentExpression<'_>,
        context: &BuildContext,
    ) -> Statement<'a> {
        let mut statements = OxcVec::new_in(&allocator);
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().view_stack_processor,
            &context.runtime_names().start_get_access_recording_for,
            ArenaVec::from_value_in(
                Argument::from(Expression::new_identifier(
                    SPAN,
                    ast.allocator().alloc_str(&context.runtime_names().elmt_id),
                    ast,
                )),
                ast,
            ),
        ));
        let mut create_arguments = ArenaVec::from_iter_in(
            [
                Argument::from(Expression::new_identifier(
                    SPAN,
                    ast.allocator()
                        .alloc_str(&context.runtime_names().deep_render_function),
                    ast,
                )),
                Argument::from(Expression::new_boolean_literal(SPAN, true, ast)),
            ],
            ast,
        );
        create_arguments.extend(component.arguments.iter().map(|argument| {
            BuildArgumentEmitter::new()
                .with_context(context)
                .argument_with_allocator(argument, allocator)
        }));
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_create,
            create_arguments,
        ));
        statements.push(Self::lazy_list_item_not_initial_render(
            ast, allocator, context,
        ));
        statements.push(Self::static_call(
            ast,
            allocator,
            &context.runtime_names().view_stack_processor,
            &context.runtime_names().stop_get_access_recording,
            ArenaVec::new_in(ast),
        ));
        Self::const_arrow_function(
            ast,
            &context.runtime_names().item_creation,
            IterationFormalParameters::render_callback(ast, context),
            statements,
        )
    }

    pub(super) fn observed_item_create<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        component_name: &str,
        source_arguments: &[Argument<'_>],
        context: &BuildContext,
    ) -> Statement<'a> {
        let empty_builder = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(IterationFormalParameters::empty(ast), ast),
            None,
            ArrowFunctionBody::new_function_body(
                SPAN,
                ArenaVec::new_in(ast),
                ArenaVec::new_in(ast),
                ast,
            ),
            ast,
        );
        let mut arguments = ArenaVec::from_iter_in(
            [
                Argument::from(empty_builder),
                Argument::from(Expression::new_boolean_literal(SPAN, false, ast)),
            ],
            ast,
        );
        arguments.extend(source_arguments.iter().map(|argument| {
            BuildArgumentEmitter::new()
                .with_context(context)
                .argument_with_allocator(argument, allocator)
        }));
        Self::static_call(
            ast,
            allocator,
            component_name,
            &context.runtime_names().method_create,
            arguments,
        )
    }

    pub(super) fn for_each_update<'a>(
        ast: &AstBuilder<'a>,
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
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
        data: Expression<'a>,
        has_id_func: bool,
    ) -> Statement<'a> {
        LazyForEachCreateEmitter::new(ast, allocator, context).statement(data, has_id_func)
    }

    pub(super) fn item_binding_declaration<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        item_param: &FormalParameter<'_>,
        context: &BuildContext,
    ) -> Statement<'a> {
        ItemBindingDeclarationEmitter::new(item_param, &context.runtime_names().item_binding)
            .statement(ast, allocator)
    }

    pub(super) fn member_call<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        object: Expression<'a>,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let property = IdentifierName::new(SPAN, allocator.alloc_str(method_name), ast);
        let member =
            MemberExpression::new_static_member_expression(SPAN, object, property, false, ast);
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(member),
            None,
            arguments,
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, call, ast)
    }

    pub(super) fn identifier_call<'a>(ast: &AstBuilder<'a>, name: &str) -> Statement<'a> {
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(
                SPAN,
                Expression::new_identifier(SPAN, ast.allocator().alloc_str(name), ast),
                None,
                ArenaVec::new_in(ast),
                false,
                ast,
            ),
            ast,
        )
    }

    pub(super) fn const_declaration<'a>(
        ast: &AstBuilder<'a>,
        name: &str,
        init: Expression<'a>,
    ) -> Statement<'a> {
        let binding =
            BindingPattern::new_binding_identifier(SPAN, ast.allocator().alloc_str(name), ast);
        let declarator = VariableDeclarator::new(SPAN, binding, None, Some(init), false, ast);
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Const,
            ArenaVec::from_value_in(declarator, ast),
            false,
            ast,
        )
    }

    pub(super) fn view_context<'a>(ast: &AstBuilder<'a>, context: &BuildContext) -> Expression<'a> {
        if !context.use_parent_context {
            return Expression::new_this_expression(SPAN, ast);
        }
        let test = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let consequent = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let alternate = Expression::new_this_expression(SPAN, ast);
        Expression::new_parenthesized_expression(
            SPAN,
            Expression::new_conditional_expression(SPAN, test, consequent, alternate, ast),
            ast,
        )
    }

    fn lazy_list_item_not_initial_render<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &BuildContext,
    ) -> Statement<'a> {
        let test = Expression::new_unary_expression(
            SPAN,
            UnaryOperator::LogicalNot,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().is_initial_render),
                ast,
            ),
            ast,
        );
        let consequent = Statement::new_block_statement(
            SPAN,
            ArenaVec::from_value_in(
                Self::static_call(
                    ast,
                    allocator,
                    &context.runtime_names().component_list_item,
                    &context.runtime_names().method_pop,
                    ArenaVec::new_in(ast),
                ),
                ast,
            ),
            ast,
        );
        Statement::new_if_statement(SPAN, test, consequent, None, ast)
    }

    pub(super) fn static_call<'a>(
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
        object_name: &str,
        method_name: &str,
        arguments: OxcVec<'a, Argument<'a>>,
    ) -> Statement<'a> {
        let object = Expression::new_identifier(SPAN, allocator.alloc_str(object_name), ast);
        let property = IdentifierName::new(SPAN, allocator.alloc_str(method_name), ast);
        let member =
            MemberExpression::new_static_member_expression(SPAN, object, property, false, ast);
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(member),
            None,
            arguments,
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, call, ast)
    }
}

pub(super) struct ForEachKeyExpression<'a> {
    pub(super) expression: Expression<'a>,
    pub(super) uses_index: bool,
}

struct ForEachUpdateEmitter<'a, 'ast, 'ctx> {
    ast: &'ast AstBuilder<'a>,
    allocator: &'a Allocator,
    context: &'ctx BuildContext,
}

impl<'a, 'ast, 'ctx> ForEachUpdateEmitter<'a, 'ast, 'ctx> {
    fn new(
        ast: &'ast AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &'ctx BuildContext,
    ) -> Self {
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
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().elmt_id),
            self.ast,
        )));
        arguments.push(Argument::from(data));
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().foreach_item_gen_function),
            self.ast,
        )));
        if !has_item_param {
            return arguments;
        }
        if let Some(key) = key {
            arguments.push(Argument::from(key.expression));
            arguments.push(Argument::from(Expression::new_boolean_literal(
                SPAN,
                item_gen_uses_index,
                self.ast,
            )));
            arguments.push(Argument::from(Expression::new_boolean_literal(
                SPAN,
                key.uses_index,
                self.ast,
            )));
        } else if item_gen_uses_index {
            arguments.push(Argument::from(Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().undefined),
                self.ast,
            )));
            arguments.push(Argument::from(Expression::new_boolean_literal(
                SPAN, true, self.ast,
            )));
            arguments.push(Argument::from(Expression::new_boolean_literal(
                SPAN, false, self.ast,
            )));
        }
        arguments
    }
}

struct LazyForEachCreateEmitter<'a, 'ast, 'ctx> {
    ast: &'ast AstBuilder<'a>,
    allocator: &'a Allocator,
    context: &'ctx BuildContext,
}

impl<'a, 'ast, 'ctx> LazyForEachCreateEmitter<'a, 'ast, 'ctx> {
    fn new(
        ast: &'ast AstBuilder<'a>,
        allocator: &'a Allocator,
        context: &'ctx BuildContext,
    ) -> Self {
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
        arguments.push(Argument::from(Expression::new_string_literal(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().lazy_foreach_create_version),
            None,
            self.ast,
        )));
        // SDK createLazyForEach always passes lexical `this`; the surrounding
        // global builder's conditional parent is only used for observation
        // APIs and lowered child components.
        arguments.push(Argument::from(Expression::new_this_expression(
            SPAN, self.ast,
        )));
        arguments.push(Argument::from(data));
        arguments.push(Argument::from(Expression::new_identifier(
            SPAN,
            self.ast
                .allocator()
                .alloc_str(&self.context.runtime_names().lazy_foreach_item_gen_function),
            self.ast,
        )));
        if has_id_func {
            arguments.push(Argument::from(Expression::new_identifier(
                SPAN,
                self.ast
                    .allocator()
                    .alloc_str(&self.context.runtime_names().lazy_foreach_item_id_func),
                self.ast,
            )));
        }
        arguments
    }
}

struct ItemBindingDeclarationEmitter<'a> {
    parameter: &'a FormalParameter<'a>,
    item_binding_name: String,
}

impl<'a> ItemBindingDeclarationEmitter<'a> {
    fn new(parameter: &'a FormalParameter<'a>, item_binding_name: &str) -> Self {
        let item_binding_name = item_binding_name.to_string();
        Self {
            parameter,
            item_binding_name,
        }
    }

    fn statement<'b>(&self, ast: &AstBuilder<'b>, allocator: &'b Allocator) -> Statement<'b> {
        let declarator = VariableDeclarator::new(
            SPAN,
            self.parameter.pattern.clone_in(allocator),
            None,
            Some(Expression::new_identifier(
                SPAN,
                ast.allocator().alloc_str(&self.item_binding_name),
                ast,
            )),
            false,
            ast,
        );
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Const,
            ArenaVec::from_value_in(declarator, ast),
            false,
            ast,
        )
    }
}
