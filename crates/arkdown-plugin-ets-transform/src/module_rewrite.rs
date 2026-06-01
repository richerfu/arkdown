use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, ExportNamedDeclaration, ExportSpecifier, ImportDeclaration,
    ImportDeclarationSpecifier, Statement, StringLiteral,
};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

use crate::module_emit::{
    ExportBinding, ImportSplitEmitter, ModuleBindingName, ModuleStringLiteral, NamedImportBinding,
    ReExportSplitEmitter, ValueImportBinding,
};
use crate::{apply_source_patches, module_export_name_text, SourcePatch};

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
    let source = ModuleStringLiteral::from_string_literal(&import.source);
    let mut value_bindings = Vec::new();
    let mut type_bindings = Vec::new();
    let mut has_type_only_named = false;

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let named = NamedImportBinding {
                    imported: ModuleBindingName::from_module_export_name(&specifier.imported),
                    local: specifier.local.name.to_string(),
                };
                let is_type_only = specifier.import_kind.is_type()
                    || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                        !scoping
                            .get_resolved_references(symbol_id)
                            .any(|reference| reference.flags().is_value())
                    });
                has_type_only_named |= is_type_only;
                if is_type_only {
                    type_bindings.push(named);
                } else {
                    value_bindings.push(ValueImportBinding::Named(named));
                }
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                value_bindings.push(ValueImportBinding::Default(
                    specifier.local.name.to_string(),
                ));
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                value_bindings.push(ValueImportBinding::Namespace(
                    specifier.local.name.to_string(),
                ));
            }
        }
    }

    if !has_type_only_named {
        return None;
    }

    Some(ImportSplitEmitter::new(source, value_bindings, type_bindings).emit())
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
        let binding = export_specifier_binding(specifier);
        let is_type = specifier.export_kind.is_type()
            || target_exports
                .get(&local_name)
                .is_some_and(|kind| *kind == ExportBindingKind::Type);
        if is_type {
            type_specifiers.push(binding);
        } else {
            value_specifiers.push(binding);
        }
    }

    if type_specifiers.is_empty() {
        return None;
    }

    Some(
        ReExportSplitEmitter::new(
            ModuleStringLiteral::from_string_literal(source),
            value_specifiers,
            type_specifiers,
        )
        .emit(),
    )
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

fn export_specifier_binding(specifier: &ExportSpecifier<'_>) -> ExportBinding {
    ExportBinding::from_export_specifier(specifier)
}
