use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt, sync::Arc};

use oxc_allocator::{Allocator, ArenaBox, CloneIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrayExpressionElement, ArrowFunctionBody,
    ArrowFunctionExpression, AssignmentTarget, BindingPattern, CallExpression, ChainElement,
    ChainExpression, Expression, FormalParameter, FormalParameterKind, FormalParameters,
    FunctionBody, IdentifierName, IfStatement, MemberExpression, ObjectExpression, ObjectProperty,
    ObjectPropertyKind, PropertyKey, PropertyKind, SpreadElement, Statement,
    VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::{GetSpan, Span, SPAN};
use oxc_syntax::operator::{AssignmentOperator, UnaryOperator};

use arkdown_path::HvigorModulePath;

use crate::source_map::offset_to_line_col;
use crate::transform::component_analysis::ExtendAttributeIndex;
use crate::transform::component_meta::{
    ArkuiRuntimeComponentKind, ArkuiRuntimeNames, ComponentMetadata,
};
use crate::transform::global_rewrite::GlobalContextCallRewriter;
use crate::transform::local_builder::LocalBuilderRuntimeEmitter;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CustomComponentPropertyKind {
    Local,
    Consumer,
    Provider,
    Regular,
}

impl CustomComponentPropertyKind {
    pub(crate) fn source_name(self) -> &'static str {
        match self {
            Self::Local => "@Local",
            Self::Consumer => "@Consumer",
            Self::Provider => "@Provider",
            Self::Regular => "regular",
        }
    }
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
    pub(crate) param_props: FxHashSet<String>,
    pub(crate) event_props: FxHashSet<String>,
    pub(crate) required_props: Vec<String>,
    pub(crate) forbidden_init_props: FxHashMap<String, CustomComponentPropertyKind>,
    pub(crate) custom_env_props: FxHashSet<String>,
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
    mutable_builder_properties: Arc<FxHashSet<String>>,
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
            mutable_builder_properties: Arc::default(),
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
    pub(super) fn new(source_text: &'a str, source_id: &'a str) -> Self {
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

#[derive(Clone, Debug, Default)]
pub struct BuildBodyInfo<'a> {
    plan: Option<BuildBodyPlan<'a>>,
}

impl<'a> BuildBodyInfo<'a> {
    pub(crate) fn plan(plan: BuildBodyPlan<'a>) -> Self {
        Self { plan: Some(plan) }
    }

    pub(crate) fn statements<'b>(
        &self,
        allocator: &'b Allocator,
    ) -> Result<OxcVec<'b, Statement<'b>>, BuildLoweringError> {
        match &self.plan {
            None => Ok(OxcVec::new_in(&allocator)),
            Some(plan) => plan.statements(allocator),
        }
    }

    pub(crate) fn has_source_statements(&self) -> bool {
        self.plan
            .as_ref()
            .and_then(|plan| plan.body)
            .is_some_and(|body| !body.statements.is_empty())
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
    mutable_builder_properties: Arc<FxHashSet<String>>,
    module_name: Option<Arc<str>>,
    xcomponent_module_identity: Option<Arc<str>>,
    context_kind: BuildBodyContextKind,
    current_component_reusable: bool,
    custom_component_base_name: &'static str,
}

pub(crate) struct BuildBodySharedInputs<'a> {
    pub(crate) source_text: Arc<str>,
    pub(crate) source_id: Arc<str>,
    pub(crate) custom_components: &'a FxHashSet<String>,
    pub(crate) builder_methods: &'a FxHashSet<String>,
    pub(crate) custom_component_update_props: &'a CustomComponentUpdateProps,
    pub(crate) component_metadata: Arc<ComponentMetadata>,
    pub(crate) raw_object_fields: &'a FxHashSet<String>,
    pub(crate) module_name: Option<Arc<str>>,
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
            BuildBodySharedInputs {
                source_text: Arc::from(source_text),
                source_id: Arc::from(source_id),
                custom_components,
                builder_methods,
                custom_component_update_props,
                component_metadata: Arc::new(component_metadata.clone()),
                raw_object_fields,
                module_name: module_name.map(Arc::from),
            },
            current_component_reusable,
            GeneratedViewpuName::VIEW_V2_BASE,
        )
    }

    pub(crate) fn component_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        inputs: BuildBodySharedInputs<'_>,
        current_component_reusable: bool,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self::new(
            body,
            inputs,
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
            BuildBodySharedInputs {
                source_text: Arc::from(source_text),
                source_id: Arc::from(source_id),
                custom_components,
                builder_methods,
                custom_component_update_props,
                component_metadata: Arc::new(component_metadata.clone()),
                raw_object_fields,
                module_name: module_name.map(Arc::from),
            },
            GeneratedViewpuName::VIEW_V2_BASE,
        )
    }

    pub(crate) fn builder_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        inputs: BuildBodySharedInputs<'_>,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self::new(
            body,
            inputs,
            BuildBodyContextKind::Builder,
            false,
            custom_component_base_name,
        )
    }

    pub(crate) fn local_builder_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        inputs: BuildBodySharedInputs<'_>,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self::new(
            body,
            inputs,
            BuildBodyContextKind::LocalBuilder,
            false,
            custom_component_base_name,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn global_builder_with_shared_inputs(
        body: Option<&'a FunctionBody<'a>>,
        inputs: BuildBodySharedInputs<'_>,
        custom_component_base_name: &'static str,
        first_parameter: Option<Arc<str>>,
    ) -> Self {
        Self::new(
            body,
            inputs,
            BuildBodyContextKind::GlobalBuilder { first_parameter },
            false,
            custom_component_base_name,
        )
    }

    fn new(
        body: Option<&'a FunctionBody<'a>>,
        inputs: BuildBodySharedInputs<'_>,
        context_kind: BuildBodyContextKind,
        current_component_reusable: bool,
        custom_component_base_name: &'static str,
    ) -> Self {
        Self {
            body,
            source_text: inputs.source_text,
            source_id: inputs.source_id,
            custom_components: inputs.custom_components.clone(),
            builder_methods: inputs.builder_methods.clone(),
            custom_component_update_props: inputs.custom_component_update_props.clone(),
            component_metadata: inputs.component_metadata,
            extend_attributes: Arc::new(ExtendAttributeIndex::default()),
            styles: StylesAttributeIndex::default(),
            raw_object_fields: inputs.raw_object_fields.clone(),
            mutable_builder_properties: Arc::default(),
            module_name: inputs.module_name,
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
        let mut statements = BuildBodyAstLowerer::new(self).statements(allocator)?;
        if matches!(self.context_kind, BuildBodyContextKind::LocalBuilder)
            && self.body.is_some_and(|body| !body.statements.is_empty())
        {
            statements.insert(
                0,
                LocalBuilderRuntimeEmitter::new().parent_statement(allocator),
            );
        }
        Ok(statements)
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

    pub(crate) fn with_shared_mutable_builder_properties(
        mut self,
        properties: Arc<FxHashSet<String>>,
    ) -> Self {
        self.mutable_builder_properties = properties;
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
            BuildBodyContextKind::LocalBuilder => BuildContext::local_builder(),
            BuildBodyContextKind::GlobalBuilder { first_parameter } => {
                BuildContext::global_builder(first_parameter.clone())
            }
        }
        .with_raw_object_fields(&self.raw_object_fields);
        context = context
            .with_shared_mutable_builder_properties(Arc::clone(&self.mutable_builder_properties));
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
    LocalBuilder,
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
            let ast = &AstBuilder::new(allocator);
            let capture_name = format!("__{parameter}__");
            let binding = BindingPattern::new_binding_identifier(
                SPAN,
                ast.allocator().alloc_str(&capture_name),
                ast,
            );
            let initializer =
                Expression::new_identifier(SPAN, ast.allocator().alloc_str(parameter), ast);
            let declarator =
                VariableDeclarator::new(SPAN, binding, None, Some(initializer), false, ast);
            statements.push(Statement::new_variable_declaration(
                SPAN,
                VariableDeclarationKind::Const,
                OxcVec::from_value_in(declarator, ast),
                false,
                ast,
            ));
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
            &AstBuilder::new(allocator),
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
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        retake_id: &Expression<'_>,
        child_statements: OxcVec<'b, Statement<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let callee = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().component_if),
                ast,
            ),
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().can_retake),
                ast,
            ),
            false,
            ast,
        );
        let retake_id = BuildArgumentEmitter::new()
            .with_context(context)
            .expression_with_allocator(retake_id, allocator);
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(callee),
            None,
            OxcVec::from_value_in(Argument::from(retake_id), ast),
            false,
            ast,
        );
        Statement::new_if_statement(
            SPAN,
            Expression::new_unary_expression(SPAN, UnaryOperator::LogicalNot, call, ast),
            Statement::new_block_statement(SPAN, child_statements, ast),
            None,
            ast,
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
                .statement(&AstBuilder::new(allocator), allocator),
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
        if component_kind.creates_child_body() && !component.children.is_empty() {
            return self
                .push_child_body_component(statements, component, &name, allocator, context);
        }

        let ast = &AstBuilder::new(allocator);
        let mut creation_stmts = OxcVec::new_in(&allocator);
        creation_stmts.push(ComponentCallStatementEmitter::create(
            ast,
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
            component.callee.span(),
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
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

    /// Mirrors ets-loader `process_component_build.js::processTabAndNav` for
    /// `TabContent`: nested ArkUI statements belong to the `create` builder
    /// callback, rather than following the creation observer as siblings.
    fn push_child_body_component<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        component: &ArkUIComponentExpression<'_>,
        name: &str,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = &AstBuilder::new(allocator);
        let mut child_body = OxcVec::new_in(&allocator);
        for child in &component.children {
            self.push_child(&mut child_body, child, allocator, context)?;
        }

        let mut creation_stmts = OxcVec::new_in(&allocator);
        creation_stmts.push(ComponentCallStatementEmitter::create_with_child_body(
            ast,
            name,
            &context.runtime_names().method_create,
            child_body,
            component.callee.span(),
        ));
        let resolved_attrs = self.component_chain_attributes(component, context)?;
        LeafAttributeStatementEmitter::new(
            name,
            component.callee.span(),
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                name,
                context.runtime_names(),
                creation_stmts,
            ),
        );
        if context.component_needs_pop(name, component.arguments.iter()) {
            statements.push(ComponentCallStatementEmitter::empty_with_source(
                ast,
                name,
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
        let ast = &AstBuilder::new(allocator);
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
            component.callee.span(),
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
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
        let ast = &AstBuilder::new(allocator);
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
            lc.component_span,
            &resolved_attrs,
            &self.plan.builder_methods,
            context,
        )
        .push_statements(&mut creation_stmts, ast, allocator);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
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
        let ast = &AstBuilder::new(allocator);
        let Some(repeat_expression) = self.repeat_call_expression(call, allocator, context)? else {
            return Ok(false);
        };
        let render_callee = MemberExpression::new_static_member_expression(
            SPAN,
            repeat_expression,
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().method_render),
                ast,
            ),
            false,
            ast,
        );
        let render_call = Expression::new_call_expression(
            SPAN,
            Expression::from(render_callee),
            None,
            OxcVec::from_value_in(
                Argument::from(Expression::new_identifier(
                    SPAN,
                    ast.allocator()
                        .alloc_str(&context.runtime_names().is_initial_render),
                    ast,
                )),
                ast,
            ),
            false,
            ast,
        );
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
                context.use_parent_context,
                context.global_builder_parameter.as_deref(),
                &context.runtime_names().component_repeat,
                context.runtime_names(),
                OxcVec::from_value_in(
                    Statement::new_expression_statement(SPAN, render_call, ast),
                    ast,
                ),
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
        let ast = &AstBuilder::new(allocator);
        match expression {
            Expression::ParenthesizedExpression(parenthesized) => self
                .repeat_expression(&parenthesized.expression, allocator, context)
                .map(|expression| {
                    expression.map(|expression| {
                        Expression::new_parenthesized_expression(SPAN, expression, ast)
                    })
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
        let ast = &AstBuilder::new(allocator);
        if self.is_repeat_base_call(call, context) {
            let mut arguments = OxcVec::new_in(&allocator);
            for argument in &call.arguments {
                arguments.push(
                    BuildArgumentEmitter::new()
                        .with_context(context)
                        .argument_with_allocator(argument, allocator),
                );
            }
            arguments.push(Argument::from(Expression::new_this_expression(SPAN, ast)));
            let callee = call.callee.clone_in(allocator);
            let type_arguments = call
                .type_arguments
                .as_ref()
                .map(|type_arguments| type_arguments.clone_in(allocator));
            return Ok(Some(Expression::new_call_expression(
                call.span,
                callee,
                type_arguments,
                arguments,
                call.optional,
                ast,
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
        let callee = MemberExpression::new_static_member_expression(
            member.span,
            object,
            member.property.clone_in(allocator),
            member.optional,
            ast,
        );
        let type_arguments = call
            .type_arguments
            .as_ref()
            .map(|type_arguments| type_arguments.clone_in(allocator));
        Ok(Some(Expression::new_call_expression(
            call.span,
            Expression::from(callee),
            type_arguments,
            arguments,
            call.optional,
            ast,
        )))
    }

    fn repeat_method_arguments<'b>(
        &self,
        call: &CallExpression<'_>,
        method_name: &str,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<OxcVec<'b, Argument<'b>>, BuildLoweringError> {
        let ast = &AstBuilder::new(allocator);
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
        Ok(OxcVec::from_iter_in(arguments, ast))
    }

    fn repeat_arrow_function<'b>(
        &self,
        arrow: &ArrowFunctionExpression<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<Expression<'b>, BuildLoweringError> {
        let ast = &AstBuilder::new(allocator);
        let mut statements = OxcVec::new_in(&allocator);
        let repeat_context = context.clone().with_inside_repeat();
        self.push_lazy_arrow_body(&mut statements, arrow, allocator, &repeat_context)?;
        let params = IterationFormalParameters::from_source(ast, allocator, &arrow.params);
        let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), statements, ast);
        Ok(Expression::new_arrow_function_expression(
            arrow.span,
            arrow.r#async,
            arrow
                .type_parameters
                .as_ref()
                .map(|type_parameters| type_parameters.clone_in(allocator)),
            ArenaBox::new_in(params, ast),
            arrow
                .return_type
                .as_ref()
                .map(|return_type| return_type.clone_in(allocator)),
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, ast)),
            ast,
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
                let attribute_span = match &chain.callee {
                    Expression::StaticMemberExpression(member) => member.property.span,
                    _ => SPAN,
                };
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
                        attribute_span,
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
        let ast = &AstBuilder::new(allocator);
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
                AstBuilder::new(allocator),
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

        let ast = &AstBuilder::new(allocator);
        // SDK process_component_build.js scopes every generated LazyForEach
        // declaration in its own block. Besides preventing generated names
        // from leaking into sibling lowering output, that lexical scope is
        // observable in es2abc register allocation.
        let mut lazy_for_each_body = OxcVec::new_in(&allocator);
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
        lazy_for_each_body.push(IterationAstStatement::const_arrow_function_with_params(
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
            lazy_for_each_body.push(IterationAstStatement::const_declaration(
                ast,
                &context.runtime_names().lazy_foreach_item_id_func,
                id_func,
            ));
            true
        } else {
            false
        };
        lazy_for_each_body.push(IterationAstStatement::lazy_for_each_create(
            ast,
            allocator,
            context,
            data,
            has_id_func,
        ));
        lazy_for_each_body.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_lazy_for_each,
            &context.runtime_names().method_pop,
        ));
        statements.push(Statement::new_block_statement(
            SPAN,
            lazy_for_each_body,
            ast,
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
        let ast = &AstBuilder::new(allocator);
        let resolved_attrs = self.component_chain_attributes(component, context)?;
        let mut block_body = OxcVec::new_in(&allocator);
        block_body.push(IterationAstStatement::lazy_list_item_creation(
            ast, allocator, component, context,
        ));
        block_body.push(
            IterationComponent::new(component).lazy_list_item_creation2_statement(
                ast,
                allocator,
                &resolved_attrs,
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
            // process_component_build.js uses the lexical `this` for the
            // ListItem itemCreation2 observer, even when its surrounding
            // global @Builder emits other observers through `parent`.
            Expression::new_this_expression(SPAN, ast),
            &context.runtime_names().observe_component_creation2,
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&context.runtime_names().item_creation2),
                        ast,
                    )),
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&context.runtime_names().component_list_item),
                        ast,
                    )),
                ],
                ast,
            ),
        ));
        block_body.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_list_item,
            &context.runtime_names().method_pop,
        ));
        statements.push(Statement::new_block_statement(SPAN, block_body, ast));
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
        let ast = &AstBuilder::new(allocator);
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
            component.callee.span(),
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
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&context.runtime_names().item_creation2),
                        ast,
                    )),
                    Argument::from(Expression::new_identifier(
                        SPAN,
                        ast.allocator().alloc_str(component_name),
                        ast,
                    )),
                ],
                ast,
            ),
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
        statements.push(Statement::new_block_statement(SPAN, block_body, ast));
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
        if let oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) = &arrow.body {
            for statement in &body.statements {
                self.push_statement(statements, statement, allocator, context)?;
            }
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
                .statement(&AstBuilder::new(allocator), allocator),
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
                    .statement(&AstBuilder::new(allocator), allocator),
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
        statements.push({
            let b = AstBuilder::new(allocator);
            Statement::new_expression_statement(SPAN, expression, &b)
        });
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
        let cloned_chain = {
            let b = AstBuilder::new(allocator);
            ArenaBox::new_in(cloned_chain, &b)
        };
        let expression = BuildArgumentEmitter::new()
            .with_context(context)
            .owned_expression_with_allocator(Expression::ChainExpression(cloned_chain), allocator);
        statements.push({
            let b = AstBuilder::new(allocator);
            Statement::new_expression_statement(SPAN, expression, &b)
        });
        true
    }

    fn push_if_statement<'b>(
        &self,
        statements: &mut OxcVec<'b, Statement<'b>>,
        statement: &IfStatement<'_>,
        allocator: &'b Allocator,
        context: &BuildContext,
    ) -> Result<(), BuildLoweringError> {
        let ast = &AstBuilder::new(allocator);
        let mut observe_stmts = OxcVec::new_in(&allocator);
        observe_stmts.push(ComponentCallStatementEmitter::empty(
            ast,
            &context.runtime_names().component_if,
            &context.runtime_names().method_create,
        ));
        observe_stmts.push(self.if_chain_statement(ast, allocator, statement, context, 0)?);
        statements.push(
            ViewpuStatementAstEmitter::observe_component_creation_ast_statement(
                AstBuilder::new(allocator),
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
        ast: &AstBuilder<'b>,
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
        Ok(Statement::new_if_statement(
            SPAN,
            BuildArgumentEmitter::new()
                .with_context(context)
                .with_nested_raw_object_rewrite()
                .expression_with_allocator(&statement.test, allocator),
            consequent,
            alternate,
            ast,
        ))
    }

    fn if_branch_update_block<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        source_statement: &Statement<'_>,
        context: &BuildContext,
        branch_index: usize,
    ) -> Result<Statement<'b>, BuildLoweringError> {
        let mut branch_stmts = OxcVec::new_in(&allocator);
        self.push_branch_statement(&mut branch_stmts, source_statement, allocator, context)?;
        Ok(Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(
                self.if_else_branch_update_statement(ast, branch_index, branch_stmts, context),
                ast,
            ),
            ast,
        ))
    }

    fn empty_if_branch_update_block<'b>(
        &self,
        ast: &AstBuilder<'b>,
        branch_index: usize,
        context: &BuildContext,
    ) -> Statement<'b> {
        // ets-loader's processElseStatement synthesizes the terminal empty
        // branch without forwarding createConditionParent(). In a top-level
        // @Builder this deliberately keeps the lexical `this` receiver even
        // though source-backed branches use `(parent ? parent : this)`.
        let mut lexical_context = context.clone();
        lexical_context.use_parent_context = false;
        Statement::new_block_statement(
            SPAN,
            OxcVec::from_value_in(
                self.if_else_branch_update_statement(
                    ast,
                    branch_index,
                    OxcVec::new_in(ast),
                    &lexical_context,
                ),
                ast,
            ),
            ast,
        )
    }

    fn if_else_branch_update_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        branch_index: usize,
        body_stmts: OxcVec<'b, Statement<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), body_stmts, ast);
        let arrow = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(
                FormalParameters::new(
                    SPAN,
                    FormalParameterKind::ArrowFormalParameters,
                    OxcVec::new_in(ast),
                    None,
                    ast,
                ),
                ast,
            ),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, ast)),
            ast,
        );
        let callee = MemberExpression::new_static_member_expression(
            SPAN,
            self.view_context_expression(ast, context),
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().if_else_branch_update_function),
                ast,
            ),
            false,
            ast,
        );
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(callee),
            None,
            OxcVec::from_iter_in(
                [
                    Argument::from(Expression::new_numeric_literal(
                        SPAN,
                        branch_index as f64,
                        None,
                        oxc_ast::ast::NumberBase::Decimal,
                        ast,
                    )),
                    Argument::from(arrow),
                ],
                ast,
            ),
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, call, ast)
    }

    fn view_context_expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        context: &BuildContext,
    ) -> Expression<'b> {
        if !context.use_parent_context {
            return Expression::new_this_expression(SPAN, ast);
        }
        let test = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let consequent = Expression::new_identifier(SPAN, GeneratedViewpuName::PARENT, ast);
        let alternate = Expression::new_this_expression(SPAN, ast);
        Expression::new_parenthesized_expression(
            SPAN,
            Expression::new_conditional_expression(SPAN, test, consequent, alternate, ast),
            ast,
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

    fn local_builder() -> Self {
        Self {
            use_parent_context: true,
            builder_proxy_scope: BuilderProxyScope::Builder,
            ..Default::default()
        }
    }

    fn with_raw_object_fields(mut self, raw_object_fields: &FxHashSet<String>) -> Self {
        self.raw_object_fields = Arc::new(raw_object_fields.clone());
        self
    }

    fn with_shared_mutable_builder_properties(
        mut self,
        properties: Arc<FxHashSet<String>>,
    ) -> Self {
        self.mutable_builder_properties = properties;
        self
    }

    fn is_mutable_builder_receiver(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::CallExpression(call) => {
                matches!(
                    &call.callee,
                    Expression::Identifier(identifier) if identifier.name == "mutableBuilder"
                )
            }
            Expression::StaticMemberExpression(member) => {
                matches!(&member.object, Expression::ThisExpression(_))
                    && self
                        .mutable_builder_properties
                        .contains(member.property.name.as_str())
            }
            _ => false,
        }
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

    fn with_mutable_builder_proxy_scope(mut self) -> Self {
        if self.builder_proxy_scope != BuilderProxyScope::GlobalBuilder {
            // `transferMutableBuilderCall` invokes
            // `traverseBuilderParams(object, true)`, which enables backing
            // property lookup for both `this.foo` and `$$.foo` outside a
            // global builder.
            self.builder_proxy_scope = BuilderProxyScope::Builder;
        }
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

    fn is_style_double_exclamation_attr(&self, attr_name: &str) -> bool {
        self.component_metadata
            .is_style_double_exclamation_attr(attr_name)
    }

    fn is_component_double_exclamation_property(
        &self,
        component_name: &str,
        property_name: &str,
    ) -> bool {
        self.component_metadata
            .is_component_double_exclamation_property(component_name, property_name)
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
        ast: &AstBuilder<'b>,
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
        ast: &AstBuilder<'b>,
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

mod attribute_emit;
mod builder_emit;

use attribute_emit::{
    ArkuiCallName, ComponentCallStatementEmitter, LeafAttribute, LeafAttributeStatementEmitter,
    LeafComponent, StylesAttributeList,
};
use builder_emit::{
    BuildArgumentEmitter, BuilderDispatchCallEmitter, BuilderMethodCallEmitter,
    BuilderMethodReceiver, NavigationCreateCall,
};

#[cfg(test)]
mod tests;
