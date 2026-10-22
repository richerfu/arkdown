//! ArkUI leaf attribute/component-call emission helpers.

use super::builder_emit::BuilderParamProxyObjectEmitter;
use super::*;
use crate::text::StaticPropertyNameText;

/// Extracted representation of a leaf component call and its attribute chain.
pub(super) struct LeafComponent<'a> {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) component_span: Span,
    pub(super) create_arguments: Vec<&'a Argument<'a>>,
    pub(super) chain_attrs: Vec<LeafAttribute<'a>>,
}

#[derive(Clone)]
pub(super) struct LeafAttribute<'a> {
    pub(super) name: String,
    pub(super) attribute_span: Span,
    pub(super) arguments: Vec<&'a Argument<'a>>,
    pub(super) gesture: Option<GestureChain<'a>>,
    pub(super) state_styles: Option<StateStylesAttributeList<'a>>,
}

impl<'a> LeafAttribute<'a> {
    pub(super) fn from_call(
        name: String,
        call: &'a CallExpression<'a>,
        context: &BuildContext,
    ) -> Self {
        let attribute_span = match &call.callee {
            Expression::Identifier(identifier) => identifier.span,
            Expression::StaticMemberExpression(member) => member.property.span,
            _ => SPAN,
        };
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
            attribute_span,
            arguments: call.arguments.iter().collect(),
            gesture,
            state_styles: None,
        }
    }

    pub(super) fn with_state_styles(
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
pub(super) struct StateStyle<'a> {
    pub(super) name: String,
    pub(super) attrs: Vec<LeafAttribute<'a>>,
}

#[derive(Clone)]
pub(super) struct StateStylesAttributeList<'a> {
    pub(super) states: Vec<StateStyle<'a>>,
}

impl<'a> StateStylesAttributeList<'a> {
    pub(super) fn from_arguments(
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
                name: name.to_owned_text(),
                attrs: Self::state_attributes(&property.value, styles, context)?,
            });
        }
        Ok(Self { states })
    }

    pub(super) fn state_attributes(
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
            let mut attrs = Vec::new();
            for attr in StylesAttributeList::from_body(body, context)?.attrs {
                attrs.push(attr.with_state_styles(styles, context)?);
            }
            return Ok(attrs);
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

pub(super) struct StylesAttributeList<'a> {
    pub(super) attrs: Vec<LeafAttribute<'a>>,
}

impl<'a> StylesAttributeList<'a> {
    pub(super) fn from_body(
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

    pub(super) fn from_expression(
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

    pub(super) fn push_call_attributes(
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
    pub(super) fn from_call(
        call: &'a CallExpression<'a>,
        context: &BuildContext,
    ) -> Result<Self, BuildLoweringError> {
        match &call.callee {
            Expression::Identifier(_) => {
                let name = ArkuiCallName::component(&call.callee)?;
                Ok(Self {
                    path: name.clone(),
                    name,
                    component_span: call.callee.span(),
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
                    _ => Ok(Self {
                        name: attr_name,
                        path: ArkuiCallName::component_path(&call.callee)?,
                        component_span: call.callee.span(),
                        create_arguments: call.arguments.iter().collect(),
                        chain_attrs: vec![],
                    }),
                }
            }
            _ => Err(BuildLoweringError::UnsupportedCallee),
        }
    }

    pub(super) fn base_call(call: &'a CallExpression<'a>) -> &'a CallExpression<'a> {
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

pub(super) struct LeafAttributeStatementEmitter<'a, 'ctx> {
    pub(super) component_name: &'a str,
    pub(super) component_span: Span,
    pub(super) attrs: &'a [LeafAttribute<'a>],
    pub(super) builder_methods: &'a FxHashSet<String>,
    pub(super) context: &'ctx BuildContext,
    pub(super) use_reusable_immutable_guard: bool,
}

impl<'a, 'ctx> LeafAttributeStatementEmitter<'a, 'ctx> {
    pub(super) fn new(
        component_name: &'a str,
        component_span: Span,
        attrs: &'a [LeafAttribute<'a>],
        builder_methods: &'a FxHashSet<String>,
        context: &'ctx BuildContext,
    ) -> Self {
        Self {
            component_name,
            component_span,
            attrs,
            builder_methods,
            context,
            use_reusable_immutable_guard: true,
        }
    }

    pub(super) fn without_reusable_immutable_guard(mut self) -> Self {
        self.use_reusable_immutable_guard = false;
        self
    }

    pub(super) fn push_statements<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        ast: &AstBuilder<'b>,
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
            let statement = ComponentCallStatementEmitter::attribute_refs_with_source(
                ast,
                allocator,
                self.component_name,
                &attr.name,
                &attr.arguments,
                self.builder_methods,
                self.context,
                self.component_span,
                attr.attribute_span,
            );
            if self.use_reusable_immutable_guard
                && self.context.current_component_reusable
                && RegularAttributeExpression::new(self.context).all_argument_refs(&attr.arguments)
            {
                immutable_stmts.push(statement);
            } else {
                stmts.push(statement);
            }
        }
        if !immutable_stmts.is_empty() {
            stmts.push(Statement::new_if_statement(
                SPAN,
                Expression::new_identifier(
                    SPAN,
                    ast.allocator()
                        .alloc_str(&self.context.runtime_names().is_initial_render),
                    ast,
                ),
                Statement::new_block_statement(SPAN, immutable_stmts, ast),
                None,
                ast,
            ));
        }
    }

    pub(super) fn push_state_styles<'b>(
        &self,
        stmts: &mut OxcVec<'b, Statement<'b>>,
        state_styles: &StateStylesAttributeList<'a>,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) {
        for state in &state_styles.states {
            stmts.push(self.visual_state_statement(ast, Some(&state.name)));
            for attr in &state.attrs {
                if let Some(nested_state_styles) = &attr.state_styles {
                    self.push_state_styles(stmts, nested_state_styles, ast, allocator);
                    continue;
                }
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

    pub(super) fn visual_state_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        state: Option<&str>,
    ) -> Statement<'b> {
        let runtime_names = self.context.runtime_names();
        let callee = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&runtime_names.view_stack_processor),
                ast,
            ),
            IdentifierName::new(
                SPAN,
                ast.allocator().alloc_str(&runtime_names.visual_state),
                ast,
            ),
            false,
            ast,
        );
        let arguments = state.map_or_else(
            || OxcVec::new_in(ast),
            |state| {
                OxcVec::from_value_in(
                    Argument::from(Expression::new_string_literal(
                        SPAN,
                        ast.allocator().alloc_str(state),
                        None,
                        ast,
                    )),
                    ast,
                )
            },
        );
        Statement::new_expression_statement(
            SPAN,
            Expression::new_call_expression(
                SPAN,
                Expression::from(callee),
                None,
                arguments,
                false,
                ast,
            ),
            ast,
        )
    }
}

pub(super) struct RegularAttributeExpression<'a> {
    context: &'a BuildContext,
}

impl<'a> RegularAttributeExpression<'a> {
    pub(super) fn new(context: &'a BuildContext) -> Self {
        Self { context }
    }

    pub(super) fn all_argument_refs(&self, arguments: &[&Argument<'_>]) -> bool {
        arguments.iter().all(|argument| {
            argument
                .as_expression()
                .is_some_and(|expression| self.matches_expression(expression))
        })
    }

    pub(super) fn matches_expression(&self, expression: &Expression<'_>) -> bool {
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
                self.matches_regular_property_access(member)
                    || matches!(&member.object, Expression::Identifier(_))
                        && self.context.is_enum_member_access(member.span)
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.matches_expression(&parenthesized.expression)
            }
            _ => false,
        }
    }

    fn matches_regular_property_access(&self, member: &StaticMemberExpression<'_>) -> bool {
        match &member.object {
            Expression::ThisExpression(_) => self
                .context
                .is_regular_attribute_property(member.property.name.as_str()),
            Expression::StaticMemberExpression(parent) => {
                self.matches_regular_property_access(parent)
            }
            _ => false,
        }
    }
}

pub(super) struct ArkuiCallName;

impl ArkuiCallName {
    pub(super) fn component(callee: &Expression<'_>) -> Result<String, BuildLoweringError> {
        match callee {
            Expression::Identifier(identifier) => Ok(identifier.name.to_string()),
            Expression::StaticMemberExpression(member) => Ok(member.property.name.to_string()),
            _ => Err(BuildLoweringError::UnsupportedCallee),
        }
    }

    pub(super) fn component_path(callee: &Expression<'_>) -> Result<String, BuildLoweringError> {
        match callee {
            Expression::Identifier(identifier) => Ok(identifier.name.to_string()),
            Expression::StaticMemberExpression(member) => {
                let prefix = Self::component_path(&member.object)?;
                Ok(format!("{prefix}.{}", member.property.name))
            }
            _ => Err(BuildLoweringError::UnsupportedCallee),
        }
    }

    pub(super) fn chain(call: &CallExpression<'_>) -> Result<String, BuildLoweringError> {
        match &call.callee {
            Expression::StaticMemberExpression(member) => Ok(member.property.name.to_string()),
            _ => Err(BuildLoweringError::UnsupportedChain),
        }
    }

    pub(super) fn builder_method_member(
        expression: &Expression<'_>,
        builder_methods: &FxHashSet<String>,
    ) -> Option<String> {
        let name = Self::this_member(expression)?;
        builder_methods.contains(name.as_str()).then_some(name)
    }

    pub(super) fn builder_member(
        expression: &Expression<'_>,
        builder_methods: &FxHashSet<String>,
    ) -> Option<String> {
        let Expression::StaticMemberExpression(member) = expression else {
            return None;
        };
        let name = member.property.name.to_string();
        builder_methods.contains(name.as_str()).then_some(name)
    }

    pub(super) fn this_member(expression: &Expression<'_>) -> Option<String> {
        let Expression::StaticMemberExpression(member) = expression else {
            return None;
        };
        if !matches!(&member.object, Expression::ThisExpression(_)) {
            return None;
        }
        Some(member.property.name.to_string())
    }

    pub(super) fn builder_dispatch_member<'a>(
        call: &'a CallExpression<'a>,
        context: &BuildContext,
    ) -> Option<(&'a StaticMemberExpression<'a>, BuilderAccessKind)> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name.as_str() != context.runtime_names().builder_property.as_str() {
            return None;
        }
        context
            .builder_access_kind(member.span)
            .map(|kind| (member.as_ref(), kind))
    }
}

pub(super) struct AttributeArgumentEmitter<'a, 'ctx> {
    pub(super) component_name: &'a str,
    pub(super) attr_name: &'a str,
    pub(super) builder_methods: &'a FxHashSet<String>,
    pub(super) context: Option<&'ctx BuildContext>,
}

impl<'a, 'ctx> AttributeArgumentEmitter<'a, 'ctx> {
    pub(super) fn new(
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

    pub(super) fn argument_refs<'b>(
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

pub(super) struct AttributeBindingArgumentEmitter<'a, 'ctx> {
    pub(super) component_name: &'a str,
    pub(super) attr_name: &'a str,
    pub(super) builder_methods: &'a FxHashSet<String>,
    pub(super) context: &'ctx BuildContext,
}

impl<'a, 'ctx> AttributeBindingArgumentEmitter<'a, 'ctx> {
    pub(super) fn new(
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

    pub(super) fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let processed = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| self.argument(argument, index, allocator))
            .collect();
        let processed = self.double_exclamation_arguments(processed, allocator);
        self.double_dollar_arguments(processed.into_iter().collect(), allocator)
    }

    pub(super) fn double_exclamation_arguments<'b>(
        &self,
        arguments: Vec<Argument<'b>>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let should_rewrite_style = self
            .context
            .is_style_double_exclamation_attr(self.attr_name)
            && !arguments.is_empty();
        let should_rewrite_component = self
            .context
            .is_component_double_exclamation_property(self.component_name, self.attr_name)
            && arguments.len() == 1;
        if !should_rewrite_style && !should_rewrite_component {
            let ast = AstBuilder::new(allocator);
            return OxcVec::from_iter_in(arguments, &ast);
        }
        let Some(first_expression) = arguments.first().and_then(Argument::as_expression) else {
            let ast = AstBuilder::new(allocator);
            return OxcVec::from_iter_in(arguments, &ast);
        };
        let emitter = DoubleExclamationBindingEmitter::new(self.context);
        let Some(value) = emitter.updated_expression(first_expression, allocator) else {
            let ast = AstBuilder::new(allocator);
            return OxcVec::from_iter_in(arguments, &ast);
        };
        let ast = AstBuilder::new(allocator);
        let mut rewritten = OxcVec::with_capacity_in(arguments.len(), &ast);
        rewritten.push(Argument::from(
            emitter.value_change_object_from_value(value, allocator),
        ));
        rewritten.extend(arguments.into_iter().skip(1));
        rewritten
    }

    pub(super) fn argument<'b>(
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

    pub(super) fn double_dollar_arguments<'b>(
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
            return {
                let b = AstBuilder::new(allocator);
                OxcVec::from_iter_in(arguments, &b)
            };
        }
        DoubleDollarBindingEmitter::new(self.context).attribute_arguments(
            arguments,
            should_rewrite_style,
            allocator,
        )
    }

    pub(super) fn bind_popup_argument_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let Expression::ObjectExpression(object) = expression else {
            return self.plain_expression(expression, allocator);
        };
        let ast = &AstBuilder::new(allocator);
        let runtime_names = self.context.runtime_names();
        let properties = OxcVec::from_iter_in(
            object.properties.iter().map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return self.plain_object_property(property, allocator);
                };
                if property.key.static_name().is_some_and(|name| {
                    name.as_str() == Some(runtime_names.custom_dialog_controller_builder.as_str())
                }) {
                    return ObjectPropertyKind::new_object_property(
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
                        ast,
                    );
                }
                self.plain_regular_object_property(property, allocator)
            }),
            ast,
        );
        Expression::new_object_expression(object.span, properties, ast)
    }

    pub(super) fn drag_argument_expression<'b>(
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
        let oxc_ast::ast::ArrowFunctionBody::FunctionBody(function_body) = &arrow.body else {
            return None;
        };
        let ast = &AstBuilder::new(allocator);
        let statements = oxc_allocator::ArenaVec::from_iter_in(
            function_body
                .statements
                .iter()
                .map(|statement| self.drag_statement(statement, allocator)),
            ast,
        );
        let body = oxc_ast::ast::ArrowFunctionBody::FunctionBody(oxc_allocator::ArenaBox::new_in(
            oxc_ast::ast::FunctionBody::new(
                function_body.span,
                function_body.directives.clone_in(allocator),
                statements,
                ast,
            ),
            ast,
        ));
        Some(Expression::new_arrow_function_expression(
            arrow.span,
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
            ast,
        ))
    }

    pub(super) fn drag_statement<'b>(
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
        {
            let b = AstBuilder::new(allocator);
            Statement::new_return_statement(return_statement.span, argument, &b)
        }
    }

    pub(super) fn drag_return_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let Expression::ObjectExpression(object) = expression else {
            return self.builder_initializer_expression(expression, self.attr_name, allocator);
        };
        let ast = &AstBuilder::new(allocator);
        let builder_property = self.context.runtime_names().builder_property.as_str();
        let properties = OxcVec::from_iter_in(
            object.properties.iter().map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return self.plain_object_property(property, allocator);
                };
                if property
                    .key
                    .static_name()
                    .is_some_and(|name| name == builder_property)
                {
                    return ObjectPropertyKind::new_object_property(
                        property.span,
                        property.kind,
                        property.key.clone_in(allocator),
                        self.builder_initializer_expression(
                            &property.value,
                            self.attr_name,
                            allocator,
                        ),
                        false,
                        false,
                        property.computed,
                        ast,
                    );
                }
                self.plain_regular_object_property(property, allocator)
            }),
            ast,
        );
        Expression::new_object_expression(object.span, properties, ast)
    }

    pub(super) fn builder_initializer_expression<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        self.builder_attribute_expression_for_attr(expression, attr_name, allocator)
            .unwrap_or_else(|| self.plain_expression(expression, allocator))
    }

    pub(super) fn builder_attribute_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        self.builder_attribute_expression_for_attr(expression, self.attr_name, allocator)
    }

    pub(super) fn builder_attribute_expression_for_attr<'b>(
        &self,
        expression: &Expression<'_>,
        attr_name: &str,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Expression::ConditionalExpression(conditional) = expression {
            let ast = &AstBuilder::new(allocator);
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
            return Some(Expression::new_conditional_expression(
                conditional.span,
                self.plain_expression(&conditional.test, allocator),
                consequent,
                alternate,
                ast,
            ));
        }
        let without_key = self.context.is_builder_attribute_without_key(attr_name);
        if let Some(target) = self.builder_reference_expression(expression, allocator) {
            let bound = self.bound_expression(
                target,
                {
                    let b = AstBuilder::new(allocator);
                    OxcVec::new_in(&b)
                },
                allocator,
            );
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

    pub(super) fn builder_reference_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if ArkuiCallName::builder_member(expression, self.builder_methods).is_some() {
            return Some(expression.clone_in(allocator));
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        self.builder_methods
            .contains(identifier.name.as_str())
            .then(|| expression.clone_in(allocator))
    }

    pub(super) fn builder_call_target<'b>(
        &self,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> Option<(Expression<'b>, String)> {
        if let Some(name) = ArkuiCallName::builder_member(&call.callee, self.builder_methods) {
            return Some((call.callee.clone_in(allocator), name));
        }
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        self.builder_methods
            .contains(identifier.name.as_str())
            .then(|| (call.callee.clone_in(allocator), identifier.name.to_string()))
    }

    pub(super) fn object_property_builder_expression<'b>(
        &self,
        object: &ObjectExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let properties = OxcVec::from_iter_in(
            object.properties.iter().map(|property| {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return self.plain_object_property(property, allocator);
                };
                let Some(property_name) = property.key.static_name() else {
                    return self.plain_regular_object_property(property, allocator);
                };
                if !self
                    .context
                    .runtime_names()
                    .is_builder_object_property_name(property_name.as_str_or_empty())
                {
                    return self.plain_regular_object_property(property, allocator);
                }
                let value = self
                    .object_property_builder_value(&property.value, allocator)
                    .unwrap_or_else(|| self.plain_expression(&property.value, allocator));
                ObjectPropertyKind::new_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    value,
                    false,
                    false,
                    property.computed,
                    ast,
                )
            }),
            ast,
        );
        Expression::new_object_expression(object.span, properties, ast)
    }

    pub(super) fn object_property_builder_value<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        if let Some(target) = self.builder_reference_expression(expression, allocator) {
            return Some(self.bound_expression(
                target,
                {
                    let b = AstBuilder::new(allocator);
                    OxcVec::new_in(&b)
                },
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

    pub(super) fn builder_call_arrow<'b>(
        &self,
        callee: Expression<'b>,
        builder_name: &str,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let call_member = MemberExpression::new_static_member_expression(
            SPAN,
            callee,
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().method_call),
                ast,
            ),
            false,
            ast,
        );
        let mut arguments = OxcVec::new_in(&allocator);
        arguments.push(Argument::from(Expression::new_this_expression(SPAN, ast)));
        arguments.extend(self.builder_call_arguments(builder_name, call, allocator));
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(call_member),
            None,
            arguments,
            false,
            ast,
        );
        let body = FunctionBody::new(
            SPAN,
            OxcVec::new_in(ast),
            OxcVec::from_value_in(Statement::new_expression_statement(SPAN, call, ast), ast),
            ast,
        );
        Expression::new_arrow_function_expression(
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
        )
    }

    pub(super) fn builder_call_arguments<'b>(
        &self,
        builder_name: &str,
        call: &CallExpression<'_>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = &AstBuilder::new(allocator);
        if call.arguments.len() == 1 {
            if let Some(Expression::ObjectExpression(object)) =
                call.arguments.first().and_then(Argument::as_expression)
            {
                let proxy_object = BuilderParamProxyObjectEmitter::new(self.context)
                    .object_expression(object, allocator);
                let proxy_call = Expression::new_call_expression(
                    SPAN,
                    Expression::new_identifier(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&self.context.runtime_names().builder_param_proxy),
                        ast,
                    ),
                    None,
                    OxcVec::from_iter_in(
                        [
                            Argument::from(Expression::new_string_literal(
                                SPAN,
                                ast.allocator().alloc_str(builder_name),
                                None,
                                ast,
                            )),
                            Argument::from(proxy_object),
                        ],
                        ast,
                    ),
                    false,
                    ast,
                );
                return OxcVec::from_value_in(Argument::from(proxy_call), ast);
            }
        }
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .arguments_with_allocator(&call.arguments, allocator)
    }

    pub(super) fn builder_object_expression<'b>(
        &self,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let property = ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().builder_property),
                ast,
            ),
            value,
            false,
            false,
            false,
            ast,
        );
        Expression::new_object_expression(SPAN, OxcVec::from_value_in(property, ast), ast)
    }

    pub(super) fn bound_expression<'b>(
        &self,
        target: Expression<'b>,
        extra_arguments: OxcVec<'b, Argument<'b>>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let bind_member = MemberExpression::new_static_member_expression(
            SPAN,
            target,
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().method_bind),
                ast,
            ),
            false,
            ast,
        );
        let mut arguments = OxcVec::new_in(&allocator);
        arguments.push(Argument::from(Expression::new_this_expression(SPAN, ast)));
        arguments.extend(extra_arguments);
        Expression::new_call_expression(
            SPAN,
            Expression::from(bind_member),
            None,
            arguments,
            false,
            ast,
        )
    }

    pub(super) fn plain_argument<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .argument_with_allocator(argument, allocator)
    }

    pub(super) fn plain_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        BuildArgumentEmitter::new()
            .with_context(self.context)
            .with_nested_raw_object_rewrite()
            .expression_with_allocator(expression, allocator)
    }

    pub(super) fn plain_object_property<'b>(
        &self,
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = &AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                self.plain_regular_object_property(property, allocator)
            }
            ObjectPropertyKind::SpreadProperty(spread) => ObjectPropertyKind::new_spread_property(
                spread.span,
                self.plain_expression(&spread.argument, allocator),
                ast,
            ),
        }
    }

    pub(super) fn plain_regular_object_property<'b>(
        &self,
        property: &ObjectProperty<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        {
            let b = AstBuilder::new(allocator);
            ObjectPropertyKind::new_object_property(
                property.span,
                property.kind,
                property.key.clone_in(allocator),
                self.plain_expression(&property.value, allocator),
                property.method,
                property.shorthand,
                property.computed,
                &b,
            )
        }
    }
}

pub(super) struct CreateArgumentEmitter<'a, 'ctx> {
    pub(super) component_name: &'a str,
    pub(super) builder_methods: &'a FxHashSet<String>,
    pub(super) context: &'ctx BuildContext,
    pub(super) top_level_raw_object_rewrite: bool,
}

impl<'a, 'ctx> CreateArgumentEmitter<'a, 'ctx> {
    pub(super) fn new(
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

    pub(super) fn arguments<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = &AstBuilder::new(allocator);
        let mut emitted = OxcVec::from_iter_in(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
            ast,
        );
        self.push_xcomponent_module_identity(&mut emitted, ast);
        emitted
    }

    pub(super) fn argument_refs<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let ast = &AstBuilder::new(allocator);
        let mut emitted = OxcVec::from_iter_in(
            arguments
                .iter()
                .map(|argument| self.argument(argument, allocator)),
            ast,
        );
        self.push_xcomponent_module_identity(&mut emitted, ast);
        emitted
    }

    pub(super) fn push_xcomponent_module_identity<'b>(
        &self,
        arguments: &mut OxcVec<'b, Argument<'b>>,
        ast: &AstBuilder<'b>,
    ) {
        if self.context.component_kind(self.component_name) != ArkuiRuntimeComponentKind::XComponent
        {
            return;
        }
        let Some(identity) = &self.context.xcomponent_module_identity else {
            return;
        };
        arguments.push(Argument::from(Expression::new_string_literal(
            SPAN,
            ast.allocator().alloc_str(identity),
            None,
            ast,
        )));
    }

    pub(super) fn argument<'b>(
        &self,
        argument: &Argument<'_>,
        allocator: &'b Allocator,
    ) -> Argument<'b> {
        let Some(expression) = argument.as_expression() else {
            return self.plain_argument(argument, allocator);
        };
        Argument::from(self.expression(expression, allocator))
    }

    pub(super) fn expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        // ets-loader `checkCreateArgumentBuilder` consumes both the explicit
        // CREATE_BIND_COMPONENT set and component-specific `create` entries in
        // BIND_OBJECT_PROPERTY. ComponentMetadata merges those SDK-owned rules
        // behind `is_builder_attribute_method`.
        let needs_builder = self.context.is_builder_attribute_method(
            self.component_name,
            &self.context.runtime_names().method_create,
        );
        let attribute_emitter = AttributeBindingArgumentEmitter::new(
            self.component_name,
            &self.context.runtime_names().method_create,
            self.builder_methods,
            self.context,
        );
        if needs_builder {
            if let Expression::CallExpression(call) = expression {
                if let Some((callee, builder_name)) =
                    attribute_emitter.builder_call_target(call, allocator)
                {
                    return attribute_emitter.builder_call_arrow(
                        callee,
                        &builder_name,
                        call,
                        allocator,
                    );
                }
            }
            if let Some(target) =
                attribute_emitter.builder_reference_expression(expression, allocator)
            {
                return attribute_emitter.bound_expression(
                    target,
                    {
                        let b = AstBuilder::new(allocator);
                        OxcVec::new_in(&b)
                    },
                    allocator,
                );
            }
        }
        let Expression::ObjectExpression(object) = expression else {
            return self.plain_expression(expression, allocator);
        };
        let has_double_binding = object.properties.iter().any(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            property.key.static_name().is_some_and(|name| {
                (self.context.is_component_double_dollar_property_name(
                    self.component_name,
                    name.as_str_or_empty(),
                ) && DoubleDollarBindingEmitter::new(self.context)
                    .updated_expression(&property.value, allocator)
                    .is_some())
                    || (self.context.is_component_double_exclamation_property(
                        self.component_name,
                        name.as_str_or_empty(),
                    ) && DoubleExclamationBindingEmitter::new(self.context)
                        .updated_expression(&property.value, allocator)
                        .is_some())
            })
        });
        if !has_double_binding && !needs_builder {
            return self.plain_expression(expression, allocator);
        }
        let ast = &AstBuilder::new(allocator);
        let mut properties = OxcVec::with_capacity_in(object.properties.len() * 2, ast);
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                properties.push(attribute_emitter.plain_object_property(property, allocator));
                continue;
            };
            let Some(property_name) = property.key.static_name() else {
                properties
                    .push(attribute_emitter.plain_regular_object_property(property, allocator));
                continue;
            };
            if self.context.is_component_double_exclamation_property(
                self.component_name,
                property_name.as_str_or_empty(),
            ) {
                if let Some(value) = DoubleExclamationBindingEmitter::new(self.context)
                    .updated_expression(&property.value, allocator)
                {
                    properties.push(ObjectPropertyKind::new_object_property(
                        property.span,
                        property.kind,
                        property.key.clone_in(allocator),
                        value.clone_in(allocator),
                        false,
                        false,
                        property.computed,
                        ast,
                    ));
                    properties.push(
                        DoubleExclamationBindingEmitter::new(self.context).change_property(
                            property_name.as_str_or_empty(),
                            value,
                            allocator,
                        ),
                    );
                    continue;
                }
            }
            if self.context.is_component_double_dollar_property_name(
                self.component_name,
                property_name.as_str_or_empty(),
            ) {
                if let Some(value) = DoubleDollarBindingEmitter::new(self.context)
                    .value_change_object(&property.value, allocator)
                {
                    properties.push(ObjectPropertyKind::new_object_property(
                        property.span,
                        property.kind,
                        property.key.clone_in(allocator),
                        value,
                        false,
                        false,
                        property.computed,
                        ast,
                    ));
                    continue;
                }
            }
            if needs_builder
                && self
                    .context
                    .runtime_names()
                    .is_builder_object_property_name(property_name.as_str_or_empty())
            {
                let value = attribute_emitter
                    .object_property_builder_value(&property.value, allocator)
                    .unwrap_or_else(|| {
                        attribute_emitter.plain_expression(&property.value, allocator)
                    });
                properties.push(ObjectPropertyKind::new_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    value,
                    false,
                    false,
                    property.computed,
                    ast,
                ));
                continue;
            }
            properties.push(attribute_emitter.plain_regular_object_property(property, allocator));
        }
        Expression::new_object_expression(object.span, properties, ast)
    }

    pub(super) fn plain_argument<'b>(
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

    pub(super) fn plain_expression<'b>(
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

pub(super) struct DoubleExclamationBindingEmitter<'ctx> {
    context: &'ctx BuildContext,
}

impl<'ctx> DoubleExclamationBindingEmitter<'ctx> {
    fn new(context: &'ctx BuildContext) -> Self {
        Self { context }
    }

    fn updated_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        let Expression::TSNonNullExpression(outer) = expression else {
            return None;
        };
        let Expression::TSNonNullExpression(inner) = &outer.expression else {
            return None;
        };
        if matches!(inner.expression, Expression::TSNonNullExpression(_)) {
            return None;
        }
        Some(inner.expression.clone_in(allocator))
    }

    fn value_change_object_from_value<'b>(
        &self,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = AstBuilder::new(allocator);
        let value_property = ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().double_dollar_value),
                &ast,
            ),
            value.clone_in(allocator),
            false,
            false,
            false,
            &ast,
        );
        let change_property = ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().double_exclamation_change_value),
                &ast,
            ),
            DoubleDollarBindingEmitter::new(self.context).change_event_arrow(value, allocator),
            false,
            false,
            false,
            &ast,
        );
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in([value_property, change_property], &ast),
            &ast,
        )
    }

    fn change_property<'b>(
        &self,
        property_name: &str,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = AstBuilder::new(allocator);
        let change_name = format!("${property_name}");
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(SPAN, ast.allocator().alloc_str(&change_name), &ast),
            DoubleDollarBindingEmitter::new(self.context).change_event_arrow(value, allocator),
            false,
            false,
            false,
            &ast,
        )
    }
}

pub(super) struct DoubleDollarBindingEmitter<'ctx> {
    pub(super) context: &'ctx BuildContext,
}

impl<'ctx> DoubleDollarBindingEmitter<'ctx> {
    pub(super) fn new(context: &'ctx BuildContext) -> Self {
        Self { context }
    }

    pub(super) fn attribute_arguments<'b>(
        &self,
        arguments: Vec<Argument<'b>>,
        style_attr: bool,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        let Some(first_expression) = arguments.first().and_then(Argument::as_expression) else {
            return {
                let b = AstBuilder::new(allocator);
                OxcVec::from_iter_in(arguments, &b)
            };
        };
        let Some(value_expression) = self.updated_expression(first_expression, allocator) else {
            return {
                let b = AstBuilder::new(allocator);
                OxcVec::from_iter_in(arguments, &b)
            };
        };
        let ast = &AstBuilder::new(allocator);
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
        OxcVec::from_iter_in(out, ast)
    }

    pub(super) fn value_change_object<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        self.updated_expression(expression, allocator)
            .map(|value| self.value_change_object_from_value(value, allocator))
    }

    pub(super) fn value_change_object_from_value<'b>(
        &self,
        value: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let change_target = value.clone_in(allocator);
        let value_property = ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().double_dollar_value),
                ast,
            ),
            value,
            false,
            false,
            false,
            ast,
        );
        let change_property = ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().double_dollar_change_event),
                ast,
            ),
            self.change_event_arrow(change_target, allocator),
            false,
            false,
            false,
            ast,
        );
        Expression::new_object_expression(
            SPAN,
            OxcVec::from_iter_in([value_property, change_property], ast),
            ast,
        )
    }

    pub(super) fn change_event_arrow<'b>(
        &self,
        target: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let target = Self::assignment_target_from_expression(target, ast);
        let new_value = Expression::new_identifier(
            SPAN,
            ast.allocator()
                .alloc_str(&self.context.runtime_names().double_dollar_new_value),
            ast,
        );
        let assignment = Expression::new_assignment_expression(
            SPAN,
            AssignmentOperator::Assign,
            target,
            new_value,
            ast,
        );
        let body = FunctionBody::new(
            SPAN,
            OxcVec::new_in(ast),
            OxcVec::from_value_in(
                Statement::new_expression_statement(SPAN, assignment, ast),
                ast,
            ),
            ast,
        );
        let mut params = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::with_capacity_in(1, ast),
            None,
            ast,
        );
        params.items.push(FormalParameter::new(
            SPAN,
            OxcVec::new_in(ast),
            BindingPattern::new_binding_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().double_dollar_new_value),
                ast,
            ),
            None,
            None,
            false,
            None,
            false,
            false,
            ast,
        ));
        Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(params, ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, ast)),
            ast,
        )
    }

    pub(super) fn updated_expression<'b>(
        &self,
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Option<Expression<'b>> {
        let ast = &AstBuilder::new(allocator);
        match expression {
            Expression::Identifier(identifier) => {
                let name = identifier.name.as_str();
                if name == self.context.runtime_names().double_dollar_this {
                    return Some(Expression::new_this_expression(identifier.span, ast));
                }
                let stripped = name.strip_prefix("$$")?;
                if stripped.is_empty() || name.starts_with("$$.") {
                    return None;
                }
                Some(Expression::new_identifier(
                    identifier.span,
                    ast.allocator().alloc_str(stripped),
                    ast,
                ))
            }
            Expression::StaticMemberExpression(member) => {
                let object = self.updated_expression(&member.object, allocator)?;
                Some(Expression::from(
                    MemberExpression::new_static_member_expression(
                        member.span,
                        object,
                        member.property.clone_in(allocator),
                        member.optional,
                        ast,
                    ),
                ))
            }
            Expression::ComputedMemberExpression(member) => {
                let object = self.updated_expression(&member.object, allocator)?;
                Some(Expression::from(
                    MemberExpression::new_computed_member_expression(
                        member.span,
                        object,
                        member.expression.clone_in(allocator),
                        member.optional,
                        ast,
                    ),
                ))
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.updated_expression(&parenthesized.expression, allocator)
            }
            _ => None,
        }
    }

    pub(super) fn assignment_target_from_expression<'b>(
        expression: Expression<'b>,
        ast: &AstBuilder<'b>,
    ) -> AssignmentTarget<'b> {
        match expression {
            Expression::Identifier(identifier) => {
                AssignmentTarget::new_assignment_target_identifier(
                    identifier.span,
                    identifier.name,
                    ast,
                )
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

pub(super) struct ComponentCallStatementEmitter;

impl ComponentCallStatementEmitter {
    const ANIMATION_ATTRIBUTE: &'static str = "animation";

    pub(super) fn create<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        create_method: &str,
        arguments: &[Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
        component_span: Span,
    ) -> Statement<'b> {
        let arguments = CreateArgumentEmitter::new(component_name, builder_methods, context, false)
            .arguments(arguments, ast.allocator());
        Self::raw_call_with_source(
            ast,
            component_name,
            create_method,
            arguments,
            component_span,
            SPAN,
        )
    }

    pub(super) fn create_with_child_body<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        create_method: &str,
        child_body: OxcVec<'b, Statement<'b>>,
        component_span: Span,
    ) -> Statement<'b> {
        let parameters = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::new_in(ast),
            None,
            ast,
        );
        let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), child_body, ast);
        let callback = Expression::new_arrow_function_expression(
            SPAN,
            false,
            None,
            ArenaBox::new_in(parameters, ast),
            None,
            ArrowFunctionBody::FunctionBody(ArenaBox::new_in(body, ast)),
            ast,
        );
        Self::raw_call_with_source(
            ast,
            component_name,
            create_method,
            OxcVec::from_value_in(Argument::from(callback), ast),
            component_span,
            SPAN,
        )
    }

    pub(super) fn is_animation_attribute(attr_name: &str) -> bool {
        attr_name == Self::ANIMATION_ATTRIBUTE
    }

    pub(super) fn animation_refs<'b>(
        ast: &AstBuilder<'b>,
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

    pub(super) fn clear_animation<'b>(
        ast: &AstBuilder<'b>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let arguments =
            OxcVec::from_value_in(Argument::from(Expression::new_null_literal(SPAN, ast)), ast);
        Self::raw_call(
            ast,
            &context.runtime_names().component_context,
            Self::ANIMATION_ATTRIBUTE,
            arguments,
        )
    }

    pub(super) fn flush_animation_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        allocator: &'b Allocator,
    ) {
        statements.extend(std::mem::replace(segment, OxcVec::new_in(&allocator)));
    }

    pub(super) fn push_animation_ref_segment<'b>(
        statements: &mut OxcVec<'b, Statement<'b>>,
        segment: &mut OxcVec<'b, Statement<'b>>,
        ast: &AstBuilder<'b>,
        arguments: &[&Argument<'_>],
        context: &BuildContext,
        emit_configuration: bool,
        emit_clear: bool,
    ) {
        if emit_configuration {
            statements.push(Self::animation_refs(
                ast,
                ast.allocator(),
                arguments,
                context,
            ));
            Self::flush_animation_segment(statements, segment, ast.allocator());
        } else {
            debug_assert!(segment.is_empty());
        }
        if emit_clear {
            statements.push(Self::clear_animation(ast, context));
        }
    }

    pub(super) fn is_null_animation_refs(arguments: &[&Argument<'_>]) -> bool {
        matches!(arguments, [argument] if matches!(argument, Argument::NullLiteral(_)))
    }

    pub(super) fn argument_refs<'b>(
        ast: &AstBuilder<'b>,
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

    pub(super) fn create_refs<'b>(
        ast: &AstBuilder<'b>,
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

    pub(super) fn attribute_refs<'b>(
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        attr_name: &str,
        arguments: &[&Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let mut arguments = AttributeArgumentEmitter::new(
            component_name,
            attr_name,
            builder_methods,
            Some(context),
        )
        .argument_refs(arguments, allocator);
        match context.extend_attribute_kind(component_name, attr_name) {
            Some(ExtendAttributeKind::Extend) => {
                return Self::raw_identifier_call(
                    ast,
                    &GeneratedViewpuName::extend_function(component_name, attr_name),
                    arguments,
                );
            }
            Some(ExtendAttributeKind::Animatable) => {
                Self::append_animatable_runtime_arguments(ast, &mut arguments, context);
                return Self::raw_identifier_call(ast, attr_name, arguments);
            }
            None => {}
        }
        if context
            .runtime_names()
            .component_attribute_requires_bind(attr_name)
        {
            return Self::raw_bound_call(ast, component_name, attr_name, arguments, context);
        }
        Self::raw_call(ast, component_name, attr_name, arguments)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn attribute_refs_with_source<'b>(
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        component_name: &str,
        attr_name: &str,
        arguments: &[&Argument<'_>],
        builder_methods: &FxHashSet<String>,
        context: &BuildContext,
        component_span: Span,
        attribute_span: Span,
    ) -> Statement<'b> {
        let mut arguments = AttributeArgumentEmitter::new(
            component_name,
            attr_name,
            builder_methods,
            Some(context),
        )
        .argument_refs(arguments, allocator);
        match context.extend_attribute_kind(component_name, attr_name) {
            Some(ExtendAttributeKind::Extend) => {
                return Self::raw_identifier_call(
                    ast,
                    &GeneratedViewpuName::extend_function(component_name, attr_name),
                    arguments,
                );
            }
            Some(ExtendAttributeKind::Animatable) => {
                Self::append_animatable_runtime_arguments(ast, &mut arguments, context);
                return Self::raw_identifier_call(ast, attr_name, arguments);
            }
            None => {}
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

    fn append_animatable_runtime_arguments<'b>(
        ast: &AstBuilder<'b>,
        arguments: &mut OxcVec<'b, Argument<'b>>,
        context: &BuildContext,
    ) {
        arguments.extend([
            Argument::from(Expression::new_identifier(
                SPAN,
                ast.allocator().alloc_str(&context.runtime_names().elmt_id),
                ast,
            )),
            Argument::from(Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().is_initial_render),
                ast,
            )),
            Argument::from(Expression::new_this_expression(SPAN, ast)),
        ]);
    }

    pub(super) fn empty<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
    ) -> Statement<'b> {
        Self::raw_call(ast, component_name, method_name, OxcVec::new_in(ast))
    }

    pub(super) fn empty_with_source<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        component_span: Span,
    ) -> Statement<'b> {
        Self::raw_call_with_source(
            ast,
            component_name,
            method_name,
            OxcVec::new_in(ast),
            component_span,
            SPAN,
        )
    }

    pub(super) fn gesture_low_priority<'b>(
        ast: &AstBuilder<'b>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let runtime_names = context.runtime_names();
        let gesture_priority = Expression::new_identifier(
            SPAN,
            ast.allocator().alloc_str(&runtime_names.gesture_priority),
            ast,
        );
        let low_member = MemberExpression::new_static_member_expression(
            SPAN,
            gesture_priority,
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&runtime_names.gesture_priority_low),
                ast,
            ),
            false,
            ast,
        );
        let arguments = OxcVec::from_value_in(Argument::from(Expression::from(low_member)), ast);
        Self::raw_call(
            ast,
            &runtime_names.component_gesture,
            &runtime_names.method_create,
            arguments,
        )
    }

    pub(super) fn raw_call<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        Self::raw_call_with_source(ast, component_name, method_name, arguments, SPAN, SPAN)
    }

    pub(super) fn raw_call_with_source<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
        component_span: Span,
        method_span: Span,
    ) -> Statement<'b> {
        let component = Expression::new_identifier(
            component_span,
            ast.allocator().alloc_str(component_name),
            ast,
        );
        // Latest SDK `process_component_build.js::createFunction` emits
        // generated calls through `globalThis` for `COMPONENT_GESTURE` and
        // `GLOBAL_CONTEXT`. This is deliberately limited to calls constructed
        // by the lowering pass: source-level `Context.animateTo(...)` remains
        // source-owned and is not rewritten by this emitter.
        let object = if matches!(component_name, "Gesture" | "Context") {
            let global_this = Expression::new_identifier(SPAN, "globalThis", ast);
            Expression::from(MemberExpression::new_static_member_expression(
                SPAN,
                global_this,
                IdentifierName::new(
                    component_span,
                    ast.allocator().alloc_str(component_name),
                    ast,
                ),
                false,
                ast,
            ))
        } else {
            component
        };
        let property =
            IdentifierName::new(method_span, ast.allocator().alloc_str(method_name), ast);
        let member =
            MemberExpression::new_static_member_expression(SPAN, object, property, false, ast);
        let call = Expression::new_call_expression(
            SPAN,
            Expression::from(member),
            None,
            arguments,
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, call, ast)
    }

    pub(super) fn raw_identifier_call<'b>(
        ast: &AstBuilder<'b>,
        function_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
    ) -> Statement<'b> {
        let callee =
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(function_name), ast);
        let call = Expression::new_call_expression(SPAN, callee, None, arguments, false, ast);
        Statement::new_expression_statement(SPAN, call, ast)
    }

    pub(super) fn raw_bound_call<'b>(
        ast: &AstBuilder<'b>,
        component_name: &str,
        method_name: &str,
        arguments: OxcVec<'b, Argument<'b>>,
        context: &BuildContext,
    ) -> Statement<'b> {
        let object =
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(component_name), ast);
        let property = IdentifierName::new(SPAN, ast.allocator().alloc_str(method_name), ast);
        let member =
            MemberExpression::new_static_member_expression(SPAN, object, property, false, ast);
        let bind_member = MemberExpression::new_static_member_expression(
            SPAN,
            Expression::from(member),
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().method_bind),
                ast,
            ),
            false,
            ast,
        );
        let bind_call = Expression::new_call_expression(
            SPAN,
            Expression::from(bind_member),
            None,
            OxcVec::from_value_in(
                Argument::from(Expression::new_this_expression(SPAN, ast)),
                ast,
            ),
            false,
            ast,
        );
        let call = Expression::new_call_expression(SPAN, bind_call, None, arguments, false, ast);
        Statement::new_expression_statement(SPAN, call, ast)
    }
}
