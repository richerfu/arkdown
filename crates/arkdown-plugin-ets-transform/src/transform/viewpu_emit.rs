//! ViewPU AST emission helpers.
//!
//! This module is the transition boundary between ArkUI lowering decisions and
//! generated code. The lowering code should call these helpers for generated
//! ViewPU statements so the final text is printed by OXC codegen.

use std::fmt::{self, Write};

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, Expression, FormalParameterKind, MemberExpression, Program, Statement,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};

pub(crate) struct ViewpuWriter<'a> {
    out: &'a mut String,
}

impl<'a> ViewpuWriter<'a> {
    pub(crate) fn new(out: &'a mut String) -> Self {
        Self { out }
    }

    pub(crate) fn line(&mut self, indent: &str, args: fmt::Arguments<'_>) {
        self.out.push_str(indent);
        self.out
            .write_fmt(args)
            .expect("writing to String cannot fail");
        self.out.push('\n');
    }

    pub(crate) fn observe_component_creation(
        &mut self,
        indent: &str,
        use_parent_context: bool,
        component_name: &str,
        body: &str,
    ) {
        self.out
            .push_str(&ViewpuAstEmitter::new(indent).observe_component_creation(
                use_parent_context,
                component_name,
                body,
            ));
    }

    pub(crate) fn observe_component_creation_start(&mut self, indent: &str, view_context: &str) {
        self.line(
            indent,
            format_args!(
                "{view_context}.observeComponentCreation2((elmtId, isInitialRender) => {{"
            ),
        );
    }

    pub(crate) fn component_create(
        &mut self,
        indent: &str,
        component_name: &str,
        create_method: &str,
        args: &str,
    ) {
        self.call_statement(indent, component_name, create_method, args);
    }

    pub(crate) fn component_attribute(
        &mut self,
        indent: &str,
        component_name: &str,
        attr_name: &str,
        attr_args: &str,
    ) {
        self.call_statement(indent, component_name, attr_name, attr_args);
    }

    pub(crate) fn observe_component_creation_end(&mut self, indent: &str, component_name: &str) {
        self.line(indent, format_args!("}}, {component_name});"));
    }

    pub(crate) fn component_pop(&mut self, indent: &str, component_name: &str) {
        self.call_statement(indent, component_name, "pop", "");
    }

    fn call_statement(&mut self, indent: &str, object_name: &str, method_name: &str, args: &str) {
        self.out
            .push_str(&ViewpuAstEmitter::new(indent).call_statement(
                object_name,
                method_name,
                args,
            ));
    }
}

struct ViewpuAstEmitter<'a> {
    indent: &'a str,
}

impl<'a> ViewpuAstEmitter<'a> {
    fn new(indent: &'a str) -> Self {
        Self { indent }
    }

    fn call_statement(&self, object_name: &str, method_name: &str, args: &str) -> String {
        let allocator = Allocator::default();
        let ast = AstBuilder::new(&allocator);
        let object = ast.expression_identifier(SPAN, object_name);
        let property = ast.identifier_name(SPAN, method_name);
        let member = ast.member_expression_static(SPAN, object, property, false);
        let callee = Self::member_expression_to_expression(member);
        let arguments = Self::parse_arguments_in(args, &allocator);
        let call = ast.expression_call(SPAN, callee, NONE, arguments, false);
        let statement = ast.statement_expression(SPAN, call);
        let program = Self::program_with_statement(ast, statement);
        self.render_program(&program)
    }

    fn observe_component_creation(
        &self,
        use_parent_context: bool,
        component_name: &str,
        body: &str,
    ) -> String {
        let allocator = Allocator::default();
        let ast = AstBuilder::new(&allocator);
        let object = Self::view_context_expression(ast, use_parent_context);
        let property = ast.identifier_name(SPAN, "observeComponentCreation2");
        let member = ast.member_expression_static(SPAN, object, property, false);
        let callee = Self::member_expression_to_expression(member);
        let arrow = Self::observe_arrow_function(ast, body, &allocator);
        let component = ast.expression_identifier(SPAN, component_name);
        let arguments = ast.vec_from_iter([
            Self::argument_from_expression(arrow),
            Self::argument_from_expression(component),
        ]);
        let call = ast.expression_call(SPAN, callee, NONE, arguments, false);
        let statement = ast.statement_expression(SPAN, call);
        let program = Self::program_with_statement(ast, statement);
        self.render_program(&program)
    }

    fn view_context_expression<'b>(
        ast: AstBuilder<'b>,
        use_parent_context: bool,
    ) -> Expression<'b> {
        if !use_parent_context {
            return ast.expression_this(SPAN);
        }

        let test = ast.expression_identifier(SPAN, "parent");
        let consequent = ast.expression_identifier(SPAN, "parent");
        let alternate = ast.expression_this(SPAN);
        let conditional = ast.expression_conditional(SPAN, test, consequent, alternate);
        ast.expression_parenthesized(SPAN, conditional)
    }

    fn observe_arrow_function<'b>(
        ast: AstBuilder<'b>,
        body: &str,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let mut params = ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec_with_capacity(2),
            NONE,
        );
        params.items.push(Self::formal_parameter(ast, "elmtId"));
        params
            .items
            .push(Self::formal_parameter(ast, "isInitialRender"));
        let statements = Self::parse_statements_in(body, allocator);
        let body = ast.function_body(SPAN, ast.vec(), statements);
        ast.expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
    }

    fn formal_parameter<'b>(
        ast: AstBuilder<'b>,
        name: &'static str,
    ) -> oxc_ast::ast::FormalParameter<'b> {
        ast.formal_parameter(
            SPAN,
            ast.vec(),
            ast.binding_pattern_binding_identifier(SPAN, name),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
        )
    }

    fn argument_from_expression<'b>(expression: Expression<'b>) -> Argument<'b> {
        match expression {
            Expression::Identifier(value) => Argument::Identifier(value),
            Expression::ArrowFunctionExpression(value) => Argument::ArrowFunctionExpression(value),
            _ => panic!("generated ViewPU observe arguments must be identifiers or arrows"),
        }
    }

    fn member_expression_to_expression<'b>(member: MemberExpression<'b>) -> Expression<'b> {
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

    fn parse_arguments_in<'b>(args: &str, allocator: &'b Allocator) -> OxcVec<'b, Argument<'b>> {
        if args.trim().is_empty() {
            return OxcVec::new_in(allocator);
        }

        let source = format!("__arkdown__({args});");
        let parse_allocator = Allocator::default();
        let source_type = SourceType::ts().with_module(true);
        let parsed = Parser::new(&parse_allocator, &source, source_type).parse();
        assert!(
            parsed.errors.is_empty(),
            "generated ViewPU call arguments must parse as TypeScript"
        );

        let Some(Statement::ExpressionStatement(statement)) = parsed.program.body.first() else {
            panic!("generated ViewPU call argument wrapper must parse as an expression statement");
        };
        let Expression::CallExpression(call) = &statement.expression else {
            panic!("generated ViewPU call argument wrapper must parse as a call expression");
        };

        let ast = AstBuilder::new(allocator);
        ast.vec_from_iter(
            call.arguments
                .iter()
                .map(|argument| argument.clone_in(allocator)),
        )
    }

    fn parse_statements_in<'b>(body: &str, allocator: &'b Allocator) -> OxcVec<'b, Statement<'b>> {
        if body.trim().is_empty() {
            return OxcVec::new_in(allocator);
        }

        let source = format!("function __arkdown__() {{\n{body}\n}}");
        let parse_allocator = Allocator::default();
        let source_type = SourceType::ts().with_module(true);
        let parsed = Parser::new(&parse_allocator, &source, source_type).parse();
        assert!(
            parsed.errors.is_empty(),
            "generated ViewPU observe body must parse as TypeScript"
        );

        let Some(Statement::FunctionDeclaration(function)) = parsed.program.body.first() else {
            panic!("generated ViewPU observe body wrapper must parse as a function declaration");
        };
        let Some(function_body) = &function.body else {
            panic!("generated ViewPU observe body wrapper must have a function body");
        };

        let ast = AstBuilder::new(allocator);
        ast.vec_from_iter(
            function_body
                .statements
                .iter()
                .map(|statement| statement.clone_in(allocator)),
        )
    }

    fn program_with_statement<'b>(ast: AstBuilder<'b>, statement: Statement<'b>) -> Program<'b> {
        ast.program(
            SPAN,
            SourceType::ts().with_module(true),
            "",
            ast.vec(),
            None,
            ast.vec(),
            ast.vec1(statement),
        )
    }

    fn render_program(&self, program: &Program<'_>) -> String {
        Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Space,
                indent_width: 4,
                initial_indent: (self.indent.len() / 4) as u32,
                ..CodegenOptions::default()
            })
            .build(program)
            .code
    }
}
