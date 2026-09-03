use rustc_hash::FxHashSet;

use oxc_allocator::{Allocator, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Expression, ImportExpression, ImportOrExportKind, LazyImportDeclaration, Program, Statement,
    StringLiteral,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};

/// Builds the standard-ESM source consumed only by Rolldown's graph scanner
/// for an ArkTS 1.2 module.
///
/// Static ArkTS remains owned by the static compiler backend. Rolldown has no
/// static ETS module type, so feeding it that source would either fail parsing
/// or incorrectly route it through the legacy TypeScript lowering. The
/// projection keeps static and dynamic request kinds distinct and deliberately
/// contains no executable ArkTS business logic.
pub(super) struct StaticArktsGraphProjection {
    requests: Vec<StaticArktsModuleRequest>,
}

impl StaticArktsGraphProjection {
    pub(super) fn new(program: &Program<'_>) -> Self {
        let mut inventory = StaticArktsModuleRequestInventory::default();
        inventory.visit_program(program);
        Self {
            requests: inventory.requests,
        }
    }

    pub(super) fn project(&self, allocator: &Allocator) -> String {
        let ast = AstBuilder::new(allocator);
        let marker = Parser::new(allocator, "export {};", SourceType::ts())
            .parse()
            .program
            .body
            .pop()
            .expect("the static graph module marker is valid TypeScript");
        let mut body = OxcVec::with_capacity_in(self.requests.len() + 1, &ast);
        for request in &self.requests {
            if request.dynamic {
                let source = Expression::new_string_literal(
                    SPAN,
                    ast.allocator().alloc_str(&request.specifier),
                    None,
                    &ast,
                );
                let import = Expression::new_import_expression(SPAN, source, None, None, &ast);
                body.push(Statement::new_expression_statement(SPAN, import, &ast));
            } else {
                body.push(Statement::new_import_declaration(
                    SPAN,
                    None,
                    StringLiteral::new(
                        SPAN,
                        ast.allocator().alloc_str(&request.specifier),
                        None,
                        &ast,
                    ),
                    None,
                    None,
                    ImportOrExportKind::Value,
                    &ast,
                ));
            }
        }
        body.push(marker);

        let mut projection = Parser::new(allocator, "", SourceType::ts()).parse().program;
        projection.body = body;
        Codegen::new().build(&projection).code
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StaticArktsModuleRequest {
    specifier: String,
    dynamic: bool,
}

#[derive(Default)]
struct StaticArktsModuleRequestInventory {
    requests: Vec<StaticArktsModuleRequest>,
    seen: FxHashSet<StaticArktsModuleRequest>,
}

impl StaticArktsModuleRequestInventory {
    fn push(&mut self, source: &StringLiteral<'_>, dynamic: bool) {
        let request = StaticArktsModuleRequest {
            specifier: source.value.to_string(),
            dynamic,
        };
        if self.seen.insert(request.clone()) {
            self.requests.push(request);
        }
    }
}

impl<'a> Visit<'a> for StaticArktsModuleRequestInventory {
    fn visit_import_declaration(&mut self, declaration: &oxc_ast::ast::ImportDeclaration<'a>) {
        self.push(&declaration.source, false);
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_lazy_import_declaration(&mut self, declaration: &LazyImportDeclaration<'a>) {
        self.push(&declaration.source, false);
        walk::walk_lazy_import_declaration(self, declaration);
    }

    fn visit_export_all_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportAllDeclaration<'a>,
    ) {
        self.push(&declaration.source, false);
        walk::walk_export_all_declaration(self, declaration);
    }

    fn visit_export_from_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportFromDeclaration<'a>,
    ) {
        self.push(&declaration.source, false);
        walk::walk_export_from_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.push(source, true);
        }
        walk::walk_import_expression(self, expression);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_keeps_request_order_without_static_syntax() {
        let allocator = Allocator::default();
        let source = r#"
import { value } from "./static"
export * from "./reexport"
@Component struct Page {
  build() {}
}
"#;
        let parsed = Parser::new(&allocator, source, SourceType::ets_static()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

        let projection = StaticArktsGraphProjection::new(&parsed.program).project(&allocator);
        assert_eq!(
            projection,
            "import \"./static\";\nimport \"./reexport\";\nexport {};\n"
        );
        assert!(!projection.contains("struct Page"));
    }

    #[test]
    fn projection_preserves_dynamic_requests_when_a_plugin_ast_contains_one() {
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            "const deferred = import('./dynamic');",
            SourceType::ts(),
        )
        .parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

        let projection = StaticArktsGraphProjection::new(&parsed.program).project(&allocator);
        assert_eq!(projection, "import(\"./dynamic\");\nexport {};\n");
    }
}
