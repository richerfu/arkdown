//! Resource-call lowering aligned with SDK `ets-loader` `processResourceData`.
//!
//! The matcher operates on OXC AST call expressions, mirroring upstream's
//! TypeScript AST visitor: only `$r(...)` and `$rawfile(...)` call nodes are
//! rewritten.  The replacement object expression is emitted from a small
//! structured model so the field order and static names stay centralized
//! instead of being assembled ad hoc at call sites.

use rustc_hash::FxHashMap;

use oxc_allocator::{Allocator, ArenaVec, CloneIn, GetAllocator};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingIdentifier, CallExpression, Expression,
    FormalParameterKind, FormalParameters, FunctionBody, FunctionType, ImportDeclarationSpecifier,
    ImportOrExportKind, NumberBase, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    StringLiteral,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_span::SPAN;
use oxc_str::{Ident, Str};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

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

pub(crate) struct ResourceReferenceRewriter<'a> {
    resource_refs: &'a FxHashMap<String, ResourceReference>,
    names: ResourceNames,
    context: ResourceTransformContext,
}

impl<'a> ResourceReferenceRewriter<'a> {
    pub(crate) fn new(
        source: &'a str,
        id: &'a str,
        resource_refs: &'a FxHashMap<String, ResourceReference>,
        bundle_name: &str,
        module_name: Option<&str>,
    ) -> Self {
        let _ = source;
        let _ = id;
        Self {
            resource_refs,
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
    ) -> bool {
        let mut visitor = ResourceReferenceMutator {
            allocator,
            resource_refs: self.resource_refs,
            names: self.names.clone(),
            context: self.context.clone(),
            changed: false,
        };
        visitor.visit_program(program);
        if visitor.changed && visitor.context.is_dynamic() {
            visitor.insert_module_info_import(program);
        }
        visitor.changed
    }
}

struct ResourceReferenceMutator<'a, 'refs> {
    allocator: &'a Allocator,
    resource_refs: &'refs FxHashMap<String, ResourceReference>,
    names: ResourceNames,
    context: ResourceTransformContext,
    changed: bool,
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

    fn resource_expression(&self, call: &CallExpression<'_>) -> Option<Expression<'a>> {
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

    fn resource_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?.as_expression()?;
        let argument_text = ResourceArgumentText::new(first_arg);
        if let Some(resource_name) = argument_text.text() {
            if let Some(module) = ResourceModuleName::parse(resource_name) {
                return Some(ResourceObject::resource_module(
                    module.name,
                    ResourceCallArguments::new(call, self.allocator).collect_all(),
                    &self.context,
                ));
            }
            let Some(name) = ResourceName::parse(resource_name) else {
                return argument_text
                    .is_template()
                    .then(|| self.dynamic_resource_object(call));
            };
            let source = name.source;
            let resource_key = name.into_key();
            let Some(resource) = self.resource_refs.get(&resource_key) else {
                return argument_text
                    .is_template()
                    .then(|| self.dynamic_resource_object(call));
            };
            return Some(ResourceObject::app_static(
                resource,
                if self.context.compile_har {
                    ResourceCallArguments::new(call, self.allocator).collect_all()
                } else {
                    ResourceCallArguments::new(call, self.allocator).collect_after_first()
                },
                &self.context,
                source,
            ));
        }

        Some(self.dynamic_resource_object(call))
    }

    fn dynamic_resource_object(&self, call: &CallExpression<'_>) -> ResourceObject<'a> {
        ResourceObject::app_dynamic(
            ResourceCallArguments::new(call, self.allocator).collect_all(),
            &self.names,
            &self.context,
        )
    }

    fn rawfile_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?;
        let static_text = first_arg
            .as_expression()
            .and_then(|expression| ResourceArgumentText::new(expression).text());
        let is_static_string = static_text.is_some();
        let resource_module = static_text.and_then(ResourceModuleName::parse);
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
            Expression::StringLiteral(literal) => Some(literal.value.as_str().trim()),
            Expression::TemplateLiteral(literal) if literal.expressions.is_empty() => literal
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_deref())
                .map(str::trim),
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
}

impl<'a> ResourceModuleName<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let value = value.trim();
        let rest = value.strip_prefix('[')?;
        let end = rest.find(']')?;
        let name = rest.get(..end)?.trim();
        let resource = rest.get(end + 1..)?.strip_prefix('.')?;
        (!name.is_empty() && !resource.is_empty()).then_some(Self { name })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    source: ResourceSource,
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceName<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let mut scanner = ResourceNameScanner::new(value);
        let source = ResourceSource::parse(scanner.next_component()?)?;
        let resource_type = scanner.next_component()?;
        let name = scanner.next_component()?;
        (scanner.is_complete()
            && !resource_type.is_empty()
            && !name.is_empty()
            && ResourceRuntimeType::from_name(resource_type).is_some())
        .then_some(Self {
            source,
            resource_type,
            name,
        })
    }

    fn into_key(self) -> String {
        ResourceRuntimeKey::new(self.source, self.resource_type, self.name).to_key()
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
}

impl<'a> ResourceNameScanner<'a> {
    fn new(value: &'a str) -> Self {
        Self { value, cursor: 0 }
    }

    fn next_component(&mut self) -> Option<&'a str> {
        if self.cursor > self.value.len() {
            return None;
        }
        let rest = self.value.get(self.cursor..)?;
        let end = rest
            .char_indices()
            .find_map(|(index, ch)| (ch == '.').then_some(index))
            .unwrap_or(rest.len());
        let component = rest.get(..end)?;
        self.cursor += end + usize::from(end < rest.len());
        Some(component)
    }

    fn is_complete(&self) -> bool {
        self.cursor >= self.value.len()
    }
}
