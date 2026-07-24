use oxc_allocator::{Allocator, CloneIn, TakeIn};
use oxc_ast::ast::{Argument, CallExpression, ChainElement, Expression, MemberExpression};
use oxc_ast::{AstBuilder, NONE};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_span::SPAN;

use super::BuildContext;
use crate::transform::viewpu_emit::GeneratedViewpuName;

pub(super) struct RawObjectArgumentTransform<'ctx> {
    context: &'ctx BuildContext,
}

impl<'ctx> RawObjectArgumentTransform<'ctx> {
    pub(super) fn new(context: &'ctx BuildContext) -> Self {
        Self { context }
    }

    pub(super) fn expression_with_call_args<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        RawObjectExpressionRewrite::new(expression, self.context).expression(allocator)
    }

    pub(super) fn owned_expression_with_call_args<'b>(
        &self,
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        RawObjectExpressionRewrite::owned_expression(expression, self.context, allocator)
    }

    pub(super) fn argument<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Option<Argument<'b>> {
        argument
            .as_expression()
            .and_then(|expression| self.argument_expression(expression, allocator))
            .map(Argument::from)
    }

    fn argument_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        RawObjectExpression::new(expression, self.context)
            .is_raw_object_field()
            .then(|| {
                RawObjectCallEmitter::new(allocator, self.context)
                    .wrap(expression.clone_in(allocator))
            })
    }
}

struct RawObjectExpressionRewrite<'a> {
    expression: &'a Expression<'a>,
    context: &'a BuildContext,
}

impl<'a> RawObjectExpressionRewrite<'a> {
    fn new(expression: &'a Expression<'a>, context: &'a BuildContext) -> Self {
        Self {
            expression,
            context,
        }
    }

    fn expression<'b>(&self, allocator: &'b Allocator) -> Option<Expression<'b>> {
        if !RawObjectArgumentDetector::new(self.context).contains_expression(self.expression) {
            return None;
        }
        let mut expression = self.expression.clone_in(allocator);
        let mut mutator = RawObjectArgumentMutator::new(allocator, self.context);
        mutator.visit_expression(&mut expression);
        debug_assert!(mutator.changed);
        Some(expression)
    }

    fn owned_expression<'b>(
        mut expression: Expression<'b>,
        context: &BuildContext,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let mut mutator = RawObjectArgumentMutator::new(allocator, context);
        mutator.visit_expression(&mut expression);
        expression
    }
}

struct RawObjectArgumentDetector<'ctx> {
    context: &'ctx BuildContext,
    found: bool,
}

impl<'ctx> RawObjectArgumentDetector<'ctx> {
    fn new(context: &'ctx BuildContext) -> Self {
        Self {
            context,
            found: false,
        }
    }

    fn contains_expression<'a>(mut self, expression: &Expression<'a>) -> bool {
        self.visit_expression(expression);
        self.found
    }
}

impl<'a, 'ctx> Visit<'a> for RawObjectArgumentDetector<'ctx> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.found {
            return;
        }
        if !RawObjectCallRules::allows_argument_rewrite(call, self.context) {
            walk::walk_call_expression(self, call);
            return;
        }
        for argument in &call.arguments {
            let Some(expression) = argument.as_expression() else {
                continue;
            };
            if RawObjectExpression::new(expression, self.context).is_raw_object_field() {
                self.found = true;
                return;
            }
        }
        walk::walk_call_expression(self, call);
    }
}

struct RawObjectArgumentMutator<'a, 'ctx> {
    allocator: &'a Allocator,
    context: &'ctx BuildContext,
    changed: bool,
}

impl<'a, 'ctx> RawObjectArgumentMutator<'a, 'ctx> {
    fn new(allocator: &'a Allocator, context: &'ctx BuildContext) -> Self {
        Self {
            allocator,
            context,
            changed: false,
        }
    }
}

impl<'a, 'ctx> VisitMut<'a> for RawObjectArgumentMutator<'a, 'ctx> {
    fn visit_call_expression(&mut self, call: &mut CallExpression<'a>) {
        if !RawObjectCallRules::allows_argument_rewrite(call, self.context) {
            walk_mut::walk_call_expression(self, call);
            return;
        }
        for argument in &mut call.arguments {
            let Some(expression) = argument.as_expression_mut() else {
                self.visit_argument(argument);
                continue;
            };
            if RawObjectExpression::new(expression, self.context).is_raw_object_field() {
                let expression = expression.take_in(&self.allocator);
                let wrapped =
                    RawObjectCallEmitter::new(self.allocator, self.context).wrap(expression);
                *argument = Argument::from(wrapped);
                self.changed = true;
            } else {
                self.visit_argument(argument);
            }
        }
        walk_mut::walk_expression(self, &mut call.callee);
    }
}

struct RawObjectCallRules;

impl RawObjectCallRules {
    /// Mirrors SDK `process_component_class.js::isNeedGetRawObject`: only
    /// arguments of property-access calls are candidates. Identifier callees
    /// such as `helper(this.value)` deliberately keep the observed value.
    fn allows_argument_rewrite(call: &CallExpression<'_>, context: &BuildContext) -> bool {
        let method_name = match &call.callee {
            Expression::StaticMemberExpression(member) => member.property.name.as_str(),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => member.property.name.as_str(),
                _ => return false,
            },
            _ => return false,
        };
        method_name != context.runtime_names().get_raw_object
            && !matches!(
                method_name,
                GeneratedViewpuName::ENABLE_V2_COMPATIBILITY
                    | GeneratedViewpuName::MAKE_V1_OBSERVED
                    | GeneratedViewpuName::CAN_BE_OBSERVED
            )
    }
}

struct RawObjectExpression<'a> {
    expression: &'a Expression<'a>,
    context: &'a BuildContext,
}

impl<'a> RawObjectExpression<'a> {
    fn new(expression: &'a Expression<'a>, context: &'a BuildContext) -> Self {
        Self {
            expression,
            context,
        }
    }

    fn is_raw_object_field(&self) -> bool {
        let member = match self.expression {
            Expression::StaticMemberExpression(member) => member.as_ref(),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => member.as_ref(),
                _ => return false,
            },
            _ => return false,
        };
        if !matches!(&member.object, Expression::ThisExpression(_)) {
            return false;
        }
        self.context
            .is_raw_object_field(member.property.name.as_str())
    }
}

struct RawObjectCallEmitter<'alloc, 'ctx> {
    allocator: &'alloc Allocator,
    context: &'ctx BuildContext,
}

impl<'alloc, 'ctx> RawObjectCallEmitter<'alloc, 'ctx> {
    fn new(allocator: &'alloc Allocator, context: &'ctx BuildContext) -> Self {
        Self { allocator, context }
    }

    fn wrap(&self, expression: Expression<'alloc>) -> Expression<'alloc> {
        let ast = AstBuilder::new(self.allocator);
        let runtime_names = self.context.runtime_names();
        let observed_object =
            ast.expression_identifier(SPAN, ast.str(&runtime_names.observed_object));
        let method = ast.identifier_name(SPAN, ast.str(&runtime_names.get_raw_object));
        let callee = Self::member_to_expression(ast.member_expression_static(
            SPAN,
            observed_object,
            method,
            false,
        ));
        ast.expression_call(
            SPAN,
            callee,
            NONE,
            ast.vec1(Argument::from(expression)),
            false,
        )
    }

    fn member_to_expression(member: MemberExpression<'alloc>) -> Expression<'alloc> {
        match member {
            MemberExpression::ComputedMemberExpression(member) => {
                Expression::ComputedMemberExpression(member)
            }
            MemberExpression::StaticMemberExpression(member) => {
                Expression::StaticMemberExpression(member)
            }
            MemberExpression::PrivateFieldExpression(member) => {
                Expression::PrivateFieldExpression(member)
            }
        }
    }
}
