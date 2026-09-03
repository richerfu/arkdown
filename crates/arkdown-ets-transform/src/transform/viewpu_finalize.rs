use oxc_allocator::{Allocator, ArenaBox, ArenaVec, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, Declaration, ExportDefaultDeclarationKind, Expression, FormalParameters,
    FunctionBody, IdentifierName, MemberExpression, Program, Statement,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::Span;
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::transform::local_builder::LocalBuilderRuntimeEmitter;
use crate::transform::viewpu_emit::GeneratedViewpuName;

/// Applies the final ViewPU statement ordering and compatibility shim to the
/// transformed program before it is printed.
///
/// The SDK performs these updates in its TypeScript transformer and only then
/// calls `printSourceFile`. Keeping the finalization on the same AST preserves
/// original node spans and ensures generated source-map locations describe the
/// final emitted layout.
pub(super) struct ViewpuProgramFinalizer<'a, 'program> {
    allocator: &'a Allocator,
    program: &'program mut Program<'a>,
    local_builder_context: bool,
}

impl<'a, 'program> ViewpuProgramFinalizer<'a, 'program> {
    pub(super) fn new(allocator: &'a Allocator, program: &'program mut Program<'a>) -> Self {
        Self {
            allocator,
            program,
            local_builder_context: false,
        }
    }

    pub(super) fn with_local_builder_context(mut self, enabled: bool) -> Self {
        self.local_builder_context = enabled;
        self
    }

    pub(super) fn finalize(self) {
        let mut pass =
            ViewpuFinalizePass::new(self.allocator, self.program, self.local_builder_context);
        pass.run();
    }
}

struct ViewpuFinalizePass<'a, 'program> {
    allocator: &'a Allocator,
    program: &'program mut Program<'a>,
    local_builder_context: bool,
}

impl<'a, 'program> ViewpuFinalizePass<'a, 'program> {
    fn new(
        allocator: &'a Allocator,
        program: &'program mut Program<'a>,
        local_builder_context: bool,
    ) -> Self {
        Self {
            allocator,
            program,
            local_builder_context,
        }
    }

    fn run(&mut self) {
        if !self.has_view_class() {
            return;
        }

        let needs_shim = !self.has_finalize_construction_probe();
        let has_leading_shim = !needs_shim && self.leading_finalize_shim_end().is_some();
        let params_interfaces = self.params_interface_indices();
        if needs_shim || self.local_builder_context || !params_interfaces.is_empty() {
            self.rebuild_body(needs_shim, has_leading_shim, &params_interfaces);
        }
    }

    fn rebuild_body(
        &mut self,
        needs_shim: bool,
        has_leading_shim: bool,
        params_interfaces: &[usize],
    ) {
        let ast = AstBuilder::new(self.allocator);
        let old_body = std::mem::replace(&mut self.program.body, OxcVec::new_in(&self.allocator));
        let mut final_body = OxcVec::new_in(&self.allocator);
        let mut params = Vec::new();
        let mut rest = Vec::new();

        for (index, statement) in old_body.into_iter().enumerate() {
            if params_interfaces.binary_search(&index).is_ok() {
                params.push(statement);
            } else {
                rest.push((index, statement));
            }
        }

        if needs_shim {
            final_body.push(FinalizeConstructionShim::new().statement(&ast));
            self.push_local_builder_context(&mut final_body, &ast);
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest {
                final_body.push(statement);
            }
        } else if has_leading_shim {
            let mut rest_iter = rest.into_iter();
            if let Some((_, statement)) = rest_iter.next() {
                final_body.push(statement);
            }
            self.push_local_builder_context(&mut final_body, &ast);
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest_iter {
                final_body.push(statement);
            }
        } else {
            self.push_local_builder_context(&mut final_body, &ast);
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest {
                final_body.push(statement);
            }
        }

        self.program.body = final_body;
    }

    fn push_local_builder_context(
        &self,
        body: &mut OxcVec<'a, Statement<'a>>,
        ast: &AstBuilder<'a>,
    ) {
        if self.local_builder_context {
            body.push(LocalBuilderRuntimeEmitter::new().guard_statement(ast));
        }
    }

    fn push_params_interfaces(
        &self,
        body: &mut OxcVec<'a, Statement<'a>>,
        mut params: Vec<Statement<'a>>,
    ) {
        while let Some(statement) = params.pop() {
            body.push(statement);
        }
    }

    fn has_view_class(&self) -> bool {
        self.program
            .body
            .iter()
            .any(ViewpuClassProbe::declares_view_class)
    }

    fn has_finalize_construction_probe(&self) -> bool {
        self.program
            .body
            .iter()
            .any(FinalizeConstructionProbe::statement_contains)
    }

    fn params_interface_indices(&self) -> Vec<usize> {
        self.program
            .body
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| match statement {
                Statement::TSInterfaceDeclaration(interface)
                    if ViewPuParamsInterfaceName::new(interface.id.name.as_str())
                        .is_params_type() =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect()
    }

    fn leading_finalize_shim_end(&self) -> Option<usize> {
        let Some(Statement::IfStatement(statement)) = self.program.body.first() else {
            return None;
        };
        FinalizeConstructionProbe::if_statement_contains(statement)
            .then_some(statement.span.end as usize)
    }
}

struct FinalizeConstructionShim;

impl FinalizeConstructionShim {
    fn new() -> Self {
        Self
    }

    fn statement<'a>(&self, ast: &AstBuilder<'a>) -> Statement<'a> {
        let test = Expression::new_unary_expression(
            Span::default(),
            UnaryOperator::LogicalNot,
            Expression::new_parenthesized_expression(
                Span::default(),
                self.finalize_in_viewpu_prototype(ast),
                ast,
            ),
            ast,
        );
        Statement::new_if_statement(
            Span::default(),
            test,
            Statement::new_block_statement(
                Span::default(),
                ArenaVec::from_value_in(self.reflect_set_statement(ast), ast),
                ast,
            ),
            None,
            ast,
        )
    }

    fn finalize_in_viewpu_prototype<'a>(&self, ast: &AstBuilder<'a>) -> Expression<'a> {
        Expression::new_binary_expression(
            Span::default(),
            Expression::new_string_literal(
                Span::default(),
                GeneratedViewpuName::FINALIZE_CONSTRUCTION,
                None,
                ast,
            ),
            BinaryOperator::In,
            self.viewpu_prototype_expression(ast),
            ast,
        )
    }

    fn viewpu_prototype_expression<'a>(&self, ast: &AstBuilder<'a>) -> Expression<'a> {
        let object =
            Expression::new_identifier(Span::default(), GeneratedViewpuName::VIEWPU_BASE, ast);
        Expression::from(MemberExpression::new_static_member_expression(
            Span::default(),
            object,
            IdentifierName::new(Span::default(), GeneratedViewpuName::PROTOTYPE, ast),
            false,
            ast,
        ))
    }

    fn reflect_set_statement<'a>(&self, ast: &AstBuilder<'a>) -> Statement<'a> {
        let callee = Expression::from(MemberExpression::new_static_member_expression(
            Span::default(),
            Expression::new_identifier(Span::default(), GeneratedViewpuName::REFLECT, ast),
            IdentifierName::new(Span::default(), GeneratedViewpuName::SET, ast),
            false,
            ast,
        ));
        let arguments = ArenaVec::from_iter_in(
            [
                Argument::from(self.viewpu_prototype_expression(ast)),
                Argument::from(Expression::new_string_literal(
                    Span::default(),
                    GeneratedViewpuName::FINALIZE_CONSTRUCTION,
                    None,
                    ast,
                )),
                Argument::from(self.empty_arrow_function(ast)),
            ],
            ast,
        );
        Statement::new_expression_statement(
            Span::default(),
            Expression::new_call_expression(Span::default(), callee, None, arguments, false, ast),
            ast,
        )
    }

    fn empty_arrow_function<'a>(&self, ast: &AstBuilder<'a>) -> Expression<'a> {
        Expression::new_arrow_function_expression(
            Span::default(),
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    Span::default(),
                    oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(ast),
                    None,
                    ast,
                ),
                ast,
            ),
            None,
            oxc_ast::ast::ArrowFunctionBody::FunctionBody(ArenaBox::new_in(
                FunctionBody::new(
                    Span::default(),
                    OxcVec::new_in(ast),
                    OxcVec::new_in(ast),
                    ast,
                ),
                ast,
            )),
            ast,
        )
    }
}

struct ViewPuParamsInterfaceName<'a> {
    name: &'a str,
}

impl<'a> ViewPuParamsInterfaceName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn is_params_type(&self) -> bool {
        GeneratedViewpuName::params_interface_component(self.name).is_some()
    }
}

struct ViewpuClassProbe;

impl ViewpuClassProbe {
    fn declares_view_class(statement: &Statement<'_>) -> bool {
        match statement {
            Statement::ClassDeclaration(class) => {
                Self::class_extends_view_base(class.heritage.as_ref())
            }
            Statement::ExportDefaultDeclaration(export) => {
                matches!(&export.declaration, ExportDefaultDeclarationKind::ClassDeclaration(class) if Self::class_extends_view_base(class.heritage.as_ref()))
            }
            Statement::ExportDeclaration(export) => {
                matches!(&export.declaration, Declaration::ClassDeclaration(class) if Self::class_extends_view_base(class.heritage.as_ref()))
            }
            _ => false,
        }
    }

    fn class_extends_view_base(heritage: Option<&oxc_ast::ast::ClassHeritage<'_>>) -> bool {
        matches!(heritage, Some(heritage) if matches!(
            &heritage.expression,
            Expression::Identifier(identifier) if matches!(
                identifier.name.as_str(),
                GeneratedViewpuName::VIEWPU_BASE | GeneratedViewpuName::VIEW_V2_BASE
            )
        ))
    }
}

struct FinalizeConstructionProbe;

impl FinalizeConstructionProbe {
    fn statement_contains(statement: &Statement<'_>) -> bool {
        match statement {
            Statement::IfStatement(statement) => Self::if_statement_contains(statement),
            Statement::BlockStatement(block) => block.body.iter().any(Self::statement_contains),
            Statement::ExpressionStatement(statement) => {
                Self::expression_contains(&statement.expression)
            }
            _ => false,
        }
    }

    fn if_statement_contains(statement: &oxc_ast::ast::IfStatement<'_>) -> bool {
        Self::expression_contains(&statement.test)
            || Self::statement_contains(&statement.consequent)
            || statement
                .alternate
                .as_ref()
                .is_some_and(|statement| Self::statement_contains(statement))
    }

    fn expression_contains(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::StringLiteral(literal) => {
                literal.value.as_str() == GeneratedViewpuName::FINALIZE_CONSTRUCTION
            }
            Expression::UnaryExpression(expression) => {
                Self::expression_contains(&expression.argument)
            }
            Expression::BinaryExpression(expression) => {
                Self::expression_contains(&expression.left)
                    || Self::expression_contains(&expression.right)
            }
            Expression::CallExpression(call) => {
                Self::expression_contains(&call.callee)
                    || call.arguments.iter().any(|argument| {
                        argument
                            .as_expression()
                            .is_some_and(Self::expression_contains)
                    })
            }
            Expression::StaticMemberExpression(member) => Self::expression_contains(&member.object),
            Expression::ComputedMemberExpression(member) => {
                Self::expression_contains(&member.object)
                    || Self::expression_contains(&member.expression)
            }
            _ => false,
        }
    }
}
