use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::ModuleSpecifierText;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, Declaration, ExportDefaultDeclarationKind,
    Expression, Function, FunctionBody, ImportDeclarationSpecifier, ModuleExportName, Program,
    Statement, StructElement, StructStatement, TSLiteral, TSType, TSTypeName,
};
use oxc_parser::Parser;
use oxc_span::SourceType;

use super::{ArkuiDecorator, ArkuiDecoratorName, ArkuiDecoratorRules, TransformOptions};
use crate::source::TransformSourceFile;
use crate::transform::build_fn::{CustomComponentInfo, CustomComponentUpdateProps};
use crate::transform::component_meta::SdkComponentInfo;
use crate::transform::viewpu_emit::GeneratedViewpuName;

struct ComponentModuleExportName;

impl ComponentModuleExportName {
    const DEFAULT: &'static str = "default";

    fn value(name: &ModuleExportName<'_>) -> String {
        match name {
            ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
            ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
            ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
        }
    }
}

pub(super) struct CustomComponentNameCollector<'a> {
    names: &'a mut FxHashSet<String>,
}

impl<'a> CustomComponentNameCollector<'a> {
    pub(super) fn new(names: &'a mut FxHashSet<String>) -> Self {
        Self { names }
    }

    pub(super) fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement) => {
                self.names.insert(statement.id.name.to_string());
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    self.names.insert(statement.id.name.to_string());
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    self.names.insert(statement.id.name.to_string());
                }
            }
            _ => {}
        }
    }
}

pub(super) struct LocalComponentUpdatePropsCollector<'a> {
    update_props: &'a mut CustomComponentUpdateProps,
}

impl<'a> LocalComponentUpdatePropsCollector<'a> {
    pub(super) fn new(update_props: &'a mut CustomComponentUpdateProps) -> Self {
        Self { update_props }
    }

    pub(super) fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement)
                if ArkuiDecoratorRules::is_component_struct(statement) =>
            {
                self.insert(statement);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    if ArkuiDecoratorRules::is_component_struct(statement) {
                        self.insert(statement);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    if ArkuiDecoratorRules::is_component_struct(statement) {
                        self.insert(statement);
                    }
                }
            }
            _ => {}
        }
    }

    fn insert(&mut self, statement: &StructStatement<'_>) {
        self.update_props.insert(
            statement.id.name.to_string(),
            ComponentInfoCollector::new(statement).collect(),
        );
    }
}

pub(super) struct ImportedComponentNamesCollector<'a, 'b> {
    program: &'a Program<'a>,
    id: &'a str,
    options: &'a TransformOptions,
    names: &'b mut FxHashSet<String>,
    update_props: &'b mut CustomComponentUpdateProps,
}

pub(super) struct ImportedCustomComponentNamesCollector<'a, 'b> {
    program: &'a Program<'a>,
    id: &'a str,
    options: &'a TransformOptions,
    names: &'b mut FxHashSet<String>,
}

impl<'a, 'b> ImportedCustomComponentNamesCollector<'a, 'b> {
    pub(super) fn new(
        program: &'a Program<'a>,
        id: &'a str,
        options: &'a TransformOptions,
        names: &'b mut FxHashSet<String>,
    ) -> Self {
        Self {
            program,
            id,
            options,
            names,
        }
    }

    pub(super) fn collect(&mut self) {
        let importer = Path::new(self.id);
        let resolver =
            ImportTargetResolver::new(importer, &self.options.sources, &self.options.module_roots);
        let external_component_usages =
            ImportedArkuiComponentUsageCollector::collect_program(self.program);
        for statement in &self.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind.is_type() {
                continue;
            }
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let source = import.source.value.as_str();
            let Some(target) = resolver.resolve(source) else {
                self.collect_unresolved_external_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            };
            let target_components = self.options.module_exports.component_names(&target.path);
            if target_components.is_empty() {
                continue;
            }
            for specifier in specifiers {
                self.collect_specifier(specifier, &target_components);
            }
        }
    }

    fn collect_unresolved_external_specifiers(
        &mut self,
        source: &str,
        specifiers: &[ImportDeclarationSpecifier<'_>],
        external_component_usages: &FxHashSet<String>,
    ) {
        if ModuleSpecifierText::new(source).is_relative() {
            return;
        }
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier)
                    if !specifier.import_kind.is_type()
                        && external_component_usages.contains(specifier.local.name.as_str()) =>
                {
                    self.names.insert(specifier.local.name.to_string());
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier)
                    if external_component_usages.contains(specifier.local.name.as_str()) =>
                {
                    self.names.insert(specifier.local.name.to_string());
                }
                _ => {}
            }
        }
    }

    fn collect_specifier(
        &mut self,
        specifier: &ImportDeclarationSpecifier<'_>,
        target_components: &ExportedComponentNames,
    ) {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let imported = ComponentModuleExportName::value(&specifier.imported);
                let local = specifier.local.name.to_string();
                if imported == ComponentModuleExportName::DEFAULT && target_components.has_default {
                    self.names.insert(local);
                } else if target_components.named.contains_key(&imported) {
                    self.names.insert(local);
                }
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                let local = specifier.local.name.to_string();
                if target_components.has_default
                    || target_components
                        .named
                        .contains_key(ComponentModuleExportName::DEFAULT)
                {
                    self.names.insert(local);
                }
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
        }
    }
}

impl<'a, 'b> ImportedComponentNamesCollector<'a, 'b> {
    pub(super) fn new(
        program: &'a Program<'a>,
        id: &'a str,
        options: &'a TransformOptions,
        names: &'b mut FxHashSet<String>,
        update_props: &'b mut CustomComponentUpdateProps,
    ) -> Self {
        Self {
            program,
            id,
            options,
            names,
            update_props,
        }
    }

    pub(super) fn collect(&mut self) {
        let importer = Path::new(self.id);
        let resolver =
            ImportTargetResolver::new(importer, &self.options.sources, &self.options.module_roots);
        let external_component_usages =
            ImportedArkuiComponentUsageCollector::collect_program(self.program);
        for statement in &self.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind.is_type() {
                continue;
            }
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            self.collect_sdk_specifiers(import.source.value.as_str(), specifiers);
            let source = import.source.value.as_str();
            let Some(target) = resolver.resolve(source) else {
                self.collect_unresolved_external_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            };
            let target_components = self.options.module_exports.component_names(&target.path);
            if target_components.is_empty() {
                continue;
            }
            for specifier in specifiers {
                self.collect_specifier(specifier, &target_components);
            }
        }
    }

    fn collect_unresolved_external_specifiers(
        &mut self,
        source: &str,
        specifiers: &[ImportDeclarationSpecifier<'_>],
        external_component_usages: &FxHashSet<String>,
    ) {
        if ModuleSpecifierText::new(source).is_relative() {
            return;
        }
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier)
                    if !specifier.import_kind.is_type()
                        && external_component_usages.contains(specifier.local.name.as_str()) =>
                {
                    self.names.insert(specifier.local.name.to_string());
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier)
                    if external_component_usages.contains(specifier.local.name.as_str()) =>
                {
                    self.names.insert(specifier.local.name.to_string());
                }
                _ => {}
            }
        }
    }

    fn collect_sdk_specifiers(
        &mut self,
        source: &str,
        specifiers: &[ImportDeclarationSpecifier<'_>],
    ) {
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported = ComponentModuleExportName::value(&specifier.imported);
                    let local = specifier.local.name.to_string();
                    if let Some(info) = self
                        .options
                        .component_metadata
                        .sdk_component(source, &imported)
                    {
                        self.names.insert(local.clone());
                        self.update_props.insert(local, info.into());
                    }
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    let local = specifier.local.name.to_string();
                    if let Some(info) = self
                        .options
                        .component_metadata
                        .sdk_component(source, ComponentModuleExportName::DEFAULT)
                    {
                        self.names.insert(local.clone());
                        self.update_props.insert(local, info.into());
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
    }

    fn collect_specifier(
        &mut self,
        specifier: &ImportDeclarationSpecifier<'_>,
        target_components: &ExportedComponentNames,
    ) {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let imported = ComponentModuleExportName::value(&specifier.imported);
                let local = specifier.local.name.to_string();
                if imported == ComponentModuleExportName::DEFAULT && target_components.has_default {
                    self.names.insert(local.clone());
                    self.update_props
                        .insert(local, target_components.default_info.clone());
                } else if let Some(props) = target_components.named.get(&imported) {
                    self.names.insert(local.clone());
                    self.update_props.insert(local, props.clone());
                }
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                let local = specifier.local.name.to_string();
                if target_components.has_default {
                    self.names.insert(local.clone());
                    self.update_props
                        .insert(local, target_components.default_info.clone());
                } else if let Some(props) = target_components
                    .named
                    .get(ComponentModuleExportName::DEFAULT)
                {
                    self.names.insert(local.clone());
                    self.update_props.insert(local, props.clone());
                }
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
        }
    }
}

struct ImportedArkuiComponentUsageCollector {
    names: FxHashSet<String>,
}

impl ImportedArkuiComponentUsageCollector {
    fn collect_program(program: &Program<'_>) -> FxHashSet<String> {
        let mut collector = Self {
            names: FxHashSet::default(),
        };
        for statement in &program.body {
            collector.collect_statement(statement);
        }
        collector.names
    }

    fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement) => {
                self.collect_struct(statement);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    self.collect_struct(statement);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    self.collect_struct(statement);
                }
            }
            _ => {}
        }
    }

    fn collect_struct(&mut self, statement: &StructStatement<'_>) {
        for element in &statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            let Some(body) = method.value.body.as_deref() else {
                continue;
            };
            if method
                .key
                .static_name()
                .is_some_and(|name| name == GeneratedViewpuName::BUILD)
                || method.decorators.iter().any(|decorator| {
                    ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER
                })
            {
                self.collect_body(body);
            }
        }
    }

    fn collect_body(&mut self, body: &FunctionBody<'_>) {
        for statement in &body.statements {
            self.collect_body_statement(statement);
        }
    }

    fn collect_body_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_body_statement(statement);
                }
            }
            Statement::ExpressionStatement(statement) => {
                self.collect_expression(&statement.expression);
            }
            Statement::IfStatement(statement) => {
                self.collect_body_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.collect_body_statement(alternate);
                }
            }
            _ => {}
        }
    }

    fn collect_expression(&mut self, expression: &Expression<'_>) {
        if let Expression::ArkUIComponentExpression(component) = expression {
            self.collect_component_expression(component);
        }
    }

    fn collect_component_expression(&mut self, component: &ArkUIComponentExpression<'_>) {
        if let Expression::Identifier(identifier) = &component.callee {
            self.names.insert(identifier.name.to_string());
        }
        for argument in &component.arguments {
            self.collect_argument(argument);
        }
        for child in &component.children {
            match child {
                ArkUIChild::Component(component) => {
                    self.collect_component_expression(component);
                }
                ArkUIChild::Expression(expression) => {
                    self.collect_expression(expression);
                }
                ArkUIChild::Statement(statement) => {
                    self.collect_body_statement(statement);
                }
            }
        }
    }

    fn collect_argument(&mut self, argument: &Argument<'_>) {
        let Some(expression) = argument.as_expression() else {
            return;
        };
        if let Expression::ArrowFunctionExpression(arrow) = expression {
            self.collect_body(&arrow.body);
        }
    }
}

impl From<&SdkComponentInfo> for CustomComponentInfo {
    fn from(info: &SdkComponentInfo) -> Self {
        Self {
            update_props: info.update_props.clone(),
            link_props: info.link_props.clone(),
            reusable: info.reusable,
            is_custom_dialog: info.is_custom_dialog,
        }
    }
}

pub(super) struct BuilderFunctionNameCollector<'a> {
    program: &'a Program<'a>,
    id: &'a str,
    options: &'a TransformOptions,
}

impl<'a> BuilderFunctionNameCollector<'a> {
    pub(super) fn new(
        program: &'a Program<'a>,
        id: &'a str,
        options: &'a TransformOptions,
    ) -> Self {
        Self {
            program,
            id,
            options,
        }
    }

    pub(super) fn collect(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        self.collect_local(&mut names);
        self.collect_imported(&mut names);
        names
    }

    fn collect_local(&self, names: &mut FxHashSet<String>) {
        for statement in &self.program.body {
            match statement {
                Statement::FunctionDeclaration(function)
                    if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER) =>
                {
                    if let Some(function_id) = &function.id {
                        names.insert(function_id.name.to_string());
                    }
                }
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                        if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER)
                            || export.decorators.iter().any(|d| {
                                ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER
                            })
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
    }

    fn collect_imported(&self, names: &mut FxHashSet<String>) {
        let importer = Path::new(self.id);
        let resolver =
            ImportTargetResolver::new(importer, &self.options.sources, &self.options.module_roots);
        for statement in &self.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let Some(target) = resolver.resolve(import.source.value.as_str()) else {
                continue;
            };
            let target_builders = self
                .options
                .module_exports
                .builder_function_names(&target.path);
            if target_builders.is_empty() {
                continue;
            }
            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = ComponentModuleExportName::value(&specifier.imported);
                if target_builders.contains(&imported) {
                    names.insert(specifier.local.name.to_string());
                }
            }
        }
    }
}

struct FunctionDecorators<'a> {
    function: &'a Function<'a>,
}

impl<'a> FunctionDecorators<'a> {
    fn new(function: &'a Function<'a>) -> Self {
        Self { function }
    }

    fn has(&self, name: &str) -> bool {
        self.function
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }
}

struct ComponentInfoCollector<'a> {
    statement: &'a StructStatement<'a>,
}

impl<'a> ComponentInfoCollector<'a> {
    fn new(statement: &'a StructStatement<'a>) -> Self {
        Self { statement }
    }

    fn collect(&self) -> CustomComponentInfo {
        CustomComponentInfo {
            update_props: self.update_prop_names(),
            link_props: self.link_prop_names(),
            reusable: self.statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE
            }),
            is_custom_dialog: self.statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_DIALOG
            }),
        }
    }

    fn update_prop_names(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            if !property.decorators.iter().any(|decorator| {
                ArkuiDecoratorName::is_custom_component_update_prop(
                    ArkuiDecorator::new(decorator).name(),
                )
            }) {
                continue;
            }
            if let Some(name) = property.key.static_name() {
                names.insert(name.into_owned());
            }
        }
        names
    }

    fn link_prop_names(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            if !property
                .decorators
                .iter()
                .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::LINK)
            {
                continue;
            }
            if let Some(name) = property.key.static_name() {
                names.insert(name.into_owned());
            }
        }
        names
    }
}

pub(super) struct StructBuilderMethodNames<'a> {
    statement: &'a StructStatement<'a>,
}

impl<'a> StructBuilderMethodNames<'a> {
    pub(super) fn new(statement: &'a StructStatement<'a>) -> Self {
        Self { statement }
    }

    pub(super) fn collect(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::MethodDefinition(method) = element else {
                continue;
            };
            if !method.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER
            }) {
                continue;
            }
            if let Some(name) = method.key.static_name() {
                names.insert(name.into_owned());
            }
        }
        names
    }
}

pub(super) struct StructBuilderParamNames<'a> {
    statement: &'a StructStatement<'a>,
}

impl<'a> StructBuilderParamNames<'a> {
    pub(super) fn new(statement: &'a StructStatement<'a>) -> Self {
        Self { statement }
    }

    pub(super) fn collect(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            if !property.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER_PARAM
            }) {
                continue;
            }
            if let Some(name) = property.key.static_name() {
                names.insert(name.into_owned());
            }
        }
        names
    }
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
        let mut enum_names = LocalEnumNamesCollector::new(program).collect();
        let importer = Path::new(id);
        let resolver = ImportTargetResolver::new(importer, &options.sources, &options.module_roots);

        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let Some(target) = resolver.resolve(import.source.value.as_str()) else {
                continue;
            };
            let target_enums = options.module_exports.enum_names(&target.path);
            if target_enums.is_empty() {
                continue;
            }

            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = ComponentModuleExportName::value(&specifier.imported);
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

struct LocalEnumNamesCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> LocalEnumNamesCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut enums = FxHashSet::default();
        for statement in &self.program.body {
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
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ModuleExportIndex {
    entries: Arc<FxHashMap<PathBuf, ModuleExportEntry>>,
}

impl ModuleExportIndex {
    pub(crate) fn build(
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
    ) -> Self {
        let mut analyzer = ModuleExportAnalyzer::new(sources, module_roots);
        let mut entries = FxHashMap::default();
        for source in sources.source_files() {
            let path = source.path.clone();
            entries.insert(path, analyzer.export_entry(source));
        }
        Self {
            entries: Arc::new(entries),
        }
    }

    fn enum_names(&self, path: &Path) -> FxHashSet<String> {
        self.entries
            .get(path)
            .map(|entry| entry.enums.clone())
            .unwrap_or_default()
    }

    fn builder_function_names(&self, path: &Path) -> FxHashSet<String> {
        self.entries
            .get(path)
            .map(|entry| entry.builders.clone())
            .unwrap_or_default()
    }

    fn component_names(&self, path: &Path) -> ExportedComponentNames {
        self.entries
            .get(path)
            .map(|entry| entry.components.clone())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default)]
struct ModuleExportEntry {
    enums: FxHashSet<String>,
    builders: FxHashSet<String>,
    components: ExportedComponentNames,
}

struct ModuleExportAnalyzer<'a> {
    sources: &'a crate::source::TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
    active: FxHashSet<std::path::PathBuf>,
    entry_cache: FxHashMap<std::path::PathBuf, ModuleExportEntry>,
}

impl<'a> ModuleExportAnalyzer<'a> {
    fn new(
        sources: &'a crate::source::TransformSourceSnapshot,
        module_roots: &'a [(String, PathBuf)],
    ) -> Self {
        Self {
            sources,
            module_roots,
            active: FxHashSet::default(),
            entry_cache: FxHashMap::default(),
        }
    }

    fn export_entry(&mut self, source: TransformSourceFile) -> ModuleExportEntry {
        let path = source.path.clone();
        if let Some(entry) = self.entry_cache.get(&path) {
            return entry.clone();
        }
        let Some(module) = self.module(source) else {
            return ModuleExportEntry::default();
        };

        let entry = module
            .with_program(|program| {
                let mut entry = ModuleExportEntry::default();
                let mut local_components = CustomComponentUpdateProps::default();
                for statement in &program.body {
                    match statement {
                        Statement::FunctionDeclaration(function)
                            if FunctionDecorators::new(function)
                                .has(ArkuiDecoratorName::BUILDER) =>
                        {
                            if let Some(function_id) = &function.id {
                                entry.builders.insert(function_id.name.to_string());
                            }
                        }
                        Statement::ExportNamedDeclaration(export) => {
                            if let Some(Declaration::TSEnumDeclaration(declaration)) =
                                &export.declaration
                            {
                                entry.enums.insert(declaration.id.name.to_string());
                            }
                            if let Some(Declaration::FunctionDeclaration(function)) =
                                &export.declaration
                            {
                                if FunctionDecorators::new(function)
                                    .has(ArkuiDecoratorName::BUILDER)
                                    || export.decorators.iter().any(|d| {
                                        ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER
                                    })
                                {
                                    if let Some(function_id) = &function.id {
                                        entry.builders.insert(function_id.name.to_string());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    LocalComponentExportCollector::new(&mut local_components)
                        .collect_statement(statement);
                }

                self.collect_imported_component_aliases(&module, program, &mut local_components);
                for statement in &program.body {
                    match statement {
                        Statement::ExportNamedDeclaration(export) => {
                            if let Some(Declaration::StructStatement(statement)) =
                                export.declaration.as_ref()
                            {
                                if ComponentExportRules::is_exported_component_struct(statement) {
                                    entry.components.named.insert(
                                        statement.id.name.to_string(),
                                        ComponentInfoCollector::new(statement).collect(),
                                    );
                                }
                                continue;
                            }

                            if let Some(source) = &export.source {
                                let Some(target) = module.import_target(
                                    source.value.as_str(),
                                    self.sources,
                                    self.module_roots,
                                ) else {
                                    continue;
                                };
                                let target_entry = self.export_entry(target);
                                for specifier in &export.specifiers {
                                    let local = ComponentModuleExportName::value(&specifier.local);
                                    let exported =
                                        ComponentModuleExportName::value(&specifier.exported);
                                    if target_entry.enums.contains(&local) {
                                        entry.enums.insert(exported.clone());
                                    }
                                    if target_entry.builders.contains(&local) {
                                        entry.builders.insert(exported.clone());
                                    }
                                    let props = if local == ComponentModuleExportName::DEFAULT
                                        && target_entry.components.has_default
                                    {
                                        Some(&target_entry.components.default_info)
                                    } else {
                                        target_entry.components.named.get(&local)
                                    };
                                    if exported == ComponentModuleExportName::DEFAULT
                                        && props.is_some()
                                    {
                                        entry.components.has_default = true;
                                        entry.components.default_info =
                                            props.cloned().unwrap_or_default();
                                    } else if let Some(props) = props {
                                        entry.components.named.insert(exported, props.clone());
                                    }
                                }
                            } else {
                                for specifier in &export.specifiers {
                                    let local = ComponentModuleExportName::value(&specifier.local);
                                    let exported =
                                        ComponentModuleExportName::value(&specifier.exported);
                                    if exported == ComponentModuleExportName::DEFAULT
                                        && local_components.contains_key(&local)
                                    {
                                        entry.components.has_default = true;
                                        entry.components.default_info = local_components
                                            .get(&local)
                                            .cloned()
                                            .unwrap_or_default();
                                    } else if let Some(props) = local_components.get(&local) {
                                        entry.components.named.insert(exported, props.clone());
                                    }
                                }
                            }
                        }
                        Statement::ExportDefaultDeclaration(export) => {
                            if let ExportDefaultDeclarationKind::StructStatement(statement) =
                                &export.declaration
                            {
                                if ComponentExportRules::is_exported_component_struct(statement) {
                                    entry.components.has_default = true;
                                    entry.components.default_info =
                                        ComponentInfoCollector::new(statement).collect();
                                }
                            }
                        }
                        Statement::ExportAllDeclaration(export) => {
                            let Some(target) = module.import_target(
                                export.source.value.as_str(),
                                self.sources,
                                self.module_roots,
                            ) else {
                                continue;
                            };
                            let target_entry = self.export_entry(target);
                            entry.enums.extend(target_entry.enums);
                            entry.builders.extend(target_entry.builders);
                            entry.components.extend(target_entry.components);
                        }
                        _ => {}
                    }
                }

                entry
            })
            .unwrap_or_default();
        self.finish_module(&path);
        self.entry_cache.insert(path, entry.clone());
        entry
    }

    fn collect_imported_component_aliases(
        &mut self,
        module: &AnalyzedModule,
        program: &Program<'_>,
        local_components: &mut CustomComponentUpdateProps,
    ) {
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let Some(target) = module.import_target(
                import.source.value.as_str(),
                self.sources,
                self.module_roots,
            ) else {
                continue;
            };
            let target_components = self.export_entry(target).components;
            if target_components.is_empty() {
                continue;
            }

            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        let imported = ComponentModuleExportName::value(&specifier.imported);
                        let props = if imported == ComponentModuleExportName::DEFAULT
                            && target_components.has_default
                        {
                            Some(&target_components.default_info)
                        } else {
                            target_components.named.get(&imported)
                        };
                        if let Some(props) = props {
                            local_components
                                .insert(specifier.local.name.to_string(), props.clone());
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        if target_components.has_default {
                            local_components.insert(
                                specifier.local.name.to_string(),
                                target_components.default_info.clone(),
                            );
                        } else if let Some(props) = target_components
                            .named
                            .get(ComponentModuleExportName::DEFAULT)
                        {
                            local_components
                                .insert(specifier.local.name.to_string(), props.clone());
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
                }
            }
        }
    }

    fn module(&mut self, source: TransformSourceFile) -> Option<AnalyzedModule> {
        let path = source.path.clone();
        if !self.active.insert(path) {
            return None;
        }

        Some(AnalyzedModule::new(source))
    }

    fn finish_module(&mut self, path: &Path) {
        self.active.remove(path);
    }
}

struct AnalyzedModule {
    path: std::path::PathBuf,
    source: std::sync::Arc<str>,
}

impl AnalyzedModule {
    fn new(source: TransformSourceFile) -> Self {
        Self {
            path: source.path,
            source: source.source,
        }
    }

    fn with_program<R>(&self, f: impl FnOnce(&Program<'_>) -> R) -> Option<R> {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&self.path).unwrap_or_else(|_| SourceType::ets());
        let parsed = Parser::new(&allocator, &self.source, source_type).parse();
        if parsed.diagnostics.is_empty() {
            Some(f(&parsed.program))
        } else {
            None
        }
    }

    fn import_target(
        &self,
        specifier: &str,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
    ) -> Option<TransformSourceFile> {
        ImportTargetResolver::new(&self.path, sources, module_roots).resolve(specifier)
    }
}

#[derive(Clone, Debug, Default)]
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

struct LocalComponentExportCollector<'a> {
    names: &'a mut CustomComponentUpdateProps,
}

impl<'a> LocalComponentExportCollector<'a> {
    fn new(names: &'a mut CustomComponentUpdateProps) -> Self {
        Self { names }
    }

    fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::StructStatement(statement)
                if ComponentExportRules::is_exported_component_struct(statement) =>
            {
                self.insert(statement);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    if ComponentExportRules::is_exported_component_struct(statement) {
                        self.insert(statement);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    if ComponentExportRules::is_exported_component_struct(statement) {
                        self.insert(statement);
                    }
                }
            }
            _ => {}
        }
    }

    fn insert(&mut self, statement: &StructStatement<'_>) {
        self.names.insert(
            statement.id.name.to_string(),
            ComponentInfoCollector::new(statement).collect(),
        );
    }
}

struct ComponentExportRules;

impl ComponentExportRules {
    fn is_exported_component_struct(statement: &StructStatement<'_>) -> bool {
        statement.decorators.iter().any(|decorator| {
            ArkuiDecoratorName::is_component_struct_or_dialog(ArkuiDecorator::new(decorator).name())
        })
    }
}

struct ImportTargetResolver<'a> {
    importer: &'a Path,
    sources: &'a crate::source::TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
}

impl<'a> ImportTargetResolver<'a> {
    fn new(
        importer: &'a Path,
        sources: &'a crate::source::TransformSourceSnapshot,
        module_roots: &'a [(String, PathBuf)],
    ) -> Self {
        Self {
            importer,
            sources,
            module_roots,
        }
    }

    fn resolve(&self, specifier: &str) -> Option<TransformSourceFile> {
        if ModuleSpecifierText::new(specifier).is_relative() {
            return self
                .sources
                .resolve_relative_with_declarations(self.importer, specifier);
        }

        if let Some(source) = self
            .sources
            .resolve_static_with_declarations(self.importer, specifier)
        {
            return Some(source);
        }

        self.module_roots.iter().find_map(|(name, root)| {
            ModuleSpecifierText::new(specifier)
                .matches_package_module_name(name)
                .then(|| self.sources.resolve_module_entry(root))
                .flatten()
        })
    }
}
