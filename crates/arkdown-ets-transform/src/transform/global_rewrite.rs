use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::{
    CallExpression, ClassElement, Expression, MemberExpression, MethodDefinition, Program,
};
use oxc_ast::AstBuilder;
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_syntax::scope::ScopeFlags;

pub(crate) struct GlobalContextCallRewriter<'a> {
    allocator: &'a Allocator,
}

impl<'a> GlobalContextCallRewriter<'a> {
    pub(crate) fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    pub(crate) fn expression<'b>(&self, expression: &Expression<'b>) -> Option<Expression<'a>> {
        if !GlobalContextCallDetector::new().contains_expression(expression) {
            return None;
        }
        let mut expression = expression.clone_in(self.allocator);
        let changed = self.rewrite_owned_expression(&mut expression);
        debug_assert!(changed);
        Some(expression)
    }

    pub(crate) fn program(&self, program: &mut Program<'a>) -> bool {
        let mut rewrite = GlobalContextCallMutator::new(self.allocator);
        rewrite.visit_program(program);
        rewrite.changed
    }

    pub(crate) fn owned_expression(&self, mut expression: Expression<'a>) -> Expression<'a> {
        self.rewrite_owned_expression(&mut expression);
        expression
    }

    pub(crate) fn method_definition(&self, method: &MethodDefinition<'_>) -> ClassElement<'a> {
        self.method_definition_with_decorators(method, false)
    }

    /// Rewrites global-context calls in a generated component method while
    /// retaining its method decorators when the owning lowering pass needs
    /// their runtime registration.
    ///
    /// `process_struct_componentV2.js` retains ComponentV2 lifecycle method
    /// decorators on the generated `ViewV2` class. Other component lowering
    /// paths still intentionally erase their source-only decorators.
    pub(crate) fn method_definition_with_decorators(
        &self,
        method: &MethodDefinition<'_>,
        retain_decorators: bool,
    ) -> ClassElement<'a> {
        let ast = AstBuilder::new(self.allocator);
        let mut method = method.clone_in(self.allocator);
        let mut rewrite = GlobalContextCallMutator::new(self.allocator);
        rewrite.visit_function(&mut method.value, ScopeFlags::Function);
        if !retain_decorators {
            method.decorators.clear();
        }
        ClassElement::MethodDefinition(ast.alloc(method))
    }

    fn rewrite_owned_expression(&self, expression: &mut Expression<'a>) -> bool {
        let mut rewrite = GlobalContextCallMutator::new(self.allocator);
        rewrite.visit_expression(expression);
        rewrite.changed
    }
}

struct GlobalContextCallDetector {
    found: bool,
}

impl GlobalContextCallDetector {
    fn new() -> Self {
        Self { found: false }
    }

    fn contains_expression<'a>(mut self, expression: &Expression<'a>) -> bool {
        self.visit_expression(expression);
        self.found
    }

    fn contains_program<'a>(mut self, program: &Program<'a>) -> bool {
        self.visit_program(program);
        self.found
    }
}

impl<'a> Visit<'a> for GlobalContextCallDetector {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.found {
            return;
        }
        if let Expression::Identifier(callee) = &call.callee {
            if GlobalContextCallRewrite::new().is_target_callee(callee.name.as_str()) {
                self.found = true;
                return;
            }
        }
        walk::walk_call_expression(self, call);
    }
}

struct GlobalContextCallMutator<'a> {
    allocator: &'a Allocator,
    changed: bool,
}

impl<'a> GlobalContextCallMutator<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            changed: false,
        }
    }

    fn context_member_expression(&self, callee_name: &str) -> Expression<'a> {
        GlobalContextCallRewrite::new().callee_expression(self.allocator, callee_name)
    }
}

impl<'a> VisitMut<'a> for GlobalContextCallMutator<'a> {
    fn visit_call_expression(&mut self, call: &mut CallExpression<'a>) {
        if let Expression::Identifier(callee) = &call.callee {
            if GlobalContextCallRewrite::new().is_target_callee(callee.name.as_str()) {
                let callee_name = callee.name.to_string();
                call.callee = self.context_member_expression(&callee_name);
                self.changed = true;
            }
        }
        walk_mut::walk_call_expression(self, call);
    }
}

pub(crate) struct GlobalContextCallRewrite;

impl GlobalContextCallRewrite {
    const ANIMATE_TO: &'static str = "animateTo";
    const ANIMATE_TO_IMMEDIATELY: &'static str = "animateToImmediately";
    const CONTEXT: &'static str = "Context";

    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn is_target_callee(&self, name: &str) -> bool {
        matches!(name, Self::ANIMATE_TO | Self::ANIMATE_TO_IMMEDIATELY)
    }

    pub(crate) fn program_needs_rewrite(&self, program: &Program<'_>) -> bool {
        GlobalContextCallDetector::new().contains_program(program)
    }

    pub(crate) fn callee_expression<'a>(
        &self,
        allocator: &'a Allocator,
        callee_name: &str,
    ) -> Expression<'a> {
        debug_assert!(self.is_target_callee(callee_name));
        let ast = AstBuilder::new(allocator);
        let object = ast.expression_identifier(oxc_span::SPAN, Self::CONTEXT);
        let property = ast.identifier_name(oxc_span::SPAN, ast.str(callee_name));
        Self::member_to_expression(ast.member_expression_static(
            oxc_span::SPAN,
            object,
            property,
            false,
        ))
    }

    fn member_to_expression<'a>(member: MemberExpression<'a>) -> Expression<'a> {
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
