use rustc_hash::{FxHashMap, FxHashSet};
use std::{fmt::Write, sync::Arc};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, BlockStatement, CallExpression, Expression,
    FunctionBody, IfStatement, ObjectPropertyKind, Statement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::transform::component_meta::ComponentMetadata;
use crate::transform::global_rewrite::rewrite_global_context_calls_in_expression_if_needed;
use crate::transform::source_emit::original_node_text;
use crate::transform::viewpu_emit::ViewpuWriter;

mod custom_component;
use custom_component::{
    custom_reuse_id_from_component_attrs, custom_reuse_id_from_leaf_attrs,
    emit_common_wrapped_custom_component_call, emit_custom_component_call,
};
mod raw_object;
use raw_object::{
    expression_text_with_raw_object_call_args, raw_object_argument_text_from_argument,
};
mod iteration;
use iteration::{is_lazy_list_item, lower_for_each, lower_lazy_for_each, lower_lazy_list_item};
mod styles;
pub use styles::lower_styles_body;

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
    pub update_props: FxHashSet<String>,
    pub link_props: FxHashSet<String>,
    pub reusable: bool,
}

pub type CustomComponentUpdateProps = FxHashMap<String, CustomComponentInfo>;

#[derive(Clone, Debug, Default)]
struct BuildContext {
    use_parent_context: bool,
    raw_object_fields: Arc<FxHashSet<String>>,
    module_name: Arc<str>,
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

    fn with_raw_object_fields(mut self, raw_object_fields: &FxHashSet<String>) -> Self {
        self.raw_object_fields = Arc::new(raw_object_fields.clone());
        self
    }

    fn with_module_name(mut self, module_name: &str) -> Self {
        self.module_name = Arc::from(module_name);
        self
    }

    fn is_raw_object_field(&self, name: &str) -> bool {
        self.raw_object_fields.contains(name)
    }
}

/// Gesture type names that receive `.create()`/`.pop()` pairs in the ViewPU
/// output.  Matches `GESTURE_TYPE_NAMES` from upstream `component_map.js`.
const GESTURE_TYPE_NAMES: &[&str] = &[
    "TapGesture",
    "LongPressGesture",
    "PanGesture",
    "PinchGesture",
    "RotationGesture",
    "SwipeGesture",
    "GestureGroup",
];

fn is_gesture_type(name: &str) -> bool {
    GESTURE_TYPE_NAMES.contains(&name)
}

/// Extracted representation of a gesture call and its attribute chain.
/// Used to emit `Gesture.create(priority)` / `XxxGesture.create(args)` /
/// `XxxGesture.onX(...)` / `XxxGesture.pop()` / `Gesture.pop()` sequences.
struct GestureChain {
    gesture_type: String,
    create_args: String,
    attrs: Vec<(String, String)>,
}

/// Extract a `GestureChain` from a chained `CallExpression` like
/// `PinchGesture().onActionStart(...).onActionEnd(...)`.
///
/// Returns `None` if the base identifier is not a known gesture type.
fn extract_gesture_chain<'a>(
    call: &'a CallExpression<'a>,
    source_text: &str,
    context: &BuildContext,
) -> Option<GestureChain> {
    extract_gesture_chain_inner(call, source_text, context)
}

fn extract_gesture_chain_inner<'a>(
    call: &'a CallExpression<'a>,
    source_text: &str,
    context: &BuildContext,
) -> Option<GestureChain> {
    match &call.callee {
        Expression::Identifier(_) => {
            let name = match component_name(&call.callee) {
                Ok(n) => n,
                Err(_) => return None,
            };
            if !is_gesture_type(&name) {
                return None;
            }
            let create_args = raw_arguments_text_with_context(&call.arguments, source_text, context);
            Some(GestureChain {
                gesture_type: name,
                create_args,
                attrs: vec![],
            })
        }
        Expression::StaticMemberExpression(member) => {
            let attr_name = member.property.name.to_string();
            let attr_args = raw_arguments_text_with_context(&call.arguments, source_text, context);
            match &member.object {
                Expression::CallExpression(inner) => {
                    let mut gc = extract_gesture_chain_inner(inner, source_text, context)?;
                    gc.attrs.push((attr_name, attr_args));
                    Some(gc)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Emit a full gesture sequence into `out`:
/// ```text
/// Gesture.create(GesturePriority.Low);
/// XxxGesture.create(args);
/// XxxGesture.onX(handler);
/// ...
/// XxxGesture.pop();
/// Gesture.pop();
/// ```
fn emit_gesture_sequence(out: &mut String, indent: &str, gc: &GestureChain) {
    let attr_indent = format!("{indent}    ");
    let mut writer = ViewpuWriter::new(out);
    writer.component_create(&attr_indent, "Gesture", "create", "GesturePriority.Low");
    writer.component_create(&attr_indent, &gc.gesture_type, "create", &gc.create_args);
    for (attr_name, attr_args) in &gc.attrs {
        writer.component_attribute(&attr_indent, &gc.gesture_type, attr_name, attr_args);
    }
    writer.component_pop(&attr_indent, &gc.gesture_type);
    writer.component_pop(&attr_indent, "Gesture");
}

/// Attempt to emit a gesture sequence from a raw argument text string.
///
/// Parses `args_text` as a TypeScript expression, extracts the gesture chain,
/// and emits the sequence.  Returns `true` if the text was a known gesture
/// type; `false` if it should be emitted as a regular attribute instead.
fn try_emit_gesture_from_text(
    out: &mut String,
    indent: &str,
    args_text: &str,
    context: &BuildContext,
) -> bool {
    let allocator = Allocator::default();
    let source = format!("__g__({args_text});");
    let source_type = SourceType::ts().with_module(true);
    let parsed = Parser::new(&allocator, &source, source_type).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let Some(Statement::ExpressionStatement(stmt)) = parsed.program.body.first() else {
        return false;
    };
    let Expression::CallExpression(wrapper) = &stmt.expression else {
        return false;
    };
    let Some(arg) = wrapper.arguments.first() else {
        return false;
    };
    let Some(arg_expr) = arg.as_expression() else {
        return false;
    };
    let Expression::CallExpression(gesture_call) = arg_expr else {
        return false;
    };
    let Some(gc) = extract_gesture_chain(gesture_call, &source, context) else {
        return false;
    };
    emit_gesture_sequence(out, indent, &gc);
    true
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
    let context = BuildContext::default();
    let attr_indent = format!("{indent}    ");
    for (attr_name, attr_args, _) in attrs {
        if attr_name == "gesture" {
            if try_emit_gesture_from_text(out, indent, attr_args, &context) {
                continue;
            }
        }
        let mut writer = ViewpuWriter::new(out);
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    if custom_components.contains(&lc.name) {
        let base_call = base_component_call(call);
        let is_reusable_reuseId_only = custom_component_update_props
            .get(&lc.name)
            .is_some_and(|info| {
                info.reusable && lc.chain_attrs.iter().all(|(name, _, _)| name == "reuseId")
            });
        if lc.chain_attrs.is_empty() || is_reusable_reuseId_only {
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
        } else {
            // Custom component with chained attribute calls — wrap in __Common__.
            emit_common_wrapped_custom_component_call(
                &lc.name,
                &base_call.arguments,
                custom_component_update_props.get(&lc.name),
                base_call.span,
                &lc.chain_attrs,
                source_text,
                source_id,
                context.clone(),
                indent,
                out,
                span_hints,
            );
        }
        return Ok(());
    }
    let mut creation_body = String::new();
    {
        let mut writer = ViewpuWriter::new(&mut creation_body);
        writer.component_create(
            "",
            &lc.name,
            create_method_name(&lc.name, false),
            &lc.create_args,
        );
    }
    push_attribute_calls(&mut creation_body, "", &lc.name, &lc.chain_attrs);
    let mut writer = ViewpuWriter::new(out);
    writer.observe_component_creation(indent, context.use_parent_context, &lc.name, &creation_body);
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
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &ComponentMetadata::default(),
    )
}

pub fn lower_build_body_with_custom_components(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_build_body_with_component_context_and_module(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        raw_object_fields,
        None,
    )
}

pub fn lower_build_body_with_component_context_and_module(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
    module_name: Option<&str>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    let mut ctx = BuildContext::default().with_raw_object_fields(raw_object_fields);
    if let Some(m) = module_name {
        ctx = ctx.with_module_name(m);
    }
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        ctx,
    )
}

pub fn lower_builder_body_with_custom_components(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    lower_builder_body_with_component_context_and_module(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        raw_object_fields,
        None,
    )
}

pub fn lower_builder_body_with_component_context_and_module(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
    module_name: Option<&str>,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    let mut ctx = BuildContext::builder().with_raw_object_fields(raw_object_fields);
    if let Some(m) = module_name {
        ctx = ctx.with_module_name(m);
    }
    lower_build_body_with_context(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        ctx,
    )
}

fn lower_build_body_with_context(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let branch_indent = format!("{indent}    ");
    let inner_indent = format!("{branch_indent}    ");

    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation_start(indent, context.view_context());
        writer.line(&branch_indent, format_args!("If.create();"));
    }

    // Emit the full if/else-if/else chain without new If.create()/If.pop() wrappers.
    let branch_count = lower_if_chain(
        statement,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &branch_indent,
        &inner_indent,
        0,
        out,
        span_hints,
    )?;

    // If there was no else clause at all, synthesize an empty else branch so
    // the ArkUI runtime always has a complete branch set.
    if branch_count == 1 {
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

    {
        let mut writer = ViewpuWriter::new(out);
        writer.line(&branch_indent, format_args!("}}"));
        writer.observe_component_creation_end(indent, "If");
        writer.component_pop(indent, "If");
    }

    Ok(())
}

/// Emit the if/else-if/else chain for a single `IfStatement` node, reusing
/// the same `If.create()`/`If.pop()` wrapper from the caller.
///
/// Returns the total number of branches emitted (i.e. `branch_start + 1 + N`
/// where N is the number of `else if` / `else` branches handled).
#[allow(clippy::too_many_arguments)]
fn lower_if_chain(
    statement: &IfStatement<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    branch_indent: &str,
    inner_indent: &str,
    branch_index: usize,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<usize, BuildLoweringError> {
    let test = original_node_text(&statement.test, source_text);

    // Emit `if (test) {` or `} else if (test) {` depending on branch_index.
    {
        let mut writer = ViewpuWriter::new(out);
        if branch_index == 0 {
            writer.line(branch_indent, format_args!("if ({test}) {{"));
        } else {
            writer.line(branch_indent, format_args!("}} else if ({test}) {{"));
        }
        writer.line(
            inner_indent,
            format_args!(
                "{}.ifElseBranchUpdateFunction({branch_index}, () => {{",
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
        writer.line(inner_indent, format_args!("}});"));
    }

    let next_index = branch_index + 1;

    match &statement.alternate {
        None => {
            // No else clause — caller will synthesize the empty branch if needed.
            Ok(next_index)
        }
        Some(Statement::IfStatement(alternate_if)) => {
            // else if — continue the chain without a new If.create()/If.pop() wrapper.
            lower_if_chain(
                alternate_if,
                source_text,
                source_id,
                custom_components,
                builder_methods,
                custom_component_update_props,
                component_metadata,
                context,
                branch_indent,
                inner_indent,
                next_index,
                out,
                span_hints,
            )
        }
        Some(alternate) => {
            // else clause.
            {
                let mut writer = ViewpuWriter::new(out);
                writer.line(branch_indent, format_args!("}} else {{"));
                writer.line(
                    inner_indent,
                    format_args!(
                        "{}.ifElseBranchUpdateFunction({next_index}, () => {{",
                        context.view_context()
                    ),
                );
            }
            lower_statement(
                alternate,
                source_text,
                source_id,
                custom_components,
                builder_methods,
                custom_component_update_props,
                component_metadata,
                context,
                &format!("{inner_indent}    "),
                out,
                span_hints,
            )?;
            {
                let mut writer = ViewpuWriter::new(out);
                writer.line(inner_indent, format_args!("}});"));
            }
            Ok(next_index + 1)
        }
    }
}

fn lower_component(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
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
        emit_builder_method_call(out, indent, None, &name, &args);
        return Ok(());
    }

    if custom_components.contains(&name) {
        let has_chain_attrs = !component.chain_expressions.is_empty();
        if has_chain_attrs {
            // Collect chain attrs from the ArkUIComponentExpression.
            let mut chain_attrs = Vec::new();
            for chain in &component.chain_expressions {
                let chain_attr_name = chain_name(chain)?;
                let chain_attr_args = attribute_arguments_text(
                    &chain_attr_name,
                    &chain.arguments,
                    source_text,
                    builder_methods,
                    &context,
                );
                let regular = arguments_are_regular_attrs(&chain.arguments, component_metadata);
                chain_attrs.push((chain_attr_name, chain_attr_args, regular));
            }
            emit_common_wrapped_custom_component_call(
                &name,
                &component.arguments,
                custom_component_update_props.get(&name),
                component.callee.span(),
                &chain_attrs,
                source_text,
                source_id,
                context,
                indent,
                out,
                span_hints,
            );
        } else {
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
        }
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

    if is_nav_router_component(&name) {
        return lower_nav_component(
            component,
            &name,
            &args,
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
    let mut creation_body = String::new();
    {
        let mut writer = ViewpuWriter::new(&mut creation_body);
        writer.component_create(
            "",
            &name,
            create_method_name(&name, !component.children.is_empty()),
            &args,
        );
    }
    // Gesture attrs are emitted inline as Gesture.create/pop sequences rather
    // than as regular attribute calls.  Each chain expression is either a
    // regular attribute or a gesture attribute; they are emitted in source order.
    enum AttrOrGesture {
        Regular(String, String),
        Gesture(GestureChain),
    }
    let mut ordered_attrs: Vec<AttrOrGesture> = Vec::new();
    for (index, chain) in component.chain_expressions.iter().enumerate() {
        let chain_name = chain_name(chain)?;
        // Gesture attributes: extract chain from the single argument AST node.
        if chain_name == "gesture" {
            if let Some(arg) = chain.arguments.first() {
                if let Some(Expression::CallExpression(gesture_call)) = arg.as_expression() {
                    if let Some(gc) = extract_gesture_chain(gesture_call, source_text, &context) {
                        ordered_attrs.push(AttrOrGesture::Gesture(gc));
                        continue;
                    }
                }
            }
        }
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
        ordered_attrs.push(AttrOrGesture::Regular(chain_name, chain_args));
    }
    // Emit attrs and gesture sequences in source order into creation_body.
    {
        let attr_indent = "    ";
        for entry in &ordered_attrs {
            match entry {
                AttrOrGesture::Regular(attr_name, attr_args) => {
                    let mut writer = ViewpuWriter::new(&mut creation_body);
                    writer.component_attribute(attr_indent, &name, attr_name, attr_args);
                }
                AttrOrGesture::Gesture(gc) => {
                    emit_gesture_sequence(&mut creation_body, "", gc);
                }
            }
        }
    }
    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation(
            indent,
            context.use_parent_context,
            &name,
            &creation_body,
        );
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

/// Returns true for components that require the `{ moduleName, pagePath }` metadata
/// second argument: `NavDestination` and `Navigation`.
fn is_nav_router_component(name: &str) -> bool {
    matches!(name, "NavDestination" | "Navigation")
}

/// Compute the `pagePath` value for NavDestination/Navigation metadata.
///
/// Uses the hvigor source id (relative path like
/// `features/challenge/ChallengeDetailView.ets`) and strips the `.ets`
/// extension, matching the upstream `ets-loader` behaviour.
fn nav_page_path(source_id: &str) -> String {
    source_id.trim_end_matches(".ets").to_string()
}

/// Lower a `NavDestination` or `Navigation` component with the full
/// builder-closure + metadata form that hvigor emits:
///
/// ```js
/// NavDestination.create(() => { ...children... }, { moduleName: "...", pagePath: "..." });
/// ```
///
/// For `Navigation`, an additional `isUserCreateStack` boolean property is
/// appended to the metadata object.
#[allow(clippy::too_many_arguments)]
fn lower_nav_component(
    component: &ArkUIComponentExpression<'_>,
    name: &str,
    create_args: &str,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let module_name = context.module_name.as_ref();
    let page_path = nav_page_path(source_id);
    let has_children = !component.children.is_empty();

    // Build the metadata object literal.
    let meta = if name == "Navigation" {
        let is_user_create_stack = has_children;
        format!(
            "{{ moduleName: \"{module_name}\", pagePath: \"{page_path}\", isUserCreateStack: {is_user_create_stack} }}"
        )
    } else {
        format!("{{ moduleName: \"{module_name}\", pagePath: \"{page_path}\" }}")
    };

    // Build the create call arguments.
    // NavDestination.create(() => { ...body... }, meta)
    // The first positional arg from the source (e.g. a NavPathStack for Navigation)
    // is passed through unchanged, then the builder closure, then the meta.
    //
    // Upstream ets-loader passes the builder closure as the *first* argument for
    // NavDestination (which takes no user-visible constructor arg), and for
    // Navigation the existing first arg is kept and the meta is appended.
    let full_create_args = if create_args.is_empty() {
        // NavDestination has no user-visible first arg; closure is first arg.
        format!("() => {{}}, {meta}")
    } else {
        // Navigation passes its first arg (e.g. new NavPathStack()) then meta.
        format!("{create_args}, {meta}")
    };

    // Build the observeComponentCreation2 wrapper with the create call.
    let mut creation_body = String::new();
    {
        let mut writer = ViewpuWriter::new(&mut creation_body);
        writer.component_create("", name, "create", &full_create_args);
    }
    let mut attrs = Vec::new();
    for chain in &component.chain_expressions {
        let chain_name = chain_name(chain)?;
        let chain_args = attribute_arguments_text(
            &chain_name,
            &chain.arguments,
            source_text,
            builder_methods,
            &context,
        );
        attrs.push((
            chain_name,
            chain_args,
            arguments_are_regular_attrs(&chain.arguments, component_metadata),
        ));
    }
    push_attribute_calls(&mut creation_body, "", name, &attrs);
    {
        let mut writer = ViewpuWriter::new(out);
        writer.observe_component_creation(indent, context.use_parent_context, name, &creation_body);
    }

    // Lower children into the builder closure body.
    // For NavDestination with children we need to emit a real closure body.
    // Upstream wraps the children in an arrow function passed as the first arg.
    // Since we already emitted the create call with `() => {}` as placeholder,
    // we need to emit children *outside* the observeComponentCreation2 but
    // *before* the pop, which is the standard pattern for container components.
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

    if component_metadata.needs_pop(name) {
        let mut writer = ViewpuWriter::new(out);
        writer.component_pop(indent, name);
    }
    Ok(())
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
    builder_methods: &FxHashSet<String>,
    context: &BuildContext,
) -> String {
    if is_builder_attribute(attr_name) && arguments.len() == 1 {
        if let Some(name) = argument_builder_method_name(&arguments[0], builder_methods) {
            return BuilderAttributeArgument::new(&name).emit();
        }
    }
    raw_arguments_text_with_context(arguments, source_text, context)
}

struct BuilderAttributeArgument<'a> {
    name: &'a str,
}

impl<'a> BuilderAttributeArgument<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn emit(&self) -> String {
        let mut out = String::new();
        write!(out, "{{ builder: this.{}.bind(this) }}", self.name)
            .expect("writing to String cannot fail");
        out
    }
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
    builder_methods: &FxHashSet<String>,
) -> Option<String> {
    let expr = argument.as_expression()?;
    builder_method_member_name(expr, builder_methods)
}

fn builder_method_member_name(
    expression: &Expression<'_>,
    builder_methods: &FxHashSet<String>,
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
    builder_methods: &FxHashSet<String>,
    context: &BuildContext,
    indent: &str,
    out: &mut String,
) -> bool {
    if let Some(name) = builder_method_member_name(&call.callee, builder_methods) {
        let args = raw_arguments_text_with_context(&call.arguments, source_text, context);
        emit_builder_method_call(out, indent, Some("this"), &name, &args);
        return true;
    }
    if let Expression::Identifier(identifier) = &call.callee {
        let name = identifier.name.as_str();
        if builder_methods.contains(name) {
            let args = raw_arguments_text_with_context(&call.arguments, source_text, context);
            emit_builder_method_call(out, indent, None, name, &args);
            return true;
        }
    };
    false
}

fn emit_builder_method_call(
    out: &mut String,
    indent: &str,
    receiver: Option<&str>,
    name: &str,
    args: &str,
) {
    let mut writer = ViewpuWriter::new(out);
    if let Some(receiver) = receiver {
        writer.line(
            indent,
            format_args!("{receiver}.{name}.bind(this)({args});"),
        );
    } else {
        writer.line(indent, format_args!("{name}.bind(this)({args});"));
    }
}

fn span_text(node: &impl GetSpan, source_text: &str) -> String {
    let span = node.span();
    source_text[span].to_string()
}

#[cfg(test)]
mod tests;
