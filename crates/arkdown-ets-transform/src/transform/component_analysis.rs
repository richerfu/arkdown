use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::ModuleSpecifierText;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionBody, CallExpression, Declaration,
    Decorator, ExportDefaultDeclarationKind, Expression, Function, FunctionBody,
    ImportDeclarationSpecifier, ModuleExportName, Program, PropertyDefinition, Statement,
    StructElement, StructStatement, TSLiteral, TSType, TSTypeName,
};
use rayon::prelude::*;

use super::{ArkuiDecorator, ArkuiDecoratorName, ArkuiDecoratorRules, TransformOptions};
use crate::source::{CompileArktsSourceType, TransformSourceFile};
use crate::transform::build_fn::{
    CustomComponentGeneration, CustomComponentInfo, CustomComponentPropertyKind,
    CustomComponentUpdateProps,
};
use crate::transform::component_meta::SdkComponentInfo;
use crate::transform::decorator::ArkuiStateDecorator;
use crate::transform::legacy_extend::LegacyExtendSource;
use crate::transform::system_api::{
    ExportedSystemApiSymbols, SystemApiModuleFacts, SystemApiSymbol,
};
use crate::transform::viewpu_emit::GeneratedViewpuName;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExtendAttributeKind {
    Extend,
    Animatable,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ExtendAttributeIndex {
    by_component: FxHashMap<String, FxHashMap<String, ExtendAttributeKind>>,
}

impl ExtendAttributeIndex {
    pub(super) fn from_program(program: &Program<'_>) -> Self {
        let mut index = Self::default();
        for statement in &program.body {
            match statement {
                Statement::FunctionDeclaration(function) => {
                    index.collect_function(function, &[]);
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::FunctionDeclaration(function) = &export.declaration {
                        index.collect_function(function, &export.decorators);
                    }
                }
                _ => {}
            }
        }
        index
    }

    pub(super) fn kind(&self, component: &str, attribute: &str) -> Option<ExtendAttributeKind> {
        self.by_component
            .get(component)
            .and_then(|attributes| attributes.get(attribute))
            .copied()
    }

    fn collect_function(&mut self, function: &Function<'_>, extra_decorators: &[Decorator<'_>]) {
        let Some(name) = function.id.as_ref() else {
            return;
        };
        let Some((component, kind)) = function
            .decorators
            .iter()
            .chain(extra_decorators.iter())
            .find_map(|decorator| {
                let decorator = ArkuiDecorator::new(decorator);
                let kind = match decorator.name() {
                    ArkuiDecoratorName::EXTEND => ExtendAttributeKind::Extend,
                    ArkuiDecoratorName::ANIMATABLE_EXTEND => ExtendAttributeKind::Animatable,
                    _ => return None,
                };
                decorator
                    .identifier_arg()
                    .map(|component| (component, kind))
            })
        else {
            return;
        };
        self.by_component
            .entry(component)
            .or_default()
            .insert(name.name.to_string(), kind);
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
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
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

pub(crate) struct CustomComponentSemanticInventory {
    pub(crate) names: FxHashSet<String>,
    pub(crate) info: CustomComponentUpdateProps,
}

impl CustomComponentSemanticInventory {
    pub(crate) fn collect(program: &Program<'_>, id: &str, options: &TransformOptions) -> Self {
        let mut names = FxHashSet::default();
        let mut info = CustomComponentUpdateProps::default();
        let mut component_collector = CustomComponentNameCollector::new(&mut names);
        let mut info_collector = LocalComponentUpdatePropsCollector::new(&mut info);
        for statement in &program.body {
            component_collector.collect_statement(statement);
            info_collector.collect_statement(statement);
        }
        ImportedComponentNamesCollector::new(program, id, options, &mut names, &mut info).collect();
        Self { names, info }
    }
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
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
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
                    if !specifier.import_kind.is_type() =>
                {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_external_binding(
        &mut self,
        local: &str,
        external_component_usages: &FxHashSet<String>,
    ) {
        if external_component_usages.contains(local) {
            self.names.insert(local.to_string());
        }
        let prefix = format!("{local}.");
        self.names.extend(
            external_component_usages
                .iter()
                .filter(|name| name.starts_with(&prefix))
                .cloned(),
        );
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
                if (imported == ComponentModuleExportName::DEFAULT && target_components.has_default)
                    || target_components.named.contains_key(&imported)
                {
                    self.names.insert(local);
                } else if let Some(namespace) = target_components.namespaces.get(&imported) {
                    self.collect_namespace(&local, namespace);
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
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                self.collect_namespace(specifier.local.name.as_str(), target_components);
            }
        }
    }

    fn collect_namespace(&mut self, local: &str, components: &ExportedComponentNames) {
        components.visit_paths(local, &mut |path, _| {
            self.names.insert(path);
        });
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
                    if !specifier.import_kind.is_type() =>
                {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    self.collect_external_binding(
                        specifier.local.name.as_str(),
                        external_component_usages,
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_external_binding(
        &mut self,
        local: &str,
        external_component_usages: &FxHashSet<String>,
    ) {
        if external_component_usages.contains(local) {
            self.names.insert(local.to_string());
        }
        let prefix = format!("{local}.");
        self.names.extend(
            external_component_usages
                .iter()
                .filter(|name| name.starts_with(&prefix))
                .cloned(),
        );
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
                } else if let Some(namespace) = target_components.namespaces.get(&imported) {
                    self.collect_namespace(&local, namespace);
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
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                self.collect_namespace(specifier.local.name.as_str(), target_components);
            }
        }
    }

    fn collect_namespace(&mut self, local: &str, components: &ExportedComponentNames) {
        components.visit_paths(local, &mut |path, info| {
            self.names.insert(path.clone());
            self.update_props.insert(path, info.clone());
        });
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
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
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
        if let Some(path) = Self::component_path(&component.callee) {
            self.names.insert(path);
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

    fn component_path(expression: &Expression<'_>) -> Option<String> {
        match expression {
            Expression::Identifier(identifier) => Some(identifier.name.to_string()),
            Expression::StaticMemberExpression(member) => {
                let prefix = Self::component_path(&member.object)?;
                Some(format!("{prefix}.{}", member.property.name))
            }
            _ => None,
        }
    }

    fn collect_argument(&mut self, argument: &Argument<'_>) {
        let Some(expression) = argument.as_expression() else {
            return;
        };
        if let Expression::ArrowFunctionExpression(arrow) = expression {
            if let ArrowFunctionBody::FunctionBody(body) = &arrow.body {
                self.collect_body(body);
            }
        }
    }
}

impl From<&SdkComponentInfo> for CustomComponentInfo {
    fn from(info: &SdkComponentInfo) -> Self {
        Self {
            update_props: info.update_props.clone(),
            link_props: info.link_props.clone(),
            builder_params: FxHashSet::default(),
            param_props: FxHashSet::default(),
            event_props: FxHashSet::default(),
            required_props: Vec::new(),
            forbidden_init_props: FxHashMap::default(),
            custom_env_props: FxHashSet::default(),
            regular_props: FxHashSet::default(),
            v1_required_props: Vec::new(),
            v1_mandatory_props: Vec::new(),
            v1_property_kinds: FxHashMap::default(),
            v1_forbidden_init_props: FxHashMap::default(),
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

pub(super) struct BuilderFunctionNames {
    pub(super) local: FxHashSet<String>,
    pub(super) all: FxHashSet<String>,
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

    pub(super) fn collect(&self) -> BuilderFunctionNames {
        let mut local = FxHashSet::default();
        self.collect_local(&mut local);
        let mut all = local.clone();
        self.collect_imported(&mut all);
        BuilderFunctionNames { local, all }
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
                Statement::ExportDeclaration(export) => {
                    if let Declaration::FunctionDeclaration(function) = &export.declaration {
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
            if import.import_kind.is_type() {
                continue;
            }
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                if specifier.import_kind.is_type() {
                    continue;
                }
                let imported = ComponentModuleExportName::value(&specifier.imported);
                if self
                    .options
                    .component_metadata
                    .is_sdk_builder_function(import.source.value.as_str(), &imported)
                {
                    names.insert(specifier.local.name.to_string());
                }
            }
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
                if specifier.import_kind.is_type() {
                    continue;
                }
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
            param_props: self.decorated_property_names(ArkuiDecoratorName::PARAM),
            event_props: self.decorated_property_names(ArkuiDecoratorName::EVENT),
            required_props: self.required_prop_names(),
            forbidden_init_props: self.forbidden_init_props(),
            custom_env_props: self.decorated_property_names(ArkuiDecoratorName::CUSTOM_ENV),
            regular_props: self.regular_property_names(),
            v1_required_props: self.v1_required_prop_names(),
            v1_mandatory_props: self.v1_mandatory_prop_names(),
            v1_property_kinds: self.v1_property_kinds(),
            v1_forbidden_init_props: self.v1_forbidden_init_props(),
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
            let update_prop = if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
                property.decorators.iter().any(|decorator| {
                    matches!(
                        ArkuiDecorator::new(decorator).name(),
                        ArkuiDecoratorName::PARAM | ArkuiDecoratorName::ONCE
                    )
                })
            } else {
                property.decorators.iter().any(|decorator| {
                    ArkuiDecoratorName::is_custom_component_update_prop(
                        ArkuiDecorator::new(decorator).name(),
                    )
                })
            };
            if !update_prop {
                continue;
            }
            if let Some(name) = property.key.static_name() {
                names.insert(name.into_owned());
            }
        }
        names
    }

    fn required_prop_names(&self) -> Vec<String> {
        if !ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return Vec::new();
        }
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property) => Some(property),
                _ => None,
            })
            .filter(|property| {
                let decorators = property
                    .decorators
                    .iter()
                    .map(|decorator| ArkuiDecorator::new(decorator).name())
                    .collect::<Vec<_>>();
                decorators.contains(&ArkuiDecoratorName::REQUIRE)
                    && (decorators.contains(&ArkuiDecoratorName::PARAM)
                        || decorators.contains(&ArkuiDecoratorName::BUILDER_PARAM))
            })
            .filter_map(|property| property.key.static_name().map(|name| name.into_owned()))
            .collect()
    }

    fn forbidden_init_props(&self) -> FxHashMap<String, CustomComponentPropertyKind> {
        if !ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return FxHashMap::default();
        }
        let mut properties = FxHashMap::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let decorators = property
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .collect::<Vec<_>>();
            let kind = if decorators.contains(&ArkuiDecoratorName::LOCAL) {
                Some(CustomComponentPropertyKind::Local)
            } else if decorators.contains(&ArkuiDecoratorName::CONSUMER) {
                Some(CustomComponentPropertyKind::Consumer)
            } else if decorators.contains(&ArkuiDecoratorName::PROVIDER) {
                Some(CustomComponentPropertyKind::Provider)
            } else if !decorators.iter().any(|decorator| {
                matches!(
                    *decorator,
                    ArkuiDecoratorName::LOCAL
                        | ArkuiDecoratorName::PARAM
                        | ArkuiDecoratorName::ONCE
                        | ArkuiDecoratorName::EVENT
                        | ArkuiDecoratorName::PROVIDER
                        | ArkuiDecoratorName::CONSUMER
                        | ArkuiDecoratorName::BUILDER_PARAM
                        | ArkuiDecoratorName::ENV
                )
            }) {
                Some(CustomComponentPropertyKind::Regular)
            } else {
                None
            };
            if let Some(kind) = kind {
                properties.insert(name.into_owned(), kind);
            }
        }
        properties
    }

    fn regular_property_names(&self) -> FxHashSet<String> {
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return FxHashSet::default();
        }
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property)
                    if property.decorators.iter().all(|decorator| {
                        !ArkuiStateDecorator::new(ArkuiDecorator::new(decorator).name()).is_state()
                    }) =>
                {
                    property.key.static_name().map(|name| name.into_owned())
                }
                _ => None,
            })
            .collect()
    }

    fn v1_required_prop_names(&self) -> Vec<String> {
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return Vec::new();
        }
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property)
                    if property.decorators.iter().any(|decorator| {
                        ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REQUIRE
                    }) =>
                {
                    Some(property)
                }
                _ => None,
            })
            .filter(|property| {
                let decorators = property
                    .decorators
                    .iter()
                    .map(|decorator| ArkuiDecorator::new(decorator).name())
                    .collect::<Vec<_>>();
                decorators.iter().any(|decorator| {
                    matches!(
                        *decorator,
                        ArkuiDecoratorName::BUILDER_PARAM
                            | ArkuiDecoratorName::PROP
                            | ArkuiDecoratorName::STATE
                            | ArkuiDecoratorName::PROVIDE
                    )
                }) || decorators
                    .iter()
                    .all(|decorator| !ArkuiStateDecorator::new(decorator).is_state())
            })
            .filter_map(|property| property.key.static_name().map(|name| name.into_owned()))
            .collect()
    }

    fn v1_mandatory_prop_names(&self) -> Vec<String> {
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return Vec::new();
        }
        let mut names = Vec::new();
        for required_decorator in [ArkuiDecoratorName::LINK, ArkuiDecoratorName::OBJECT_LINK] {
            names.extend(
                self.statement
                    .body
                    .body
                    .iter()
                    .filter_map(|element| match element {
                        StructElement::PropertyDefinition(property)
                            if property.decorators.iter().any(|decorator| {
                                ArkuiDecorator::new(decorator).name() == required_decorator
                            }) =>
                        {
                            property.key.static_name().map(|name| name.into_owned())
                        }
                        _ => None,
                    }),
            );
        }
        names
    }

    fn v1_property_kinds(&self) -> FxHashMap<String, CustomComponentPropertyKind> {
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return FxHashMap::default();
        }
        let mut properties = FxHashMap::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let kind = property
                .decorators
                .iter()
                .find_map(|decorator| Self::v1_transfer_kind(ArkuiDecorator::new(decorator).name()))
                .or_else(|| {
                    property
                        .decorators
                        .iter()
                        .all(|decorator| {
                            !ArkuiStateDecorator::new(ArkuiDecorator::new(decorator).name())
                                .is_state()
                        })
                        .then_some(CustomComponentPropertyKind::Regular)
                });
            if let Some(kind) = kind {
                properties.insert(name.into_owned(), kind);
            }
        }
        properties
    }

    fn v1_transfer_kind(decorator: &str) -> Option<CustomComponentPropertyKind> {
        match decorator {
            ArkuiDecoratorName::STATE => Some(CustomComponentPropertyKind::State),
            ArkuiDecoratorName::LINK => Some(CustomComponentPropertyKind::Link),
            ArkuiDecoratorName::PROP => Some(CustomComponentPropertyKind::Prop),
            ArkuiDecoratorName::PROVIDE => Some(CustomComponentPropertyKind::Provide),
            ArkuiDecoratorName::OBJECT_LINK => Some(CustomComponentPropertyKind::ObjectLink),
            _ => None,
        }
    }

    fn v1_forbidden_init_props(&self) -> FxHashMap<String, CustomComponentPropertyKind> {
        if ArkuiDecoratorRules::is_component_v2_struct(self.statement) {
            return FxHashMap::default();
        }
        let mut properties = FxHashMap::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let kind = property
                .decorators
                .iter()
                .find_map(|decorator| match ArkuiDecorator::new(decorator).name() {
                    ArkuiDecoratorName::STORAGE_LINK => {
                        Some(CustomComponentPropertyKind::StorageLink)
                    }
                    ArkuiDecoratorName::STORAGE_PROP => {
                        Some(CustomComponentPropertyKind::StorageProp)
                    }
                    ArkuiDecoratorName::LOCAL_STORAGE_LINK => {
                        Some(CustomComponentPropertyKind::LocalStorageLink)
                    }
                    ArkuiDecoratorName::LOCAL_STORAGE_PROP => {
                        Some(CustomComponentPropertyKind::LocalStorageProp)
                    }
                    ArkuiDecoratorName::CONSUME => Some(CustomComponentPropertyKind::Consume),
                    _ => None,
                });
            if let Some(kind) = kind {
                properties.insert(name.into_owned(), kind);
            }
        }
        properties
    }

    fn decorated_property_names(&self, decorator_name: &str) -> FxHashSet<String> {
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property)
                    if property.decorators.iter().any(|decorator| {
                        ArkuiDecorator::new(decorator).name() == decorator_name
                    }) =>
                {
                    property.key.static_name().map(|name| name.into_owned())
                }
                _ => None,
            })
            .collect()
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
                matches!(
                    ArkuiDecorator::new(decorator).name(),
                    ArkuiDecoratorName::BUILDER | ArkuiDecoratorName::LOCAL_BUILDER
                )
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

/// Resolves the component properties whose `.builder()` call has
/// `MutableBuilder` semantics. The upstream implementation asks the TypeScript
/// checker for the receiver type in
/// `process_component_build.ts::isMutableBuilder`; component property type
/// annotations and direct `mutableBuilder(...)` initializers are the
/// authoritative syntax available to the OXC lowering stage.
pub(super) struct StructMutableBuilderProperties<'a> {
    statement: &'a StructStatement<'a>,
}

/// Collects instance fields that ets-loader treats as immutable attribute
/// inputs while lowering a reusable component. V1 records every undecorated
/// property in `regularCollection`; V2 additionally requires an explicit
/// primitive annotation in `process_component_build.ts::isSimpleType`.
pub(super) struct StructRegularAttributeProperties<'a> {
    statement: &'a StructStatement<'a>,
    component_v2: bool,
}

pub(super) struct PreviewComponentNameCollector<'a> {
    program: &'a Program<'a>,
}

pub(super) struct LocalBuilderFileInventory<'a> {
    program: &'a Program<'a>,
}

impl<'a> LocalBuilderFileInventory<'a> {
    pub(super) fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    pub(super) fn has_local_builder(&self) -> bool {
        self.program
            .body
            .iter()
            .filter_map(PreviewComponentNameCollector::struct_statement)
            .flat_map(|statement| statement.body.body.iter())
            .any(|element| {
                matches!(
                    element,
                    StructElement::MethodDefinition(method)
                        if method.decorators.iter().any(|decorator| {
                            ArkuiDecorator::new(decorator).name()
                                == ArkuiDecoratorName::LOCAL_BUILDER
                        })
                )
            })
    }
}

impl<'a> PreviewComponentNameCollector<'a> {
    pub(super) fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    pub(super) fn collect(&self) -> Vec<String> {
        self.program
            .body
            .iter()
            .filter_map(Self::struct_statement)
            .filter(|statement| {
                statement.decorators.iter().any(|decorator| {
                    ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::PREVIEW
                })
            })
            .map(|statement| statement.id.name.to_string())
            .collect()
    }

    fn struct_statement(statement: &'a Statement<'a>) -> Option<&'a StructStatement<'a>> {
        match statement {
            Statement::StructStatement(statement) => Some(statement),
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::StructStatement(statement) => Some(statement),
                _ => None,
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::StructStatement(statement) => Some(statement),
                _ => None,
            },
            _ => None,
        }
    }
}

impl<'a> StructMutableBuilderProperties<'a> {
    const CLASS: &'static str = "MutableBuilder";
    const FACTORY: &'static str = "mutableBuilder";

    pub(super) fn new(statement: &'a StructStatement<'a>) -> Self {
        Self { statement }
    }

    pub(super) fn collect(&self) -> FxHashSet<String> {
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| {
                let StructElement::PropertyDefinition(property) = element else {
                    return None;
                };
                Self::is_mutable_builder_property(property)
                    .then(|| property.key.static_name().map(|name| name.into_owned()))
                    .flatten()
            })
            .collect()
    }

    fn is_mutable_builder_property(property: &PropertyDefinition<'_>) -> bool {
        property.type_annotation.as_ref().is_some_and(|annotation| {
            matches!(
                &annotation.type_annotation,
                TSType::TSTypeReference(reference)
                    if matches!(
                        &reference.type_name,
                        TSTypeName::IdentifierReference(identifier)
                            if identifier.name == Self::CLASS
                    )
            )
        }) || property.value.as_ref().is_some_and(|initializer| {
            matches!(
                initializer,
                Expression::CallExpression(call)
                    if matches!(
                        &call.callee,
                        Expression::Identifier(identifier)
                            if identifier.name == Self::FACTORY
                    )
            )
        })
    }
}

impl<'a> StructRegularAttributeProperties<'a> {
    pub(super) fn new(statement: &'a StructStatement<'a>, component_v2: bool) -> Self {
        Self {
            statement,
            component_v2,
        }
    }

    pub(super) fn collect(&self) -> FxHashSet<String> {
        self.statement
            .body
            .body
            .iter()
            .filter_map(|element| {
                let StructElement::PropertyDefinition(property) = element else {
                    return None;
                };
                if property.r#static || !self.is_regular_property(property) {
                    return None;
                }
                property.key.static_name().map(|name| name.into_owned())
            })
            .collect()
    }

    fn is_regular_property(&self, property: &PropertyDefinition<'_>) -> bool {
        let decorator_names: Vec<&str> = property
            .decorators
            .iter()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .collect();
        if ArkuiDecoratorRules::primary_state(&decorator_names).is_some()
            || decorator_names.contains(&ArkuiDecoratorName::CUSTOM_ENV)
        {
            return false;
        }
        !self.component_v2
            || property.type_annotation.as_ref().is_some_and(|annotation| {
                matches!(
                    annotation.type_annotation,
                    TSType::TSStringKeyword(_)
                        | TSType::TSNumberKeyword(_)
                        | TSType::TSBooleanKeyword(_)
                )
            })
    }
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
                Statement::ExportDeclaration(export) => {
                    if let Declaration::TSEnumDeclaration(declaration) = &export.declaration {
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
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::TSTypeAliasDeclaration(declaration) => Some(declaration.as_ref()),
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClassObservation {
    pub(crate) observed_v1: bool,
    pub(crate) observed_v2: bool,
}

impl ClassObservation {
    fn from_class(class: &oxc_ast::ast::Class<'_>) -> Self {
        Self {
            observed_v1: class.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED
            }),
            observed_v2: class.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED_V2
            }),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImportedClassObservations {
    pub(crate) bindings: FxHashMap<String, ClassObservation>,
    pub(crate) namespaces: FxHashMap<String, FxHashMap<String, ClassObservation>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImportedInsightIntentEntities {
    bindings: FxHashMap<String, String>,
    namespaces: FxHashMap<String, FxHashMap<String, String>>,
}

impl ImportedInsightIntentEntities {
    pub(crate) fn class_name(&self, name: &str) -> Option<&str> {
        if let Some(class_name) = self.bindings.get(name) {
            return Some(class_name);
        }
        let (namespace, member) = name.split_once('.')?;
        self.namespaces
            .get(namespace)
            .and_then(|members| members.get(member))
            .map(String::as_str)
    }
}

impl ModuleExportIndex {
    pub(crate) fn build(
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
        component_metadata: &crate::transform::component_meta::ComponentMetadata,
    ) -> Self {
        Self::build_with_version(
            sources,
            module_roots,
            component_metadata,
            arkdown_core::ArktsLanguageVersion::default(),
        )
    }

    pub(crate) fn build_with_version(
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
        component_metadata: &crate::transform::component_meta::ComponentMetadata,
        arkts_language_version: arkdown_core::ArktsLanguageVersion,
    ) -> Self {
        let inventory = ModuleExportFactInventoryBuilder::new(
            sources,
            module_roots,
            component_metadata,
            arkts_language_version,
        )
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

    pub(super) fn system_api_exports(&self, path: &Path) -> Option<&ExportedSystemApiSymbols> {
        self.entries.get(path).map(|entry| &entry.system_apis)
    }

    pub(crate) fn has_system_api_exports(&self) -> bool {
        self.entries
            .values()
            .any(|entry| !entry.system_apis.is_empty())
    }

    pub(crate) fn imported_class_observations(
        &self,
        program: &Program<'_>,
        importer: &Path,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
    ) -> ImportedClassObservations {
        let resolver = ImportTargetResolver::new(importer, sources, module_roots);
        let mut observations = ImportedClassObservations::default();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind.is_type() {
                continue;
            }
            let Some(target) = resolver.resolve(import.source.value.as_str()) else {
                continue;
            };
            let Some(exports) = self.entries.get(&target.path).map(|entry| &entry.classes) else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier)
                        if !specifier.import_kind.is_type() =>
                    {
                        let imported = ComponentModuleExportName::value(&specifier.imported);
                        if let Some(observation) = exports.get(&imported) {
                            observations
                                .bindings
                                .insert(specifier.local.name.to_string(), observation);
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        if let Some(observation) = exports.default {
                            observations
                                .bindings
                                .insert(specifier.local.name.to_string(), observation);
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        observations
                            .namespaces
                            .insert(specifier.local.name.to_string(), exports.namespace());
                    }
                    _ => {}
                }
            }
        }
        observations
    }

    pub(crate) fn imported_insight_intent_entities(
        &self,
        program: &Program<'_>,
        importer: &Path,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
    ) -> ImportedInsightIntentEntities {
        let resolver = ImportTargetResolver::new(importer, sources, module_roots);
        let mut entities = ImportedInsightIntentEntities::default();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(target) = resolver.resolve(import.source.value.as_str()) else {
                continue;
            };
            let Some(exports) = self.entries.get(&target.path).map(|entry| &entry.entities) else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        let imported = ComponentModuleExportName::value(&specifier.imported);
                        if let Some(class_name) = exports.get(&imported) {
                            entities
                                .bindings
                                .insert(specifier.local.name.to_string(), class_name.to_string());
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        if let Some(class_name) = exports.default.as_deref() {
                            entities
                                .bindings
                                .insert(specifier.local.name.to_string(), class_name.to_string());
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        let mut namespace = exports.named.clone();
                        if let Some(class_name) = exports.default.as_ref() {
                            namespace.insert(
                                ComponentModuleExportName::DEFAULT.to_string(),
                                class_name.clone(),
                            );
                        }
                        entities
                            .namespaces
                            .insert(specifier.local.name.to_string(), namespace);
                    }
                }
            }
        }
        entities
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ModuleExportEntry {
    enums: FxHashSet<String>,
    simple_type_aliases: FxHashSet<String>,
    builders: FxHashSet<String>,
    components: ExportedComponentNames,
    classes: ExportedClassObservations,
    entities: ExportedInsightIntentEntities,
    system_apis: ExportedSystemApiSymbols,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ExportedInsightIntentEntities {
    named: FxHashMap<String, String>,
    default: Option<String>,
}

impl ExportedInsightIntentEntities {
    fn get(&self, name: &str) -> Option<&str> {
        if name == ComponentModuleExportName::DEFAULT {
            self.default.as_deref()
        } else {
            self.named.get(name).map(String::as_str)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ExportedClassObservations {
    named: FxHashMap<String, ClassObservation>,
    default: Option<ClassObservation>,
}

impl ExportedClassObservations {
    fn get(&self, name: &str) -> Option<ClassObservation> {
        if name == ComponentModuleExportName::DEFAULT {
            self.default
        } else {
            self.named.get(name).copied()
        }
    }

    fn namespace(&self) -> FxHashMap<String, ClassObservation> {
        let mut namespace = self.named.clone();
        if let Some(default) = self.default {
            namespace.insert(ComponentModuleExportName::DEFAULT.to_string(), default);
        }
        namespace
    }

    fn extend_named(&mut self, other: &Self) {
        self.named.extend(other.named.clone());
    }
}

struct ModuleExportFactInventory {
    paths: Vec<PathBuf>,
    facts: FxHashMap<PathBuf, Arc<ModuleExportFacts>>,
}

struct ModuleExportFactInventoryBuilder<'a> {
    sources: &'a crate::source::TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
    component_metadata: &'a crate::transform::component_meta::ComponentMetadata,
    arkts_language_version: arkdown_core::ArktsLanguageVersion,
}

impl<'a> ModuleExportFactInventoryBuilder<'a> {
    fn new(
        sources: &'a crate::source::TransformSourceSnapshot,
        module_roots: &'a [(String, PathBuf)],
        component_metadata: &'a crate::transform::component_meta::ComponentMetadata,
        arkts_language_version: arkdown_core::ArktsLanguageVersion,
    ) -> Self {
        Self {
            sources,
            module_roots,
            component_metadata,
            arkts_language_version,
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
                    self.arkts_language_version,
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
    local_builders: FxHashSet<String>,
    local_components: CustomComponentUpdateProps,
    local_classes: FxHashMap<String, ClassObservation>,
    local_entities: FxHashMap<String, String>,
    local_system_apis: FxHashMap<Box<str>, SystemApiSymbol>,
    direct_system_api_bindings: Vec<(Box<str>, Box<str>)>,
    imports: Vec<ModuleImportFact>,
    exports: Vec<ModuleExportFact>,
}

impl ModuleExportFacts {
    fn from_source(
        source: &TransformSourceFile,
        sources: &crate::source::TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
        component_metadata: &crate::transform::component_meta::ComponentMetadata,
        arkts_language_version: arkdown_core::ArktsLanguageVersion,
    ) -> Self {
        let allocator = Allocator::default();
        let source_type =
            CompileArktsSourceType::from_path_with_version(&source.path, arkts_language_version);
        let parser_source = LegacyExtendSource::new(&source.source, arkts_language_version);
        let parsed = component_metadata.parse_ets(&allocator, parser_source.as_str(), source_type);
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
        let system_api = SystemApiModuleFacts::from_program(
            path,
            program,
            sources.is_sdk_declaration(path),
            module_roots.iter().any(|(_, root)| path.starts_with(root)),
        );
        let mut facts = Self {
            local_enums: LocalEnumNamesCollector::new(program).collect(),
            local_simple_type_aliases: LocalSimpleTypeAliasCollector::new(program).collect(),
            local_system_apis: system_api.local,
            direct_system_api_bindings: system_api.direct_bindings,
            direct: ModuleExportEntry {
                system_apis: system_api.direct,
                ..ModuleExportEntry::default()
            },
            ..Self::default()
        };
        for statement in &program.body {
            match statement {
                Statement::ClassDeclaration(class) => {
                    if let Some(name) = &class.id {
                        facts
                            .local_classes
                            .insert(name.name.to_string(), ClassObservation::from_class(class));
                        if Self::is_insight_intent_entity(&class.decorators) {
                            facts
                                .local_entities
                                .insert(name.name.to_string(), name.name.to_string());
                        }
                    }
                }
                Statement::StructStatement(structure)
                    if Self::is_insight_intent_entity(&structure.decorators) =>
                {
                    facts
                        .local_entities
                        .insert(structure.id.name.to_string(), structure.id.name.to_string());
                }
                Statement::FunctionDeclaration(function)
                    if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER) =>
                {
                    if let Some(function_id) = &function.id {
                        facts.local_builders.insert(function_id.name.to_string());
                    }
                }
                Statement::ExportDeclaration(export) => {
                    if let Declaration::ClassDeclaration(class) = &export.declaration {
                        if let Some(name) = &class.id {
                            let observation = ClassObservation::from_class(class);
                            facts
                                .local_classes
                                .insert(name.name.to_string(), observation);
                            facts
                                .direct
                                .classes
                                .named
                                .insert(name.name.to_string(), observation);
                            if Self::is_insight_intent_entity(&class.decorators) {
                                facts
                                    .local_entities
                                    .insert(name.name.to_string(), name.name.to_string());
                                facts
                                    .direct
                                    .entities
                                    .named
                                    .insert(name.name.to_string(), name.name.to_string());
                            }
                        }
                    }
                    if let Declaration::TSEnumDeclaration(declaration) = &export.declaration {
                        facts.direct.enums.insert(declaration.id.name.to_string());
                    }
                    if let Declaration::TSTypeAliasDeclaration(declaration) = &export.declaration {
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
                    if let Declaration::FunctionDeclaration(function) = &export.declaration {
                        if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER)
                            || export.decorators.iter().any(|decorator| {
                                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::BUILDER
                            })
                        {
                            if let Some(function_id) = &function.id {
                                let name = function_id.name.to_string();
                                facts.local_builders.insert(name.clone());
                                facts.direct.builders.insert(name);
                            }
                        }
                    }
                    if let Declaration::StructStatement(statement) = &export.declaration {
                        if Self::is_insight_intent_entity(&statement.decorators) {
                            facts.local_entities.insert(
                                statement.id.name.to_string(),
                                statement.id.name.to_string(),
                            );
                            facts.direct.entities.named.insert(
                                statement.id.name.to_string(),
                                statement.id.name.to_string(),
                            );
                        }
                        if ComponentExportRules::is_exported_component_struct(statement) {
                            facts.direct.components.named.insert(
                                statement.id.name.to_string(),
                                ComponentInfoCollector::new(statement).collect(),
                            );
                        }
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::FunctionDeclaration(function) =
                        &export.declaration
                    {
                        if FunctionDecorators::new(function).has(ArkuiDecoratorName::BUILDER) {
                            facts
                                .direct
                                .builders
                                .insert(ComponentModuleExportName::DEFAULT.to_string());
                            if let Some(function_id) = &function.id {
                                facts.local_builders.insert(function_id.name.to_string());
                            }
                        }
                    }
                    if let ExportDefaultDeclarationKind::ClassDeclaration(class) =
                        &export.declaration
                    {
                        let observation = ClassObservation::from_class(class);
                        facts.direct.classes.default = Some(observation);
                        facts.direct.entities.default =
                            Self::is_insight_intent_entity(&class.decorators).then(|| {
                                class
                                    .id
                                    .as_ref()
                                    .map(|name| name.name.to_string())
                                    .unwrap_or_else(|| {
                                        ComponentModuleExportName::DEFAULT.to_string()
                                    })
                            });
                        if let Some(name) = &class.id {
                            facts
                                .local_classes
                                .insert(name.name.to_string(), observation);
                            if let Some(class_name) = facts.direct.entities.default.as_ref() {
                                facts
                                    .local_entities
                                    .insert(name.name.to_string(), class_name.clone());
                            }
                        }
                    }
                    if let ExportDefaultDeclarationKind::StructStatement(statement) =
                        &export.declaration
                    {
                        facts.direct.entities.default =
                            Self::is_insight_intent_entity(&statement.decorators)
                                .then(|| statement.id.name.to_string());
                        if let Some(class_name) = facts.direct.entities.default.as_ref() {
                            facts
                                .local_entities
                                .insert(statement.id.name.to_string(), class_name.clone());
                        }
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
                                fact.default_system_apis
                                    .push(specifier.local.name.to_string());
                            }
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                                fact.namespace_system_apis
                                    .push(specifier.local.name.to_string());
                            }
                        }
                    }
                    facts.imports.push(fact);
                }
                Statement::ExportDeclaration(export) => {
                    if matches!(&export.declaration, Declaration::StructStatement(_)) {
                        continue;
                    }
                }
                Statement::ExportNamedDeclaration(export) => {
                    let bindings = export
                        .specifiers
                        .iter()
                        .map(|specifier| ModuleExportBindingFact {
                            local: ComponentModuleExportName::value(&specifier.local),
                            exported: ComponentModuleExportName::value(&specifier.exported),
                        })
                        .collect::<Vec<_>>();
                    if !bindings.is_empty() {
                        facts
                            .exports
                            .push(ModuleExportFact::NamedLocal { bindings });
                    }
                }
                Statement::ExportFromDeclaration(export) => {
                    let bindings = export
                        .specifiers
                        .iter()
                        .map(|specifier| ModuleExportBindingFact {
                            local: ComponentModuleExportName::value(&specifier.local),
                            exported: ComponentModuleExportName::value(&specifier.exported),
                        })
                        .collect::<Vec<_>>();
                    if let Some(target) = resolver.resolve(export.source.value.as_str()) {
                        facts.exports.push(ModuleExportFact::NamedFrom {
                            target: target.path,
                            bindings,
                        });
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::Identifier(identifier) =
                        &export.declaration
                    {
                        facts.exports.push(ModuleExportFact::DefaultLocal {
                            local: identifier.name.to_string(),
                        });
                    }
                }
                Statement::TSExportAssignment(export) => {
                    if let Expression::Identifier(identifier) = &export.expression {
                        facts.exports.push(ModuleExportFact::DefaultLocal {
                            local: identifier.name.to_string(),
                        });
                    }
                }
                Statement::ExportAllDeclaration(export) => {
                    if let Some(target) = resolver.resolve(export.source.value.as_str()) {
                        if let Some(exported) = &export.exported {
                            facts.exports.push(ModuleExportFact::NamespaceFrom {
                                target: target.path,
                                exported: ComponentModuleExportName::value(exported),
                            });
                        } else {
                            facts.exports.push(ModuleExportFact::AllFrom {
                                target: target.path,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        facts
    }

    fn is_insight_intent_entity(decorators: &[Decorator<'_>]) -> bool {
        decorators.iter().any(|decorator| {
            let Expression::CallExpression(call) = &decorator.expression else {
                return false;
            };
            matches!(
                &call.callee,
                Expression::Identifier(identifier) if identifier.name == "InsightIntentEntity"
            )
        })
    }
}

#[derive(Clone, Debug, Default)]
struct ModuleImportFact {
    target: PathBuf,
    named: Vec<ModuleImportBindingFact>,
    default_components: Vec<String>,
    default_system_apis: Vec<String>,
    namespace_system_apis: Vec<String>,
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
    DefaultLocal {
        local: String,
    },
    AllFrom {
        target: PathBuf,
    },
    NamespaceFrom {
        target: PathBuf,
        exported: String,
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
        let mut local_builders = facts.local_builders.clone();
        let mut local_components = facts.local_components.clone();
        let mut local_classes = facts.local_classes.clone();
        let mut local_entities = facts.local_entities.clone();
        let mut local_system_apis = facts.local_system_apis.clone();

        for import in &facts.imports {
            let target_entry = self.export_entry(&import.target);
            for binding in &import.named {
                if target_entry.enums.contains(&binding.imported) {
                    local_enums.insert(binding.local.clone());
                }
                if target_entry.builders.contains(&binding.imported) {
                    local_builders.insert(binding.local.clone());
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
                if let Some(observation) = target_entry.classes.get(&binding.imported) {
                    local_classes.insert(binding.local.clone(), observation);
                }
                if let Some(class_name) = target_entry.entities.get(&binding.imported) {
                    local_entities.insert(binding.local.clone(), class_name.to_string());
                }
                if let Some(symbol) = target_entry.system_apis.get(&binding.imported) {
                    local_system_apis.insert(binding.local.clone().into(), symbol.clone());
                }
            }
            for local in &import.default_components {
                if target_entry
                    .builders
                    .contains(ComponentModuleExportName::DEFAULT)
                {
                    local_builders.insert(local.clone());
                }
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
                if let Some(observation) = target_entry.classes.default {
                    local_classes.insert(local.clone(), observation);
                }
                if let Some(class_name) = target_entry.entities.default.as_ref() {
                    local_entities.insert(local.clone(), class_name.clone());
                }
            }
            for local in &import.default_system_apis {
                if let Some(symbol) = target_entry.system_apis.default.as_ref() {
                    local_system_apis.insert(local.clone().into(), symbol.clone());
                }
            }
            for local in &import.namespace_system_apis {
                local_system_apis
                    .insert(local.clone().into(), target_entry.system_apis.namespace());
            }
        }

        SystemApiModuleFacts::resolve_local_references(&mut local_system_apis);
        for (local, exported) in &facts.direct_system_api_bindings {
            if let Some(symbol) = local_system_apis.get(local) {
                entry.system_apis.insert(exported.clone(), symbol.clone());
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
                        if let Some(observation) = target_entry.classes.get(&binding.local) {
                            if binding.exported == ComponentModuleExportName::DEFAULT {
                                entry.classes.default = Some(observation);
                            } else {
                                entry
                                    .classes
                                    .named
                                    .insert(binding.exported.clone(), observation);
                            }
                        }
                        if let Some(class_name) = target_entry.entities.get(&binding.local) {
                            if binding.exported == ComponentModuleExportName::DEFAULT {
                                entry.entities.default = Some(class_name.to_string());
                            } else {
                                entry
                                    .entities
                                    .named
                                    .insert(binding.exported.clone(), class_name.to_string());
                            }
                        }
                        if let Some(symbol) = target_entry.system_apis.get(&binding.local) {
                            entry
                                .system_apis
                                .insert(binding.exported.clone(), symbol.clone());
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
                        if local_builders.contains(&binding.local) {
                            entry.builders.insert(binding.exported.clone());
                        }
                        if let Some(observation) = local_classes.get(&binding.local).copied() {
                            if binding.exported == ComponentModuleExportName::DEFAULT {
                                entry.classes.default = Some(observation);
                            } else {
                                entry
                                    .classes
                                    .named
                                    .insert(binding.exported.clone(), observation);
                            }
                        }
                        if let Some(class_name) = local_entities.get(&binding.local) {
                            if binding.exported == ComponentModuleExportName::DEFAULT {
                                entry.entities.default = Some(class_name.clone());
                            } else {
                                entry
                                    .entities
                                    .named
                                    .insert(binding.exported.clone(), class_name.clone());
                            }
                        }
                        if let Some(symbol) = local_system_apis.get(binding.local.as_str()) {
                            entry
                                .system_apis
                                .insert(binding.exported.clone(), symbol.clone());
                        }
                    }
                }
                ModuleExportFact::DefaultLocal { local } => {
                    if local_builders.contains(local) {
                        entry
                            .builders
                            .insert(ComponentModuleExportName::DEFAULT.to_string());
                    }
                    if let Some(info) = local_components.get(local) {
                        entry.components.has_default = true;
                        entry.components.default_info = info.clone();
                    }
                    if let Some(observation) = local_classes.get(local).copied() {
                        entry.classes.default = Some(observation);
                    }
                    if let Some(class_name) = local_entities.get(local) {
                        entry.entities.default = Some(class_name.clone());
                    }
                    if let Some(symbol) = local_system_apis.get(local.as_str()) {
                        entry.system_apis.insert("default", symbol.clone());
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
                    entry.classes.extend_named(&target_entry.classes);
                    entry.entities.named.extend(target_entry.entities.named);
                    entry.system_apis.extend_named(&target_entry.system_apis);
                }
                ModuleExportFact::NamespaceFrom { target, exported } => {
                    let target_entry = self.export_entry(target);
                    if !target_entry.components.is_empty() {
                        entry
                            .components
                            .namespaces
                            .insert(exported.clone(), target_entry.components);
                    }
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
    namespaces: FxHashMap<String, ExportedComponentNames>,
}

impl ExportedComponentNames {
    fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.has_default && self.namespaces.is_empty()
    }

    fn extend(&mut self, other: ExportedComponentNames) {
        self.named.extend(other.named);
        self.namespaces.extend(other.namespaces);
        if other.has_default {
            self.has_default = true;
            self.default_info = other.default_info;
        }
    }

    fn visit_paths(&self, prefix: &str, visitor: &mut impl FnMut(String, &CustomComponentInfo)) {
        for (name, info) in &self.named {
            visitor(format!("{prefix}.{name}"), info);
        }
        if self.has_default {
            visitor(
                format!("{prefix}.{}", ComponentModuleExportName::DEFAULT),
                &self.default_info,
            );
        }
        for (name, namespace) in &self.namespaces {
            namespace.visit_paths(&format!("{prefix}.{name}"), visitor);
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
            Statement::ExportDeclaration(export) => {
                if let Declaration::StructStatement(statement) = &export.declaration {
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

pub(super) struct ImportTargetResolver<'a> {
    importer: &'a Path,
    sources: &'a crate::source::TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
}

impl<'a> ImportTargetResolver<'a> {
    pub(super) fn new(
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

    pub(super) fn resolve(&self, specifier: &str) -> Option<TransformSourceFile> {
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
