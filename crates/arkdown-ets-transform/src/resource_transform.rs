//! Resource-call lowering aligned with SDK `ets-loader` `processResourceData`.
//!
//! The matcher operates on OXC AST call expressions, mirroring upstream's
//! TypeScript AST visitor: only `$r(...)` and `$rawfile(...)` call nodes are
//! rewritten.  The replacement object expression is emitted from a small
//! structured model so the field order and static names stay centralized
//! instead of being assembled ad hoc at call sites.

use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};

use oxc_allocator::{Allocator, ArenaVec, CloneIn, GetAllocator};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingIdentifier, CallExpression, Expression,
    FormalParameterKind, FormalParameters, FunctionBody, FunctionType, ImportDeclarationSpecifier,
    ImportOrExportKind, NumberBase, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    StringLiteral,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_span::{GetSpan, Span, SPAN};
use oxc_str::{Ident, Str};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::error::{DiagnosticSeverity, EtsTransformError};

pub(crate) const RAWFILE_RUNTIME_TYPE: i64 = 30000;

struct ResourceCallName;

impl ResourceCallName {
    const RESOURCE: &'static str = "$r";
    const RAWFILE: &'static str = "$rawfile";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReference {
    pub id: u32,
    pub runtime_type: u32,
    pub bundle_name: String,
    pub module_name: String,
}

/// Resource names and rawfiles exported by one HSP dependency.
///
/// The coordinator constructs this once from the HSP `ResourceTable.txt` and
/// adjacent `resources/rawfile` directory. Transform hooks only query it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceModuleReference {
    resources: FxHashMap<String, FxHashSet<String>>,
    rawfiles: FxHashSet<String>,
    resource_table_available: bool,
    rawfile_root_available: bool,
}

impl ResourceModuleReference {
    pub fn new(
        resources: FxHashSet<String>,
        rawfiles: FxHashSet<String>,
        resource_table_available: bool,
        rawfile_root_available: bool,
    ) -> Self {
        let mut resources_by_type: FxHashMap<String, FxHashSet<String>> = FxHashMap::default();
        for resource in resources {
            let Some((resource_type, name)) = resource.split_once('.') else {
                continue;
            };
            resources_by_type
                .entry(resource_type.to_string())
                .or_default()
                .insert(name.to_string());
        }
        Self {
            resources: resources_by_type,
            rawfiles,
            resource_table_available,
            rawfile_root_available,
        }
    }

    fn contains_resource(&self, resource_type: &str, name: &str) -> bool {
        self.resources
            .get(resource_type)
            .is_some_and(|resources| resources.contains(name))
    }

    fn contains_type(&self, resource_type: &str) -> bool {
        self.resources.contains_key(resource_type)
    }

    fn contains_rawfile(&self, name: &str) -> bool {
        self.rawfiles.contains(name)
    }

    fn has_rawfile_root(&self) -> bool {
        self.rawfile_root_available
    }

    fn has_resource_table(&self) -> bool {
        self.resource_table_available
    }
}

#[derive(Debug)]
struct ResourceObject<'a> {
    id: ResourceObjectId,
    runtime_type: i64,
    params: Vec<Argument<'a>>,
    bundle_name: ResourceObjectName,
    module_name: ResourceObjectName,
}

impl<'a> ResourceObject<'a> {
    fn app_static(
        reference: &ResourceReference,
        params: Vec<Argument<'a>>,
        context: &ResourceTransformContext,
        source: ResourceSource,
    ) -> Self {
        Self {
            id: context.static_resource_id(i64::from(reference.id), source),
            runtime_type: i64::from(reference.runtime_type),
            params,
            bundle_name: context.bundle_name(&context.bundle_name),
            module_name: context.module_name(&context.module_name),
        }
    }

    fn app_dynamic(
        params: Vec<Argument<'a>>,
        names: &ResourceNames,
        context: &ResourceTransformContext,
    ) -> Self {
        Self {
            id: context.dynamic_resource_id(),
            runtime_type: -1,
            params,
            bundle_name: context.bundle_name(&names.bundle_name),
            module_name: context.module_name(&names.module_name),
        }
    }

    fn resource_module(
        module_name: &str,
        params: Vec<Argument<'a>>,
        context: &ResourceTransformContext,
    ) -> Self {
        Self {
            id: context.resource_module_id(),
            runtime_type: -1,
            params,
            bundle_name: context.bundle_name(&context.bundle_name),
            module_name: ResourceObjectName::Literal(module_name.to_string()),
        }
    }

    fn rawfile(
        id: i64,
        params: Vec<Argument<'a>>,
        names: &ResourceNames,
        context: &ResourceTransformContext,
        resource_module: Option<&str>,
    ) -> Self {
        Self {
            id: ResourceObjectId::Literal(id),
            runtime_type: RAWFILE_RUNTIME_TYPE,
            params,
            bundle_name: context.bundle_name(&names.bundle_name),
            module_name: resource_module.map_or_else(
                || context.module_name(&names.module_name),
                |module| ResourceObjectName::Literal(module.to_string()),
            ),
        }
    }

    fn expression<'b>(&self, ast: &AstBuilder<'b>) -> Expression<'b> {
        ResourceObjectEmitter::new(self).expression(ast)
    }
}

#[derive(Debug, Clone, Copy)]
enum ResourceObjectId {
    Literal(i64),
    RuntimeGetter(&'static str),
}

#[derive(Debug, Clone)]
enum ResourceObjectName {
    Literal(String),
    RuntimeIdentifier(&'static str),
}

struct ResourceObjectEmitter<'a, 'b> {
    resource: &'a ResourceObject<'b>,
}

impl<'a, 'b> ResourceObjectEmitter<'a, 'b> {
    fn new(resource: &'a ResourceObject<'b>) -> Self {
        Self { resource }
    }

    fn expression<'c>(&self, ast: &AstBuilder<'c>) -> Expression<'c> {
        Expression::new_object_expression(
            SPAN,
            ArenaVec::from_iter_in(self.properties(ast), ast),
            ast,
        )
    }

    fn properties<'c>(&self, ast: &AstBuilder<'c>) -> [ObjectPropertyKind<'c>; 5] {
        [
            self.id_property(ast),
            self.number_property(
                ast,
                ResourceObjectPropertyName::TYPE,
                self.resource.runtime_type,
            ),
            self.params_property(ast),
            self.name_property(
                ast,
                ResourceObjectPropertyName::BUNDLE_NAME,
                &self.resource.bundle_name,
            ),
            self.name_property(
                ast,
                ResourceObjectPropertyName::MODULE_NAME,
                &self.resource.module_name,
            ),
        ]
    }

    fn id_property<'c>(&self, ast: &AstBuilder<'c>) -> ObjectPropertyKind<'c> {
        match self.resource.id {
            ResourceObjectId::Literal(value) => {
                self.number_property(ast, ResourceObjectPropertyName::ID, value)
            }
            ResourceObjectId::RuntimeGetter(function) => self.runtime_id_getter(ast, function),
        }
    }

    fn runtime_id_getter<'c>(
        &self,
        ast: &AstBuilder<'c>,
        function: &'static str,
    ) -> ObjectPropertyKind<'c> {
        let function_identifier = || Expression::new_identifier(SPAN, function, ast);
        let test = Expression::new_binary_expression(
            SPAN,
            Expression::new_unary_expression(
                SPAN,
                UnaryOperator::Typeof,
                function_identifier(),
                ast,
            ),
            BinaryOperator::StrictEquality,
            Expression::new_string_literal(SPAN, "function", None, ast),
            ast,
        );
        let call = Expression::new_call_expression(
            SPAN,
            function_identifier(),
            None,
            ArenaVec::from_value_in(
                Argument::from(Expression::new_this_expression(SPAN, ast)),
                ast,
            ),
            false,
            ast,
        );
        let raw = Str::from_str_in("1", ast);
        let alternate = Expression::new_unary_expression(
            SPAN,
            UnaryOperator::UnaryNegation,
            Expression::new_numeric_literal(SPAN, 1.0, Some(raw), NumberBase::Decimal, ast),
            ast,
        );
        let result = Expression::new_conditional_expression(SPAN, test, call, alternate, ast);
        let body = FunctionBody::boxed(
            SPAN,
            ArenaVec::new_in(ast),
            ArenaVec::from_value_in(
                Statement::new_return_statement(SPAN, Some(result), ast),
                ast,
            ),
            ast,
        );
        let params = FormalParameters::boxed(
            SPAN,
            FormalParameterKind::FormalParameter,
            ArenaVec::new_in(ast),
            None,
            ast,
        );
        let getter = Expression::new_function_expression(
            SPAN,
            FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            None,
            None,
            params,
            None,
            Some(body),
            ast,
        );
        self.property_with_kind(
            ast,
            ResourceObjectPropertyName::ID,
            getter,
            PropertyKind::Get,
        )
    }

    fn number_property<'c>(
        &self,
        ast: &AstBuilder<'c>,
        name: &'static str,
        value: i64,
    ) -> ObjectPropertyKind<'c> {
        let raw = Str::from_str_in(&value.to_string(), ast);
        self.property(
            ast,
            name,
            Expression::new_numeric_literal(
                SPAN,
                value as f64,
                Some(raw),
                NumberBase::Decimal,
                ast,
            ),
        )
    }

    fn name_property<'c>(
        &self,
        ast: &AstBuilder<'c>,
        name: &'static str,
        value: &ResourceObjectName,
    ) -> ObjectPropertyKind<'c> {
        let value = match value {
            ResourceObjectName::Literal(value) => {
                Expression::new_string_literal(SPAN, Str::from_str_in(value, ast), None, ast)
            }
            ResourceObjectName::RuntimeIdentifier(value) => {
                Expression::new_identifier(SPAN, *value, ast)
            }
        };
        self.property(ast, name, value)
    }

    fn params_property<'c>(&self, ast: &AstBuilder<'c>) -> ObjectPropertyKind<'c> {
        let elements = ArenaVec::from_iter_in(
            self.resource
                .params
                .iter()
                .map(|argument| ArrayExpressionElement::from(argument.clone_in(ast.allocator()))),
            ast,
        );
        self.property(
            ast,
            ResourceObjectPropertyName::PARAMS,
            Expression::new_array_expression(SPAN, elements, ast),
        )
    }

    fn property<'c>(
        &self,
        ast: &AstBuilder<'c>,
        name: &'static str,
        value: Expression<'c>,
    ) -> ObjectPropertyKind<'c> {
        self.property_with_kind(ast, name, value, PropertyKind::Init)
    }

    fn property_with_kind<'c>(
        &self,
        ast: &AstBuilder<'c>,
        name: &'static str,
        value: Expression<'c>,
        kind: PropertyKind,
    ) -> ObjectPropertyKind<'c> {
        let key = if kind != PropertyKind::Get
            && ResourceObjectPropertyName::uses_string_literal_key(name)
        {
            // Latest `process_ui_syntax.js::createResourceParam` constructs
            // id/type/bundle/module keys with `createStringLiteral`; `params`
            // alone is created with `createIdentifier`. Keeping this AST shape
            // matters because CompileArkTS feeds the emitted source into
            // es2abc, including generated closure-name hashing.
            PropertyKey::StringLiteral(StringLiteral::boxed(
                SPAN,
                Str::from_str_in(name, ast),
                None,
                ast,
            ))
        } else {
            PropertyKey::new_static_identifier(SPAN, name, ast)
        };
        ObjectPropertyKind::new_object_property(SPAN, kind, key, value, false, false, false, ast)
    }
}

struct ResourceObjectPropertyName;

impl ResourceObjectPropertyName {
    const ID: &'static str = "id";
    const TYPE: &'static str = "type";
    const PARAMS: &'static str = "params";
    const BUNDLE_NAME: &'static str = "bundleName";
    const MODULE_NAME: &'static str = "moduleName";

    fn uses_string_literal_key(name: &str) -> bool {
        matches!(
            name,
            Self::ID | Self::TYPE | Self::BUNDLE_NAME | Self::MODULE_NAME
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResourceNames {
    bundle_name: String,
    module_name: String,
}

/// Immutable source-compiler resource behavior selected before Rolldown
/// starts. This mirrors the fields read by `processResourceData` without
/// consulting project globals from a transform hook.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceTransformContext {
    pub min_api_version: u32,
    pub compile_har: bool,
    pub byte_code_har: bool,
    pub integrated_hsp: bool,
    pub use_get_current_bundle_name: bool,
    pub ui_transform_optimization: bool,
    pub reset_bundle_name: bool,
    pub allow_empty_bundle_name: bool,
    pub bundle_name: String,
    pub module_name: String,
    /// Whether the Stage build supplied `hspResourcesMap`, including an empty map.
    pub validate_resource_modules: bool,
    /// Whether the compiled current-module rawfile directory was inventoried.
    pub validate_rawfiles: bool,
    pub rawfiles: Arc<FxHashSet<String>>,
    pub resource_modules: Arc<FxHashMap<String, ResourceModuleReference>>,
}

impl ResourceTransformContext {
    const GET_RESOURCE_ID: &'static str = "__getResourceId__";
    const RESOURCE_ID_HAR: &'static str = "__resourceIdHar__";
    const BUNDLE_NAME: &'static str = "__BUNDLE_NAME__";
    const MODULE_NAME: &'static str = "__MODULE_NAME__";
    const HAR_DEFAULT_BUNDLE_NAME: &'static str = "__harDefaultBundleName__";
    const HAR_DEFAULT_MODULE_NAME: &'static str = "__harDefaultModuleName__";

    fn static_resource_id(&self, id: i64, source: ResourceSource) -> ResourceObjectId {
        if !self.compile_har {
            return ResourceObjectId::Literal(id);
        }
        if self.api_level() < 23 {
            return ResourceObjectId::Literal(-1);
        }
        if source == ResourceSource::Sys {
            ResourceObjectId::Literal(id)
        } else {
            ResourceObjectId::RuntimeGetter(Self::RESOURCE_ID_HAR)
        }
    }

    fn resource_module_id(&self) -> ResourceObjectId {
        if self.api_level() >= 22 {
            ResourceObjectId::RuntimeGetter(Self::GET_RESOURCE_ID)
        } else {
            ResourceObjectId::Literal(-1)
        }
    }

    fn dynamic_resource_id(&self) -> ResourceObjectId {
        self.resource_module_id()
    }

    fn bundle_name(&self, configured: &str) -> ResourceObjectName {
        if self.allow_empty_bundle_name
            || (self.reset_bundle_name && !self.use_get_current_bundle_name)
        {
            return ResourceObjectName::Literal(String::new());
        }
        if self.is_dynamic() {
            return ResourceObjectName::RuntimeIdentifier(Self::BUNDLE_NAME);
        }
        ResourceObjectName::Literal(if self.compile_har {
            Self::HAR_DEFAULT_BUNDLE_NAME.to_string()
        } else {
            configured.to_string()
        })
    }

    fn module_name(&self, configured: &str) -> ResourceObjectName {
        if self.is_dynamic() {
            return ResourceObjectName::RuntimeIdentifier(Self::MODULE_NAME);
        }
        ResourceObjectName::Literal(if self.compile_har {
            Self::HAR_DEFAULT_MODULE_NAME.to_string()
        } else {
            configured.to_string()
        })
    }

    fn is_dynamic(&self) -> bool {
        self.ui_transform_optimization
            || (self.compile_har && self.byte_code_har)
            || (self.integrated_hsp && self.use_get_current_bundle_name)
    }

    fn api_level(&self) -> u32 {
        self.min_api_version % 1000
    }
}

impl ResourceNames {
    fn new(bundle_name: &str, module_name: Option<&str>) -> Self {
        Self {
            bundle_name: bundle_name.to_string(),
            module_name: module_name.unwrap_or_default().to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ResourceReferenceIndex {
    types: FxHashMap<ResourceSource, FxHashSet<String>>,
}

impl ResourceReferenceIndex {
    pub(crate) fn new(resource_refs: &FxHashMap<String, ResourceReference>) -> Self {
        let mut types: FxHashMap<ResourceSource, FxHashSet<String>> = FxHashMap::default();
        for key in resource_refs.keys() {
            let mut components = key.split('.');
            let (Some(source), Some(resource_type)) = (components.next(), components.next()) else {
                continue;
            };
            let Some(source) = ResourceSource::parse(source) else {
                continue;
            };
            types
                .entry(source)
                .or_default()
                .insert(resource_type.to_string());
        }
        Self { types }
    }

    fn contains_type(&self, source: ResourceSource, resource_type: &str) -> bool {
        self.types
            .get(&source)
            .is_some_and(|types| types.contains(resource_type))
    }
}

pub(crate) struct ResourceReferenceRewriter<'a> {
    id: &'a str,
    resource_refs: &'a FxHashMap<String, ResourceReference>,
    reference_index: &'a ResourceReferenceIndex,
    names: ResourceNames,
    context: ResourceTransformContext,
}

impl<'a> ResourceReferenceRewriter<'a> {
    pub(crate) fn new(
        source: &'a str,
        id: &'a str,
        resource_refs: &'a FxHashMap<String, ResourceReference>,
        reference_index: &'a ResourceReferenceIndex,
        bundle_name: &str,
        module_name: Option<&str>,
    ) -> Self {
        let _ = source;
        Self {
            id,
            resource_refs,
            reference_index,
            names: ResourceNames::new(bundle_name, module_name),
            context: ResourceTransformContext {
                bundle_name: bundle_name.to_string(),
                module_name: module_name.unwrap_or_default().to_string(),
                ..ResourceTransformContext::default()
            },
        }
    }

    pub(crate) fn with_context(mut self, mut context: ResourceTransformContext) -> Self {
        if context.bundle_name.is_empty() {
            context.bundle_name.clone_from(&self.names.bundle_name);
        }
        if context.module_name.is_empty() {
            context.module_name.clone_from(&self.names.module_name);
        }
        self.context = context;
        self
    }

    pub(crate) fn rewrite_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
    ) -> Result<ResourceRewriteResult, EtsTransformError> {
        let mut visitor = ResourceReferenceMutator {
            allocator,
            id: self.id,
            resource_refs: self.resource_refs,
            reference_index: self.reference_index,
            names: self.names.clone(),
            context: self.context.clone(),
            changed: false,
            diagnostics: Vec::new(),
        };
        visitor.visit_program(program);
        if visitor.changed && visitor.context.is_dynamic() {
            visitor.insert_module_info_import(program);
        }
        visitor.finish()
    }
}

pub(crate) struct ResourceRewriteResult {
    pub(crate) changed: bool,
    pub(crate) warnings: Vec<EtsTransformError>,
}

struct ResourceReferenceMutator<'a, 'refs> {
    allocator: &'a Allocator,
    id: &'refs str,
    resource_refs: &'refs FxHashMap<String, ResourceReference>,
    reference_index: &'refs ResourceReferenceIndex,
    names: ResourceNames,
    context: ResourceTransformContext,
    changed: bool,
    diagnostics: Vec<EtsTransformError>,
}

impl<'a> VisitMut<'a> for ResourceReferenceMutator<'a, '_> {
    fn visit_expression(&mut self, expression: &mut Expression<'a>) {
        let replacement = match expression {
            Expression::CallExpression(call) => self.resource_expression(call),
            _ => None,
        };
        if let Some(replacement) = replacement {
            *expression = replacement;
            self.changed = true;
            return;
        }
        walk_mut::walk_expression(self, expression);
    }
}

impl<'a> ResourceReferenceMutator<'a, '_> {
    fn finish(self) -> Result<ResourceRewriteResult, EtsTransformError> {
        if let Some(error) = self
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.is_error())
        {
            return Err(error.clone());
        }
        Ok(ResourceRewriteResult {
            changed: self.changed,
            warnings: self.diagnostics,
        })
    }

    fn insert_module_info_import(&self, program: &mut oxc_ast::ast::Program<'a>) {
        let already_imported = program.body.iter().any(|statement| {
            matches!(
                statement,
                Statement::ImportDeclaration(import) if import.source.value == "ModuleInfo"
            )
        });
        if already_imported {
            return;
        }
        let ast = AstBuilder::new(self.allocator);
        let specifiers = ArenaVec::from_iter_in(
            [
                ResourceTransformContext::MODULE_NAME,
                ResourceTransformContext::BUNDLE_NAME,
            ]
            .into_iter()
            .map(|name| {
                ImportDeclarationSpecifier::new_import_specifier(
                    SPAN,
                    oxc_ast::ast::ModuleExportName::new_identifier_name(SPAN, name, &ast),
                    BindingIdentifier::new(SPAN, Ident::from_str_in(name, &ast), &ast),
                    ImportOrExportKind::Value,
                    &ast,
                )
            }),
            &ast,
        );
        program.body.insert(
            0,
            Statement::new_import_declaration(
                SPAN,
                Some(specifiers),
                StringLiteral::new(SPAN, "ModuleInfo", None, &ast),
                None,
                None,
                ImportOrExportKind::Value,
                &ast,
            ),
        );
    }

    fn resource_expression(&mut self, call: &CallExpression<'_>) -> Option<Expression<'a>> {
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };

        let replacement = match callee.name.as_str() {
            ResourceCallName::RESOURCE => self.resource_object(call),
            ResourceCallName::RAWFILE => self.rawfile_object(call),
            _ => return None,
        }?;
        Some(replacement.expression(&AstBuilder::new(self.allocator)))
    }

    fn resource_object(&mut self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?.as_expression()?;
        let argument_text = ResourceArgumentText::new(first_arg);
        if let Some(resource_name) = argument_text.text() {
            return self.static_resource_object(
                call,
                resource_name.trim(),
                argument_text.is_template(),
            );
        }

        Some(self.dynamic_resource_object(call))
    }

    fn static_resource_object(
        &mut self,
        call: &CallExpression<'_>,
        resource_name: &str,
        is_template: bool,
    ) -> Option<ResourceObject<'a>> {
        let Some(name) = ResourceName::parse(resource_name) else {
            self.coded_diagnostic(
                "10905332",
                "Invalid resource file parameter. Enter a value in the format of 'xxx.yyy.zzz'.",
                Self::template_severity(is_template),
                call.span(),
            );
            return is_template.then(|| self.dynamic_resource_object(call));
        };
        if let Some(module) = name.resource_module() {
            return self.resource_module_object(call, &name, module, is_template);
        }
        self.app_or_system_resource_object(call, name, is_template)
    }

    fn app_or_system_resource_object(
        &mut self,
        call: &CallExpression<'_>,
        name: ResourceName<'_>,
        is_template: bool,
    ) -> Option<ResourceObject<'a>> {
        let Some(source) = ResourceSource::parse(name.source) else {
            self.coded_diagnostic(
                "10903331",
                &format!("Unknown resource source '{}'.", name.source),
                Self::template_severity(is_template),
                call.span(),
            );
            return is_template.then(|| self.dynamic_resource_object(call));
        };
        if !self
            .reference_index
            .contains_type(source, name.resource_type)
        {
            self.coded_diagnostic(
                "10903330",
                &format!("Unknown resource type '{}'.", name.resource_type),
                Self::template_severity(is_template),
                call.span(),
            );
            return is_template.then(|| self.dynamic_resource_object(call));
        }
        let resource_key = name.runtime_key(source);
        let Some(resource) = self.resource_refs.get(&resource_key) else {
            self.coded_diagnostic(
                "10903329",
                &format!("Unknown resource name '{}'.", name.name),
                Self::template_severity(is_template),
                call.span(),
            );
            return is_template.then(|| self.dynamic_resource_object(call));
        };
        if ResourceRuntimeType::from_name(name.resource_type).is_none() {
            self.unsupported_type(name.resource_type, is_template, call.span());
            return is_template.then(|| self.dynamic_resource_object(call));
        }
        Some(ResourceObject::app_static(
            resource,
            if self.context.compile_har {
                ResourceCallArguments::new(call, self.allocator).collect_all()
            } else {
                ResourceCallArguments::new(call, self.allocator).collect_after_first()
            },
            &self.context,
            source,
        ))
    }

    fn resource_module_object(
        &mut self,
        call: &CallExpression<'_>,
        name: &ResourceName<'_>,
        module_name: &str,
        is_template: bool,
    ) -> Option<ResourceObject<'a>> {
        if self.context.validate_resource_modules {
            let Some(module) = self
                .context
                .resource_modules
                .get(module_name)
                .filter(|module| module.has_resource_table())
            else {
                self.coded_diagnostic(
                    "10903331",
                    &format!("Unknown resource source '{module_name}'."),
                    DiagnosticSeverity::Warning,
                    call.span(),
                );
                return is_template.then(|| self.dynamic_resource_object(call));
            };
            if !module.contains_type(name.resource_type) {
                self.coded_diagnostic(
                    "10903330",
                    &format!("Unknown resource type '{}'.", name.resource_type),
                    DiagnosticSeverity::Warning,
                    call.span(),
                );
                return is_template.then(|| self.dynamic_resource_object(call));
            }
            if !module.contains_resource(name.resource_type, name.name) {
                self.coded_diagnostic(
                    "10903329",
                    &format!("Unknown resource name '{}'.", name.name),
                    DiagnosticSeverity::Warning,
                    call.span(),
                );
                return is_template.then(|| self.dynamic_resource_object(call));
            }
        }
        if ResourceRuntimeType::from_name(name.resource_type).is_none() {
            self.unsupported_type(name.resource_type, is_template, call.span());
            return is_template.then(|| self.dynamic_resource_object(call));
        }
        Some(ResourceObject::resource_module(
            module_name,
            ResourceCallArguments::new(call, self.allocator).collect_all(),
            &self.context,
        ))
    }

    fn dynamic_resource_object(&self, call: &CallExpression<'_>) -> ResourceObject<'a> {
        ResourceObject::app_dynamic(
            ResourceCallArguments::new(call, self.allocator).collect_all(),
            &self.names,
            &self.context,
        )
    }

    fn rawfile_object(&mut self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?;
        let static_argument = first_arg.as_expression().map(ResourceArgumentText::new);
        let static_text = static_argument
            .as_ref()
            .and_then(ResourceArgumentText::text);
        let is_static_string = static_text.is_some();
        let is_template = static_argument
            .as_ref()
            .is_some_and(|argument| argument.is_template());
        let resource_module = static_text.and_then(|text| ResourceModuleName::parse(text.trim()));
        if let Some(module) = resource_module {
            self.validate_module_rawfile(call, module);
        } else if let Some(resource_text) = static_text {
            if self.context.validate_rawfiles && !self.context.rawfiles.contains(resource_text) {
                self.coded_diagnostic(
                    "10904333",
                    &format!("No such '{resource_text}' resource in current module."),
                    Self::template_severity(is_template),
                    call.span(),
                );
                if is_template {
                    return Some(ResourceObject::rawfile(
                        -1,
                        ResourceCallArguments::new(call, self.allocator).collect_all(),
                        &self.names,
                        &self.context,
                        None,
                    ));
                }
            }
        }
        let params = if is_static_string {
            vec![first_arg.clone_in(self.allocator)]
        } else {
            ResourceCallArguments::new(call, self.allocator).collect_all()
        };
        Some(ResourceObject::rawfile(
            if resource_module.is_some() || !is_static_string {
                -1
            } else {
                0
            },
            params,
            &self.names,
            &self.context,
            resource_module.map(|module| module.name),
        ))
    }

    fn validate_module_rawfile(
        &mut self,
        call: &CallExpression<'_>,
        module: ResourceModuleName<'_>,
    ) {
        if !self.context.validate_resource_modules {
            return;
        }
        let Some(reference) = self.context.resource_modules.get(module.name) else {
            self.diagnostic(
                &format!("Unknown resource source '[{}]'.", module.name),
                DiagnosticSeverity::Warning,
                call.span(),
            );
            return;
        };
        if !reference.has_rawfile_root() {
            self.diagnostic(
                &format!("Unknown resource source '[{}]'.", module.name),
                DiagnosticSeverity::Warning,
                call.span(),
            );
        } else if !reference.contains_rawfile(module.resource) {
            self.diagnostic(
                &format!("No such '[{}].{}' resource.", module.name, module.resource),
                DiagnosticSeverity::Warning,
                call.span(),
            );
        }
    }

    fn unsupported_type(&mut self, resource_type: &str, is_template: bool, span: Span) {
        self.coded_diagnostic(
            "10906334",
            &format!("The resource type '{resource_type}' is not supported."),
            Self::template_severity(is_template),
            span,
        );
    }

    fn template_severity(is_template: bool) -> DiagnosticSeverity {
        if is_template {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Error
        }
    }

    fn coded_diagnostic(
        &mut self,
        code: &str,
        message: &str,
        severity: DiagnosticSeverity,
        span: Span,
    ) {
        self.diagnostics.push(EtsTransformError::CodedDiagnostic {
            file: self.id.to_string(),
            code: code.to_string(),
            message: message.to_string(),
            severity,
            span: Some(span),
            solutions: Vec::new(),
        });
    }

    fn diagnostic(&mut self, message: &str, severity: DiagnosticSeverity, _span: Span) {
        self.diagnostics.push(EtsTransformError::Diagnostic {
            file: self.id.to_string(),
            message: message.to_string(),
            severity,
        });
    }
}

struct ResourceArgumentText<'a> {
    expression: &'a Expression<'a>,
}

impl<'a> ResourceArgumentText<'a> {
    fn new(expression: &'a Expression<'a>) -> Self {
        Self { expression }
    }

    fn text(&self) -> Option<&'a str> {
        match self.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            Expression::TemplateLiteral(literal) if literal.expressions.is_empty() => literal
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_deref()),
            _ => None,
        }
    }

    fn is_template(&self) -> bool {
        matches!(self.expression, Expression::TemplateLiteral(_))
    }
}

#[derive(Debug, Clone, Copy)]
struct ResourceModuleName<'a> {
    name: &'a str,
    resource: &'a str,
}

impl<'a> ResourceModuleName<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let value = value.trim();
        let rest = value.strip_prefix('[')?;
        let end = rest.find(']')?;
        let name = rest.get(..end)?.trim();
        let resource = rest.get(end + 1..)?.strip_prefix('.')?;
        (!resource.is_empty()).then_some(Self { name, resource })
    }
}

struct ResourceCallArguments<'a, 'b> {
    call: &'b CallExpression<'b>,
    allocator: &'a Allocator,
}

impl<'a, 'b> ResourceCallArguments<'a, 'b> {
    fn new(call: &'b CallExpression<'b>, allocator: &'a Allocator) -> Self {
        Self { call, allocator }
    }

    fn collect_all(&self) -> Vec<Argument<'a>> {
        self.collect_from(0)
    }

    fn collect_after_first(&self) -> Vec<Argument<'a>> {
        self.collect_from(1)
    }

    fn collect_from(&self, skip: usize) -> Vec<Argument<'a>> {
        self.call
            .arguments
            .iter()
            .skip(skip)
            .map(|argument| argument.clone_in(self.allocator))
            .collect()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ResourceRuntimeType {
    value: u32,
}

impl ResourceRuntimeType {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        let value = match name {
            "color" => 10001,
            "float" => 10002,
            "string" => 10003,
            "plural" => 10004,
            "boolean" => 10005,
            "intarray" => 10006,
            "integer" => 10007,
            "pattern" => 10008,
            "strarray" => 10009,
            "media" => 20000,
            "rawfile" => RAWFILE_RUNTIME_TYPE as u32,
            "symbol" => 40000,
            _ => return None,
        };
        Some(Self { value })
    }

    pub(crate) fn value(self) -> u32 {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ResourceSource {
    App,
    Sys,
}

impl ResourceSource {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "app" => Some(Self::App),
            "sys" => Some(Self::Sys),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Sys => "sys",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceName<'a> {
    source: &'a str,
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceName<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let mut scanner = ResourceNameScanner::new(value);
        let source = scanner.next_component()?;
        let resource_type = scanner.next_component()?;
        let name = scanner.next_component()?;
        scanner.is_complete().then_some(Self {
            source,
            resource_type,
            name,
        })
    }

    fn runtime_key(&self, source: ResourceSource) -> String {
        ResourceRuntimeKey::new(source, self.resource_type, self.name).to_key()
    }

    fn resource_module(&self) -> Option<&'a str> {
        let name = self.source.strip_prefix('[')?.strip_suffix(']')?.trim();
        Some(name)
    }
}

pub(crate) struct ResourceRuntimeKey<'a> {
    source: ResourceSource,
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceRuntimeKey<'a> {
    pub(crate) fn new(source: ResourceSource, resource_type: &'a str, name: &'a str) -> Self {
        Self {
            source,
            resource_type,
            name,
        }
    }

    pub(crate) fn to_key(&self) -> String {
        let mut key = String::with_capacity(
            self.source.as_str().len() + 1 + self.resource_type.len() + 1 + self.name.len(),
        );
        key.push_str(self.source.as_str());
        key.push('.');
        key.push_str(self.resource_type);
        key.push('.');
        key.push_str(self.name);
        key
    }
}

struct ResourceNameScanner<'a> {
    value: &'a str,
    cursor: usize,
    ended_without_separator: bool,
}

impl<'a> ResourceNameScanner<'a> {
    fn new(value: &'a str) -> Self {
        Self {
            value,
            cursor: 0,
            ended_without_separator: false,
        }
    }

    fn next_component(&mut self) -> Option<&'a str> {
        if self.cursor > self.value.len()
            || (self.cursor == self.value.len() && self.ended_without_separator)
        {
            return None;
        }
        let rest = self.value.get(self.cursor..)?;
        let end = rest
            .char_indices()
            .find_map(|(index, ch)| (ch == '.').then_some(index))
            .unwrap_or(rest.len());
        let component = rest.get(..end)?;
        self.ended_without_separator = end == rest.len();
        self.cursor += end + usize::from(end < rest.len());
        Some(component)
    }

    fn is_complete(&self) -> bool {
        self.ended_without_separator && self.cursor >= self.value.len()
    }
}
