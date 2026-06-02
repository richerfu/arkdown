use std::path::{Path, PathBuf};
use rustc_hash::{FxHashMap, FxHashSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Function, ImportDeclarationSpecifier,
    ModuleExportName, Program, Statement, StructElement, StructStatement, TSLiteral, TSType,
    TSTypeName,
};
use oxc_parser::Parser;
use oxc_span::SourceType;

use super::{decorator_name, is_component_struct, TransformOptions};
use crate::transform::build_fn::{CustomComponentInfo, CustomComponentUpdateProps};

pub(super) fn collect_custom_component_names(
    statement: &Statement<'_>,
    names: &mut FxHashSet<String>,
) {
    match statement {
        Statement::StructStatement(statement) => {
            names.insert(statement.id.name.to_string());
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                names.insert(statement.id.name.to_string());
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                names.insert(statement.id.name.to_string());
            }
        }
        _ => {}
    }
}

pub(super) fn collect_local_component_update_props(
    statement: &Statement<'_>,
    update_props: &mut CustomComponentUpdateProps,
) {
    match statement {
        Statement::StructStatement(statement) if is_component_struct(statement) => {
            update_props.insert(statement.id.name.to_string(), component_info(statement));
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                if is_component_struct(statement) {
                    update_props.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                if is_component_struct(statement) {
                    update_props.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        _ => {}
    }
}

pub(super) fn collect_imported_component_names(
    program: &Program<'_>,
    id: &str,
    options: &TransformOptions,
    names: &mut FxHashSet<String>,
    update_props: &mut CustomComponentUpdateProps,
) {
    let importer = Path::new(id);
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(target) = resolve_import_target(import.source.value.as_str(), importer, options)
        else {
            continue;
        };
        let target_components = exported_component_names(&target, &mut FxHashSet::default());
        if target_components.is_empty() {
            continue;
        }
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported = module_export_name_value(&specifier.imported);
                    let local = specifier.local.name.to_string();
                    if imported == "default" && target_components.has_default {
                        names.insert(local.clone());
                        update_props.insert(local, target_components.default_info.clone());
                    } else if let Some(props) = target_components.named.get(&imported) {
                        names.insert(local.clone());
                        update_props.insert(local, props.clone());
                    }
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    if target_components.has_default {
                        let local = specifier.local.name.to_string();
                        names.insert(local.clone());
                        update_props.insert(local, target_components.default_info.clone());
                    } else if let Some(props) = target_components.named.get("default") {
                        let local = specifier.local.name.to_string();
                        names.insert(local.clone());
                        update_props.insert(local, props.clone());
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
    }
}

pub(super) fn collect_builder_function_names(
    program: &Program<'_>,
    id: &str,
    options: &TransformOptions,
) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function)
                if function_has_decorator(function, "Builder") =>
            {
                if let Some(function_id) = &function.id {
                    names.insert(function_id.name.to_string());
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                    if function_has_decorator(function, "Builder")
                        || export
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "Builder")
                    {
                        if let Some(function_id) = &function.id {
                            names.insert(function_id.name.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let importer = Path::new(id);
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(target) = resolve_import_target(import.source.value.as_str(), importer, options)
        else {
            continue;
        };
        let target_builders = exported_builder_function_names(&target, &mut FxHashSet::default());
        if target_builders.is_empty() {
            continue;
        }
        for specifier in specifiers {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            let imported = module_export_name_value(&specifier.imported);
            if target_builders.contains(&imported) {
                names.insert(specifier.local.name.to_string());
            }
        }
    }
    names
}

fn function_has_decorator(function: &Function<'_>, name: &str) -> bool {
    function
        .decorators
        .iter()
        .any(|decorator| decorator_name(decorator) == name)
}

fn component_update_prop_names(statement: &StructStatement<'_>) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| matches!(decorator_name(decorator), "Prop" | "ObjectLink"))
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn component_link_prop_names(statement: &StructStatement<'_>) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Link")
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn component_info(statement: &StructStatement<'_>) -> CustomComponentInfo {
    CustomComponentInfo {
        update_props: component_update_prop_names(statement),
        link_props: component_link_prop_names(statement),
        reusable: statement
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Reusable"),
    }
}

pub(super) fn collect_struct_builder_method_names(
    statement: &StructStatement<'_>,
) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        if !method
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Builder")
        {
            continue;
        }
        if let Some(name) = method.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

pub(super) fn collect_struct_builder_param_names(
    statement: &StructStatement<'_>,
) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "BuilderParam")
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

pub(super) struct SimpleTypeContext {
    enum_names: FxHashSet<String>,
}

impl SimpleTypeContext {
    /// Mirrors ets-loader `process_component_member.js`: primitive annotations
    /// are simple, and `TSTypeReference` is simple only when its identifier is
    /// present in the enum collection populated from local/imported AST exports.
    pub(super) fn from_program(
        program: &Program<'_>,
        id: &str,
        options: &TransformOptions,
    ) -> Self {
        let mut enum_names = local_enum_names(program);
        let importer = Path::new(id);

        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let Some(target) =
                resolve_import_target(import.source.value.as_str(), importer, options)
            else {
                continue;
            };
            let target_enums = exported_enum_names(&target, &mut FxHashSet::default());
            if target_enums.is_empty() {
                continue;
            }

            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = module_export_name_value(&specifier.imported);
                if target_enums.contains(&imported) {
                    enum_names.insert(specifier.local.name.to_string());
                }
            }
        }

        Self { enum_names }
    }

    pub(super) fn is_simple_type_annotation(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSStringKeyword(_)
            | TSType::TSNumberKeyword(_)
            | TSType::TSBooleanKeyword(_) => true,
            TSType::TSLiteralType(literal) => matches!(
                &literal.literal,
                TSLiteral::StringLiteral(_)
                    | TSLiteral::NumericLiteral(_)
                    | TSLiteral::BooleanLiteral(_)
            ),
            TSType::TSTypeReference(reference) => match &reference.type_name {
                TSTypeName::IdentifierReference(identifier) => {
                    self.enum_names.contains(identifier.name.as_str())
                }
                _ => false,
            },
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .all(|ty| self.is_simple_type_annotation(ty)),
            TSType::TSParenthesizedType(parenthesized) => {
                self.is_simple_type_annotation(&parenthesized.type_annotation)
            }
            _ => false,
        }
    }
}

fn local_enum_names(program: &Program<'_>) -> FxHashSet<String> {
    let mut enums = FxHashSet::default();
    for statement in &program.body {
        match statement {
            Statement::TSEnumDeclaration(declaration) => {
                enums.insert(declaration.id.name.to_string());
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                    enums.insert(declaration.id.name.to_string());
                }
            }
            _ => {}
        }
    }
    enums
}

fn exported_enum_names(path: &Path, visited: &mut FxHashSet<PathBuf>) -> FxHashSet<String> {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return FxHashSet::default();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return FxHashSet::default();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return FxHashSet::default();
    }

    let mut exports = FxHashSet::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                    exports.insert(declaration.id.name.to_string());
                }

                let Some(source) = &export.source else {
                    continue;
                };
                let Some(target) = resolve_relative_import(path.parent(), source.value.as_str())
                else {
                    continue;
                };
                let target_enums = exported_enum_names(&target, visited);
                for specifier in &export.specifiers {
                    let local = module_export_name_value(&specifier.local);
                    if target_enums.contains(&local) {
                        exports.insert(module_export_name_value(&specifier.exported));
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_enum_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

fn exported_builder_function_names(path: &Path, visited: &mut FxHashSet<PathBuf>) -> FxHashSet<String> {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return FxHashSet::default();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return FxHashSet::default();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return FxHashSet::default();
    }

    let mut exports = FxHashSet::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::FunctionDeclaration(function)
                if function_has_decorator(function, "Builder") =>
            {
                if let Some(function_id) = &function.id {
                    exports.insert(function_id.name.to_string());
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                    if function_has_decorator(function, "Builder")
                        || export
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "Builder")
                    {
                        if let Some(function_id) = &function.id {
                            exports.insert(function_id.name.to_string());
                        }
                    }
                }

                let Some(source) = &export.source else {
                    continue;
                };
                let Some(target) = resolve_relative_import(path.parent(), source.value.as_str())
                else {
                    continue;
                };
                let target_builders = exported_builder_function_names(&target, visited);
                for specifier in &export.specifiers {
                    let local = module_export_name_value(&specifier.local);
                    if target_builders.contains(&local) {
                        exports.insert(module_export_name_value(&specifier.exported));
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_builder_function_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

#[derive(Default)]
struct ExportedComponentNames {
    named: FxHashMap<String, CustomComponentInfo>,
    has_default: bool,
    default_info: CustomComponentInfo,
}

impl ExportedComponentNames {
    fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.has_default
    }

    fn extend(&mut self, other: ExportedComponentNames) {
        self.named.extend(other.named);
        if other.has_default {
            self.has_default = true;
            self.default_info = other.default_info;
        }
    }
}

fn exported_component_names(path: &Path, visited: &mut FxHashSet<PathBuf>) -> ExportedComponentNames {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return ExportedComponentNames::default();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return ExportedComponentNames::default();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return ExportedComponentNames::default();
    }

    let mut local_components = CustomComponentUpdateProps::default();
    for statement in &parsed.program.body {
        collect_local_component_exports(statement, &mut local_components);
    }

    let mut exports = ExportedComponentNames::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    if is_component_struct(statement) {
                        exports
                            .named
                            .insert(statement.id.name.to_string(), component_info(statement));
                    }
                    continue;
                }

                if let Some(source) = &export.source {
                    let Some(target) =
                        resolve_relative_import(path.parent(), source.value.as_str())
                    else {
                        continue;
                    };
                    let target_components = exported_component_names(&target, visited);
                    for specifier in &export.specifiers {
                        let local = module_export_name_value(&specifier.local);
                        let exported = module_export_name_value(&specifier.exported);
                        let props = if local == "default" && target_components.has_default {
                            Some(&target_components.default_info)
                        } else {
                            target_components.named.get(&local)
                        };
                        if exported == "default" && props.is_some() {
                            exports.has_default = true;
                            exports.default_info = props.cloned().unwrap_or_default();
                        } else if let Some(props) = props {
                            exports.named.insert(exported, props.clone());
                        }
                    }
                } else {
                    for specifier in &export.specifiers {
                        let local = module_export_name_value(&specifier.local);
                        let exported = module_export_name_value(&specifier.exported);
                        if exported == "default" && local_components.contains_key(&local) {
                            exports.has_default = true;
                            exports.default_info =
                                local_components.get(&local).cloned().unwrap_or_default();
                        } else if let Some(props) = local_components.get(&local) {
                            exports.named.insert(exported, props.clone());
                        }
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    if is_component_struct(statement) {
                        exports.has_default = true;
                        exports.default_info = component_info(statement);
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_component_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

fn collect_local_component_exports(
    statement: &Statement<'_>,
    names: &mut CustomComponentUpdateProps,
) {
    match statement {
        Statement::StructStatement(statement) if is_component_struct(statement) => {
            names.insert(statement.id.name.to_string(), component_info(statement));
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                if is_component_struct(statement) {
                    names.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                if is_component_struct(statement) {
                    names.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        _ => {}
    }
}

fn resolve_import_target(
    specifier: &str,
    importer: &Path,
    options: &TransformOptions,
) -> Option<PathBuf> {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        return resolve_relative_import(importer.parent(), specifier);
    }

    let package_name = specifier
        .strip_prefix("@ohos/")
        .or_else(|| specifier.strip_prefix("@ohos:"))
        .unwrap_or(specifier)
        .split('/')
        .next()
        .unwrap_or(specifier);

    options.module_roots.iter().find_map(|(name, root)| {
        (name == package_name || specifier.ends_with(name))
            .then(|| module_entry_path(root))
            .flatten()
    })
}

fn module_entry_path(root: &Path) -> Option<PathBuf> {
    [
        root.join("Index.ets"),
        root.join("index.ets"),
        root.join("Index.ts"),
        root.join("index.ts"),
        root.join("src/main/ets/Index.ets"),
        root.join("src/main/ets/index.ets"),
        root.join("src/main/ets/Index.ts"),
        root.join("src/main/ets/index.ts"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn resolve_relative_import(base: Option<&Path>, specifier: &str) -> Option<PathBuf> {
    let candidate = base?.join(specifier);
    [
        candidate.clone(),
        candidate.with_extension("ets"),
        candidate.with_extension("ts"),
        candidate.with_extension("d.ets"),
        candidate.with_extension("d.ts"),
        candidate.join("Index.ets"),
        candidate.join("index.ets"),
        candidate.join("Index.ts"),
        candidate.join("index.ts"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn module_export_name_value(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}
