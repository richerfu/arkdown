use oxc_allocator::{Allocator, ArenaBox, CloneIn, FromIn};
use oxc_ast::ast::{
    CallExpression, ClassElement, Expression, IdentifierName, MemberExpression, MethodDefinition,
    NewExpression, Program, TSType, TSTypeName,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_str::Str;
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
        ClassElement::MethodDefinition(ArenaBox::new_in(method, &ast))
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
        let object = Expression::new_identifier(oxc_span::SPAN, Self::CONTEXT, &ast);
        let callee_name = oxc_str::Ident::from_in(callee_name, allocator);
        let property = IdentifierName::new(oxc_span::SPAN, callee_name, &ast);
        Self::member_to_expression(MemberExpression::new_static_member_expression(
            oxc_span::SPAN,
            object,
            property,
            false,
            &ast,
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

/// Rewrites the first literal path passed to `new <namespace>.Worker(...)`.
///
/// This is the exact `process_ui_syntax.ts::processWorker` dynamic ArkTS rule:
/// only property-access constructors named `Worker` participate and only a
/// trailing `.ts` suffix becomes `.js`.
pub(crate) struct WorkerPathRewriter<'a> {
    allocator: &'a Allocator,
}

impl<'a> WorkerPathRewriter<'a> {
    pub(crate) fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    pub(crate) fn program(&self, program: &mut Program<'a>) -> bool {
        let mut mutator = WorkerPathMutator {
            allocator: self.allocator,
            changed: false,
        };
        mutator.visit_program(program);
        mutator.changed
    }
}

pub(crate) struct WorkerPathRewrite;

impl WorkerPathRewrite {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn program_needs_rewrite(&self, program: &Program<'_>) -> bool {
        let mut detector = WorkerPathDetector { found: false };
        detector.visit_program(program);
        detector.found
    }

    fn is_target(&self, expression: &NewExpression<'_>) -> bool {
        matches!(
            &expression.callee,
            Expression::StaticMemberExpression(member) if member.property.name == "Worker"
        ) && matches!(
            expression.arguments.first(),
            Some(oxc_ast::ast::Argument::StringLiteral(literal)) if literal.value.ends_with(".ts")
        )
    }
}

struct WorkerPathDetector {
    found: bool,
}

impl<'a> Visit<'a> for WorkerPathDetector {
    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if WorkerPathRewrite::new().is_target(expression) {
            self.found = true;
            return;
        }
        walk::walk_new_expression(self, expression);
    }
}

struct WorkerPathMutator<'a> {
    allocator: &'a Allocator,
    changed: bool,
}

impl<'a> VisitMut<'a> for WorkerPathMutator<'a> {
    fn visit_new_expression(&mut self, expression: &mut NewExpression<'a>) {
        if WorkerPathRewrite::new().is_target(expression) {
            let Some(oxc_ast::ast::Argument::StringLiteral(literal)) =
                expression.arguments.first_mut()
            else {
                unreachable!("WorkerPathRewrite established a literal first argument");
            };
            let rewritten = literal.value.trim_end_matches(".ts").to_string() + ".js";
            literal.value = Str::from_str_in(&rewritten, &self.allocator);
            literal.raw = None;
            literal.span = oxc_span::SPAN;
            self.changed = true;
        }
        walk_mut::walk_new_expression(self, expression);
    }
}

/// Dynamic ArkTS erases the interop-only `ESObject` type to TypeScript `any`.
/// This mirrors `process_ui_syntax.ts::isESObjectNode` in the same OXC pass as
/// the other whole-program rewrites.
pub(crate) struct EsObjectTypeRewriter<'a> {
    allocator: &'a Allocator,
}

impl<'a> EsObjectTypeRewriter<'a> {
    pub(crate) fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    pub(crate) fn program(&self, program: &mut Program<'a>) -> bool {
        let mut mutator = EsObjectTypeMutator {
            allocator: self.allocator,
            changed: false,
        };
        mutator.visit_program(program);
        mutator.changed
    }
}

pub(crate) struct EsObjectTypeRewrite;

impl EsObjectTypeRewrite {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn program_needs_rewrite(&self, program: &Program<'_>) -> bool {
        let mut detector = EsObjectTypeDetector { found: false };
        detector.visit_program(program);
        detector.found
    }

    fn is_target(&self, ty: &TSType<'_>) -> bool {
        matches!(
            ty,
            TSType::TSTypeReference(reference)
                if matches!(
                    &reference.type_name,
                    TSTypeName::IdentifierReference(identifier) if identifier.name == "ESObject"
                )
        )
    }
}

struct EsObjectTypeDetector {
    found: bool,
}

impl<'a> Visit<'a> for EsObjectTypeDetector {
    fn visit_ts_type(&mut self, ty: &TSType<'a>) {
        if EsObjectTypeRewrite::new().is_target(ty) {
            self.found = true;
            return;
        }
        walk::walk_ts_type(self, ty);
    }
}

struct EsObjectTypeMutator<'a> {
    allocator: &'a Allocator,
    changed: bool,
}

impl<'a> VisitMut<'a> for EsObjectTypeMutator<'a> {
    fn visit_ts_type(&mut self, ty: &mut TSType<'a>) {
        if EsObjectTypeRewrite::new().is_target(ty) {
            *ty = TSType::new_ts_any_keyword(oxc_span::SPAN, &AstBuilder::new(self.allocator));
            self.changed = true;
            return;
        }
        walk_mut::walk_ts_type(self, ty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    #[test]
    fn worker_path_rewrite_matches_process_worker() {
        let allocator = Allocator::default();
        let mut parsed = Parser::new(
            &allocator,
            "const task = new worker.Worker('workers/task.ts');",
            SourceType::ets(),
        )
        .parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert!(
            WorkerPathRewrite::new().program_needs_rewrite(&parsed.program),
            "{:#?}",
            parsed.program
        );
        assert!(WorkerPathRewriter::new(&allocator).program(&mut parsed.program));
        let output = oxc_codegen::Codegen::new().build(&parsed.program).code;
        assert!(
            output.contains("new worker.Worker(\"workers/task.js\")"),
            "{output}"
        );

        for source in [
            "const task = new Worker('workers/task.ts');",
            "const task = new worker.Worker(workerPath);",
            "const task = new worker.Other('workers/task.ts');",
        ] {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
            assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
            assert!(
                !WorkerPathRewrite::new().program_needs_rewrite(&parsed.program),
                "processWorker must leave this form unchanged: {source}"
            );
        }
    }

    #[test]
    fn es_object_type_rewrite_matches_process_ui_syntax() {
        let allocator = Allocator::default();
        let mut parsed = Parser::new(
            &allocator,
            "function run(value: ESObject): ESObject[] { return [value]; }",
            SourceType::ets(),
        )
        .parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert!(EsObjectTypeRewrite::new().program_needs_rewrite(&parsed.program));
        assert!(EsObjectTypeRewriter::new(&allocator).program(&mut parsed.program));
        let output = oxc_codegen::Codegen::new().build(&parsed.program).code;
        assert!(
            output.contains("function run(value: any): any[]"),
            "{output}"
        );
    }
}
