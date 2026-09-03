use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, BindingPattern, Expression, IdentifierName, MemberExpression, Statement,
    VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::SPAN;
use oxc_syntax::{
    number::NumberBase,
    operator::{BinaryOperator, LogicalOperator},
};

use crate::transform::viewpu_emit::GeneratedViewpuName;

/// Owns every generated `PUV2ViewBase.contextStack` shape required by
/// `@LocalBuilder`. The source compiler uses the same file-scoped stack for
/// the global guard, component render/rerender entry, and local-builder parent
/// recovery, so keeping them together prevents those contracts from drifting.
pub(super) struct LocalBuilderRuntimeEmitter;

impl LocalBuilderRuntimeEmitter {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn parent_statement<'a>(&self, allocator: &'a Allocator) -> Statement<'a> {
        let ast = AstBuilder::new(allocator);
        let context_stack = self.context_stack(&ast);
        let length = Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            context_stack.clone_in(allocator),
            IdentifierName::new(SPAN, "length", &ast),
            false,
            &ast,
        ));
        let test = Expression::new_logical_expression(
            SPAN,
            context_stack.clone_in(allocator),
            LogicalOperator::And,
            length.clone_in(allocator),
            &ast,
        );
        let index = Expression::new_binary_expression(
            SPAN,
            length,
            BinaryOperator::Subtraction,
            Expression::new_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal, &ast),
            &ast,
        );
        let value = Expression::from(MemberExpression::new_computed_member_expression(
            SPAN,
            context_stack,
            index,
            false,
            &ast,
        ));
        let initializer = Expression::new_conditional_expression(
            SPAN,
            test,
            value,
            Expression::new_null_literal(SPAN, &ast),
            &ast,
        );
        Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Const,
            OxcVec::from_value_in(
                VariableDeclarator::new(
                    SPAN,
                    BindingPattern::new_binding_identifier(SPAN, GeneratedViewpuName::PARENT, &ast),
                    None,
                    Some(initializer),
                    false,
                    &ast,
                ),
                &ast,
            ),
            false,
            &ast,
        )
    }

    pub(super) fn push_statement<'a>(&self, allocator: &'a Allocator) -> Statement<'a> {
        self.stack_call_statement(allocator, "push", true)
    }

    pub(super) fn pop_statement<'a>(&self, allocator: &'a Allocator) -> Statement<'a> {
        self.stack_call_statement(allocator, "pop", false)
    }

    pub(super) fn guard_statement<'a>(&self, ast: &AstBuilder<'a>) -> Statement<'a> {
        Statement::new_if_statement(
            SPAN,
            Expression::new_binary_expression(
                SPAN,
                self.context_stack(ast),
                BinaryOperator::StrictEquality,
                Expression::new_identifier(SPAN, GeneratedViewpuName::UNDEFINED, ast),
                ast,
            ),
            Statement::new_block_statement(
                SPAN,
                OxcVec::from_value_in(self.reflect_set_statement(ast), ast),
                ast,
            ),
            None,
            ast,
        )
    }

    fn stack_call_statement<'a>(
        &self,
        allocator: &'a Allocator,
        method: &str,
        pass_this: bool,
    ) -> Statement<'a> {
        let ast = AstBuilder::new(allocator);
        let context_stack = self.context_stack(&ast);
        let callee = Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            context_stack.clone_in(allocator),
            IdentifierName::new(SPAN, allocator.alloc_str(method), &ast),
            false,
            &ast,
        ));
        let arguments = if pass_this {
            OxcVec::from_value_in(
                Argument::from(Expression::new_this_expression(SPAN, &ast)),
                &ast,
            )
        } else {
            OxcVec::new_in(&ast)
        };
        let call = Expression::new_call_expression(SPAN, callee, None, arguments, false, &ast);
        Statement::new_expression_statement(
            SPAN,
            Expression::new_logical_expression(
                SPAN,
                context_stack,
                LogicalOperator::And,
                call,
                &ast,
            ),
            &ast,
        )
    }

    fn context_stack<'a>(&self, ast: &AstBuilder<'a>) -> Expression<'a> {
        Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, "PUV2ViewBase", ast),
            IdentifierName::new(SPAN, "contextStack", ast),
            false,
            ast,
        ))
    }

    fn reflect_set_statement<'a>(&self, ast: &AstBuilder<'a>) -> Statement<'a> {
        let callee = Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, GeneratedViewpuName::REFLECT, ast),
            IdentifierName::new(SPAN, GeneratedViewpuName::SET, ast),
            false,
            ast,
        ));
        let arguments = OxcVec::from_iter_in(
            [
                Argument::from(Expression::new_identifier(SPAN, "PUV2ViewBase", ast)),
                Argument::from(Expression::new_string_literal(
                    SPAN,
                    "contextStack",
                    None,
                    ast,
                )),
                Argument::from(Expression::new_array_expression(
                    SPAN,
                    OxcVec::new_in(ast),
                    ast,
                )),
            ],
            ast,
        );
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(SPAN, callee, None, arguments, false, ast),
            ast,
        )
    }
}
