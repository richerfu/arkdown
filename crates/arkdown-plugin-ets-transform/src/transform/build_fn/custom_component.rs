use std::fmt::Write;

use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, Expression, ObjectExpression, ObjectPropertyKind,
    PropertyKey,
};
use oxc_span::Span;

use crate::transform::source_emit::original_node_text;
use crate::transform::viewpu_emit::ViewpuWriter;

use super::raw_object::expression_text_with_raw_object_call_args;
use super::{
    chain_name, component_page_path, push_span_hint, raw_arguments_text, source_position,
    BuildContext, CustomComponentInfo, SpanHint,
};

/// Emit a custom component wrapped in `__Common__` for chained attribute calls.
///
/// This is emitted when a custom component call has chained attribute calls
/// (`.width(...)`, `.height(...)`, etc.). The pattern is:
///
/// ```ts
/// this.observeComponentCreation2((elmtId, isInitialRender) => {
///     __Common__.create();
///     __Common__.width(...);
///     // ... more attribute calls ...
/// }, __Common__);
/// {
///     // custom component observeComponentCreation2 block
/// }
/// __Common__.pop();
/// ```
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_common_wrapped_custom_component_call(
    name: &str,
    arguments: &[Argument<'_>],
    component_info: Option<&CustomComponentInfo>,
    span: Span,
    chain_attrs: &[(String, String, bool)],
    source_text: &str,
    source_id: &str,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) {
    // Build the __Common__ observeComponentCreation2 body.
    let mut common_body = String::new();
    {
        let mut writer = crate::transform::viewpu_emit::ViewpuWriter::new(&mut common_body);
        writer.component_create("", "__Common__", "create", "");
        for (attr_name, attr_args, _) in chain_attrs {
            writer.component_attribute("", "__Common__", attr_name, attr_args);
        }
    }
    {
        let mut writer = crate::transform::viewpu_emit::ViewpuWriter::new(out);
        writer.observe_component_creation(indent, context.use_parent_context, "__Common__", &common_body);
    }
    // Emit the custom component call inside a block.
    out.push_str(indent);
    out.push_str("{\n");
    emit_custom_component_call(
        name,
        arguments,
        component_info,
        None,
        span,
        source_text,
        source_id,
        context.clone(),
        &format!("{indent}    "),
        out,
        span_hints,
    );
    out.push_str(indent);
    out.push_str("}\n");
    // Emit __Common__.pop().
    let mut writer = crate::transform::viewpu_emit::ViewpuWriter::new(out);
    writer.component_pop(indent, "__Common__");
}

pub(super) fn emit_custom_component_call(
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
    let mut writer = CustomComponentWriter::new(out, indent);
    let i = CustomComponentIndent::new(indent);
    writer.base(format_args!("{{"));
    writer.line(
        &i.inner,
        format_args!(
            "{}.observeComponentCreation2((elmtId, isInitialRender) => {{",
            context.view_context()
        ),
    );
    writer.line(&i.inner2, format_args!("if (isInitialRender) {{"));
    writer.line(
        &i.inner3,
        format_args!(
            "let componentCall = new {name}({}, {}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {line}, col: {col} }});",
            context.view_context(),
            params.constructor
        ),
    );
    writer.line(&i.inner3, format_args!("ViewPU.create(componentCall);"));
    writer.line(&i.inner3, format_args!("let paramsLambda = () => {{"));
    writer.line(&i.inner4, format_args!("return {};", params.lambda));
    writer.line(&i.inner3, format_args!("}};"));
    writer.line(
        &i.inner3,
        format_args!("componentCall.paramsGenerator_ = paramsLambda;"),
    );
    writer.line(&i.inner2, format_args!("}} else {{"));
    writer.line(
        &i.inner3,
        format_args!(
            "{}.updateStateVarsOfChildByElmtId(elmtId, {});",
            context.view_context(),
            params.update
        ),
    );
    writer.line(&i.inner2, format_args!("}}"));
    writer.line(&i.inner, format_args!("}}, {{ name: \"{name}\" }});"));
    writer.base(format_args!("}}"));
}

struct CustomComponentWriter<'a, 'b> {
    writer: ViewpuWriter<'a>,
    indent: &'b str,
}

impl<'a, 'b> CustomComponentWriter<'a, 'b> {
    fn new(out: &'a mut String, indent: &'b str) -> Self {
        Self {
            writer: ViewpuWriter::new(out),
            indent,
        }
    }

    fn line(&mut self, indent: &str, args: std::fmt::Arguments<'_>) {
        self.writer.line(indent, args);
    }

    fn base(&mut self, args: std::fmt::Arguments<'_>) {
        self.writer.line(self.indent, args);
    }
}

struct CustomComponentIndent {
    inner: String,
    inner2: String,
    inner3: String,
    inner4: String,
    inner5: String,
    inner6: String,
}

impl CustomComponentIndent {
    fn new(indent: &str) -> Self {
        let inner = format!("{indent}    ");
        let inner2 = format!("{inner}    ");
        let inner3 = format!("{inner}        ");
        let inner4 = format!("{inner}            ");
        let inner5 = format!("{inner}                ");
        let inner6 = format!("{inner}                    ");
        Self {
            inner,
            inner2,
            inner3,
            inner4,
            inner5,
            inner6,
        }
    }
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
    let reuse_id = reuse_id.unwrap_or_else(|| CustomComponentReuseId::default_for(name));
    let mut writer = CustomComponentWriter::new(out, indent);
    let i = CustomComponentIndent::new(indent);

    writer.base(format_args!(
        "{}.observeComponentCreation2((elmtId, isInitialRender) => {{",
        context.view_context()
    ));
    writer.base(format_args!("    __Recycle__.create();"));
    writer.base(format_args!("}}, __Recycle__);"));
    writer.base(format_args!("{{"));
    writer.line(
        &i.inner,
        format_args!(
            "{}.observeRecycleComponentCreation({reuse_id}, (elmtId, isInitialRender, recycleNode = null) => {{",
            context.view_context()
        ),
    );
    writer.line(
        &i.inner,
        format_args!("    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);"),
    );
    writer.line(&i.inner2, format_args!("if (isInitialRender) {{"));
    writer.line(
        &i.inner3,
        format_args!(
            "let componentCall = recycleNode ? recycleNode : new {name}({}, {}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {line}, col: {col} }});",
            context.view_context(),
            params.constructor
        ),
    );
    writer.line(
        &i.inner3,
        format_args!(
            "ViewPU.createRecycle(componentCall, recycleNode !== null, {reuse_id}, () => {{"
        ),
    );
    writer.line(
        &i.inner4,
        format_args!(
            "if (recycleNode && typeof recycleNode.aboutToReuseInternal === \"function\") {{"
        ),
    );
    writer.line(
        &i.inner5,
        format_args!("recycleNode.aboutToReuseInternal();"),
    );
    writer.line(&i.inner4, format_args!("}} else {{"));
    writer.line(
        &i.inner5,
        format_args!(
            "if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === \"function\") {{"
        ),
    );
    writer.line(
        &i.inner6,
        format_args!("recycleNode.aboutToReuse({});", params.lambda),
    );
    writer.line(&i.inner5, format_args!("}}"));
    writer.line(&i.inner5, format_args!("recycleNode.rerender();"));
    writer.line(&i.inner4, format_args!("}}"));
    writer.line(&i.inner3, format_args!("}});"));
    writer.line(&i.inner3, format_args!("let paramsLambda = () => {{"));
    writer.line(&i.inner4, format_args!("return {};", params.lambda));
    writer.line(&i.inner3, format_args!("}};"));
    writer.line(
        &i.inner3,
        format_args!("componentCall.paramsGenerator_ = paramsLambda;"),
    );
    writer.line(&i.inner2, format_args!("}} else {{"));
    writer.line(
        &i.inner3,
        format_args!(
            "{}.updateStateVarsOfChildByElmtId(elmtId, {});",
            context.view_context(),
            params.update
        ),
    );
    writer.line(&i.inner2, format_args!("}}"));
    writer.line(
        &i.inner,
        format_args!("    ViewStackProcessor.StopGetAccessRecording();"),
    );
    writer.line(&i.inner, format_args!("}});"));
    writer.base(format_args!("}}"));
    writer.base(format_args!("__Recycle__.pop();"));
}

struct CustomComponentReuseId;

impl CustomComponentReuseId {
    fn default_for(name: &str) -> String {
        let mut out = String::new();
        write!(out, "'{name}'").expect("writing to String cannot fail");
        out
    }
}

pub(super) fn custom_reuse_id_from_leaf_attrs(attrs: &[(String, String, bool)]) -> Option<String> {
    attrs
        .iter()
        .find(|(name, _, _)| name == "reuseId")
        .map(|(_, args, _)| args.trim().to_string())
}

pub(super) fn custom_reuse_id_from_component_attrs(
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
        CustomComponentObjectEmitter::new(properties).emit()
    }
}

struct CustomComponentObjectEmitter {
    properties: Vec<String>,
}

impl CustomComponentObjectEmitter {
    fn new(properties: Vec<String>) -> Self {
        Self { properties }
    }

    fn emit(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        for (index, property) in self.properties.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str(property);
        }
        out.push_str("\n      }");
        out
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
            Some(CustomComponentPropertyEmitter::key_value(&key, &value))
        }
        ObjectPropertyKind::SpreadProperty(spread) => {
            if mode == ParamMode::Update {
                None
            } else {
                Some(CustomComponentPropertyEmitter::spread(&original_node_text(
                    spread.as_ref(),
                    source_text,
                )))
            }
        }
    }
}

struct CustomComponentPropertyEmitter;

impl CustomComponentPropertyEmitter {
    fn key_value(key: &str, value: &str) -> String {
        let mut out = String::new();
        write!(out, "        {key}: {value}").expect("writing to String cannot fail");
        out
    }

    fn spread(spread: &str) -> String {
        let mut out = String::new();
        write!(out, "        {spread}").expect("writing to String cannot fail");
        out
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
            ParamMode::Ctor => CustomComponentReference::backing(&name),
            ParamMode::Lambda => CustomComponentReference::direct(&name),
            ParamMode::Update => original_node_text(value, source_text),
        };
    }
    if mode == ParamMode::Ctor
        && property_name
            .is_some_and(|name| component_info.is_some_and(|info| info.link_props.contains(name)))
    {
        if let Some(name) = this_member_name(value) {
            return CustomComponentReference::backing(&name);
        }
    }
    if matches!(mode, ParamMode::Ctor | ParamMode::Lambda) {
        if let Some(text) = expression_text_with_raw_object_call_args(value, source_text, context) {
            return text;
        }
    }
    original_node_text(value, source_text)
}

struct CustomComponentReference;

impl CustomComponentReference {
    fn backing(name: &str) -> String {
        let mut out = String::new();
        write!(out, "this.__{name}").expect("writing to String cannot fail");
        out
    }

    fn direct(name: &str) -> String {
        let mut out = String::new();
        write!(out, "this.{name}").expect("writing to String cannot fail");
        out
    }
}

fn this_member_name(value: &Expression<'_>) -> Option<String> {
    let Expression::StaticMemberExpression(member) = value else {
        return None;
    };
    matches!(&member.object, Expression::ThisExpression(_))
        .then(|| member.property.name.to_string())
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
