use rustc_hash::FxHashSet;
use std::fmt::Write;

use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, ArrowFunctionExpression, BindingPattern,
    Expression, FormalParameter,
};

use crate::transform::component_meta::ComponentMetadata;
use crate::transform::source_emit::original_node_text;
use crate::transform::viewpu_emit::ViewpuWriter;

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
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    context: BuildContext,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let mut writer = ViewpuWriter::new(out);
    writer.line(indent, format_args!("{{"));
    let indent_scope = IterationIndent::new(indent);
    let inner = indent_scope.level(1);
    let child_indent = indent_scope.level(3);
    writer.line(
        &inner,
        format_args!("const itemCreation = (elmtId, isInitialRender) => {{"),
    );
    writer.line(
        &inner,
        format_args!("    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);"),
    );
    writer.line(
        &inner,
        format_args!("    ListItem.create(deepRenderFunction, true);"),
    );
    writer.line(&inner, format_args!("    if (!isInitialRender) {{"));
    writer.line(&inner, format_args!("        ListItem.pop();"));
    writer.line(&inner, format_args!("    }}"));
    writer.line(
        &inner,
        format_args!("    ViewStackProcessor.StopGetAccessRecording();"),
    );
    writer.line(&inner, format_args!("}};"));
    writer.line(
        &inner,
        format_args!("const itemCreation2 = (elmtId, isInitialRender) => {{"),
    );
    writer.line(
        &inner,
        format_args!("    ListItem.create(deepRenderFunction, true);"),
    );
    for chain in &component.chain_expressions {
        let chain_name = chain_name(chain)?;
        let chain_args = attribute_arguments_text(
            &chain_name,
            &chain.arguments,
            source_text,
            builder_methods,
            &context,
        );
        writer.line(
            &inner,
            format_args!("    ListItem.{chain_name}({chain_args});"),
        );
    }
    writer.line(&inner, format_args!("}};"));
    writer.line(
        &inner,
        format_args!("const deepRenderFunction = (elmtId, isInitialRender) => {{"),
    );
    writer.line(
        &inner,
        format_args!("    itemCreation(elmtId, isInitialRender);"),
    );
    drop(writer);
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
                    &child_indent,
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
                    &child_indent,
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
                    &child_indent,
                    out,
                    span_hints,
                )?;
            }
        }
    }
    let mut writer = ViewpuWriter::new(out);
    writer.line(&inner, format_args!("    ListItem.pop();"));
    writer.line(&inner, format_args!("}};"));
    writer.line(
        &inner,
        format_args!(
            "{}.observeComponentCreation2(itemCreation2, ListItem);",
            context.view_context()
        ),
    );
    writer.line(&inner, format_args!("ListItem.pop();"));
    writer.line(indent, format_args!("}}"));
    Ok(())
}

pub(super) fn lower_for_each(
    arguments: &[Argument<'_>],
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
    let generator_params = ForEachGeneratorParams::new(item_param, index_param, source_text).emit();

    {
        let mut writer = ViewpuWriter::new(out);
        writer.line(
            indent,
            format_args!(
                "{}.observeComponentCreation2((elmtId, isInitialRender) => {{",
                context.view_context()
            ),
        );
        writer.line(indent, format_args!("    ForEach.create();"));
        writer.line(
            indent,
            format_args!("    const forEachItemGenFunction = {generator_params} => {{"),
        );
        if let Some(item_param) = item_param {
            let Some(item_name) = binding_parameter_name(item_param) else {
                return Err(BuildLoweringError::UnsupportedChild);
            };
            writer.line(indent, format_args!("        const {item_name} = _item;"));
        }
    }
    let body_indent = IterationIndent::new(indent).level(2);
    lower_lazy_arrow_body(
        item_gen,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &body_indent,
        out,
        span_hints,
    )?;

    let mut writer = ViewpuWriter::new(out);
    writer.line(indent, format_args!("    }};"));
    if item_param.is_some() {
        if let Some(key_arg) = arguments.get(2) {
            let key = raw_argument_text_with_context(key_arg, source_text, &context);
            let item_gen_uses_index = index_param.is_some();
            let key_uses_index = argument_arrow(key_arg)
                .and_then(|arrow| arrow.params.items.get(1))
                .is_some();
            writer.line(
                indent,
                format_args!(
                    "    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction, {key}, {item_gen_uses_index}, {key_uses_index});",
                    context.view_context()
                ),
            );
        } else {
            writer.line(
                indent,
                format_args!(
                    "    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction);",
                    context.view_context()
                ),
            );
        }
    } else {
        writer.line(
            indent,
            format_args!(
                "    {}.forEachUpdateFunction(elmtId, {data}, forEachItemGenFunction);",
                context.view_context()
            ),
        );
    }

    writer.line(indent, format_args!("}}, ForEach);"));
    writer.line(indent, format_args!("ForEach.pop();"));
    Ok(())
}

pub(super) fn lower_lazy_for_each(
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
    {
        let mut writer = ViewpuWriter::new(out);
        writer.line(
            indent,
            format_args!("const __lazyForEachItemGenFunction = {item_params} => {{"),
        );
    }
    let body_indent = IterationIndent::new(indent).level(1);
    lower_lazy_arrow_body(
        item_gen,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        context.clone(),
        &body_indent,
        out,
        span_hints,
    )?;

    let mut writer = ViewpuWriter::new(out);
    writer.line(indent, format_args!("}};"));
    if let Some(id_func_arg) = component.arguments.get(2) {
        let id_func = raw_argument_text_with_context(id_func_arg, source_text, &context);
        writer.line(
            indent,
            format_args!("const __lazyForEachItemIdFunc = {id_func};"),
        );
        writer.line(
            indent,
            format_args!(
                "LazyForEach.create(\"1\", {}, {data}, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);",
                context.view_context()
            ),
        );
    } else {
        writer.line(
            indent,
            format_args!(
                "LazyForEach.create(\"1\", {}, {data}, __lazyForEachItemGenFunction);",
                context.view_context()
            ),
        );
    }
    writer.line(indent, format_args!("LazyForEach.pop();"));
    Ok(())
}

fn lower_lazy_arrow_body(
    arrow: &ArrowFunctionExpression<'_>,
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
    if arrow.expression {
        ViewpuWriter::new(out).line(indent, format_args!("return undefined;"));
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

struct IterationIndent<'a> {
    base: &'a str,
}

impl<'a> IterationIndent<'a> {
    fn new(base: &'a str) -> Self {
        Self { base }
    }

    fn level(&self, depth: usize) -> String {
        let mut out = String::from(self.base);
        for _ in 0..depth {
            out.push_str("    ");
        }
        out
    }
}

struct ForEachGeneratorParams<'a> {
    item: Option<&'a FormalParameter<'a>>,
    index: Option<&'a FormalParameter<'a>>,
    source_text: &'a str,
}

impl<'a> ForEachGeneratorParams<'a> {
    fn new(
        item: Option<&'a FormalParameter<'a>>,
        index: Option<&'a FormalParameter<'a>>,
        source_text: &'a str,
    ) -> Self {
        Self {
            item,
            index,
            source_text,
        }
    }

    fn emit(&self) -> String {
        match (self.item, self.index) {
            (None, _) => "()".to_string(),
            (Some(_), Some(index_param)) => {
                let mut out = String::new();
                write!(
                    out,
                    "(_item, {})",
                    original_node_text(index_param, self.source_text)
                )
                .expect("writing to String cannot fail");
                out
            }
            (Some(_), None) => "_item".to_string(),
        }
    }
}
