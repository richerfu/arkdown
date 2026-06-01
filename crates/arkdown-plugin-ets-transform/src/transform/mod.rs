pub mod build_fn;
pub mod builder;
pub mod component;
pub mod component_meta;
pub mod decorator;
pub mod entry;
pub mod global_rewrite;
pub mod source_emit;
pub mod viewpu_emit;
mod viewpu_finalize;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use arkdown_core::ModuleSource;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, Argument, ArkUIChild, ArkUIComponentExpression,
    CallExpression, Declaration, Decorator, ExportDefaultDeclarationKind, Expression, Function,
    FunctionBody, ImportDeclarationSpecifier, ModuleExportName, Program, Statement, StructElement,
    StructStatement, TSLiteral, TSType, TSTypeName,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::error::EtsTransformError;
use crate::source_map::{build_ets_source_map, offset_to_line_col, MapEntry};
use crate::transform::build_fn::{
    lower_build_body_with_component_context, lower_builder_body_with_component_context,
    lower_builder_body_with_custom_components, lower_styles_body, CustomComponentInfo,
    CustomComponentUpdateProps,
};
use crate::transform::component::{
    generate_view_pu_class, BuilderMethodInfo, ComponentInfo, ParamField, RegularField, StateField,
    StylesMethodInfo,
};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::is_state_decorator;
use crate::transform::entry::EntryRegistrationInfo;
use crate::transform::global_rewrite::rewrite_global_context_calls_in_function_text;
use crate::transform::viewpu_finalize::finalize_viewpu_output;

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

const KNOWN_ARKUI_DECORATORS: &[&str] = &[
    "Component",
    "Entry",
    "CustomDialog",
    "Reusable",
    "ComponentV2",
    "ReusableV2",
    "Builder",
    "LocalBuilder",
    "Styles",
    "Extend",
    "AnimatableExtend",
    "State",
    "Prop",
    "Link",
    "ObjectLink",
    "Provide",
    "Consume",
    "BuilderParam",
    "StorageLink",
    "StorageProp",
    "StoragePropRef",
    "LocalStorageLink",
    "LocalStorageProp",
    "LocalStoragePropRef",
    "Local",
    "Param",
    "Once",
    "Provider",
    "Consumer",
    "Event",
    "Env",
    "Require",
    "Watch",
    "Trace",
    "Computed",
    "Monitor",
    "SyncMonitor",
    "ComponentInit",
    "ComponentAppear",
    "ComponentBuilt",
    "ComponentAttach",
    "ComponentDetach",
    "ComponentReuse",
    "ComponentRecycle",
    "ComponentDisappear",
    "ComponentActive",
    "ComponentInactive",
];

pub(crate) fn has_arkts_transform_syntax(content: &str, id: &str) -> bool {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    !parsed.errors.is_empty() || program_has_arkts_transform_syntax(&parsed.program)
}

fn program_has_arkts_transform_syntax(program: &Program<'_>) -> bool {
    program
        .body
        .iter()
        .any(statement_has_arkts_transform_syntax)
}

fn statement_has_arkts_transform_syntax(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::StructStatement(_)
        | Statement::AnnotationDeclaration(_)
        | Statement::LazyImportDeclaration(_) => true,
        Statement::ClassDeclaration(class) => decorators_have_arkui_syntax(&class.decorators),
        Statement::FunctionDeclaration(function) => {
            decorators_have_arkui_syntax(&function.decorators)
        }
        Statement::ExportNamedDeclaration(export) => {
            decorators_have_arkui_syntax(&export.decorators)
                || matches!(
                    export.declaration.as_ref(),
                    Some(Declaration::StructStatement(_))
                        | Some(Declaration::AnnotationDeclaration(_))
                )
                || matches!(
                    export.declaration.as_ref(),
                    Some(Declaration::FunctionDeclaration(function))
                        if decorators_have_arkui_syntax(&function.decorators)
                )
        }
        Statement::ExportDefaultDeclaration(export) => {
            matches!(
                &export.declaration,
                ExportDefaultDeclarationKind::StructStatement(_)
            )
        }
        _ => false,
    }
}

fn decorators_have_arkui_syntax(decorators: &[Decorator<'_>]) -> bool {
    decorators
        .iter()
        .map(decorator_name)
        .any(|name| KNOWN_ARKUI_DECORATORS.contains(&name))
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

fn decorator_first_arg_text(decorator: &Decorator<'_>, source_text: &str) -> Option<String> {
    let Expression::CallExpression(call) = &decorator.expression else {
        return None;
    };
    call.arguments
        .first()
        .map(|arg| span_text(arg.span(), source_text).trim().to_string())
}

fn component_replacement_start(statement: &Statement<'_>, source_text: &str) -> usize {
    let mut start = statement.span().start;
    match statement {
        Statement::StructStatement(statement) => {
            for decorator in &statement.decorators {
                start = start.min(decorator.span.start);
            }
        }
        Statement::ExportNamedDeclaration(export) => {
            for decorator in &export.decorators {
                start = start.min(decorator.span.start);
            }
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                for decorator in &statement.decorators {
                    start = start.min(decorator.span.start);
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                for decorator in &statement.decorators {
                    start = start.min(decorator.span.start);
                }
            }
        }
        _ => {}
    }
    leading_decorator_start(source_text, start as usize)
}

fn leading_decorator_start(source_text: &str, start: usize) -> usize {
    let mut cursor = start;
    loop {
        let before = &source_text[..cursor];
        let trimmed_before = before.trim_end();
        if trimmed_before.is_empty() {
            return cursor;
        }
        let line_end = trimmed_before.len();
        let line_start = source_text[..line_end].rfind('\n').map_or(0, |idx| idx + 1);
        let line = &source_text[line_start..line_end];
        if line.trim_start().starts_with('@') {
            cursor = line_start;
            continue;
        }
        return cursor;
    }
}

fn collect_custom_component_names(statement: &Statement<'_>, names: &mut HashSet<String>) {
    match statement {
        Statement::StructStatement(statement) => {
            names.insert(statement.id.name.to_string());
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                names.insert(statement.id.name.to_string());
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                names.insert(statement.id.name.to_string());
            }
        }
        _ => {}
    }
}

fn collect_local_component_update_props(
    statement: &Statement<'_>,
    update_props: &mut CustomComponentUpdateProps,
) {
    match statement {
        Statement::StructStatement(statement) if is_component_struct(statement) => {
            update_props.insert(statement.id.name.to_string(), component_info(statement));
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                if is_component_struct(statement) {
                    update_props.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                if is_component_struct(statement) {
                    update_props.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        _ => {}
    }
}

fn collect_imported_component_names(
    program: &Program<'_>,
    id: &str,
    options: &TransformOptions,
    names: &mut HashSet<String>,
    update_props: &mut CustomComponentUpdateProps,
) {
    let importer = Path::new(id);
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(target) = resolve_import_target(import.source.value.as_str(), importer, options)
        else {
            continue;
        };
        let target_components = exported_component_names(&target, &mut HashSet::new());
        if target_components.is_empty() {
            continue;
        }
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported = module_export_name_value(&specifier.imported);
                    let local = specifier.local.name.to_string();
                    if imported == "default" && target_components.has_default {
                        names.insert(local.clone());
                        update_props.insert(local, target_components.default_info.clone());
                    } else if let Some(props) = target_components.named.get(&imported) {
                        names.insert(local.clone());
                        update_props.insert(local, props.clone());
                    }
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    if target_components.has_default {
                        let local = specifier.local.name.to_string();
                        names.insert(local.clone());
                        update_props.insert(local, target_components.default_info.clone());
                    } else if let Some(props) = target_components.named.get("default") {
                        let local = specifier.local.name.to_string();
                        names.insert(local.clone());
                        update_props.insert(local, props.clone());
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
    }
}

fn collect_builder_function_names(
    program: &Program<'_>,
    id: &str,
    options: &TransformOptions,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function)
                if function_has_decorator(function, "Builder") =>
            {
                if let Some(function_id) = &function.id {
                    names.insert(function_id.name.to_string());
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                    if function_has_decorator(function, "Builder")
                        || export
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "Builder")
                    {
                        if let Some(function_id) = &function.id {
                            names.insert(function_id.name.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let importer = Path::new(id);
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(target) = resolve_import_target(import.source.value.as_str(), importer, options)
        else {
            continue;
        };
        let target_builders = exported_builder_function_names(&target, &mut HashSet::new());
        if target_builders.is_empty() {
            continue;
        }
        for specifier in specifiers {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            let imported = module_export_name_value(&specifier.imported);
            if target_builders.contains(&imported) {
                names.insert(specifier.local.name.to_string());
            }
        }
    }
    names
}

fn function_has_decorator(function: &Function<'_>, name: &str) -> bool {
    function
        .decorators
        .iter()
        .any(|decorator| decorator_name(decorator) == name)
}

fn collect_statement_component_names(
    statement: &Statement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::StructStatement(statement) => {
            collect_struct_component_names(statement, custom_components, names, seen);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                collect_struct_component_names(statement, custom_components, names, seen);
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                collect_struct_component_names(statement, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_struct_component_names(
    statement: &StructStatement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        let Some(body) = method.value.body.as_deref() else {
            continue;
        };
        if method.key.static_name().is_some_and(|name| name == "build")
            || method
                .decorators
                .iter()
                .any(|decorator| decorator_name(decorator) == "Builder")
        {
            collect_body_component_names(body, custom_components, names, seen);
        }
    }
}

fn collect_statement_if_component_names(
    statement: &Statement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::StructStatement(statement) => {
            collect_struct_if_component_names(statement, names, seen);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                collect_struct_if_component_names(statement, names, seen);
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                collect_struct_if_component_names(statement, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_struct_if_component_names(
    statement: &StructStatement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        let Some(body) = method.value.body.as_deref() else {
            continue;
        };
        collect_body_if_component_names(body, names, seen);
    }
}

fn collect_body_if_component_names(
    body: &FunctionBody<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for statement in &body.statements {
        collect_if_statement_component_names(statement, names, seen);
    }
}

fn collect_if_statement_component_names(
    statement: &Statement<'_>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_if_statement_component_names(statement, names, seen);
            }
        }
        Statement::IfStatement(statement) => {
            push_component_name("If", &HashSet::new(), names, seen);
            collect_if_statement_component_names(&statement.consequent, names, seen);
            if let Some(alternate) = &statement.alternate {
                collect_if_statement_component_names(alternate, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_body_component_names(
    body: &FunctionBody<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for statement in &body.statements {
        collect_ui_statement_component_names(statement, custom_components, names, seen);
    }
}

fn collect_ui_statement_component_names(
    statement: &Statement<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_ui_statement_component_names(statement, custom_components, names, seen);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_ui_expression_component_names(
                &statement.expression,
                custom_components,
                names,
                seen,
            );
        }
        Statement::IfStatement(statement) => {
            push_component_name("If", custom_components, names, seen);
            collect_ui_statement_component_names(
                &statement.consequent,
                custom_components,
                names,
                seen,
            );
            if let Some(alternate) = &statement.alternate {
                collect_ui_statement_component_names(alternate, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn collect_ui_expression_component_names(
    expression: &Expression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match expression {
        Expression::ArkUIComponentExpression(component) => {
            collect_arkui_component_expression_names(component, custom_components, names, seen);
        }
        Expression::CallExpression(call) => {
            collect_call_expression_component_names(call, custom_components, names, seen);
        }
        _ => {}
    }
}

fn collect_arkui_component_expression_names(
    component: &ArkUIComponentExpression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let Expression::Identifier(identifier) = &component.callee {
        push_component_name(identifier.name.as_str(), custom_components, names, seen);
    }
    for argument in &component.arguments {
        collect_argument_component_names(argument, custom_components, names, seen);
    }
    for child in &component.children {
        match child {
            ArkUIChild::Component(component) => {
                collect_arkui_component_expression_names(component, custom_components, names, seen);
            }
            ArkUIChild::Expression(expression) => {
                collect_ui_expression_component_names(expression, custom_components, names, seen);
            }
            ArkUIChild::Statement(statement) => {
                collect_ui_statement_component_names(statement, custom_components, names, seen);
            }
        }
    }
}

fn collect_argument_component_names(
    argument: &Argument<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let Some(expression) = argument.as_expression() else {
        return;
    };
    if let Expression::ArrowFunctionExpression(arrow) = expression {
        collect_body_component_names(&arrow.body, custom_components, names, seen);
    }
}

fn collect_call_expression_component_names(
    call: &CallExpression<'_>,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match &call.callee {
        Expression::Identifier(identifier) => {
            if identifier
                .name
                .as_str()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
            {
                push_component_name(identifier.name.as_str(), custom_components, names, seen);
            }
        }
        Expression::StaticMemberExpression(member) => {
            if let Expression::CallExpression(inner) = &member.object {
                collect_call_expression_component_names(inner, custom_components, names, seen);
            }
        }
        _ => {}
    }
}

fn push_component_name(
    name: &str,
    custom_components: &HashSet<String>,
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if custom_components.contains(name) {
        return;
    }
    if seen.insert(name.to_string()) {
        names.push(name.to_string());
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

fn component_update_prop_names(statement: &StructStatement<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| matches!(decorator_name(decorator), "Prop" | "ObjectLink"))
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn component_link_prop_names(statement: &StructStatement<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Link")
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn component_info(statement: &StructStatement<'_>) -> CustomComponentInfo {
    CustomComponentInfo {
        update_props: component_update_prop_names(statement),
        link_props: component_link_prop_names(statement),
        reusable: statement
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Reusable"),
    }
}

fn collect_struct_builder_method_names(statement: &StructStatement<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for element in &statement.body.body {
        let StructElement::MethodDefinition(method) = element else {
            continue;
        };
        if !method
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "Builder")
        {
            continue;
        }
        if let Some(name) = method.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn collect_struct_builder_param_names(statement: &StructStatement<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        if !property
            .decorators
            .iter()
            .any(|decorator| decorator_name(decorator) == "BuilderParam")
        {
            continue;
        }
        if let Some(name) = property.key.static_name() {
            names.insert(name.into_owned());
        }
    }
    names
}

fn append_builder_parent_param(params: String) -> String {
    if params.trim().is_empty() {
        "parent = null".to_string()
    } else {
        format!("{params}, parent = null")
    }
}

fn member_text_without_decorators(
    source_text: &str,
    member_span: Span,
    decorators: &[Decorator<'_>],
) -> String {
    if decorators.is_empty() {
        return span_text(member_span, source_text).to_string();
    }

    let start = member_span.start as usize;
    let end = member_span.end as usize;
    let mut ranges = decorators
        .iter()
        .filter_map(|decorator| {
            let mut remove_start = decorator.span.start as usize;
            let mut remove_end = decorator.span.end as usize;
            if remove_end <= start || remove_start >= end {
                return None;
            }

            let line_start = source_text[..remove_start]
                .rfind('\n')
                .map(|index| index + 1)
                .unwrap_or(0);
            if source_text[line_start..remove_start].trim().is_empty() {
                remove_start = line_start;
            }

            let after = &source_text[remove_end..end];
            let whitespace_len = after
                .char_indices()
                .take_while(|(_, ch)| ch.is_whitespace())
                .last()
                .map_or(0, |(index, ch)| index + ch.len_utf8());
            remove_end += whitespace_len;

            Some((remove_start.max(start), remove_end.min(end)))
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();

    let mut out = String::with_capacity(end - start);
    let mut cursor = start;
    for (remove_start, remove_end) in ranges {
        if remove_start > cursor {
            out.push_str(&source_text[cursor..remove_start]);
        }
        cursor = cursor.max(remove_end);
    }
    if cursor < end {
        out.push_str(&source_text[cursor..end]);
    }
    out
}

fn balanced_method_text(
    method: &oxc_ast::ast::MethodDefinition<'_>,
    _struct_span: Span,
    source_text: &str,
) -> Option<(usize, String)> {
    let key_start = method.key.span().start as usize;
    let start = method_source_line_start(key_start, source_text)?;
    balanced_member_text(start, source_text).map(|text| (start, text))
}

fn method_source_line_start(key_start: usize, source_text: &str) -> Option<usize> {
    source_text.get(..key_start)?;
    let line_start = source_text[..key_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = source_text.get(line_start..key_start)?;
    let leading_ws = prefix.len() - prefix.trim_start().len();
    let trimmed = prefix.trim();
    if trimmed.is_empty() || is_method_modifier_prefix(trimmed) {
        Some(line_start + leading_ws)
    } else {
        Some(key_start)
    }
}

fn is_method_modifier_prefix(prefix: &str) -> bool {
    prefix.split_whitespace().all(|token| {
        matches!(
            token,
            "async" | "public" | "private" | "protected" | "static" | "override" | "get" | "set"
        )
    })
}

fn balanced_member_text(start: usize, source_text: &str) -> Option<String> {
    let rest = source_text.get(start..)?;
    let open_rel = rest.find('{')?;
    let open = start + open_rel;
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for (offset, ch) in source_text.get(open..)?.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => in_string = Some(ch),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = open + offset + ch.len_utf8();
                    return source_text.get(start..end).map(str::to_string);
                }
            }
            _ => {}
        }
    }
    None
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

/// Transform a struct-annotated `.ets` file to ViewPU TypeScript and produce a
/// cross-transform source map.
///
/// Unlike `transform_ets_with_options`, which returns only the ViewPU code, this
/// variant also builds a Source Map v3 JSON string that maps generated ViewPU
/// line positions back to the original `.ets` source positions.  It is used by
/// the plugin for Case B (struct-transformed) files so the `sourceMaps.map`
/// rolldown writes references the original `.ets` lines instead of self-referencing
/// the ViewPU output.
///
/// Returns `(viewpu_code, Some(source_map_json))` when at least one component was
/// transformed, `(original_code, None)` when the file contains no `@Component`
/// struct.
struct StructResult {
    viewpu_code: String,
    span_hints: Vec<crate::transform::build_fn::SpanHint>,
    static_span_hints: Vec<crate::transform::build_fn::SpanHint>,
    /// Byte range in the pre-finalized `out` string where this struct's ViewPU code starts.
    out_offset: usize,
    class_line_in_viewpu: usize,
    class_line_text: String,
}

pub fn transform_ets_viewpu_with_source_map(
    source: &ModuleSource,
    options: &TransformOptions,
    ets_source: &str,
) -> Result<(String, Option<String>), EtsTransformError> {
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
        return Ok((content.clone(), None));
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
    let mut struct_results: Vec<StructResult> = Vec::new();
    let mut transformed_any = false;

    for statement in &parsed.program.body {
        let span = statement.span();
        if let Some((component, struct_statement)) = component_with_struct_from_statement(
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
            let out_offset = out.len();
            let span_hints = component.initial_render_span_hints.clone();
            let viewpu_code = generate_view_pu_class(&component);
            let (class_line_in_viewpu, class_line_text) =
                viewpu_class_line_marker(&viewpu_code).unwrap_or((0, String::new()));
            let static_span_hints = collect_component_static_span_hints(
                &component,
                struct_statement,
                content,
                &viewpu_code,
            );
            out.push_str(&viewpu_code);
            struct_results.push(StructResult {
                viewpu_code,
                span_hints,
                static_span_hints,
                out_offset,
                class_line_in_viewpu,
                class_line_text,
            });
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

    if struct_results.is_empty() {
        return if transformed_any {
            Ok((out, None))
        } else {
            Ok((content.clone(), None))
        };
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out = finalize_viewpu_output(&out);

    // Build cross-transform source map entries.  For each struct's span hints,
    // compute the absolute ViewPU line number of the `initialRender()` body and
    // translate the relative body-line offsets to absolute output lines.
    let mut entries: Vec<MapEntry> = Vec::new();

    for sr in &struct_results {
        // Line offset of this struct's ViewPU code within the full output.
        let struct_line_start = final_struct_line_start(&out, sr)
            .unwrap_or_else(|| out[..sr.out_offset].bytes().filter(|&b| b == b'\n').count());

        for hint in &sr.static_span_hints {
            let abs_gen_line = struct_line_start + hint.body_line;
            let (src_line, src_col) = offset_to_line_col(ets_source, hint.original_span.start);
            entries.push(MapEntry {
                generated_line: abs_gen_line,
                generated_col: hint.body_col,
                src_line,
                src_col,
            });
        }

        // Find the `initialRender()` method declaration line within the ViewPU code
        // so we can locate the body start.
        let initial_render_line_in_struct = sr
            .viewpu_code
            .lines()
            .enumerate()
            .find(|(_, line)| line.trim_start().starts_with("initialRender()"))
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Body starts on the line after the declaration.
        let body_start_line = struct_line_start + initial_render_line_in_struct + 1;

        for hint in &sr.span_hints {
            let abs_gen_line = body_start_line + hint.body_line;
            let (src_line, src_col) = offset_to_line_col(ets_source, hint.original_span.start);
            entries.push(MapEntry {
                generated_line: abs_gen_line,
                generated_col: hint.body_col,
                src_line,
                src_col,
            });
        }
    }

    let total_lines = out.lines().count();
    let source_map = build_ets_source_map(&source.id.id, total_lines.max(1), &entries);

    Ok((out, Some(source_map)))
}

fn viewpu_class_line_marker(viewpu_code: &str) -> Option<(usize, String)> {
    viewpu_code.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        if trimmed.starts_with("class ") && trimmed.contains(" extends ViewPU") {
            Some((index, trimmed.to_string()))
        } else {
            None
        }
    })
}

fn final_struct_line_start(out: &str, sr: &StructResult) -> Option<usize> {
    if sr.class_line_text.is_empty() {
        return None;
    }
    out.lines().enumerate().find_map(|(index, line)| {
        (line.trim_start() == sr.class_line_text)
            .then_some(index.saturating_sub(sr.class_line_in_viewpu))
    })
}

fn component_with_struct_from_statement<'a>(
    statement: &'a Statement<'a>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
    simple_type_context: &SimpleTypeContext,
    custom_components: &HashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &HashSet<String>,
    hvigor_source_id: &str,
) -> Result<Option<(ComponentInfo, &'a StructStatement<'a>)>, EtsTransformError> {
    match statement {
        Statement::StructStatement(statement) => Ok(component_from_struct(
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
        )?
        .map(|component| (component, statement.as_ref()))),
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::StructStatement(statement)) => Ok(component_from_struct(
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
            )?
            .map(|component| (component, statement.as_ref()))),
            _ => Ok(None),
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::StructStatement(statement) => Ok(component_from_struct(
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
            )?
            .map(|component| (component, statement.as_ref()))),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn collect_component_static_span_hints(
    component: &ComponentInfo,
    statement: &StructStatement<'_>,
    source_text: &str,
    viewpu_code: &str,
) -> Vec<crate::transform::build_fn::SpanHint> {
    let mut hints = Vec::new();
    let lines: Vec<&str> = viewpu_code.lines().collect();

    if let Some(line) = find_line(&lines, |line| line.trim_start().starts_with("class ")) {
        push_hint_for_text(&mut hints, &lines, line, &component.name, statement.id.span);
    }

    if let Some(line) = find_line(&lines, |line| line.trim_start().starts_with("constructor(")) {
        push_hint(
            &mut hints,
            line,
            leading_spaces(lines[line]),
            statement.span,
        );
    }

    if let Some(line) = find_line(&lines, |line| line.contains("this.finalizeConstruction()"))
        .and_then(|line| find_next_line(&lines, line + 1, |line| line.trim() == "}"))
    {
        push_hint(
            &mut hints,
            line,
            lines[line].trim_end().len(),
            statement.span,
        );
    }

    if let Some(line) = find_line(&lines, |line| {
        line.trim_start().starts_with("initialRender()")
    }) {
        if let Some(build_span) = build_method_span(statement) {
            push_hint(&mut hints, line, leading_spaces(lines[line]), build_span);
            if let Some(close_line) = find_next_line(&lines, line + 1, |line| line.trim() == "}") {
                push_hint(
                    &mut hints,
                    close_line,
                    lines[close_line].trim_end().len(),
                    end_span(build_span),
                );
            }
        }
    }

    for element in &statement.body.body {
        let StructElement::PropertyDefinition(property) = element else {
            continue;
        };
        let Some(name) = property.key.static_name() else {
            continue;
        };
        if !component
            .state_fields
            .iter()
            .any(|field| field.name == name)
        {
            continue;
        }
        let name = name.as_ref();
        let name_span = property.key.span();
        let type_span = property
            .type_annotation
            .as_ref()
            .map(|annotation| annotation.span());
        let init_span = property.value.as_ref().map(|value| value.span());

        if let Some(line) = find_line(&lines, |line| line.trim_start().starts_with(name)) {
            push_hint_for_text(&mut hints, &lines, line, name, name_span);
            if let Some(type_span) = type_span {
                push_hint_for_text(
                    &mut hints,
                    &lines,
                    line,
                    "string",
                    trim_type_span(type_span),
                );
            }
        }
        if let Some(line) = find_line(&lines, |line| {
            line.contains(&format!("this.__{name} = new "))
        }) {
            if let Some(init_span) = init_span {
                let initializer = span_text(init_span, source_text).trim();
                push_hint_for_text(&mut hints, &lines, line, initializer, init_span);
            }
        }
        if let Some(line) = find_line(&lines, |line| line.contains(&format!("private __{name}:"))) {
            push_hint(
                &mut hints,
                line,
                leading_spaces(lines[line]),
                property.span(),
            );
            if let Some(type_span) = type_span {
                push_hint_for_text(
                    &mut hints,
                    &lines,
                    line,
                    "string",
                    trim_type_span(type_span),
                );
            }
            push_hint(
                &mut hints,
                line,
                lines[line].trim_end().len(),
                end_span(property.span()),
            );
        }
        if let Some(line) = find_line(&lines, |line| {
            line.trim_start().starts_with(&format!("get {name}("))
        }) {
            push_hint_for_text(&mut hints, &lines, line, name, name_span);
        }
        if let Some(line) = find_line(&lines, |line| {
            line.trim_start().starts_with(&format!("set {name}("))
        }) {
            push_hint_for_text(&mut hints, &lines, line, name, name_span);
            if let Some(type_span) = type_span {
                push_hint_for_text(
                    &mut hints,
                    &lines,
                    line,
                    "string",
                    trim_type_span(type_span),
                );
            }
        }
    }

    hints
}

fn find_line(lines: &[&str], pred: impl Fn(&str) -> bool) -> Option<usize> {
    lines.iter().position(|line| pred(line))
}

fn find_next_line(lines: &[&str], start: usize, pred: impl Fn(&str) -> bool) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| pred(line).then_some(index))
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn push_hint(
    hints: &mut Vec<crate::transform::build_fn::SpanHint>,
    body_line: usize,
    body_col: usize,
    original_span: Span,
) {
    hints.push(crate::transform::build_fn::SpanHint {
        body_line,
        body_col: body_col as u32,
        original_span,
    });
}

fn push_hint_for_text(
    hints: &mut Vec<crate::transform::build_fn::SpanHint>,
    lines: &[&str],
    body_line: usize,
    text: &str,
    original_span: Span,
) {
    if let Some(body_col) = lines[body_line].find(text) {
        push_hint(hints, body_line, body_col, original_span);
        push_hint(
            hints,
            body_line,
            body_col + text.len(),
            end_span(original_span),
        );
    }
}

fn end_span(span: Span) -> Span {
    Span::new(span.end, span.end)
}

fn trim_type_span(span: Span) -> Span {
    span
}

fn build_method_span(statement: &StructStatement<'_>) -> Option<Span> {
    statement.body.body.iter().find_map(|element| {
        let StructElement::MethodDefinition(method) = element else {
            return None;
        };
        method
            .key
            .static_name()
            .is_some_and(|name| name == "build")
            .then_some(method.span())
    })
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

fn decorator_name<'a>(decorator: &'a Decorator<'a>) -> &'a str {
    match &decorator.expression {
        Expression::Identifier(identifier) => identifier.name.as_str(),
        Expression::CallExpression(call) => match &call.callee {
            Expression::Identifier(identifier) => identifier.name.as_str(),
            _ => "",
        },
        _ => "",
    }
}

/// Extract the first string-literal argument from a decorator call expression.
///
/// Used to extract the handler name from `@Watch("handler")` and the alias
/// from `@Provide("alias")` / `@Consume("alias")`.  Returns `None` if the
/// decorator has no arguments or the first argument is not a string literal.
fn decorator_string_arg(decorator: &Decorator<'_>) -> Option<String> {
    if let Expression::CallExpression(call) = &decorator.expression {
        if let Some(arg) = call.arguments.first() {
            if let Some(expr) = arg.as_expression() {
                if let Expression::StringLiteral(lit) = expr {
                    return Some(lit.value.to_string());
                }
            }
        }
    }
    None
}

fn decorator_arg_text(decorator: &Decorator<'_>, source_text: &str) -> Option<String> {
    if let Expression::CallExpression(call) = &decorator.expression {
        let arg = call.arguments.first()?;
        let expr = arg.as_expression()?;
        let raw = span_text(expr.span(), source_text).trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw.to_string())
        }
    } else {
        None
    }
}

#[derive(Debug, Default)]
struct SimpleTypeContext {
    enum_names: HashSet<String>,
}

impl SimpleTypeContext {
    /// Mirrors ets-loader `process_component_member.js`: primitive annotations
    /// are simple, and `TSTypeReference` is simple only when its identifier is
    /// present in the enum collection populated from local/imported AST exports.
    fn from_program(program: &Program<'_>, id: &str, options: &TransformOptions) -> Self {
        let mut enum_names = local_enum_names(program);
        let importer = Path::new(id);

        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let Some(target) =
                resolve_import_target(import.source.value.as_str(), importer, options)
            else {
                continue;
            };
            let target_enums = exported_enum_names(&target, &mut HashSet::new());
            if target_enums.is_empty() {
                continue;
            }

            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = module_export_name_value(&specifier.imported);
                if target_enums.contains(&imported) {
                    enum_names.insert(specifier.local.name.to_string());
                }
            }
        }

        Self { enum_names }
    }

    fn is_simple_type_annotation(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSStringKeyword(_)
            | TSType::TSNumberKeyword(_)
            | TSType::TSBooleanKeyword(_) => true,
            TSType::TSLiteralType(literal) => matches!(
                &literal.literal,
                TSLiteral::StringLiteral(_)
                    | TSLiteral::NumericLiteral(_)
                    | TSLiteral::BooleanLiteral(_)
            ),
            TSType::TSTypeReference(reference) => match &reference.type_name {
                TSTypeName::IdentifierReference(identifier) => {
                    self.enum_names.contains(identifier.name.as_str())
                }
                _ => false,
            },
            TSType::TSUnionType(union) => union
                .types
                .iter()
                .all(|ty| self.is_simple_type_annotation(ty)),
            TSType::TSParenthesizedType(parenthesized) => {
                self.is_simple_type_annotation(&parenthesized.type_annotation)
            }
            _ => false,
        }
    }
}

fn local_enum_names(program: &Program<'_>) -> HashSet<String> {
    let mut enums = HashSet::new();
    for statement in &program.body {
        match statement {
            Statement::TSEnumDeclaration(declaration) => {
                enums.insert(declaration.id.name.to_string());
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                    enums.insert(declaration.id.name.to_string());
                }
            }
            _ => {}
        }
    }
    enums
}

fn exported_enum_names(path: &Path, visited: &mut HashSet<PathBuf>) -> HashSet<String> {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return HashSet::new();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return HashSet::new();
    }

    let mut exports = HashSet::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                    exports.insert(declaration.id.name.to_string());
                }

                let Some(source) = &export.source else {
                    continue;
                };
                let Some(target) = resolve_relative_import(path.parent(), source.value.as_str())
                else {
                    continue;
                };
                let target_enums = exported_enum_names(&target, visited);
                for specifier in &export.specifiers {
                    let local = module_export_name_value(&specifier.local);
                    if target_enums.contains(&local) {
                        exports.insert(module_export_name_value(&specifier.exported));
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_enum_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

fn exported_builder_function_names(path: &Path, visited: &mut HashSet<PathBuf>) -> HashSet<String> {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return HashSet::new();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return HashSet::new();
    }

    let mut exports = HashSet::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::FunctionDeclaration(function)
                if function_has_decorator(function, "Builder") =>
            {
                if let Some(function_id) = &function.id {
                    exports.insert(function_id.name.to_string());
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::FunctionDeclaration(function)) = &export.declaration {
                    if function_has_decorator(function, "Builder")
                        || export
                            .decorators
                            .iter()
                            .any(|d| decorator_name(d) == "Builder")
                    {
                        if let Some(function_id) = &function.id {
                            exports.insert(function_id.name.to_string());
                        }
                    }
                }

                let Some(source) = &export.source else {
                    continue;
                };
                let Some(target) = resolve_relative_import(path.parent(), source.value.as_str())
                else {
                    continue;
                };
                let target_builders = exported_builder_function_names(&target, visited);
                for specifier in &export.specifiers {
                    let local = module_export_name_value(&specifier.local);
                    if target_builders.contains(&local) {
                        exports.insert(module_export_name_value(&specifier.exported));
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_builder_function_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

#[derive(Default)]
struct ExportedComponentNames {
    named: HashMap<String, CustomComponentInfo>,
    has_default: bool,
    default_info: CustomComponentInfo,
}

impl ExportedComponentNames {
    fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.has_default
    }

    fn extend(&mut self, other: ExportedComponentNames) {
        self.named.extend(other.named);
        if other.has_default {
            self.has_default = true;
            self.default_info = other.default_info;
        }
    }
}

fn exported_component_names(path: &Path, visited: &mut HashSet<PathBuf>) -> ExportedComponentNames {
    let path = normalize_existing_path(path);
    if !visited.insert(path.clone()) {
        return ExportedComponentNames::default();
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return ExportedComponentNames::default();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&path).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, &content, source_type).parse();
    if !parsed.errors.is_empty() {
        return ExportedComponentNames::default();
    }

    let mut local_components = CustomComponentUpdateProps::new();
    for statement in &parsed.program.body {
        collect_local_component_exports(statement, &mut local_components);
    }

    let mut exports = ExportedComponentNames::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                    if is_component_struct(statement) {
                        exports
                            .named
                            .insert(statement.id.name.to_string(), component_info(statement));
                    }
                    continue;
                }

                if let Some(source) = &export.source {
                    let Some(target) =
                        resolve_relative_import(path.parent(), source.value.as_str())
                    else {
                        continue;
                    };
                    let target_components = exported_component_names(&target, visited);
                    for specifier in &export.specifiers {
                        let local = module_export_name_value(&specifier.local);
                        let exported = module_export_name_value(&specifier.exported);
                        let props = if local == "default" && target_components.has_default {
                            Some(&target_components.default_info)
                        } else {
                            target_components.named.get(&local)
                        };
                        if exported == "default" && props.is_some() {
                            exports.has_default = true;
                            exports.default_info = props.cloned().unwrap_or_default();
                        } else if let Some(props) = props {
                            exports.named.insert(exported, props.clone());
                        }
                    }
                } else {
                    for specifier in &export.specifiers {
                        let local = module_export_name_value(&specifier.local);
                        let exported = module_export_name_value(&specifier.exported);
                        if exported == "default" && local_components.contains_key(&local) {
                            exports.has_default = true;
                            exports.default_info =
                                local_components.get(&local).cloned().unwrap_or_default();
                        } else if let Some(props) = local_components.get(&local) {
                            exports.named.insert(exported, props.clone());
                        }
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::StructStatement(statement) =
                    &export.declaration
                {
                    if is_component_struct(statement) {
                        exports.has_default = true;
                        exports.default_info = component_info(statement);
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let Some(target) =
                    resolve_relative_import(path.parent(), export.source.value.as_str())
                else {
                    continue;
                };
                exports.extend(exported_component_names(&target, visited));
            }
            _ => {}
        }
    }

    exports
}

fn collect_local_component_exports(
    statement: &Statement<'_>,
    names: &mut CustomComponentUpdateProps,
) {
    match statement {
        Statement::StructStatement(statement) if is_component_struct(statement) => {
            names.insert(statement.id.name.to_string(), component_info(statement));
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::StructStatement(statement)) = export.declaration.as_ref() {
                if is_component_struct(statement) {
                    names.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::StructStatement(statement) = &export.declaration {
                if is_component_struct(statement) {
                    names.insert(statement.id.name.to_string(), component_info(statement));
                }
            }
        }
        _ => {}
    }
}

fn resolve_import_target(
    specifier: &str,
    importer: &Path,
    options: &TransformOptions,
) -> Option<PathBuf> {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        return resolve_relative_import(importer.parent(), specifier);
    }

    let package_name = specifier
        .strip_prefix("@ohos/")
        .or_else(|| specifier.strip_prefix("@ohos:"))
        .unwrap_or(specifier)
        .split('/')
        .next()
        .unwrap_or(specifier);

    options.module_roots.iter().find_map(|(name, root)| {
        (name == package_name || specifier.ends_with(name))
            .then(|| module_entry_path(root))
            .flatten()
    })
}

fn module_entry_path(root: &Path) -> Option<PathBuf> {
    [
        root.join("Index.ets"),
        root.join("index.ets"),
        root.join("Index.ts"),
        root.join("index.ts"),
        root.join("src/main/ets/Index.ets"),
        root.join("src/main/ets/index.ets"),
        root.join("src/main/ets/Index.ts"),
        root.join("src/main/ets/index.ts"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn resolve_relative_import(base: Option<&Path>, specifier: &str) -> Option<PathBuf> {
    let candidate = base?.join(specifier);
    [
        candidate.clone(),
        candidate.with_extension("ets"),
        candidate.with_extension("ts"),
        candidate.with_extension("d.ets"),
        candidate.with_extension("d.ts"),
        candidate.join("Index.ets"),
        candidate.join("index.ets"),
        candidate.join("Index.ts"),
        candidate.join("index.ts"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn module_export_name_value(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

fn type_annotation_text(span: Span, source_text: &str) -> String {
    span_text(span, source_text)
        .trim()
        .trim_start_matches(':')
        .trim()
        .to_string()
}

fn span_text(span: Span, source_text: &str) -> &str {
    &source_text[span]
}

/// Extract the parameter list text from a `FormalParameters` span, stripped of
/// the surrounding parentheses.
///
/// For `(text: string, count: number)`, returns `"text: string, count: number"`.
/// For `()`, returns `""`.
fn extract_params_text(params_span: Span, source_text: &str) -> String {
    span_text(params_span, source_text)
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests;
