use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt, sync::Arc};

use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrayExpressionElement,
    ArrowFunctionExpression, AssignmentTarget, CallExpression, ChainElement, ChainExpression,
    Expression, FormalParameterKind, FunctionBody, IfStatement, MemberExpression, ObjectExpression,
    ObjectProperty, ObjectPropertyKind, PropertyKind, Statement,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::{GetSpan, Span, SPAN};
use oxc_syntax::operator::AssignmentOperator;

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
pub(crate) use styles::StylesBodyLowerer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLoweringError {
    UnsupportedStatement,
    UnsupportedChild,
    UnsupportedCallee,
    UnsupportedChain,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomComponentInfo {
    pub update_props: FxHashSet<String>,
    pub link_props: FxHashSet<String>,
    pub builder_params: FxHashSet<String>,
    pub reusable: bool,
    pub is_custom_dialog: bool,
}

pub type CustomComponentUpdateProps = FxHashMap<String, CustomComponentInfo>;

#[derive(Clone, Debug)]
struct BuildContext {
    use_parent_context: bool,
    current_component_reusable: bool,
    raw_object_fields: Arc<FxHashSet<String>>,
    component_metadata: Arc<ComponentMetadata>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    module_name: Arc<str>,
    custom_component_base_name: &'static str,
}

impl Default for BuildContext {
    fn default() -> Self {
        Self {
            use_parent_context: false,
            current_component_reusable: false,
            raw_object_fields: Arc::default(),
            component_metadata: Arc::new(ComponentMetadata::default()),
            extend_attributes: Arc::new(ExtendAttributeIndex::default()),
            module_name: Arc::from(""),
            custom_component_base_name: GeneratedViewpuName::VIEWPU_BASE,
        }
    }
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
    builder_param_methods: FxHashSet<String>,
    custom_component_update_props: CustomComponentUpdateProps,
    component_metadata: Arc<ComponentMetadata>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    raw_object_fields: FxHashSet<String>,
    module_name: Option<Arc<str>>,
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
        builder_param_methods: &FxHashSet<String>,
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
            builder_param_methods,
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
        builder_param_methods: &FxHashSet<String>,
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
            builder_param_methods,
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
        builder_param_methods: &FxHashSet<String>,
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
            builder_param_methods,
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
        builder_param_methods: &FxHashSet<String>,
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
            builder_param_methods,
            custom_component_update_props,
            component_metadata,
            raw_object_fields,
            module_name,
            BuildBodyContextKind::Builder,
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
        builder_param_methods: &FxHashSet<String>,
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
            builder_param_methods: builder_param_methods.clone(),
            custom_component_update_props: custom_component_update_props.clone(),
            component_metadata,
            extend_attributes: Arc::new(ExtendAttributeIndex::default()),
            raw_object_fields: raw_object_fields.clone(),
            module_name,
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

    fn context(&self) -> BuildContext {
        let mut context = match self.context_kind {
            BuildBodyContextKind::Component => BuildContext::default()
                .with_current_component_reusable(self.current_component_reusable),
            BuildBodyContextKind::Builder => BuildContext::builder(),
        }
        .with_raw_object_fields(&self.raw_object_fields);
        if let Some(module_name) = &self.module_name {
            context = context.with_module_name(module_name);
        }
        context
            .with_shared_component_metadata(Arc::clone(&self.component_metadata))
            .with_shared_extend_attributes(Arc::clone(&self.extend_attributes))
            .with_custom_component_base_name(self.custom_component_base_name)
    }

    fn source_metadata(&self) -> BuildSourceMetadata<'_> {
        BuildSourceMetadata::new(&self.source_text, &self.source_id)
    }
}

#[derive(Clone, Copy, Debug)]
enum BuildBodyContextKind {
    Component,
    Builder,
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
                    self.push_statement(statements, statement, allocator, context)?;
                }
                Ok(())
            }
            _ => self.push_statement(statements, statement, allocator, context),
        }
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
        if self.plan.builder_methods.contains(&name) {
            if !component.children.is_empty() {
                return Err(BuildLoweringError::UnsupportedChild);
            }
            statements.push(
                BuilderMethodCallEmitter::new(
                    BuilderMethodReceiver::Direct,
                    &name,
                    &component.arguments,
                    self.plan.builder_param_methods.contains(&name),
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
            let mut chain_attrs = Vec::new();
            for chain in &component.chain_expressions {
                let name = ArkuiCallName::chain(chain)?;
                if context.is_reuse_id_attribute_name(&name) {
                    continue;
                }
                chain_attrs.push(CommonAttribute {
                    name,
                    arguments: chain.arguments.iter().collect(),
                });
            }
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
        if component_kind.is_list_item()
            && !component.children.is_empty()
            && IterationComponent::new(component).is_lazy_list_item()
        {
            return self.push_lazy_list_item(statements, component, allocator, context);
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
        self.push_component_chain_attrs(
            &mut creation_stmts,
            ast,
            allocator,
            &name,
            component,
            context,
        )?;
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                &name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        for child in &component.children {
            self.push_child(statements, child, allocator, context)?;
        }
        if self.plan.component_metadata.needs_pop(&name) {
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
        for child in &component.children {
            self.push_child(&mut body, child, allocator, context)?;
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
        self.push_component_chain_attrs(
            &mut creation_stmts,
            ast,
            allocator,
            name,
            component,
            context,
        )?;
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
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
        if self.plan.component_metadata.needs_pop(name) {
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
        if self.plan.custom_components.contains(&lc.name) {
            let source_metadata = self.plan.source_metadata();
            let base_call = LeafComponent::base_call(call);
            let reuse_id = CustomReuseId::from_leaf_attrs(&lc.chain_attrs, context);
            let is_reusable_reuse_id_only = self
                .plan
                .custom_component_update_props
                .get(&lc.name)
                .is_some_and(|info| info.reusable && lc.is_reuse_id_only(context));
            if lc.chain_attrs.is_empty() || is_reusable_reuse_id_only {
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
                let chain_attrs = CommonAttributeList::from_leaf(&lc.chain_attrs);
                let common_attrs = chain_attrs
                    .attrs
                    .into_iter()
                    .filter(|attr| !context.is_reuse_id_attribute_name(&attr.name))
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
            &lc.chain_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                ast,
                context.use_parent_context,
                &lc.name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        if self.plan.component_metadata.needs_pop(&lc.name) {
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
        self.push_lazy_arrow_body(&mut statements, arrow, allocator, context)?;
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

    fn push_component_chain_attrs<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        component: &ArkUIComponentExpression<'_>,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let mut has_animation = false;
        for chain in &component.chain_expressions {
            let name = ArkuiCallName::chain(chain)?;
            has_animation |= ComponentCallStatementEmitter::is_animation_attribute(&name);
        }
        if has_animation {
            let mut animation_segment = OxcVec::new_in(&allocator);
            for chain in &component.chain_expressions {
                let name = ArkuiCallName::chain(chain)?;
                if ComponentCallStatementEmitter::is_animation_attribute(&name) {
                    ComponentCallStatementEmitter::push_animation_segment(
                        statements,
                        &mut animation_segment,
                        ast,
                        allocator,
                        &chain.arguments,
                        context,
                    );
                    continue;
                }
                if context.is_gesture_attribute_name(&name) {
                    if let Some(gesture_call) = chain
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .and_then(|expression| {
                            if let Expression::CallExpression(call) = expression {
                                Some(call)
                            } else {
                                None
                            }
                        })
                        .and_then(|call| GestureChain::from_call(call, context))
                    {
                        gesture_call.push_sequence_statements(
                            &mut animation_segment,
                            ast,
                            allocator,
                            context,
                        );
                        continue;
                    }
                }
                animation_segment.push(ComponentCallStatementEmitter::attribute_with_source(
                    ast,
                    allocator,
                    component_name,
                    &name,
                    &chain.arguments,
                    &self.plan.builder_methods,
                    context,
                    component.callee.span(),
                    ArkuiCallName::chain_span(chain),
                ));
            }
            ComponentCallStatementEmitter::flush_animation_segment(
                statements,
                &mut animation_segment,
                allocator,
            );
            return Ok(());
        }
        let mut immutable_stmts = OxcVec::new_in(&allocator);
        for chain in &component.chain_expressions {
            let name = ArkuiCallName::chain(chain)?;
            if context.is_gesture_attribute_name(&name) {
                if let Some(gesture_call) = chain
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|expression| {
                        if let Expression::CallExpression(call) = expression {
                            Some(call)
                        } else {
                            None
                        }
                    })
                    .and_then(|call| GestureChain::from_call(call, context))
                {
                    gesture_call.push_sequence_statements(statements, ast, allocator, context);
                    continue;
                }
            }
            let attr_statement = ComponentCallStatementEmitter::attribute_with_source(
                ast,
                allocator,
                component_name,
                &name,
                &chain.arguments,
                &self.plan.builder_methods,
                context,
                component.callee.span(),
                ArkuiCallName::chain_span(chain),
            );
            if context.current_component_reusable
                && RegularAttributeExpression::new(&self.plan.component_metadata)
                    .all_arguments(&chain.arguments)
            {
                immutable_stmts.push(attr_statement);
            } else {
                statements.push(attr_statement);
            }
        }
        if !immutable_stmts.is_empty() {
            statements.push(ast.statement_if(
                SPAN,
                ast.expression_identifier(
                    SPAN,
                    ast.str(&context.runtime_names().is_initial_render),
                ),
                ast.statement_block(SPAN, immutable_stmts),
                None,
            ));
        }
        Ok(())
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
        self.push_lazy_arrow_body(&mut item_body, item_gen, allocator, context)?;
        statements.push(IterationAstStatement::const_arrow_function_with_params(
            ast,
            &context.runtime_names().lazy_foreach_item_gen_function,
            IterationFormalParameters::from_source(ast, allocator, &item_gen.params),
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
            ast, allocator, context,
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
                    self.plan.builder_param_methods.contains(&name),
                    context,
                )
                .statement(AstBuilder::new(allocator), allocator),
            );
            return true;
        }
        if let Expression::Identifier(identifier) = &call.callee {
            let name = identifier.name.as_str();
            if self.plan.builder_methods.contains(name) {
                statements.push(
                    BuilderMethodCallEmitter::new(
                        BuilderMethodReceiver::Direct,
                        name,
                        &call.arguments,
                        self.plan.builder_param_methods.contains(name),
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
            None if branch_index == 0 => Some(self.empty_if_branch_update_block(ast, 1, context)),
            None => None,
        };
        Ok(ast.statement_if(
            SPAN,
            statement.test.clone_in(allocator),
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
        ast.statement_block(
            SPAN,
            ast.vec1(self.if_else_branch_update_statement(ast, branch_index, ast.vec(), context)),
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

    fn with_current_component_reusable(mut self, reusable: bool) -> Self {
        self.current_component_reusable = reusable;
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

/// Extracted representation of a gesture call and its attribute chain.
/// Used to emit `Gesture.create(priority)` / `XxxGesture.create(args)` /
/// `XxxGesture.onX(...)` / `XxxGesture.pop()` / `Gesture.pop()` sequences.
///
/// For `GestureGroup`, `create_args` holds only the first argument (the
/// `GestureMode`) and `children` holds the expanded child gesture chains.
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

struct LeafAttribute<'a> {
    name: String,
    arguments: Vec<&'a Argument<'a>>,
    gesture: Option<GestureChain<'a>>,
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
                        let gesture = context
                            .is_gesture_attribute_name(&attr_name)
                            .then(|| {
                                call.arguments
                                    .first()
                                    .and_then(Argument::as_expression)
                                    .and_then(|expression| {
                                        if let Expression::CallExpression(gesture_call) = expression
                                        {
                                            GestureChain::from_call(gesture_call, context)
                                        } else {
                                            None
                                        }
                                    })
                            })
                            .flatten();
                        leaf.chain_attrs.push(LeafAttribute {
                            name: attr_name,
                            arguments: call.arguments.iter().collect(),
                            gesture,
                        });
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

    fn is_reuse_id_only(&self, context: &BuildContext) -> bool {
        self.chain_attrs
            .iter()
            .all(|attr| context.is_reuse_id_attribute_name(&attr.name))
    }
}

struct CommonAttribute<'a> {
    name: String,
    arguments: Vec<&'a Argument<'a>>,
}

struct CommonAttributeList<'a> {
    attrs: Vec<CommonAttribute<'a>>,
}

impl<'a> CommonAttributeList<'a> {
    fn from_leaf(attrs: &[LeafAttribute<'a>]) -> Self {
        Self {
            attrs: attrs
                .iter()
                .map(|attr| CommonAttribute {
                    name: attr.name.clone(),
                    arguments: attr.arguments.clone(),
                })
                .collect(),
        }
    }
}

struct LeafAttributeStatementEmitter<'a, 'ctx> {
    component_name: &'a str,
    attrs: &'a [LeafAttribute<'a>],
    builder_methods: &'a FxHashSet<String>,
    context: &'ctx BuildContext,
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
        }
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
            for attr in self.attrs {
                if ComponentCallStatementEmitter::is_animation_attribute(&attr.name) {
                    ComponentCallStatementEmitter::push_animation_ref_segment(
                        stmts,
                        &mut animation_segment,
                        ast,
                        allocator,
                        &attr.arguments,
                        self.context,
                    );
                    continue;
                }
                if let Some(gesture) = &attr.gesture {
                    gesture.push_sequence_statements(
                        &mut animation_segment,
                        ast,
                        allocator,
                        self.context,
                    );
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
            }
            ComponentCallStatementEmitter::flush_animation_segment(
                stmts,
                &mut animation_segment,
                allocator,
            );
            return;
        }
        for attr in self.attrs {
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
}

struct RegularAttributeExpression<'a> {
    component_metadata: &'a ComponentMetadata,
}

impl<'a> RegularAttributeExpression<'a> {
    fn new(component_metadata: &'a ComponentMetadata) -> Self {
        Self { component_metadata }
    }

    fn all_arguments(&self, arguments: &[Argument<'_>]) -> bool {
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

    fn chain_span(call: &CallExpression<'_>) -> Span {
        match &call.callee {
            Expression::StaticMemberExpression(member) => member.property.span,
            _ => SPAN,
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
            if let Some(expression) = self.builder_attribute_expression(expression, true, allocator)
            {
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
                        false,
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
            return self.builder_initializer_expression(
                expression,
                self.attr_name,
                false,
                allocator,
            );
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
                    self.builder_initializer_expression(
                        &property.value,
                        self.attr_name,
                        false,
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

    fn builder_initializer_expression<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allow_any_this_member: bool,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        self.builder_attribute_expression_for_attr(
            expression,
            attr_name,
            allow_any_this_member,
            allocator,
        )
        .unwrap_or_else(|| self.plain_expression(expression, allocator))
    }

    fn builder_attribute_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allow_any_this_member: bool,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        self.builder_attribute_expression_for_attr(
            expression,
            self.attr_name,
            allow_any_this_member,
            allocator,
        )
    }

    fn builder_attribute_expression_for_attr<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allow_any_this_member: bool,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Expression::ConditionalExpression(conditional) = expression {
            let ast = AstBuilder::new(allocator);
            let consequent = self
                .builder_attribute_expression_for_attr(
                    &conditional.consequent,
                    attr_name,
                    allow_any_this_member,
                    allocator,
                )
                .unwrap_or_else(|| self.plain_expression(&conditional.consequent, allocator));
            let alternate = self
                .builder_attribute_expression_for_attr(
                    &conditional.alternate,
                    attr_name,
                    allow_any_this_member,
                    allocator,
                )
                .unwrap_or_else(|| self.plain_expression(&conditional.alternate, allocator));
            return Some(ast.expression_conditional(
                conditional.span,
                self.plain_expression(&conditional.test, allocator),
                consequent,
                alternate,
            ));
        }
        let without_key = self.context.is_builder_attribute_without_key(attr_name);
        if let Some(target) =
            self.builder_reference_expression(expression, allow_any_this_member, allocator)
        {
            let bound = self.bound_expression(target, AstBuilder::new(allocator).vec(), allocator);
            return Some(if without_key {
                bound
            } else {
                self.builder_object_expression(bound, allocator)
            });
        }
        if let Expression::CallExpression(call) = expression {
            if let Some((callee, builder_name)) =
                self.builder_call_target(call, allow_any_this_member, allocator)
            {
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
        allow_any_this_member: bool,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Some(name) = ArkuiCallName::this_member(expression) {
            if allow_any_this_member || self.builder_methods.contains(name.as_str()) {
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
        allow_any_this_member: bool,
        allocator: &'b Allocator,
    ) -> Option<(Expression<'b>, String)> {
        if let Some(name) = ArkuiCallName::this_member(&call.callee) {
            if allow_any_this_member || self.builder_methods.contains(name.as_str()) {
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
        if let Some(target) = self.builder_reference_expression(expression, false, allocator) {
            return Some(self.bound_expression(
                target,
                AstBuilder::new(allocator).vec(),
                allocator,
            ));
        }
        if let Expression::CallExpression(call) = expression {
            if let Some((callee, builder_name)) = self.builder_call_target(call, false, allocator) {
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
        AstBuilder::new(allocator).vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
        )
    }

    fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        AstBuilder::new(allocator).vec_from_iter(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
        )
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

    fn push_animation_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[Argument<'_>],
        context: &BuildContext,
    ) {
        statements.push(Self::animation(ast, allocator, arguments, context));
        Self::flush_animation_segment(statements, segment, allocator);
        statements.push(Self::clear_animation(ast, context));
    }

    fn push_animation_ref_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        ast: AstBuilder<'b>,
        allocator: &'b Allocator,
        arguments: &[&Argument<'_>],
        context: &BuildContext,
    ) {
        statements.push(Self::animation_refs(ast, allocator, arguments, context));
        Self::flush_animation_segment(statements, segment, allocator);
        statements.push(Self::clear_animation(ast, context));
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
    builder_param: bool,
    context: &'a BuildContext,
}

impl<'a> BuilderMethodCallEmitter<'a> {
    fn new(
        receiver: BuilderMethodReceiver,
        name: &'a str,
        arguments: &'a [Argument<'a>],
        builder_param: bool,
        context: &'a BuildContext,
    ) -> Self {
        Self {
            receiver,
            name,
            arguments,
            builder_param,
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
        if self.builder_param {
            if let Some(argument) = self.proxy_argument(ast, allocator) {
                return ast.vec1(argument);
            }
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .arguments_with_allocator(self.arguments, allocator)
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
        if let Some(name) = Self::this_static_member_name(value) {
            return self.this_property_fallback(name, allocator);
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .expression_with_allocator(value, allocator)
    }

    fn this_static_member_name<'b>(value: &'b Expression<'_>) -> Option<&'b str> {
        let Expression::StaticMemberExpression(member) = value else {
            return None;
        };
        matches!(&member.object, Expression::ThisExpression(_))
            .then_some(member.property.name.as_str())
    }

    fn this_property_fallback<'b>(&self, name: &str, allocator: &'b Allocator) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let backing = self.this_computed_member(ast, &format!("__{name}"));
        let consequent = self.this_computed_member(ast, &format!("__{name}"));
        let alternate = self.this_computed_member(ast, name);
        ast.expression_conditional(SPAN, backing, consequent, alternate)
    }

    fn this_computed_member<'b>(&self, ast: AstBuilder<'b>, name: &str) -> Expression<'b> {
        Expression::from(ast.member_expression_computed(
            SPAN,
            ast.expression_this(SPAN),
            ast.expression_string_literal(SPAN, ast.str(name), None),
            false,
        ))
    }
}

#[cfg(test)]
mod tests;
