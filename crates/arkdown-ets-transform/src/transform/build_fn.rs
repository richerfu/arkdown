use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt, sync::Arc};

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrayExpressionElement,
    ArrowFunctionExpression, AssignmentTarget, CallExpression, ChainElement, ChainExpression,
    Expression, FormalParameterKind, FunctionBody, IfStatement, MemberExpression, ObjectExpression,
    ObjectProperty, ObjectPropertyKind, PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::{GetSpan, Span, SPAN};
use oxc_syntax::operator::{AssignmentOperator, UnaryOperator};

use arkdown_path::HvigorModulePath;

use crate::source_map::offset_to_line_col;
use crate::transform::component_analysis::ExtendAttributeIndex;
use crate::transform::component_meta::{
    ArkuiRuntimeComponentKind, ArkuiRuntimeNames, ComponentMetadata,
};
use crate::transform::global_rewrite::GlobalContextCallRewriter;
use crate::transform::viewpu_emit::{GeneratedViewpuName, ViewpuStatementAstEmitter};

mod custom_component;
use custom_component::{CustomComponentStatementEmitter, CustomReuseId};
mod raw_object;
use raw_object::RawObjectArgumentTransform;
mod iteration;
use iteration::{
    ForEachKeyExpression, IterationArgument, IterationAstStatement, IterationComponent,
    IterationFormalParameters,
};
mod styles;
use styles::StylesAttributeIndex;
pub(crate) use styles::StylesMethodInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLoweringError {
    UnsupportedStatement,
    UnsupportedChild,
    UnsupportedCallee,
    UnsupportedChain,
    InvalidStateStyles,
    InvalidTrailingLambda {
        component: String,
        builder_param_count: usize,
    },
}

impl BuildLoweringError {
    pub(crate) fn hvigor_message(&self) -> &'static str {
        match self {
            Self::UnsupportedStatement => "Only UI component syntax can be written here.",
            Self::UnsupportedChild => "Unsupported ArkUI child syntax.",
            Self::UnsupportedCallee => "Unsupported ArkUI component callee.",
            Self::UnsupportedChain => "Unsupported ArkUI attribute chain.",
            Self::InvalidStateStyles => {
                "The stateStyles attribute requires an object whose values are @Styles references or inline style objects."
            }
            Self::InvalidTrailingLambda { .. } => {
                "A custom component trailing lambda requires exactly one @BuilderParam property."
            }
        }
    }
}

impl fmt::Display for BuildLoweringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTrailingLambda {
                component,
                builder_param_count,
            } => write!(
                f,
                "In the trailing lambda case, '{component}' must have one and only one property decorated with '@BuilderParam' (found {builder_param_count})."
            ),
            _ => f.write_str(self.hvigor_message()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomComponentGeneration {
    #[default]
    V1,
    V2,
}

impl CustomComponentGeneration {
    fn runtime_base_name(self) -> &'static str {
        match self {
            Self::V1 => GeneratedViewpuName::VIEWPU_BASE,
            Self::V2 => GeneratedViewpuName::VIEW_V2_BASE,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomComponentInfo {
    pub update_props: FxHashSet<String>,
    pub link_props: FxHashSet<String>,
    pub builder_params: FxHashSet<String>,
    pub reusable: bool,
    pub reusable_v2: bool,
    pub is_custom_dialog: bool,
    pub generation: CustomComponentGeneration,
}

impl CustomComponentInfo {
    fn runtime_base_name(&self) -> &'static str {
        self.generation.runtime_base_name()
    }
}

pub type CustomComponentUpdateProps = FxHashMap<String, CustomComponentInfo>;

#[derive(Clone, Debug)]
struct BuildContext {
    use_parent_context: bool,
    global_builder_parameter: Option<Arc<str>>,
    builder_proxy_scope: BuilderProxyScope,
    current_component_reusable: bool,
    inside_lazy_for_each: bool,
    inside_repeat: bool,
    raw_object_fields: Arc<FxHashSet<String>>,
    component_metadata: Arc<ComponentMetadata>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    module_name: Arc<str>,
    xcomponent_module_identity: Option<Arc<str>>,
    custom_component_base_name: &'static str,
}

impl Default for BuildContext {
    fn default() -> Self {
        Self {
            use_parent_context: false,
            global_builder_parameter: None,
            builder_proxy_scope: BuilderProxyScope::Component,
            current_component_reusable: false,
            inside_lazy_for_each: false,
            inside_repeat: false,
            raw_object_fields: Arc::default(),
            component_metadata: Arc::new(ComponentMetadata::default()),
            extend_attributes: Arc::new(ExtendAttributeIndex::default()),
            module_name: Arc::from(""),
            xcomponent_module_identity: None,
            custom_component_base_name: GeneratedViewpuName::VIEWPU_BASE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BuilderProxyScope {
    #[default]
    Component,
    Builder,
    GlobalBuilder,
}

pub(super) struct BuildSourceMetadata<'a> {
    source_text: &'a str,
    source_id: &'a str,
}

impl<'a> BuildSourceMetadata<'a> {
    fn new(source_text: &'a str, source_id: &'a str) -> Self {
        Self {
            source_text,
            source_id,
        }
    }

    fn nav_page_path(&self) -> String {
        HvigorModulePath::new(self.source_id).extensionless_path(HvigorModulePath::ETS_SEGMENT)
    }

    pub(super) fn source_position(&self, offset: u32) -> (usize, usize) {
        let (line, col) = offset_to_line_col(self.source_text, offset);
        (line as usize + 1, col as usize + 1)
    }

    pub(super) fn component_page_path(&self) -> String {
        HvigorModulePath::new(self.source_id)
            .hvigor_segment_path()
            .unwrap_or_else(|| self.source_id.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum BuildBodyInfo<'a> {
    Empty,
    Plan(BuildBodyPlan<'a>),
}

impl Default for BuildBodyInfo<'_> {
    fn default() -> Self {
        Self::Empty
    }
}

impl<'a> BuildBodyInfo<'a> {
    pub(crate) fn statements<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<OxcVec<'b, Statement<'b>>, BuildLoweringError> {
        match self {
            Self::Empty => Ok(OxcVec::new_in(&allocator)),
            Self::Plan(plan) => plan.statements(allocator),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BuildBodyPlan<'a> {
    body: Option<&'a FunctionBody<'a>>,
    source_text: Arc<str>,
    source_id: Arc<str>,
    custom_components: FxHashSet<String>,
    builder_methods: FxHashSet<String>,
    custom_component_update_props: CustomComponentUpdateProps,
    component_metadata: Arc<ComponentMetadata>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    styles: StylesAttributeIndex<'a>,
    raw_object_fields: FxHashSet<String>,
    module_name: Option<Arc<str>>,
    xcomponent_module_identity: Option<Arc<str>>,
    context_kind: BuildBodyContextKind,
    current_component_reusable: bool,
    custom_component_base_name: &'static str,
}

impl<'a> BuildBodyPlan<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn component_v2(
        body: Option<&'a FunctionBody<'a>>,
        source_text: &str,
        source_id: &str,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: &ComponentMetadata,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<&str>,
        current_component_reusable: bool,
    ) -> Self {
        Self::component_with_shared_inputs(
            body,
            Arc::from(source_text),
            Arc::from(source_id),
            custom_components,
            builder_methods,
            custom_component_update_props,
            Arc::new(component_metadata.clone()),
            raw_object_fields,
            module_name.map(Arc::from),
            current_component_reusable,
            GeneratedViewpuName::VIEW_V2_BASE,
        )
    }

    pub(crate) fn component_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        source_text: Arc<str>,
        source_id: Arc<str>,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: Arc<ComponentMetadata>,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<Arc<str>>,
        current_component_reusable: bool,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self::new(
            body,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            raw_object_fields,
            module_name,
            BuildBodyContextKind::Component,
            current_component_reusable,
            custom_component_base_name,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn builder_v2(
        body: Option<&'a FunctionBody<'a>>,
        source_text: &str,
        source_id: &str,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: &ComponentMetadata,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<&str>,
    ) -> Self {
        Self::builder_with_shared_inputs(
            body,
            Arc::from(source_text),
            Arc::from(source_id),
            custom_components,
            builder_methods,
            custom_component_update_props,
            Arc::new(component_metadata.clone()),
            raw_object_fields,
            module_name.map(Arc::from),
            GeneratedViewpuName::VIEW_V2_BASE,
        )
    }

    pub(crate) fn builder_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        source_text: Arc<str>,
        source_id: Arc<str>,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: Arc<ComponentMetadata>,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<Arc<str>>,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self::new(
            body,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            raw_object_fields,
            module_name,
            BuildBodyContextKind::Builder,
            false,
            custom_component_base_name,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn global_builder_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        source_text: Arc<str>,
        source_id: Arc<str>,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: Arc<ComponentMetadata>,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<Arc<str>>,
        custom_component_base_name: &'static str,
        first_parameter: Option<Arc<str>>,
    ) -> Self {
        Self::new(
            body,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            raw_object_fields,
            module_name,
            BuildBodyContextKind::GlobalBuilder { first_parameter },
            false,
            custom_component_base_name,
        )
    }

    fn new(
        body: Option<&'a FunctionBody<'a>>,
        source_text: Arc<str>,
        source_id: Arc<str>,
        custom_components: &FxHashSet<String>,
        builder_methods: &FxHashSet<String>,
        custom_component_update_props: &CustomComponentUpdateProps,
        component_metadata: Arc<ComponentMetadata>,
        raw_object_fields: &FxHashSet<String>,
        module_name: Option<Arc<str>>,
        context_kind: BuildBodyContextKind,
        current_component_reusable: bool,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self {
            body,
            source_text,
            source_id,
            custom_components: custom_components.clone(),
            builder_methods: builder_methods.clone(),
            custom_component_update_props: custom_component_update_props.clone(),
            component_metadata,
            extend_attributes: Arc::new(ExtendAttributeIndex::default()),
            styles: StylesAttributeIndex::default(),
            raw_object_fields: raw_object_fields.clone(),
            module_name,
            xcomponent_module_identity: None,
            context_kind,
            current_component_reusable,
            custom_component_base_name,
        }
    }

    fn statements<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<OxcVec<'b, Statement<'b>>, BuildLoweringError> {
        BuildBodyAstLowerer::new(self).statements(allocator)
    }

    pub(crate) fn with_shared_extend_attributes(
        mut self,
        extend_attributes: Arc<ExtendAttributeIndex>,
    ) -> Self {
        self.extend_attributes = extend_attributes;
        self
    }

    pub(crate) fn with_styles_methods(mut self, methods: &[StylesMethodInfo<'a>]) -> Self {
        self.styles = StylesAttributeIndex::from_methods(methods);
        self
    }

    pub(crate) fn with_xcomponent_module_identity(
        mut self,
        bundle_name: Option<&str>,
        module_name: Option<&str>,
    ) -> Self {
        self.xcomponent_module_identity = bundle_name
            .filter(|name| !name.is_empty())
            .zip(module_name.filter(|name| !name.is_empty()))
            .map(|(bundle_name, module_name)| Arc::from(format!("{bundle_name}/{module_name}")));
        self
    }

    fn context(&self) -> BuildContext {
        let mut context = match &self.context_kind {
            BuildBodyContextKind::Component => BuildContext::default()
                .with_current_component_reusable(self.current_component_reusable),
            BuildBodyContextKind::Builder => BuildContext::builder(),
            BuildBodyContextKind::GlobalBuilder { first_parameter } => {
                BuildContext::global_builder(first_parameter.clone())
            }
        }
        .with_raw_object_fields(&self.raw_object_fields);
        if let Some(module_name) = &self.module_name {
            context = context.with_module_name(module_name);
        }
        context = context.with_xcomponent_module_identity(
            self.xcomponent_module_identity.as_ref().map(Arc::clone),
        );
        context
            .with_shared_component_metadata(Arc::clone(&self.component_metadata))
            .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
            .with_custom_component_base_name(self.custom_component_base_name)
    }

    fn source_metadata(&self) -> BuildSourceMetadata<'_> {
        BuildSourceMetadata::new(&self.source_text, &self.source_id)
    }
}

#[derive(Clone, Debug)]
enum BuildBodyContextKind {
    Component,
    Builder,
    GlobalBuilder { first_parameter: Option<Arc<str>> },
}

struct BuildBodyAstLowerer<'a, 'plan> {
    plan: &'plan BuildBodyPlan<'a>,
}

impl<'a, 'plan> BuildBodyAstLowerer<'a, 'plan> {
    fn new(plan: &'plan BuildBodyPlan<'a>) -> Self {
        Self { plan }
    }

    fn statements<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<OxcVec<'b, Statement<'b>>, BuildLoweringError> {
        let Some(body) = self.plan.body else {
            return Ok(OxcVec::new_in(&allocator));
        };
        let mut statements = OxcVec::new_in(&allocator);
        let context = self.plan.context();
        if let Some(parameter) = &context.global_builder_parameter {
            let ast = AstBuilder::new(allocator);
            let capture_name = format!("__{parameter}__");
            let binding = ast.binding_pattern_binding_identifier(SPAN, ast.str(&capture_name));
            let initializer = ast.expression_identifier(SPAN, ast.str(parameter));
            let declarator = ast.variable_declarator(
                SPAN,
                VariableDeclarationKind::Const,
                binding,
                NONE,
                Some(initializer),
                false,
            );
            statements.push(Statement::from(ast.declaration_variable(
                SPAN,
                VariableDeclarationKind::Const,
                ast.vec1(declarator),
                false,
            )));
        }
        for statement in &body.statements {
            self.push_statement(&mut statements, statement, allocator, &context)?;
        }
        Ok(statements)
    }

    fn push_statement<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        statement: &Statement<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        match statement {
            Statement::BlockStatement(_) => Ok(()),
            Statement::EmptyStatement(_) => Ok(()),
            Statement::ExpressionStatement(statement) => {
                self.push_expression(statements, &statement.expression, allocator, context)
            }
            Statement::IfStatement(statement) => {
                self.push_if_statement(statements, statement, allocator, context)
            }
            _ => Err(BuildLoweringError::UnsupportedStatement),
        }
    }

    fn push_branch_statement<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        statement: &Statement<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.push_if_branch_child(statements, statement, allocator, context)?;
                }
                Ok(())
            }
            _ => self.push_if_branch_child(statements, statement, allocator, context),
        }
    }

    fn push_if_branch_child<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        statement: &Statement<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let mut child_statements = OxcVec::new_in(&allocator);
        self.push_statement(&mut child_statements, statement, allocator, context)?;
        let Some(retake_id) = self.if_retake_id(statement, context) else {
            statements.extend(child_statements);
            return Ok(());
        };
        statements.push(self.if_retake_statement(
            AstBuilder::new(allocator),
            allocator,
            retake_id,
            child_statements,
            context,
        ));
        Ok(())
    }

    fn if_retake_id<'source>(
        &self,
        statement: &'source Statement<'source>,
        context: &BuildContext,
    ) -> Option<&'source Expression<'source>> {
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        match &statement.expression {
            Expression::ArkUIComponentExpression(component) => component
                .chain_expressions
                .iter()
                .find(|chain| {
                    ArkuiCallName::chain(chain)
                        .is_ok_and(|name| name == context.runtime_names().attribute_id)
                })
                .and_then(|chain| chain.arguments.first())
                .and_then(Argument::as_expression),
            Expression::CallExpression(call) => LeafComponent::from_call(call, context)
                .ok()?
                .chain_attrs
                .into_iter()
                .find(|attribute| attribute.name == context.runtime_names().attribute_id)
                .and_then(|attribute| attribute.arguments.into_iter().next())
                .and_then(Argument::as_expression),
            _ => None,
        }
    }

    fn if_retake_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        retake_id: &Expression<'_>,
        child_statements: OxcVec<'b, Statement<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let callee = ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, ast.str(&context.runtime_names().component_if)),
            ast.identifier_name(SPAN, ast.str(&context.runtime_names().can_retake)),
            false,
        );
        let retake_id = BuildArgumentEmitter::new()
            .with_context(context)
            .expression_with_allocator(retake_id, allocator);
        let call = ast.expression_call(
            SPAN,
            Expression::from(callee),
            NONE,
            ast.vec1(Argument::from(retake_id)),
            false,
        );
        ast.statement_if(
            SPAN,
            ast.expression_unary(SPAN, UnaryOperator::LogicalNot, call),
            ast.statement_block(SPAN, child_statements),
            None,
        )
    }

    fn push_expression<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        match expression {
            Expression::ArkUIComponentExpression(component) => {
                self.push_component(statements, component, allocator, context)
            }
            Expression::CallExpression(call) => {
                if self.push_repeat_component(statements, call, allocator, context)? {
                    return Ok(());
                }
                if self.push_builder_method_call(statements, call, allocator, context) {
                    return Ok(());
                }
                if self.push_wrapped_builder_call(statements, call, allocator, context) {
                    return Ok(());
                }
                self.push_leaf_call(statements, call, allocator, context)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(call) => {
                    if self.push_wrapped_builder_chain_call(
                        statements, chain, call, allocator, context,
                    ) {
                        return Ok(());
                    }
                    Err(BuildLoweringError::UnsupportedStatement)
                }
                _ => Err(BuildLoweringError::UnsupportedStatement),
            },
            _ => Err(BuildLoweringError::UnsupportedStatement),
        }
    }

    fn push_component<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let name = ArkuiCallName::component(&component.callee)?;
        // SDK process_component_build.js::getComponentType resolves a known
        // custom component before CUSTOM_BUILDER_METHOD. This matters when an
        // imported component and a local @Builder method share a name:
        // unqualified `Name()` is the component, while `this.Name()` is the
        // builder method.
        if self.plan.builder_methods.contains(&name) && !self.plan.custom_components.contains(&name)
        {
            if !component.children.is_empty() {
                return Err(BuildLoweringError::UnsupportedChild);
            }
            statements.push(
                BuilderMethodCallEmitter::new(
                    BuilderMethodReceiver::Direct,
                    &name,
                    &component.arguments,
                    context,
                )
                .statement(AstBuilder::new(allocator), allocator),
            );
            return Ok(());
        }
        if self.plan.custom_components.contains(&name) {
            let source_metadata = self.plan.source_metadata();
            let reuse_id = CustomReuseId::from_component_attrs(component, context);
            let component_info = self.plan.custom_component_update_props.get(&name);
            let builder_param = self.custom_component_builder_param(
                &name,
                component,
                component_info,
                allocator,
                context,
            )?;
            let resolved_attrs = self.component_chain_attributes(component, context)?;
            let chain_attrs = resolved_attrs
                .iter()
                .filter(|attr| !context.is_reuse_id_attribute_name(&attr.name))
                .cloned()
                .collect::<Vec<_>>();
            if chain_attrs.is_empty() {
                statements.extend(CustomComponentStatementEmitter::custom(
                    allocator,
                    &name,
                    &component.arguments,
                    component_info,
                    builder_param,
                    reuse_id,
                    component.callee.span(),
                    &source_metadata,
                    context,
                ));
            } else {
                statements.extend(CustomComponentStatementEmitter::common_wrapped(
                    allocator,
                    &name,
                    &component.arguments,
                    component_info,
                    builder_param,
                    reuse_id,
                    component.callee.span(),
                    &chain_attrs,
                    &self.plan.builder_methods,
                    &source_metadata,
                    context,
                ));
            }
            return Ok(());
        }
        let component_kind = context.component_kind(&name);
        if component_kind.is_for_each() {
            return self.push_for_each(statements, &component.arguments, allocator, context);
        }
        if component_kind.is_lazy_for_each() {
            return self.push_lazy_for_each(statements, component, allocator, context);
        }
        if component_kind.is_item_component() {
            let uses_lazy_creation = component_kind.is_list_item()
                && !context.inside_lazy_for_each
                && !context.inside_repeat
                && IterationComponent::new(component).uses_lazy_creation();
            if uses_lazy_creation {
                return self.push_lazy_list_item(statements, component, allocator, context);
            }
            return self
                .push_observed_item_component(statements, component, &name, allocator, context);
        }
        if component_kind.is_nav_router() {
            return self.push_nav_component(statements, component, &name, allocator, context);
        }

        let ast = AstBuilder::new(allocator);
        let mut creation_stmts = OxcVec::new_in(&allocator);
        creation_stmts.push(ComponentCallStatementEmitter::create(
            ast,
            allocator,
            &name,
            context.create_method_name(component_kind, !component.children.is_empty()),
            &component.arguments,
            &self.plan.builder_methods,
            context,
            component.callee.span(),
        ));
        let resolved_attrs = self.component_chain_attributes(component, context)?;
        LeafAttributeStatementEmitter::new(
            &name,
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        for child in &component.children {
            self.push_child(statements, child, allocator, context)?;
        }
        if context.component_needs_pop(&name, component.arguments.iter()) {
            statements.push(ComponentCallStatementEmitter::empty_with_source(
                ast,
                &name,
                &context.runtime_names().method_pop,
                component.callee.span(),
            ));
        }
        Ok(())
    }

    fn custom_component_builder_param<'b, 'c>(
        &'c self,
        name: &str,
        component: &ArkUIComponentExpression<'_>,
        component_info: Option<&'c CustomComponentInfo>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<Option<(&'c str, OxcVec<'b, Statement<'b>>)>, BuildLoweringError> {
        if component.children.is_empty() {
            return Ok(None);
        }
        let Some(component_info) = component_info else {
            return Err(BuildLoweringError::InvalidTrailingLambda {
                component: name.to_string(),
                builder_param_count: 0,
            });
        };
        if component_info.builder_params.len() != 1 {
            return Err(BuildLoweringError::InvalidTrailingLambda {
                component: name.to_string(),
                builder_param_count: component_info.builder_params.len(),
            });
        }
        let builder_param_name = component_info
            .builder_params
            .iter()
            .next()
            .expect("single @BuilderParam name exists")
            .as_str();
        let mut body = OxcVec::new_in(&allocator);
        let builder_param_context = context.clone().with_builder_param_body();
        for child in &component.children {
            self.push_child(&mut body, child, allocator, &builder_param_context)?;
        }
        Ok(Some((builder_param_name, body)))
    }

    fn push_nav_component<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        name: &str,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let component_kind = context.component_kind(name);
        let children_in_create = component_kind == ArkuiRuntimeComponentKind::NavDestination
            && !component.children.is_empty();
        let builder_body = if children_in_create {
            let mut child_stmts = OxcVec::new_in(&allocator);
            for child in &component.children {
                self.push_child(&mut child_stmts, child, allocator, context)?;
            }
            Some(child_stmts)
        } else {
            None
        };
        let module_name = context.module_name.as_ref();
        let source_metadata = self.plan.source_metadata();
        let page_path = source_metadata.nav_page_path();
        let mut creation_stmts = OxcVec::new_in(&allocator);
        creation_stmts.push(
            NavigationCreateCall::new(component, name, module_name, &page_path).statement(
                ast,
                allocator,
                context,
                builder_body,
            ),
        );
        let resolved_attrs = self.component_chain_attributes(component, context)?;
        LeafAttributeStatementEmitter::new(
            name,
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        if !children_in_create {
            for child in &component.children {
                self.push_child(statements, child, allocator, context)?;
            }
        }
        if context.component_needs_pop(name, component.arguments.iter()) {
            statements.push(ComponentCallStatementEmitter::empty(
                ast,
                name,
                &context.runtime_names().method_pop,
            ));
        }
        Ok(())
    }

    fn push_leaf_call<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        if context.call_matches_component_kind(call, ArkuiRuntimeComponentKind::ForEach) {
            return self.push_for_each(statements, &call.arguments, allocator, context);
        }
        if context.call_matches_component_kind(call, ArkuiRuntimeComponentKind::LazyForEach) {
            return self.push_lazy_for_each_arguments(
                statements,
                &call.arguments,
                allocator,
                context,
            );
        }
        let lc = LeafComponent::from_call(call, context)?;
        let resolved_attrs = self.expand_leaf_attributes(&lc.chain_attrs, context)?;
        if self.plan.custom_components.contains(&lc.name) {
            let source_metadata = self.plan.source_metadata();
            let base_call = LeafComponent::base_call(call);
            let reuse_id = CustomReuseId::from_leaf_attrs(&resolved_attrs, context);
            let is_reusable_reuse_id_only = self
                .plan
                .custom_component_update_props
                .get(&lc.name)
                .is_some_and(|info| {
                    info.reusable
                        && resolved_attrs
                            .iter()
                            .all(|attr| context.is_reuse_id_attribute_name(&attr.name))
                });
            if resolved_attrs.is_empty() || is_reusable_reuse_id_only {
                statements.extend(CustomComponentStatementEmitter::custom(
                    allocator,
                    &lc.name,
                    &base_call.arguments,
                    self.plan.custom_component_update_props.get(&lc.name),
                    None,
                    reuse_id,
                    base_call.span,
                    &source_metadata,
                    context,
                ));
            } else {
                let common_attrs = resolved_attrs
                    .iter()
                    .filter(|attr| !context.is_reuse_id_attribute_name(&attr.name))
                    .cloned()
                    .collect::<Vec<_>>();
                if common_attrs.is_empty() {
                    statements.extend(CustomComponentStatementEmitter::custom(
                        allocator,
                        &lc.name,
                        &base_call.arguments,
                        self.plan.custom_component_update_props.get(&lc.name),
                        None,
                        reuse_id,
                        base_call.span,
                        &source_metadata,
                        context,
                    ));
                } else {
                    statements.extend(CustomComponentStatementEmitter::common_wrapped(
                        allocator,
                        &lc.name,
                        &base_call.arguments,
                        self.plan.custom_component_update_props.get(&lc.name),
                        None,
                        reuse_id,
                        base_call.span,
                        &common_attrs,
                        &self.plan.builder_methods,
                        &source_metadata,
                        context,
                    ));
                }
            }
            return Ok(());
        }
        let ast = AstBuilder::new(allocator);
        let mut creation_stmts = OxcVec::new_in(&allocator);
        let component_kind = context.component_kind(&lc.name);
        creation_stmts.push(ComponentCallStatementEmitter::create_refs(
            ast,
            allocator,
            &lc.name,
            context.create_method_name(component_kind, false),
            &lc.create_arguments,
            &self.plan.builder_methods,
            context,
        ));
        LeafAttributeStatementEmitter::new(
            &lc.name,
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &lc.name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        if context.component_needs_pop(&lc.name, lc.create_arguments.iter().copied()) {
            statements.push(ComponentCallStatementEmitter::empty(
                ast,
                &lc.name,
                &context.runtime_names().method_pop,
            ));
        }
        Ok(())
    }

    fn push_repeat_component<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<bool, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let Some(repeat_expression) = self.repeat_call_expression(call, allocator, context)? else {
            return Ok(false);
        };
        let render_callee = ast.member_expression_static(
            SPAN,
            repeat_expression,
            ast.identifier_name(SPAN, ast.str(&context.runtime_names().method_render)),
            false,
        );
        let render_call = ast.expression_call(
            SPAN,
            Expression::from(render_callee),
            NONE,
            ast.vec1(Argument::from(ast.expression_identifier(
                SPAN,
                ast.str(&context.runtime_names().is_initial_render),
            ))),
            false,
        );
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &context.runtime_names().component_repeat,
                context.runtime_names(),
                ast.vec1(ast.statement_expression(SPAN, render_call)),
            ),
        );
        Ok(true)
    }

    fn repeat_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<Option<Expression<'b>>, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        match expression {
            Expression::ParenthesizedExpression(parenthesized) => self
                .repeat_expression(&parenthesized.expression, allocator, context)
                .map(|expression| {
                    expression.map(|expression| ast.expression_parenthesized(SPAN, expression))
                }),
            Expression::CallExpression(call) => {
                self.repeat_call_expression(call, allocator, context)
            }
            _ => Ok(None),
        }
    }

    fn repeat_call_expression<'b>(
        &self,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<Option<Expression<'b>>, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        if self.is_repeat_base_call(call, context) {
            let mut arguments = OxcVec::new_in(&allocator);
            for argument in &call.arguments {
                arguments.push(
                    BuildArgumentEmitter::new()
                        .with_context(context)
                        .argument_with_allocator(argument, allocator),
                );
            }
            arguments.push(Argument::from(ast.expression_this(SPAN)));
            let callee = call.callee.clone_in(allocator);
            let type_arguments = call
                .type_arguments
                .as_ref()
                .map(|type_arguments| type_arguments.clone_in(allocator));
            return Ok(Some(ast.expression_call(
                call.span,
                callee,
                type_arguments,
                arguments,
                call.optional,
            )));
        }

        let Expression::StaticMemberExpression(member) = &call.callee else {
            return Ok(None);
        };
        let Some(object) = self.repeat_expression(&member.object, allocator, context)? else {
            return Ok(None);
        };
        let property_name = member.property.name.as_str();
        let arguments = self.repeat_method_arguments(call, property_name, allocator, context)?;
        let callee = ast.member_expression_static(
            member.span,
            object,
            member.property.clone_in(allocator),
            member.optional,
        );
        let type_arguments = call
            .type_arguments
            .as_ref()
            .map(|type_arguments| type_arguments.clone_in(allocator));
        Ok(Some(ast.expression_call(
            call.span,
            Expression::from(callee),
            type_arguments,
            arguments,
            call.optional,
        )))
    }

    fn repeat_method_arguments<'b>(
        &self,
        call: &CallExpression<'_>,
        method_name: &str,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<OxcVec<'b, Argument<'b>>, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut arguments = OxcVec::new_in(&allocator);
        for (index, argument) in call.arguments.iter().enumerate() {
            let should_lower_arrow = method_name == context.runtime_names().repeat_each
                && index == 0
                || method_name == context.runtime_names().repeat_template && index == 1;
            if should_lower_arrow {
                let Some(arrow) = IterationArgument::arrow(argument) else {
                    return Err(BuildLoweringError::UnsupportedChild);
                };
                arguments.push(Argument::from(
                    self.repeat_arrow_function(arrow, allocator, context)?,
                ));
            } else {
                arguments.push(
                    BuildArgumentEmitter::new()
                        .with_context(context)
                        .argument_with_allocator(argument, allocator),
                );
            }
        }
        if (method_name == context.runtime_names().repeat_each
            || method_name == context.runtime_names().repeat_template)
            && arguments.is_empty()
        {
            return Err(BuildLoweringError::UnsupportedChild);
        }
        Ok(ast.vec_from_iter(arguments))
    }

    fn repeat_arrow_function<'b>(
        &self,
        arrow: &ArrowFunctionExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<Expression<'b>, BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut statements = OxcVec::new_in(&allocator);
        let repeat_context = context.clone().with_inside_repeat();
        self.push_lazy_arrow_body(&mut statements, arrow, allocator, &repeat_context)?;
        let params = IterationFormalParameters::from_source(ast, allocator, &arrow.params);
        let body = ast.function_body(SPAN, ast.vec(), statements);
        Ok(ast.expression_arrow_function(
            arrow.span,
            false,
            arrow.r#async,
            arrow
                .type_parameters
                .as_ref()
                .map(|type_parameters| type_parameters.clone_in(allocator)),
            params,
            arrow
                .return_type
                .as_ref()
                .map(|return_type| return_type.clone_in(allocator)),
            body,
        ))
    }

    fn is_repeat_base_call(&self, call: &CallExpression<'_>, context: &BuildContext) -> bool {
        matches!(
            &call.callee,
            Expression::Identifier(identifier)
                if identifier.name.as_str() == context.runtime_names().component_repeat
        )
    }

    fn component_chain_attributes<'source>(
        &self,
        component: &'source ArkUIComponentExpression<'source>,
        context: &BuildContext,
    ) -> Result<Vec<LeafAttribute<'source>>, BuildLoweringError>
    where
        'a: 'source,
    {
        let mut attrs = Vec::new();
        for chain in &component.chain_expressions {
            let name = ArkuiCallName::chain(chain)?;
            if let Some(body) = self.plan.styles.body(&name) {
                attrs.extend(StylesAttributeList::from_body(body, context)?.attrs);
            } else {
                let gesture = context
                    .is_gesture_attribute_name(&name)
                    .then(|| {
                        chain
                            .arguments
                            .first()
                            .and_then(Argument::as_expression)
                            .and_then(|expression| {
                                if let Expression::CallExpression(call) = expression {
                                    GestureChain::from_call(call, context)
                                } else {
                                    None
                                }
                            })
                    })
                    .flatten();
                attrs.push(
                    LeafAttribute {
                        name,
                        arguments: chain.arguments.iter().collect(),
                        gesture,
                        state_styles: None,
                    }
                    .with_state_styles(&self.plan.styles, context)?,
                );
            }
        }
        Ok(attrs)
    }

    fn expand_leaf_attributes<'source>(
        &self,
        attrs: &[LeafAttribute<'source>],
        context: &BuildContext,
    ) -> Result<Vec<LeafAttribute<'source>>, BuildLoweringError>
    where
        'a: 'source,
    {
        let mut expanded = Vec::new();
        for attr in attrs {
            if let Some(body) = self.plan.styles.body(&attr.name) {
                expanded.extend(StylesAttributeList::from_body(body, context)?.attrs);
            } else {
                expanded.push(attr.clone().with_state_styles(&self.plan.styles, context)?);
            }
        }
        Ok(expanded)
    }

    fn push_for_each<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let Some(data_arg) = arguments.first() else {
            return Err(BuildLoweringError::UnsupportedChild);
        };
        let Some(item_gen_arg) = arguments.get(1) else {
            return Err(BuildLoweringError::UnsupportedChild);
        };
        let Some(item_gen) = IterationArgument::arrow(item_gen_arg) else {
            return Err(BuildLoweringError::UnsupportedChild);
        };

        let item_param = item_gen.params.items.first();
        let index_param = item_gen.params.items.get(1);
        let ast = AstBuilder::new(allocator);
        let mut item_body = OxcVec::new_in(&allocator);
        if let Some(item_param) = item_param {
            item_body.push(IterationAstStatement::item_binding_declaration(
                ast, allocator, item_param, context,
            ));
        }
        self.push_lazy_arrow_body(&mut item_body, item_gen, allocator, context)?;

        let mut observe_body = OxcVec::new_in(&allocator);
        observe_body.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_for_each,
            &context.runtime_names().method_create,
        ));
        observe_body.push(IterationAstStatement::const_arrow_function_with_params(
            ast,
            &context.runtime_names().foreach_item_gen_function,
            IterationFormalParameters::for_each_generator(
                ast,
                allocator,
                item_param,
                index_param,
                &context.runtime_names().item_binding,
            ),
            item_body,
        ));

        let data = IterationArgument::expression_with_context(data_arg, context, allocator)
            .ok_or(BuildLoweringError::UnsupportedStatement)?;
        let key = arguments
            .get(2)
            .map(|key_arg| {
                IterationArgument::expression_with_context(key_arg, context, allocator)
                    .ok_or(BuildLoweringError::UnsupportedStatement)
                    .map(|expression| ForEachKeyExpression {
                        expression,
                        uses_index: IterationArgument::arrow(key_arg)
                            .and_then(|arrow| arrow.params.items.get(1))
                            .is_some(),
                    })
            })
            .transpose()?;
        observe_body.push(IterationAstStatement::for_each_update(
            ast,
            allocator,
            context,
            data,
            item_param.is_some(),
            index_param.is_some(),
            key,
        ));
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &context.runtime_names().component_for_each,
                context.runtime_names(),
                observe_body,
            ),
        );
        statements.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_for_each,
            &context.runtime_names().method_pop,
        ));
        Ok(())
    }

    fn push_lazy_for_each<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        self.push_lazy_for_each_arguments(statements, &component.arguments, allocator, context)
    }

    fn push_lazy_for_each_arguments<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let Some(data_arg) = arguments.first() else {
            return Err(BuildLoweringError::UnsupportedChild);
        };
        let Some(item_gen_arg) = arguments.get(1) else {
            return Err(BuildLoweringError::UnsupportedChild);
        };
        let Some(item_gen) = IterationArgument::arrow(item_gen_arg) else {
            return Err(BuildLoweringError::UnsupportedChild);
        };

        let ast = AstBuilder::new(allocator);
        let mut item_body = OxcVec::new_in(&allocator);
        let item_param = item_gen.params.items.first();
        let index_param = item_gen.params.items.get(1);
        if let Some(item_param) = item_param {
            item_body.push(IterationAstStatement::item_binding_declaration(
                ast, allocator, item_param, context,
            ));
        }
        let lazy_context = context.clone().with_inside_lazy_for_each();
        self.push_lazy_arrow_body(&mut item_body, item_gen, allocator, &lazy_context)?;
        statements.push(IterationAstStatement::const_arrow_function_with_params(
            ast,
            &context.runtime_names().lazy_foreach_item_gen_function,
            IterationFormalParameters::for_each_generator(
                ast,
                allocator,
                item_param,
                index_param,
                &context.runtime_names().item_binding,
            ),
            item_body,
        ));
        let data = IterationArgument::expression_with_context(data_arg, context, allocator)
            .ok_or(BuildLoweringError::UnsupportedStatement)?;
        let has_id_func = if let Some(id_func_arg) = arguments.get(2) {
            let id_func =
                IterationArgument::expression_with_context(id_func_arg, context, allocator)
                    .ok_or(BuildLoweringError::UnsupportedStatement)?;
            statements.push(IterationAstStatement::const_declaration(
                ast,
                &context.runtime_names().lazy_foreach_item_id_func,
                id_func,
            ));
            true
        } else {
            false
        };
        statements.push(IterationAstStatement::lazy_for_each_create(
            ast,
            allocator,
            context,
            data,
            has_id_func,
        ));
        statements.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_lazy_for_each,
            &context.runtime_names().method_pop,
        ));
        Ok(())
    }

    fn push_lazy_list_item<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut block_body = OxcVec::new_in(&allocator);
        block_body.push(IterationAstStatement::lazy_list_item_creation(
            ast, allocator, component, context,
        ));
        block_body.push(
            IterationComponent::new(component).lazy_list_item_creation2_statement(
                ast,
                allocator,
                &self.plan.builder_methods,
                context,
            )?,
        );

        let mut deep_render_body = OxcVec::new_in(&allocator);
        deep_render_body.push(IterationAstStatement::item_creation_call(ast, context));
        for child in &component.children {
            self.push_child(&mut deep_render_body, child, allocator, context)?;
        }
        deep_render_body.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_pop,
        ));
        block_body.push(IterationAstStatement::const_arrow_function(
            ast,
            &context.runtime_names().deep_render_function,
            IterationFormalParameters::render_callback(ast, context),
            deep_render_body,
        ));

        block_body.push(IterationAstStatement::member_call(
            ast,
            allocator,
            IterationAstStatement::view_context(ast, context),
            &context.runtime_names().observe_component_creation2,
            ast.vec_from_iter([
                Argument::from(
                    ast.expression_identifier(
                        SPAN,
                        ast.str(&context.runtime_names().item_creation2),
                    ),
                ),
                Argument::from(ast.expression_identifier(
                    SPAN,
                    ast.str(&context.runtime_names().component_list_item),
                )),
            ]),
        ));
        block_body.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_pop,
        ));
        statements.push(ast.statement_block(SPAN, block_body));
        Ok(())
    }

    fn push_observed_item_component<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        component_name: &str,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut item_creation_body = OxcVec::new_in(&allocator);
        item_creation_body.push(IterationAstStatement::observed_item_create(
            ast,
            allocator,
            component_name,
            &component.arguments,
            context,
        ));
        let resolved_attrs = self.component_chain_attributes(component, context)?;
        LeafAttributeStatementEmitter::new(
            component_name,
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut item_creation_body, ast, allocator);

        let mut block_body = OxcVec::new_in(&allocator);
        block_body.push(IterationAstStatement::const_arrow_function(
            ast,
            &context.runtime_names().item_creation2,
            IterationFormalParameters::render_callback(ast, context),
            item_creation_body,
        ));

        let mut observed_body = OxcVec::new_in(&allocator);
        observed_body.push(IterationAstStatement::member_call(
            ast,
            allocator,
            IterationAstStatement::view_context(ast, context),
            &context.runtime_names().observe_component_creation2,
            ast.vec_from_iter([
                Argument::from(
                    ast.expression_identifier(
                        SPAN,
                        ast.str(&context.runtime_names().item_creation2),
                    ),
                ),
                Argument::from(ast.expression_identifier(SPAN, ast.str(component_name))),
            ]),
        ));
        for child in &component.children {
            self.push_child(&mut observed_body, child, allocator, context)?;
        }
        observed_body.push(ComponentCallStatementEmitter::empty(
            ast,
            component_name,
            &context.runtime_names().method_pop,
        ));
        block_body.push(IterationAstStatement::const_arrow_function(
            ast,
            &context.runtime_names().observed_deep_render,
            IterationFormalParameters::empty(ast),
            observed_body,
        ));
        block_body.push(IterationAstStatement::identifier_call(
            ast,
            &context.runtime_names().observed_deep_render,
        ));
        statements.push(ast.statement_block(SPAN, block_body));
        Ok(())
    }

    fn push_lazy_arrow_body<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        arrow: &oxc_ast::ast::ArrowFunctionExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        if let Some(expression) = arrow.get_expression() {
            self.push_expression(statements, expression, allocator, context)?;
            return Ok(());
        }
        for statement in &arrow.body.statements {
            self.push_statement(statements, statement, allocator, context)?;
        }
        Ok(())
    }

    fn push_child<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        child: &ArkUIChild<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        match child {
            ArkUIChild::Component(component) => {
                self.push_component(statements, component, allocator, context)
            }
            ArkUIChild::Expression(expression) => {
                self.push_expression(statements, expression, allocator, context)
            }
            ArkUIChild::Statement(statement) => {
                self.push_statement(statements, statement, allocator, context)
            }
        }
    }

    fn push_builder_method_call<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> bool {
        if let Some(name) =
            ArkuiCallName::builder_method_member(&call.callee, &self.plan.builder_methods)
        {
            statements.push(
                BuilderMethodCallEmitter::new(
                    BuilderMethodReceiver::This,
                    &name,
                    &call.arguments,
                    context,
                )
                .statement(AstBuilder::new(allocator), allocator),
            );
            return true;
        }
        if let Expression::Identifier(identifier) = &call.callee {
            let name = identifier.name.as_str();
            if self.plan.builder_methods.contains(name)
                && !self.plan.custom_components.contains(name)
            {
                statements.push(
                    BuilderMethodCallEmitter::new(
                        BuilderMethodReceiver::Direct,
                        name,
                        &call.arguments,
                        context,
                    )
                    .statement(AstBuilder::new(allocator), allocator),
                );
                return true;
            }
        }
        false
    }

    fn push_wrapped_builder_call<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> bool {
        let emitter = BuilderDispatchCallEmitter::new(call, context);
        if !emitter.matches() {
            return false;
        }
        let expression = emitter.expression(allocator);
        statements.push(AstBuilder::new(allocator).statement_expression(SPAN, expression));
        true
    }

    fn push_wrapped_builder_chain_call<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        chain: &ChainExpression<'_>,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> bool {
        let emitter = BuilderDispatchCallEmitter::new(call, context);
        if !emitter.matches() {
            return false;
        }
        let mut cloned_chain = chain.clone_in(allocator);
        let ChainElement::CallExpression(cloned_call) = &mut cloned_chain.expression else {
            return false;
        };
        emitter.rewrite(cloned_call, allocator);
        let cloned_chain = AstBuilder::new(allocator).alloc(cloned_chain);
        let expression = BuildArgumentEmitter::new()
            .with_context(context)
            .owned_expression_with_allocator(Expression::ChainExpression(cloned_chain), allocator);
        statements.push(AstBuilder::new(allocator).statement_expression(SPAN, expression));
        true
    }

    fn push_if_statement<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        statement: &IfStatement<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = AstBuilder::new(allocator);
        let mut observe_stmts = OxcVec::new_in(&allocator);
        observe_stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_if,
            &context.runtime_names().method_create,
        ));
        observe_stmts.push(self.if_chain_statement(ast, allocator, statement, context, 0)?);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &context.runtime_names().component_if,
                context.runtime_names(),
                observe_stmts,
            ),
        );
        statements.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_if,
            &context.runtime_names().method_pop,
        ));
        Ok(())
    }

    fn if_chain_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        statement: &IfStatement<'_>,
        context: &BuildContext,
        branch_index: usize,
    ) -> Result<Statement<'b>, BuildLoweringError> {
        let consequent = self.if_branch_update_block(
            ast,
            allocator,
            &statement.consequent,
            context,
            branch_index,
        )?;
        let alternate = match &statement.alternate {
            Some(Statement::IfStatement(alternate)) => Some(self.if_chain_statement(
                ast,
                allocator,
                alternate,
                context,
                branch_index + 1,
            )?),
            Some(alternate) => Some(self.if_branch_update_block(
                ast,
                allocator,
                alternate,
                context,
                branch_index + 1,
            )?),
            None => Some(self.empty_if_branch_update_block(ast, branch_index + 1, context)),
        };
        Ok(ast.statement_if(
            SPAN,
            BuildArgumentEmitter::new()
                .with_context(context)
                .with_nested_raw_object_rewrite()
                .expression_with_allocator(&statement.test, allocator),
            consequent,
            alternate,
        ))
    }

    fn if_branch_update_block<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        source_statement: &Statement<'_>,
        context: &BuildContext,
        branch_index: usize,
    ) -> Result<Statement<'b>, BuildLoweringError> {
        let mut branch_stmts = OxcVec::new_in(&allocator);
        self.push_branch_statement(&mut branch_stmts, source_statement, allocator, context)?;
        Ok(ast.statement_block(
            SPAN,
            ast.vec1(self.if_else_branch_update_statement(
                ast,
                branch_index,
                branch_stmts,
                context,
            )),
        ))
    }

    fn empty_if_branch_update_block<'b>(
        &self,
        ast: AstBuilder<'b>,
        branch_index: usize,
        context: &BuildContext,
    ) -> Statement<'b> {
        // ets-loader's processElseStatement synthesizes the terminal empty
        // branch without forwarding createConditionParent(). In a top-level
        // @Builder this deliberately keeps the lexical `this` receiver even
        // though source-backed branches use `(parent ? parent : this)`.
        let mut lexical_context = context.clone();
        lexical_context.use_parent_context = false;
        ast.statement_block(
            SPAN,
            ast.vec1(self.if_else_branch_update_statement(
                ast,
                branch_index,
                ast.vec(),
                &lexical_context,
            )),
        )
    }

    fn if_else_branch_update_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        branch_index: usize,
        body_stmts: OxcVec<'b, Statement<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let body = ast.function_body(SPAN, ast.vec(), body_stmts);
        let arrow = ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            ast.formal_parameters(
                SPAN,
                FormalParameterKind::ArrowFormalParameters,
                ast.vec(),
                NONE,
            ),
            NONE,
            body,
        );
        let callee = ast.member_expression_static(
            SPAN,
            self.view_context_expression(ast, context),
            ast.identifier_name(
                SPAN,
                ast.str(&context.runtime_names().if_else_branch_update_function),
            ),
            false,
        );
        let call = ast.expression_call(
            SPAN,
            Expression::from(callee),
            NONE,
            ast.vec_from_iter([
                Argument::from(ast.expression_numeric_literal(
                    SPAN,
                    branch_index as f64,
                    None,
                    oxc_ast::ast::NumberBase::Decimal,
                )),
                Argument::from(arrow),
            ]),
            false,
        );
        ast.statement_expression(SPAN, call)
    }

    fn view_context_expression<'b>(
        &self,
        ast: AstBuilder<'b>,
        context: &BuildContext,
    ) -> Expression<'b> {
        if !context.use_parent_context {
            return ast.expression_this(SPAN);
        }
        let test = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let consequent = ast.expression_identifier(SPAN, GeneratedViewpuName::PARENT);
        let alternate = ast.expression_this(SPAN);
        ast.expression_parenthesized(
            SPAN,
            ast.expression_conditional(SPAN, test, consequent, alternate),
        )
    }
}

impl BuildContext {
    fn builder() -> Self {
        Self {
            // SDK ets-loader keeps the `parent = null` parameter on @Builder methods,
            // but `addCustomComponentId` only switches component receivers to
            // parentConditionalExpression() for @LocalBuilder/reuse cases.
            use_parent_context: false,
            builder_proxy_scope: BuilderProxyScope::Builder,
            ..Default::default()
        }
    }

    fn global_builder(first_parameter: Option<Arc<str>>) -> Self {
        Self {
            // SDK process_ui_syntax.js invokes processComponentBlock for a top-level
            // @Builder with the global-builder flag. createConditionParent() then
            // selects `(parent ? parent : this)` for every component receiver.
            use_parent_context: true,
            global_builder_parameter: first_parameter,
            builder_proxy_scope: BuilderProxyScope::GlobalBuilder,
            ..Default::default()
        }
    }

    fn with_raw_object_fields(mut self, raw_object_fields: &FxHashSet<String>) -> Self {
        self.raw_object_fields = Arc::new(raw_object_fields.clone());
        self
    }

    fn with_shared_component_metadata(
        mut self,
        component_metadata: Arc<ComponentMetadata>,
    ) -> Self {
        self.component_metadata = component_metadata;
        self
    }

    fn with_shared_extend_attributes(
        mut self,
        extend_attributes: Arc<ExtendAttributeIndex>,
    ) -> Self {
        self.extend_attributes = extend_attributes;
        self
    }

    fn with_module_name(mut self, module_name: &str) -> Self {
        self.module_name = Arc::from(module_name);
        self
    }

    fn with_xcomponent_module_identity(mut self, identity: Option<Arc<str>>) -> Self {
        self.xcomponent_module_identity = identity;
        self
    }

    fn with_current_component_reusable(mut self, reusable: bool) -> Self {
        self.current_component_reusable = reusable;
        self
    }

    fn with_inside_lazy_for_each(mut self) -> Self {
        self.inside_lazy_for_each = true;
        self
    }

    fn with_inside_repeat(mut self) -> Self {
        self.inside_repeat = true;
        self
    }

    fn with_builder_param_body(mut self) -> Self {
        self.use_parent_context = false;
        self.global_builder_parameter = None;
        self.builder_proxy_scope = BuilderProxyScope::Component;
        self
    }

    fn with_custom_component_base_name(mut self, base_name: &'static str) -> Self {
        self.custom_component_base_name = base_name;
        self
    }

    fn custom_component_base_name(&self) -> &'static str {
        self.custom_component_base_name
    }

    fn is_raw_object_field(&self, name: &str) -> bool {
        self.raw_object_fields.contains(name)
    }

    fn is_builder_attribute_method(&self, component_name: &str, method_name: &str) -> bool {
        self.component_metadata
            .is_builder_attribute_method(component_name, method_name)
    }

    fn is_extend_attribute(&self, component_name: &str, attr_name: &str) -> bool {
        self.extend_attributes.contains(component_name, attr_name)
    }

    fn is_create_bind_component(&self, component_name: &str) -> bool {
        self.component_metadata
            .is_create_bind_component(component_name)
    }

    fn is_builder_attribute_without_key(&self, attr_name: &str) -> bool {
        self.component_metadata
            .is_builder_attribute_without_key(attr_name)
    }

    fn is_style_double_dollar_attr(&self, attr_name: &str) -> bool {
        self.component_metadata
            .is_style_double_dollar_attr(attr_name)
    }

    fn is_component_double_dollar_property(&self, component_name: &str, attr_name: &str) -> bool {
        self.component_metadata
            .is_component_double_dollar_property(component_name, attr_name)
    }

    fn is_component_double_dollar_property_name(
        &self,
        component_name: &str,
        property_name: &str,
    ) -> bool {
        self.component_metadata
            .is_component_double_dollar_property_name(component_name, property_name)
    }

    fn is_bind_popup_attr(&self, attr_name: &str) -> bool {
        self.component_metadata.is_bind_popup_attr(attr_name)
    }

    fn is_bind_drag_attr(&self, attr_name: &str) -> bool {
        self.component_metadata.is_bind_drag_attr(attr_name)
    }

    fn is_gesture_type_name(&self, name: &str) -> bool {
        self.component_metadata.is_gesture_type_name(name)
    }

    fn is_gesture_attribute_name(&self, name: &str) -> bool {
        self.component_metadata.is_gesture_attribute_name(name)
    }

    fn is_reuse_id_attribute_name(&self, name: &str) -> bool {
        name == self.runtime_names().reuse_id
    }

    fn component_kind(&self, name: &str) -> ArkuiRuntimeComponentKind {
        self.runtime_names().component_kind(name)
    }

    fn component_needs_pop<'a>(
        &self,
        name: &str,
        arguments: impl IntoIterator<Item = &'a Argument<'a>>,
    ) -> bool {
        if self.component_kind(name) != ArkuiRuntimeComponentKind::XComponent {
            return self.component_metadata.needs_pop(name);
        }
        XComponentCreateContract::new(arguments.into_iter().next(), self.runtime_names())
            .is_container()
    }

    fn call_matches_component_kind(
        &self,
        call: &CallExpression<'_>,
        expected: ArkuiRuntimeComponentKind,
    ) -> bool {
        match &call.callee {
            Expression::Identifier(identifier) => {
                self.component_kind(identifier.name.as_str()) == expected
            }
            _ => false,
        }
    }

    fn create_method_name(
        &self,
        component_kind: ArkuiRuntimeComponentKind,
        has_children: bool,
    ) -> &str {
        let runtime_names = self.runtime_names();
        if matches!(component_kind, ArkuiRuntimeComponentKind::Button) {
            if has_children {
                &runtime_names.method_create_with_child
            } else {
                &runtime_names.method_create_with_label
            }
        } else {
            &runtime_names.method_create
        }
    }

    fn runtime_names(&self) -> &ArkuiRuntimeNames {
        self.component_metadata.runtime_names()
    }
}

/// Mirrors ets-loader `checkContainer` / `isXComponentContainer`: an XComponent
/// participates in the container stack only when its source `type` is statically
/// the component variant.
struct XComponentCreateContract<'a> {
    first_argument: Option<&'a Argument<'a>>,
    runtime_names: &'a ArkuiRuntimeNames,
}

impl<'a> XComponentCreateContract<'a> {
    const TYPE_PROPERTY: &'static str = "type";
    const COMPONENT_STRING: &'static str = "component";
    const COMPONENT_NUMBER: f64 = 1.0;

    fn new(first_argument: Option<&'a Argument<'a>>, runtime_names: &'a ArkuiRuntimeNames) -> Self {
        Self {
            first_argument,
            runtime_names,
        }
    }

    fn is_container(&self) -> bool {
        let Some(Expression::ObjectExpression(object)) =
            self.first_argument.and_then(Argument::as_expression)
        else {
            return false;
        };
        object.properties.iter().any(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            property.key.static_name().is_some_and(|name| {
                name == Self::TYPE_PROPERTY && self.is_container_type(&property.value)
            })
        })
    }

    fn is_container_type(&self, value: &Expression<'_>) -> bool {
        match value {
            Expression::StringLiteral(value) => value.value == Self::COMPONENT_STRING,
            Expression::NumericLiteral(value) => value.value == Self::COMPONENT_NUMBER,
            Expression::StaticMemberExpression(member) => {
                matches!(
                    &member.object,
                    Expression::Identifier(object)
                        if object.name.as_str() == self.runtime_names.xcomponent_type
                ) && member.property.name.as_str() == self.runtime_names.xcomponent_type_container
            }
            _ => false,
        }
    }
}

/// Extracted representation of a gesture call and its attribute chain.
/// Used to emit `Gesture.create(priority)` / `XxxGesture.create(args)` /
/// `XxxGesture.onX(...)` / `XxxGesture.pop()` / `Gesture.pop()` sequences.
///
/// For `GestureGroup`, `create_args` holds only the first argument (the
/// `GestureMode`) and `children` holds the expanded child gesture chains.
#[derive(Clone)]
struct GestureChain<'a> {
    gesture_type: String,
    create_args: Vec<&'a Argument<'a>>,
    attrs: Vec<(String, Vec<&'a Argument<'a>>)>,
    /// Child gestures for `GestureGroup(mode, child1, child2, ...)`.
    /// Empty for all other gesture types.
    children: Vec<GestureChain<'a>>,
}

impl<'a> GestureChain<'a> {
    fn from_call(call: &'a CallExpression<'a>, context: &BuildContext) -> Option<Self> {
        match &call.callee {
            Expression::Identifier(_) => {
                let name = match ArkuiCallName::component(&call.callee) {
                    Ok(n) => n,
                    Err(_) => return None,
                };
                if !context.is_gesture_type_name(&name) {
                    return None;
                }
                // GestureGroup: first arg is the GestureMode; remaining args are
                // child gesture expressions that must be expanded recursively.
                if context.component_kind(&name).is_gesture_group() {
                    let create_args = call.arguments.first().into_iter().collect();
                    let children = call
                        .arguments
                        .iter()
                        .skip(1)
                        .filter_map(|arg| arg.as_expression())
                        .filter_map(|expr| {
                            if let Expression::CallExpression(child_call) = expr {
                                Self::from_call(child_call, context)
                            } else {
                                None
                            }
                        })
                        .collect();
                    return Some(Self {
                        gesture_type: name,
                        create_args,
                        attrs: vec![],
                        children,
                    });
                }
                let create_args = call.arguments.iter().collect();
                Some(Self {
                    gesture_type: name,
                    create_args,
                    attrs: vec![],
                    children: vec![],
                })
            }
            Expression::StaticMemberExpression(member) => {
                let attr_name = member.property.name.to_string();
                match &member.object {
                    Expression::CallExpression(inner) => {
                        let mut chain = Self::from_call(inner, context)?;
                        chain
                            .attrs
                            .push((attr_name, call.arguments.iter().collect()));
                        Some(chain)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn push_sequence_statements<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) {
        stmts.push(ComponentCallStatementEmitter::gesture_low_priority(
            ast, context,
        ));
        self.push_node_statements(stmts, ast, allocator, context);
        stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_gesture,
            &context.runtime_names().method_pop,
        ));
    }

    fn push_node_statements<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) {
        stmts.push(ComponentCallStatementEmitter::argument_refs(
            ast,
            allocator,
            &self.gesture_type,
            &context.runtime_names().method_create,
            &self.create_args,
            context,
        ));
        for (attr_name, attr_args) in &self.attrs {
            stmts.push(ComponentCallStatementEmitter::argument_refs(
                ast,
                allocator,
                &self.gesture_type,
                attr_name,
                attr_args,
                context,
            ));
        }
        for child in &self.children {
            child.push_node_statements(stmts, ast, allocator, context);
        }
        stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &self.gesture_type,
            &context.runtime_names().method_pop,
        ));
    }
}

/// Extracted representation of a leaf component call and its attribute chain.
struct LeafComponent<'a> {
    name: String,
    create_arguments: Vec<&'a Argument<'a>>,
    chain_attrs: Vec<LeafAttribute<'a>>,
}

#[derive(Clone)]
struct LeafAttribute<'a> {
    name: String,
    arguments: Vec<&'a Argument<'a>>,
    gesture: Option<GestureChain<'a>>,
    state_styles: Option<StateStylesAttributeList<'a>>,
}

impl<'a> LeafAttribute<'a> {
    fn from_call(name: String, call: &'a CallExpression<'a>, context: &BuildContext) -> Self {
        let gesture = context
            .is_gesture_attribute_name(&name)
            .then(|| {
                call.arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|expression| {
                        if let Expression::CallExpression(gesture_call) = expression {
                            GestureChain::from_call(gesture_call, context)
                        } else {
                            None
                        }
                    })
            })
            .flatten();
        Self {
            name,
            arguments: call.arguments.iter().collect(),
            gesture,
            state_styles: None,
        }
    }

    fn with_state_styles(
        mut self,
        styles: &StylesAttributeIndex<'a>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        if self.name == context.runtime_names().attribute_state_styles {
            self.state_styles = Some(StateStylesAttributeList::from_arguments(
                &self.arguments,
                styles,
                context,
            )?);
        }
        Ok(self)
    }
}

#[derive(Clone)]
struct StateStyle<'a> {
    name: String,
    attrs: Vec<LeafAttribute<'a>>,
}

#[derive(Clone)]
struct StateStylesAttributeList<'a> {
    states: Vec<StateStyle<'a>>,
}

impl<'a> StateStylesAttributeList<'a> {
    fn from_arguments(
        arguments: &[&'a Argument<'a>],
        styles: &StylesAttributeIndex<'a>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        let [argument] = arguments else {
            return Err(BuildLoweringError::InvalidStateStyles);
        };
        let Some(Expression::ObjectExpression(object)) = argument.as_expression() else {
            return Err(BuildLoweringError::InvalidStateStyles);
        };
        let mut states = Vec::with_capacity(object.properties.len());
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return Err(BuildLoweringError::InvalidStateStyles);
            };
            let Some(name) = property.key.static_name() else {
                return Err(BuildLoweringError::InvalidStateStyles);
            };
            states.push(StateStyle {
                name: name.into_owned(),
                attrs: Self::state_attributes(&property.value, styles, context)?,
            });
        }
        Ok(Self { states })
    }

    fn state_attributes(
        value: &'a Expression<'a>,
        styles: &StylesAttributeIndex<'a>,
        context: &BuildContext,
    ) -> Result<Vec<LeafAttribute<'a>>, BuildLoweringError> {
        let style_name = match value {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(member)
                if matches!(&member.object, Expression::ThisExpression(_)) =>
            {
                Some(member.property.name.as_str())
            }
            _ => None,
        };
        if let Some(style_name) = style_name {
            let Some(body) = styles.body(style_name) else {
                return Err(BuildLoweringError::InvalidStateStyles);
            };
            return Ok(StylesAttributeList::from_body(body, context)?.attrs);
        }

        let Expression::ObjectExpression(object) = value else {
            return Err(BuildLoweringError::InvalidStateStyles);
        };
        if object.properties.is_empty() {
            return Ok(Vec::new());
        }
        let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
            return Err(BuildLoweringError::InvalidStateStyles);
        };
        StylesAttributeList::from_expression(&property.value, context).map(|styles| styles.attrs)
    }
}

struct StylesAttributeList<'a> {
    attrs: Vec<LeafAttribute<'a>>,
}

impl<'a> StylesAttributeList<'a> {
    fn from_body(
        body: Option<&'a FunctionBody<'a>>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        let Some(body) = body else {
            return Ok(Self { attrs: Vec::new() });
        };
        let mut attrs = Vec::new();
        for statement in &body.statements {
            let Statement::ExpressionStatement(statement) = statement else {
                return Err(BuildLoweringError::UnsupportedStatement);
            };
            let expression = match &statement.expression {
                Expression::LeadingDotExpression(expression) => &expression.expression,
                expression => expression,
            };
            let Expression::CallExpression(call) = expression else {
                return Err(BuildLoweringError::UnsupportedChain);
            };
            Self::push_call_attributes(&mut attrs, call, context)?;
        }
        Ok(Self { attrs })
    }

    fn from_expression(
        expression: &'a Expression<'a>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        let expression = match expression {
            Expression::LeadingDotExpression(expression) => &expression.expression,
            expression => expression,
        };
        let Expression::CallExpression(call) = expression else {
            return Err(BuildLoweringError::InvalidStateStyles);
        };
        let mut attrs = Vec::new();
        Self::push_call_attributes(&mut attrs, call, context)?;
        Ok(Self { attrs })
    }

    fn push_call_attributes(
        attrs: &mut Vec<LeafAttribute<'a>>,
        call: &'a CallExpression<'a>,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        match &call.callee {
            Expression::Identifier(identifier) => {
                attrs.push(LeafAttribute::from_call(
                    identifier.name.to_string(),
                    call,
                    context,
                ));
                Ok(())
            }
            Expression::StaticMemberExpression(member) => {
                let Expression::CallExpression(inner) = &member.object else {
                    return Err(BuildLoweringError::UnsupportedChain);
                };
                Self::push_call_attributes(attrs, inner, context)?;
                attrs.push(LeafAttribute::from_call(
                    member.property.name.to_string(),
                    call,
                    context,
                ));
                Ok(())
            }
            _ => Err(BuildLoweringError::UnsupportedChain),
        }
    }
}

impl<'a> LeafComponent<'a> {
    fn from_call(
        call: &'a CallExpression<'a>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        match &call.callee {
            Expression::Identifier(_) => {
                let name = ArkuiCallName::component(&call.callee)?;
                Ok(Self {
                    name,
                    create_arguments: call.arguments.iter().collect(),
                    chain_attrs: vec![],
                })
            }
            Expression::StaticMemberExpression(member) => {
                let attr_name = member.property.name.to_string();
                match &member.object {
                    Expression::CallExpression(inner) => {
                        let mut leaf = Self::from_call(inner, context)?;
                        leaf.chain_attrs
                            .push(LeafAttribute::from_call(attr_name, call, context));
                        Ok(leaf)
                    }
                    _ => Err(BuildLoweringError::UnsupportedCallee),
                }
            }
            _ => Err(BuildLoweringError::UnsupportedCallee),
        }
    }

    fn base_call(call: &'a CallExpression<'a>) -> &'a CallExpression<'a> {
        let mut current = call;
        while let Expression::StaticMemberExpression(member) = &current.callee {
            let Expression::CallExpression(inner) = &member.object else {
                break;
            };
            current = inner;
        }
        current
    }
}

struct LeafAttributeStatementEmitter<'a, 'ctx> {
    component_name: &'a str,
    attrs: &'a [LeafAttribute<'a>],
    builder_methods: &'a FxHashSet<String>,
    context: &'ctx BuildContext,
    use_reusable_immutable_guard: bool,
}

impl<'a, 'ctx> LeafAttributeStatementEmitter<'a, 'ctx> {
    fn new(
        component_name: &'a str,
        attrs: &'a [LeafAttribute<'a>],
        builder_methods: &'a FxHashSet<String>,
        context: &'ctx BuildContext,
    ) -> Self {
        Self {
            component_name,
            attrs,
            builder_methods,
            context,
            use_reusable_immutable_guard: true,
        }
    }

    fn without_reusable_immutable_guard(mut self) -> Self {
        self.use_reusable_immutable_guard = false;
        self
    }

    fn push_statements<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) {
        if self
            .attrs
            .iter()
            .any(|attr| ComponentCallStatementEmitter::is_animation_attribute(&attr.name))
        {
            let mut animation_segment = OxcVec::new_in(&allocator);
            let last_animation_index = self.attrs.iter().rposition(|attr| {
                ComponentCallStatementEmitter::is_animation_attribute(&attr.name)
            });
            let mut has_preceding_attribute = false;
            for (index, attr) in self.attrs.iter().enumerate() {
                if ComponentCallStatementEmitter::is_animation_attribute(&attr.name) {
                    let is_null_animation =
                        ComponentCallStatementEmitter::is_null_animation_refs(&attr.arguments);
                    ComponentCallStatementEmitter::push_animation_ref_segment(
                        stmts,
                        &mut animation_segment,
                        ast,
                        allocator,
                        &attr.arguments,
                        self.context,
                        has_preceding_attribute,
                        !is_null_animation || last_animation_index.is_some_and(|last| index < last),
                    );
                    has_preceding_attribute = true;
                    continue;
                }
                if let Some(state_styles) = &attr.state_styles {
                    self.push_state_styles(&mut animation_segment, state_styles, ast, allocator);
                    has_preceding_attribute = true;
                    continue;
                }
                if let Some(gesture) = &attr.gesture {
                    gesture.push_sequence_statements(
                        &mut animation_segment,
                        ast,
                        allocator,
                        self.context,
                    );
                    has_preceding_attribute = true;
                    continue;
                }
                animation_segment.push(ComponentCallStatementEmitter::attribute_refs(
                    ast,
                    allocator,
                    self.component_name,
                    &attr.name,
                    &attr.arguments,
                    self.builder_methods,
                    self.context,
                ));
                has_preceding_attribute = true;
            }
            ComponentCallStatementEmitter::flush_animation_segment(
                stmts,
                &mut animation_segment,
                allocator,
            );
            return;
        }
        let mut immutable_stmts = OxcVec::new_in(&allocator);
        for attr in self.attrs {
            if let Some(state_styles) = &attr.state_styles {
                self.push_state_styles(stmts, state_styles, ast, allocator);
                continue;
            }
            if let Some(gesture) = &attr.gesture {
                gesture.push_sequence_statements(stmts, ast, allocator, self.context);
                continue;
            }
            let statement = ComponentCallStatementEmitter::attribute_refs(
                ast,
                allocator,
                self.component_name,
                &attr.name,
                &attr.arguments,
                self.builder_methods,
                self.context,
            );
            if self.use_reusable_immutable_guard
                && self.context.current_component_reusable
                && RegularAttributeExpression::new(&self.context.component_metadata)
                    .all_argument_refs(&attr.arguments)
            {
                immutable_stmts.push(statement);
            } else {
                stmts.push(statement);
            }
        }
        if !immutable_stmts.is_empty() {
            stmts.push(ast.statement_if(
                SPAN,
                ast.expression_identifier(
                    SPAN,
                    ast.str(&self.context.runtime_names().is_initial_render),
                ),
                ast.statement_block(SPAN, immutable_stmts),
                None,
            ));
        }
    }

    fn push_state_styles<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        state_styles: &StateStylesAttributeList<'a>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) {
        for state in &state_styles.states {
            stmts.push(self.visual_state_statement(ast, Some(&state.name)));
            for attr in &state.attrs {
                if let Some(gesture) = &attr.gesture {
                    gesture.push_sequence_statements(stmts, ast, allocator, self.context);
                    continue;
                }
                stmts.push(ComponentCallStatementEmitter::attribute_refs(
                    ast,
                    allocator,
                    self.component_name,
                    &attr.name,
                    &attr.arguments,
                    self.builder_methods,
                    self.context,
                ));
            }
        }
        stmts.push(self.visual_state_statement(ast, None));
    }

    fn visual_state_statement<'b>(
        &self,
        ast: AstBuilder<'b>,
        state: Option<&str>,
    ) -> Statement<'b> {
        let runtime_names = self.context.runtime_names();
        let callee = ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, ast.str(&runtime_names.view_stack_processor)),
            ast.identifier_name(SPAN, ast.str(&runtime_names.visual_state)),
            false,
        );
        let arguments = state.map_or_else(
            || ast.vec(),
            |state| {
                ast.vec1(Argument::from(ast.expression_string_literal(
                    SPAN,
                    ast.str(state),
                    None,
                )))
            },
        );
        ast.statement_expression(
            SPAN,
            ast.expression_call(SPAN, Expression::from(callee), NONE, arguments, false),
        )
    }
}

struct RegularAttributeExpression<'a> {
    component_metadata: &'a ComponentMetadata,
}

impl<'a> RegularAttributeExpression<'a> {
    fn new(component_metadata: &'a ComponentMetadata) -> Self {
        Self { component_metadata }
    }

    fn all_argument_refs(&self, arguments: &[&Argument<'_>]) -> bool {
        arguments.iter().all(|argument| {
            argument
                .as_expression()
                .is_some_and(|expression| self.matches_expression(expression))
        })
    }

    fn matches_expression(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::ObjectExpression(object) => object.properties.iter().all(|property| {
                matches!(
                    property,
                    ObjectPropertyKind::ObjectProperty(property)
                        if self.matches_expression(&property.value)
                )
            }),
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
                element
                    .as_expression()
                    .is_some_and(|expression| self.matches_expression(expression))
            }),
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::ArrowFunctionExpression(_) => true,
            Expression::StaticMemberExpression(member) => {
                matches!(
                    &member.object,
                    Expression::Identifier(identifier)
                        if self.component_metadata.is_known_enum(identifier.name.as_str())
                )
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.matches_expression(&parenthesized.expression)
            }
            _ => false,
        }
    }
}

pub(super) struct ArkuiCallName;

impl ArkuiCallName {
    fn component(callee: &Expression<'_>) -> Result<String, BuildLoweringError> {
        match callee {
            Expression::Identifier(identifier) => Ok(identifier.name.to_string()),
            _ => Err(BuildLoweringError::UnsupportedCallee),
        }
    }

    pub(super) fn chain(call: &CallExpression<'_>) -> Result<String, BuildLoweringError> {
        match &call.callee {
            Expression::StaticMemberExpression(member) => Ok(member.property.name.to_string()),
            _ => Err(BuildLoweringError::UnsupportedChain),
        }
    }

    fn builder_method_member(
        expression: &Expression<'_>,
        builder_methods: &FxHashSet<String>,
    ) -> Option<String> {
        let name = Self::this_member(expression)?;
        builder_methods.contains(name.as_str()).then_some(name)
    }

    fn this_member(expression: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = expression else {
            return None;
        };
        if !matches!(&member.object, Expression::ThisExpression(_)) {
            return None;
        }
        Some(member.property.name.to_string())
    }

    fn is_builder_dispatch_call(call: &CallExpression<'_>, context: &BuildContext) -> bool {
        matches!(
            &call.callee,
            Expression::StaticMemberExpression(member)
                if member.property.name.as_str()
                    == context.runtime_names().builder_property.as_str()
        )
    }
}

struct AttributeArgumentEmitter<'a, 'ctx> {
    component_name: &'a str,
    attr_name: &'a str,
    builder_methods: &'a FxHashSet<String>,
    context: Option<&'ctx BuildContext>,
}

impl<'a, 'ctx> AttributeArgumentEmitter<'a, 'ctx> {
    fn new(
        component_name: &'a str,
        attr_name: &'a str,
        builder_methods: &'a FxHashSet<String>,
        context: Option<&'ctx BuildContext>,
    ) -> Self {
        Self {
            component_name,
            attr_name,
            builder_methods,
            context,
        }
    }

    fn arguments<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        if let Some(context) = self.context {
            return AttributeBindingArgumentEmitter::new(
                self.component_name,
                self.attr_name,
                self.builder_methods,
                context,
            )
            .arguments(arguments, allocator);
        }
        BuildArgumentEmitter::new()
            .with_optional_context(self.context)
            .with_nested_raw_object_rewrite()
            .arguments_with_allocator(arguments, allocator)
    }

    fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        if let Some(context) = self.context {
            return AttributeBindingArgumentEmitter::new(
                self.component_name,
                self.attr_name,
                self.builder_methods,
                context,
            )
            .argument_refs(arguments, allocator);
        }
        BuildArgumentEmitter::new()
            .with_optional_context(self.context)
            .with_nested_raw_object_rewrite()
            .argument_refs_with_allocator(arguments, allocator)
    }
}

struct AttributeBindingArgumentEmitter<'a, 'ctx> {
    component_name: &'a str,
    attr_name: &'a str,
    builder_methods: &'a FxHashSet<String>,
    context: &'ctx BuildContext,
}

impl<'a, 'ctx> AttributeBindingArgumentEmitter<'a, 'ctx> {
    fn new(
        component_name: &'a str,
        attr_name: &'a str,
        builder_methods: &'a FxHashSet<String>,
        context: &'ctx BuildContext,
    ) -> Self {
        Self {
            component_name,
            attr_name,
            builder_methods,
            context,
        }
    }

    fn arguments<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let processed = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| self.argument(argument, index, allocator))
            .collect();
        self.double_dollar_arguments(processed, allocator)
    }

    fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let processed = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| self.argument(argument, index, allocator))
            .collect();
        self.double_dollar_arguments(processed, allocator)
    }

    fn argument<'b>(
        &self,
        argument: &Argument<'_>,
        index: usize,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        let Some(expression) = argument.as_expression() else {
            return self.plain_argument(argument, allocator);
        };
        if self.context.is_bind_popup_attr(self.attr_name) {
            if index == 1 {
                return Argument::from(self.bind_popup_argument_expression(expression, allocator));
            }
            return self.plain_argument(argument, allocator);
        }
        if self.context.is_bind_drag_attr(self.attr_name) {
            if index == 0 {
                if let Some(expression) = self.drag_argument_expression(expression, allocator) {
                    return Argument::from(expression);
                }
            }
            return self.plain_argument(argument, allocator);
        }
        if self
            .context
            .is_builder_attribute_method(self.component_name, self.attr_name)
        {
            if let Some(expression) = self.builder_attribute_expression(expression, allocator) {
                return Argument::from(expression);
            }
        }
        self.plain_argument(argument, allocator)
    }

    fn double_dollar_arguments<'b>(
        &self,
        arguments: Vec<Argument<'b>>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let should_rewrite_style =
            self.context.is_style_double_dollar_attr(self.attr_name) && !arguments.is_empty();
        let should_rewrite_component = self
            .context
            .is_component_double_dollar_property(self.component_name, self.attr_name)
            && arguments.len() == 1;
        if !should_rewrite_style && !should_rewrite_component {
            return AstBuilder::new(allocator).vec_from_iter(arguments);
        }
        DoubleDollarBindingEmitter::new(self.context).attribute_arguments(
            arguments,
            should_rewrite_style,
            allocator,
        )
    }

    fn bind_popup_argument_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let Expression::ObjectExpression(object) = expression else {
            return self.plain_expression(expression, allocator);
        };
        let ast = AstBuilder::new(allocator);
        let runtime_names = self.context.runtime_names();
        let properties = ast.vec_from_iter(object.properties.iter().map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return self.plain_object_property(property, allocator);
            };
            if property
                .key
                .static_name()
                .is_some_and(|name| name == runtime_names.custom_dialog_controller_builder)
            {
                return ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    self.builder_initializer_expression(
                        &property.value,
                        &runtime_names.custom_dialog_controller_builder,
                        allocator,
                    ),
                    false,
                    false,
                    property.computed,
                );
            }
            self.plain_regular_object_property(property, allocator)
        }));
        ast.expression_object(object.span, properties)
    }

    fn drag_argument_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        let Expression::ArrowFunctionExpression(arrow) = expression else {
            return None;
        };
        if arrow.get_expression().is_some() {
            return None;
        }
        let ast = AstBuilder::new(allocator);
        let statements = ast.vec_from_iter(
            arrow
                .body
                .statements
                .iter()
                .map(|statement| self.drag_statement(statement, allocator)),
        );
        let body = ast.function_body(
            arrow.body.span,
            arrow.body.directives.clone_in(allocator),
            statements,
        );
        Some(
            ast.expression_arrow_function(
                arrow.span,
                arrow.expression,
                arrow.r#async,
                arrow
                    .type_parameters
                    .as_ref()
                    .map(|type_parameters| type_parameters.clone_in(allocator)),
                arrow.params.clone_in(allocator),
                arrow
                    .return_type
                    .as_ref()
                    .map(|return_type| return_type.clone_in(allocator)),
                body,
            ),
        )
    }

    fn drag_statement<'b>(
        &self,
        statement: &Statement<'_>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        let Statement::ReturnStatement(return_statement) = statement else {
            return statement.clone_in(allocator);
        };
        let argument = return_statement
            .argument
            .as_ref()
            .map(|expression| self.drag_return_expression(expression, allocator));
        AstBuilder::new(allocator).statement_return(return_statement.span, argument)
    }

    fn drag_return_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let Expression::ObjectExpression(object) = expression else {
            return self.builder_initializer_expression(expression, self.attr_name, allocator);
        };
        let ast = AstBuilder::new(allocator);
        let builder_property = self.context.runtime_names().builder_property.as_str();
        let properties = ast.vec_from_iter(object.properties.iter().map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return self.plain_object_property(property, allocator);
            };
            if property
                .key
                .static_name()
                .is_some_and(|name| name == builder_property)
            {
                return ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    self.builder_initializer_expression(&property.value, self.attr_name, allocator),
                    false,
                    false,
                    property.computed,
                );
            }
            self.plain_regular_object_property(property, allocator)
        }));
        ast.expression_object(object.span, properties)
    }

    fn builder_initializer_expression<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        self.builder_attribute_expression_for_attr(expression, attr_name, allocator)
            .unwrap_or_else(|| self.plain_expression(expression, allocator))
    }

    fn builder_attribute_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        self.builder_attribute_expression_for_attr(expression, self.attr_name, allocator)
    }

    fn builder_attribute_expression_for_attr<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Expression::ConditionalExpression(conditional) = expression {
            let ast = AstBuilder::new(allocator);
            let consequent = self
                .builder_attribute_expression_for_attr(
                    &conditional.consequent,
                    attr_name,
                    allocator,
                )
                .unwrap_or_else(|| self.plain_expression(&conditional.consequent, allocator));
            let alternate = self
                .builder_attribute_expression_for_attr(&conditional.alternate, attr_name, allocator)
                .unwrap_or_else(|| self.plain_expression(&conditional.alternate, allocator));
            return Some(ast.expression_conditional(
                conditional.span,
                self.plain_expression(&conditional.test, allocator),
                consequent,
                alternate,
            ));
        }
        let without_key = self.context.is_builder_attribute_without_key(attr_name);
        if let Some(target) = self.builder_reference_expression(expression, allocator) {
            let bound = self.bound_expression(target, AstBuilder::new(allocator).vec(), allocator);
            return Some(if without_key {
                bound
            } else {
                self.builder_object_expression(bound, allocator)
            });
        }
        if let Expression::CallExpression(call) = expression {
            if let Some((callee, builder_name)) = self.builder_call_target(call, allocator) {
                let arrow = self.builder_call_arrow(callee, &builder_name, call, allocator);
                return Some(if without_key {
                    arrow
                } else {
                    self.builder_object_expression(arrow, allocator)
                });
            }
        }
        if let Expression::ObjectExpression(object) = expression {
            return Some(self.object_property_builder_expression(object, allocator));
        }
        None
    }

    fn builder_reference_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Some(name) = ArkuiCallName::this_member(expression) {
            if self.builder_methods.contains(name.as_str()) {
                return Some(expression.clone_in(allocator));
            }
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        self.builder_methods
            .contains(identifier.name.as_str())
            .then(|| expression.clone_in(allocator))
    }

    fn builder_call_target<'b>(
        &self,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> Option<(Expression<'b>, String)> {
        if let Some(name) = ArkuiCallName::this_member(&call.callee) {
            if self.builder_methods.contains(name.as_str()) {
                return Some((call.callee.clone_in(allocator), name));
            }
        }
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        self.builder_methods
            .contains(identifier.name.as_str())
            .then(|| (call.callee.clone_in(allocator), identifier.name.to_string()))
    }

    fn object_property_builder_expression<'b>(
        &self,
        object: &ObjectExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let properties = ast.vec_from_iter(object.properties.iter().map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return self.plain_object_property(property, allocator);
            };
            let Some(property_name) = property.key.static_name() else {
                return self.plain_regular_object_property(property, allocator);
            };
            if !self
                .context
                .runtime_names()
                .is_builder_object_property_name(property_name.as_ref())
            {
                return self.plain_regular_object_property(property, allocator);
            }
            let value = self
                .object_property_builder_value(&property.value, allocator)
                .unwrap_or_else(|| self.plain_expression(&property.value, allocator));
            ast.object_property_kind_object_property(
                property.span,
                property.kind,
                property.key.clone_in(allocator),
                value,
                false,
                false,
                property.computed,
            )
        }));
        ast.expression_object(object.span, properties)
    }

    fn object_property_builder_value<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Some(target) = self.builder_reference_expression(expression, allocator) {
            return Some(self.bound_expression(
                target,
                AstBuilder::new(allocator).vec(),
                allocator,
            ));
        }
        if let Expression::CallExpression(call) = expression {
            if let Some((callee, builder_name)) = self.builder_call_target(call, allocator) {
                let arguments = self.builder_call_arguments(&builder_name, call, allocator);
                return Some(self.bound_expression(callee, arguments, allocator));
            }
        }
        if let Expression::ObjectExpression(object) = expression {
            return Some(self.object_property_builder_expression(object, allocator));
        }
        None
    }

    fn builder_call_arrow<'b>(
        &self,
        callee: Expression<'b>,
        builder_name: &str,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let call_member = ast.member_expression_static(
            SPAN,
            callee,
            ast.identifier_name(SPAN, ast.str(&self.context.runtime_names().method_call)),
            false,
        );
        let mut arguments = OxcVec::new_in(&allocator);
        arguments.push(Argument::from(ast.expression_this(SPAN)));
        arguments.extend(self.builder_call_arguments(builder_name, call, allocator));
        let call = ast.expression_call(SPAN, Expression::from(call_member), NONE, arguments, false);
        let body = ast.function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, call)),
        );
        ast.expression_arrow_function(
            SPAN,
            false,
            false,
            NONE,
            ast.formal_parameters(
                SPAN,
                FormalParameterKind::ArrowFormalParameters,
                ast.vec(),
                NONE,
            ),
            NONE,
            body,
        )
    }

    fn builder_call_arguments<'b>(
        &self,
        builder_name: &str,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = AstBuilder::new(allocator);
        if call.arguments.len() == 1 {
            if let Some(Expression::ObjectExpression(object)) =
                call.arguments.first().and_then(Argument::as_expression)
            {
                let proxy_object = BuilderParamProxyObjectEmitter::new(self.context)
                    .object_expression(object, allocator);
                let proxy_call = ast.expression_call(
                    SPAN,
                    ast.expression_identifier(
                        SPAN,
                        ast.str(&self.context.runtime_names().builder_param_proxy),
                    ),
                    NONE,
                    ast.vec_from_iter([
                        Argument::from(ast.expression_string_literal(
                            SPAN,
                            ast.str(builder_name),
                            None,
                        )),
                        Argument::from(proxy_object),
                    ]),
                    false,
                );
                return ast.vec1(Argument::from(proxy_call));
            }
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .arguments_with_allocator(&call.arguments, allocator)
    }

    fn builder_object_expression<'b>(
        &self,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let property = ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(
                SPAN,
                ast.str(&self.context.runtime_names().builder_property),
            ),
            value,
            false,
            false,
            false,
        );
        ast.expression_object(SPAN, ast.vec1(property))
    }

    fn bound_expression<'b>(
        &self,
        target: Expression<'b>,
        extra_arguments: OxcVec<'b, Argument<'b>>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let bind_member = ast.member_expression_static(
            SPAN,
            target,
            ast.identifier_name(SPAN, ast.str(&self.context.runtime_names().method_bind)),
            false,
        );
        let mut arguments = OxcVec::new_in(&allocator);
        arguments.push(Argument::from(ast.expression_this(SPAN)));
        arguments.extend(extra_arguments);
        ast.expression_call(SPAN, Expression::from(bind_member), NONE, arguments, false)
    }

    fn plain_argument<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .argument_with_allocator(argument, allocator)
    }

    fn plain_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .expression_with_allocator(expression, allocator)
    }

    fn plain_object_property<'b>(
        &self,
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                self.plain_regular_object_property(property, allocator)
            }
            ObjectPropertyKind::SpreadProperty(spread) => ast.object_property_kind_spread_property(
                spread.span,
                self.plain_expression(&spread.argument, allocator),
            ),
        }
    }

    fn plain_regular_object_property<'b>(
        &self,
        property: &ObjectProperty<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        AstBuilder::new(allocator).object_property_kind_object_property(
            property.span,
            property.kind,
            property.key.clone_in(allocator),
            self.plain_expression(&property.value, allocator),
            property.method,
            property.shorthand,
            property.computed,
        )
    }
}

struct CreateArgumentEmitter<'a, 'ctx> {
    component_name: &'a str,
    builder_methods: &'a FxHashSet<String>,
    context: &'ctx BuildContext,
    top_level_raw_object_rewrite: bool,
}

impl<'a, 'ctx> CreateArgumentEmitter<'a, 'ctx> {
    fn new(
        component_name: &'a str,
        builder_methods: &'a FxHashSet<String>,
        context: &'ctx BuildContext,
        top_level_raw_object_rewrite: bool,
    ) -> Self {
        Self {
            component_name,
            builder_methods,
            context,
            top_level_raw_object_rewrite,
        }
    }

    fn arguments<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = AstBuilder::new(allocator);
        let mut emitted = ast.vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
        );
        self.push_xcomponent_module_identity(&mut emitted, ast);
        emitted
    }

    fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = AstBuilder::new(allocator);
        let mut emitted = ast.vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
        );
        self.push_xcomponent_module_identity(&mut emitted, ast);
        emitted
    }

    fn push_xcomponent_module_identity<'b>(
        &self,
        arguments: &mut OxcVec<'b, Argument<'b>>,
        ast: AstBuilder<'b>,
    ) {
        if self.context.component_kind(self.component_name) != ArkuiRuntimeComponentKind::XComponent
        {
            return;
        }
        let Some(identity) = &self.context.xcomponent_module_identity else {
            return;
        };
        arguments.push(Argument::from(ast.expression_string_literal(
            SPAN,
            ast.str(identity),
            None,
        )));
    }

    fn argument<'b>(&self, argument: &Argument<'_>, allocator: &'b Allocator) -> Argument<'b> {
        let Some(expression) = argument.as_expression() else {
            return self.plain_argument(argument, allocator);
        };
        Argument::from(self.expression(expression, allocator))
    }

    fn expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let Expression::ObjectExpression(object) = expression else {
            return self.plain_expression(expression, allocator);
        };
        let has_double_dollar = object.properties.iter().any(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            property.key.static_name().is_some_and(|name| {
                self.context
                    .is_component_double_dollar_property_name(self.component_name, name.as_ref())
                    && DoubleDollarBindingEmitter::new(self.context)
                        .updated_expression(&property.value, allocator)
                        .is_some()
            })
        });
        // ets-loader `checkCreateArgumentBuilder` consumes both the explicit
        // CREATE_BIND_COMPONENT set and component-specific `create` entries in
        // BIND_OBJECT_PROPERTY. ComponentMetadata merges those SDK-owned rules
        // behind `is_builder_attribute_method`.
        let needs_builder = self.context.is_builder_attribute_method(
            self.component_name,
            &self.context.runtime_names().method_create,
        );
        if !has_double_dollar && !needs_builder {
            return self.plain_expression(expression, allocator);
        }
        let ast = AstBuilder::new(allocator);
        let attribute_emitter = AttributeBindingArgumentEmitter::new(
            self.component_name,
            &self.context.runtime_names().method_create,
            self.builder_methods,
            self.context,
        );
        let properties = ast.vec_from_iter(object.properties.iter().map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return attribute_emitter.plain_object_property(property, allocator);
            };
            let Some(property_name) = property.key.static_name() else {
                return attribute_emitter.plain_regular_object_property(property, allocator);
            };
            if self.context.is_component_double_dollar_property_name(
                self.component_name,
                property_name.as_ref(),
            ) {
                if let Some(value) = DoubleDollarBindingEmitter::new(self.context)
                    .value_change_object(&property.value, allocator)
                {
                    return ast.object_property_kind_object_property(
                        property.span,
                        property.kind,
                        property.key.clone_in(allocator),
                        value,
                        false,
                        false,
                        property.computed,
                    );
                }
            }
            if needs_builder
                && self
                    .context
                    .runtime_names()
                    .is_builder_object_property_name(property_name.as_ref())
            {
                let value = attribute_emitter
                    .object_property_builder_value(&property.value, allocator)
                    .unwrap_or_else(|| {
                        attribute_emitter.plain_expression(&property.value, allocator)
                    });
                return ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    value,
                    false,
                    false,
                    property.computed,
                );
            }
            attribute_emitter.plain_regular_object_property(property, allocator)
        }));
        ast.expression_object(object.span, properties)
    }

    fn plain_argument<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        let mut emitter = BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite();
        if !self.top_level_raw_object_rewrite {
            emitter = emitter.without_top_level_raw_object_rewrite();
        }
        emitter.argument_with_allocator(argument, allocator)
    }

    fn plain_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let mut emitter = BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite();
        if !self.top_level_raw_object_rewrite {
            emitter = emitter.without_top_level_raw_object_rewrite();
        }
        emitter.expression_with_allocator(expression, allocator)
    }
}

struct DoubleDollarBindingEmitter<'ctx> {
    context: &'ctx BuildContext,
}

impl<'ctx> DoubleDollarBindingEmitter<'ctx> {
    fn new(context: &'ctx BuildContext) -> Self {
        Self { context }
    }

    fn attribute_arguments<'b>(
        &self,
        arguments: Vec<Argument<'b>>,
        style_attr: bool,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let Some(first_expression) = arguments.first().and_then(Argument::as_expression) else {
            return AstBuilder::new(allocator).vec_from_iter(arguments);
        };
        let Some(value_expression) = self.updated_expression(first_expression, allocator) else {
            return AstBuilder::new(allocator).vec_from_iter(arguments);
        };
        let ast = AstBuilder::new(allocator);
        if style_attr && arguments.len() >= 2 {
            let mut out = OxcVec::new_in(&allocator);
            out.push(Argument::from(
                self.value_change_object_from_value(value_expression, allocator),
            ));
            out.extend(arguments.into_iter().skip(1));
            return out;
        }
        let mut out = OxcVec::new_in(&allocator);
        out.push(Argument::from(value_expression.clone_in(allocator)));
        out.push(Argument::from(
            self.change_event_arrow(value_expression, allocator),
        ));
        out.extend(arguments.into_iter().skip(1));
        ast.vec_from_iter(out)
    }

    fn value_change_object<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        self.updated_expression(expression, allocator)
            .map(|value| self.value_change_object_from_value(value, allocator))
    }

    fn value_change_object_from_value<'b>(
        &self,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let change_target = value.clone_in(allocator);
        let value_property = ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(
                SPAN,
                ast.str(&self.context.runtime_names().double_dollar_value),
            ),
            value,
            false,
            false,
            false,
        );
        let change_property = ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(
                SPAN,
                ast.str(&self.context.runtime_names().double_dollar_change_event),
            ),
            self.change_event_arrow(change_target, allocator),
            false,
            false,
            false,
        );
        ast.expression_object(SPAN, ast.vec_from_iter([value_property, change_property]))
    }

    fn change_event_arrow<'b>(
        &self,
        target: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let target = Self::assignment_target_from_expression(target, ast);
        let new_value = ast.expression_identifier(
            SPAN,
            ast.str(&self.context.runtime_names().double_dollar_new_value),
        );
        let assignment =
            ast.expression_assignment(SPAN, AssignmentOperator::Assign, target, new_value);
        let body = ast.function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, assignment)),
        );
        let mut params = ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec_with_capacity(1),
            NONE,
        );
        params.items.push(ast.formal_parameter(
            SPAN,
            ast.vec(),
            ast.binding_pattern_binding_identifier(
                SPAN,
                ast.str(&self.context.runtime_names().double_dollar_new_value),
            ),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
        ));
        ast.expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
    }

    fn updated_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        let ast = AstBuilder::new(allocator);
        match expression {
            Expression::Identifier(identifier) => {
                let name = identifier.name.as_str();
                if name == self.context.runtime_names().double_dollar_this {
                    return Some(ast.expression_this(identifier.span));
                }
                let stripped = name.strip_prefix("$$")?;
                if stripped.is_empty() || name.starts_with("$$.") {
                    return None;
                }
                Some(ast.expression_identifier(identifier.span, ast.str(stripped)))
            }
            Expression::StaticMemberExpression(member) => {
                let object = self.updated_expression(&member.object, allocator)?;
                Some(Expression::from(ast.member_expression_static(
                    member.span,
                    object,
                    member.property.clone_in(allocator),
                    member.optional,
                )))
            }
            Expression::ComputedMemberExpression(member) => {
                let object = self.updated_expression(&member.object, allocator)?;
                Some(Expression::from(ast.member_expression_computed(
                    member.span,
                    object,
                    member.expression.clone_in(allocator),
                    member.optional,
                )))
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.updated_expression(&parenthesized.expression, allocator)
            }
            _ => None,
        }
    }

    fn assignment_target_from_expression<'b>(
        expression: Expression<'b>,
        ast: AstBuilder<'b>,
    ) -> AssignmentTarget<'b> {
        match expression {
            Expression::Identifier(identifier) => {
                AssignmentTarget::from(ast.simple_assignment_target_assignment_target_identifier(
                    identifier.span,
                    identifier.name,
                ))
            }
            Expression::StaticMemberExpression(member) => {
                AssignmentTarget::from(MemberExpression::StaticMemberExpression(member))
            }
            Expression::ComputedMemberExpression(member) => {
                AssignmentTarget::from(MemberExpression::ComputedMemberExpression(member))
            }
            _ => unreachable!("double-dollar target must be identifier or member expression"),
        }
    }
}

struct ComponentCallStatementEmitter;

impl ComponentCallStatementEmitter {
    const ANIMATION_ATTRIBUTE: &'static str = "animation";

    fn create<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        create_method: &str,
        arguments: &[Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
        component_span: Span,
    ) -> Statement<'b> {
        let arguments = CreateArgumentEmitter::new(component_name, builder_methods, context, false)
            .arguments(arguments, allocator);
        Self::raw_call_with_source(
            ast,
            component_name,
            create_method,
            arguments,
            component_span,
            SPAN,
        )
    }

    fn attribute<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        attr_name: &str,
        arguments: &[Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Statement<'b> {
        Self::attribute_with_source(
            ast,
            allocator,
            component_name,
            attr_name,
            arguments,
            builder_methods,
            context,
            SPAN,
            SPAN,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn attribute_with_source<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        attr_name: &str,
        arguments: &[Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
        component_span: Span,
        attribute_span: Span,
    ) -> Statement<'b> {
        let arguments = AttributeArgumentEmitter::new(
            component_name,
            attr_name,
            builder_methods,
            Some(context),
        )
        .arguments(arguments, allocator);
        if context.is_extend_attribute(component_name, attr_name) {
            return Self::raw_identifier_call(
                ast,
                &GeneratedViewpuName::extend_function(component_name, attr_name),
                arguments,
            );
        }
        if context
            .runtime_names()
            .component_attribute_requires_bind(attr_name)
        {
            return Self::raw_bound_call(ast, component_name, attr_name, arguments, context);
        }
        Self::raw_call_with_source(
            ast,
            component_name,
            attr_name,
            arguments,
            component_span,
            attribute_span,
        )
    }

    fn is_animation_attribute(attr_name: &str) -> bool {
        attr_name == Self::ANIMATION_ATTRIBUTE
    }

    fn animation<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[Argument<'_>],
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments = BuildArgumentEmitter::new()
            .with_context(context)
            .with_nested_raw_object_rewrite()
            .arguments_with_allocator(arguments, allocator);
        Self::raw_call(
            ast,
            &context.runtime_names().component_context,
            Self::ANIMATION_ATTRIBUTE,
            arguments,
        )
    }

    fn animation_refs<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[&Argument<'_>],
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments = BuildArgumentEmitter::new()
            .with_context(context)
            .with_nested_raw_object_rewrite()
            .argument_refs_with_allocator(arguments, allocator);
        Self::raw_call(
            ast,
            &context.runtime_names().component_context,
            Self::ANIMATION_ATTRIBUTE,
            arguments,
        )
    }

    fn clear_animation<'b>(ast: AstBuilder<'b>, context: &BuildContext) -> Statement<'b> {
        let arguments = ast.vec1(Argument::from(ast.expression_null_literal(SPAN)));
        Self::raw_call(
            ast,
            &context.runtime_names().component_context,
            Self::ANIMATION_ATTRIBUTE,
            arguments,
        )
    }

    fn flush_animation_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        allocator: &'b Allocator,
    ) {
        statements.extend(std::mem::replace(segment, OxcVec::new_in(&allocator)));
    }

    #[allow(clippy::too_many_arguments)]
    fn push_animation_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[Argument<'_>],
        context: &BuildContext,
        emit_configuration: bool,
        emit_clear: bool,
    ) {
        if emit_configuration {
            statements.push(Self::animation(ast, allocator, arguments, context));
            Self::flush_animation_segment(statements, segment, allocator);
        } else {
            debug_assert!(segment.is_empty());
        }
        if emit_clear {
            statements.push(Self::clear_animation(ast, context));
        }
    }

    fn push_animation_ref_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[&Argument<'_>],
        context: &BuildContext,
        emit_configuration: bool,
        emit_clear: bool,
    ) {
        if emit_configuration {
            statements.push(Self::animation_refs(ast, allocator, arguments, context));
            Self::flush_animation_segment(statements, segment, allocator);
        } else {
            debug_assert!(segment.is_empty());
        }
        if emit_clear {
            statements.push(Self::clear_animation(ast, context));
        }
    }

    fn is_null_animation_refs(arguments: &[&Argument<'_>]) -> bool {
        matches!(arguments, [argument] if matches!(argument, Argument::NullLiteral(_)))
    }

    fn is_null_animation(arguments: &[Argument<'_>]) -> bool {
        matches!(arguments, [Argument::NullLiteral(_)])
    }

    fn argument_refs<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        method_name: &str,
        arguments: &[&Argument<'_>],
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments = BuildArgumentEmitter::new()
            .with_context(context)
            .with_nested_raw_object_rewrite()
            .argument_refs_with_allocator(arguments, allocator);
        Self::raw_call(ast, component_name, method_name, arguments)
    }

    fn create_refs<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        method_name: &str,
        arguments: &[&Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments = CreateArgumentEmitter::new(component_name, builder_methods, context, false)
            .argument_refs(arguments, allocator);
        Self::raw_call(ast, component_name, method_name, arguments)
    }

    fn attribute_refs<'b>(
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        attr_name: &str,
        arguments: &[&Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments = AttributeArgumentEmitter::new(
            component_name,
            attr_name,
            builder_methods,
            Some(context),
        )
        .argument_refs(arguments, allocator);
        if context.is_extend_attribute(component_name, attr_name) {
            return Self::raw_identifier_call(
                ast,
                &GeneratedViewpuName::extend_function(component_name, attr_name),
                arguments,
            );
        }
        if context
            .runtime_names()
            .component_attribute_requires_bind(attr_name)
        {
            return Self::raw_bound_call(ast, component_name, attr_name, arguments, context);
        }
        Self::raw_call(ast, component_name, attr_name, arguments)
    }

    fn empty<'b>(ast: AstBuilder<'b>, component_name: &str, method_name: &str) -> Statement<'b> {
        Self::raw_call(ast, component_name, method_name, ast.vec())
    }

    fn empty_with_source<'b>(
        ast: AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        component_span: Span,
    ) -> Statement<'b> {
        Self::raw_call_with_source(
            ast,
            component_name,
            method_name,
            ast.vec(),
            component_span,
            SPAN,
        )
    }

    fn gesture_low_priority<'b>(ast: AstBuilder<'b>, context: &BuildContext) -> Statement<'b> {
        let runtime_names = context.runtime_names();
        let gesture_priority =
            ast.expression_identifier(SPAN, ast.str(&runtime_names.gesture_priority));
        let low_member = ast.member_expression_static(
            SPAN,
            gesture_priority,
            ast.identifier_name(SPAN, ast.str(&runtime_names.gesture_priority_low)),
            false,
        );
        let arguments = ast.vec1(Argument::from(Expression::from(low_member)));
        Self::raw_call(
            ast,
            &runtime_names.component_gesture,
            &runtime_names.method_create,
            arguments,
        )
    }

    fn raw_call<'b>(
        ast: AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        Self::raw_call_with_source(ast, component_name, method_name, arguments, SPAN, SPAN)
    }

    fn raw_call_with_source<'b>(
        ast: AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
        component_span: Span,
        method_span: Span,
    ) -> Statement<'b> {
        let object = ast.expression_identifier(component_span, ast.str(component_name));
        let property = ast.identifier_name(method_span, ast.str(method_name));
        let member = ast.member_expression_static(SPAN, object, property, false);
        let call = ast.expression_call(SPAN, Expression::from(member), NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }

    fn raw_identifier_call<'b>(
        ast: AstBuilder<'b>,
        function_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        let callee = ast.expression_identifier(SPAN, ast.str(function_name));
        let call = ast.expression_call(SPAN, callee, NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }

    fn raw_bound_call<'b>(
        ast: AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let object = ast.expression_identifier(SPAN, ast.str(component_name));
        let property = ast.identifier_name(SPAN, ast.str(method_name));
        let member = ast.member_expression_static(SPAN, object, property, false);
        let bind_member = ast.member_expression_static(
            SPAN,
            Expression::from(member),
            ast.identifier_name(SPAN, ast.str(&context.runtime_names().method_bind)),
            false,
        );
        let bind_call = ast.expression_call(
            SPAN,
            Expression::from(bind_member),
            NONE,
            ast.vec1(Argument::from(ast.expression_this(SPAN))),
            false,
        );
        let call = ast.expression_call(SPAN, bind_call, NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }
}

struct NavigationCreateCall<'a, 'b> {
    component: &'a ArkUIComponentExpression<'a>,
    name: &'b str,
    module_name: &'b str,
    page_path: &'b str,
}

impl<'a, 'b> NavigationCreateCall<'a, 'b> {
    fn new(
        component: &'a ArkUIComponentExpression<'a>,
        name: &'b str,
        module_name: &'b str,
        page_path: &'b str,
    ) -> Self {
        Self {
            component,
            name,
            module_name,
            page_path,
        }
    }

    fn statement<'c>(
        &self,
        ast: AstBuilder<'c>,
        allocator: &'c Allocator,
        context: &BuildContext,
        builder_body: Option<OxcVec<'c, Statement<'c>>>,
    ) -> Statement<'c> {
        let callee = Expression::from(ast.member_expression_static(
            SPAN,
            ast.expression_identifier(SPAN, ast.str(self.name)),
            ast.identifier_name(SPAN, ast.str(&context.runtime_names().method_create)),
            false,
        ));
        let arguments = self.arguments(ast, allocator, context, builder_body);
        let call = ast.expression_call(SPAN, callee, NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }

    fn arguments<'c>(
        &self,
        ast: AstBuilder<'c>,
        allocator: &'c Allocator,
        context: &BuildContext,
        builder_body: Option<OxcVec<'c, Statement<'c>>>,
    ) -> OxcVec<'c, Argument<'c>> {
        let metadata = NavigationCreateMetadata::new(
            self.module_name,
            self.page_path,
            context
                .component_kind(self.name)
                .is_navigation()
                .then_some(!self.component.arguments.is_empty()),
        )
        .expression(ast, context);
        let mut arguments = OxcVec::new_in(&allocator);
        if let Some(builder_body) = builder_body {
            arguments.push(Argument::from(self.builder_arrow(ast, builder_body)));
        } else if self.component.arguments.is_empty() {
            arguments.push(Argument::from(self.empty_builder_arrow(ast)));
        } else {
            for argument in &self.component.arguments {
                arguments.push(self.source_argument(argument, allocator, context));
            }
        }
        arguments.push(Argument::from(metadata));
        arguments
    }

    fn source_argument<'c>(
        &self,
        argument: &Argument<'_>,
        allocator: &'c Allocator,
        context: &BuildContext,
    ) -> Argument<'c> {
        BuildArgumentEmitter::new()
            .with_context(context)
            .with_nested_raw_object_rewrite()
            .without_top_level_raw_object_rewrite()
            .argument_with_allocator(argument, allocator)
    }

    fn empty_builder_arrow<'c>(&self, ast: AstBuilder<'c>) -> Expression<'c> {
        let params = ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec(),
            NONE,
        );
        let body = ast.function_body(SPAN, ast.vec(), ast.vec());
        ast.expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
    }

    fn builder_arrow<'c>(
        &self,
        ast: AstBuilder<'c>,
        body_stmts: OxcVec<'c, Statement<'c>>,
    ) -> Expression<'c> {
        let params = ast.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            ast.vec(),
            NONE,
        );
        let body = ast.function_body(SPAN, ast.vec(), body_stmts);
        ast.expression_arrow_function(SPAN, false, false, NONE, params, NONE, body)
    }
}

struct NavigationCreateMetadata<'a> {
    module_name: &'a str,
    page_path: &'a str,
    user_create_stack: Option<bool>,
}

impl<'a> NavigationCreateMetadata<'a> {
    fn new(module_name: &'a str, page_path: &'a str, user_create_stack: Option<bool>) -> Self {
        Self {
            module_name,
            page_path,
            user_create_stack,
        }
    }

    fn expression<'b>(&self, ast: AstBuilder<'b>, context: &BuildContext) -> Expression<'b> {
        let runtime_names = context.runtime_names();
        let mut properties = ast.vec_from_iter([
            self.string_property(ast, &runtime_names.module_name, self.module_name),
            self.string_property(ast, &runtime_names.page_path, self.page_path),
        ]);
        if let Some(value) = self.user_create_stack {
            properties.push(ast.object_property_kind_object_property(
                SPAN,
                PropertyKind::Init,
                ast.property_key_static_identifier(
                    SPAN,
                    ast.str(&runtime_names.is_user_create_stack),
                ),
                ast.expression_boolean_literal(SPAN, value),
                false,
                false,
                false,
            ));
        }
        ast.expression_object(SPAN, properties)
    }

    fn string_property<'b>(
        &self,
        ast: AstBuilder<'b>,
        name: &str,
        value: &str,
    ) -> ObjectPropertyKind<'b> {
        ast.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            ast.property_key_static_identifier(SPAN, ast.str(name)),
            ast.expression_string_literal(SPAN, ast.str(value), None),
            false,
            false,
            false,
        )
    }
}

struct BuildArgumentEmitter<'a> {
    context: Option<&'a BuildContext>,
    top_level_raw_object_rewrite: bool,
    nested_raw_object_rewrite: bool,
}

impl<'a> BuildArgumentEmitter<'a> {
    fn new() -> Self {
        Self {
            context: None,
            top_level_raw_object_rewrite: true,
            nested_raw_object_rewrite: false,
        }
    }

    fn with_context(mut self, context: &'a BuildContext) -> Self {
        self.context = Some(context);
        self
    }

    fn with_optional_context(mut self, context: Option<&'a BuildContext>) -> Self {
        self.context = context;
        self
    }

    fn with_nested_raw_object_rewrite(mut self) -> Self {
        self.nested_raw_object_rewrite = true;
        self
    }

    fn without_top_level_raw_object_rewrite(mut self) -> Self {
        self.top_level_raw_object_rewrite = false;
        self
    }

    fn arguments_with_allocator<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        AstBuilder::new(allocator).vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument_with_allocator(argument, allocator)),
        )
    }

    fn argument_refs_with_allocator<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        AstBuilder::new(allocator).vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument_with_allocator(argument, allocator)),
        )
    }

    fn argument_with_allocator<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        if self.top_level_raw_object_rewrite {
            if let Some(context) = self.context {
                if let Some(argument) =
                    RawObjectArgumentTransform::new(context).argument(argument, allocator)
                {
                    return argument;
                }
            }
        }
        match argument {
            Argument::SpreadElement(spread) => {
                let expression = self.expression_with_allocator(&spread.argument, allocator);
                let ast = AstBuilder::new(allocator);
                Argument::SpreadElement(ast.alloc_spread_element(spread.span, expression))
            }
            _ => argument
                .as_expression()
                .map(|expression| {
                    Argument::from(self.expression_with_allocator(expression, allocator))
                })
                .unwrap_or_else(|| argument.clone_in(allocator)),
        }
    }

    fn expression_with_allocator<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let expression = match GlobalContextCallRewriter::new(allocator).expression(expression) {
            Some(expression) => {
                ArkuiStateStylesObjectEmitter::owned_expression(expression, allocator)
            }
            None => ArkuiStateStylesObjectEmitter::expression(expression, allocator),
        };
        self.finalize_owned_expression(expression, allocator)
    }

    fn owned_expression_with_allocator<'b>(
        &self,
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let expression = GlobalContextCallRewriter::new(allocator).owned_expression(expression);
        let expression = ArkuiStateStylesObjectEmitter::owned_expression(expression, allocator);
        self.finalize_owned_expression(expression, allocator)
    }

    fn finalize_owned_expression<'b>(
        &self,
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        if self.nested_raw_object_rewrite {
            if let Some(context) = self.context {
                return RawObjectArgumentTransform::new(context)
                    .owned_expression_with_call_args(expression, allocator);
            }
        }
        expression
    }
}

struct ArkuiStateStylesObjectEmitter;

impl ArkuiStateStylesObjectEmitter {
    fn expression<'b>(expression: &Expression<'_>, allocator: &'b Allocator) -> Expression<'b> {
        if Self::should_rewrite_expression(expression) {
            Self::rewrite_expression(expression, allocator)
        } else {
            expression.clone_in(allocator)
        }
    }

    fn owned_expression<'b>(
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        if Self::should_rewrite_expression(&expression) {
            Self::rewrite_expression(&expression, allocator)
        } else {
            expression
        }
    }

    fn rewrite_expression<'b>(
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        match expression {
            Expression::ObjectExpression(object) => {
                let properties = ast.vec_from_iter(
                    object
                        .properties
                        .iter()
                        .map(|property| Self::rewrite_object_property(property, allocator)),
                );
                ast.expression_object(object.span, properties)
            }
            Expression::ArrayExpression(array) => {
                let elements = ast.vec_from_iter(array.elements.iter().map(|element| {
                    if let Some(expression) = element.as_expression() {
                        ArrayExpressionElement::from(Self::rewrite_expression(
                            expression, allocator,
                        ))
                    } else {
                        element.clone_in(allocator)
                    }
                }));
                ast.expression_array(array.span, elements)
            }
            _ => expression.clone_in(allocator),
        }
    }

    fn should_rewrite_expression(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::ObjectExpression(object) => object
                .properties
                .iter()
                .any(Self::should_rewrite_object_property),
            Expression::ArrayExpression(array) => array.elements.iter().any(|element| {
                element
                    .as_expression()
                    .is_some_and(Self::should_rewrite_expression)
            }),
            _ => false,
        }
    }

    fn should_rewrite_object_property(property: &ObjectPropertyKind<'_>) -> bool {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                Self::is_leading_dot_property(&property.value)
                    || Self::should_rewrite_expression(&property.value)
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                Self::should_rewrite_expression(&spread.argument)
            }
        }
    }

    fn rewrite_object_property<'b>(
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if let Some((name, value)) = Self::leading_dot_property(&property.value, allocator)
                {
                    return ast.object_property_kind_object_property(
                        property.span,
                        PropertyKind::Init,
                        ast.property_key_static_identifier(property.span, ast.str(&name)),
                        value,
                        false,
                        false,
                        false,
                    );
                }
                ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    Self::rewrite_expression(&property.value, allocator),
                    property.method,
                    property.shorthand,
                    property.computed,
                )
            }
            ObjectPropertyKind::SpreadProperty(spread) => ast.object_property_kind_spread_property(
                spread.span,
                Self::rewrite_expression(&spread.argument, allocator),
            ),
        }
    }

    fn is_leading_dot_property(expression: &Expression<'_>) -> bool {
        let Expression::LeadingDotExpression(leading) = expression else {
            return false;
        };
        let Expression::CallExpression(call) = &leading.expression else {
            return false;
        };
        matches!(&call.callee, Expression::Identifier(_)) && call.arguments.len() == 1
    }

    fn leading_dot_property<'b>(
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<(String, Expression<'b>)> {
        let Expression::LeadingDotExpression(leading) = expression else {
            return None;
        };
        let Expression::CallExpression(call) = &leading.expression else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        let mut args = call.arguments.iter();
        let value = args
            .next()?
            .as_expression()
            .map(|expression| Self::rewrite_expression(expression, allocator))?;
        if args.next().is_some() {
            return None;
        }
        Some((callee.name.to_string(), value))
    }
}

#[derive(Clone, Copy)]
enum BuilderMethodReceiver {
    Direct,
    This,
}

/// Emits the `transferBuilderCall` shape used by ets-loader for a
/// `WrappedBuilder.builder(...)` expression. The dispatch and bind property
/// names come from SDK metadata instead of being duplicated here.
struct BuilderDispatchCallEmitter<'a, 'ctx> {
    call: &'a CallExpression<'a>,
    context: &'ctx BuildContext,
}

impl<'a, 'ctx> BuilderDispatchCallEmitter<'a, 'ctx> {
    fn new(call: &'a CallExpression<'a>, context: &'ctx BuildContext) -> Self {
        Self { call, context }
    }

    fn matches(&self) -> bool {
        ArkuiCallName::is_builder_dispatch_call(self.call, self.context)
    }

    fn expression<'b>(&self, allocator: &'b Allocator) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let mut call = self.call.clone_in(allocator);
        self.rewrite(&mut call, allocator);
        Expression::CallExpression(ast.alloc(call))
    }

    fn rewrite<'b>(&self, call: &mut CallExpression<'b>, allocator: &'b Allocator) {
        let ast = AstBuilder::new(allocator);
        let bind_member = ast.member_expression_static(
            SPAN,
            self.call.callee.clone_in(allocator),
            ast.identifier_name(SPAN, ast.str(&self.context.runtime_names().method_bind)),
            false,
        );
        call.callee = ast.expression_call(
            SPAN,
            Expression::from(bind_member),
            NONE,
            ast.vec1(Argument::from(ast.expression_this(SPAN))),
            false,
        );
        call.arguments = self.arguments(ast, allocator);
    }

    fn arguments<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        if let [argument] = self.call.arguments.as_slice() {
            if let Some(Expression::ObjectExpression(object)) = argument.as_expression() {
                let proxy_object = BuilderParamProxyObjectEmitter::new(self.context)
                    .object_expression(object, allocator);
                let arguments = ast.vec_from_iter([
                    Argument::from(ast.expression_string_literal(
                        SPAN,
                        ast.str(&self.context.runtime_names().builder_property),
                        None,
                    )),
                    Argument::from(proxy_object),
                ]);
                let proxy = ast.expression_call(
                    SPAN,
                    ast.expression_identifier(
                        SPAN,
                        ast.str(&self.context.runtime_names().builder_param_proxy),
                    ),
                    NONE,
                    arguments,
                    false,
                );
                return ast.vec1(Argument::from(proxy));
            }
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .arguments_with_allocator(&self.call.arguments, allocator)
    }
}

struct BuilderMethodCallEmitter<'a> {
    receiver: BuilderMethodReceiver,
    name: &'a str,
    arguments: &'a [Argument<'a>],
    context: &'a BuildContext,
}

impl<'a> BuilderMethodCallEmitter<'a> {
    fn new(
        receiver: BuilderMethodReceiver,
        name: &'a str,
        arguments: &'a [Argument<'a>],
        context: &'a BuildContext,
    ) -> Self {
        Self {
            receiver,
            name,
            arguments,
            context,
        }
    }

    fn statement<'b>(&self, ast: AstBuilder<'b>, allocator: &'b Allocator) -> Statement<'b> {
        let target = match self.receiver {
            BuilderMethodReceiver::Direct => ast.expression_identifier(SPAN, ast.str(self.name)),
            BuilderMethodReceiver::This => {
                let object = ast.expression_this(SPAN);
                let property = ast.identifier_name(SPAN, ast.str(self.name));
                Expression::from(ast.member_expression_static(SPAN, object, property, false))
            }
        };
        let bind_member = ast.member_expression_static(
            SPAN,
            target,
            ast.identifier_name(SPAN, ast.str(&self.context.runtime_names().method_bind)),
            false,
        );
        let bind_call = ast.expression_call(
            SPAN,
            Expression::from(bind_member),
            NONE,
            ast.vec1(Argument::from(ast.expression_this(SPAN))),
            false,
        );
        let arguments = self.arguments(ast, allocator);
        let call = ast.expression_call(SPAN, bind_call, NONE, arguments, false);
        ast.statement_expression(SPAN, call)
    }

    fn arguments<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        // SDK process_component_build.js::processComponentChild routes
        // customBuilderMethod, builderParamMethod, and builderTypeFunction
        // through the same transferBuilderCall implementation. Every one of
        // those call kinds proxies a sole object-literal argument.
        if let Some(argument) = self.proxy_argument(ast, allocator) {
            return ast.vec1(argument);
        }
        let mut emitter = BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite();
        // SDK process_component_class.js::processBuildMember decides whether
        // to unwrap an observed argument from the source call, before
        // transferBuilderCall appends `.bind(this)`. An unqualified
        // `builder(this.value)` has an identifier callee and therefore keeps
        // the top-level observed value; `this.builder(this.value)` still has a
        // property-access callee and is eligible.
        if matches!(self.receiver, BuilderMethodReceiver::Direct) {
            emitter = emitter.without_top_level_raw_object_rewrite();
        }
        emitter.arguments_with_allocator(self.arguments, allocator)
    }

    fn proxy_argument<'b>(
        &self,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Option<Argument<'b>> {
        let [argument] = self.arguments else {
            return None;
        };
        let Expression::ObjectExpression(object) = argument.as_expression()? else {
            return None;
        };
        let proxy_object =
            BuilderParamProxyObjectEmitter::new(self.context).object_expression(object, allocator);
        let args = ast.vec_from_iter([
            Argument::from(ast.expression_string_literal(SPAN, ast.str(self.name), None)),
            Argument::from(proxy_object),
        ]);
        let call = ast.expression_call(
            SPAN,
            ast.expression_identifier(
                SPAN,
                ast.str(&self.context.runtime_names().builder_param_proxy),
            ),
            NONE,
            args,
            false,
        );
        Some(Argument::from(call))
    }
}

struct BuilderParamProxyObjectEmitter<'a> {
    context: &'a BuildContext,
}

impl<'a> BuilderParamProxyObjectEmitter<'a> {
    fn new(context: &'a BuildContext) -> Self {
        Self { context }
    }

    fn object_expression<'b>(
        &self,
        object: &ObjectExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let properties = ast.vec_from_iter(
            object
                .properties
                .iter()
                .map(|property| self.object_property(property, allocator)),
        );
        ast.expression_object(object.span, properties)
    }

    fn object_property<'b>(
        &self,
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                let value = self.arrow_expression(&property.value, allocator);
                ast.object_property_kind_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    value,
                    false,
                    property.shorthand,
                    property.computed,
                )
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                let value = BuildArgumentEmitter::new()
                    .with_context(self.context)
                    .expression_with_allocator(&spread.argument, allocator);
                ast.object_property_kind_spread_property(spread.span, value)
            }
        }
    }

    fn arrow_expression<'b>(
        &self,
        value: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let return_value = self.return_expression(value, allocator);
        let body = ast.function_body(
            SPAN,
            ast.vec(),
            ast.vec1(ast.statement_expression(SPAN, return_value)),
        );
        ast.expression_arrow_function(
            SPAN,
            true,
            false,
            NONE,
            ast.formal_parameters(
                SPAN,
                FormalParameterKind::ArrowFormalParameters,
                ast.vec(),
                NONE,
            ),
            NONE,
            body,
        )
    }

    fn return_expression<'b>(
        &self,
        value: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        if let Some((receiver, name)) = Self::proxy_property(value) {
            let uses_backing_property = match receiver {
                BuilderProxyReceiver::This => {
                    self.context.builder_proxy_scope != BuilderProxyScope::GlobalBuilder
                }
                BuilderProxyReceiver::BuilderParameter => {
                    self.context.builder_proxy_scope != BuilderProxyScope::Component
                }
            };
            if uses_backing_property {
                return self.property_fallback(receiver, name, allocator);
            }
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .expression_with_allocator(value, allocator)
    }

    fn proxy_property<'b>(value: &'b Expression<'_>) -> Option<(BuilderProxyReceiver, &'b str)> {
        let Expression::StaticMemberExpression(member) = value else {
            return None;
        };
        let receiver = match &member.object {
            Expression::ThisExpression(_) => BuilderProxyReceiver::This,
            Expression::Identifier(identifier) if identifier.name == "$$" => {
                BuilderProxyReceiver::BuilderParameter
            }
            _ => return None,
        };
        Some((receiver, member.property.name.as_str()))
    }

    fn property_fallback<'b>(
        &self,
        receiver: BuilderProxyReceiver,
        name: &str,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let backing = self.computed_member(ast, receiver, &format!("__{name}"));
        let consequent = self.computed_member(ast, receiver, &format!("__{name}"));
        let alternate = self.computed_member(ast, receiver, name);
        ast.expression_conditional(SPAN, backing, consequent, alternate)
    }

    fn computed_member<'b>(
        &self,
        ast: AstBuilder<'b>,
        receiver: BuilderProxyReceiver,
        name: &str,
    ) -> Expression<'b> {
        let object = match receiver {
            BuilderProxyReceiver::This => ast.expression_this(SPAN),
            BuilderProxyReceiver::BuilderParameter => {
                ast.expression_identifier(SPAN, ast.str("$$"))
            }
        };
        Expression::from(ast.member_expression_computed(
            SPAN,
            object,
            ast.expression_string_literal(SPAN, ast.str(name), None),
            false,
        ))
    }
}

#[derive(Clone, Copy)]
enum BuilderProxyReceiver {
    This,
    BuilderParameter,
}

#[cfg(test)]
mod tests;
