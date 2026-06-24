//! Resource-call lowering aligned with SDK `ets-loader` `processResourceData`.
//!
//! The matcher operates on OXC AST call expressions, mirroring upstream's
//! TypeScript AST visitor: only `$r(...)` and `$rawfile(...)` call nodes are
//! rewritten.  The replacement object expression is emitted from a small
//! structured model so the field order and static names stay centralized
//! instead of being assembled ad hoc at call sites.

use rustc_hash::FxHashMap;

use oxc_allocator::{Allocator, CloneIn};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, CallExpression, Expression, NumberBase, ObjectPropertyKind,
    PropertyKind,
};
use oxc_ast::AstBuilder;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_span::SPAN;

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
    id: i64,
    runtime_type: i64,
    params: Vec<Argument<'a>>,
    bundle_name: String,
    module_name: String,
}

impl<'a> ResourceObject<'a> {
    fn app_static(reference: &ResourceReference, params: Vec<Argument<'a>>) -> Self {
        Self {
            id: i64::from(reference.id),
            runtime_type: i64::from(reference.runtime_type),
            params,
            bundle_name: reference.bundle_name.clone(),
            module_name: reference.module_name.clone(),
        }
    }

    fn app_dynamic(params: Vec<Argument<'a>>, names: &ResourceNames) -> Self {
        Self {
            id: -1,
            runtime_type: -1,
            params,
            bundle_name: names.bundle_name.clone(),
            module_name: names.module_name.clone(),
        }
    }

    fn rawfile(id: i64, params: Vec<Argument<'a>>, names: &ResourceNames) -> Self {
        Self {
            id,
            runtime_type: RAWFILE_RUNTIME_TYPE,
            params,
            bundle_name: names.bundle_name.clone(),
            module_name: names.module_name.clone(),
        }
    }

    fn expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
        ResourceObjectEmitter::new(self).expression(ast)
    }
}

struct ResourceObjectEmitter<'a, 'b> {
    resource: &'a ResourceObject<'b>,
}

impl<'a, 'b> ResourceObjectEmitter<'a, 'b> {
    fn new(resource: &'a ResourceObject<'b>) -> Self {
        Self { resource }
    }

    fn expression<'c>(&self, ast: AstBuilder<'c>) -> Expression<'c> {
        ast.expression_object(SPAN, ast.vec_from_iter(self.properties(ast)))
    }

    fn properties<'c>(&self, ast: AstBuilder<'c>) -> [ObjectPropertyKind<'c>; 5] {
        [
            self.number_property(ast, ResourceObjectPropertyName::ID, self.resource.id),
            self.number_property(
                ast,
                ResourceObjectPropertyName::TYPE,
                self.resource.runtime_type,
            ),
            self.params_property(ast),
            self.string_property(
                ast,
                ResourceObjectPropertyName::BUNDLE_NAME,
                &self.resource.bundle_name,
            ),
            self.string_property(
                ast,
                ResourceObjectPropertyName::MODULE_NAME,
                &self.resource.module_name,
            ),
        ]
    }

    fn number_property<'c>(
        &self,
        ast: AstBuilder<'c>,
        name: &'static str,
        value: i64,
    ) -> ObjectPropertyKind<'c> {
        let raw = ast.str(&value.to_string());
        self.property(
            ast,
            name,
            ast.expression_numeric_literal(SPAN, value as f64, Some(raw), NumberBase::Decimal),
        )
    }

    fn string_property<'c>(
        &self,
        ast: AstBuilder<'c>,
        name: &'static str,
        value: &str,
    ) -> ObjectPropertyKind<'c> {
        self.property(
            ast,
            name,
            ast.expression_string_literal(SPAN, ast.str(value), None),
        )
    }

    fn params_property<'c>(&self, ast: AstBuilder<'c>) -> ObjectPropertyKind<'c> {
        let elements = ast.vec_from_iter(
            self.resource
                .params
                .iter()
                .map(|argument| ArrayExpressionElement::from(argument.clone_in(ast.allocator))),
        );
        self.property(
            ast,
            ResourceObjectPropertyName::PARAMS,
            ast.expression_array(SPAN, elements),
        )
    }

    fn property<'c>(
        &self,
        ast: AstBuilder<'c>,
        name: &'static str,
        value: Expression<'c>,
    ) -> ObjectPropertyKind<'c> {
        ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(SPAN, ast.str(name)),
            value,
            false,
            false,
            false,
        )
    }
}

struct ResourceObjectPropertyName;

impl ResourceObjectPropertyName {
    const ID: &'static str = "id";
    const TYPE: &'static str = "type";
    const PARAMS: &'static str = "params";
    const BUNDLE_NAME: &'static str = "bundleName";
    const MODULE_NAME: &'static str = "moduleName";
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResourceNames {
    bundle_name: String,
    module_name: String,
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
    source: &'a str,
    resource_refs: &'a FxHashMap<String, ResourceReference>,
    names: ResourceNames,
}

impl<'a> ResourceReferenceRewriter<'a> {
    pub(crate) fn new(
        source: &'a str,
        id: &'a str,
        resource_refs: &'a FxHashMap<String, ResourceReference>,
        bundle_name: &str,
        module_name: Option<&str>,
    ) -> Self {
        let _ = id;
        Self {
            source,
            resource_refs,
            names: ResourceNames::new(bundle_name, module_name),
        }
    }

    pub(crate) fn may_rewrite(&self) -> bool {
        self.source.contains(ResourceCallName::RESOURCE)
            || self.source.contains(ResourceCallName::RAWFILE)
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
            changed: false,
        };
        visitor.visit_program(program);
        visitor.changed
    }
}

struct ResourceReferenceMutator<'a, 'refs> {
    allocator: &'a Allocator,
    resource_refs: &'refs FxHashMap<String, ResourceReference>,
    names: ResourceNames,
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
    fn resource_expression(&self, call: &CallExpression<'_>) -> Option<Expression<'a>> {
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };

        let replacement = match callee.name.as_str() {
            ResourceCallName::RESOURCE => self.resource_object(call),
            ResourceCallName::RAWFILE => self.rawfile_object(call),
            _ => return None,
        }?;
        Some(replacement.expression(AstBuilder::new(self.allocator)))
    }

    fn resource_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?.as_expression()?;
        if let Expression::StringLiteral(resource_name) = first_arg {
            let resource_key = ResourceName::parse(resource_name.value.as_str())?.into_key();
            let resource = self.resource_refs.get(&resource_key)?;
            return Some(ResourceObject::app_static(
                resource,
                ResourceCallArguments::new(call, self.allocator).collect_after_first(),
            ));
        }

        Some(ResourceObject::app_dynamic(
            ResourceCallArguments::new(call, self.allocator).collect_all(),
            &self.names,
        ))
    }

    fn rawfile_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject<'a>> {
        let first_arg = call.arguments.first()?;
        let is_static_string = first_arg
            .as_expression()
            .is_some_and(|expr| matches!(expr, Expression::StringLiteral(_)));
        let params = if is_static_string {
            vec![first_arg.clone_in(self.allocator)]
        } else {
            ResourceCallArguments::new(call, self.allocator).collect_all()
        };
        Some(ResourceObject::rawfile(
            if is_static_string { 0 } else { -1 },
            params,
            &self.names,
        ))
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
