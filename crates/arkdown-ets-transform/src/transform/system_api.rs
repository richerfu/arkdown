use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};

use oxc_ast::{
    ast::{
        Class, ClassElement, Declaration, Decorator, ExportDefaultDeclarationKind, Expression,
        FormalParameters, ImportDeclarationSpecifier, ModuleExportName, Program, PropertyKey,
        Statement, TSInterfaceDeclaration, TSNamespaceDeclaration, TSNamespaceDeclarationBody,
        TSSignature, TSType, TSTypeAnnotation, TSTypeName,
    },
    AstKind,
};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{JSDoc, Scoping, SemanticBuilder};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::BinaryOperator, symbol::SymbolId};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    error::{DiagnosticSeverity, EtsTransformError},
    TransformSourceSnapshot,
};

use super::{
    api_available::{ApiAvailableSdkBindings, ApiAvailableTransformOptions, ApiAvailableValidator},
    available_annotation::{AvailableAnnotationValidator, AvailableSdkBindings},
    component_analysis::ImportTargetResolver,
    ModuleExportIndex,
};

/// Immutable build context consumed by ArkTS 1.1 system-API diagnostics.
///
/// The source compiler obtains these values from `projectConfig` before its
/// TypeScript language service starts. Arkdown keeps the equivalent data in
/// the native transform plan so individual Rolldown hooks never read profiles
/// or SDK metadata.
#[derive(Clone, Debug)]
pub struct SystemApiCheckOptions {
    compatible_sdk_version: Box<str>,
    compile_sdk_version: Box<str>,
    runtime_os: Box<str>,
    strict_api_compatibility: bool,
    request_permissions: FxHashSet<Box<str>>,
    syscap_intersection: FxHashSet<Box<str>>,
    syscap_union: FxHashSet<Box<str>>,
    check_syscap: bool,
    form_source: bool,
    atomic_service: bool,
    ohos_test_source: bool,
    crossplatform: bool,
    ignore_crossplatform_check: bool,
}

impl Default for SystemApiCheckOptions {
    fn default() -> Self {
        Self {
            compatible_sdk_version: "0".into(),
            compile_sdk_version: "0".into(),
            runtime_os: "OpenHarmony".into(),
            strict_api_compatibility: false,
            request_permissions: FxHashSet::default(),
            syscap_intersection: FxHashSet::default(),
            syscap_union: FxHashSet::default(),
            check_syscap: false,
            form_source: false,
            atomic_service: false,
            ohos_test_source: false,
            crossplatform: false,
            ignore_crossplatform_check: false,
        }
    }
}

impl SystemApiCheckOptions {
    pub fn with_compatible_sdk_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.compatible_sdk_version = version.into();
        self
    }

    pub fn with_strict_api_compatibility(mut self, strict: bool) -> Self {
        self.strict_api_compatibility = strict;
        self
    }

    pub fn with_compile_sdk_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.compile_sdk_version = version.into();
        self
    }

    pub fn with_runtime_os(mut self, runtime_os: impl Into<Box<str>>) -> Self {
        self.runtime_os = runtime_os.into();
        self
    }

    pub fn with_request_permissions<I, S>(mut self, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.request_permissions = permissions.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_system_capabilities<I, U, S>(mut self, intersection: I, union: U) -> Self
    where
        I: IntoIterator<Item = S>,
        U: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.syscap_intersection = intersection.into_iter().map(Into::into).collect();
        self.syscap_union = union.into_iter().map(Into::into).collect();
        self.check_syscap = true;
        self
    }

    pub fn with_form_source(mut self, form_source: bool) -> Self {
        self.form_source = form_source;
        self
    }

    pub fn with_atomic_service(mut self, atomic_service: bool) -> Self {
        self.atomic_service = atomic_service;
        self
    }

    pub fn with_ohos_test_source(mut self, ohos_test_source: bool) -> Self {
        self.ohos_test_source = ohos_test_source;
        self
    }

    pub fn with_crossplatform(mut self, crossplatform: bool, ignore_check: bool) -> Self {
        self.crossplatform = crossplatform;
        self.ignore_crossplatform_check = ignore_check;
        self
    }

    fn severity(&self) -> DiagnosticSeverity {
        if self.strict_api_compatibility {
            DiagnosticSeverity::Error
        } else {
            DiagnosticSeverity::Warning
        }
    }
}

/// Owned JSDoc contract attached to one SDK declaration symbol.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SystemApiContract {
    sdk_declaration: bool,
    since: Option<Box<str>>,
    available: Option<Box<str>>,
    permissions: Vec<Box<str>>,
    syscap: Option<Box<str>>,
    system_api: Option<Box<str>>,
    deprecated: bool,
    since_whitelisted: bool,
    form: Option<Box<str>>,
    crossplatform: Option<Box<str>>,
    atomic_service: Option<Box<str>>,
    fa_model_only: Option<Box<str>>,
    test_only: Option<Box<str>>,
}

impl SystemApiContract {
    fn is_empty(&self) -> bool {
        !self.sdk_declaration
            && self.since.is_none()
            && self.available.is_none()
            && self.permissions.is_empty()
            && self.syscap.is_none()
            && self.system_api.is_none()
            && !self.deprecated
            && self.form.is_none()
            && self.crossplatform.is_none()
            && self.atomic_service.is_none()
            && self.fa_model_only.is_none()
            && self.test_only.is_none()
    }

    fn merge(&mut self, other: &Self) {
        self.sdk_declaration |= other.sdk_declaration;
        if self.since.is_none() {
            self.since.clone_from(&other.since);
        }
        if self.available.is_none() {
            self.available.clone_from(&other.available);
        }
        for permission in &other.permissions {
            if !self.permissions.contains(permission) {
                self.permissions.push(permission.clone());
            }
        }
        if self.syscap.is_none() {
            self.syscap.clone_from(&other.syscap);
        }
        if self.system_api.is_none() {
            self.system_api.clone_from(&other.system_api);
        }
        self.deprecated |= other.deprecated;
        self.since_whitelisted |= other.since_whitelisted;
        if self.form.is_none() {
            self.form.clone_from(&other.form);
        }
        if self.crossplatform.is_none() {
            self.crossplatform.clone_from(&other.crossplatform);
        }
        if self.atomic_service.is_none() {
            self.atomic_service.clone_from(&other.atomic_service);
        }
        if self.fa_model_only.is_none() {
            self.fa_model_only.clone_from(&other.fa_model_only);
        }
        if self.test_only.is_none() {
            self.test_only.clone_from(&other.test_only);
        }
    }
}

/// Export/type tree for an API symbol. References are retained as owned names
/// until the module export closure has resolved imports and declaration merges.
#[derive(Clone, Debug, Default)]
pub(super) struct SystemApiSymbol {
    contract: SystemApiContract,
    members: FxHashMap<Box<str>, SystemApiSymbol>,
    references: Vec<Vec<Box<str>>>,
    call_signatures: Vec<SystemApiCallSignature>,
    awaited: Option<Box<SystemApiSymbol>>,
    type_table: Option<Arc<FxHashMap<Box<str>, SystemApiSymbol>>>,
}

impl PartialEq for SystemApiSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.contract == other.contract
            && self.members == other.members
            && self.references == other.references
            && self.call_signatures == other.call_signatures
            && self.awaited == other.awaited
    }
}

impl Eq for SystemApiSymbol {}

impl SystemApiSymbol {
    fn is_empty(&self) -> bool {
        self.contract.is_empty()
            && self.members.is_empty()
            && self.references.is_empty()
            && self.call_signatures.is_empty()
            && self.awaited.is_none()
    }

    pub(super) fn merge(&mut self, other: &Self) {
        self.contract.merge(&other.contract);
        for (name, symbol) in &other.members {
            self.members.entry(name.clone()).or_default().merge(symbol);
        }
        for reference in &other.references {
            if !self.references.contains(reference) {
                self.references.push(reference.clone());
            }
        }
        for signature in &other.call_signatures {
            if !self.call_signatures.contains(signature) {
                self.call_signatures.push(signature.clone());
            }
        }
        if let Some(other_awaited) = &other.awaited {
            self.awaited
                .get_or_insert_with(|| Box::new(SystemApiSymbol::default()))
                .merge(other_awaited);
        }
        if self.type_table.is_none() {
            self.type_table.clone_from(&other.type_table);
        }
    }

    fn merge_type_shape(&mut self, other: &Self) {
        for (name, symbol) in &other.members {
            self.members.entry(name.clone()).or_default().merge(symbol);
        }
        for signature in &other.call_signatures {
            if !self.call_signatures.contains(signature) {
                self.call_signatures.push(signature.clone());
            }
        }
        if let Some(other_awaited) = &other.awaited {
            self.awaited
                .get_or_insert_with(|| Box::new(SystemApiSymbol::default()))
                .merge_type_shape(other_awaited);
        }
        if self.type_table.is_none() {
            self.type_table.clone_from(&other.type_table);
        }
    }

    fn type_shape(&self) -> Self {
        Self {
            members: self.members.clone(),
            call_signatures: self.call_signatures.clone(),
            awaited: self.awaited.clone(),
            type_table: self.type_table.clone(),
            ..Self::default()
        }
    }

    fn resolved_member(&self, name: &str) -> Option<Self> {
        self.resolved_member_with(name, &mut FxHashSet::default())
    }

    fn resolved_member_with(
        &self,
        name: &str,
        active_references: &mut FxHashSet<Vec<Box<str>>>,
    ) -> Option<Self> {
        let mut result = self.members.get(name).cloned();
        for reference in &self.references {
            if !active_references.insert(reference.clone()) {
                continue;
            }
            let referenced_member = self.type_table.as_ref().and_then(|table| {
                let root = reference.first()?;
                let mut target = table.get(root)?.clone();
                target.type_table = Some(Arc::clone(table));
                for segment in &reference[1..] {
                    target = target.resolved_member_with(segment, active_references)?;
                }
                target.resolved_member_with(name, active_references)
            });
            active_references.remove(reference);
            if let Some(referenced_member) = referenced_member {
                result
                    .get_or_insert_with(SystemApiSymbol::default)
                    .merge(&referenced_member);
            }
        }
        if let Some(result) = result.as_mut() {
            if result.type_table.is_none() {
                result.type_table.clone_from(&self.type_table);
            }
        }
        result
    }

    fn add_type_projection(&mut self, projection: SystemApiTypeProjection) {
        match projection {
            SystemApiTypeProjection::Named(path) => {
                if !path.is_empty() && !self.references.contains(&path) {
                    self.references.push(path);
                }
            }
            SystemApiTypeProjection::Awaited(inner) => {
                let awaited = self
                    .awaited
                    .get_or_insert_with(|| Box::new(SystemApiSymbol::default()));
                awaited.add_type_projection(*inner);
            }
            SystemApiTypeProjection::Composite(projections) => {
                for projection in projections {
                    self.add_type_projection(projection);
                }
            }
        }
    }

    fn call_result(&self, argument_count: usize) -> Option<Self> {
        let mut matches = self
            .call_signatures
            .iter()
            .filter(|signature| signature.accepts(argument_count));
        let first = matches.next()?;
        let Some(second) = matches.next() else {
            let mut result = first.result.clone();
            result.type_table.clone_from(&self.type_table);
            return Some(result);
        };
        let mut result = first.result.type_shape();
        result.merge_type_shape(&second.result);
        for signature in matches {
            result.merge_type_shape(&signature.result);
        }
        result.type_table.clone_from(&self.type_table);
        Some(result)
    }

    fn awaited_result(&self) -> Option<Self> {
        let mut result = self.awaited.as_deref()?.clone();
        result.type_table.clone_from(&self.type_table);
        Some(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SystemApiCallSignature {
    min_arguments: usize,
    max_arguments: Option<usize>,
    result: SystemApiSymbol,
}

impl SystemApiCallSignature {
    fn new(params: &FormalParameters<'_>, return_type: Option<&TSTypeAnnotation<'_>>) -> Self {
        let min_arguments = params
            .items
            .iter()
            .filter(|parameter| !parameter.optional && parameter.initializer.is_none())
            .count();
        let max_arguments = params.rest.is_none().then_some(params.items.len());
        let mut result = SystemApiSymbol::default();
        if let Some(projection) = return_type
            .and_then(|annotation| SystemApiTypeProjection::new(&annotation.type_annotation))
        {
            result.add_type_projection(projection);
        }
        Self {
            min_arguments,
            max_arguments,
            result,
        }
    }

    fn accepts(&self, argument_count: usize) -> bool {
        argument_count >= self.min_arguments
            && self
                .max_arguments
                .is_none_or(|maximum| argument_count <= maximum)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ExportedSystemApiSymbols {
    pub(super) named: FxHashMap<Box<str>, SystemApiSymbol>,
    pub(super) default: Option<SystemApiSymbol>,
}

impl ExportedSystemApiSymbols {
    pub(super) fn get(&self, name: &str) -> Option<&SystemApiSymbol> {
        if name == "default" {
            self.default.as_ref()
        } else {
            self.named.get(name)
        }
    }

    pub(super) fn insert(&mut self, name: impl Into<Box<str>>, symbol: SystemApiSymbol) {
        let name = name.into();
        if name.as_ref() == "default" {
            self.default
                .get_or_insert_with(SystemApiSymbol::default)
                .merge(&symbol);
        } else {
            self.named.entry(name).or_default().merge(&symbol);
        }
    }

    pub(super) fn namespace(&self) -> SystemApiSymbol {
        let mut namespace = SystemApiSymbol::default();
        for (name, symbol) in &self.named {
            namespace.members.insert(name.clone(), symbol.clone());
        }
        if let Some(default) = &self.default {
            namespace.members.insert("default".into(), default.clone());
        }
        namespace
    }

    pub(super) fn extend_named(&mut self, other: &Self) {
        for (name, symbol) in &other.named {
            self.insert(name.clone(), symbol.clone());
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.named.is_empty() && self.default.is_none()
    }
}

/// SDK declaration facts collected during the same parallel parse pass as the
/// existing module export inventory.
#[derive(Debug, Default)]
pub(super) struct SystemApiModuleFacts {
    pub(super) local: FxHashMap<Box<str>, SystemApiSymbol>,
    pub(super) direct: ExportedSystemApiSymbols,
    pub(super) direct_bindings: Vec<(Box<str>, Box<str>)>,
}

impl SystemApiModuleFacts {
    pub(super) fn from_program(
        path: &Path,
        program: &Program<'_>,
        is_sdk: bool,
        is_project: bool,
    ) -> Self {
        if !is_sdk && (!is_project || !program.source_text.contains("Available")) {
            return Self::default();
        }
        let available_contracts = if is_project {
            let semantic = SemanticBuilder::new().build(program).semantic;
            ProjectAvailableContractInventory::new(program, semantic.scoping()).collect()
        } else {
            FxHashMap::default()
        };
        let mut collector = SystemApiDeclarationCollector::new(
            path,
            is_sdk.then(|| SystemApiJsDocInventory::from_program(program)),
            available_contracts,
        );
        collector.collect_program(program);
        collector.finish()
    }

    pub(super) fn resolve_local_references(local: &mut FxHashMap<Box<str>, SystemApiSymbol>) {
        let type_table = Arc::new(local.clone());
        for symbol in local.values_mut() {
            symbol.type_table = Some(Arc::clone(&type_table));
        }
    }
}

struct ProjectAvailableContractInventory<'a, 'program> {
    program: &'program Program<'program>,
    scoping: &'a Scoping,
    bindings: AvailableSdkBindings,
    by_span: FxHashMap<u32, Box<str>>,
}

impl<'a, 'program> ProjectAvailableContractInventory<'a, 'program> {
    fn new(program: &'program Program<'program>, scoping: &'a Scoping) -> Self {
        Self {
            program,
            scoping,
            bindings: AvailableSdkBindings::from_program(program),
            by_span: FxHashMap::default(),
        }
    }

    fn collect(mut self) -> FxHashMap<u32, Box<str>> {
        if !self.bindings.is_empty() {
            self.visit_program(self.program);
        }
        self.by_span
    }

    fn decorator_version(&self, decorator: &Decorator<'program>) -> Option<Box<str>> {
        let Expression::CallExpression(call) = &decorator.expression else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        if !self.bindings.contains(callee, self.scoping) {
            return None;
        }
        let Expression::ObjectExpression(object) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())?
        else {
            return None;
        };
        for property in &object.properties {
            let property = property.as_property()?;
            if property.key.static_name().as_deref() != Some("minApiVersion") {
                continue;
            }
            return match &property.value {
                Expression::StringLiteral(value) => Some(value.value.as_str().into()),
                Expression::NumericLiteral(value) => Some(value.value.to_string().into()),
                _ => None,
            };
        }
        None
    }
}

impl<'program> Visit<'program> for ProjectAvailableContractInventory<'_, 'program> {
    fn enter_node(&mut self, kind: AstKind<'program>) {
        let version = SystemApiAncestorDecorators::new(kind)
            .iter()
            .find_map(|decorator| self.decorator_version(decorator));
        if let Some(version) = version {
            self.by_span.entry(kind.span().start).or_insert(version);
        }
    }
}

#[derive(Clone, Default)]
struct SystemApiJsDocInventory {
    by_attached_to: FxHashMap<u32, Vec<SystemApiJsDocTag>>,
}

impl SystemApiJsDocInventory {
    fn from_program(program: &Program<'_>) -> Self {
        let mut inventory = Self::default();
        for comment in &program.comments {
            if !comment.is_jsdoc() || comment.span.end < comment.span.start + 5 {
                continue;
            }
            let content_span = Span::new(comment.span.start + 3, comment.span.end - 2);
            let Some(content) = program
                .source_text
                .get(content_span.start as usize..content_span.end as usize)
            else {
                continue;
            };
            let document = JSDoc::new(content, content_span);
            let tags = document
                .tags()
                .iter()
                .map(|tag| SystemApiJsDocTag {
                    name: tag.kind.parsed().to_ascii_lowercase().into(),
                    value: tag.comment().parsed().into(),
                })
                .collect::<Vec<_>>();
            inventory
                .by_attached_to
                .entry(comment.attached_to)
                .or_default()
                .extend(tags);
        }
        inventory
    }

    fn tags(&self, span: Span) -> Option<&[SystemApiJsDocTag]> {
        self.by_attached_to.get(&span.start).map(Vec::as_slice)
    }
}

#[derive(Clone)]
struct SystemApiJsDocTag {
    name: Box<str>,
    value: Box<str>,
}

struct SystemApiDeclarationCollector<'a> {
    declaration_path: &'a Path,
    jsdoc: Option<SystemApiJsDocInventory>,
    available_contracts: FxHashMap<u32, Box<str>>,
    facts: SystemApiModuleFacts,
}

impl<'a> SystemApiDeclarationCollector<'a> {
    fn new(
        declaration_path: &'a Path,
        jsdoc: Option<SystemApiJsDocInventory>,
        available_contracts: FxHashMap<u32, Box<str>>,
    ) -> Self {
        Self {
            declaration_path,
            jsdoc,
            available_contracts,
            facts: SystemApiModuleFacts::default(),
        }
    }

    fn collect_program<'program>(&mut self, program: &'program Program<'program>) {
        for statement in &program.body {
            if let Statement::TSExternalModuleDeclaration(module) = statement {
                if let Some(body) = &module.body {
                    self.collect_module_block(&body.body, true);
                }
            } else {
                self.collect_statement(statement, false);
            }
        }
    }

    fn collect_module_block<'program>(
        &mut self,
        statements: &'program [Statement<'program>],
        ambient: bool,
    ) {
        for statement in statements {
            self.collect_statement(statement, ambient);
        }
    }

    fn collect_statement<'program>(
        &mut self,
        statement: &'program Statement<'program>,
        ambient: bool,
    ) {
        match statement {
            Statement::ExportDeclaration(export) => {
                self.collect_declaration(&export.declaration, Some(export.span), true);
            }
            Statement::ExportDefaultDeclaration(export) => {
                self.collect_default_declaration(&export.declaration, export.span);
            }
            Statement::ExportNamedDeclaration(export) => {
                for specifier in &export.specifiers {
                    self.facts.direct_bindings.push((
                        SystemApiExportName::value(&specifier.local).into(),
                        SystemApiExportName::value(&specifier.exported).into(),
                    ));
                }
            }
            Statement::TSExternalModuleDeclaration(module) => {
                if let Some(body) = &module.body {
                    self.collect_module_block(&body.body, true);
                }
            }
            Statement::TSNamespaceDeclaration(namespace) => {
                self.collect_namespace(namespace, None, ambient);
            }
            declaration if declaration.is_declaration() => {
                if let Some(declaration) = declaration.as_declaration() {
                    self.collect_declaration(declaration, None, ambient);
                }
            }
            _ => {}
        }
    }

    fn collect_default_declaration<'program>(
        &mut self,
        declaration: &'program ExportDefaultDeclarationKind<'program>,
        wrapper_span: Span,
    ) {
        match declaration {
            ExportDefaultDeclarationKind::Identifier(identifier) => {
                self.facts
                    .direct_bindings
                    .push((identifier.name.as_str().into(), "default".into()));
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                let symbol = self.function_symbol(function, Some(wrapper_span));
                if let Some(id) = &function.id {
                    self.insert_local(id.name.as_str(), symbol.clone());
                }
                self.facts.direct.insert("default", symbol);
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                let symbol = self.class_symbol(class, Some(wrapper_span));
                if let Some(id) = &class.id {
                    self.insert_local(id.name.as_str(), symbol.clone());
                }
                self.facts.direct.insert("default", symbol);
            }
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
                let symbol = self.interface_symbol(interface, Some(wrapper_span));
                self.insert_local(interface.id.name.as_str(), symbol.clone());
                self.facts.direct.insert("default", symbol);
            }
            _ => {}
        }
    }

    fn collect_declaration<'program>(
        &mut self,
        declaration: &'program Declaration<'program>,
        wrapper_span: Option<Span>,
        exported: bool,
    ) {
        match declaration {
            Declaration::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    let symbol = self.function_symbol(function, wrapper_span);
                    self.insert_local(id.name.as_str(), symbol);
                    self.record_direct_binding(id.name.as_str(), exported);
                }
            }
            Declaration::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    let symbol = self.class_symbol(class, wrapper_span);
                    self.insert_local(id.name.as_str(), symbol);
                    self.record_direct_binding(id.name.as_str(), exported);
                }
            }
            Declaration::TSInterfaceDeclaration(interface) => {
                let symbol = self.interface_symbol(interface, wrapper_span);
                self.insert_local(interface.id.name.as_str(), symbol);
                self.record_direct_binding(interface.id.name.as_str(), exported);
            }
            Declaration::TSTypeAliasDeclaration(alias) => {
                let mut symbol = self.symbol_for_spans([wrapper_span, Some(alias.span)]);
                if let Some(projection) = SystemApiTypeProjection::new(&alias.type_annotation) {
                    symbol.add_type_projection(projection);
                }
                self.insert_local(alias.id.name.as_str(), symbol);
                self.record_direct_binding(alias.id.name.as_str(), exported);
            }
            Declaration::VariableDeclaration(variable) => {
                for declaration in &variable.declarations {
                    let Some(id) = declaration.id.get_binding_identifier() else {
                        continue;
                    };
                    let mut symbol = self.symbol_for_spans([
                        wrapper_span,
                        Some(declaration.span),
                        Some(variable.span),
                    ]);
                    if let Some(annotation) = &declaration.type_annotation {
                        if let Some(projection) =
                            SystemApiTypeProjection::new(&annotation.type_annotation)
                        {
                            symbol.add_type_projection(projection);
                        }
                    }
                    self.insert_local(id.name.as_str(), symbol);
                    self.record_direct_binding(id.name.as_str(), exported);
                }
            }
            Declaration::TSEnumDeclaration(enumeration) => {
                let mut symbol = self.symbol_for_spans([wrapper_span, Some(enumeration.span)]);
                for member in &enumeration.body.members {
                    let contract = self.contract_for_spans([Some(member.span)]);
                    if !contract.is_empty() {
                        symbol
                            .members
                            .entry(member.id.static_name().as_str().into())
                            .or_default()
                            .contract = contract;
                    }
                }
                self.insert_local(enumeration.id.name.as_str(), symbol);
                self.record_direct_binding(enumeration.id.name.as_str(), exported);
            }
            Declaration::TSNamespaceDeclaration(namespace) => {
                self.collect_namespace(namespace, wrapper_span, exported);
            }
            Declaration::TSExternalModuleDeclaration(module) => {
                if let Some(body) = &module.body {
                    self.collect_module_block(&body.body, true);
                }
            }
            _ => {}
        }
    }

    fn collect_namespace<'program>(
        &mut self,
        namespace: &'program TSNamespaceDeclaration<'program>,
        wrapper_span: Option<Span>,
        exported: bool,
    ) {
        let mut nested = Self {
            declaration_path: self.declaration_path,
            jsdoc: self.jsdoc.clone(),
            available_contracts: self.available_contracts.clone(),
            facts: SystemApiModuleFacts::default(),
        };
        match &namespace.body {
            TSNamespaceDeclarationBody::TSModuleBlock(block) => {
                nested.collect_module_block(&block.body, true);
            }
            TSNamespaceDeclarationBody::TSNamespaceDeclaration(child) => {
                nested.collect_namespace(child, None, true);
            }
        }
        let mut symbol = self.symbol_for_spans([wrapper_span, Some(namespace.span)]);
        SystemApiModuleFacts::resolve_local_references(&mut nested.facts.local);
        for (local, exported_name) in &nested.facts.direct_bindings {
            if let Some(member) = nested.facts.local.get(local) {
                nested
                    .facts
                    .direct
                    .insert(exported_name.clone(), member.clone());
            }
        }
        symbol.members.extend(nested.facts.direct.named);
        if let Some(default) = nested.facts.direct.default {
            symbol.members.insert("default".into(), default);
        }
        self.insert_local(namespace.id.name.as_str(), symbol);
        self.record_direct_binding(namespace.id.name.as_str(), exported);
    }

    fn class_symbol<'program>(
        &self,
        class: &'program Class<'program>,
        wrapper_span: Option<Span>,
    ) -> SystemApiSymbol {
        let mut symbol = self.symbol_for_spans([wrapper_span, Some(class.span)]);
        if let Some(heritage) = &class.heritage {
            if let Some(reference) = SystemApiExpressionPath::new(&heritage.expression) {
                symbol.references.push(reference.path);
            }
        }
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if method.kind == oxc_ast::ast::MethodDefinitionKind::Get {
                        let projection = method.value.return_type.as_ref().and_then(|annotation| {
                            SystemApiTypeProjection::new(&annotation.type_annotation)
                        });
                        self.insert_member(&mut symbol, &method.key, method.span, projection);
                    } else {
                        self.insert_callable_member(
                            &mut symbol,
                            &method.key,
                            method.span,
                            &method.value.params,
                            method.value.return_type.as_deref(),
                        );
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    let projection = property.type_annotation.as_ref().and_then(|annotation| {
                        SystemApiTypeProjection::new(&annotation.type_annotation)
                    });
                    self.insert_member(&mut symbol, &property.key, property.span, projection);
                }
                ClassElement::AccessorProperty(property) => {
                    let projection = property.type_annotation.as_ref().and_then(|annotation| {
                        SystemApiTypeProjection::new(&annotation.type_annotation)
                    });
                    self.insert_member(&mut symbol, &property.key, property.span, projection);
                }
                ClassElement::ETSOverloadDeclaration(overload) => {
                    self.insert_member(&mut symbol, &overload.key, overload.span, None);
                }
                _ => {}
            }
        }
        symbol
    }

    fn interface_symbol<'program>(
        &self,
        interface: &'program TSInterfaceDeclaration<'program>,
        wrapper_span: Option<Span>,
    ) -> SystemApiSymbol {
        let mut symbol = self.symbol_for_spans([wrapper_span, Some(interface.span)]);
        for heritage in &interface.extends {
            symbol
                .references
                .push(SystemApiTypeName::new(&heritage.type_name).path());
        }
        for signature in &interface.body.body {
            match signature {
                TSSignature::TSMethodSignature(method) => {
                    if method.kind == oxc_ast::ast::TSMethodSignatureKind::Get {
                        let projection = method.return_type.as_ref().and_then(|annotation| {
                            SystemApiTypeProjection::new(&annotation.type_annotation)
                        });
                        self.insert_member(&mut symbol, &method.key, method.span, projection);
                    } else {
                        self.insert_callable_member(
                            &mut symbol,
                            &method.key,
                            method.span,
                            &method.params,
                            method.return_type.as_deref(),
                        );
                    }
                }
                TSSignature::TSPropertySignature(property) => {
                    let projection = property.type_annotation.as_ref().and_then(|annotation| {
                        SystemApiTypeProjection::new(&annotation.type_annotation)
                    });
                    self.insert_member(&mut symbol, &property.key, property.span, projection);
                }
                TSSignature::MethodDefinition(method) => {
                    if method.kind == oxc_ast::ast::MethodDefinitionKind::Get {
                        let projection = method.value.return_type.as_ref().and_then(|annotation| {
                            SystemApiTypeProjection::new(&annotation.type_annotation)
                        });
                        self.insert_member(&mut symbol, &method.key, method.span, projection);
                    } else {
                        self.insert_callable_member(
                            &mut symbol,
                            &method.key,
                            method.span,
                            &method.value.params,
                            method.value.return_type.as_deref(),
                        );
                    }
                }
                TSSignature::PropertyDefinition(property) => {
                    let projection = property.type_annotation.as_ref().and_then(|annotation| {
                        SystemApiTypeProjection::new(&annotation.type_annotation)
                    });
                    self.insert_member(&mut symbol, &property.key, property.span, projection);
                }
                TSSignature::ETSOverloadDeclaration(overload) => {
                    self.insert_member(&mut symbol, &overload.key, overload.span, None);
                }
                _ => {}
            }
        }
        symbol
    }

    fn function_symbol<'program>(
        &self,
        function: &'program oxc_ast::ast::Function<'program>,
        wrapper_span: Option<Span>,
    ) -> SystemApiSymbol {
        let mut symbol = self.symbol_for_spans([wrapper_span, Some(function.span)]);
        symbol.call_signatures.push(SystemApiCallSignature::new(
            &function.params,
            function.return_type.as_deref(),
        ));
        symbol
    }

    fn insert_member<'program>(
        &self,
        owner: &mut SystemApiSymbol,
        key: &PropertyKey<'program>,
        span: Span,
        projection: Option<SystemApiTypeProjection>,
    ) {
        let Some(name) = key.static_name() else {
            return;
        };
        let member = owner.members.entry(name.into_owned().into()).or_default();
        member
            .contract
            .merge(&self.contract_for_spans([Some(span)]));
        if let Some(projection) = projection {
            member.add_type_projection(projection);
        }
    }

    fn insert_callable_member<'program>(
        &self,
        owner: &mut SystemApiSymbol,
        key: &PropertyKey<'program>,
        span: Span,
        params: &FormalParameters<'program>,
        return_type: Option<&TSTypeAnnotation<'program>>,
    ) {
        let Some(name) = key.static_name() else {
            return;
        };
        let member = owner.members.entry(name.into_owned().into()).or_default();
        member
            .contract
            .merge(&self.contract_for_spans([Some(span)]));
        let signature = SystemApiCallSignature::new(params, return_type);
        if !member.call_signatures.contains(&signature) {
            member.call_signatures.push(signature);
        }
    }

    fn symbol_for_spans<const N: usize>(&self, spans: [Option<Span>; N]) -> SystemApiSymbol {
        SystemApiSymbol {
            contract: self.contract_for_spans(spans),
            ..SystemApiSymbol::default()
        }
    }

    fn contract_for_spans<const N: usize>(&self, spans: [Option<Span>; N]) -> SystemApiContract {
        let mut contract = SystemApiContract {
            sdk_declaration: self.jsdoc.is_some(),
            ..SystemApiContract::default()
        };
        for span in spans.into_iter().flatten() {
            if contract.available.is_none() {
                contract.available = self.available_contracts.get(&span.start).cloned();
            }
            if let Some(tags) = self.jsdoc.as_ref().and_then(|jsdoc| jsdoc.tags(span)) {
                for tag in tags {
                    let value = tag.value.as_ref();
                    match tag.name.as_ref() {
                        "since"
                            if contract.since.is_none()
                                && SystemApiVersion::is_valid_since(value) =>
                        {
                            contract.since = Some(value.trim().into());
                        }
                        "permission" if !value.trim().is_empty() => {
                            let value: Box<str> = value.trim().into();
                            if !contract.permissions.contains(&value) {
                                contract.permissions.push(value);
                            }
                        }
                        "syscap" if contract.syscap.is_none() && !value.trim().is_empty() => {
                            contract.syscap = Some(value.trim().into());
                        }
                        "systemapi" => contract.system_api = Some(value.trim().into()),
                        "deprecated" => contract.deprecated = true,
                        "form" => contract.form = Some(value.trim().into()),
                        "crossplatform" => {
                            contract.crossplatform = Some(value.trim().into());
                        }
                        "atomicservice" => {
                            contract.atomic_service = Some(value.trim().into());
                        }
                        "famodelonly" => {
                            contract.fa_model_only = Some(value.trim().into());
                        }
                        "test" => contract.test_only = Some(value.trim().into()),
                        _ => {}
                    }
                }
            }
            if !contract.is_empty() {
                break;
            }
        }
        contract
    }

    fn insert_local(&mut self, name: &str, mut symbol: SystemApiSymbol) {
        symbol.contract.since_whitelisted |=
            SystemApiSinceWhitelist::new(self.declaration_path, name).contains();
        if symbol.is_empty() {
            return;
        }
        self.facts
            .local
            .entry(name.into())
            .or_default()
            .merge(&symbol);
    }

    fn record_direct_binding(&mut self, name: &str, exported: bool) {
        if exported {
            self.facts.direct_bindings.push((name.into(), name.into()));
        }
    }

    fn finish(mut self) -> SystemApiModuleFacts {
        SystemApiModuleFacts::resolve_local_references(&mut self.facts.local);
        for (local, exported) in &self.facts.direct_bindings {
            if let Some(symbol) = self.facts.local.get(local) {
                self.facts.direct.insert(exported.clone(), symbol.clone());
            }
        }
        self.facts
    }
}

struct SystemApiSinceWhitelist<'a> {
    path: &'a Path,
    name: &'a str,
}

impl<'a> SystemApiSinceWhitelist<'a> {
    fn new(path: &'a Path, name: &'a str) -> Self {
        Self { path, name }
    }

    fn contains(&self) -> bool {
        let Some(file_name) = self.path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        match file_name {
            "@arkts.lang.d.ets" => matches!(
                self.name,
                "RetentionPolicy" | "Retention" | "SOURCE" | "BYTECODE"
            ),
            "@ohos.deviceInfo.d.ts" => self.name == "apiAvailable",
            _ => false,
        }
    }
}

enum SystemApiTypeProjection {
    Named(Vec<Box<str>>),
    Awaited(Box<SystemApiTypeProjection>),
    Composite(Vec<SystemApiTypeProjection>),
}

impl SystemApiTypeProjection {
    fn new(ty: &TSType<'_>) -> Option<Self> {
        match ty {
            TSType::TSTypeReference(reference) => {
                let path = SystemApiTypeName::new(&reference.type_name).path();
                if path
                    .last()
                    .is_some_and(|name| matches!(name.as_ref(), "Promise" | "PromiseLike"))
                {
                    let inner = reference
                        .type_arguments
                        .as_ref()
                        .and_then(|arguments| arguments.params.first())
                        .and_then(Self::new)?;
                    Some(Self::Awaited(Box::new(inner)))
                } else {
                    (!path.is_empty()).then_some(Self::Named(path))
                }
            }
            TSType::TSParenthesizedType(parenthesized) => Self::new(&parenthesized.type_annotation),
            TSType::TSArrayType(array) => Self::new(&array.element_type),
            TSType::TSTypeOperatorType(operator) => Self::new(&operator.type_annotation),
            TSType::TSUnionType(union) => Self::from_many(&union.types),
            TSType::TSIntersectionType(intersection) => Self::from_many(&intersection.types),
            _ => None,
        }
    }

    fn from_many(types: &[TSType<'_>]) -> Option<Self> {
        let projections = types.iter().filter_map(Self::new).collect::<Vec<_>>();
        match projections.len() {
            0 => None,
            1 => projections.into_iter().next(),
            _ => Some(Self::Composite(projections)),
        }
    }
}

struct SystemApiTypeName<'a> {
    name: &'a TSTypeName<'a>,
}

impl<'a> SystemApiTypeName<'a> {
    fn new(name: &'a TSTypeName<'a>) -> Self {
        Self { name }
    }

    fn path(&self) -> Vec<Box<str>> {
        match self.name {
            TSTypeName::IdentifierReference(identifier) => vec![identifier.name.as_str().into()],
            TSTypeName::QualifiedName(qualified) => {
                let mut path = Self::new(&qualified.left).path();
                path.push(qualified.right.name.as_str().into());
                path
            }
            TSTypeName::ThisExpression(_) => Vec::new(),
        }
    }
}

struct SystemApiExpressionPath {
    path: Vec<Box<str>>,
}

impl SystemApiExpressionPath {
    fn new(expression: &Expression<'_>) -> Option<Self> {
        let (_, path, _) = SystemApiUsePath::resolve(expression)?;
        Some(Self { path })
    }
}

struct SystemApiExportName;

impl SystemApiExportName {
    fn value<'a>(name: &'a ModuleExportName<'a>) -> &'a str {
        match name {
            ModuleExportName::IdentifierName(identifier) => identifier.name.as_str(),
            ModuleExportName::IdentifierReference(identifier) => identifier.name.as_str(),
            ModuleExportName::StringLiteral(literal) => literal.value.as_str(),
        }
    }
}

/// Current-module import bindings resolved to SDK declaration contracts.
#[derive(Clone, Debug, Default)]
pub(super) struct ImportedSystemApiBindings {
    symbols: FxHashMap<SymbolId, SystemApiSymbol>,
}

impl ImportedSystemApiBindings {
    pub(super) fn from_program(
        program: &Program<'_>,
        importer: &Path,
        sources: &TransformSourceSnapshot,
        module_roots: &[(String, PathBuf)],
        index: &ModuleExportIndex,
        scoping: &Scoping,
    ) -> Self {
        let resolver = ImportTargetResolver::new(importer, sources, module_roots);
        let mut bindings = Self::default();
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
            let Some(exports) = index.system_api_exports(&target.path) else {
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
                        if let Some(symbol) =
                            exports.get(SystemApiExportName::value(&specifier.imported))
                        {
                            bindings
                                .symbols
                                .insert(specifier.local.symbol_id(), symbol.clone());
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        if let Some(symbol) = exports.default.as_ref() {
                            bindings
                                .symbols
                                .insert(specifier.local.symbol_id(), symbol.clone());
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        bindings
                            .symbols
                            .insert(specifier.local.symbol_id(), exports.namespace());
                    }
                    _ => {}
                }
            }
        }
        bindings.collect_local_value_bindings(program, scoping);
        bindings
    }

    fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    fn root_symbol<'a>(
        &'a self,
        root: &oxc_ast::ast::IdentifierReference<'_>,
        scoping: &Scoping,
    ) -> Option<&'a SystemApiSymbol> {
        let symbol_id = root
            .reference_id
            .get()
            .and_then(|reference_id| scoping.get_reference(reference_id).symbol_id())?;
        self.symbols.get(&symbol_id)
    }

    fn resolve_value(
        &self,
        expression: &Expression<'_>,
        scoping: &Scoping,
    ) -> Option<SystemApiSymbol> {
        let path = SystemApiValuePath::new(expression)?;
        let mut symbol = self.root_symbol(path.root, scoping)?.clone();
        for segment in path.segments {
            symbol = match segment {
                SystemApiValueSegment::Member(name) => symbol.resolved_member(name.as_ref())?,
                SystemApiValueSegment::Call(argument_count) => {
                    if symbol.call_signatures.is_empty() {
                        symbol
                    } else {
                        symbol.call_result(argument_count)?
                    }
                }
                SystemApiValueSegment::Await => symbol.awaited_result()?,
            };
        }
        Some(symbol)
    }

    fn resolve_api_use(
        &self,
        expression: &Expression<'_>,
        scoping: &Scoping,
    ) -> Option<SystemApiSymbol> {
        match expression {
            Expression::CallExpression(call) => self.resolve_value(&call.callee, scoping),
            Expression::NewExpression(constructor) => {
                self.resolve_value(&constructor.callee, scoping)
            }
            _ => self.resolve_value(expression, scoping),
        }
    }

    fn collect_local_value_bindings(&mut self, program: &Program<'_>, scoping: &Scoping) {
        let mut rounds = 0usize;
        loop {
            let facts = SystemApiLocalValueCollector::new(self, scoping).collect(program);
            let mut changed = false;
            for fact in facts {
                changed |= self.symbols.insert(fact.symbol_id, fact.symbol) != Some(fact.previous);
            }
            rounds += 1;
            if !changed || rounds > self.symbols.len().saturating_add(1) {
                break;
            }
        }
    }
}

struct SystemApiLocalValueFact {
    symbol_id: SymbolId,
    symbol: SystemApiSymbol,
    previous: SystemApiSymbol,
}

struct SystemApiLocalValueCollector<'a> {
    bindings: &'a ImportedSystemApiBindings,
    scoping: &'a Scoping,
    facts: Vec<SystemApiLocalValueFact>,
}

impl<'a> SystemApiLocalValueCollector<'a> {
    fn new(bindings: &'a ImportedSystemApiBindings, scoping: &'a Scoping) -> Self {
        Self {
            bindings,
            scoping,
            facts: Vec::new(),
        }
    }

    fn collect(mut self, program: &Program<'_>) -> Vec<SystemApiLocalValueFact> {
        self.visit_program(program);
        self.facts
    }
}

impl<'program> Visit<'program> for SystemApiLocalValueCollector<'_> {
    fn visit_variable_declarator(
        &mut self,
        declaration: &oxc_ast::ast::VariableDeclarator<'program>,
    ) {
        if let (Some(identifier), Some(initializer)) = (
            declaration.id.get_binding_identifier(),
            declaration.init.as_ref(),
        ) {
            if let Some(symbol) = self.bindings.resolve_value(initializer, self.scoping) {
                let symbol_id = identifier.symbol_id();
                self.facts.push(SystemApiLocalValueFact {
                    previous: self
                        .bindings
                        .symbols
                        .get(&symbol_id)
                        .cloned()
                        .unwrap_or_default(),
                    symbol_id,
                    symbol,
                });
            }
        }
        walk::walk_variable_declarator(self, declaration);
    }
}

/// Single semantic entry for dynamic ArkTS 1.1 system-API checks.
pub(crate) struct Arkts11SystemApiValidator<'a> {
    source_id: &'a str,
    source: &'a str,
    api_available: ApiAvailableTransformOptions,
    options: &'a SystemApiCheckOptions,
    module_exports: &'a ModuleExportIndex,
    sources: &'a TransformSourceSnapshot,
    module_roots: &'a [(String, PathBuf)],
}

impl<'a> Arkts11SystemApiValidator<'a> {
    pub(crate) fn new(
        source_id: &'a str,
        source: &'a str,
        api_available: ApiAvailableTransformOptions,
        options: &'a SystemApiCheckOptions,
        module_exports: &'a ModuleExportIndex,
        sources: &'a TransformSourceSnapshot,
        module_roots: &'a [(String, PathBuf)],
    ) -> Self {
        Self {
            source_id,
            source,
            api_available,
            options,
            module_exports,
            sources,
            module_roots,
        }
    }

    pub(crate) fn is_required(&self) -> bool {
        self.api_available.should_validate_source(self.source)
            || (self.module_exports.has_system_api_exports() && self.source.contains("import"))
    }

    pub(crate) fn diagnostics<'program>(
        &self,
        program: &'program Program<'program>,
    ) -> Vec<EtsTransformError> {
        if !self.is_required() {
            return Vec::new();
        }
        let semantic = SemanticBuilder::new().build(program).semantic;
        let scoping = semantic.scoping();
        let mut diagnostics = Vec::new();
        if self.api_available.should_validate_annotation(self.source) {
            diagnostics.extend(
                AvailableAnnotationValidator::new(self.source_id, self.api_available)
                    .diagnostics_with_scoping(program, scoping),
            );
        }
        if self.api_available.should_transform_source(self.source) {
            diagnostics.extend(
                ApiAvailableValidator::new(self.source_id, self.api_available)
                    .diagnostics_with_scoping(program, scoping),
            );
        }

        let bindings = ImportedSystemApiBindings::from_program(
            program,
            Path::new(self.source_id),
            self.sources,
            self.module_roots,
            self.module_exports,
            scoping,
        );
        if !bindings.is_empty() {
            let mut validator = SystemApiUseValidator::new(SystemApiUseValidatorInput {
                source_id: self.source_id,
                source: self.source,
                program,
                scoping,
                bindings: &bindings,
                device_info_bindings: ApiAvailableSdkBindings::from_program(program),
                constants: SystemApiConstantInventory::from_program(program),
                options: self.options,
            });
            validator.visit_program(program);
            diagnostics.extend(validator.finish());
        }
        diagnostics
    }
}

struct SystemApiUseValidator<'a, 'program> {
    source_id: &'a str,
    source: &'a str,
    program: &'program Program<'program>,
    scoping: &'a Scoping,
    bindings: &'a ImportedSystemApiBindings,
    device_info_bindings: ApiAvailableSdkBindings,
    constants: SystemApiConstantInventory,
    options: &'a SystemApiCheckOptions,
    ancestors: Vec<AstKind<'program>>,
    diagnostics: Vec<EtsTransformError>,
}

struct SystemApiUseValidatorInput<'a, 'program> {
    source_id: &'a str,
    source: &'a str,
    program: &'program Program<'program>,
    scoping: &'a Scoping,
    bindings: &'a ImportedSystemApiBindings,
    device_info_bindings: ApiAvailableSdkBindings,
    constants: SystemApiConstantInventory,
    options: &'a SystemApiCheckOptions,
}

impl<'a, 'program> SystemApiUseValidator<'a, 'program> {
    fn new(input: SystemApiUseValidatorInput<'a, 'program>) -> Self {
        Self {
            source_id: input.source_id,
            source: input.source,
            program: input.program,
            scoping: input.scoping,
            bindings: input.bindings,
            device_info_bindings: input.device_info_bindings,
            constants: input.constants,
            options: input.options,
            ancestors: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn validate_expression(&mut self, expression: &Expression<'program>) -> bool {
        let Some((root, path, span)) = SystemApiUsePath::resolve(expression) else {
            return false;
        };
        let Some(symbol) = self.bindings.resolve_api_use(expression, self.scoping) else {
            return false;
        };
        if symbol.contract.is_empty() {
            return false;
        }
        let api_name = path.last().map_or(root.name.as_str(), |name| name.as_ref());
        self.validate_contract(&symbol.contract, api_name, span);
        true
    }

    fn validate_contract(&mut self, contract: &SystemApiContract, api_name: &str, span: Span) {
        let suppression = SystemApiWarningSuppression::new(SystemApiWarningSuppressionInput {
            source: self.source,
            program: self.program,
            ancestors: &self.ancestors,
            scoping: self.scoping,
            device_info_bindings: &self.device_info_bindings,
            constants: &self.constants,
            api_name,
            span,
        });
        if let Some(since) = contract.since.as_deref() {
            if !contract.since_whitelisted
                && SystemApiVersion::new(&self.options.compatible_sdk_version)
                    .is_less_than(&SystemApiVersion::new(since))
                && !suppression.suppresses_since(since)
            {
                self.diagnostics.push(EtsTransformError::CodedDiagnostic {
                    file: self.source_id.to_string(),
                    code: "11706011".to_string(),
                    message: format!(
                        "The '{api_name}' API is supported since SDK version {since}. However, the current compatible SDK version is {}. It is recommended to use apiAvailable to safeguard API compatibility.",
                        self.options.compatible_sdk_version
                    ),
                    severity: self.options.severity(),
                    span: Some(span),
                    solutions: vec![
                        "Check the official API reference documentation, and switch to the supported interfaces."
                            .to_string(),
                    ],
                });
            }
        }

        if let Some(available) = contract.available.as_deref() {
            let available = SystemApiAvailableVersion::new(available);
            if available.matches_runtime(&self.options.runtime_os)
                && SystemApiVersion::new(&self.options.compatible_sdk_version)
                    .is_less_than(&SystemApiVersion::new(available.version))
                && !suppression.suppresses_available(available.version)
            {
                self.diagnostics.push(EtsTransformError::CodedDiagnostic {
                    file: self.source_id.to_string(),
                    code: "11706012".to_string(),
                    message: format!(
                        "The '{api_name}' API is available since SDK version {}. However, the current compatible SDK version is {}. It is recommended to use apiAvailable to safeguard API compatibility.",
                        available.version, self.options.compatible_sdk_version
                    ),
                    severity: self.options.severity(),
                    span: Some(span),
                    solutions: vec![
                        "Check the official API reference documentation, and switch to the supported interfaces."
                            .to_string(),
                    ],
                });
            }
        }

        let missing_permissions = contract
            .permissions
            .iter()
            .filter_map(|permission| {
                SystemApiVersionedComment::new(permission)
                    .active_content(&self.options.compile_sdk_version)
            })
            .filter(|permission| {
                !SystemApiPermissionExpression::new(permission.as_ref())
                    .is_satisfied_by(&self.options.request_permissions)
            })
            .collect::<Vec<_>>();
        if !missing_permissions.is_empty() && !suppression.suppresses("permission") {
            self.diagnostics.push(EtsTransformError::Diagnostic {
                file: self.source_id.to_string(),
                message: format!(
                    "To use this API, you need to apply for the permissions: {}",
                    missing_permissions
                        .iter()
                        .map(Cow::as_ref)
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
                severity: DiagnosticSeverity::Warning,
            });
        }

        if self.options.check_syscap {
            if let Some(syscap) = contract.syscap.as_deref() {
                if !self.options.syscap_intersection.contains(syscap)
                    && !suppression.suppresses_syscap(syscap)
                {
                    let message = if self.options.syscap_union.contains(syscap) {
                        "The API is not supported on all devices. Use the canIUse condition to determine whether the API is supported."
                            .to_string()
                    } else {
                        format!(
                            "The system capacity of this api '{api_name}' is not supported on all devices"
                        )
                    };
                    self.diagnostics.push(EtsTransformError::Diagnostic {
                        file: self.source_id.to_string(),
                        message,
                        severity: DiagnosticSeverity::Warning,
                    });
                }
            }
        }

        if contract.sdk_declaration {
            if self.options.form_source
                && !SystemApiVersionedTag::new(contract.form.as_deref())
                    .supports(&self.options.compile_sdk_version)
            {
                self.push_profile_error(
                    "11706006",
                    format!("'{api_name}' can't support form application."),
                    span,
                    "Check the official API reference documentation, and switch to the supported interfaces.",
                );
            }
            if self.options.crossplatform
                && !SystemApiVersionedTag::new(contract.crossplatform.as_deref())
                    .supports(&self.options.compile_sdk_version)
            {
                self.diagnostics.push(EtsTransformError::CodedDiagnostic {
                    file: self.source_id.to_string(),
                    code: "11706007".to_string(),
                    message: format!("'{api_name}' can't support crossplatform application."),
                    severity: if self.options.ignore_crossplatform_check {
                        DiagnosticSeverity::Warning
                    } else {
                        DiagnosticSeverity::Error
                    },
                    span: Some(span),
                    solutions: vec![
                        "Check the official API reference documentation, and switch to the supported interfaces."
                            .to_string(),
                    ],
                });
            }
            if self.options.atomic_service
                && SystemApiVersion::new(&self.options.compile_sdk_version)
                    .at_least(&SystemApiVersion::new("11"))
                && !SystemApiVersionedTag::new(contract.atomic_service.as_deref())
                    .supports(&self.options.compile_sdk_version)
            {
                self.push_profile_error(
                    "11706010",
                    format!("'{api_name}' can't support atomicservice application."),
                    span,
                    "Check the official API reference documentation, and switch to the supported interfaces.",
                );
            }
            if SystemApiVersionedTag::new(contract.fa_model_only.as_deref())
                .is_active(&self.options.compile_sdk_version)
            {
                self.push_profile_error(
                    "11706008",
                    "This API is used only in FA Mode, but the current Mode is Stage.".to_string(),
                    span,
                    "Check the official API reference documentation, and switch to the supported Stage model interfaces.",
                );
            }
            if !self.options.ohos_test_source
                && SystemApiVersionedTag::new(contract.test_only.as_deref())
                    .is_active(&self.options.compile_sdk_version)
            {
                self.diagnostics.push(EtsTransformError::Diagnostic {
                    file: self.source_id.to_string(),
                    message: format!("'{api_name}' can only be used for testing directories "),
                    severity: DiagnosticSeverity::Warning,
                });
            }
        }

        if SystemApiVersionedTag::new(contract.system_api.as_deref())
            .is_active(&self.options.compile_sdk_version)
        {
            self.diagnostics.push(EtsTransformError::Diagnostic {
                file: self.source_id.to_string(),
                message: format!("'{api_name}' is system api"),
                severity: DiagnosticSeverity::Warning,
            });
        }
        if contract.deprecated {
            self.diagnostics.push(EtsTransformError::Diagnostic {
                file: self.source_id.to_string(),
                message: format!("'{api_name}' has been deprecated."),
                severity: DiagnosticSeverity::Warning,
            });
        }
    }

    fn finish(self) -> Vec<EtsTransformError> {
        self.diagnostics
    }

    fn push_profile_error(&mut self, code: &str, message: String, span: Span, solution: &str) {
        self.diagnostics.push(EtsTransformError::CodedDiagnostic {
            file: self.source_id.to_string(),
            code: code.to_string(),
            message,
            severity: DiagnosticSeverity::Error,
            span: Some(span),
            solutions: vec![solution.to_string()],
        });
    }
}

impl<'program> Visit<'program> for SystemApiUseValidator<'_, 'program> {
    fn enter_node(&mut self, kind: AstKind<'program>) {
        self.ancestors.push(kind);
    }

    fn leave_node(&mut self, _kind: AstKind<'program>) {
        self.ancestors.pop();
    }

    fn visit_expression(&mut self, expression: &Expression<'program>) {
        if self.validate_expression(expression) {
            match expression {
                Expression::CallExpression(call) => self.visit_arguments(&call.arguments),
                Expression::NewExpression(constructor) => {
                    self.visit_arguments(&constructor.arguments)
                }
                _ => {}
            }
        } else {
            walk::walk_expression(self, expression);
        }
    }
}

struct SystemApiValuePath<'a> {
    root: &'a oxc_ast::ast::IdentifierReference<'a>,
    segments: Vec<SystemApiValueSegment>,
}

enum SystemApiValueSegment {
    Member(Box<str>),
    Call(usize),
    Await,
}

impl<'a> SystemApiValuePath<'a> {
    fn new(expression: &'a Expression<'a>) -> Option<Self> {
        match expression {
            Expression::Identifier(root) => Some(Self {
                root,
                segments: Vec::new(),
            }),
            Expression::StaticMemberExpression(member) => {
                let mut path = Self::new(&member.object)?;
                path.segments.push(SystemApiValueSegment::Member(
                    member.property.name.as_str().into(),
                ));
                Some(path)
            }
            Expression::ComputedMemberExpression(member) => {
                let Expression::StringLiteral(property) = &member.expression else {
                    return None;
                };
                let mut path = Self::new(&member.object)?;
                path.segments.push(SystemApiValueSegment::Member(
                    property.value.as_str().into(),
                ));
                Some(path)
            }
            Expression::CallExpression(call) => {
                let mut path = Self::new(&call.callee)?;
                path.segments
                    .push(SystemApiValueSegment::Call(call.arguments.len()));
                Some(path)
            }
            Expression::NewExpression(constructor) => {
                let mut path = Self::new(&constructor.callee)?;
                path.segments
                    .push(SystemApiValueSegment::Call(constructor.arguments.len()));
                Some(path)
            }
            Expression::AwaitExpression(await_expression) => {
                let mut path = Self::new(&await_expression.argument)?;
                path.segments.push(SystemApiValueSegment::Await);
                Some(path)
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::new(&parenthesized.expression)
            }
            Expression::TSAsExpression(as_expression) => Self::new(&as_expression.expression),
            Expression::TSSatisfiesExpression(satisfies) => Self::new(&satisfies.expression),
            Expression::TSNonNullExpression(non_null) => Self::new(&non_null.expression),
            Expression::TSTypeAssertion(assertion) => Self::new(&assertion.expression),
            Expression::TSInstantiationExpression(instantiation) => {
                Self::new(&instantiation.expression)
            }
            _ => None,
        }
    }
}

struct SystemApiUsePath;

impl SystemApiUsePath {
    fn resolve<'a>(
        expression: &'a Expression<'a>,
    ) -> Option<(
        &'a oxc_ast::ast::IdentifierReference<'a>,
        Vec<Box<str>>,
        Span,
    )> {
        match expression {
            Expression::Identifier(identifier) => Some((identifier, Vec::new(), identifier.span)),
            Expression::StaticMemberExpression(member) => {
                let (root, mut path, _) = Self::resolve(&member.object)?;
                path.push(member.property.name.as_str().into());
                Some((root, path, member.property.span))
            }
            Expression::ComputedMemberExpression(member) => {
                let Expression::StringLiteral(property) = &member.expression else {
                    return None;
                };
                let (root, mut path, _) = Self::resolve(&member.object)?;
                path.push(property.value.as_str().into());
                Some((root, path, property.span))
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::resolve(&parenthesized.expression)
            }
            Expression::TSAsExpression(as_expression) => Self::resolve(&as_expression.expression),
            Expression::TSSatisfiesExpression(satisfies) => Self::resolve(&satisfies.expression),
            Expression::TSNonNullExpression(non_null) => Self::resolve(&non_null.expression),
            Expression::TSTypeAssertion(assertion) => Self::resolve(&assertion.expression),
            Expression::AwaitExpression(await_expression) => {
                Self::resolve(&await_expression.argument)
            }
            Expression::TSInstantiationExpression(instantiation) => {
                Self::resolve(&instantiation.expression)
            }
            Expression::CallExpression(call) => Self::resolve(&call.callee),
            Expression::NewExpression(constructor) => Self::resolve(&constructor.callee),
            _ => None,
        }
    }
}

struct SystemApiWarningSuppression<'a, 'program> {
    source: &'a str,
    program: &'program Program<'program>,
    ancestors: &'a [AstKind<'program>],
    scoping: &'a Scoping,
    device_info_bindings: &'a ApiAvailableSdkBindings,
    constants: &'a SystemApiConstantInventory,
    api_name: &'a str,
    span: Span,
}

struct SystemApiWarningSuppressionInput<'a, 'program> {
    source: &'a str,
    program: &'program Program<'program>,
    ancestors: &'a [AstKind<'program>],
    scoping: &'a Scoping,
    device_info_bindings: &'a ApiAvailableSdkBindings,
    constants: &'a SystemApiConstantInventory,
    api_name: &'a str,
    span: Span,
}

impl<'a, 'program> SystemApiWarningSuppression<'a, 'program> {
    fn new(input: SystemApiWarningSuppressionInput<'a, 'program>) -> Self {
        Self {
            source: input.source,
            program: input.program,
            ancestors: input.ancestors,
            scoping: input.scoping,
            device_info_bindings: input.device_info_bindings,
            constants: input.constants,
            api_name: input.api_name,
            span: input.span,
        }
    }

    fn suppresses_since(&self, since: &str) -> bool {
        self.suppresses("compatibility")
            || self
                .ancestors
                .iter()
                .any(|ancestor| matches!(ancestor, AstKind::TryStatement(_)))
            || self.has_undefined_guard()
            || self.has_available_guard(since)
            || self.has_sdk_guard(since)
    }

    fn suppresses_available(&self, since: &str) -> bool {
        self.suppresses("compatibility")
            || self.has_available_guard(since)
            || self.has_sdk_guard(since)
    }

    fn suppresses_syscap(&self, syscap: &str) -> bool {
        self.suppresses("syscap")
            || self.ancestors.iter().any(|ancestor| {
                let AstKind::IfStatement(statement) = ancestor else {
                    return false;
                };
                SystemApiCanIUseGuard::new(&statement.test, self.scoping, self.constants)
                    .matches(syscap)
            })
    }

    fn suppresses(&self, category: &str) -> bool {
        self.has_suppress_warnings_decorator(category)
            || self.has_suppress_warnings_comment(category)
    }

    fn has_suppress_warnings_decorator(&self, category: &str) -> bool {
        self.ancestors.iter().rev().any(|ancestor| {
            SystemApiAncestorDecorators::new(*ancestor)
                .iter()
                .any(|decorator| {
                    SystemApiSuppressWarningsDecorator::new(decorator).matches(category)
                })
        })
    }

    fn has_suppress_warnings_comment(&self, category: &str) -> bool {
        self.program.comments.iter().rev().any(|comment| {
            if !comment.is_line()
                || comment.span.end > self.span.start
                || comment.attached_to > self.span.start
            {
                return false;
            }
            let between = self
                .source
                .get(comment.span.end as usize..self.span.start as usize)
                .unwrap_or_default();
            if between.lines().any(|line| !line.trim().is_empty()) {
                return false;
            }
            let content_span = comment.content_span();
            let content = self
                .source
                .get(content_span.start as usize..content_span.end as usize)
                .unwrap_or_default();
            let lower = content.to_ascii_lowercase();
            lower.contains("@suppresswarnings")
                && lower
                    .split(|character: char| character.is_whitespace() || character == ',')
                    .any(|part| part == category)
        })
    }

    fn has_undefined_guard(&self) -> bool {
        self.ancestors.iter().any(|ancestor| {
            let AstKind::IfStatement(statement) = ancestor else {
                return false;
            };
            let Expression::BinaryExpression(binary) = &statement.test else {
                return false;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::StrictInequality | BinaryOperator::Inequality
            ) {
                return false;
            }
            (SystemApiGuardOperand::new(&binary.left).is_api(self.api_name)
                && SystemApiGuardOperand::new(&binary.right).is_undefined())
                || (SystemApiGuardOperand::new(&binary.right).is_api(self.api_name)
                    && SystemApiGuardOperand::new(&binary.left).is_undefined())
        })
    }

    fn has_available_guard(&self, since: &str) -> bool {
        self.ancestors.iter().any(|ancestor| {
            SystemApiAncestorDecorators::new(*ancestor)
                .iter()
                .any(|decorator| {
                    SystemApiAvailableDecorator::new(decorator)
                        .version()
                        .is_some_and(|version| {
                            !SystemApiVersion::new(version)
                                .is_less_than(&SystemApiVersion::new(since))
                        })
                })
        })
    }

    fn has_sdk_guard(&self, since: &str) -> bool {
        self.ancestors.iter().any(|ancestor| {
            let AstKind::IfStatement(statement) = ancestor else {
                return false;
            };
            if !(self.span.start >= statement.consequent.span().start
                && self.span.end <= statement.consequent.span().end)
            {
                return false;
            }
            SystemApiSdkVersionGuard::new(
                &statement.test,
                self.scoping,
                self.device_info_bindings,
                self.constants,
            )
            .supports(since)
        })
    }
}

struct SystemApiAncestorDecorators<'a> {
    decorators: &'a [Decorator<'a>],
}

impl<'a> SystemApiAncestorDecorators<'a> {
    fn new(kind: AstKind<'a>) -> Self {
        let decorators = match kind {
            AstKind::Function(function) => function.decorators.as_slice(),
            AstKind::Class(class) => class.decorators.as_slice(),
            AstKind::MethodDefinition(method) => method.decorators.as_slice(),
            AstKind::PropertyDefinition(property) => property.decorators.as_slice(),
            AstKind::VariableDeclaration(variable) => variable
                .decorators
                .as_deref()
                .map_or(&[] as &[Decorator<'a>], |decorators| decorators.as_slice()),
            AstKind::TSInterfaceDeclaration(interface) => interface
                .decorators
                .as_deref()
                .map_or(&[] as &[Decorator<'a>], |decorators| decorators.as_slice()),
            _ => &[],
        };
        Self { decorators }
    }

    fn iter(&self) -> impl Iterator<Item = &'a Decorator<'a>> {
        self.decorators.iter()
    }
}

struct SystemApiSuppressWarningsDecorator<'a> {
    decorator: &'a Decorator<'a>,
}

impl<'a> SystemApiSuppressWarningsDecorator<'a> {
    fn new(decorator: &'a Decorator<'a>) -> Self {
        Self { decorator }
    }

    fn matches(&self, category: &str) -> bool {
        let Expression::CallExpression(call) = &self.decorator.expression else {
            return false;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        if callee.name != "SuppressWarnings" {
            return false;
        }
        let Some(Expression::ObjectExpression(object)) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
        else {
            return false;
        };
        let Some(property) = object
            .properties
            .first()
            .and_then(|property| property.as_property())
        else {
            return false;
        };
        let Expression::ArrayExpression(array) = &property.value else {
            return false;
        };
        let expected = match category {
            "compatibility" => "COMPATIBILITY",
            "syscap" => "SYSCAP",
            "permission" => "PERMISSION",
            _ => return false,
        };
        array.elements.iter().any(|element| {
            element.as_expression().is_some_and(|expression| {
                SystemApiExpressionText::new(expression).ends_with(expected)
            })
        })
    }
}

struct SystemApiAvailableDecorator<'a> {
    decorator: &'a Decorator<'a>,
}

impl<'a> SystemApiAvailableDecorator<'a> {
    fn new(decorator: &'a Decorator<'a>) -> Self {
        Self { decorator }
    }

    fn version(&self) -> Option<&'a str> {
        let Expression::CallExpression(call) = &self.decorator.expression else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        if callee.name != "Available" {
            return None;
        }
        let Expression::ObjectExpression(object) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())?
        else {
            return None;
        };
        for property in &object.properties {
            let property = property.as_property()?;
            if property.key.static_name().as_deref() != Some("minApiVersion") {
                continue;
            }
            if let Expression::StringLiteral(value) = &property.value {
                return Some(value.value.as_str());
            }
        }
        None
    }
}

struct SystemApiGuardOperand<'a> {
    expression: &'a Expression<'a>,
}

impl<'a> SystemApiGuardOperand<'a> {
    fn new(expression: &'a Expression<'a>) -> Self {
        Self { expression }
    }

    fn is_api(&self, api_name: &str) -> bool {
        match self.expression {
            Expression::Identifier(identifier) => identifier.name == api_name,
            Expression::StaticMemberExpression(member) => member.property.name == api_name,
            _ => false,
        }
    }

    fn is_undefined(&self) -> bool {
        matches!(self.expression, Expression::Identifier(identifier) if identifier.name == "undefined")
    }
}

struct SystemApiSdkVersionGuard<'a> {
    expression: &'a Expression<'a>,
    scoping: &'a Scoping,
    device_info_bindings: &'a ApiAvailableSdkBindings,
    constants: &'a SystemApiConstantInventory,
}

impl<'a> SystemApiSdkVersionGuard<'a> {
    fn new(
        expression: &'a Expression<'a>,
        scoping: &'a Scoping,
        device_info_bindings: &'a ApiAvailableSdkBindings,
        constants: &'a SystemApiConstantInventory,
    ) -> Self {
        Self {
            expression,
            scoping,
            device_info_bindings,
            constants,
        }
    }

    fn supports(&self, since: &str) -> bool {
        if let Expression::CallExpression(call) = self.expression {
            if let Expression::StaticMemberExpression(member) = &call.callee {
                if member.property.name == "apiAvailable"
                    && call.arguments.len() == 1
                    && SystemApiUsePath::resolve(&member.object).is_some_and(|(root, path, _)| {
                        path.is_empty() && self.device_info_bindings.contains(root, self.scoping)
                    })
                {
                    return call
                        .arguments
                        .first()
                        .and_then(|argument| argument.as_expression())
                        .and_then(|expression| {
                            SystemApiLiteralValue::parse(expression, self.scoping, self.constants)
                        })
                        .is_some_and(|version| {
                            !SystemApiVersion::new(&version)
                                .is_less_than(&SystemApiVersion::new(since))
                        });
                }
            }
        }
        let Expression::BinaryExpression(binary) = self.expression else {
            return false;
        };
        let (api_side, value_side, flipped) =
            if SystemApiExpressionText::new(&binary.left).ends_with("sdkApiVersion") {
                (&binary.left, &binary.right, false)
            } else if SystemApiExpressionText::new(&binary.right).ends_with("sdkApiVersion") {
                (&binary.right, &binary.left, true)
            } else {
                return false;
            };
        if !SystemApiUsePath::resolve(api_side).is_some_and(|(root, path, _)| {
            path.last()
                .is_some_and(|name| name.as_ref() == "sdkApiVersion")
                && self.device_info_bindings.contains(root, self.scoping)
        }) {
            return false;
        }
        let Some(value) = SystemApiLiteralValue::parse(value_side, self.scoping, self.constants)
        else {
            return false;
        };
        let Some(number) = value.parse::<f64>().ok().map(f64::ceil) else {
            return false;
        };
        let effective = match (binary.operator, flipped) {
            (BinaryOperator::GreaterThan, false) | (BinaryOperator::LessThan, true) => number + 1.0,
            (BinaryOperator::GreaterEqualThan, false)
            | (BinaryOperator::LessEqualThan, true)
            | (BinaryOperator::Equality, _)
            | (BinaryOperator::StrictEquality, _) => number,
            _ => return false,
        };
        !SystemApiVersion::new(&effective.to_string()).is_less_than(&SystemApiVersion::new(since))
    }
}

struct SystemApiCanIUseGuard<'a> {
    expression: &'a Expression<'a>,
    scoping: &'a Scoping,
    constants: &'a SystemApiConstantInventory,
}

impl<'a> SystemApiCanIUseGuard<'a> {
    fn new(
        expression: &'a Expression<'a>,
        scoping: &'a Scoping,
        constants: &'a SystemApiConstantInventory,
    ) -> Self {
        Self {
            expression,
            scoping,
            constants,
        }
    }

    fn matches(&self, syscap: &str) -> bool {
        let Expression::CallExpression(call) = self.expression else {
            return false;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        callee.name == "canIUse"
            && call.arguments.len() == 1
            && call
                .arguments
                .first()
                .and_then(|argument| argument.as_expression())
                .and_then(|expression| {
                    SystemApiLiteralValue::parse(expression, self.scoping, self.constants)
                })
                .is_some_and(|value| value == syscap)
    }
}

struct SystemApiLiteralValue;

#[derive(Default)]
struct SystemApiConstantInventory {
    values: FxHashMap<SymbolId, String>,
}

impl SystemApiConstantInventory {
    fn from_program(program: &Program<'_>) -> Self {
        let mut inventory = Self::default();
        inventory.visit_program(program);
        inventory
    }

    fn value<'a>(
        &'a self,
        identifier: &oxc_ast::ast::IdentifierReference<'_>,
        scoping: &Scoping,
    ) -> Option<&'a String> {
        let symbol_id = identifier
            .reference_id
            .get()
            .and_then(|reference_id| scoping.get_reference(reference_id).symbol_id())?;
        self.values.get(&symbol_id)
    }

    fn literal(expression: &Expression<'_>) -> Option<String> {
        match expression {
            Expression::StringLiteral(value) => Some(value.value.to_string()),
            Expression::NumericLiteral(value) => Some(value.value.to_string()),
            _ => None,
        }
    }
}

impl<'program> Visit<'program> for SystemApiConstantInventory {
    fn visit_variable_declarator(
        &mut self,
        declaration: &oxc_ast::ast::VariableDeclarator<'program>,
    ) {
        if let (Some(identifier), Some(value)) = (
            declaration.id.get_binding_identifier(),
            declaration.init.as_ref().and_then(Self::literal),
        ) {
            self.values.insert(identifier.symbol_id(), value);
        }
        walk::walk_variable_declarator(self, declaration);
    }
}

impl SystemApiLiteralValue {
    fn parse(
        expression: &Expression<'_>,
        scoping: &Scoping,
        constants: &SystemApiConstantInventory,
    ) -> Option<String> {
        match expression {
            Expression::StringLiteral(value) => Some(value.value.to_string()),
            Expression::NumericLiteral(value) => Some(value.value.to_string()),
            Expression::Identifier(identifier) => constants.value(identifier, scoping).cloned(),
            _ => None,
        }
    }
}

struct SystemApiExpressionText<'a> {
    expression: &'a Expression<'a>,
}

impl<'a> SystemApiExpressionText<'a> {
    fn new(expression: &'a Expression<'a>) -> Self {
        Self { expression }
    }

    fn ends_with(&self, expected: &str) -> bool {
        match self.expression {
            Expression::Identifier(identifier) => identifier.name == expected,
            Expression::StaticMemberExpression(member) => member.property.name == expected,
            _ => false,
        }
    }
}

struct SystemApiPermissionExpression<'a> {
    expression: &'a str,
}

impl<'a> SystemApiPermissionExpression<'a> {
    fn new(expression: &'a str) -> Self {
        Self { expression }
    }

    fn is_satisfied_by(&self, permissions: &FxHashSet<Box<str>>) -> bool {
        SystemApiPermissionParser::new(self.expression, permissions).expression()
    }
}

struct SystemApiVersionedComment<'a> {
    comment: &'a str,
}

struct SystemApiVersionedTag<'a> {
    comment: Option<&'a str>,
}

impl<'a> SystemApiVersionedTag<'a> {
    fn new(comment: Option<&'a str>) -> Self {
        Self { comment }
    }

    fn supports(&self, compile_sdk_version: &str) -> bool {
        self.comment
            .is_some_and(|comment| Self::range_is_active(comment, compile_sdk_version))
    }

    fn is_active(&self, compile_sdk_version: &str) -> bool {
        self.supports(compile_sdk_version)
    }

    fn range_is_active(comment: &str, compile_sdk_version: &str) -> bool {
        let Some(range_start) = comment.find("[since ") else {
            return true;
        };
        let range_content_start = range_start + "[since ".len();
        let Some(relative_end) = comment[range_content_start..].find(']') else {
            return true;
        };
        let range_end = range_content_start + relative_end;
        let Some((start, end)) = comment[range_content_start..range_end].split_once(" - ") else {
            return true;
        };
        SystemApiVersionRange::new(start, end).contains(compile_sdk_version)
    }
}

impl<'a> SystemApiVersionedComment<'a> {
    fn new(comment: &'a str) -> Self {
        Self { comment }
    }

    fn active_content(&self, compile_sdk_version: &str) -> Option<Cow<'a, str>> {
        let Some(range_start) = self.comment.find("[since ") else {
            return Some(Cow::Borrowed(self.comment.trim()));
        };
        let range_content_start = range_start + "[since ".len();
        let range_end = self.comment[range_content_start..].find(']')? + range_content_start;
        let (start, end) = self.comment[range_content_start..range_end].split_once(" - ")?;
        if !SystemApiVersionRange::new(start, end).contains(compile_sdk_version) {
            return None;
        }
        let before = self.comment[..range_start].trim();
        let after = self.comment[range_end + 1..].trim();
        if before.is_empty() {
            Some(Cow::Borrowed(after))
        } else if after.is_empty() {
            Some(Cow::Borrowed(before))
        } else {
            Some(Cow::Owned(format!("{before} {after}")))
        }
    }
}

struct SystemApiVersionRange<'a> {
    start: &'a str,
    end: &'a str,
}

impl<'a> SystemApiVersionRange<'a> {
    fn new(start: &'a str, end: &'a str) -> Self {
        Self { start, end }
    }

    fn contains(&self, version: &str) -> bool {
        let start = SystemApiVersion::new(self.start).parts();
        let end = SystemApiVersion::new(self.end).parts();
        let version = SystemApiVersion::new(version).parts();
        let (lower, upper) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        lower <= version && version <= upper
    }
}

struct SystemApiPermissionParser<'a> {
    tokens: Vec<String>,
    position: usize,
    permissions: &'a FxHashSet<Box<str>>,
}

impl<'a> SystemApiPermissionParser<'a> {
    fn new(expression: &str, permissions: &'a FxHashSet<Box<str>>) -> Self {
        let tokens = expression
            .replace('(', " ( ")
            .replace(')', " ) ")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        Self {
            tokens,
            position: 0,
            permissions,
        }
    }

    fn expression(mut self) -> bool {
        let value = self.sequence();
        value && self.position == self.tokens.len()
    }

    /// `api_check_permission.ts::getPermissionVaildAtoms` evaluates operators
    /// in source order rather than assigning conventional `and` precedence.
    fn sequence(&mut self) -> bool {
        let mut value = self.factor();
        while matches!(self.peek(), Some("and" | "or")) {
            let operator = self.peek().unwrap_or_default().to_string();
            self.position += 1;
            let right = self.factor();
            if operator == "and" {
                value &= right;
            } else {
                value |= right;
            }
        }
        value
    }

    fn factor(&mut self) -> bool {
        if self.peek() == Some("(") {
            self.position += 1;
            let value = self.sequence();
            if self.peek() == Some(")") {
                self.position += 1;
            }
            return value;
        }
        let Some(permission) = self.peek().map(str::to_string) else {
            return true;
        };
        self.position += 1;
        self.permissions.contains(permission.as_str())
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(String::as_str)
    }
}

struct SystemApiVersion<'a> {
    raw: &'a str,
}

struct SystemApiAvailableVersion<'a> {
    os: &'a str,
    version: &'a str,
}

impl<'a> SystemApiAvailableVersion<'a> {
    fn new(raw: &'a str) -> Self {
        let mut parts = raw.split_whitespace();
        let first = parts.next().unwrap_or_default();
        if let Some(second) = parts.next() {
            Self {
                os: first,
                version: second,
            }
        } else {
            Self {
                os: "OpenHarmony",
                version: first,
            }
        }
    }

    fn matches_runtime(&self, runtime_os: &str) -> bool {
        self.os == "OpenHarmony" || self.os.eq_ignore_ascii_case(runtime_os)
    }
}

impl<'a> SystemApiVersion<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_valid_since(raw: &str) -> bool {
        let raw = raw.trim();
        if Self::valid_integer(raw) {
            return true;
        }
        let raw = raw
            .split_once('(')
            .and_then(|(version, build)| build.strip_suffix(')').map(|_| version))
            .unwrap_or(raw);
        let segments = raw.split('.').collect::<Vec<_>>();
        segments.len() == 3
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment.len() <= 3
                    && segment.bytes().all(|byte| byte.is_ascii_digit())
            })
            && segments[0]
                .parse::<u16>()
                .is_ok_and(|major| major > 0 && major <= 999)
    }

    fn valid_integer(raw: &str) -> bool {
        !raw.starts_with('0')
            && raw.len() <= 3
            && raw
                .parse::<u16>()
                .is_ok_and(|value| (1..=999).contains(&value))
    }

    fn is_less_than(&self, other: &Self) -> bool {
        self.parts() < other.parts()
    }

    fn at_least(&self, other: &Self) -> bool {
        !self.is_less_than(other)
    }

    fn parts(&self) -> [u32; 3] {
        let mut parts = [0; 3];
        for (index, segment) in self.raw.trim().split('.').take(3).enumerate() {
            let digits = segment
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect::<String>();
            parts[index] = digits.parse().unwrap_or_default();
        }
        parts
    }
}
