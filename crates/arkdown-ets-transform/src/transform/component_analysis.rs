use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::ModuleSpecifierText;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, CallExpression, Declaration, Decorator,
    ExportDefaultDeclarationKind, Expression, Function, FunctionBody, ImportDeclarationSpecifier,
    ModuleExportName, Program, Statement, StructElement, StructStatement, TSLiteral, TSType,
    TSTypeName,
};
use oxc_span::SourceType;
use rayon::prelude::*;

use super::{ArkuiDecorator, ArkuiDecoratorName, ArkuiDecoratorRules, TransformOptions};
use crate::source::TransformSourceFile;
use crate::transform::build_fn::{
    CustomComponentGeneration, CustomComponentInfo, CustomComponentUpdateProps,
};
use crate::transform::component_meta::SdkComponentInfo;
use crate::transform::viewpu_emit::GeneratedViewpuName;

#[derive(Clone, Debug, Default)]
pub(super) struct ExtendAttributeIndex {
    by_component: FxHashMap<String, FxHashSet<String>>,
}

impl ExtendAttributeIndex {
    pub(super) fn from_program(program: &Program<'_>) -> Self {
        let mut index = Self::default();
        for statement in &program.body {
            match statement {
                Statement::FunctionDeclaration(function) => {
                    index.collect_function(function, &[]);
                }
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::FunctionDeclaration(function)) =
                        export.declaration.as_ref()
                    {
                        index.collect_function(function, &export.decorators);
                    }
                }
                _ => {}
            }
        }
        index
    }

    pub(super) fn contains(&self, component: &str, attribute: &str) -> bool {
        self.by_component
            .get(component)
            .is_some_and(|attributes| attributes.contains(attribute))
    }

    fn collect_function(&mut self, function: &Function<'_>, extra_decorators: &[Decorator<'_>]) {
        let Some(name) = function.id.as_ref() else {
            return;
        };
        let Some(component) = function
            .decorators
            .iter()
            .chain(extra_decorators.iter())
            .find_map(|decorator| {
                let decorator = ArkuiDecorator::new(decorator);
                (decorator.name() == ArkuiDecoratorName::EXTEND)
                    .then(|| decorator.identifier_arg())
                    .flatten()
            })
        else {
            return;
        };
        self.by_component
            .entry(component)
            .or_default()
            .insert(name.name.to_string());
    }
}

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
            Statement::StructStatement(statement) if Self::is_component(statement) => {
                self.insert(statement);
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    if Self::is_component(statement) {
                        self.insert(statement);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    if Self::is_component(statement) {
                        self.insert(statement);
                    }
                }
            }
            _ => {}
        }
    }

    fn is_component(statement: &StructStatement<'_>) -> bool {
        ArkuiDecoratorRules::is_component_struct(statement)
            || ArkuiDecoratorRules::is_component_v2_struct(statement)
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
                self.collect_external_usage_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            };
            let target_components = self.options.module_exports.component_names(&target.path);
            if target_components.is_empty() {
                self.collect_external_usage_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            }
            for specifier in specifiers {
                self.collect_specifier(specifier, &target_components);
            }
        }
    }

    fn collect_external_usage_specifiers(
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
                self.collect_external_usage_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            };
            let target_components = self.options.module_exports.component_names(&target.path);
            if target_components.is_empty() {
                self.collect_external_usage_specifiers(
                    source,
                    specifiers,
                    &external_component_usages,
                );
                continue;
            }
            for specifier in specifiers {
                self.collect_specifier(specifier, &target_components);
            }
        }
    }

    fn collect_external_usage_specifiers(
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
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                self.collect_component_expression(component);
            }
            // OXC 0.140 keeps unresolved, non-built-in leaf calls as regular call
            // expressions inside an ArkUI child. BuildBodyLowerer treats this same
            // shape as a leaf component call, so import classification must mirror it.
            Expression::CallExpression(call) => {
                self.collect_call_expression(call);
            }
            _ => {}
        }
    }

    fn collect_call_expression(&mut self, call: &CallExpression<'_>) {
        match &call.callee {
            Expression::Identifier(identifier) => {
                self.names.insert(identifier.name.to_string());
            }
            Expression::StaticMemberExpression(member) => {
                if let Expression::CallExpression(inner) = &member.object {
                    self.collect_call_expression(inner);
                }
            }
            _ => {}
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
            builder_params: FxHashSet::default(),
            reusable: info.reusable,
            reusable_v2: false,
            is_custom_dialog: info.is_custom_dialog,
            generation: CustomComponentGeneration::V1,
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
            builder_params: StructBuilderParamNames::new(self.statement).collect(),
            reusable: self.statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE
            }),
            reusable_v2: self.statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE_V2
            }),
            is_custom_dialog: self.statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_DIALOG
            }),
            generation: if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
                CustomComponentGeneration::V2
            } else {
                CustomComponentGeneration::V1
            },
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
    simple_type_aliases: FxHashSet<String>,
    qualified_simple_type_names: FxHashMap<String, FxHashSet<String>>,
    authoritative_namespace_imports: FxHashSet<String>,
    non_simple_import_bindings: FxHashSet<String>,
    component_metadata: Arc<crate::transform::component_meta::ComponentMetadata>,
}

impl SimpleTypeContext {
    /// Mirrors ets-loader `process_component_member.js`: primitive annotations
    /// are simple, and `TSTypeReference` is simple when its identifier is in
    /// `validate_ui_syntax.enumCollection`.
    ///
    /// `process_import.js::processImportNode` discovers enums through the
    /// current file's import/export graph, but the SDK stores their unqualified
    /// names in a process-wide collection that is cleared only by plugin
    /// `cleanUp()`. With concurrent Rollup entry loading, unrelated same-named
    /// declarations can therefore change wrapper selection according to I/O
    /// scheduling. Resolve the current program's bindings structurally so
    /// parallel transforms remain deterministic while preserving the SDK's
    /// local, imported, re-exported, and declaration-file enum semantics.
    pub(super) fn from_program(
        program: &Program<'_>,
        id: &str,
        options: &TransformOptions,
    ) -> Self {
        let mut enum_names = LocalEnumNamesCollector::new(program).collect();
        let mut simple_type_aliases = LocalSimpleTypeAliasCollector::new(program).collect();
        let mut qualified_simple_type_names: FxHashMap<String, FxHashSet<String>> =
            FxHashMap::default();
        let mut authoritative_namespace_imports = FxHashSet::default();
        let mut non_simple_import_bindings = FxHashSet::default();
        let importer = Path::new(id);
        let resolver = ImportTargetResolver::new(importer, &options.sources, &options.module_roots);
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let import_source = import.source.value.as_str();
            let target = resolver.resolve(import_source);
            let target_enums = target
                .as_ref()
                .map(|target| options.module_exports.enum_names(&target.path))
                .unwrap_or_default();
            let target_aliases = target
                .as_ref()
                .map(|target| options.module_exports.simple_type_alias_names(&target.path))
                .unwrap_or_default();
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        let imported = ComponentModuleExportName::value(&specifier.imported);
                        let local = specifier.local.name.to_string();
                        let sdk_enum = options
                            .component_metadata
                            .is_sdk_enum_export(import_source, &imported);
                        if target_enums.contains(&imported) || sdk_enum {
                            enum_names.insert(specifier.local.name.to_string());
                        }
                        if target_aliases.contains(&imported) {
                            simple_type_aliases.insert(specifier.local.name.to_string());
                        }
                        if let Some(names) = options
                            .component_metadata
                            .sdk_namespace_enum_exports(import_source, &imported)
                        {
                            authoritative_namespace_imports.insert(local.clone());
                            qualified_simple_type_names
                                .entry(local.clone())
                                .or_default()
                                .extend(names.iter().cloned());
                        }
                        if !target_enums.contains(&imported)
                            && !target_aliases.contains(&imported)
                            && !sdk_enum
                            && !authoritative_namespace_imports.contains(&local)
                        {
                            non_simple_import_bindings.insert(local);
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        let local = specifier.local.name.to_string();
                        authoritative_namespace_imports.insert(local.clone());
                        let names = qualified_simple_type_names.entry(local).or_default();
                        names.extend(target_enums.iter().cloned());
                        names.extend(target_aliases.iter().cloned());
                        if let Some(sdk_names) = options
                            .component_metadata
                            .sdk_namespace_enum_exports(import_source, "default")
                        {
                            names.extend(sdk_names.iter().cloned());
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        let local = specifier.local.name.to_string();
                        if let Some(names) = options
                            .component_metadata
                            .sdk_namespace_enum_exports(import_source, "default")
                        {
                            authoritative_namespace_imports.insert(local.clone());
                            qualified_simple_type_names
                                .entry(local)
                                .or_default()
                                .extend(names.iter().cloned());
                        } else {
                            non_simple_import_bindings.insert(local);
                        }
                    }
                }
            }
        }

        Self {
            enum_names,
            simple_type_aliases,
            qualified_simple_type_names,
            authoritative_namespace_imports,
            non_simple_import_bindings,
            component_metadata: Arc::clone(&options.component_metadata),
        }
    }

    pub(super) fn is_simple_type_annotation(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSStringKeyword(_)
            | TSType::TSNumberKeyword(_)
            | TSType::TSBooleanKeyword(_) => true,
            TSType::TSTypeReference(reference) => {
                self.is_direct_enum_reference_name(&reference.type_name)
                    || self.is_type_checker_simple_reference_name(&reference.type_name)
            }
            // ets-loader getDeclarationType first checks only StringKeyword,
            // NumberKeyword, BooleanKeyword and enum declarations by syntax.
            // Every other annotation goes through TypeScript's TypeFlags path.
            _ => self.is_type_checker_simple(ty),
        }
    }

    fn is_type_checker_simple(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSTypeReference(reference) => {
                self.is_type_checker_simple_reference_name(&reference.type_name)
            }
            TSType::TSUnionType(union) => {
                let non_nullable = union
                    .types
                    .iter()
                    .filter(|ty| {
                        !matches!(ty, TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_))
                    })
                    .collect::<Vec<_>>();
                match non_nullable.as_slice() {
                    // utils.findNonNullType collapses a one-member nullable
                    // union. getDeclarationType then accepts only EnumLike;
                    // TypeScript represents `boolean` as two boolean literals,
                    // so it remains the one primitive exception.
                    [ty] => {
                        matches!(ty, TSType::TSBooleanKeyword(_)) || self.is_type_checker_enum(ty)
                    }
                    [] => false,
                    types => {
                        types.iter().all(|ty| self.is_basic_union_type(ty))
                            && !TypeScriptUnionNormalization::collapses_to_single_primitive(types)
                    }
                }
            }
            TSType::TSParenthesizedType(parenthesized) => {
                self.is_type_checker_simple(&parenthesized.type_annotation)
            }
            _ => false,
        }
    }

    fn is_basic_union_type(&self, ty: &TSType<'_>) -> bool {
        matches!(
            ty,
            TSType::TSStringKeyword(_)
                | TSType::TSNumberKeyword(_)
                | TSType::TSBooleanKeyword(_)
                | TSType::TSBigIntKeyword(_)
        ) || matches!(
            ty,
            TSType::TSLiteralType(literal)
                if matches!(
                    &literal.literal,
                    TSLiteral::StringLiteral(_)
                        | TSLiteral::NumericLiteral(_)
                        | TSLiteral::BooleanLiteral(_)
                        | TSLiteral::BigIntLiteral(_)
                )
        ) || self.is_type_checker_enum(ty)
    }

    fn is_type_checker_enum(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSTypeReference(reference) => {
                self.is_type_checker_simple_reference_name(&reference.type_name)
            }
            TSType::TSParenthesizedType(parenthesized) => {
                self.is_type_checker_enum(&parenthesized.type_annotation)
            }
            _ => false,
        }
    }

    fn is_direct_enum_reference_name(&self, type_name: &TSTypeName<'_>) -> bool {
        let name = match type_name {
            TSTypeName::IdentifierReference(identifier) => identifier.name.as_str(),
            // SDK getDeclarationType requires `ts.isIdentifier(typeName)`.
            TSTypeName::QualifiedName(_) | TSTypeName::ThisExpression(_) => return false,
        };
        self.enum_names.contains(name)
            || (!self.non_simple_import_bindings.contains(name)
                && self.component_metadata.is_known_enum(name))
    }

    fn is_type_checker_simple_reference_name(&self, type_name: &TSTypeName<'_>) -> bool {
        match type_name {
            TSTypeName::IdentifierReference(identifier) => {
                let name = identifier.name.as_str();
                self.enum_names.contains(name)
                    || self.simple_type_aliases.contains(name)
                    || (!self.non_simple_import_bindings.contains(name)
                        && self.component_metadata.is_known_enum(name))
            }
            TSTypeName::QualifiedName(qualified) => {
                let right = qualified.right.name.as_str();
                match &qualified.left {
                    TSTypeName::IdentifierReference(namespace) => {
                        let namespace = namespace.name.as_str();
                        if self.authoritative_namespace_imports.contains(namespace) {
                            return self
                                .qualified_simple_type_names
                                .get(namespace)
                                .is_some_and(|names| names.contains(right));
                        }
                        self.qualified_simple_type_names
                            .get(namespace)
                            .is_some_and(|names| names.contains(right))
                            || self.component_metadata.is_known_enum(right)
                    }
                    TSTypeName::QualifiedName(_) | TSTypeName::ThisExpression(_) => false,
                }
            }
            TSTypeName::ThisExpression(_) => false,
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

struct LocalSimpleTypeAliasCollector<'a> {
    program: &'a Program<'a>,
}

impl<'a> LocalSimpleTypeAliasCollector<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut aliases = FxHashSet::default();
        for statement in &self.program.body {
            let declaration = match statement {
                Statement::TSTypeAliasDeclaration(declaration) => Some(declaration.as_ref()),
                Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                    Some(Declaration::TSTypeAliasDeclaration(declaration)) => {
                        Some(declaration.as_ref())
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(declaration) = declaration else {
                continue;
            };
            if BasicTypeAliasRules::is_simple(&declaration.type_annotation) {
                aliases.insert(declaration.id.name.to_string());
            }
        }
        aliases
    }
}

struct BasicTypeAliasRules;

impl BasicTypeAliasRules {
    /// Structural subset of TypeScript `TypeFlags` used by SDK
    /// `getDeclarationType`: aliases resolving to a multi-member union of
    /// string/number/boolean/bigint primitives or literals are simple.
    fn is_simple(ty: &TSType<'_>) -> bool {
        let TSType::TSUnionType(union) = ty else {
            return false;
        };
        let non_nullable = union
            .types
            .iter()
            .filter(|ty| !matches!(ty, TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_)))
            .collect::<Vec<_>>();
        match non_nullable.as_slice() {
            [ty] => matches!(ty, TSType::TSBooleanKeyword(_)),
            types if 1 < types.len() => {
                types.iter().all(|ty| Self::is_basic(ty))
                    && !TypeScriptUnionNormalization::collapses_to_single_primitive(types)
            }
            _ => false,
        }
    }

    fn is_basic(ty: &TSType<'_>) -> bool {
        matches!(
            ty,
            TSType::TSStringKeyword(_)
                | TSType::TSNumberKeyword(_)
                | TSType::TSBooleanKeyword(_)
                | TSType::TSBigIntKeyword(_)
        ) || matches!(
            ty,
            TSType::TSLiteralType(literal)
                if matches!(
                    &literal.literal,
                    TSLiteral::StringLiteral(_)
                        | TSLiteral::NumericLiteral(_)
                        | TSLiteral::BooleanLiteral(_)
                        | TSLiteral::BigIntLiteral(_)
                )
        )
    }
}

struct TypeScriptUnionNormalization;

impl TypeScriptUnionNormalization {
    /// TypeChecker.getTypeFromTypeNode normalizes a union before
    /// getDeclarationType inspects `type.types`. A broad primitive absorbs
    /// literals of the same family (`'toast' | string` becomes `string`) and
    /// therefore no longer has the multi-member union shape accepted by the
    /// SDK. Boolean is excluded because TypeScript represents `boolean`
    /// itself as the union of its two literal types.
    fn collapses_to_single_primitive(types: &[&TSType<'_>]) -> bool {
        Self::primitive_absorbs_all_literals(types, PrimitiveTypeFamily::String)
            || Self::primitive_absorbs_all_literals(types, PrimitiveTypeFamily::Number)
            || Self::primitive_absorbs_all_literals(types, PrimitiveTypeFamily::BigInt)
    }

    fn primitive_absorbs_all_literals(types: &[&TSType<'_>], family: PrimitiveTypeFamily) -> bool {
        types.iter().any(|ty| family.is_keyword(ty))
            && types
                .iter()
                .all(|ty| family.is_keyword(ty) || family.is_literal(ty))
    }
}

#[derive(Clone, Copy)]
enum PrimitiveTypeFamily {
    String,
    Number,
    BigInt,
}

impl PrimitiveTypeFamily {
    fn is_keyword(self, ty: &TSType<'_>) -> bool {
        matches!(
            (self, ty),
            (Self::String, TSType::TSStringKeyword(_))
                | (Self::Number, TSType::TSNumberKeyword(_))
                | (Self::BigInt, TSType::TSBigIntKeyword(_))
        )
    }

    fn is_literal(self, ty: &TSType<'_>) -> bool {
        let TSType::TSLiteralType(literal) = ty else {
            return false;
        };
        matches!(
            (self, &literal.literal),
            (Self::String, TSLiteral::StringLiteral(_))
                | (Self::Number, TSLiteral::NumericLiteral(_))
                | (Self::BigInt, TSLiteral::BigIntLiteral(_))
        )
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
        component_metadata: &crate::transform::component_meta::ComponentMetadata,
    ) -> Self {
        let inventory =
            ModuleExportFactInventoryBuilder::new(sources, module_roots, component_metadata)
                .build();
        let mut entries = FxHashMap::default();
        for _ in 0..inventory.paths.len().saturating_add(1).max(1) {
            let mut analyzer = ModuleExportClosureAnalyzer::new(&inventory.facts, &entries);
            let mut next_entries = FxHashMap::default();
            for path in &inventory.paths {
                next_entries.insert(path.clone(), analyzer.export_entry(path));
            }
            if !analyzer.used_cycle_fallback || next_entries == entries {
                return Self::from_entries(next_entries);
            }
            entries = next_entries;
        }
        Self::from_entries(entries)
    }

    fn from_entries(entries: FxHashMap<PathBuf, ModuleExportEntry>) -> Self {
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

    fn simple_type_alias_names(&self, path: &Path) -> FxHashSet<String> {
        self.entries
            .get(path)
            .map(|entry| entry.simple_type_aliases.clone())
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ModuleExportEntry {
    enums: FxHashSet<String>,
    simple_type_aliases: FxHashSet<String>,
    builders: FxHashSet<String>,
    components: ExportedComponentNames,
}

struct ModuleExportFactInventory {
    paths: Vec<PathBuf>,
    facts: FxHashMap<PathBuf, Arc<ModuleExportFacts>>,
}

struct ModuleExportFactInventoryBuilder<'a> {
    sources: &'a crate::source::TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
    component_metadata: &'a crate::transform::component_meta::ComponentMetadata,
}

impl<'a> ModuleExportFactInventoryBuilder<'a> {
    fn new(
        sources: &'a crate::source::TransformSourceSnapshot,
        module_roots: &'a [(String, PathBuf)],
        component_metadata: &'a crate::transform::component_meta::ComponentMetadata,
    ) -> Self {
        Self {
            sources,
            module_roots,
            component_metadata,
        }
    }

    fn build(&self) -> ModuleExportFactInventory {
        let source_files = self.sources.source_files();
        let collected = source_files
            .par_iter()
            .map(|source| {
                let facts = ModuleExportFacts::from_source(
                    source,
                    self.sources,
                    self.module_roots,
                    self.component_metadata,
                );
                (source.path.clone(), Arc::new(facts))
            })
            .collect::<Vec<_>>();
        let paths = collected
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        let facts = collected.into_iter().collect();
        ModuleExportFactInventory { paths, facts }
    }
}

#[derive(Clone, Debug, Default)]
struct ModuleExportFacts {
    direct: ModuleExportEntry,
    local_enums: FxHashSet<String>,
    local_simple_type_aliases: FxHashSet<String>,
    local_components: CustomComponentUpdateProps,
    imports: Vec<ModuleImportFact>,
    exports: Vec<ModuleExportFact>,
}

impl ModuleExportFacts {
    fn from_source(
        source: &TransformSourceFile,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
        component_metadata: &crate::transform::component_meta::ComponentMetadata,
    ) -> Self {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&source.path).unwrap_or_else(|_| SourceType::ets());
        let parsed = component_metadata.parse_ets(&allocator, &source.source, source_type);
        if !parsed.diagnostics.is_empty() {
            return Self::default();
        }
        Self::from_program(&source.path, &parsed.program, sources, module_roots)
    }

    fn from_program(
        path: &Path,
        program: &Program<'_>,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
    ) -> Self {
        let mut facts = Self {
            local_enums: LocalEnumNamesCollector::new(program).collect(),
            local_simple_type_aliases: LocalSimpleTypeAliasCollector::new(program).collect(),
            ..Self::default()
        };
        for statement in &program.body {
            match statement {
                Statement::FunctionDeclaration(function)
                    if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER) =>
                {
                    if let Some(function_id) = &function.id {
                        facts.direct.builders.insert(function_id.name.to_string());
                    }
                }
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                        facts.direct.enums.insert(declaration.id.name.to_string());
                    }
                    if let Some(Declaration::TSTypeAliasDeclaration(declaration)) =
                        &export.declaration
                    {
                        if facts
                            .local_simple_type_aliases
                            .contains(declaration.id.name.as_str())
                        {
                            facts
                                .direct
                                .simple_type_aliases
                                .insert(declaration.id.name.to_string());
                        }
                    }
                    if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                        if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER)
                            || export.decorators.iter().any(|decorator| {
                                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER
                            })
                        {
                            if let Some(function_id) = &function.id {
                                facts.direct.builders.insert(function_id.name.to_string());
                            }
                        }
                    }
                    if let Some(Declaration::StructStatement(statement)) =
                        export.declaration.as_ref()
                    {
                        if ComponentExportRules::is_exported_component_struct(statement) {
                            facts.direct.components.named.insert(
                                statement.id.name.to_string(),
                                ComponentInfoCollector::new(statement).collect(),
                            );
                        }
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::StructStatement(statement) =
                        &export.declaration
                    {
                        if ComponentExportRules::is_exported_component_struct(statement) {
                            facts.direct.components.has_default = true;
                            facts.direct.components.default_info =
                                ComponentInfoCollector::new(statement).collect();
                        }
                    }
                }
                _ => {}
            }
            LocalComponentExportCollector::new(&mut facts.local_components)
                .collect_statement(statement);
        }

        let resolver = ImportTargetResolver::new(path, sources, module_roots);
        for statement in &program.body {
            match statement {
                Statement::ImportDeclaration(import) => {
                    let Some(target) = resolver.resolve(import.source.value.as_str()) else {
                        continue;
                    };
                    let Some(specifiers) = &import.specifiers else {
                        continue;
                    };
                    let mut fact = ModuleImportFact {
                        target: target.path,
                        ..ModuleImportFact::default()
                    };
                    for specifier in specifiers {
                        match specifier {
                            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                                fact.named.push(ModuleImportBindingFact {
                                    imported: ComponentModuleExportName::value(&specifier.imported),
                                    local: specifier.local.name.to_string(),
                                });
                            }
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                                fact.default_components
                                    .push(specifier.local.name.to_string());
                            }
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
                        }
                    }
                    facts.imports.push(fact);
                }
                Statement::ExportNamedDeclaration(export) => {
                    if matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::StructStatement(_))
                    ) {
                        continue;
                    }
                    let bindings = export
                        .specifiers
                        .iter()
                        .map(|specifier| ModuleExportBindingFact {
                            local: ComponentModuleExportName::value(&specifier.local),
                            exported: ComponentModuleExportName::value(&specifier.exported),
                        })
                        .collect::<Vec<_>>();
                    if let Some(source) = &export.source {
                        if let Some(target) = resolver.resolve(source.value.as_str()) {
                            facts.exports.push(ModuleExportFact::NamedFrom {
                                target: target.path,
                                bindings,
                            });
                        }
                    } else if !bindings.is_empty() {
                        facts
                            .exports
                            .push(ModuleExportFact::NamedLocal { bindings });
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::Identifier(identifier) =
                        &export.declaration
                    {
                        facts.exports.push(ModuleExportFact::DefaultLocalComponent {
                            local: identifier.name.to_string(),
                        });
                    }
                }
                Statement::ExportAllDeclaration(export) => {
                    if let Some(target) = resolver.resolve(export.source.value.as_str()) {
                        facts.exports.push(ModuleExportFact::AllFrom {
                            target: target.path,
                        });
                    }
                }
                _ => {}
            }
        }
        facts
    }
}

#[derive(Clone, Debug, Default)]
struct ModuleImportFact {
    target: PathBuf,
    named: Vec<ModuleImportBindingFact>,
    default_components: Vec<String>,
}

#[derive(Clone, Debug)]
struct ModuleImportBindingFact {
    imported: String,
    local: String,
}

#[derive(Clone, Debug)]
struct ModuleExportBindingFact {
    local: String,
    exported: String,
}

#[derive(Clone, Debug)]
enum ModuleExportFact {
    NamedFrom {
        target: PathBuf,
        bindings: Vec<ModuleExportBindingFact>,
    },
    NamedLocal {
        bindings: Vec<ModuleExportBindingFact>,
    },
    DefaultLocalComponent {
        local: String,
    },
    AllFrom {
        target: PathBuf,
    },
}

struct ModuleExportClosureAnalyzer<'a> {
    facts: &'a FxHashMap<PathBuf, Arc<ModuleExportFacts>>,
    previous_entries: &'a FxHashMap<PathBuf, ModuleExportEntry>,
    active: FxHashSet<PathBuf>,
    entry_cache: FxHashMap<PathBuf, ModuleExportEntry>,
    used_cycle_fallback: bool,
}

impl<'a> ModuleExportClosureAnalyzer<'a> {
    fn new(
        facts: &'a FxHashMap<PathBuf, Arc<ModuleExportFacts>>,
        previous_entries: &'a FxHashMap<PathBuf, ModuleExportEntry>,
    ) -> Self {
        Self {
            facts,
            previous_entries,
            active: FxHashSet::default(),
            entry_cache: FxHashMap::default(),
            used_cycle_fallback: false,
        }
    }

    fn export_entry(&mut self, path: &Path) -> ModuleExportEntry {
        if let Some(entry) = self.entry_cache.get(path) {
            return entry.clone();
        }
        if !self.active.insert(path.to_path_buf()) {
            self.used_cycle_fallback = true;
            return self.previous_entries.get(path).cloned().unwrap_or_default();
        }

        let facts = self.facts.get(path).cloned().unwrap_or_default();
        let mut entry = facts.direct.clone();
        let mut local_enums = facts.local_enums.clone();
        let local_simple_type_aliases = facts.local_simple_type_aliases.clone();
        let mut local_components = facts.local_components.clone();

        for import in &facts.imports {
            let target_entry = self.export_entry(&import.target);
            for binding in &import.named {
                if target_entry.enums.contains(&binding.imported) {
                    local_enums.insert(binding.local.clone());
                }
                let props = if binding.imported == ComponentModuleExportName::DEFAULT
                    && target_entry.components.has_default
                {
                    Some(&target_entry.components.default_info)
                } else {
                    target_entry.components.named.get(&binding.imported)
                };
                if let Some(props) = props {
                    local_components.insert(binding.local.clone(), props.clone());
                }
            }
            for local in &import.default_components {
                if target_entry.components.has_default {
                    local_components
                        .insert(local.clone(), target_entry.components.default_info.clone());
                } else if let Some(props) = target_entry
                    .components
                    .named
                    .get(ComponentModuleExportName::DEFAULT)
                {
                    local_components.insert(local.clone(), props.clone());
                }
            }
        }

        for export in &facts.exports {
            match export {
                ModuleExportFact::NamedFrom { target, bindings } => {
                    let target_entry = self.export_entry(target);
                    for binding in bindings {
                        if target_entry.enums.contains(&binding.local) {
                            entry.enums.insert(binding.exported.clone());
                        }
                        if target_entry.simple_type_aliases.contains(&binding.local) {
                            entry.simple_type_aliases.insert(binding.exported.clone());
                        }
                        if target_entry.builders.contains(&binding.local) {
                            entry.builders.insert(binding.exported.clone());
                        }
                        let props = if binding.local == ComponentModuleExportName::DEFAULT
                            && target_entry.components.has_default
                        {
                            Some(&target_entry.components.default_info)
                        } else {
                            target_entry.components.named.get(&binding.local)
                        };
                        if binding.exported == ComponentModuleExportName::DEFAULT && props.is_some()
                        {
                            entry.components.has_default = true;
                            entry.components.default_info = props.cloned().unwrap_or_default();
                        } else if let Some(props) = props {
                            entry
                                .components
                                .named
                                .insert(binding.exported.clone(), props.clone());
                        }
                    }
                }
                ModuleExportFact::NamedLocal { bindings } => {
                    for binding in bindings {
                        if binding.exported == ComponentModuleExportName::DEFAULT
                            && local_components.contains_key(&binding.local)
                        {
                            entry.components.has_default = true;
                            entry.components.default_info = local_components
                                .get(&binding.local)
                                .cloned()
                                .unwrap_or_default();
                        } else if let Some(props) = local_components.get(&binding.local) {
                            entry
                                .components
                                .named
                                .insert(binding.exported.clone(), props.clone());
                        }
                        if local_simple_type_aliases.contains(&binding.local) {
                            entry.simple_type_aliases.insert(binding.exported.clone());
                        }
                        if local_enums.contains(&binding.local) {
                            entry.enums.insert(binding.exported.clone());
                        }
                    }
                }
                ModuleExportFact::DefaultLocalComponent { local } => {
                    if let Some(info) = local_components.get(local) {
                        entry.components.has_default = true;
                        entry.components.default_info = info.clone();
                    }
                }
                ModuleExportFact::AllFrom { target } => {
                    let target_entry = self.export_entry(target);
                    entry.enums.extend(target_entry.enums);
                    entry
                        .simple_type_aliases
                        .extend(target_entry.simple_type_aliases);
                    entry.builders.extend(target_entry.builders);
                    entry.components.extend(target_entry.components);
                }
            }
        }

        self.active.remove(path);
        self.entry_cache.insert(path.to_path_buf(), entry.clone());
        entry
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
