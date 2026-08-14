//! SDK `process_lazy_import.js::transformLazyImport` equivalent.
//!
//! When `arkOptions.autoLazyImport` is enabled, value imports are rewritten
//! into `import lazy` declarations so the CompileArkTS cache source hands
//! lazy-loading records to es2abc. The Rolldown scan input is lowered back
//! to plain imports by `LazyImportRolldownAdapter`.

use std::borrow::Cow;

use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::{
    ImportDeclaration, ImportDeclarationSpecifier, LazyImportDeclaration, Program, Statement,
};
use oxc_ast::AstBuilder;
use oxc_codegen::{CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;

use crate::source::CompileArktsSourceType;
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::source_text::ViewpuOutputText;
use crate::transform::TransformOptions;

/// Parse `source`, rewrite its value imports into `import lazy`, and print
/// the result. Returns the input unchanged when the rewrite does not apply.
///
/// Runs after kit expansion (SDK ordering: `process_kit_import` type-only
/// classification, then `process_lazy_import` — type-only imports stay plain).
/// The hvigor printer is used so numeric/string literal spelling survives.
pub(crate) fn apply<'a>(source: &'a str, id: &str, options: &TransformOptions) -> Cow<'a, str> {
    if !options.auto_lazy_import || is_not_auto_lazy_import(options) {
        return Cow::Borrowed(source);
    }
    let allocator = Allocator::default();
    let source_type = CompileArktsSourceType::from_id(id);
    let mut parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() || !lower_program(&mut parsed.program, &allocator, options) {
        return Cow::Borrowed(source);
    }
    let code = HvigorTypescriptPrinter::print(
        &parsed.program,
        CodegenOptions {
            comments: CommentOptions::disabled(),
            source_map_path: None,
            indent_char: IndentChar::Space,
            indent_width: 4,
            ..CodegenOptions::default()
        },
    );
    Cow::Owned(ViewpuOutputText::new(code).without_extra_trailing_blank_lines())
}

/// Rewrites the value imports of `program` into `import lazy` declarations.
///
/// Mirrors `process_lazy_import.js`:
/// - the whole file is skipped when `isNotAutoLazyImport` excludes the
///   owning module's package name (`autoLazyFilter.include` / `.exclude`);
/// - type-only imports, namespace imports, and `.json` sources are kept as-is;
/// - named specifiers that are never referenced are eliminated
///   (`eliminateTypeSymbol` via the semantic reference set).
pub(crate) fn lower_program<'a>(
    program: &mut Program<'a>,
    allocator: &'a Allocator,
    options: &TransformOptions,
) -> bool {
    if !options.auto_lazy_import || is_not_auto_lazy_import(options) {
        return false;
    }

    let scoping = {
        let semantic_ret = SemanticBuilder::new().build(program);
        if !semantic_ret.diagnostics.is_empty() {
            return false;
        }
        semantic_ret.semantic.into_scoping()
    };

    let ast = AstBuilder::new(allocator);
    let mut changed = false;
    for statement in &mut program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(lazy) = lazy_import(import, &scoping, &ast) else {
            continue;
        };
        *statement = Statement::LazyImportDeclaration(lazy);
        changed = true;
    }
    changed
}

/// `isNotAutoLazyImport(pkgName, filter)` from `process_lazy_import.js`:
/// an explicit `include` list restricts the transform to those packages; an
/// `exclude` list (with no `include`) skips the listed packages. An absent
/// filter object never skips.
fn is_not_auto_lazy_import(options: &TransformOptions) -> bool {
    let Some(pkg_name) = options.pkg_name.as_deref() else {
        return false;
    };
    let Some(filter) = options.auto_lazy_filter.as_ref() else {
        return false;
    };
    if !filter.include.is_empty() {
        return !filter.include.iter().any(|name| name == pkg_name);
    }
    !filter.exclude.is_empty() && filter.exclude.iter().any(|name| name == pkg_name)
}

fn lazy_import<'a>(
    import: &ImportDeclaration<'a>,
    scoping: &oxc_semantic::Scoping,
    ast: &AstBuilder<'a>,
) -> Option<ArenaBox<'a, LazyImportDeclaration<'a>>> {
    if import.import_kind.is_type() {
        return None;
    }
    if import.source.value.ends_with(".json") {
        return None;
    }
    let specifiers = import.specifiers.as_ref()?;
    if specifiers.iter().any(|specifier| {
        matches!(
            specifier,
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_)
        )
    }) {
        return None;
    }

    let mut kept: oxc_allocator::Vec<'_, ImportDeclarationSpecifier<'_>> =
        ast.vec_with_capacity(specifiers.len());
    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                if is_referenced(specifier.local.symbol_id.get(), scoping) {
                    kept.push(ImportDeclarationSpecifier::ImportSpecifier(
                        specifier.clone_in(ast.allocator),
                    ));
                }
            }
            _ => kept.push(specifier.clone_in(ast.allocator)),
        }
    }
    if kept.is_empty() {
        return None;
    }

    Some(
        ast.alloc_lazy_import_declaration(
            import.span,
            Some(kept),
            import.source.clone_in(ast.allocator),
            import
                .with_clause
                .as_ref()
                .map(|with_clause| with_clause.clone_in(ast.allocator)),
        ),
    )
}

/// `meta.isReferencedAliasDeclaration` equivalent: the binding symbol has at
/// least one resolved reference beyond the import declaration itself.
fn is_referenced(
    symbol_id: Option<oxc_syntax::symbol::SymbolId>,
    scoping: &oxc_semantic::Scoping,
) -> bool {
    let Some(symbol_id) = symbol_id else {
        // No semantic binding (unresolved): keep the specifier — the SDK
        // checker would also keep unresolved aliases.
        return true;
    };
    !scoping.get_resolved_reference_ids(symbol_id).is_empty()
}

use oxc_allocator::ArenaBox;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::CompileArktsSourceType;
    use oxc_parser::Parser;

    fn transform(source: &str, options: &TransformOptions) -> String {
        let allocator = Allocator::default();
        let mut parsed = Parser::new(
            &allocator,
            source,
            CompileArktsSourceType::from_id("src/main/ets/Test.ets"),
        )
        .parse();
        lower_program(&mut parsed.program, &allocator, options);
        oxc_codegen::Codegen::new().build(&parsed.program).code
    }

    fn options(auto_lazy: bool) -> TransformOptions {
        TransformOptions {
            auto_lazy_import: auto_lazy,
            pkg_name: Some("entry".to_string()),
            ..TransformOptions::default()
        }
    }

    #[test]
    fn value_imports_become_lazy() {
        let source = "import { foo } from \"@ohos/pkg\";\nimport def from \"./local\";\nconsole.log(foo, def);";
        let out = transform(source, &options(true));
        assert!(
            out.contains("import lazy { foo } from \"@ohos/pkg\";"),
            "got: {out}"
        );
        assert!(
            out.contains("import lazy def from \"./local\";"),
            "got: {out}"
        );
    }

    #[test]
    fn type_only_and_namespace_imports_stay_plain() {
        let source = "import type { T } from \"@ohos/pkg\";\nimport * as ns from \"./ns\";\nconsole.log(ns);";
        let out = transform(source, &options(true));
        assert!(out.contains("import type { T }"), "got: {out}");
        assert!(out.contains("import * as ns"), "got: {out}");
        assert!(!out.contains("import lazy"), "got: {out}");
    }

    #[test]
    fn json_imports_stay_plain() {
        let source = "import data from \"./data.json\";\nconsole.log(data);";
        let out = transform(source, &options(true));
        assert!(out.contains("import data from"), "got: {out}");
        assert!(!out.contains("import lazy"), "got: {out}");
    }

    #[test]
    fn unreferenced_named_specifiers_are_eliminated() {
        let source = "import { used, unused } from \"@ohos/pkg\";\nconsole.log(used);";
        let out = transform(source, &options(true));
        assert!(out.contains("import lazy { used }"), "got: {out}");
        assert!(!out.contains("unused"), "got: {out}");
    }

    #[test]
    fn disabled_option_keeps_imports_plain() {
        let source = "import { foo } from \"@ohos/pkg\";\nconsole.log(foo);";
        let out = transform(source, &options(false));
        assert!(!out.contains("import lazy"), "got: {out}");
    }

    #[test]
    fn include_filter_restricts_packages() {
        let mut options = options(true);
        options.auto_lazy_filter = Some(arkdown_core::AutoLazyFilter {
            include: vec!["@ohos/library".to_string()],
            exclude: Vec::new(),
        });
        // Current package "entry" is not in include → skip whole file.
        let source = "import { foo } from \"@ohos/pkg\";\nconsole.log(foo);";
        let out = transform(source, &options);
        assert!(!out.contains("import lazy"), "got: {out}");

        options.pkg_name = Some("@ohos/library".to_string());
        let out = transform(source, &options);
        assert!(out.contains("import lazy { foo }"), "got: {out}");
    }

    #[test]
    fn exclude_filter_skips_packages() {
        let mut options = options(true);
        options.auto_lazy_filter = Some(arkdown_core::AutoLazyFilter {
            include: Vec::new(),
            exclude: vec!["entry".to_string()],
        });
        let source = "import { foo } from \"@ohos/pkg\";\nconsole.log(foo);";
        let out = transform(source, &options);
        assert!(!out.contains("import lazy"), "got: {out}");
    }
}
