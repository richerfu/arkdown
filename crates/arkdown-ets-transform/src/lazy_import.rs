//! Lazy import lowering support.
//!
//! Corresponds to SDK `process_lazy_import.js`.

use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::{ImportOrExportKind, LazyImportDeclaration, Statement};
use oxc_ast::AstBuilder;

/// Rewrites `import lazy { ... } from '...'` to a standard import statement.
pub(crate) struct LazyImportTransform;

impl LazyImportTransform {
    pub(crate) fn import_statement<'a>(
        declaration: &LazyImportDeclaration<'_>,
        allocator: &'a Allocator,
    ) -> Statement<'a> {
        let ast = AstBuilder::new(allocator);
        Statement::ImportDeclaration(
            ast.alloc_import_declaration(
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
            ),
        )
    }
}
