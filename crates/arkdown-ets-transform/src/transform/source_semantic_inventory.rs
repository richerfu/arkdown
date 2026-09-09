//! OXC-owned source inventory for the TypeScript semantic phase.
//!
//! TypeScript owns type inference, but it must not rediscover every transform
//! candidate by traversing the complete project a second time. OXC records the
//! exact syntax whose upstream transform calls the checker:
//!
//! - `process_component_member.ts::isSimpleType` for component properties;
//! - `process_component_build.ts::{isWrappedBuilder,isMutableBuilder}` for
//!   `.builder` accesses;
//! - `process_component_build.ts::isRegularAttrNode` for enum-valued ArkUI
//!   attribute arguments.
//!
//! The same parse records the source-only `$field` and `$$binding` names used
//! by `ets_checker.ts::validateError/validateNewExtend`.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionBody, CallExpression,
    ChainElement, Declaration, Decorator, ExportDeclaration, Expression, Function, FunctionBody,
    IfStatement, MethodDefinition, ObjectPropertyKind, PropertyKey, Statement,
    StaticMemberExpression, StructElement, StructStatement,
};
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::scope::ScopeFlags;

use super::{
    component_meta::ComponentMetadata,
    decorator::{ArkuiDecorator, ArkuiDecoratorName},
};

pub struct ArktsSourceSemanticCollector<'metadata> {
    component_metadata: &'metadata ComponentMetadata,
}

impl<'metadata> ArktsSourceSemanticCollector<'metadata> {
    pub fn new(component_metadata: &'metadata ComponentMetadata) -> Self {
        Self { component_metadata }
    }

    pub fn collect(
        &self,
        source: &str,
        include_ui_diagnostic_names: bool,
    ) -> ArktsSourceSemanticInventory {
        let allocator = Allocator::default();
        let parsed = self
            .component_metadata
            .parse_ets(&allocator, source, SourceType::ets());
        let mut collector = SourceSemanticInventoryCollector {
            source,
            component_metadata: self.component_metadata,
            include_ui_diagnostic_names,
            inventory: ArktsSourceSemanticInventory::default(),
        };
        collector.visit_program(&parsed.program);
        collector.inventory.normalize();
        collector.inventory
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArktsSourceSemanticInventory {
    diagnostic_names: BTreeSet<String>,
    property_queries: Vec<oxc_span::Span>,
    builder_access_queries: Vec<oxc_span::Span>,
    member_access_queries: Vec<oxc_span::Span>,
}

impl ArktsSourceSemanticInventory {
    pub fn diagnostic_names(&self) -> &BTreeSet<String> {
        &self.diagnostic_names
    }

    pub fn property_queries(&self) -> &[oxc_span::Span] {
        &self.property_queries
    }

    pub fn builder_access_queries(&self) -> &[oxc_span::Span] {
        &self.builder_access_queries
    }

    pub fn member_access_queries(&self) -> &[oxc_span::Span] {
        &self.member_access_queries
    }

    pub fn has_type_queries(&self) -> bool {
        !self.property_queries.is_empty()
            || !self.builder_access_queries.is_empty()
            || !self.member_access_queries.is_empty()
    }

    fn normalize(&mut self) {
        Self::normalize_spans(&mut self.property_queries);
        Self::normalize_spans(&mut self.builder_access_queries);
        Self::normalize_spans(&mut self.member_access_queries);
    }

    fn normalize_spans(spans: &mut Vec<oxc_span::Span>) {
        spans.sort_unstable_by_key(|span| (span.start, span.end));
        spans.dedup();
    }
}

struct SourceSemanticInventoryCollector<'source, 'metadata> {
    source: &'source str,
    component_metadata: &'metadata ComponentMetadata,
    include_ui_diagnostic_names: bool,
    inventory: ArktsSourceSemanticInventory,
}

impl SourceSemanticInventoryCollector<'_, '_> {
    fn collect_function_body(&mut self, body: Option<&FunctionBody<'_>>) {
        let Some(body) = body else {
            return;
        };
        let mut collector = BuildDoubleDollarNameCollector {
            source: self.source,
            component_metadata: self.component_metadata,
            names: &mut self.inventory.diagnostic_names,
            member_access_queries: &mut self.inventory.member_access_queries,
            collect_diagnostic_names: self.include_ui_diagnostic_names,
        };
        collector.collect_statements(&body.statements);
    }

    fn has_decorator(decorators: &[Decorator<'_>], name: &str) -> bool {
        decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn method_is_ui_body(method: &MethodDefinition<'_>) -> bool {
        matches!(&method.key, PropertyKey::StaticIdentifier(name) if name.name == "build")
            || Self::has_decorator(&method.decorators, ArkuiDecoratorName::BUILDER)
            || Self::has_decorator(&method.decorators, ArkuiDecoratorName::LOCAL_BUILDER)
    }

    fn function_is_builder(function: &Function<'_>) -> bool {
        Self::has_decorator(&function.decorators, ArkuiDecoratorName::BUILDER)
    }

    fn collect_exported_builder(&mut self, export: &ExportDeclaration<'_>) {
        if !Self::has_decorator(&export.decorators, ArkuiDecoratorName::BUILDER) {
            return;
        }
        if let Declaration::FunctionDeclaration(function) = &export.declaration {
            self.collect_function_body(function.body.as_deref());
        }
    }
}

impl<'ast> Visit<'ast> for SourceSemanticInventoryCollector<'_, '_> {
    fn visit_struct_statement(&mut self, structure: &StructStatement<'ast>) {
        for element in &structure.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            self.inventory.property_queries.push(property.key.span());
            if !self.include_ui_diagnostic_names {
                continue;
            }
            let PropertyKey::StaticIdentifier(name) = &property.key else {
                continue;
            };
            if property.decorators.iter().any(|decorator| {
                ArkuiDecoratorName::contributes_checker_dollar_property(
                    ArkuiDecorator::new(decorator).name(),
                )
            }) {
                self.inventory
                    .diagnostic_names
                    .insert(format!("${}", name.name));
            }
        }
        walk::walk_struct_statement(self, structure);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'ast>) {
        if member.property.name.as_str()
            == self
                .component_metadata
                .runtime_names()
                .builder_property
                .as_str()
        {
            self.inventory.builder_access_queries.push(member.span);
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'ast>) {
        if Self::method_is_ui_body(method) {
            self.collect_function_body(method.value.body.as_deref());
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_function(&mut self, function: &Function<'ast>, flags: ScopeFlags) {
        if Self::function_is_builder(function) {
            self.collect_function_body(function.body.as_deref());
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_export_declaration(&mut self, export: &ExportDeclaration<'ast>) {
        self.collect_exported_builder(export);
        walk::walk_export_declaration(self, export);
    }
}

struct BuildDoubleDollarNameCollector<'source, 'metadata, 'names> {
    source: &'source str,
    component_metadata: &'metadata ComponentMetadata,
    names: &'names mut BTreeSet<String>,
    member_access_queries: &'names mut Vec<oxc_span::Span>,
    collect_diagnostic_names: bool,
}

impl BuildDoubleDollarNameCollector<'_, '_, '_> {
    fn collect_statements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            self.collect_statement(statement);
        }
    }

    fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.collect_build_expression(&statement.expression);
            }
            Statement::IfStatement(statement) => self.collect_if_statement(statement),
            _ => {}
        }
    }

    fn collect_if_statement(&mut self, statement: &IfStatement<'_>) {
        if let Statement::BlockStatement(block) = &statement.consequent {
            self.collect_statements(&block.body);
        }
        match statement.alternate.as_ref() {
            Some(Statement::IfStatement(alternate)) => self.collect_if_statement(alternate),
            Some(Statement::BlockStatement(block)) => self.collect_statements(&block.body),
            _ => {}
        }
    }

    fn collect_build_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::ArkUIComponentExpression(component) => self.collect_component(component),
            Expression::CallExpression(call) if self.collect_iteration_body(call) => {}
            Expression::CallExpression(call) => self.collect_leaf_component_call(call),
            _ => {}
        }
    }

    fn collect_iteration_body(&mut self, call: &CallExpression<'_>) -> bool {
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        let runtime_names = self.component_metadata.runtime_names();
        if callee.name.as_str() != runtime_names.component_for_each
            && callee.name.as_str() != runtime_names.component_lazy_for_each
        {
            return false;
        }
        let Some(Expression::ArrowFunctionExpression(arrow)) =
            call.arguments.get(1).and_then(Argument::as_expression)
        else {
            return true;
        };
        if let ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            self.collect_statements(&body.statements);
        }
        true
    }

    fn collect_leaf_component_call(&mut self, call: &CallExpression<'_>) {
        let Some(component_name) = Self::leaf_component_name(call) else {
            return;
        };
        self.collect_leaf_call_layer(component_name, call);
    }

    fn leaf_component_name<'ast>(call: &'ast CallExpression<'ast>) -> Option<&'ast str> {
        match &call.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(member) => match &member.object {
                Expression::CallExpression(inner) => Self::leaf_component_name(inner),
                _ => None,
            },
            _ => None,
        }
    }

    fn collect_leaf_call_layer(&mut self, component_name: &str, call: &CallExpression<'_>) {
        match &call.callee {
            Expression::Identifier(_) => {
                self.collect_component_arguments(component_name, &call.arguments);
            }
            Expression::StaticMemberExpression(member) => {
                self.collect_attribute_arguments(
                    component_name,
                    member.property.name.as_str(),
                    &call.arguments,
                );
                if let Expression::CallExpression(inner) = &member.object {
                    self.collect_leaf_call_layer(component_name, inner);
                }
            }
            _ => {}
        }
    }

    fn collect_component(&mut self, component: &ArkUIComponentExpression<'_>) {
        let Some(component_name) = Self::component_name(component) else {
            return;
        };
        self.collect_component_arguments(component_name, &component.arguments);
        for chain in &component.chain_expressions {
            self.collect_chain_arguments(component_name, chain);
        }
        for child in &component.children {
            match child {
                ArkUIChild::Component(component) => self.collect_component(component),
                ArkUIChild::Statement(statement) => self.collect_statement(statement),
                ArkUIChild::Expression(expression) => self.collect_build_expression(expression),
            }
        }
    }

    fn component_name<'ast>(component: &'ast ArkUIComponentExpression<'ast>) -> Option<&'ast str> {
        match &component.callee {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn collect_component_arguments(&mut self, component_name: &str, arguments: &[Argument<'_>]) {
        for argument in arguments {
            if let Some(expression) = argument.as_expression() {
                self.collect_regular_member_accesses(expression);
            }
            let Some(Expression::ObjectExpression(object)) = argument.as_expression() else {
                continue;
            };
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                let PropertyKey::StaticIdentifier(name) = &property.key else {
                    continue;
                };
                if self
                    .component_metadata
                    .is_component_double_dollar_property_name(component_name, name.name.as_str())
                {
                    self.collect_double_dollar_root(&property.value);
                }
            }
        }
    }

    fn collect_chain_arguments(&mut self, component_name: &str, chain: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(member) = &chain.callee else {
            return;
        };
        self.collect_attribute_arguments(
            component_name,
            member.property.name.as_str(),
            &chain.arguments,
        );
    }

    fn collect_attribute_arguments(
        &mut self,
        component_name: &str,
        attribute_name: &str,
        arguments: &[Argument<'_>],
    ) {
        for argument in arguments {
            if let Some(expression) = argument.as_expression() {
                self.collect_regular_member_accesses(expression);
            }
        }
        if !self
            .component_metadata
            .is_component_double_dollar_property(component_name, attribute_name)
            && !self
                .component_metadata
                .is_style_double_dollar_attr(attribute_name)
        {
            return;
        }
        for argument in arguments {
            if let Some(expression) = argument.as_expression() {
                self.collect_double_dollar_root(expression);
            }
        }
    }

    /// Source owner: `process_component_build.ts::isRegularAttrNode`.
    /// Only the `Identifier.Member` branch asks TypeScript for a type; object,
    /// array and parenthesized expressions recurse into that same branch.
    fn collect_regular_member_accesses(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    if let ObjectPropertyKind::ObjectProperty(property) = property {
                        self.collect_regular_member_accesses(&property.value);
                    }
                }
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = element.as_expression() {
                        self.collect_regular_member_accesses(expression);
                    }
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.collect_regular_member_accesses(&parenthesized.expression);
            }
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::Identifier(_)) =>
            {
                self.member_access_queries.push(member.span);
            }
            _ => {}
        }
    }

    fn collect_double_dollar_root(&mut self, expression: &Expression<'_>) {
        if !self.collect_diagnostic_names {
            return;
        }
        let span = expression.span();
        if !self
            .source
            .get(span.start as usize..span.end as usize)
            .is_some_and(|text| text.starts_with("$$"))
        {
            return;
        }
        if let Some(name) = Self::root_identifier(expression) {
            self.names.insert(name.to_string());
        }
    }

    fn root_identifier<'ast>(expression: &'ast Expression<'ast>) -> Option<&'ast str> {
        match expression {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(member) => Self::root_identifier(&member.object),
            Expression::ComputedMemberExpression(member) => Self::root_identifier(&member.object),
            Expression::CallExpression(call) => Self::root_identifier(&call.callee),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(call) => Self::root_identifier(&call.callee),
                ChainElement::TSNonNullExpression(expression) => {
                    Self::root_identifier(&expression.expression)
                }
                ChainElement::StaticMemberExpression(member) => {
                    Self::root_identifier(&member.object)
                }
                ChainElement::ComputedMemberExpression(member) => {
                    Self::root_identifier(&member.object)
                }
                ChainElement::PrivateFieldExpression(member) => {
                    Self::root_identifier(&member.object)
                }
            },
            Expression::TSAsExpression(expression) => Self::root_identifier(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => {
                Self::root_identifier(&expression.expression)
            }
            Expression::TSNonNullExpression(expression) => {
                Self::root_identifier(&expression.expression)
            }
            Expression::TSInstantiationExpression(expression) => {
                Self::root_identifier(&expression.expression)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ArktsSourceSemanticCollector, ComponentMetadata};

    fn metadata() -> ComponentMetadata {
        ComponentMetadata::from_ets_loader_path(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ets-loader"
        )))
        .expect("test ets-loader metadata")
    }

    #[test]
    fn collects_only_source_owned_state_and_binding_names() {
        let metadata = metadata();
        let inventory = ArktsSourceSemanticCollector::new(&metadata).collect(
            r#"
@Component
struct Page {
  @State input: string = ''
  @Require plain: string = ''
  unrelated: string = ''

  build() {
    TextInput({ text: $$this.input, placeholder: $$notAccepted.placeholder })
      .bindSheet($$this.show, this.sheet)
      .fontSize($$notAccepted.size)
  }
}
"#,
            true,
        );
        let names = inventory.diagnostic_names();
        assert!(names.contains("$input"), "{names:?}");
        assert!(names.contains("$plain"), "{names:?}");
        assert!(names.contains("$$this"), "{names:?}");
        assert!(!names.contains("$unrelated"));
        assert!(!names.contains("$$notAccepted"));
    }

    #[test]
    fn collects_builder_and_conditional_binding_names() {
        let metadata = metadata();
        let inventory = ArktsSourceSemanticCollector::new(&metadata).collect(
            r#"
@Builder
function Content() {
  if (enabled) {
    TextInput({ text: $$model.value })
  } else if (fallback) {
    TextInput({ text: $$fallbackModel.value })
  }
}
"#,
            true,
        );
        let names = inventory.diagnostic_names();
        assert!(names.contains("$$model"), "{names:?}");
        assert!(names.contains("$$fallbackModel"), "{names:?}");
    }

    #[test]
    fn selects_only_upstream_transform_type_query_syntax() {
        let metadata = metadata();
        let source = r#"
@Component
struct Page {
  @State count: number = 0
  content: WrappedBuilder

  build() {
    Text('value').fontColor(Color.Red).fontSize(model.size)
    this.content.builder()
    service.unrelated()
  }
}
"#;
        let inventory = ArktsSourceSemanticCollector::new(&metadata).collect(source, false);
        let texts = |spans: &[oxc_span::Span]| {
            spans
                .iter()
                .map(|span| &source[span.start as usize..span.end as usize])
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(inventory.property_queries()), ["count", "content"]);
        assert_eq!(
            texts(inventory.builder_access_queries()),
            ["this.content.builder"]
        );
        assert_eq!(
            texts(inventory.member_access_queries()),
            ["Color.Red", "model.size"]
        );
        assert!(inventory.diagnostic_names().is_empty());
    }
}
