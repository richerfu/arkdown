use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, ExportNamedDeclaration, ExportSpecifier, ImportDeclaration,
    ImportDeclarationSpecifier, Statement, StringLiteral,
};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

use crate::{apply_source_patches, module_export_name_text, SourcePatch};

#[derive(Debug)]
struct ImportSpecText {
    text: String,
    is_type_only: bool,
}

pub(super) fn split_type_only_named_imports(source: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let semantic_ret = SemanticBuilder::new().build(&parsed.program);
    if !semantic_ret.errors.is_empty() {
        return source.to_string();
    }
    let scoping = semantic_ret.semantic.scoping();
    let mut patches = Vec::new();

    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(replacement) = split_import_declaration(import, scoping) else {
            continue;
        };
        patches.push(SourcePatch {
            start: import.span.start as usize,
            end: import.span.end as usize,
            replacement,
        });
    }

    apply_source_patches(source, patches)
}

fn split_import_declaration(
    import: &ImportDeclaration<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<String> {
    if import.import_kind.is_type() {
        return None;
    }
    let specifiers = import.specifiers.as_ref()?;
    let source_text = import.source.value.as_str();
    let quote = string_literal_quote(&import.source);
    let mut default_or_namespace = Vec::new();
    let mut named = Vec::new();
    let mut has_type_only_named = false;

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let text = import_specifier_text(specifier);
                let is_type_only = specifier.import_kind.is_type()
                    || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                        !scoping
                            .get_resolved_references(symbol_id)
                            .any(|reference| reference.flags().is_value())
                    });
                has_type_only_named |= is_type_only;
                named.push(ImportSpecText { text, is_type_only });
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                default_or_namespace.push(specifier.local.name.to_string());
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                default_or_namespace.push(format!("* as {}", specifier.local.name));
            }
        }
    }

    if !has_type_only_named {
        return None;
    }

    let value_named = named
        .iter()
        .filter(|specifier| !specifier.is_type_only)
        .map(|specifier| specifier.text.as_str())
        .collect::<Vec<_>>();
    let type_named = named
        .iter()
        .filter(|specifier| specifier.is_type_only)
        .map(|specifier| specifier.text.as_str())
        .collect::<Vec<_>>();

    let mut out = String::new();
    if !default_or_namespace.is_empty() || !value_named.is_empty() {
        out.push_str("import ");
        out.push_str(&default_or_namespace.join(", "));
        if !default_or_namespace.is_empty() && !value_named.is_empty() {
            out.push_str(", ");
        }
        if !value_named.is_empty() {
            out.push_str("{ ");
            out.push_str(&value_named.join(", "));
            out.push_str(" }");
        }
        out.push_str(" from ");
        out.push(quote);
        out.push_str(source_text);
        out.push(quote);
        out.push_str(";\n");
    }
    out.push_str("import type { ");
    out.push_str(&type_named.join(", "));
    out.push_str(" } from ");
    out.push(quote);
    out.push_str(source_text);
    out.push(quote);
    out.push(';');
    Some(out)
}

fn string_literal_quote(literal: &StringLiteral<'_>) -> char {
    literal
        .raw
        .as_ref()
        .and_then(|raw| raw.chars().next())
        .filter(|quote| *quote == '\'' || *quote == '"')
        .unwrap_or('"')
}

fn import_specifier_text(specifier: &oxc_ast::ast::ImportSpecifier<'_>) -> String {
    let imported = module_export_name_text(&specifier.imported);
    let local = specifier.local.name.to_string();
    if imported == local {
        local
    } else {
        format!("{imported} as {local}")
    }
}

/// Rewrites static `import` / `export ... from` module specifiers using OXC AST
/// spans.  This mirrors the TypeScript visitor shape used by hvigor's
/// `process_import` / `process_kit_import`: only module specifier nodes are
/// rewritten, while the surrounding declaration syntax is preserved.
pub fn rewrite_static_module_specifiers_with(
    source: &str,
    id: &str,
    mut rewrite: impl FnMut(&str) -> Option<String>,
) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let mut patches = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                if import.import_kind.is_type() && import.source.value.as_str().starts_with('.') {
                    continue;
                }
                push_module_specifier_patch(&import.source, &mut rewrite, &mut patches);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(source) = &export.source {
                    if export.export_kind.is_type() && source.value.as_str().starts_with('.') {
                        continue;
                    }
                    push_module_specifier_patch(source, &mut rewrite, &mut patches);
                }
            }
            Statement::ExportAllDeclaration(export) => {
                push_module_specifier_patch(&export.source, &mut rewrite, &mut patches);
            }
            _ => {}
        }
    }

    apply_source_patches(source, patches)
}

fn push_module_specifier_patch(
    literal: &StringLiteral<'_>,
    rewrite: &mut impl FnMut(&str) -> Option<String>,
    patches: &mut Vec<SourcePatch>,
) {
    let specifier = literal.value.as_str();
    let Some(replacement) = rewrite(specifier) else {
        return;
    };
    if replacement == specifier {
        return;
    }
    patches.push(SourcePatch {
        start: literal.span.start as usize,
        end: literal.span.end as usize,
        replacement: format!("\"{replacement}\""),
    });
}

pub(super) fn rewrite_type_only_reexports(source: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let mut patches = Vec::new();
    for statement in &parsed.program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(replacement) = rewrite_type_only_reexport(export, id) else {
            continue;
        };
        patches.push(SourcePatch {
            start: export.span.start as usize,
            end: export.span.end as usize,
            replacement,
        });
    }

    apply_source_patches(source, patches)
}

fn rewrite_type_only_reexport(export: &ExportNamedDeclaration<'_>, id: &str) -> Option<String> {
    if export.export_kind.is_type() || export.specifiers.is_empty() {
        return None;
    }
    let source = export.source.as_ref()?;
    let specifier = source.value.as_str();
    let target = resolve_relative_source(id, specifier)?;
    let target_exports = exported_binding_kinds(&target)?;

    let mut value_specifiers = Vec::new();
    let mut type_specifiers = Vec::new();

    for specifier in &export.specifiers {
        let local_name = module_export_name_text(&specifier.local);
        let specifier_text = export_specifier_text(specifier);
        let is_type = specifier.export_kind.is_type()
            || target_exports
                .get(&local_name)
                .is_some_and(|kind| *kind == ExportBindingKind::Type);
        if is_type {
            type_specifiers.push(specifier_text);
        } else {
            value_specifiers.push(specifier_text);
        }
    }

    if type_specifiers.is_empty() {
        return None;
    }

    let module_source = module_source_text(source);
    let mut replacement = String::new();
    if !value_specifiers.is_empty() {
        replacement.push_str("export { ");
        replacement.push_str(&value_specifiers.join(", "));
        replacement.push_str(" } from ");
        replacement.push_str(&module_source);
        replacement.push_str(";\n");
    }
    replacement.push_str("export type { ");
    replacement.push_str(&type_specifiers.join(", "));
    replacement.push_str(" } from ");
    replacement.push_str(&module_source);
    replacement.push(';');
    Some(replacement)
}

fn resolve_relative_source(id: &str, specifier: &str) -> Option<std::path::PathBuf> {
    let base = std::path::Path::new(id).parent()?;
    let candidate = base.join(specifier);
    let candidates = [
        candidate.clone(),
        candidate.with_extension("ets"),
        candidate.with_extension("ts"),
        candidate.join("Index.ets"),
        candidate.join("index.ets"),
        candidate.join("Index.ts"),
        candidate.join("index.ts"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportBindingKind {
    Type,
    Value,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn exported_binding_kinds(
    path: &std::path::Path,
) -> Option<HashMap<String, ExportBindingKind>> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return None;
    };
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }

    let mut exports = HashMap::new();
    for statement in &parsed.program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if let Some(declaration) = &export.declaration {
            collect_exported_declaration(declaration, &mut exports);
        }
        for specifier in &export.specifiers {
            let exported_name = module_export_name_text(&specifier.exported);
            let local_name = module_export_name_text(&specifier.local);
            if specifier.export_kind.is_type() || export.export_kind.is_type() {
                exports.insert(exported_name, ExportBindingKind::Type);
            } else if let Some(kind) = exports.get(&local_name).copied() {
                exports.insert(exported_name, kind);
            } else {
                exports
                    .entry(exported_name)
                    .or_insert(ExportBindingKind::Value);
            }
        }
    }
    Some(exports)
}

fn collect_exported_declaration(
    declaration: &Declaration<'_>,
    exports: &mut HashMap<String, ExportBindingKind>,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSEnumDeclaration(declaration) if declaration.r#const => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Type);
        }
        Declaration::TSEnumDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Value);
        }
        Declaration::FunctionDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                exports.insert(id.name.to_string(), ExportBindingKind::Value);
            }
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                exports.insert(id.name.to_string(), ExportBindingKind::Value);
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_names(&declarator.id, exports);
            }
        }
        Declaration::TSModuleDeclaration(_)
        | Declaration::TSGlobalDeclaration(_)
        | Declaration::StructStatement(_)
        | Declaration::AnnotationDeclaration(_) => {}
        Declaration::TSImportEqualsDeclaration(declaration) => {
            exports.insert(declaration.id.name.to_string(), ExportBindingKind::Value);
        }
    }
}

fn collect_binding_names(
    pattern: &BindingPattern<'_>,
    exports: &mut HashMap<String, ExportBindingKind>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            exports.insert(identifier.name.to_string(), ExportBindingKind::Value);
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_names(&property.value, exports);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, exports);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_names(element, exports);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, exports);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_names(&pattern.left, exports);
        }
    }
}

fn export_specifier_text(specifier: &ExportSpecifier<'_>) -> String {
    let local = module_export_name_text(&specifier.local);
    let exported = module_export_name_text(&specifier.exported);
    if local == exported {
        local
    } else {
        format!("{local} as {exported}")
    }
}

fn module_source_text(source: &oxc_ast::ast::StringLiteral<'_>) -> String {
    source.raw.as_ref().map_or_else(
        || format!("{:?}", source.value.as_str()),
        ToString::to_string,
    )
}
