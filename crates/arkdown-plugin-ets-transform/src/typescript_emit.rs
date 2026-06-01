//! TypeScript emit helpers for CompileArkTS cache parity.
//!
//! Upstream `ets-loader` uses TypeScript Program emit in `etsTransform` for
//! files whose `shouldEmitJs` flag is true.  This module keeps that behavior in
//! the transform crate: first lower imported const-enum member reads, then let
//! OXC's TypeScript transformer strip types and local const enums.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, Expression, ImportDeclaration, ImportDeclarationSpecifier, Statement,
    TSModuleDeclarationBody, TSModuleDeclarationName,
};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_transformer::{TransformOptions, Transformer};

use crate::{apply_source_patches, module_export_name_text, SourcePatch};

#[derive(Debug, Clone, PartialEq)]
enum EnumValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Default)]
struct ConstEnumExports {
    enums: HashMap<String, HashMap<String, EnumValue>>,
    namespaces: HashMap<String, HashMap<String, HashMap<String, EnumValue>>>,
}

#[derive(Debug, Clone)]
enum ConstEnumImport {
    Enum(HashMap<String, EnumValue>),
    Namespace(HashMap<String, HashMap<String, EnumValue>>),
}

pub(crate) fn emit_javascript(source: &str, id: &str) -> Option<String> {
    let source = rewrite_imported_const_enums(source, id);
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let mut parsed = Parser::new(&allocator, &source, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }

    let scoping = {
        let semantic_ret = SemanticBuilder::new()
            .with_enum_eval(true)
            .build(&parsed.program);
        if !semantic_ret.errors.is_empty() {
            return None;
        }
        semantic_ret.semantic.into_scoping()
    };

    let mut options = TransformOptions::default();
    options.assumptions.set_public_class_fields = true;
    options.typescript.optimize_const_enums = true;
    options.typescript.remove_class_fields_without_initializer = true;
    let path = Path::new(id);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Transformer::new(&allocator, path, &options)
            .build_with_scoping(scoping, &mut parsed.program)
    }));
    if result.is_err() {
        return None;
    }

    Some(
        Codegen::new()
            .with_options(CodegenOptions::default())
            .build(&parsed.program)
            .code,
    )
}

pub(crate) fn rewrite_imported_const_enums(source: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let imports = collect_const_enum_imports(&parsed.program.body, id);
    if imports.is_empty() {
        return source.to_string();
    }

    let mut visitor = ConstEnumInlineVisitor {
        imports,
        patches: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    apply_source_patches(source, visitor.patches)
}

fn collect_const_enum_imports(
    statements: &[Statement<'_>],
    id: &str,
) -> HashMap<String, ConstEnumImport> {
    let mut imports = HashMap::new();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        collect_const_enum_import(import, id, &mut imports);
    }
    imports
}

fn collect_const_enum_import(
    import: &ImportDeclaration<'_>,
    id: &str,
    imports: &mut HashMap<String, ConstEnumImport>,
) {
    if import.import_kind.is_type() {
        return;
    }
    let Some(target) = resolve_relative_source(id, import.source.value.as_str()) else {
        return;
    };
    let Some(exports) = const_enum_exports(&target) else {
        return;
    };
    let Some(specifiers) = &import.specifiers else {
        return;
    };

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let imported = module_export_name_text(&specifier.imported);
                let local = specifier.local.name.to_string();
                if let Some(members) = exports.enums.get(&imported) {
                    imports.insert(local, ConstEnumImport::Enum(members.clone()));
                } else if let Some(namespace) = exports.namespaces.get(&imported) {
                    imports.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                }
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                imports.insert(
                    specifier.local.name.to_string(),
                    ConstEnumImport::Namespace(exports.enums.clone()),
                );
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {}
        }
    }
}

fn const_enum_exports(path: &Path) -> Option<ConstEnumExports> {
    let source = std::fs::read_to_string(path).ok()?;
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, &source, source_type).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let semantic_ret = SemanticBuilder::new()
        .with_enum_eval(true)
        .build(&parsed.program);
    if !semantic_ret.errors.is_empty() {
        return None;
    }
    let scoping = semantic_ret.semantic.scoping();

    let mut exports = ConstEnumExports::default();
    for statement in &parsed.program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(declaration) = &export.declaration else {
            continue;
        };
        match declaration {
            Declaration::TSEnumDeclaration(declaration) if declaration.r#const => {
                if let Some(members) = collect_enum_members(declaration, scoping) {
                    exports
                        .enums
                        .insert(declaration.id.name.to_string(), members);
                }
            }
            Declaration::TSModuleDeclaration(declaration) => {
                if let Some((name, namespace)) = collect_namespace_enums(declaration, scoping) {
                    exports.namespaces.insert(name, namespace);
                }
            }
            _ => {}
        }
    }
    Some(exports)
}

fn collect_namespace_enums(
    declaration: &oxc_ast::ast::TSModuleDeclaration<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<(String, HashMap<String, HashMap<String, EnumValue>>)> {
    let TSModuleDeclarationName::Identifier(name) = &declaration.id else {
        return None;
    };
    let Some(TSModuleDeclarationBody::TSModuleBlock(block)) = &declaration.body else {
        return None;
    };
    let mut enums = HashMap::new();
    for statement in &block.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(Declaration::TSEnumDeclaration(enum_decl)) = &export.declaration else {
            continue;
        };
        if enum_decl.r#const {
            if let Some(members) = collect_enum_members(enum_decl, scoping) {
                enums.insert(enum_decl.id.name.to_string(), members);
            }
        }
    }
    (!enums.is_empty()).then(|| (name.name.to_string(), enums))
}

fn collect_enum_members(
    declaration: &oxc_ast::ast::TSEnumDeclaration<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<HashMap<String, EnumValue>> {
    let enum_symbol = declaration.id.symbol_id();
    let enum_scope = scoping
        .get_enum_body_scopes(enum_symbol)?
        .first()
        .copied()?;
    let mut members = HashMap::new();
    for member in &declaration.body.members {
        let member_name = member.id.static_name().to_string();
        let symbol = scoping.get_binding(enum_scope, member_name.as_str().into())?;
        let value = scoping.get_enum_member_value(symbol)?;
        let value = match value {
            oxc_syntax::constant_value::ConstantValue::Number(number) => EnumValue::Number(*number),
            oxc_syntax::constant_value::ConstantValue::String(text) => {
                EnumValue::String(text.to_string())
            }
        };
        members.insert(member_name, value);
    }
    Some(members)
}

struct ConstEnumInlineVisitor {
    imports: HashMap<String, ConstEnumImport>,
    patches: Vec<SourcePatch>,
}

impl<'a> Visit<'a> for ConstEnumInlineVisitor {
    fn visit_expression(&mut self, expr: &Expression<'a>) {
        if let Some(replacement) = self.const_enum_replacement(expr) {
            self.patches.push(SourcePatch {
                start: expr.span().start as usize,
                end: expr.span().end as usize,
                replacement,
            });
            return;
        }
        walk::walk_expression(self, expr);
    }
}

impl ConstEnumInlineVisitor {
    fn const_enum_replacement(&self, expr: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = expr else {
            return None;
        };
        let member_name = member.property.name.as_str();

        if let Expression::Identifier(object) = &member.object {
            if let Some(ConstEnumImport::Enum(members)) = self.imports.get(object.name.as_str()) {
                return members.get(member_name).map(enum_value_source);
            }
        }

        let Expression::StaticMemberExpression(namespace_member) = &member.object else {
            return None;
        };
        let Expression::Identifier(namespace) = &namespace_member.object else {
            return None;
        };
        let enum_name = namespace_member.property.name.as_str();
        let ConstEnumImport::Namespace(enums) = self.imports.get(namespace.name.as_str())? else {
            return None;
        };
        enums
            .get(enum_name)
            .and_then(|members| members.get(member_name))
            .map(enum_value_source)
    }
}

fn enum_value_source(value: &EnumValue) -> String {
    match value {
        EnumValue::Number(number) => {
            if number.fract() == 0.0 {
                format!("{number:.0}")
            } else {
                number.to_string()
            }
        }
        EnumValue::String(text) => format!("{text:?}"),
    }
}

fn resolve_relative_source(id: &str, specifier: &str) -> Option<PathBuf> {
    if !specifier.starts_with('.') {
        return None;
    }
    let base = Path::new(id).parent()?;
    let candidate = base.join(specifier);
    [
        candidate.clone(),
        candidate.with_extension("ets"),
        candidate.with_extension("ts"),
        candidate.join("Index.ets"),
        candidate.join("index.ets"),
        candidate.join("Index.ts"),
        candidate.join("index.ts"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_js_and_inlines_imported_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("ErrorCode.ts");
        std::fs::write(
            &enum_path,
            "export const enum ErrorCode { OK, INTERNAL_ERROR = 1240001 }\n",
        )
        .unwrap();
        let source_path = dir.path().join("Use.ts");
        let source = "import { ErrorCode } from './ErrorCode';\nconst x: ErrorCode = ErrorCode.INTERNAL_ERROR;\n";
        let output = emit_javascript(source, source_path.to_str().unwrap()).unwrap();
        assert!(
            output.contains("1240001") && !output.contains("from './ErrorCode'"),
            "const enum import must be inlined and removed; got:\n{output}"
        );
        assert!(
            !output.contains(": ErrorCode"),
            "Type annotations must be stripped; got:\n{output}"
        );
    }

    #[test]
    fn inlines_namespace_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("Constant.ts");
        std::fs::write(
            &enum_path,
            "export namespace Constant { export const enum Key { MapProviderKey = 'provider' } }\n",
        )
        .unwrap();
        let source_path = dir.path().join("Use.ts");
        let source =
            "import { Constant } from './Constant';\nconst key = Constant.Key.MapProviderKey;\n";
        let output = emit_javascript(source, source_path.to_str().unwrap()).unwrap();
        assert!(
            output.contains("\"provider\"") && !output.contains("Constant.Key.MapProviderKey"),
            "namespace const enum must be inlined; got:\n{output}"
        );
    }

    #[test]
    fn emit_js_matches_typescript_class_field_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("Model.ts");
        let source =
            "class Model { value: string; count = 1; constructor() { this.value = 'ok'; } }\n";
        let output = emit_javascript(source, source_path.to_str().unwrap()).unwrap();
        assert!(
            !output.contains("value;"),
            "type-only class fields must be removed like TypeScript useDefineForClassFields=false; got:\n{output}"
        );
        assert!(
            output.contains("this.count = 1"),
            "initialized class fields must be lowered into constructor assignments; got:\n{output}"
        );
    }
}
