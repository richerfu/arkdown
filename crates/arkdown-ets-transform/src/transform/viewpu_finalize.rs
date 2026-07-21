use oxc_allocator::{Allocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, Declaration, ExportDefaultDeclarationKind, Expression, Program, Statement,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::transform::source_emit::CodegenSourceText;
use crate::transform::viewpu_emit::GeneratedViewpuName;

pub(super) struct ViewpuOutputFinalizer<'a> {
    source: &'a str,
}

impl<'a> ViewpuOutputFinalizer<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub(super) fn finalize(&self) -> String {
        let allocator = Allocator::default();
        // The lowered source no longer contains `struct` declarations, but it
        // can still contain ArkUI class/member decorators. Keep the ArkUI
        // source goal through this final AST pass so OXC applies the same
        // decorator/export ordering as the SDK's TypeScript emitter.
        let mut parsed = Parser::new(&allocator, self.source, SourceType::ets()).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return self.source.to_owned();
        };

        let mut pass = ViewpuFinalizePass::new(&allocator, &mut parsed.program);
        pass.run();
        if !pass.changed {
            return self.source.to_owned();
        }

        CodegenSourceText::new(
            Codegen::new()
                .with_options(CodegenOptions {
                    comments: CommentOptions::disabled(),
                    indent_char: IndentChar::Space,
                    indent_width: 4,
                    ..CodegenOptions::default()
                })
                .build(&parsed.program)
                .code,
        )
        .without_trailing_newline()
    }
}

struct ViewpuFinalizePass<'a, 'program> {
    allocator: &'a Allocator,
    program: &'program mut Program<'a>,
    changed: bool,
}

impl<'a, 'program> ViewpuFinalizePass<'a, 'program> {
    fn new(allocator: &'a Allocator, program: &'program mut Program<'a>) -> Self {
        Self {
            allocator,
            program,
            changed: false,
        }
    }

    fn run(&mut self) {
        self.changed |= self.first_statement_start().is_some_and(|start| start > 0);
        if !self.has_view_class() {
            return;
        }

        let needs_shim = !self.has_finalize_construction_probe();
        let has_leading_shim = !needs_shim && self.leading_finalize_shim_end().is_some();
        let params_interfaces = self.params_interface_indices();
        if needs_shim || !params_interfaces.is_empty() {
            self.rebuild_body(needs_shim, has_leading_shim, &params_interfaces);
            self.changed = true;
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
            final_body.push(FinalizeConstructionShim::new().statement(ast, self.allocator));
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest {
                final_body.push(statement);
            }
        } else if has_leading_shim {
            let mut rest_iter = rest.into_iter();
            if let Some((_, statement)) = rest_iter.next() {
                final_body.push(statement);
            }
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest_iter {
                final_body.push(statement);
            }
        } else {
            self.push_params_interfaces(&mut final_body, params);
            for (_, statement) in rest {
                final_body.push(statement);
            }
        }

        self.program.body = final_body;
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

    fn first_statement_start(&self) -> Option<usize> {
        self.program
            .body
            .first()
            .map(|statement| statement.span().start as usize)
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

    fn statement<'a>(&self, ast: AstBuilder<'a>, allocator: &'a Allocator) -> Statement<'a> {
        let test = ast.expression_unary(
            Span::default(),
            UnaryOperator::LogicalNot,
            ast.expression_parenthesized(Span::default(), self.finalize_in_viewpu_prototype(ast)),
        );
        ast.statement_if(
            Span::default(),
            test,
            ast.statement_block(
                Span::default(),
                ast.vec1(self.reflect_set_statement(ast, allocator)),
            ),
            None,
        )
    }

    fn finalize_in_viewpu_prototype<'a>(&self, ast: AstBuilder<'a>) -> Expression<'a> {
        ast.expression_binary(
            Span::default(),
            ast.expression_string_literal(
                Span::default(),
                GeneratedViewpuName::FINALIZE_CONSTRUCTION,
                None,
            ),
            BinaryOperator::In,
            self.viewpu_prototype_expression(ast),
        )
    }

    fn viewpu_prototype_expression<'a>(&self, ast: AstBuilder<'a>) -> Expression<'a> {
        let object = ast.expression_identifier(Span::default(), GeneratedViewpuName::VIEWPU_BASE);
        Expression::from(ast.member_expression_static(
            Span::default(),
            object,
            ast.identifier_name(Span::default(), GeneratedViewpuName::PROTOTYPE),
            false,
        ))
    }

    fn reflect_set_statement<'a>(
        &self,
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
    ) -> Statement<'a> {
        let callee = Expression::from(ast.member_expression_static(
            Span::default(),
            ast.expression_identifier(Span::default(), GeneratedViewpuName::REFLECT),
            ast.identifier_name(Span::default(), GeneratedViewpuName::SET),
            false,
        ));
        let arguments = ast.vec_from_iter([
            Argument::from(self.viewpu_prototype_expression(ast)),
            Argument::from(ast.expression_string_literal(
                Span::default(),
                GeneratedViewpuName::FINALIZE_CONSTRUCTION,
                None,
            )),
            Argument::from(self.empty_arrow_function(ast, allocator)),
        ]);
        ast.statement_expression(
            Span::default(),
            ast.expression_call(Span::default(), callee, NONE, arguments, false),
        )
    }

    fn empty_arrow_function<'a>(
        &self,
        ast: AstBuilder<'a>,
        allocator: &'a Allocator,
    ) -> Expression<'a> {
        ast.expression_arrow_function(
            Span::default(),
            false,
            false,
            NONE,
            ast.formal_parameters(
                Span::default(),
                oxc_ast::ast::FormalParameterKind::ArrowFormalParameters,
                OxcVec::new_in(&allocator),
                NONE,
            ),
            NONE,
            ast.function_body(
                Span::default(),
                OxcVec::new_in(&allocator),
                OxcVec::new_in(&allocator),
            ),
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
            Statement::ClassDeclaration(class) => Self::class_extends_view_base(&class.super_class),
            Statement::ExportDefaultDeclaration(export) => {
                matches!(&export.declaration, ExportDefaultDeclarationKind::ClassDeclaration(class) if Self::class_extends_view_base(&class.super_class))
            }
            Statement::ExportNamedDeclaration(export) => {
                matches!(&export.declaration, Some(Declaration::ClassDeclaration(class)) if Self::class_extends_view_base(&class.super_class))
            }
            _ => false,
        }
    }

    fn class_extends_view_base(super_class: &Option<Expression<'_>>) -> bool {
        matches!(super_class, Some(Expression::Identifier(identifier)) if matches!(
            identifier.name.as_str(),
            GeneratedViewpuName::VIEWPU_BASE | GeneratedViewpuName::VIEW_V2_BASE
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
