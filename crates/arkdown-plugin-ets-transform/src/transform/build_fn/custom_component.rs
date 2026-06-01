use oxc_ast::ast::{
    Argument, ArkUIComponentExpression, Expression, ObjectExpression, ObjectPropertyKind,
    PropertyKey,
};
use oxc_span::Span;

use crate::transform::source_emit::original_node_text;

use super::raw_object::expression_text_with_raw_object_call_args;
use super::{
    chain_name, component_page_path, push_span_hint, raw_arguments_text, source_position,
    BuildContext, CustomComponentInfo, SpanHint,
};

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
