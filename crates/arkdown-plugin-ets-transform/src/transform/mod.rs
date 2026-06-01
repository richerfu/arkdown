pub mod build_fn;
pub mod builder;
pub mod component;
mod component_analysis;
mod component_collection;
pub mod component_meta;
pub mod decorator;
pub mod entry;
pub mod global_rewrite;
pub mod source_emit;
mod source_text;
mod syntax;
pub mod viewpu_emit;
mod viewpu_finalize;
mod viewpu_source_map;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use arkdown_core::ModuleSource;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, Declaration, Decorator, ExportDefaultDeclarationKind,
    Function, Statement, StructElement, StructStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::error::EtsTransformError;
use crate::transform::build_fn::{
    lower_build_body_with_component_context, lower_builder_body_with_component_context,
    lower_builder_body_with_custom_components, lower_styles_body, CustomComponentUpdateProps,
};
use crate::transform::component::{
    generate_view_pu_class, BuilderMethodInfo, ComponentInfo, ParamField, RegularField, StateField,
    StylesMethodInfo,
};
use crate::transform::component_analysis::{
    collect_builder_function_names, collect_custom_component_names,
    collect_imported_component_names, collect_local_component_update_props,
    collect_struct_builder_method_names, collect_struct_builder_param_names, SimpleTypeContext,
};
use crate::transform::component_collection::{
    collect_statement_component_names, collect_statement_if_component_names,
};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{
    decorator_arg_text, decorator_name, decorator_string_arg, is_state_decorator,
};
use crate::transform::entry::EntryRegistrationInfo;
use crate::transform::global_rewrite::rewrite_global_context_calls_in_function_text;
use crate::transform::source_text::{
    balanced_method_text, component_replacement_start, decorator_first_arg_text,
    extract_params_text, leading_decorator_start, member_text_without_decorators, span_text,
    type_annotation_text,
};
pub(crate) use crate::transform::syntax::has_arkts_transform_syntax;
use crate::transform::syntax::program_has_arkts_transform_syntax;
use crate::transform::viewpu_finalize::finalize_viewpu_output;
pub use crate::transform::viewpu_source_map::transform_ets_viewpu_with_source_map;

#[derive(Debug, Clone, Default)]
pub struct TransformOptions {
    pub bundle_name: Option<String>,
    pub module_name: Option<String>,
    pub project_root: Option<PathBuf>,
    pub module_roots: Vec<(String, PathBuf)>,
    pub component_metadata: ComponentMetadata,
}

/// Transform an ArkTS `.ets` source file into TypeScript.
///
/// ArkUI parsing is handled by the workspace-pinned ohos-rs/oxc fork using
/// `SourceType::ets()`. This lets us extract real `StructStatement` and
/// `ArkUIComponentExpression` nodes instead of guessing from text.
pub fn transform_ets(source: &ModuleSource) -> Result<String, EtsTransformError> {
    transform_ets_with_options(source, &TransformOptions::default())
}

/// Collect ArkUI component names from an ETS file using OXC's ArkUI AST nodes.
///
/// Hvigor writes this information to `component_collection.json` during the
/// widget CompileArkTS pass.  The collection is based on the parsed ArkUI
/// component expressions rather than source-text matching.
pub fn collect_arkui_component_names(
    content: &str,
    id: &str,
) -> Result<Vec<String>, EtsTransformError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(EtsTransformError::Fatal {
            file: id.to_string(),
            message: format!("ArkUI parse failed: {:?}", parsed.errors),
        });
    }

    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut custom_components = HashSet::new();
    for statement in &parsed.program.body {
        collect_custom_component_names(statement, &mut custom_components);
    }
    for statement in &parsed.program.body {
        collect_statement_component_names(statement, &custom_components, &mut names, &mut seen);
    }
    for statement in &parsed.program.body {
        collect_statement_if_component_names(statement, &mut names, &mut seen);
    }
    Ok(names)
}

pub fn transform_ets_with_options(
    source: &ModuleSource,
    options: &TransformOptions,
) -> Result<String, EtsTransformError> {
    let content = &source.content;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&source.id.id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(EtsTransformError::Fatal {
            file: source.id.id.clone(),
            message: format!("ArkUI parse failed: {:?}", parsed.errors),
        });
    }

    if !program_has_arkts_transform_syntax(&parsed.program) {
        return Ok(content.clone());
    }

    let diagnostics = crate::checker::check(content, &source.id.id);
    for diag in &diagnostics {
        tracing::debug!(file = %source.id.id, diagnostic = ?diag, "ets checker diagnostic");
    }
    if let Some(error) = diagnostics.iter().find(|diag| {
        matches!(
            diag,
            EtsTransformError::Diagnostic {
                severity: crate::error::DiagnosticSeverity::Error,
                ..
            } | EtsTransformError::Fatal { .. }
        )
    }) {
        return Err(error.clone());
    }

    let simple_type_context =
        SimpleTypeContext::from_program(&parsed.program, &source.id.id, options);
    let hvigor_source_id = hvigor_source_path(&source.id.id, options);
    let mut custom_components = HashSet::new();
    let mut custom_component_update_props = CustomComponentUpdateProps::new();
    for statement in &parsed.program.body {
        collect_custom_component_names(statement, &mut custom_components);
        collect_local_component_update_props(statement, &mut custom_component_update_props);
    }
    collect_imported_component_names(
        &parsed.program,
        &source.id.id,
        options,
        &mut custom_components,
        &mut custom_component_update_props,
    );
    let builder_functions = collect_builder_function_names(&parsed.program, &source.id.id, options);

    let mut out = String::with_capacity(content.len() + 512);
    let mut last_end = 0usize;
    let mut transformed_any = false;

    for statement in &parsed.program.body {
        let span = statement.span();
        if let Some(component) = component_from_statement(
            statement,
            content,
            &source.id.id,
            options,
            &simple_type_context,
            &custom_components,
            &custom_component_update_props,
            &builder_functions,
            &hvigor_source_id,
        )? {
            let start = component_replacement_start(statement, content);
            out.push_str(&content[last_end..start]);
            out.push_str(&generate_view_pu_class(&component));
            transformed_any = true;
        } else if let Some((start, replacement)) = transform_non_component_statement(
            statement,
            content,
            &source.id.id,
            options,
            &custom_components,
            &custom_component_update_props,
            &builder_functions,
            &hvigor_source_id,
        )? {
            out.push_str(&content[last_end..start]);
            out.push_str(&replacement);
            transformed_any = true;
        } else {
            out.push_str(&content[last_end..span.start as usize]);
            out.push_str(span_text(span, content));
        }

        last_end = span.end as usize;
    }

    out.push_str(&content[last_end..]);

    if transformed_any {
        while out.ends_with("\n\n") {
            out.pop();
        }
        Ok(finalize_viewpu_output(&out))
    } else {
        Ok(content.clone())
    }
}

fn component_from_statement(
    statement: &Statement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
    simple_type_context: &SimpleTypeContext,
    custom_components: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &HashSet<String>,
    hvigor_source_id: &str,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    match statement {
        Statement::StructStatement(statement) => component_from_struct(
            statement,
            source_text,
            id,
            options,
            false,
            false,
            simple_type_context,
            custom_components,
            custom_component_update_props,
            builder_functions,
            hvigor_source_id,
        ),
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::StructStatement(statement)) => component_from_struct(
                statement,
                source_text,
                id,
                options,
                true,
                false,
                simple_type_context,
                custom_components,
                custom_component_update_props,
                builder_functions,
                hvigor_source_id,
            ),
            _ => Ok(None),
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::StructStatement(statement) => component_from_struct(
                statement,
                source_text,
                id,
                options,
                true,
                true,
                simple_type_context,
                custom_components,
                custom_component_update_props,
                builder_functions,
                hvigor_source_id,
            ),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn transform_non_component_statement(
    statement: &Statement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
    custom_components: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &HashSet<String>,
    hvigor_source_id: &str,
) -> Result<Option<(usize, String)>, EtsTransformError> {
    let replacement = match statement {
        Statement::AnnotationDeclaration(annotation) => Some((
            annotation.span.start as usize,
            annotation_to_interface(annotation, source_text),
        )),
        Statement::LazyImportDeclaration(_) => {
            let text =
                span_text(statement.span(), source_text).replacen("import lazy", "import", 1);
            Some((statement.span().start as usize, text))
        }
        Statement::FunctionDeclaration(function) => transform_decorated_function(
            function,
            source_text,
            id,
            custom_components,
            custom_component_update_props,
            builder_functions,
            &options.component_metadata,
            false,
            &[],
            hvigor_source_id,
        )?,
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::FunctionDeclaration(function)) => {
                let mut result = transform_decorated_function(
                    function,
                    source_text,
                    id,
                    custom_components,
                    custom_component_update_props,
                    builder_functions,
                    &options.component_metadata,
                    true,
                    &export.decorators,
                    hvigor_source_id,
                )?
                .ok_or_else(|| EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: "decorated export function did not transform".to_string(),
                })?;
                for decorator in &export.decorators {
                    result.0 = result.0.min(decorator.span.start as usize);
                }
                result.0 = leading_decorator_start(source_text, result.0);
                Some(result)
            }
            Some(Declaration::AnnotationDeclaration(annotation)) => {
                let mut start = export.span.start as usize;
                for decorator in &export.decorators {
                    start = start.min(decorator.span.start as usize);
                }
                Some((
                    start,
                    format!(
                        "export {}",
                        annotation_to_interface(annotation, source_text)
                    ),
                ))
            }
            _ => None,
        },
        _ => None,
    };
    Ok(replacement)
}

fn annotation_to_interface(annotation: &AnnotationDeclaration<'_>, source_text: &str) -> String {
    let mut out = format!("interface {} {{\n", annotation.id.name);
    for element in &annotation.body.body {
        let AnnotationElement::PropertyDefinition(property) = element;
        let Some(name) = property.key.static_name() else {
            continue;
        };
        let ty = property
            .type_annotation
            .as_ref()
            .map(|annotation| type_annotation_text(annotation.span(), source_text))
            .unwrap_or_else(|| "unknown".to_string());
        let optional = if property.optional { "?" } else { "" };
        out.push_str(&format!("    {name}{optional}: {ty};\n"));
    }
    out.push_str("}\n");
    out
}

fn transform_decorated_function(
    function: &Function<'_>,
    source_text: &str,
    id: &str,
    custom_components: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &HashSet<String>,
    component_metadata: &ComponentMetadata,
    exported: bool,
    extra_decorators: &[Decorator<'_>],
    hvigor_source_id: &str,
) -> Result<Option<(usize, String)>, EtsTransformError> {
    let decorators = function.decorators.iter().chain(extra_decorators.iter());
    let decorator_kind = decorators
        .clone()
        .map(decorator_name)
        .find(|name| {
            matches!(
                *name,
                "Builder" | "LocalBuilder" | "Styles" | "Extend" | "AnimatableExtend"
            )
        })
        .map(str::to_string);
    let Some(decorator_kind) = decorator_kind else {
        return Ok(None);
    };
    let Some(function_id) = function.id.as_ref() else {
        return Ok(None);
    };
    let name = function_id.name.as_str();
    let params = extract_params_text(function.params.span(), source_text);
    let params = if matches!(decorator_kind.as_str(), "Builder" | "LocalBuilder") {
        append_builder_parent_param(params)
    } else {
        params
    };
    let return_annotation = function
        .return_type
        .as_ref()
        .map(|annotation| format!(": {}", type_annotation_text(annotation.span(), source_text)))
        .unwrap_or_default();
    let export_prefix = if exported { "export " } else { "" };
    let start = decorators
        .clone()
        .fold(function.span.start, |start, decorator| {
            start.min(decorator.span.start)
        }) as usize;
    let start = leading_decorator_start(source_text, start);

    let body = match decorator_kind.as_str() {
        "Styles" => lower_styles_body(function.body.as_deref(), source_text),
        "Extend" | "AnimatableExtend" => {
            let component = function
                .decorators
                .iter()
                .chain(extra_decorators.iter())
                .find(|decorator| decorator_name(decorator) == decorator_kind)
                .and_then(|decorator| decorator_first_arg_text(decorator, source_text))
                .unwrap_or_else(|| "View".to_string());
            return Ok(Some((
                start,
                format!(
                    "{export_prefix}function __{component}__{name}({params}): void {{\n{}    }}",
                    lower_extend_body(function.body.as_deref(), source_text, &component)
                ),
            )));
        }
        _ => {
            lower_builder_body_with_custom_components(
                function.body.as_deref(),
                source_text,
                hvigor_source_id,
                custom_components,
                builder_functions,
                custom_component_update_props,
                component_metadata,
            )
            .map_err(|err| EtsTransformError::Fatal {
                file: id.to_string(),
                message: format!("unsupported ArkUI builder DSL: {err:?}"),
            })?
            .0
        }
    };

    Ok(Some((
        start,
        format!("{export_prefix}function {name}({params}){return_annotation} {{\n{body}    }}"),
    )))
}

fn lower_extend_body(
    body: Option<&oxc_ast::ast::FunctionBody<'_>>,
    source_text: &str,
    component: &str,
) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let body_text = span_text(body.span(), source_text);
    let inner = strip_outer_block(&body_text);
    let mut out = String::new();
    for line in inner.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str("        ");
        if trimmed.starts_with('.') {
            out.push_str(component);
            out.push_str(trimmed);
        } else {
            out.push_str(trimmed);
        }
        if !trimmed.ends_with(';') && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
            out.push(';');
        }
        out.push('\n');
    }
    out
}

fn strip_outer_block(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

fn component_from_struct(
    statement: &StructStatement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
    is_exported: bool,
    is_default_export: bool,
    simple_type_context: &SimpleTypeContext,
    custom_components: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &HashSet<String>,
    hvigor_source_id: &str,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    if !is_component_struct(statement) {
        return Ok(None);
    }

    let is_entry = statement
        .decorators
        .iter()
        .any(|decorator| decorator_name(decorator) == "Entry");
    let mut state_fields = Vec::new();
    let mut initial_render_body = String::new();
    let mut initial_render_span_hints = Vec::new();
    let mut builder_methods: Vec<BuilderMethodInfo> = Vec::new();
    let builder_method_names = collect_struct_builder_method_names(statement);
    let builder_param_names = collect_struct_builder_param_names(statement);
    let mut visible_builder_methods = builder_functions.clone();
    visible_builder_methods.extend(builder_method_names.iter().cloned());
    visible_builder_methods.extend(builder_param_names.iter().cloned());
    let raw_object_fields = component_raw_object_field_names(statement, simple_type_context);
    let mut styles_methods: Vec<StylesMethodInfo> = Vec::new();
    let mut regular_members: Vec<String> = Vec::new();
    let mut params_fields: Vec<ParamField> = Vec::new();
    let mut property_order: Vec<String> = Vec::new();
    let mut regular_fields: Vec<RegularField> = Vec::new();

    for element in &statement.body.body {
        match element {
            StructElement::PropertyDefinition(property) => {
                if let Some(name) = property.key.static_name() {
                    let type_annotation = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| type_annotation_text(annotation.span(), source_text))
                        .unwrap_or_default();
                    let name = name.into_owned();
                    property_order.push(name.clone());
                    params_fields.push(ParamField {
                        name,
                        type_annotation: type_annotation.clone(),
                    });
                }

                let decorator_names: Vec<&str> =
                    property.decorators.iter().map(decorator_name).collect();
                let Some(decorator) = primary_state_decorator(&decorator_names) else {
                    if let Some(name) = property.key.static_name() {
                        let type_annotation = property
                            .type_annotation
                            .as_ref()
                            .map(|annotation| type_annotation_text(annotation.span(), source_text))
                            .unwrap_or_default();
                        let initializer = property
                            .value
                            .as_ref()
                            .map(|value| span_text(value.span(), source_text).trim().to_string());
                        regular_fields.push(RegularField {
                            name: name.into_owned(),
                            type_annotation,
                            initializer,
                            optional: property.optional,
                        });
                    } else {
                        regular_members.push(
                            member_text_without_decorators(
                                source_text,
                                element.span(),
                                &property.decorators,
                            )
                            .trim()
                            .to_string(),
                        );
                    }
                    continue;
                };
                let Some(name) = property.key.static_name() else {
                    continue;
                };
                let type_node = property
                    .type_annotation
                    .as_ref()
                    .map(|annotation| &annotation.type_annotation);
                let type_annotation = property
                    .type_annotation
                    .as_ref()
                    .map(|annotation| type_annotation_text(annotation.span(), source_text))
                    .unwrap_or_default();
                let initializer = property
                    .value
                    .as_ref()
                    .map(|value| span_text(value.span(), source_text).trim().to_string());
                // Extract @Watch("handlerName") from co-decorators on this field.
                let watch_handler = property
                    .decorators
                    .iter()
                    .find(|d| decorator_name(d) == "Watch")
                    .and_then(decorator_string_arg);
                // Extract alias from @Provide("alias") or @Consume("alias").
                let alias = property
                    .decorators
                    .iter()
                    .find(|d| {
                        matches!(
                            decorator_name(d),
                            "Provide" | "Consume" | "Provider" | "Consumer"
                        )
                    })
                    .and_then(decorator_string_arg);
                let decorator_arg = property
                    .decorators
                    .iter()
                    .find(|d| decorator_name(d) == decorator)
                    .and_then(|d| decorator_arg_text(d, source_text));
                state_fields.push(StateField {
                    name: name.into_owned(),
                    type_annotation,
                    decorator: decorator.to_string(),
                    initializer,
                    watch_handler,
                    alias,
                    decorator_arg,
                    is_simple_type: type_node
                        .is_none_or(|ty| simple_type_context.is_simple_type_annotation(ty)),
                });
            }
            StructElement::MethodDefinition(method)
                if method.key.static_name().is_some_and(|name| name == "build") =>
            {
                let (body, hints) = lower_build_body_with_component_context(
                    method.value.body.as_deref(),
                    source_text,
                    hvigor_source_id,
                    custom_components,
                    &visible_builder_methods,
                    custom_component_update_props,
                    &options.component_metadata,
                    &raw_object_fields,
                )
                .map_err(|err| EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: format!("unsupported ArkUI build() DSL: {err:?}"),
                })?;
                initial_render_body = body;
                initial_render_span_hints = hints;
            }
            StructElement::MethodDefinition(method)
                if method
                    .decorators
                    .iter()
                    .any(|d| decorator_name(d) == "Builder") =>
            {
                // @Builder method: keep same name, desugar body like build().
                let Some(name) = method.key.static_name() else {
                    continue;
                };
                let params = append_builder_parent_param(extract_params_text(
                    method.value.params.span(),
                    source_text,
                ));
                let (lowered_body, _) = lower_builder_body_with_component_context(
                    method.value.body.as_deref(),
                    source_text,
                    hvigor_source_id,
                    custom_components,
                    &visible_builder_methods,
                    custom_component_update_props,
                    &options.component_metadata,
                    &raw_object_fields,
                )
                .map_err(|err| EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: format!("unsupported ArkUI @Builder DSL: {err:?}"),
                })?;
                builder_methods.push(BuilderMethodInfo {
                    name: name.into_owned(),
                    params,
                    return_annotation: method
                        .value
                        .return_type
                        .as_ref()
                        .map(|annotation| {
                            format!(": {}", type_annotation_text(annotation.span(), source_text))
                        })
                        .unwrap_or_default(),
                    lowered_body,
                });
            }
            StructElement::MethodDefinition(method)
                if method
                    .decorators
                    .iter()
                    .any(|d| decorator_name(d) == "Styles") =>
            {
                // @Styles method: strip decorator, emit as private, lower body.
                let Some(name) = method.key.static_name() else {
                    continue;
                };
                let params = extract_params_text(method.value.params.span(), source_text);
                let lowered_body = lower_styles_body(method.value.body.as_deref(), source_text);
                styles_methods.push(StylesMethodInfo {
                    name: name.into_owned(),
                    params,
                    lowered_body,
                });
            }
            StructElement::MethodDefinition(method) => {
                let (member_start, member_text) =
                    balanced_method_text(method, statement.span(), source_text).unwrap_or_else(
                        || {
                            (
                                element.span().start as usize,
                                span_text(element.span(), source_text).to_string(),
                            )
                        },
                    );
                let member_text = rewrite_global_context_calls_in_function_text(
                    &method.value,
                    source_text,
                    member_start,
                    &member_text,
                );
                regular_members.push(member_text.trim().to_string());
            }
        }
    }

    Ok(Some(ComponentInfo {
        name: statement.id.name.to_string(),
        is_entry,
        is_exported,
        is_default_export,
        is_reusable: statement
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Reusable"),
        state_fields,
        params_fields,
        property_order,
        regular_fields,
        regular_members,
        initial_render_body,
        initial_render_span_hints,
        entry_registration: is_entry.then(|| {
            let storage = statement
                .decorators
                .iter()
                .find(|decorator| decorator_name(decorator) == "Entry")
                .and_then(|decorator| decorator_arg_text(decorator, source_text));
            entry_registration(hvigor_source_id, options, storage)
        }),
        builder_methods,
        styles_methods,
    }))
}

fn is_component_struct(statement: &StructStatement<'_>) -> bool {
    statement.decorators.iter().any(|decorator| {
        matches!(
            decorator_name(decorator),
            "Component" | "ComponentV2" | "CustomDialog"
        )
    })
}
fn append_builder_parent_param(params: String) -> String {
    if params.trim().is_empty() {
        "parent = null".to_string()
    } else {
        format!("{params}, parent = null")
    }
}

fn primary_state_decorator(names: &[&str]) -> Option<&'static str> {
    // Mirrors upstream arkui-plugins classifyV1Property/classifyV2Property order.
    // Meta decorators such as @Watch and @Require are intentionally absent.
    [
        "State",
        "StorageLink",
        "StorageProp",
        "LocalStorageLink",
        "LocalStorageProp",
        "Link",
        "ObjectLink",
        "Prop",
        "LocalStoragePropRef",
        "StoragePropRef",
        "PropRef",
        "Provide",
        "Consume",
        "BuilderParam",
        "Local",
        "Once",
        "Param",
        "Provider",
        "Consumer",
        "Event",
        "Env",
        "Trace",
    ]
    .into_iter()
    .find(|candidate| names.iter().any(|name| name == candidate) && is_state_decorator(candidate))
}

fn component_raw_object_field_names(
    statement: &StructStatement<'_>,
    simple_type_context: &SimpleTypeContext,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        let decorator_names: Vec<&str> = property.decorators.iter().map(decorator_name).collect();
        let Some(decorator) = primary_state_decorator(&decorator_names) else {
            continue;
        };
        if decorator == "BuilderParam" {
            continue;
        }
        let Some(name) = property.key.static_name() else {
            continue;
        };
        let Some(type_node) = property
            .type_annotation
            .as_ref()
            .map(|annotation| &annotation.type_annotation)
        else {
            continue;
        };
        if !simple_type_context.is_simple_type_annotation(type_node) {
            names.insert(name.into_owned());
        }
    }
    names
}

fn entry_registration(
    id: &str,
    options: &TransformOptions,
    storage: Option<String>,
) -> EntryRegistrationInfo {
    let module_name = options
        .module_name
        .clone()
        .or_else(|| id.split("/src/main/ets/").next().map(str::to_string))
        .unwrap_or_else(|| "entry".to_string());
    let page_path = id
        .split("/src/main/ets/")
        .nth(1)
        .unwrap_or(id)
        .trim_end_matches(".ets")
        .to_string();
    let page_full_path = id.trim_end_matches(".ets").to_string();

    EntryRegistrationInfo {
        bundle_name: options.bundle_name.clone().unwrap_or_default(),
        module_name,
        page_path,
        page_full_path,
        integrated_hsp: "false".to_string(),
        module_type: "followWithHap".to_string(),
        storage,
    }
}

fn hvigor_source_path(id: &str, options: &TransformOptions) -> String {
    id.find("products/")
        .or_else(|| id.find("features/"))
        .or_else(|| id.find("commons/"))
        .map(|idx| id[idx..].to_string())
        .unwrap_or_else(|| {
            let path = Path::new(id);
            if let Some(project_root) = &options.project_root {
                if let Ok(relative) = path.strip_prefix(project_root) {
                    return slash_path(relative).trim_start_matches('/').to_string();
                }
            }
            id.to_string()
        })
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
