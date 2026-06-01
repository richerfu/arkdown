use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionExpression, BindingPattern,
    BlockStatement, CallExpression, ChainElement, Expression, FormalParameter, FunctionBody,
    IfStatement, ObjectExpression, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_span::{GetSpan, Span};

use crate::transform::component_meta::ComponentMetadata;
use crate::transform::global_rewrite::rewrite_global_context_calls_in_expression_if_needed;
use crate::transform::source_emit::original_node_text;
use crate::transform::viewpu_emit::ViewpuWriter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLoweringError {
    UnsupportedStatement,
    UnsupportedChild,
    UnsupportedCallee(String),
    UnsupportedChain,
}

/// Records the position where an ArkUI component call was emitted in the
/// lowered body text, along with the original `.ets` span of the component
/// expression.  Used to build a cross-transform source map.
#[derive(Debug, Clone)]
pub struct SpanHint {
    /// 0-based line index within the `initialRender()` body text.
    pub body_line: usize,
    /// 0-based column index within the generated line.
    pub body_col: u32,
    /// Byte span of the ArkUI component expression in the original `.ets` source.
    pub original_span: Span,
}

#[derive(Clone, Debug, Default)]
pub struct CustomComponentInfo {
    pub update_props: HashSet<String>,
    pub link_props: HashSet<String>,
    pub reusable: bool,
}

pub type CustomComponentUpdateProps = HashMap<String, CustomComponentInfo>;

#[derive(Clone, Debug, Default)]
struct BuildContext {
    use_parent_context: bool,
    raw_object_fields: Arc<HashSet<String>>,
}

impl BuildContext {
    fn builder() -> Self {
        Self {
            use_parent_context: true,
            ..Default::default()
        }
    }

    fn view_context(&self) -> &'static str {
        if self.use_parent_context {
            "(parent ? parent : this)"
        } else {
            "this"
        }
    }

    fn with_raw_object_fields(mut self, raw_object_fields: &HashSet<String>) -> Self {
        self.raw_object_fields = Arc::new(raw_object_fields.clone());
        self
    }

    fn is_raw_object_field(&self, name: &str) -> bool {
        self.raw_object_fields.contains(name)
    }
}

/// Extracted representation of a leaf component call and its attribute chain.
struct LeafComponent {
    name: String,
    create_args: String,
    chain_attrs: Vec<(String, String, bool)>,
}

fn create_method_name(component_name: &str, has_children: bool) -> &'static str {
    if component_name == "Button" {
        if has_children {
            "createWithChild"
        } else {
            "createWithLabel"
        }
    } else {
        "create"
    }
}

fn push_attribute_calls(
    out: &mut String,
    indent: &str,
    component_name: &str,
    attrs: &[(String, String, bool)],
) {
    let mut writer = ViewpuWriter::new(out);
    let attr_indent = format!("{indent}    ");
    for (attr_name, attr_args, _) in attrs {
        writer.component_attribute(&attr_indent, component_name, attr_name, attr_args);
    }
}

fn arguments_are_regular_attrs(
    arguments: &[Argument<'_>],
    component_metadata: &ComponentMetadata,
) -> bool {
    arguments.iter().all(|argument| {
        argument
            .as_expression()
            .is_some_and(|expression| is_regular_attr_expression(expression, component_metadata))
    })
}

fn is_regular_attr_expression(
    expression: &Expression<'_>,
    component_metadata: &ComponentMetadata,
) -> bool {
    match expression {
        Expression::ObjectExpression(object) => object.properties.iter().all(|property| {
            matches!(
                property,
                ObjectPropertyKind::ObjectProperty(property)
                    if is_regular_attr_expression(&property.value, component_metadata)
            )
        }),
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
            element.as_expression().is_some_and(|expression| {
                is_regular_attr_expression(expression, component_metadata)
            })
        }),
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::ArrowFunctionExpression(_) => true,
        Expression::StaticMemberExpression(member) => {
            matches!(
                &member.object,
                Expression::Identifier(identifier) if component_metadata.is_known_enum(identifier.name.as_str())
            )
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            is_regular_attr_expression(&parenthesized.expression, component_metadata)
        }
        _ => false,
    }
}

/// Recursively unwrap a chained `CallExpression` into its base component name,
/// constructor arguments, and ordered attribute calls.
///
/// For `Button("OK").width(100).height(40)` the outer call is `.height(40)`.
/// Recursion reaches the base `Button("OK")` and then collects attrs in
/// source order on the way back up: `[("width", "100"), ("height", "40")]`.
fn extract_leaf_component(
    call: &CallExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    component_metadata: &ComponentMetadata,
    context: &BuildContext,
) -> Result<LeafComponent, BuildLoweringError> {
    match &call.callee {
        Expression::Identifier(_) => {
            let name = component_name(&call.callee)?;
            let create_args =
                raw_arguments_text_with_context(&call.arguments, source_text, context);
            Ok(LeafComponent {
                name,
                create_args,
                chain_attrs: vec![],
            })
        }
        Expression::StaticMemberExpression(member) => {
            let attr_name = member.property.name.to_string();
            let attr_args = attribute_arguments_text(
                &attr_name,
                &call.arguments,
                source_text,
                builder_methods,
                context,
            );
            match &member.object {
                Expression::CallExpression(inner) => {
                    let mut lc = extract_leaf_component(
                        inner,
                        source_text,
                        source_id,
                        custom_components,
                        builder_methods,
                        component_metadata,
                        context,
                    )?;
                    lc.chain_attrs.push((
                        attr_name,
                        attr_args,
                        arguments_are_regular_attrs(&call.arguments, component_metadata),
                    ));
                    Ok(lc)
                }
                _ => Err(BuildLoweringError::UnsupportedCallee(
                    span_text(&member.object, source_text).trim().to_string(),
                )),
            }
        }
        _ => Err(BuildLoweringError::UnsupportedCallee(
            span_text(&call.callee, source_text).trim().to_string(),
        )),
    }
}

/// Lower a leaf component `CallExpression` (no block body) to ViewPU JS.
///
/// Handles both plain calls (`Divider()`) and chained attribute calls
/// (`Button("OK").width(100).height(40)`). The OXC ETS parser only creates
/// `ArkUIComponentExpression` nodes for component calls that have a block
/// body `{ }`. Leaf components without blocks appear as regular
/// `CallExpression` nodes in the top-level build() statement list.
fn lower_call_expression(
    call: &CallExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    if matches_component_call(call, "ForEach") {
        return lower_for_each(
            &call.arguments,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context.clone(),
            indent,
            out,
            span_hints,
        );
    }

    let lc = extract_leaf_component(
        call,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        component_metadata,
        &context,
    )?;
    if custom_components.contains(&lc.name)
        && (lc.chain_attrs.is_empty()
            || custom_component_update_props
                .get(&lc.name)
                .is_some_and(|info| {
                    info.reusable && lc.chain_attrs.iter().all(|(name, _, _)| name == "reuseId")
                }))
    {
        let base_call = base_component_call(call);
        emit_custom_component_call(
            &lc.name,
            &base_call.arguments,
            custom_component_update_props.get(&lc.name),
            custom_reuse_id_from_leaf_attrs(&lc.chain_attrs),
            base_call.span,
            source_text,
            source_id,
            context.clone(),
            indent,
            out,
            span_hints,
        );
        return Ok(());
    }
    {
        let mut writer = ViewpuWriter::new(out);
        let create_indent = format!("{indent}    ");
        writer.observe_component_creation_start(indent, context.view_context());
        writer.component_create(
            &create_indent,
            &lc.name,
            create_method_name(&lc.name, false),
            &lc.create_args,
        );
    }
    push_attribute_calls(out, indent, &lc.name, &lc.chain_attrs);
    let mut writer = ViewpuWriter::new(out);
    writer.observe_component_creation_end(indent, &lc.name);
    if component_metadata.needs_pop(&lc.name) {
        writer.component_pop(indent, &lc.name);
    }
    Ok(())
}

fn base_component_call<'a>(call: &'a CallExpression<'a>) -> &'a CallExpression<'a> {
    let mut current = call;
    while let Expression::StaticMemberExpression(member) = &current.callee {
        let Expression::CallExpression(inner) = &member.object else {
            break;
        };
        current = inner;
    }
    current
}

/// Lower an ArkUI `build()` body using the ohos-rs/oxc ArkUI AST.
///
/// This intentionally covers only the minimal fixture shape confirmed from
/// hvigor bytecode disassembly:
/// `Component(args) { children }.attr(args)` lowers to an
/// `observeComponentCreation2` callback containing `create` and chained
/// attribute calls, followed by child lowering and a post-order `pop`.
///
/// Returns the lowered text together with span hints that map each emitted
/// `observeComponentCreation2` call back to the original `.ets` component
/// expression.  The caller uses these to build a cross-transform source map.
pub fn lower_build_body(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_custom_components(
        body,
        source_text,
        "",
        &HashSet::new(),
        &HashSet::new(),
        &CustomComponentUpdateProps::new(),
        &ComponentMetadata::default(),
    )
}

pub fn lower_build_body_with_custom_components(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        BuildContext::default(),
    )
}

pub fn lower_build_body_with_component_context(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &HashSet<String>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        BuildContext::default().with_raw_object_fields(raw_object_fields),
    )
}

pub fn lower_builder_body_with_custom_components(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        BuildContext::builder(),
    )
}

pub fn lower_builder_body_with_component_context(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &HashSet<String>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        BuildContext::builder().with_raw_object_fields(raw_object_fields),
    )
}

fn lower_build_body_with_context(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    let Some(body) = body else {
        return Ok((String::new(), vec![]));
    };

    let mut out = String::new();
    let mut span_hints: Vec<SpanHint> = Vec::new();
    for statement in &body.statements {
        lower_statement(
            statement,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context.clone(),
            "        ",
            &mut out,
            &mut span_hints,
        )?;
    }

    Ok((out, span_hints))
}

fn lower_statement(
    statement: &Statement<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    match statement {
        Statement::BlockStatement(block) => lower_block(
            block,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context.clone(),
            indent,
            out,
            span_hints,
        ),
        Statement::EmptyStatement(_) => Ok(()),
        Statement::ExpressionStatement(statement) => lower_expression(
            &statement.expression,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        ),
        Statement::IfStatement(statement) => lower_if_statement(
            statement,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        ),
        _ => Err(BuildLoweringError::UnsupportedStatement),
    }
}

fn lower_block(
    block: &BlockStatement<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    for statement in &block.body {
        lower_statement(
            statement,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context.clone(),
            indent,
            out,
            span_hints,
        )?;
    }
    Ok(())
}

fn lower_expression(
    expression: &Expression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    match expression {
        Expression::ArkUIComponentExpression(component) => lower_component(
            component,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        ),
        Expression::CallExpression(call) => {
            if lower_builder_method_call(call, source_text, builder_methods, &context, indent, out)
            {
                Ok(())
            } else {
                lower_call_expression(
                    call,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context,
                    indent,
                    out,
                    span_hints,
                )
            }
        }
        _ => Err(BuildLoweringError::UnsupportedStatement),
    }
}

fn lower_if_statement(
    statement: &IfStatement<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let test = original_node_text(&statement.test, source_text);
    let branch_indent = format!("{indent}    ");
    let inner_indent = format!("{branch_indent}    ");

    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation_start(indent, context.view_context());
        writer.line(&branch_indent, format_args!("If.create();"));
        writer.line(&branch_indent, format_args!("if ({test}) {{"));
        writer.line(
            &inner_indent,
            format_args!(
                "{}.ifElseBranchUpdateFunction(0, () => {{",
                context.view_context()
            ),
        );
    }
    lower_statement(
        &statement.consequent,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &format!("{inner_indent}    "),
        out,
        span_hints,
    )?;
    {
        let mut writer = ViewpuWriter::new(out);
        writer.line(&inner_indent, format_args!("}});"));
    }

    let mut branch_index = 1usize;
    if let Some(alternate) = &statement.alternate {
        {
            let mut writer = ViewpuWriter::new(out);
            writer.line(&branch_indent, format_args!("}} else {{"));
            writer.line(
                &inner_indent,
                format_args!(
                    "{}.ifElseBranchUpdateFunction({branch_index}, () => {{",
                    context.view_context()
                ),
            );
        }
        match alternate {
            Statement::IfStatement(alternate_if) => {
                lower_if_statement(
                    alternate_if,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    &format!("{inner_indent}    "),
                    out,
                    span_hints,
                )?;
            }
            _ => {
                lower_statement(
                    alternate,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    &format!("{inner_indent}    "),
                    out,
                    span_hints,
                )?;
            }
        }
        {
            let mut writer = ViewpuWriter::new(out);
            writer.line(&inner_indent, format_args!("}});"));
        }
        branch_index += 1;
    }
    if branch_index == 1 {
        let mut writer = ViewpuWriter::new(out);
        writer.line(&branch_indent, format_args!("}} else {{"));
        writer.line(
            &inner_indent,
            format_args!(
                "{}.ifElseBranchUpdateFunction(1, () => {{",
                context.view_context()
            ),
        );
        writer.line(&inner_indent, format_args!("}});"));
    }
    let mut writer = ViewpuWriter::new(out);
    writer.line(&branch_indent, format_args!("}}"));
    writer.observe_component_creation_end(indent, "If");
    writer.component_pop(indent, "If");

    Ok(())
}

fn lower_component(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let name = component_name(&component.callee)?;

    if builder_methods.contains(&name) {
        if !component.children.is_empty() {
            return Err(BuildLoweringError::UnsupportedChild);
        }
        let args = raw_arguments_text_with_context(&component.arguments, source_text, &context);
        out.push_str(&format!("{indent}{name}.bind(this)({args});\n"));
        return Ok(());
    }

    if custom_components.contains(&name) {
        emit_custom_component_call(
            &name,
            &component.arguments,
            custom_component_update_props.get(&name),
            custom_reuse_id_from_component_attrs(component, source_text),
            component.callee.span(),
            source_text,
            source_id,
            context,
            indent,
            out,
            span_hints,
        );
        return Ok(());
    }

    if name == "ForEach" {
        return lower_for_each(
            &component.arguments,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        );
    }
    if name == "LazyForEach" {
        return lower_lazy_for_each(
            component,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        );
    }

    let args = raw_arguments_text_with_context(&component.arguments, source_text, &context);
    if name == "ListItem"
        && !component.children.is_empty()
        && is_lazy_list_item(component, source_text)
    {
        return lower_lazy_list_item(
            component,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context,
            indent,
            out,
            span_hints,
        );
    }

    let body_line = out.bytes().filter(|&b| b == b'\n').count();

    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation_start(indent, context.view_context());
    }
    push_span_hint(
        span_hints,
        body_line + 1,
        indent.len() + 4,
        component.callee.span(),
    );
    push_span_hint(
        span_hints,
        body_line + 1,
        indent.len() + 4 + name.len(),
        end_span(component.callee.span()),
    );
    push_argument_hints(
        span_hints,
        body_line + 1,
        indent.len() + 4 + name.len() + ".create(".len(),
        &component.arguments,
        source_text,
    );
    {
        let mut writer = ViewpuWriter::new(out);
        let create_indent = format!("{indent}    ");
        writer.component_create(
            &create_indent,
            &name,
            create_method_name(&name, !component.children.is_empty()),
            &args,
        );
    }
    let mut attrs = Vec::new();
    for (index, chain) in component.chain_expressions.iter().enumerate() {
        let chain_name = chain_name(chain)?;
        let chain_args = attribute_arguments_text(
            &chain_name,
            &chain.arguments,
            source_text,
            builder_methods,
            &context,
        );
        let line = body_line + 2 + index;
        push_span_hint(span_hints, line, indent.len() + 4, component.callee.span());
        push_span_hint(
            span_hints,
            line,
            indent.len() + 4 + name.len(),
            end_span(component.callee.span()),
        );
        if let Some(property_span) = chain_property_span(chain) {
            let property_col = indent.len() + 4 + name.len() + 1;
            push_span_hint(span_hints, line, property_col, property_span);
            push_span_hint(
                span_hints,
                line,
                property_col + chain_name.len(),
                end_span(property_span),
            );
        }
        push_argument_hints(
            span_hints,
            line,
            indent.len() + 4 + name.len() + 1 + chain_name.len() + 1,
            &chain.arguments,
            source_text,
        );
        attrs.push((
            chain_name,
            chain_args,
            arguments_are_regular_attrs(&chain.arguments, component_metadata),
        ));
    }
    push_attribute_calls(out, indent, &name, &attrs);
    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation_end(indent, &name);
    }

    for child in &component.children {
        match child {
            ArkUIChild::Component(child) => {
                lower_component(
                    child,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    indent,
                    out,
                    span_hints,
                )?;
            }
            ArkUIChild::Expression(expression) => {
                lower_expression(
                    expression,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    indent,
                    out,
                    span_hints,
                )?;
            }
            ArkUIChild::Statement(statement) => {
                lower_statement(
                    statement,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    indent,
                    out,
                    span_hints,
                )?;
            }
        }
    }

    if component_metadata.needs_pop(&name) {
        let pop_line = out.bytes().filter(|&b| b == b'\n').count();
        push_span_hint(span_hints, pop_line, indent.len(), component.callee.span());
        push_span_hint(
            span_hints,
            pop_line,
            indent.len() + name.len(),
            end_span(component.callee.span()),
        );
        let mut writer = ViewpuWriter::new(out);
        writer.component_pop(indent, &name);
    }
    Ok(())
}

fn emit_custom_component_call(
    name: &str,
    arguments: &[Argument<'_>],
    component_info: Option<&CustomComponentInfo>,
    reuse_id: Option<String>,
    span: Span,
    source_text: &str,
    source_id: &str,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) {
    if component_info.is_some_and(|info| info.reusable) {
        emit_reusable_custom_component_call(
            name,
            arguments,
            component_info,
            reuse_id,
            span,
            source_text,
            source_id,
            context,
            indent,
            out,
            span_hints,
        );
        return;
    }

    let body_line = out.bytes().filter(|&b| b == b'\n').count();
    push_span_hint(span_hints, body_line, indent.len(), span);

    let params = custom_component_params(arguments, source_text, component_info, &context);
    let (line, col) = source_position(source_text, span.start);
    let page = component_page_path(source_id);
    out.push_str(&format!("{indent}{{\n"));
    let inner = format!("{indent}    ");
    let inner2 = format!("{inner}    ");
    let inner3 = format!("{inner}        ");
    let inner4 = format!("{inner}            ");
    out.push_str(&format!(
        "{inner}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        context.view_context()
    ));
    out.push_str(&format!("{inner2}if (isInitialRender) {{\n"));
    out.push_str(&format!(
        "{inner3}let componentCall = new {name}({}, {}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {line}, col: {col} }});\n",
        context.view_context(),
        params.constructor
    ));
    out.push_str(&format!("{inner3}ViewPU.create(componentCall);\n"));
    out.push_str(&format!("{inner3}let paramsLambda = () => {{\n"));
    out.push_str(&format!("{inner4}return {};\n", params.lambda));
    out.push_str(&format!("{inner3}}};\n"));
    out.push_str(&format!(
        "{inner3}componentCall.paramsGenerator_ = paramsLambda;\n"
    ));
    out.push_str(&format!("{inner2}}} else {{\n"));
    out.push_str(&format!(
        "{inner3}{}.updateStateVarsOfChildByElmtId(elmtId, {});\n",
        context.view_context(),
        params.update
    ));
    out.push_str(&format!("{inner2}}}\n"));
    out.push_str(&format!("{inner}}}, {{ name: \"{name}\" }});\n"));
    out.push_str(&format!("{indent}}}\n"));
}

fn emit_reusable_custom_component_call(
    name: &str,
    arguments: &[Argument<'_>],
    component_info: Option<&CustomComponentInfo>,
    reuse_id: Option<String>,
    span: Span,
    source_text: &str,
    source_id: &str,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) {
    let body_line = out.bytes().filter(|&b| b == b'\n').count();
    push_span_hint(span_hints, body_line, indent.len(), span);

    let params = custom_component_params(arguments, source_text, component_info, &context);
    let (line, col) = source_position(source_text, span.start);
    let page = component_page_path(source_id);
    let reuse_id = reuse_id.unwrap_or_else(|| format!("'{name}'"));

    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        context.view_context()
    ));
    out.push_str(&format!("{indent}    __Recycle__.create();\n"));
    out.push_str(&format!("{indent}}}, __Recycle__);\n"));
    out.push_str(&format!("{indent}{{\n"));
    let inner = format!("{indent}    ");
    let inner2 = format!("{inner}    ");
    let inner3 = format!("{inner}        ");
    let inner4 = format!("{inner}            ");
    let inner5 = format!("{inner}                ");
    out.push_str(&format!(
        "{inner}{}.observeRecycleComponentCreation({reuse_id}, (elmtId, isInitialRender, recycleNode = null) => {{\n",
        context.view_context()
    ));
    out.push_str(&format!(
        "{inner}    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);\n"
    ));
    out.push_str(&format!("{inner2}if (isInitialRender) {{\n"));
    out.push_str(&format!(
        "{inner3}let componentCall = recycleNode ? recycleNode : new {name}({}, {}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {line}, col: {col} }});\n",
        context.view_context(),
        params.constructor
    ));
    out.push_str(&format!(
        "{inner3}ViewPU.createRecycle(componentCall, recycleNode !== null, {reuse_id}, () => {{\n"
    ));
    out.push_str(&format!(
        "{inner4}if (recycleNode && typeof recycleNode.aboutToReuseInternal === \"function\") {{\n"
    ));
    out.push_str(&format!("{inner5}recycleNode.aboutToReuseInternal();\n"));
    out.push_str(&format!("{inner4}}} else {{\n"));
    out.push_str(&format!(
        "{inner5}if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === \"function\") {{\n"
    ));
    out.push_str(&format!(
        "{}recycleNode.aboutToReuse({});\n",
        format!("{inner}                    "),
        params.lambda
    ));
    out.push_str(&format!("{inner5}}}\n"));
    out.push_str(&format!("{inner5}recycleNode.rerender();\n"));
    out.push_str(&format!("{inner4}}}\n"));
    out.push_str(&format!("{inner3}}});\n"));
    out.push_str(&format!("{inner3}let paramsLambda = () => {{\n"));
    out.push_str(&format!("{inner4}return {};\n", params.lambda));
    out.push_str(&format!("{inner3}}};\n"));
    out.push_str(&format!(
        "{inner3}componentCall.paramsGenerator_ = paramsLambda;\n"
    ));
    out.push_str(&format!("{inner2}}} else {{\n"));
    out.push_str(&format!(
        "{inner3}{}.updateStateVarsOfChildByElmtId(elmtId, {});\n",
        context.view_context(),
        params.update
    ));
    out.push_str(&format!("{inner2}}}\n"));
    out.push_str(&format!(
        "{inner}    ViewStackProcessor.StopGetAccessRecording();\n"
    ));
    out.push_str(&format!("{inner}}});\n"));
    out.push_str(&format!("{indent}}}\n"));
    out.push_str(&format!("{indent}__Recycle__.pop();\n"));
}

fn custom_reuse_id_from_leaf_attrs(attrs: &[(String, String, bool)]) -> Option<String> {
    attrs
        .iter()
        .find(|(name, _, _)| name == "reuseId")
        .map(|(_, args, _)| args.trim().to_string())
}

fn custom_reuse_id_from_component_attrs(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
) -> Option<String> {
    component.chain_expressions.iter().find_map(|chain| {
        let name = chain_name(chain).ok()?;
        if name == "reuseId" {
            Some(
                raw_arguments_text(&chain.arguments, source_text)
                    .trim()
                    .to_string(),
            )
        } else {
            None
        }
    })
}

struct CustomComponentParams {
    constructor: String,
    lambda: String,
    update: String,
}

fn custom_component_params(
    arguments: &[Argument<'_>],
    source_text: &str,
    component_info: Option<&CustomComponentInfo>,
    context: &BuildContext,
) -> CustomComponentParams {
    let Some(Argument::ObjectExpression(object)) = arguments.first() else {
        let params = if arguments.is_empty() {
            "{}".to_string()
        } else {
            raw_arguments_text(arguments, source_text)
        };
        return CustomComponentParams {
            constructor: params.clone(),
            lambda: params,
            update: "{}".to_string(),
        };
    };

    CustomComponentParams {
        constructor: custom_component_object_text(
            object,
            source_text,
            component_info,
            ParamMode::Ctor,
            context,
        ),
        lambda: custom_component_object_text(
            object,
            source_text,
            component_info,
            ParamMode::Lambda,
            context,
        ),
        update: custom_component_object_text(
            object,
            source_text,
            component_info,
            ParamMode::Update,
            context,
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamMode {
    Ctor,
    Lambda,
    Update,
}

fn custom_component_object_text(
    object: &ObjectExpression<'_>,
    source_text: &str,
    component_info: Option<&CustomComponentInfo>,
    mode: ParamMode,
    context: &BuildContext,
) -> String {
    let properties = object
        .properties
        .iter()
        .filter_map(|property| {
            custom_component_property_text(property, source_text, component_info, mode, context)
        })
        .collect::<Vec<_>>();
    if properties.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n      }}", properties.join(",\n"))
    }
}

fn custom_component_property_text(
    property: &ObjectPropertyKind<'_>,
    source_text: &str,
    component_info: Option<&CustomComponentInfo>,
    mode: ParamMode,
    context: &BuildContext,
) -> Option<String> {
    match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            let key_name = property_key_name(&property.key);
            if mode == ParamMode::Update
                && !key_name.as_deref().is_some_and(|name| {
                    component_info.is_some_and(|info| info.update_props.contains(name))
                })
            {
                return None;
            }
            let key = original_node_text(&property.key, source_text);
            let value = custom_component_value_text(
                &property.value,
                source_text,
                key_name.as_deref(),
                component_info,
                mode,
                context,
            );
            Some(format!("        {key}: {value}"))
        }
        ObjectPropertyKind::SpreadProperty(spread) => {
            if mode == ParamMode::Update {
                None
            } else {
                Some(format!(
                    "        {}",
                    original_node_text(spread.as_ref(), source_text)
                ))
            }
        }
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn custom_component_value_text(
    value: &Expression<'_>,
    source_text: &str,
    property_name: Option<&str>,
    component_info: Option<&CustomComponentInfo>,
    mode: ParamMode,
    context: &BuildContext,
) -> String {
    if let Some(name) = dollar_reference_name(value) {
        return match mode {
            ParamMode::Ctor => format!("this.__{name}"),
            ParamMode::Lambda => format!("this.{name}"),
            ParamMode::Update => original_node_text(value, source_text),
        };
    }
    if mode == ParamMode::Ctor
        && property_name
            .is_some_and(|name| component_info.is_some_and(|info| info.link_props.contains(name)))
    {
        if let Some(name) = this_member_name(value) {
            return format!("this.__{name}");
        }
    }
    if matches!(mode, ParamMode::Ctor | ParamMode::Lambda) {
        if let Some(text) = expression_text_with_raw_object_call_args(value, source_text, context) {
            return text;
        }
    }
    original_node_text(value, source_text)
}

fn this_member_name(value: &Expression<'_>) -> Option<String> {
    let Expression::StaticMemberExpression(member) = value else {
        return None;
    };
    matches!(&member.object, Expression::ThisExpression(_))
        .then(|| member.property.name.to_string())
}

fn expression_text_with_raw_object_call_args(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    let mut replacements = Vec::new();
    collect_raw_object_call_arg_replacements(expression, source_text, context, &mut replacements);
    apply_span_replacements(expression.span(), source_text, replacements)
}

fn collect_raw_object_call_arg_replacements(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    match expression {
        Expression::CallExpression(call) => {
            collect_call_raw_object_replacements(call, source_text, context, replacements);
        }
        Expression::ChainExpression(chain) => {
            if let ChainElement::CallExpression(call) = &chain.expression {
                collect_call_raw_object_replacements(call, source_text, context, replacements);
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            collect_function_body_raw_object_replacements(
                &arrow.body,
                source_text,
                context,
                replacements,
            );
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            collect_raw_object_call_arg_replacements(
                &parenthesized.expression,
                source_text,
                context,
                replacements,
            );
        }
        _ => {}
    }
}

fn collect_call_raw_object_replacements(
    call: &CallExpression<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    for argument in &call.arguments {
        let Some(expression) = argument.as_expression() else {
            continue;
        };
        if let Some(raw_object_text) = raw_object_argument_text(expression, source_text, context) {
            replacements.push((argument.span(), raw_object_text));
        } else {
            collect_raw_object_call_arg_replacements(
                expression,
                source_text,
                context,
                replacements,
            );
        }
    }
}

fn collect_function_body_raw_object_replacements(
    body: &FunctionBody<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    for statement in &body.statements {
        collect_statement_raw_object_replacements(statement, source_text, context, replacements);
    }
}

fn collect_statement_raw_object_replacements(
    statement: &Statement<'_>,
    source_text: &str,
    context: &BuildContext,
    replacements: &mut Vec<(Span, String)>,
) {
    match statement {
        Statement::ExpressionStatement(statement) => collect_raw_object_call_arg_replacements(
            &statement.expression,
            source_text,
            context,
            replacements,
        ),
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_raw_object_call_arg_replacements(
                    argument,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_statement_raw_object_replacements(
                    statement,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        Statement::IfStatement(statement) => {
            collect_statement_raw_object_replacements(
                &statement.consequent,
                source_text,
                context,
                replacements,
            );
            if let Some(alternate) = &statement.alternate {
                collect_statement_raw_object_replacements(
                    alternate,
                    source_text,
                    context,
                    replacements,
                );
            }
        }
        _ => {}
    }
}

fn raw_object_argument_text(
    expression: &Expression<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    if !matches!(&member.object, Expression::ThisExpression(_)) {
        return None;
    }
    let name = member.property.name.as_str();
    if !context.is_raw_object_field(name) {
        return None;
    }
    Some(format!(
        "ObservedObject.GetRawObject({})",
        original_node_text(expression, source_text)
    ))
}

fn raw_object_argument_text_from_argument(
    argument: &Argument<'_>,
    source_text: &str,
    context: &BuildContext,
) -> Option<String> {
    argument
        .as_expression()
        .and_then(|expression| raw_object_argument_text(expression, source_text, context))
}

fn apply_span_replacements(
    base_span: Span,
    source_text: &str,
    mut replacements: Vec<(Span, String)>,
) -> Option<String> {
    if replacements.is_empty() {
        return None;
    }
    replacements.sort_by_key(|(span, _)| span.start);
    let mut out = String::new();
    let mut cursor = base_span.start as usize;
    for (span, replacement) in replacements {
        let start = span.start as usize;
        let end = span.end as usize;
        if start < cursor || end > base_span.end as usize {
            continue;
        }
        out.push_str(&source_text[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&source_text[cursor..base_span.end as usize]);
    Some(out.trim().to_string())
}

fn dollar_reference_name(value: &Expression<'_>) -> Option<String> {
    match value {
        Expression::Identifier(identifier) => identifier
            .name
            .as_str()
            .strip_prefix('$')
            .map(str::to_string),
        Expression::StaticMemberExpression(member)
            if matches!(&member.object, Expression::ThisExpression(_)) =>
        {
            member
                .property
                .name
                .as_str()
                .strip_prefix('$')
                .map(str::to_string)
        }
        _ => None,
    }
}

fn is_lazy_list_item(component: &ArkUIComponentExpression<'_>, source_text: &str) -> bool {
    !component
        .arguments
        .first()
        .is_some_and(|arg| raw_argument_text(arg, source_text).trim() == "\"false\"")
}

fn lower_lazy_list_item(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    out.push_str(&format!("{indent}{{\n"));
    let inner = format!("{indent}    ");
    out.push_str(&format!(
        "{inner}const itemCreation = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{inner}    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);\n"
    ));
    out.push_str(&format!(
        "{inner}    ListItem.create(deepRenderFunction, true);\n"
    ));
    out.push_str(&format!("{inner}    if (!isInitialRender) {{\n"));
    out.push_str(&format!("{inner}        ListItem.pop();\n"));
    out.push_str(&format!("{inner}    }}\n"));
    out.push_str(&format!(
        "{inner}    ViewStackProcessor.StopGetAccessRecording();\n"
    ));
    out.push_str(&format!("{inner}}};\n"));
    out.push_str(&format!(
        "{inner}const itemCreation2 = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{inner}    ListItem.create(deepRenderFunction, true);\n"
    ));
    for chain in &component.chain_expressions {
        let chain_name = chain_name(chain)?;
        let chain_args = attribute_arguments_text(
            &chain_name,
            &chain.arguments,
            source_text,
            builder_methods,
            &context,
        );
        out.push_str(&format!(
            "{inner}    ListItem.{chain_name}({chain_args});\n"
        ));
    }
    out.push_str(&format!("{inner}}};\n"));
    out.push_str(&format!(
        "{inner}const deepRenderFunction = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{inner}    itemCreation(elmtId, isInitialRender);\n"
    ));
    for child in &component.children {
        match child {
            ArkUIChild::Component(child) => {
                lower_component(
                    child,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    &format!("{inner}        "),
                    out,
                    span_hints,
                )?;
            }
            ArkUIChild::Expression(expression) => {
                lower_expression(
                    expression,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    &format!("{inner}        "),
                    out,
                    span_hints,
                )?;
            }
            ArkUIChild::Statement(statement) => {
                lower_statement(
                    statement,
                    source_text,
                    source_id,
                    custom_components,
                    builder_methods,
                    custom_component_update_props,
                    component_metadata,
                    context.clone(),
                    &format!("{inner}        "),
                    out,
                    span_hints,
                )?;
            }
        }
    }
    out.push_str(&format!("{inner}    ListItem.pop();\n"));
    out.push_str(&format!("{inner}}};\n"));
    out.push_str(&format!(
        "{inner}{}.observeComponentCreation2(itemCreation2, ListItem);\n",
        context.view_context()
    ));
    out.push_str(&format!("{inner}ListItem.pop();\n"));
    out.push_str(&format!("{indent}}}\n"));
    Ok(())
}

fn lower_for_each(
    arguments: &[Argument<'_>],
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let Some(data_arg) = arguments.first() else {
        return Err(BuildLoweringError::UnsupportedChild);
    };
    let Some(item_gen_arg) = arguments.get(1) else {
        return Err(BuildLoweringError::UnsupportedChild);
    };
    let Some(item_gen) = argument_arrow(item_gen_arg) else {
        return Err(BuildLoweringError::UnsupportedChild);
    };

    let data = raw_object_argument_text_from_argument(data_arg, source_text, &context)
        .unwrap_or_else(|| raw_argument_text(data_arg, source_text));
    let item_param = item_gen.params.items.first();
    let index_param = item_gen.params.items.get(1);
    let generator_params = match (item_param, index_param) {
        (None, _) => "()".to_string(),
        (Some(_), Some(index_param)) => {
            format!("(_item, {})", original_node_text(index_param, source_text))
        }
        (Some(_), None) => "_item".to_string(),
    };

    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        context.view_context()
    ));
    out.push_str(&format!("{indent}    ForEach.create();\n"));
    out.push_str(&format!(
        "{indent}    const forEachItemGenFunction = {generator_params} => {{\n"
    ));
    if let Some(item_param) = item_param {
        let Some(item_name) = binding_parameter_name(item_param) else {
            return Err(BuildLoweringError::UnsupportedChild);
        };
        out.push_str(&format!("{indent}        const {item_name} = _item;\n"));
    }
    lower_lazy_arrow_body(
        item_gen,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &format!("{indent}        "),
        out,
        span_hints,
    )?;
    out.push_str(&format!("{indent}    }};\n"));

    if item_param.is_some() {
        if let Some(key_arg) = arguments.get(2) {
            let key = raw_argument_text_with_context(key_arg, source_text, &context);
            let item_gen_uses_index = index_param.is_some();
            let key_uses_index = argument_arrow(key_arg)
                .and_then(|arrow| arrow.params.items.get(1))
                .is_some();
            out.push_str(&format!(
            "{indent}    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction, {key}, {item_gen_uses_index}, {key_uses_index});\n",
            context.view_context()
        ));
        } else {
            out.push_str(&format!(
                "{indent}    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction);\n",
                context.view_context()
            ));
        }
    } else {
        out.push_str(&format!(
            "{indent}    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction);\n",
            context.view_context()
        ));
    }

    out.push_str(&format!("{indent}}}, ForEach);\n"));
    out.push_str(&format!("{indent}ForEach.pop();\n"));
    Ok(())
}

fn lower_lazy_for_each(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let Some(data_arg) = component.arguments.first() else {
        return Err(BuildLoweringError::UnsupportedChild);
    };
    let Some(item_gen_arg) = component.arguments.get(1) else {
        return Err(BuildLoweringError::UnsupportedChild);
    };
    let Some(item_gen) = argument_arrow(item_gen_arg) else {
        return Err(BuildLoweringError::UnsupportedChild);
    };

    let data = raw_object_argument_text_from_argument(data_arg, source_text, &context)
        .unwrap_or_else(|| raw_argument_text(data_arg, source_text));
    let item_params = source_text[item_gen.params.span].trim();
    out.push_str(&format!(
        "{indent}const __lazyForEachItemGenFunction = {item_params} => {{\n"
    ));
    lower_lazy_arrow_body(
        item_gen,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &format!("{indent}    "),
        out,
        span_hints,
    )?;
    out.push_str(&format!("{indent}}};\n"));

    if let Some(id_func_arg) = component.arguments.get(2) {
        let id_func = raw_argument_text_with_context(id_func_arg, source_text, &context);
        out.push_str(&format!(
            "{indent}const __lazyForEachItemIdFunc = {id_func};\n"
        ));
        out.push_str(&format!(
            "{indent}LazyForEach.create(\"1\", {}, {data}, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);\n",
            context.view_context()
        ));
    } else {
        out.push_str(&format!(
            "{indent}LazyForEach.create(\"1\", {}, {data}, __lazyForEachItemGenFunction);\n",
            context.view_context()
        ));
    }
    out.push_str(&format!("{indent}LazyForEach.pop();\n"));
    Ok(())
}

fn lower_lazy_arrow_body(
    arrow: &ArrowFunctionExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &HashSet<String>,
    builder_methods: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    if arrow.expression {
        out.push_str(&format!("{indent}return undefined;\n"));
        return Ok(());
    }
    for statement in &arrow.body.statements {
        lower_statement(
            statement,
            source_text,
            source_id,
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata,
            context.clone(),
            indent,
            out,
            span_hints,
        )?;
    }
    Ok(())
}

fn argument_arrow<'a>(argument: &'a Argument<'a>) -> Option<&'a ArrowFunctionExpression<'a>> {
    match argument.as_expression()? {
        Expression::ArrowFunctionExpression(arrow) => Some(arrow),
        _ => None,
    }
}

fn binding_parameter_name<'a>(parameter: &'a FormalParameter<'a>) -> Option<&'a str> {
    match &parameter.pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn matches_component_call(call: &CallExpression<'_>, expected: &str) -> bool {
    matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == expected)
}

fn push_span_hint(span_hints: &mut Vec<SpanHint>, body_line: usize, body_col: usize, span: Span) {
    span_hints.push(SpanHint {
        body_line,
        body_col: body_col as u32,
        original_span: span,
    });
}

fn source_position(source_text: &str, offset: u32) -> (usize, usize) {
    let end = (offset as usize).min(source_text.len());
    let prefix = &source_text[..end];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = prefix
        .rfind('\n')
        .map_or(prefix.len(), |idx| prefix.len().saturating_sub(idx + 1))
        + 1;
    (line, col)
}

fn component_page_path(source_id: &str) -> &str {
    source_id
        .find("products/")
        .map(|idx| &source_id[idx..])
        .unwrap_or(source_id)
}

fn end_span(span: Span) -> Span {
    Span::new(span.end, span.end)
}

fn chain_property_span(call: &CallExpression<'_>) -> Option<Span> {
    match &call.callee {
        Expression::StaticMemberExpression(member) => Some(member.property.span),
        _ => None,
    }
}

fn push_argument_hints(
    span_hints: &mut Vec<SpanHint>,
    body_line: usize,
    mut body_col: usize,
    arguments: &[Argument<'_>],
    source_text: &str,
) {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            body_col += 2;
        }
        let text = original_node_text(argument, source_text);
        if !push_static_member_argument_hints(
            span_hints,
            body_line,
            body_col,
            argument,
            source_text,
        ) {
            let span = argument.span();
            push_span_hint(span_hints, body_line, body_col, span);
            push_span_hint(span_hints, body_line, body_col + text.len(), end_span(span));
        }
        body_col += text.len();
    }
}

fn push_static_member_argument_hints(
    span_hints: &mut Vec<SpanHint>,
    body_line: usize,
    body_col: usize,
    argument: &Argument<'_>,
    source_text: &str,
) -> bool {
    let Argument::StaticMemberExpression(member) = argument else {
        return false;
    };

    let object_text = original_node_text(&member.object, source_text);
    let property_text = member.property.name.as_str();
    let property_col = body_col + object_text.len() + 1;

    push_span_hint(span_hints, body_line, body_col, member.object.span());
    push_span_hint(
        span_hints,
        body_line,
        body_col + object_text.len(),
        end_span(member.object.span()),
    );
    push_span_hint(span_hints, body_line, property_col, member.property.span);
    push_span_hint(
        span_hints,
        body_line,
        property_col + property_text.len(),
        end_span(member.property.span),
    );
    true
}

fn component_name(callee: &Expression<'_>) -> Result<String, BuildLoweringError> {
    match callee {
        Expression::Identifier(identifier) => Ok(identifier.name.to_string()),
        _ => Err(BuildLoweringError::UnsupportedCallee(
            "non-identifier component callee".to_string(),
        )),
    }
}

fn chain_name(call: &CallExpression<'_>) -> Result<String, BuildLoweringError> {
    match &call.callee {
        Expression::StaticMemberExpression(member) => Ok(member.property.name.to_string()),
        _ => Err(BuildLoweringError::UnsupportedChain),
    }
}

fn raw_arguments_text(arguments: &[Argument<'_>], source_text: &str) -> String {
    arguments
        .iter()
        .map(|argument| raw_argument_text(argument, source_text))
        .collect::<Vec<_>>()
        .join(", ")
}

fn raw_arguments_text_with_context(
    arguments: &[Argument<'_>],
    source_text: &str,
    context: &BuildContext,
) -> String {
    arguments
        .iter()
        .map(|argument| raw_argument_text_with_context(argument, source_text, context))
        .collect::<Vec<_>>()
        .join(", ")
}

fn attribute_arguments_text(
    attr_name: &str,
    arguments: &[Argument<'_>],
    source_text: &str,
    builder_methods: &HashSet<String>,
    context: &BuildContext,
) -> String {
    if is_builder_attribute(attr_name) && arguments.len() == 1 {
        if let Some(name) = argument_builder_method_name(&arguments[0], builder_methods) {
            return format!("{{ builder: this.{name}.bind(this) }}");
        }
    }
    raw_arguments_text_with_context(arguments, source_text, context)
}

fn is_builder_attribute(name: &str) -> bool {
    matches!(
        name,
        "background"
            | "bindPopup"
            | "bindMenu"
            | "bindContextMenu"
            | "bindContextMenuByResponseType"
            | "bindContextMenuByIsShow"
            | "bindContextMenuWithResponse"
            | "title"
            | "menus"
            | "toolBar"
            | "tabBar"
            | "toolbar"
            | "onDragStart"
            | "onItemDragStart"
            | "swipeAction"
            | "bindContentCover"
            | "bindSheet"
            | "navDestination"
            | "overlay"
            | "toolbarConfiguration"
            | "customKeyboard"
            | "bindSelectionMenu"
            | "description"
            | "showUnit"
            | "create"
            | "addBuilderSpan"
            | "dragPreview"
            | "accessibilityVirtualNode"
            | "header"
            | "footer"
    )
}

fn argument_builder_method_name<'a>(
    argument: &'a Argument<'a>,
    builder_methods: &HashSet<String>,
) -> Option<String> {
    let expr = argument.as_expression()?;
    builder_method_member_name(expr, builder_methods)
}

fn builder_method_member_name(
    expression: &Expression<'_>,
    builder_methods: &HashSet<String>,
) -> Option<String> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    if !matches!(&member.object, Expression::ThisExpression(_)) {
        return None;
    }
    let name = member.property.name.as_str();
    builder_methods.contains(name).then(|| name.to_string())
}

fn raw_argument_text(argument: &Argument<'_>, source_text: &str) -> String {
    if let Some(expression) = argument.as_expression() {
        return rewrite_global_context_calls_in_expression_if_needed(expression, source_text)
            .unwrap_or_else(|| original_node_text(expression, source_text));
    }
    original_node_text(argument, source_text)
}

fn raw_argument_text_with_context(
    argument: &Argument<'_>,
    source_text: &str,
    context: &BuildContext,
) -> String {
    raw_object_argument_text_from_argument(argument, source_text, context)
        .or_else(|| {
            argument.as_expression().and_then(|expression| {
                expression_text_with_raw_object_call_args(expression, source_text, context)
            })
        })
        .unwrap_or_else(|| raw_argument_text(argument, source_text))
}

fn lower_builder_method_call(
    call: &CallExpression<'_>,
    source_text: &str,
    builder_methods: &HashSet<String>,
    context: &BuildContext,
    indent: &str,
    out: &mut String,
) -> bool {
    if let Some(name) = builder_method_member_name(&call.callee, builder_methods) {
        let args = raw_arguments_text_with_context(&call.arguments, source_text, context);
        out.push_str(&format!("{indent}this.{name}.bind(this)({args});\n"));
        return true;
    }
    if let Expression::Identifier(identifier) = &call.callee {
        let name = identifier.name.as_str();
        if builder_methods.contains(name) {
            let args = raw_arguments_text_with_context(&call.arguments, source_text, context);
            out.push_str(&format!("{indent}{name}.bind(this)({args});\n"));
            return true;
        }
    };
    false
}

fn span_text(node: &impl GetSpan, source_text: &str) -> String {
    let span = node.span();
    source_text[span].to_string()
}

/// Lower a `@Styles` method body into valid TypeScript.
///
/// ArkUI `@Styles` bodies consist of attribute chains starting with `.`:
/// ```text
/// { .width(100).height(100) }
/// ```
/// These are transformed to `this.attr(x)` calls so the generated TypeScript
/// is valid. Lines that do not start with `.` are preserved as-is.
///
/// Returns the indented statements for insertion inside a `private methodName()` body.
pub fn lower_styles_body(body: Option<&FunctionBody<'_>>, source_text: &str) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let body_text = span_text(body, source_text);
    // Strip the surrounding braces and process each line.
    let inner = strip_outer_block(&body_text);
    let mut out = String::new();
    let mut chain = String::new();
    let mut paren_depth = 0i32;
    for line in inner.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !chain.is_empty() {
            chain.push('\n');
            chain.push_str("        ");
            chain.push_str(trimmed);
            paren_depth += paren_delta(trimmed);
            if paren_depth <= 0 {
                push_styles_chain(&mut out, &chain);
                chain.clear();
                paren_depth = 0;
            }
            continue;
        }
        if trimmed.starts_with('.') {
            chain.push_str("this");
            chain.push_str(trimmed);
            paren_depth = paren_delta(trimmed);
            if paren_depth <= 0 {
                push_styles_chain(&mut out, &chain);
                chain.clear();
                paren_depth = 0;
            }
        } else {
            // Regular statement — emit as-is with indentation.
            out.push_str("        ");
            out.push_str(trimmed);
            if !trimmed.ends_with(';') && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                out.push(';');
            }
            out.push('\n');
        }
    }
    if !chain.is_empty() {
        push_styles_chain(&mut out, &chain);
    }
    out
}

fn push_styles_chain(out: &mut String, chain: &str) {
    out.push_str("        ");
    out.push_str(chain);
    if !chain.trim_end().ends_with(';') {
        out.push(';');
    }
    out.push('\n');
}

fn paren_delta(text: &str) -> i32 {
    text.chars().fold(0, |acc, ch| match ch {
        '(' => acc + 1,
        ')' => acc - 1,
        _ => acc,
    })
}

fn strip_outer_block(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Declaration, StructElement};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::path::Path;

    fn test_component_metadata() -> ComponentMetadata {
        ComponentMetadata::from_ets_loader_path(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ets-loader"
        )))
        .expect("test ets-loader metadata")
    }

    fn parse_and_lower(source: &str) -> Result<String, BuildLoweringError> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let struct_statement = parsed
            .program
            .body
            .iter()
            .find_map(|s| match s {
                Statement::StructStatement(st) => Some(st.as_ref()),
                Statement::ExportNamedDeclaration(e) => match e.declaration.as_ref() {
                    Some(Declaration::StructStatement(st)) => Some(st.as_ref()),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected struct");
        let build = struct_statement
            .body
            .body
            .iter()
            .find_map(|el| match el {
                StructElement::MethodDefinition(m)
                    if m.key.static_name().is_some_and(|n| n == "build") =>
                {
                    Some(m)
                }
                _ => None,
            })
            .expect("expected build method");
        lower_build_body_with_custom_components(
            build.value.body.as_deref(),
            source,
            "",
            &HashSet::new(),
            &HashSet::new(),
            &CustomComponentUpdateProps::new(),
            &test_component_metadata(),
        )
        .map(|(code, _)| code)
    }

    #[test]
    fn lowers_none_body_returns_empty_string() {
        let (out, _) = lower_build_body(None, "").unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn lowers_empty_build_body_returns_empty_string() {
        let source = "@Component\nstruct Foo {\n  build() {}\n}";
        let out = parse_and_lower(source).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn lowers_single_leaf_component_with_string_arg() {
        let source = r#"@Component
struct Foo {
  build() {
    Text("Hello")
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Text.create(\"Hello\");"), "got:\n{out}");
        assert!(out.contains("}, Text);"), "got:\n{out}");
        assert!(out.contains("Text.pop();"), "got:\n{out}");
        assert!(
            !out.contains(".pop();\n        this.observeComponentCreation2"),
            "got:\n{out}"
        );
    }

    #[test]
    fn lowers_leaf_component_no_args_no_attrs() {
        let source = "@Component\nstruct Foo {\n  build() {\n    Divider()\n  }\n}";
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Divider.create();"), "got:\n{out}");
        assert!(out.contains("}, Divider);"), "got:\n{out}");
        assert!(!out.contains("Divider.pop();"), "got:\n{out}");
        assert!(
            !out.contains("Divider.create(,"),
            "no empty-arg comma; got:\n{out}"
        );
    }

    #[test]
    fn lowers_container_with_two_text_siblings() {
        let source = r#"@Component
struct Foo {
  build() {
    Column() {
      Text("first")
      Text("second")
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        let col_create = out.find("Column.create();").expect("Column.create missing");
        let first = out
            .find("Text.create(\"first\");")
            .expect("first Text.create missing");
        let second = out
            .find("Text.create(\"second\");")
            .expect("second Text.create missing");
        let col_pop = out.find("Column.pop();").expect("Column.pop missing");
        assert!(
            col_create < first,
            "Column.create must precede first Text.create"
        );
        assert!(first < second, "first Text.create must precede second");
        assert!(
            second < col_pop,
            "second Text.create must precede Column.pop"
        );
        assert_eq!(
            out.matches("Text.pop();").count(),
            2,
            "two Text.pop() calls expected"
        );
    }

    #[test]
    fn lowers_multiple_root_siblings_in_build() {
        let source = r#"@Component
struct Foo {
  build() {
    Text("a")
    Text("b")
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert_eq!(
            out.matches("this.observeComponentCreation2").count(),
            2,
            "expected two observeComponentCreation2 calls; got:\n{out}"
        );
        assert_eq!(
            out.matches("Text.pop();").count(),
            2,
            "two Text.pop() calls; got:\n{out}"
        );
    }

    #[test]
    fn lowers_component_with_member_expression_arg() {
        let source = "@Component\nstruct Foo {\n  @State msg: string = \"hi\";\n  build() {\n    Text(this.msg)\n  }\n}";
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Text.create(this.msg);"), "got:\n{out}");
    }

    #[test]
    fn lowers_component_with_multiple_chain_attrs() {
        let source = r#"@Component
struct Foo {
  build() {
    Button("OK")
      .width(100)
      .height(40)
      .borderRadius(8)
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(
            out.contains("Button.createWithLabel(\"OK\");"),
            "got:\n{out}"
        );
        assert!(out.contains("Button.width(100);"), "got:\n{out}");
        assert!(out.contains("Button.height(40);"), "got:\n{out}");
        assert!(out.contains("Button.borderRadius(8);"), "got:\n{out}");
        assert!(out.contains("Button.pop();"), "got:\n{out}");
        let create_pos = out.find("Button.create").unwrap();
        let pop_pos = out.find("Button.pop").unwrap();
        assert!(create_pos < pop_pos, "create must precede pop");
    }

    #[test]
    fn lowers_button_with_children_using_create_with_child() {
        let source = r#"@Component
struct Foo {
  build() {
    Button("OK") {
      Text("child")
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(
            out.contains("Button.createWithChild(\"OK\");"),
            "got:\n{out}"
        );
        assert!(out.contains("Text.create(\"child\");"), "got:\n{out}");
    }

    #[test]
    fn lowers_deeply_nested_four_levels() {
        let source = r#"@Component
struct Foo {
  build() {
    Flex() {
      Stack() {
        Column() {
          Row() {
          }
        }
      }
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        for name in ["Flex", "Stack", "Column", "Row"] {
            assert!(
                out.contains(&format!("{name}.create();")),
                "{name}.create missing; got:\n{out}"
            );
            assert!(
                out.contains(&format!("{name}.pop();")),
                "{name}.pop missing; got:\n{out}"
            );
        }
        let flex_create = out.find("Flex.create();").unwrap();
        let row_create = out.find("Row.create();").unwrap();
        let row_pop = out.find("Row.pop();").unwrap();
        let flex_pop = out.find("Flex.pop();").unwrap();
        assert!(
            flex_create < row_create,
            "Flex.create must come before Row.create"
        );
        assert!(row_create < row_pop, "Row.create before Row.pop");
        assert!(row_pop < flex_pop, "Row.pop before Flex.pop");
    }

    #[test]
    fn lowers_container_no_attrs_one_child() {
        let source = r#"@Component
struct Foo {
  build() {
    Row() {
      Spacer()
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        let row_create = out.find("Row.create();").expect("Row.create");
        let spacer_create = out.find("Spacer.create();").expect("Spacer.create");
        let spacer_pop = out.find("Spacer.pop();").expect("Spacer.pop");
        let row_pop = out.find("Row.pop();").expect("Row.pop");
        assert!(row_create < spacer_create);
        assert!(spacer_create < spacer_pop);
        assert!(spacer_pop < row_pop);
        assert!(
            !out.contains("Row.pop();\n        Row.pop()"),
            "no duplicate Row.pop"
        );
    }

    #[test]
    fn lowers_component_with_enum_member_arg() {
        let source = r#"@Component
struct Foo {
  build() {
    Image($r("app.media.logo"))
      .objectFit(ImageFit.Cover)
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(
            out.contains("Image.create($r(\"app.media.logo\"));"),
            "got:\n{out}"
        );
        assert!(
            out.contains("Image.objectFit(ImageFit.Cover);"),
            "got:\n{out}"
        );
        assert!(!out.contains("Image.pop();"), "got:\n{out}");
    }

    #[test]
    fn preserves_regular_function_calls_inside_attribute_arrow() {
        let source = r#"@Component
struct Foo {
  build() {
    Row() {
    }.onClick(() => {
      postCardAction(this, { action: "router" });
    })
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(
            out.contains(
                "Row.onClick(() => {\n      postCardAction(this, { action: \"router\" });\n    });"
            ),
            "attribute arrow body should be preserved as source text; got:\n{out}"
        );
        assert!(
            !out.contains("postCardAction.create"),
            "ordinary function calls in attribute callbacks are not ArkUI components; got:\n{out}"
        );
    }

    #[test]
    fn lowers_if_statement_children() {
        let source = r#"@Component
struct Foo {
  @State show: boolean = true;
  build() {
    Column() {
      if (this.show) {
        Text("shown")
      } else {
        Text("hidden")
      }
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        let if_pos = out.find("if (this.show) {").expect("if missing");
        let shown = out
            .find("Text.create(\"shown\");")
            .expect("shown Text.create missing");
        let else_pos = out.find("} else {").expect("else missing");
        let hidden = out
            .find("Text.create(\"hidden\");")
            .expect("hidden Text.create missing");
        let col_pop = out.find("Column.pop();").expect("Column.pop missing");
        assert!(if_pos < shown, "then branch lowered inside if; got:\n{out}");
        assert!(shown < else_pos, "then branch before else; got:\n{out}");
        assert!(else_pos < hidden, "else branch lowered; got:\n{out}");
        assert!(hidden < col_pop, "children before parent pop; got:\n{out}");
        assert_eq!(
            out.matches("Text.pop();").count(),
            2,
            "both branch Text components are popped; got:\n{out}"
        );
    }

    #[test]
    fn lowers_sibling_containers_with_children() {
        let source = r#"@Component
struct Foo {
  build() {
    Column() {
      Row() {
        Text("left")
      }
      Row() {
        Text("right")
      }
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert_eq!(
            out.matches("Row.create();").count(),
            2,
            "two Row.create() calls; got:\n{out}"
        );
        assert_eq!(
            out.matches("Row.pop();").count(),
            2,
            "two Row.pop() calls; got:\n{out}"
        );
        assert_eq!(
            out.matches("Text.create(").count(),
            2,
            "two Text.create() calls; got:\n{out}"
        );
        let col_create = out.find("Column.create();").expect("Column.create");
        let col_pop = out.find("Column.pop();").expect("Column.pop");
        assert!(col_create < col_pop);
    }

    #[test]
    fn lowers_minimal_fixture_build_body_from_oxc_arkui_ast() {
        let source = r#"
@Entry
@Component
struct Index {
  @State message: string = "Hello World";

  build() {
    Row() {
      Column() {
        Text(this.message)
          .fontSize(50)
          .fontWeight(FontWeight.Bold)
      }.width("100%")
    }.height("100%");
  }
}
"#;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );

        let struct_statement = parsed
            .program
            .body
            .iter()
            .find_map(|statement| match statement {
                Statement::StructStatement(statement) => Some(statement.as_ref()),
                Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                    Some(Declaration::StructStatement(statement)) => Some(statement.as_ref()),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected ArkUI struct");

        let build = struct_statement
            .body
            .body
            .iter()
            .find_map(|element| match element {
                StructElement::MethodDefinition(method)
                    if method.key.static_name().is_some_and(|name| name == "build") =>
                {
                    Some(method)
                }
                _ => None,
            })
            .expect("expected build method");

        let (out, span_hints) = lower_build_body_with_custom_components(
            build.value.body.as_deref(),
            source,
            "",
            &HashSet::new(),
            &HashSet::new(),
            &CustomComponentUpdateProps::new(),
            &test_component_metadata(),
        )
        .unwrap();
        assert!(
            span_hints.len() >= 18,
            "expected token-level span hints for Row, Column, and Text"
        );
        let expected = concat!(
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Row.create();\n",
            "            Row.height(\"100%\");\n",
            "        }, Row);\n",
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Column.create();\n",
            "            Column.width(\"100%\");\n",
            "        }, Column);\n",
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Text.create(this.message);\n",
            "            Text.fontSize(50);\n",
            "            Text.fontWeight(FontWeight.Bold);\n",
            "        }, Text);\n",
            "        Text.pop();\n",
            "        Column.pop();\n",
            "        Row.pop();\n",
        );
        assert_eq!(out, expected);
    }
}
