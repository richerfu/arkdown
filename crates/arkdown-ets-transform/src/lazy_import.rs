//! Lazy import boundary support.
//!
//! Corresponds to SDK `process_lazy_import.js`.

use std::borrow::Cow;

use arkdown_core::ArktsLanguageVersion;
use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::{ImportOrExportKind, LazyImportDeclaration, Statement};
use oxc_ast::builder::AstBuilder;
use oxc_codegen::Codegen;
use oxc_parser::Parser;

use crate::source::{CompileArktsSourceType, SourceClassifier};

/// Adapts hvigor's `import lazy` TypeScript to the standard import syntax
/// understood by the current Rolldown scanner. The CompileArkTS module source
/// remains unchanged and is handed to es2abc separately.
pub(crate) struct LazyImportRolldownAdapter<'a> {
    source: &'a str,
    id: &'a str,
    arkts_language_version: ArktsLanguageVersion,
}

impl<'a> LazyImportRolldownAdapter<'a> {
    pub(crate) fn new(
        source: &'a str,
        id: &'a str,
        arkts_language_version: ArktsLanguageVersion,
    ) -> Self {
        Self {
            source,
            id,
            arkts_language_version,
        }
    }

    pub(crate) fn lower(&self) -> Cow<'a, str> {
        let allocator = Allocator::default();
        // Current ace_ets2bundle accepts `import lazy` in JavaScript through
        // `processJsCodeLazyImport(..., ScriptKind.JS)`. OXC exposes the same
        // extension through its ArkTS parser, while plain `SourceType::js()`
        // rejects it before this parser-only Rolldown adaptation can run.
        let source_type = if SourceClassifier::classify(self.id).is_javascript_runtime() {
            oxc_span::SourceType::ets()
        } else {
            CompileArktsSourceType::from_id_with_version(self.id, self.arkts_language_version)
        };
        let mut parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(self.source);
        }

        let replacements = parsed
            .program
            .body
            .iter()
            .map(|statement| matches!(statement, Statement::LazyImportDeclaration(_)))
            .collect::<Vec<_>>();
        if !replacements.iter().any(|replace| *replace) {
            return Cow::Borrowed(self.source);
        }

        let ast = AstBuilder::new(&allocator);
        let mut body = oxc_allocator::Vec::with_capacity_in(parsed.program.body.len(), &ast);
        let old_body =
            std::mem::replace(&mut parsed.program.body, oxc_allocator::Vec::new_in(&ast));
        for (statement, replace) in old_body.into_iter().zip(replacements) {
            if replace {
                let Statement::LazyImportDeclaration(declaration) = statement else {
                    unreachable!("lazy-import replacement plan must match its statement")
                };
                body.push(Self::import_statement(&declaration, &allocator));
            } else {
                body.push(statement);
            }
        }
        parsed.program.body = body;
        Cow::Owned(Codegen::new().build(&parsed.program).code)
    }

    fn import_statement<'ast>(
        declaration: &LazyImportDeclaration<'_>,
        allocator: &'ast Allocator,
    ) -> Statement<'ast> {
        let ast = AstBuilder::new(allocator);
        Statement::new_import_declaration(
            declaration.span,
            declaration
                .specifiers
                .as_ref()
                .map(|specifiers| specifiers.clone_in(allocator)),
            declaration.source.clone_in(allocator),
            None,
            declaration
                .with_clause
                .as_ref()
                .map(|with_clause| with_clause.clone_in(allocator)),
            ImportOrExportKind::Value,
            &ast,
        )
    }
}
