//! Module-request rewriting for materialized CompileArkTS module sources.

use std::borrow::Cow;

use arkdown_ets_transform::{CompileArktsSourceType, SourceClassifier};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ImportExpression, StringLiteral};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;

use crate::chunk_module_request::HvigorChunkImportMap;

use super::*;

pub(crate) struct HvigorModuleSourceImportRewriter<'a> {
    pub(crate) source: &'a str,
    pub(crate) module_id: &'a str,
    pub(crate) imports: &'a HvigorChunkImportMap,
    pub(crate) request_rewrite_kind: HvigorModuleRequestRewriteKind,
}

impl<'a> HvigorModuleSourceImportRewriter<'a> {
    pub(crate) fn new(
        source: &'a str,
        module_id: &'a str,
        imports: &'a HvigorChunkImportMap,
    ) -> Self {
        Self {
            source,
            module_id,
            imports,
            request_rewrite_kind: HvigorModuleRequestRewriteKind::from_module_id(module_id),
        }
    }

    pub(crate) fn rewrite(&self) -> Cow<'a, str> {
        if self.imports.is_empty() {
            return Cow::Borrowed(self.source);
        }

        let mut patches = self.collect_module_request_literal_patches();
        if patches.is_empty() {
            return Cow::Borrowed(self.source);
        }

        patches.sort_by_key(|patch| (patch.start, patch.end));
        patches.dedup_by_key(|patch| (patch.start, patch.end));

        let mut rewritten = self.source.to_string();
        let mut previous_start = self.source.len();
        for patch in patches.into_iter().rev() {
            if patch.end > previous_start || patch.start >= patch.end || patch.end > rewritten.len()
            {
                continue;
            }
            previous_start = patch.start;
            rewritten.replace_range(patch.start..patch.end, &patch.quoted_replacement());
        }
        Cow::Owned(rewritten)
    }

    pub(crate) fn collect_module_request_literal_patches(
        &self,
    ) -> Vec<HvigorModuleSourceImportPatch> {
        HvigorModuleRequestSpecifiers::new(self.source, self.module_id)
            .collect_literals()
            .into_iter()
            .filter_map(|literal| {
                let replacement = self.imports.get(literal.specifier.as_str())?;
                if replacement == literal.specifier.as_str() {
                    return None;
                }
                let start = literal.source_span.start as usize;
                let end = literal.source_span.end as usize;
                let source_literal = self.source.get(start..end)?;
                Some(HvigorModuleSourceImportPatch {
                    start,
                    end,
                    replacement: replacement.clone(),
                    quote: self.request_rewrite_kind.source_quote(source_literal),
                })
            })
            .collect()
    }
}

/// Latest ets-loader uses two different module-request paths. Runtime
/// JavaScript takes the regular-expression path and retains its delimiter,
/// while transformed `.ets`/`.ts` is rewritten through
/// `ts.factory.createStringLiteral`, which emits a generated double-quoted
/// literal. The two paths must not be collapsed into one source-preservation
/// rule: es2abc includes source text in anonymous function identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HvigorModuleRequestRewriteKind {
    JavaScript,
    TransformedTypeScript,
}

impl HvigorModuleRequestRewriteKind {
    pub(crate) fn from_module_id(module_id: &str) -> Self {
        if SourceClassifier::classify(module_id).is_javascript_runtime() {
            Self::JavaScript
        } else {
            Self::TransformedTypeScript
        }
    }

    pub(crate) fn source_quote(self, source_literal: &str) -> Option<char> {
        if self != Self::JavaScript {
            return None;
        }
        match source_literal.as_bytes() {
            [b'\'', .., b'\''] => Some('\''),
            [b'"', .., b'"'] => Some('"'),
            _ => None,
        }
    }
}

/// AST inventory of requests which hvigor's `ModuleSourceFile` may rewrite.
///
/// `processTransformedTsModuleRequest` visits ESM static imports/exports and
/// `import()` expressions. It deliberately does not rewrite CommonJS
/// `require()` calls, whose relative runtime resolution must survive in the
/// materialized module source.
pub(crate) struct HvigorModuleRequestSpecifiers<'a> {
    pub(crate) source: &'a str,
    pub(crate) module_id: &'a str,
}

impl<'a> HvigorModuleRequestSpecifiers<'a> {
    pub(crate) fn new(source: &'a str, module_id: &'a str) -> Self {
        Self { source, module_id }
    }

    pub(crate) fn collect(&self) -> Vec<String> {
        self.collect_literals()
            .into_iter()
            .map(|literal| literal.specifier)
            .collect()
    }

    pub(crate) fn collect_literals(&self) -> Vec<HvigorModuleRequestLiteral> {
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id(self.module_id);
        let parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Vec::new();
        }
        let mut inventory = HvigorModuleRequestLiteralInventory::default();
        inventory.visit_program(&parsed.program);
        inventory.literals
    }
}

#[derive(Default)]
pub(crate) struct HvigorModuleRequestLiteralInventory {
    pub(crate) literals: Vec<HvigorModuleRequestLiteral>,
}

impl HvigorModuleRequestLiteralInventory {
    pub(crate) fn push(&mut self, literal: &StringLiteral<'_>, is_dynamic: bool) {
        self.literals.push(HvigorModuleRequestLiteral {
            specifier: literal.value.to_string(),
            source_span: HvigorSourceSpan {
                start: literal.span.start,
                end: literal.span.end,
            },
            is_dynamic,
        });
    }
}

impl<'a> Visit<'a> for HvigorModuleRequestLiteralInventory {
    fn visit_import_declaration(&mut self, declaration: &oxc_ast::ast::ImportDeclaration<'a>) {
        self.push(&declaration.source, false);
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_lazy_import_declaration(
        &mut self,
        declaration: &oxc_ast::ast::LazyImportDeclaration<'a>,
    ) {
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

    fn visit_export_named_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        walk::walk_export_named_declaration(self, declaration);
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

pub(crate) struct HvigorModuleRequestLiteral {
    pub(crate) specifier: String,
    pub(crate) source_span: HvigorSourceSpan,
    pub(crate) is_dynamic: bool,
}

pub(crate) struct HvigorModuleSourceImportPatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
    pub(crate) quote: Option<char>,
}

impl HvigorModuleSourceImportPatch {
    pub(crate) fn quoted_replacement(&self) -> String {
        let Some(quote) = self.quote else {
            return serde_json::to_string(&self.replacement)
                .expect("module request replacement must serialize as a JSON string");
        };

        // `module_source_file.js::processJsModuleRequest` retains the capture
        // from `(['"])` when it replaces a JavaScript request. Transformed
        // TypeScript deliberately leaves `quote` empty above and follows
        // `ts.factory.createStringLiteral` through the JSON double-quote path.
        // Apart from matching the source-cache text, this matters to es2abc:
        // anonymous-function names are derived from source text and a
        // different quote character changes those names even when the parsed
        // program is otherwise equal.
        let mut replacement = String::with_capacity(self.replacement.len() + 2);
        replacement.push(quote);
        for character in self.replacement.chars() {
            match character {
                '\\' => replacement.push_str("\\\\"),
                '\n' => replacement.push_str("\\n"),
                '\r' => replacement.push_str("\\r"),
                '\t' => replacement.push_str("\\t"),
                '\u{0008}' => replacement.push_str("\\b"),
                '\u{000c}' => replacement.push_str("\\f"),
                '\u{2028}' => replacement.push_str("\\u2028"),
                '\u{2029}' => replacement.push_str("\\u2029"),
                character if character == quote => {
                    replacement.push('\\');
                    replacement.push(character);
                }
                character if character.is_control() => {
                    use std::fmt::Write;

                    write!(&mut replacement, "\\u{:04x}", character as u32)
                        .expect("writing into a String cannot fail");
                }
                character => replacement.push(character),
            }
        }
        replacement.push(quote);
        replacement
    }
}
