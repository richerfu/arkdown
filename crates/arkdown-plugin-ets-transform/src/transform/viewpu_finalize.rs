use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, ExportDefaultDeclarationKind, Expression, Program, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::source_edit::{apply_source_patches, SourcePatch};

const VIEWPU_FINALIZE_CONSTRUCTION_SHIM: &str = "if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n    Reflect.set(ViewPU.prototype, \"finalizeConstruction\", () => { });\n}\n";

pub(super) fn finalize_viewpu_output(source: &str) -> String {
    let stripped = strip_leading_file_comments(source);
    let shimmed = ensure_viewpu_finalize_construction_shim(&stripped);
    hoist_viewpu_params_interfaces(&shimmed)
}

fn ensure_viewpu_finalize_construction_shim(source: &str) -> String {
    let Some((has_viewpu_class, has_finalize_probe)) = with_parsed_program(source, |program| {
        (
            has_viewpu_class(program),
            has_finalize_construction_probe(program),
        )
    }) else {
        return source.to_owned();
    };
    if !has_viewpu_class || has_finalize_probe {
        return source.to_owned();
    }

    let mut out = String::with_capacity(VIEWPU_FINALIZE_CONSTRUCTION_SHIM.len() + source.len());
    out.push_str(VIEWPU_FINALIZE_CONSTRUCTION_SHIM);
    out.push_str(source);
    out
}

fn strip_leading_file_comments(source: &str) -> String {
    let Some(start) = with_parsed_program(source, first_statement_start) else {
        return source.trim_start().to_owned();
    };
    start.map_or_else(
        || source.trim_start().to_owned(),
        |start| source[start..].to_owned(),
    )
}

fn hoist_viewpu_params_interfaces(source: &str) -> String {
    let Some(hoist) = with_parsed_program(source, |program| {
        if has_viewpu_class(program) {
            params_interface_hoist(program, source)
        } else {
            None
        }
    }) else {
        return source.to_owned();
    };

    let Some(hoist) = hoist else {
        return source.to_owned();
    };
    apply_source_patches(source, hoist.into_patches())
}

fn with_parsed_program<R>(source: &str, f: impl FnOnce(&Program<'_>) -> R) -> Option<R> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts().with_module(true)).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    Some(f(&parsed.program))
}

fn first_statement_start(program: &Program<'_>) -> Option<usize> {
    program
        .body
        .first()
        .map(|statement| statement.span().start as usize)
}

fn has_viewpu_class(program: &Program<'_>) -> bool {
    program.body.iter().any(statement_declares_viewpu_class)
}

fn has_finalize_construction_probe(program: &Program<'_>) -> bool {
    program
        .body
        .iter()
        .any(statement_contains_finalize_construction)
}

fn params_interface_hoist(program: &Program<'_>, source: &str) -> Option<ParamsInterfaceHoist> {
    let mut spans = params_interface_spans(program);
    if spans.is_empty() {
        return None;
    }

    let insertion = leading_finalize_shim_end(program)
        .unwrap_or_else(|| first_statement_start(program).unwrap_or(0));
    spans.reverse();

    Some(ParamsInterfaceHoist {
        insertion,
        interfaces: spans
            .iter()
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
            .collect(),
        removed_spans: spans,
    })
}

fn params_interface_spans(program: &Program<'_>) -> Vec<Span> {
    program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::TSInterfaceDeclaration(interface)
                if interface.id.name.as_str().ends_with("_Params") =>
            {
                Some(interface.span)
            }
            _ => None,
        })
        .collect()
}

fn leading_finalize_shim_end(program: &Program<'_>) -> Option<usize> {
    let Some(Statement::IfStatement(statement)) = program.body.first() else {
        return None;
    };
    if_statement_contains_finalize_construction(statement).then_some(statement.span.end as usize)
}

struct ParamsInterfaceHoist {
    insertion: usize,
    interfaces: Vec<String>,
    removed_spans: Vec<Span>,
}

impl ParamsInterfaceHoist {
    fn into_patches(self) -> Vec<SourcePatch> {
        let mut patches = Vec::with_capacity(self.removed_spans.len() + 1);
        patches.push(SourcePatch {
            start: self.insertion,
            end: self.insertion,
            replacement: self.insertion_text(),
        });
        patches.extend(self.removed_spans.into_iter().map(|span| SourcePatch {
            start: span.start as usize,
            end: span.end as usize,
            replacement: String::new(),
        }));
        patches
    }

    fn insertion_text(&self) -> String {
        let mut text = String::new();
        text.push('\n');
        for interface in &self.interfaces {
            text.push_str(interface);
            if !interface.ends_with('\n') {
                text.push('\n');
            }
        }
        text
    }
}

fn statement_declares_viewpu_class(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ClassDeclaration(class) => class_extends_viewpu(&class.super_class),
        Statement::ExportDefaultDeclaration(export) => {
            matches!(&export.declaration, ExportDefaultDeclarationKind::ClassDeclaration(class) if class_extends_viewpu(&class.super_class))
        }
        Statement::ExportNamedDeclaration(export) => {
            matches!(&export.declaration, Some(Declaration::ClassDeclaration(class)) if class_extends_viewpu(&class.super_class))
        }
        _ => false,
    }
}

fn class_extends_viewpu(super_class: &Option<Expression<'_>>) -> bool {
    matches!(super_class, Some(Expression::Identifier(identifier)) if identifier.name.as_str() == "ViewPU")
}

fn statement_contains_finalize_construction(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::IfStatement(statement) => if_statement_contains_finalize_construction(statement),
        Statement::BlockStatement(block) => block
            .body
            .iter()
            .any(statement_contains_finalize_construction),
        Statement::ExpressionStatement(statement) => {
            expression_contains_finalize_construction(&statement.expression)
        }
        _ => false,
    }
}

fn if_statement_contains_finalize_construction(statement: &oxc_ast::ast::IfStatement<'_>) -> bool {
    expression_contains_finalize_construction(&statement.test)
        || statement_contains_finalize_construction(&statement.consequent)
        || statement
            .alternate
            .as_ref()
            .is_some_and(|statement| statement_contains_finalize_construction(statement))
}

fn expression_contains_finalize_construction(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(literal) => literal.value.as_str() == "finalizeConstruction",
        Expression::UnaryExpression(expression) => {
            expression_contains_finalize_construction(&expression.argument)
        }
        Expression::BinaryExpression(expression) => {
            expression_contains_finalize_construction(&expression.left)
                || expression_contains_finalize_construction(&expression.right)
        }
        Expression::CallExpression(call) => {
            expression_contains_finalize_construction(&call.callee)
                || call.arguments.iter().any(|argument| {
                    argument
                        .as_expression()
                        .is_some_and(expression_contains_finalize_construction)
                })
        }
        Expression::StaticMemberExpression(member) => {
            expression_contains_finalize_construction(&member.object)
        }
        Expression::ComputedMemberExpression(member) => {
            expression_contains_finalize_construction(&member.object)
                || expression_contains_finalize_construction(&member.expression)
        }
        _ => false,
    }
}
