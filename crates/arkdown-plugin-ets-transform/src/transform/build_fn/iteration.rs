use std::collections::HashSet;

use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionExpression, BindingPattern,
    Expression, FormalParameter,
};

use crate::transform::component_meta::ComponentMetadata;
use crate::transform::source_emit::original_node_text;

use super::raw_object::raw_object_argument_text_from_argument;
use super::{
    attribute_arguments_text, chain_name, lower_component, lower_expression, lower_statement,
    raw_argument_text, raw_argument_text_with_context, BuildContext, BuildLoweringError,
    CustomComponentUpdateProps, SpanHint,
};

pub(super) fn is_lazy_list_item(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
) -> bool {
    !component
        .arguments
        .first()
        .is_some_and(|arg| raw_argument_text(arg, source_text).trim() == "\"false\"")
}

pub(super) fn lower_lazy_list_item(
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

pub(super) fn lower_for_each(
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

pub(super) fn lower_lazy_for_each(
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
