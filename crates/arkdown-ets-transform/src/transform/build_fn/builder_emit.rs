//! Navigation and @Builder call emission helpers.

use super::*;

pub(super) struct NavigationCreateCall<'a, 'b> {
    pub(super) component: &'a ArkUIComponentExpression<'a>,
    pub(super) name: &'b str,
    pub(super) module_name: &'b str,
    pub(super) page_path: &'b str,
}

impl<'a, 'b> NavigationCreateCall<'a, 'b> {
    pub(super) fn new(
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

    pub(super) fn statement<'c>(
        &self,
        ast: &AstBuilder<'c>,
        allocator: &'c Allocator,
        context: &BuildContext,
        builder_body: Option<OxcVec<'c, Statement<'c>>>,
    ) -> Statement<'c> {
        let callee = Expression::from(MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, ast.allocator().alloc_str(self.name), ast),
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&context.runtime_names().method_create),
                ast,
            ),
            false,
            ast,
        ));
        let arguments = self.arguments(ast, allocator, context, builder_body);
        let call = Expression::new_call_expression(SPAN, callee, None, arguments, false, ast);
        Statement::new_expression_statement(SPAN, call, ast)
    }

    pub(super) fn arguments<'c>(
        &self,
        ast: &AstBuilder<'c>,
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

    pub(super) fn source_argument<'c>(
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

    pub(super) fn empty_builder_arrow<'c>(&self, ast: &AstBuilder<'c>) -> Expression<'c> {
        let params = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::new_in(ast),
            None,
            ast,
        );
        let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), OxcVec::new_in(ast), ast);
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

    pub(super) fn builder_arrow<'c>(
        &self,
        ast: &AstBuilder<'c>,
        body_stmts: OxcVec<'c, Statement<'c>>,
    ) -> Expression<'c> {
        let params = FormalParameters::new(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            OxcVec::new_in(ast),
            None,
            ast,
        );
        let body = FunctionBody::new(SPAN, OxcVec::new_in(ast), body_stmts, ast);
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
}

pub(super) struct NavigationCreateMetadata<'a> {
    pub(super) module_name: &'a str,
    pub(super) page_path: &'a str,
    pub(super) user_create_stack: Option<bool>,
}

impl<'a> NavigationCreateMetadata<'a> {
    pub(super) fn new(
        module_name: &'a str,
        page_path: &'a str,
        user_create_stack: Option<bool>,
    ) -> Self {
        Self {
            module_name,
            page_path,
            user_create_stack,
        }
    }

    pub(super) fn expression<'b>(
        &self,
        ast: &AstBuilder<'b>,
        context: &BuildContext,
    ) -> Expression<'b> {
        let runtime_names = context.runtime_names();
        let mut properties = OxcVec::from_iter_in(
            [
                self.string_property(ast, &runtime_names.module_name, self.module_name),
                self.string_property(ast, &runtime_names.page_path, self.page_path),
            ],
            ast,
        );
        if let Some(value) = self.user_create_stack {
            properties.push(ObjectPropertyKind::new_object_property(
                SPAN,
                PropertyKind::Init,
                PropertyKey::new_static_identifier(
                    SPAN,
                    ast.allocator()
                        .alloc_str(&runtime_names.is_user_create_stack),
                    ast,
                ),
                Expression::new_boolean_literal(SPAN, value, ast),
                false,
                false,
                false,
                ast,
            ));
        }
        Expression::new_object_expression(SPAN, properties, ast)
    }

    pub(super) fn string_property<'b>(
        &self,
        ast: &AstBuilder<'b>,
        name: &str,
        value: &str,
    ) -> ObjectPropertyKind<'b> {
        ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::new_static_identifier(SPAN, ast.allocator().alloc_str(name), ast),
            Expression::new_string_literal(SPAN, ast.allocator().alloc_str(value), None, ast),
            false,
            false,
            false,
            ast,
        )
    }
}

pub(super) struct BuildArgumentEmitter<'a> {
    pub(super) context: Option<&'a BuildContext>,
    pub(super) top_level_raw_object_rewrite: bool,
    pub(super) nested_raw_object_rewrite: bool,
}

impl<'a> BuildArgumentEmitter<'a> {
    pub(super) fn new() -> Self {
        Self {
            context: None,
            top_level_raw_object_rewrite: true,
            nested_raw_object_rewrite: false,
        }
    }

    pub(super) fn with_context(mut self, context: &'a BuildContext) -> Self {
        self.context = Some(context);
        self
    }

    pub(super) fn with_optional_context(mut self, context: Option<&'a BuildContext>) -> Self {
        self.context = context;
        self
    }

    pub(super) fn with_nested_raw_object_rewrite(mut self) -> Self {
        self.nested_raw_object_rewrite = true;
        self
    }

    pub(super) fn without_top_level_raw_object_rewrite(mut self) -> Self {
        self.top_level_raw_object_rewrite = false;
        self
    }

    pub(super) fn arguments_with_allocator<'b>(
        &self,
        arguments: &[Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        {
            let b = AstBuilder::new(allocator);
            OxcVec::from_iter_in(
                arguments
                    .iter()
                    .map(|argument| self.argument_with_allocator(argument, allocator)),
                &b,
            )
        }
    }

    pub(super) fn argument_refs_with_allocator<'b>(
        &self,
        arguments: &[&Argument<'_>],
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        {
            let b = AstBuilder::new(allocator);
            OxcVec::from_iter_in(
                arguments
                    .iter()
                    .map(|argument| self.argument_with_allocator(argument, allocator)),
                &b,
            )
        }
    }

    pub(super) fn argument_with_allocator<'b>(
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
                let ast = &AstBuilder::new(allocator);
                Argument::SpreadElement(SpreadElement::boxed(spread.span, expression, ast))
            }
            _ => argument
                .as_expression()
                .map(|expression| {
                    Argument::from(self.expression_with_allocator(expression, allocator))
                })
                .unwrap_or_else(|| argument.clone_in(allocator)),
        }
    }

    pub(super) fn expression_with_allocator<'b>(
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

    pub(super) fn owned_expression_with_allocator<'b>(
        &self,
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let expression = GlobalContextCallRewriter::new(allocator).owned_expression(expression);
        let expression = ArkuiStateStylesObjectEmitter::owned_expression(expression, allocator);
        self.finalize_owned_expression(expression, allocator)
    }

    pub(super) fn finalize_owned_expression<'b>(
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

pub(super) struct ArkuiStateStylesObjectEmitter;

impl ArkuiStateStylesObjectEmitter {
    pub(super) fn expression<'b>(
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        if Self::should_rewrite_expression(expression) {
            Self::rewrite_expression(expression, allocator)
        } else {
            expression.clone_in(allocator)
        }
    }

    pub(super) fn owned_expression<'b>(
        expression: Expression<'b>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        if Self::should_rewrite_expression(&expression) {
            Self::rewrite_expression(&expression, allocator)
        } else {
            expression
        }
    }

    pub(super) fn rewrite_expression<'b>(
        expression: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        match expression {
            Expression::ObjectExpression(object) => {
                let properties = OxcVec::from_iter_in(
                    object
                        .properties
                        .iter()
                        .map(|property| Self::rewrite_object_property(property, allocator)),
                    ast,
                );
                Expression::new_object_expression(object.span, properties, ast)
            }
            Expression::ArrayExpression(array) => {
                let elements = OxcVec::from_iter_in(
                    array.elements.iter().map(|element| {
                        if let Some(expression) = element.as_expression() {
                            ArrayExpressionElement::from(Self::rewrite_expression(
                                expression, allocator,
                            ))
                        } else {
                            element.clone_in(allocator)
                        }
                    }),
                    ast,
                );
                Expression::new_array_expression(array.span, elements, ast)
            }
            _ => expression.clone_in(allocator),
        }
    }

    pub(super) fn should_rewrite_expression(expression: &Expression<'_>) -> bool {
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

    pub(super) fn should_rewrite_object_property(property: &ObjectPropertyKind<'_>) -> bool {
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

    pub(super) fn rewrite_object_property<'b>(
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = &AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if let Some((name, value)) = Self::leading_dot_property(&property.value, allocator)
                {
                    return ObjectPropertyKind::new_object_property(
                        property.span,
                        PropertyKind::Init,
                        PropertyKey::new_static_identifier(
                            property.span,
                            ast.allocator().alloc_str(&name),
                            ast,
                        ),
                        value,
                        false,
                        false,
                        false,
                        ast,
                    );
                }
                ObjectPropertyKind::new_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    Self::rewrite_expression(&property.value, allocator),
                    property.method,
                    property.shorthand,
                    property.computed,
                    ast,
                )
            }
            ObjectPropertyKind::SpreadProperty(spread) => ObjectPropertyKind::new_spread_property(
                spread.span,
                Self::rewrite_expression(&spread.argument, allocator),
                ast,
            ),
        }
    }

    pub(super) fn is_leading_dot_property(expression: &Expression<'_>) -> bool {
        let Expression::LeadingDotExpression(leading) = expression else {
            return false;
        };
        let Expression::CallExpression(call) = &leading.expression else {
            return false;
        };
        matches!(&call.callee, Expression::Identifier(_)) && call.arguments.len() == 1
    }

    pub(super) fn leading_dot_property<'b>(
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
pub(super) enum BuilderMethodReceiver {
    Direct,
    This,
}

/// Emits the `transferBuilderCall` shape used by ets-loader for a
/// `WrappedBuilder.builder(...)` expression. The dispatch and bind property
/// names come from SDK metadata instead of being duplicated here.
pub(super) struct BuilderDispatchCallEmitter<'a, 'ctx> {
    pub(super) call: &'a CallExpression<'a>,
    pub(super) context: &'ctx BuildContext,
}

impl<'a, 'ctx> BuilderDispatchCallEmitter<'a, 'ctx> {
    pub(super) fn new(call: &'a CallExpression<'a>, context: &'ctx BuildContext) -> Self {
        Self { call, context }
    }

    pub(super) fn matches(&self) -> bool {
        ArkuiCallName::is_builder_dispatch_call(self.call, self.context)
    }

    pub(super) fn expression<'b>(&self, allocator: &'b Allocator) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let mut call = self.call.clone_in(allocator);
        self.rewrite(&mut call, allocator);
        Expression::CallExpression(ArenaBox::new_in(call, ast))
    }

    pub(super) fn rewrite<'b>(&self, call: &mut CallExpression<'b>, allocator: &'b Allocator) {
        let ast = &AstBuilder::new(allocator);
        let mutable_receiver = self.mutable_receiver();
        let dispatch_target = mutable_receiver.map_or_else(
            || self.call.callee.clone_in(allocator),
            |receiver| {
                Expression::from(MemberExpression::new_static_member_expression(
                    SPAN,
                    receiver.clone_in(allocator),
                    IdentifierName::new(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&self.context.runtime_names().mutable_builder_dispatch),
                        ast,
                    ),
                    false,
                    ast,
                ))
            },
        );
        let bind_member = MemberExpression::new_static_member_expression(
            SPAN,
            dispatch_target,
            IdentifierName::new(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().method_bind),
                ast,
            ),
            false,
            ast,
        );
        call.callee = Expression::new_call_expression(
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
        call.arguments = self.arguments(ast, allocator, mutable_receiver);
    }

    pub(super) fn arguments<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        mutable_receiver: Option<&Expression<'a>>,
    ) -> OxcVec<'b, Argument<'b>> {
        let receiver_argument = mutable_receiver.map(|receiver| {
            let body = ArrowFunctionBody::from(receiver.clone_in(allocator));
            Argument::from(Expression::new_arrow_function_expression(
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
                body,
                ast,
            ))
        });
        if let [argument] = self.call.arguments.as_slice() {
            if let Some(Expression::ObjectExpression(object)) = argument.as_expression() {
                let mutable_proxy_context = self.context.clone().with_mutable_builder_proxy_scope();
                let proxy_context = if mutable_receiver.is_some() {
                    &mutable_proxy_context
                } else {
                    self.context
                };
                let proxy_object = BuilderParamProxyObjectEmitter::new(proxy_context)
                    .object_expression(object, allocator);
                let arguments = OxcVec::from_iter_in(
                    [
                        Argument::from(Expression::new_string_literal(
                            SPAN,
                            ast.allocator()
                                .alloc_str(&self.context.runtime_names().builder_property),
                            None,
                            ast,
                        )),
                        Argument::from(proxy_object),
                    ],
                    ast,
                );
                let proxy = Expression::new_call_expression(
                    SPAN,
                    Expression::new_identifier(
                        SPAN,
                        ast.allocator()
                            .alloc_str(&self.context.runtime_names().builder_param_proxy),
                        ast,
                    ),
                    None,
                    arguments,
                    false,
                    ast,
                );
                return OxcVec::from_iter_in(
                    receiver_argument.into_iter().chain([Argument::from(proxy)]),
                    ast,
                );
            }
        }
        let arguments = BuildArgumentEmitter::new()
            .with_context(self.context)
            .arguments_with_allocator(&self.call.arguments, allocator);
        OxcVec::from_iter_in(receiver_argument.into_iter().chain(arguments), ast)
    }

    fn mutable_receiver(&self) -> Option<&Expression<'a>> {
        let Expression::StaticMemberExpression(member) = &self.call.callee else {
            return None;
        };
        self.context
            .is_mutable_builder_receiver(&member.object)
            .then_some(&member.object)
    }
}

pub(super) struct BuilderMethodCallEmitter<'a> {
    pub(super) receiver: BuilderMethodReceiver,
    pub(super) name: &'a str,
    pub(super) arguments: &'a [Argument<'a>],
    pub(super) context: &'a BuildContext,
}

impl<'a> BuilderMethodCallEmitter<'a> {
    pub(super) fn new(
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

    pub(super) fn statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> Statement<'b> {
        let target = match self.receiver {
            BuilderMethodReceiver::Direct => {
                Expression::new_identifier(SPAN, ast.allocator().alloc_str(self.name), ast)
            }
            BuilderMethodReceiver::This => {
                let object = Expression::new_this_expression(SPAN, ast);
                let property = IdentifierName::new(SPAN, ast.allocator().alloc_str(self.name), ast);
                Expression::from(MemberExpression::new_static_member_expression(
                    SPAN, object, property, false, ast,
                ))
            }
        };
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
        let arguments = self.arguments(ast, allocator);
        let call = Expression::new_call_expression(SPAN, bind_call, None, arguments, false, ast);
        Statement::new_expression_statement(SPAN, call, ast)
    }

    pub(super) fn arguments<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Argument<'b>> {
        // SDK process_component_build.js::processComponentChild routes
        // customBuilderMethod, builderParamMethod, and builderTypeFunction
        // through the same transferBuilderCall implementation. Every one of
        // those call kinds proxies a sole object-literal argument.
        if let Some(argument) = self.proxy_argument(ast, allocator) {
            return OxcVec::from_value_in(argument, ast);
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

    pub(super) fn proxy_argument<'b>(
        &self,
        ast: &AstBuilder<'b>,
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
        let args = OxcVec::from_iter_in(
            [
                Argument::from(Expression::new_string_literal(
                    SPAN,
                    ast.allocator().alloc_str(self.name),
                    None,
                    ast,
                )),
                Argument::from(proxy_object),
            ],
            ast,
        );
        let call = Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(
                SPAN,
                ast.allocator()
                    .alloc_str(&self.context.runtime_names().builder_param_proxy),
                ast,
            ),
            None,
            args,
            false,
            ast,
        );
        Some(Argument::from(call))
    }
}

pub(super) struct BuilderParamProxyObjectEmitter<'a> {
    pub(super) context: &'a BuildContext,
}

impl<'a> BuilderParamProxyObjectEmitter<'a> {
    pub(super) fn new(context: &'a BuildContext) -> Self {
        Self { context }
    }

    pub(super) fn object_expression<'b>(
        &self,
        object: &ObjectExpression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let properties = OxcVec::from_iter_in(
            object
                .properties
                .iter()
                .map(|property| self.object_property(property, allocator)),
            ast,
        );
        Expression::new_object_expression(object.span, properties, ast)
    }

    pub(super) fn object_property<'b>(
        &self,
        property: &ObjectPropertyKind<'_>,
        allocator: &'b Allocator,
    ) -> ObjectPropertyKind<'b> {
        let ast = &AstBuilder::new(allocator);
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                let value = self.arrow_expression(&property.value, allocator);
                ObjectPropertyKind::new_object_property(
                    property.span,
                    property.kind,
                    property.key.clone_in(allocator),
                    value,
                    false,
                    property.shorthand,
                    property.computed,
                    ast,
                )
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                let value = BuildArgumentEmitter::new()
                    .with_context(self.context)
                    .expression_with_allocator(&spread.argument, allocator);
                ObjectPropertyKind::new_spread_property(spread.span, value, ast)
            }
        }
    }

    pub(super) fn arrow_expression<'b>(
        &self,
        value: &Expression<'_>,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let return_value = self.return_expression(value, allocator);
        let body = ArrowFunctionBody::from(return_value);
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
            body,
            ast,
        )
    }

    pub(super) fn return_expression<'b>(
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

    pub(super) fn proxy_property<'b>(
        value: &'b Expression<'_>,
    ) -> Option<(BuilderProxyReceiver, &'b str)> {
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

    pub(super) fn property_fallback<'b>(
        &self,
        receiver: BuilderProxyReceiver,
        name: &str,
        allocator: &'b Allocator,
    ) -> Expression<'b> {
        let ast = &AstBuilder::new(allocator);
        let backing = self.computed_member(ast, receiver, &format!("__{name}"));
        let consequent = self.computed_member(ast, receiver, &format!("__{name}"));
        let alternate = self.computed_member(ast, receiver, name);
        Expression::new_conditional_expression(SPAN, backing, consequent, alternate, ast)
    }

    pub(super) fn computed_member<'b>(
        &self,
        ast: &AstBuilder<'b>,
        receiver: BuilderProxyReceiver,
        name: &str,
    ) -> Expression<'b> {
        let object = match receiver {
            BuilderProxyReceiver::This => Expression::new_this_expression(SPAN, ast),
            BuilderProxyReceiver::BuilderParameter => {
                Expression::new_identifier(SPAN, ast.allocator().alloc_str("$$"), ast)
            }
        };
        Expression::from(MemberExpression::new_computed_member_expression(
            SPAN,
            object,
            Expression::new_string_literal(SPAN, ast.allocator().alloc_str(name), None, ast),
            false,
            ast,
        ))
    }
}

#[derive(Clone, Copy)]
pub(super) enum BuilderProxyReceiver {
    This,
    BuilderParameter,
}
