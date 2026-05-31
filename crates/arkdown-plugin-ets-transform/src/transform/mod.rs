pub mod build_fn;
pub mod builder;
pub mod component;
pub mod decorator;
pub mod entry;

use arkdown_core::ModuleSource;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AnnotationDeclaration, AnnotationElement, Declaration, Decorator, ExportDefaultDeclarationKind,
    Expression, Function, Statement, StructElement, StructStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::error::EtsTransformError;
use crate::source_map::{build_ets_source_map, offset_to_line_col, MapEntry};
use crate::transform::build_fn::{lower_build_body, lower_build_body_or_raw, lower_styles_body};
use crate::transform::component::{
    generate_view_pu_class, BuilderMethodInfo, ComponentInfo, StateField, StylesMethodInfo,
};
use crate::transform::decorator::is_state_decorator;
use crate::transform::entry::EntryRegistrationInfo;

#[derive(Debug, Clone, Default)]
pub struct TransformOptions {
    pub bundle_name: Option<String>,
    pub module_name: Option<String>,
}

/// Transform an ArkTS `.ets` source file into TypeScript.
///
/// ArkUI parsing is handled by the workspace-pinned ohos-rs/oxc fork using
/// `SourceType::ets()`. This lets us extract real `StructStatement` and
/// `ArkUIComponentExpression` nodes instead of guessing from text.
pub fn transform_ets(source: &ModuleSource) -> Result<String, EtsTransformError> {
    transform_ets_with_options(source, &TransformOptions::default())
}

pub fn transform_ets_with_options(
    source: &ModuleSource,
    options: &TransformOptions,
) -> Result<String, EtsTransformError> {
    let content = &source.content;

    // Fast path: files with no ArkUI-specific syntax pass through unchanged.
    // The module_type change from Ets → Ts still happens in the plugin.
    if !has_arkts_transform_syntax(content) {
        return Ok(content.clone());
    }

    let diagnostics = crate::checker::check(content, &source.id.id);
    for diag in &diagnostics {
        eprintln!("[ets-checker] {:?}", diag);
    }

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&source.id.id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(EtsTransformError::Fatal {
            file: source.id.id.clone(),
            message: format!("ArkUI parse failed: {:?}", parsed.errors),
        });
    }

    let mut out = String::with_capacity(content.len() + 512);
    let mut last_end = 0usize;
    let mut transformed_any = false;
    let mut pending_function_decorator: Option<PendingFunctionDecorator> = None;

    for statement in &parsed.program.body {
        let span = statement.span();
        if let Some(decorator) = pending_function_decorator_from_statement(statement, content) {
            out.push_str(&content[last_end..span.start as usize]);
            pending_function_decorator = Some(decorator);
            transformed_any = true;
            last_end = span.end as usize;
            continue;
        }

        let pending_decorator = pending_function_decorator.take();

        if let Some(component) =
            component_from_statement(statement, content, &source.id.id, options)?
        {
            let start = component_replacement_start(statement, content);
            out.push_str(&content[last_end..start]);
            out.push_str(&generate_view_pu_class(&component));
            transformed_any = true;
        } else if let Some((start, replacement)) =
            transform_non_component_statement(statement, content, pending_decorator.as_ref())
        {
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
        Ok(strip_orphan_arkui_decorator_lines(&out))
    } else {
        Ok(content.clone())
    }
}

fn strip_orphan_arkui_decorator_lines(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim();
        if is_known_arkui_decorator_line(trimmed) {
            continue;
        }
        if let Some(stripped) = strip_known_arkui_decorator_prefix(line) {
            out.push_str(&stripped);
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn strip_known_arkui_decorator_prefix(line: &str) -> Option<String> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, trimmed) = line.split_at(indent_len);
    if !trimmed.starts_with('@') {
        return None;
    }

    for name in KNOWN_ARKUI_DECORATORS {
        let bare = format!("@{name}");
        if let Some(rest) = trimmed.strip_prefix(&bare) {
            if rest.starts_with(char::is_whitespace) {
                return Some(format!("{indent}{}", rest.trim_start()));
            }
        }

        let call = format!("@{name}(");
        if trimmed.starts_with(&call) {
            let close = trimmed.find(')')?;
            let rest = trimmed[close + 1..].trim_start();
            if !rest.is_empty() {
                return Some(format!("{indent}{rest}"));
            }
        }
    }

    None
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

fn is_known_arkui_decorator_line(trimmed: &str) -> bool {
    if KNOWN_ARKUI_DECORATORS
        .iter()
        .any(|name| trimmed == format!("@{name}"))
    {
        return true;
    }

    KNOWN_ARKUI_DECORATORS.iter().any(|name| {
        let prefix = format!("@{name}(");
        trimmed.starts_with(&prefix) && trimmed.ends_with(')')
    })
}

pub(crate) fn has_arkts_transform_syntax(content: &str) -> bool {
    [
        "@Component",
        "@Entry",
        "@CustomDialog",
        "@ComponentV2",
        "@Builder",
        "@LocalBuilder",
        "@Styles",
        "@Extend",
        "@AnimatableExtend",
        "@interface",
        "@Local",
        "@Param",
        "@Once",
        "@Provider",
        "@Consumer",
        "@Event",
        "@Env",
        "@Require",
        "@PropRef",
        "@StoragePropRef",
        "@LocalStoragePropRef",
        "@Trace",
        "@Computed",
        "@Monitor",
        "@SyncMonitor",
        "@ComponentInit",
        "@ComponentAppear",
        "@ComponentBuilt",
        "@ComponentAttach",
        "@ComponentDetach",
        "@ComponentReuse",
        "@ComponentRecycle",
        "@ComponentDisappear",
        "@ComponentActive",
        "@ComponentInactive",
        "import lazy",
        "struct ",
    ]
    .iter()
    .any(|needle| content.contains(needle))
}

fn component_from_statement(
    statement: &Statement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    match statement {
        Statement::StructStatement(statement) => {
            component_from_struct(statement, source_text, id, options, false, false)
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::StructStatement(statement)) => {
                component_from_struct(statement, source_text, id, options, true, false)
            }
            _ => Ok(None),
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::StructStatement(statement) => {
                component_from_struct(statement, source_text, id, options, true, true)
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn transform_non_component_statement(
    statement: &Statement<'_>,
    source_text: &str,
    pending_decorator: Option<&PendingFunctionDecorator>,
) -> Option<(usize, String)> {
    match statement {
        _ if is_orphan_arkui_decorator_statement(statement, source_text) => {
            Some((statement.span().start as usize, String::new()))
        }
        Statement::AnnotationDeclaration(annotation) => Some((
            annotation.span.start as usize,
            annotation_to_interface(annotation, source_text),
        )),
        Statement::LazyImportDeclaration(_) => {
            let text =
                span_text(statement.span(), source_text).replacen("import lazy", "import", 1);
            Some((statement.span().start as usize, text))
        }
        Statement::FunctionDeclaration(function) => transform_decorated_function_with_pending(
            function,
            source_text,
            false,
            &[],
            pending_decorator,
        ),
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::FunctionDeclaration(function)) => {
                let mut result = transform_decorated_function_with_extra_decorators(
                    function,
                    source_text,
                    true,
                    &export.decorators,
                    pending_decorator,
                )?;
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
    }
}

fn is_orphan_arkui_decorator_statement(statement: &Statement<'_>, source_text: &str) -> bool {
    let text = span_text(statement.span(), source_text).trim();
    matches!(
        text,
        "@Component" | "@Entry" | "@CustomDialog" | "@Reusable" | "@ComponentV2" | "@ReusableV2"
    )
}

#[derive(Debug, Clone)]
struct PendingFunctionDecorator {
    kind: String,
    first_arg: Option<String>,
}

fn pending_function_decorator_from_statement(
    statement: &Statement<'_>,
    source_text: &str,
) -> Option<PendingFunctionDecorator> {
    let text = span_text(statement.span(), source_text).trim();
    let (kind, first_arg) = parse_pending_function_decorator(text)?;
    Some(PendingFunctionDecorator { kind, first_arg })
}

fn parse_pending_function_decorator(text: &str) -> Option<(String, Option<String>)> {
    for kind in [
        "Builder",
        "LocalBuilder",
        "Styles",
        "Extend",
        "AnimatableExtend",
    ] {
        if text == format!("@{kind}") {
            return Some((kind.to_string(), None));
        }
        let prefix = format!("@{kind}(");
        if text.starts_with(&prefix) && text.ends_with(')') {
            let first_arg = text[prefix.len()..text.len() - 1]
                .split(',')
                .next()
                .map(str::trim)
                .filter(|arg| !arg.is_empty())
                .map(str::to_string);
            return Some((kind.to_string(), first_arg));
        }
    }
    None
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

fn transform_decorated_function_with_extra_decorators(
    function: &Function<'_>,
    source_text: &str,
    exported: bool,
    extra_decorators: &[Decorator<'_>],
    pending_decorator: Option<&PendingFunctionDecorator>,
) -> Option<(usize, String)> {
    transform_decorated_function_with_pending(
        function,
        source_text,
        exported,
        extra_decorators,
        pending_decorator,
    )
}

fn transform_decorated_function_with_pending(
    function: &Function<'_>,
    source_text: &str,
    exported: bool,
    extra_decorators: &[Decorator<'_>],
    pending_decorator: Option<&PendingFunctionDecorator>,
) -> Option<(usize, String)> {
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
        .map(str::to_string)
        .or_else(|| pending_decorator.map(|decorator| decorator.kind.clone()))?;
    let name = function.id.as_ref()?.name.as_str();
    let params = extract_params_text(function.params.span(), source_text);
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
                .or_else(|| pending_decorator.and_then(|decorator| decorator.first_arg.clone()))
                .unwrap_or_else(|| "View".to_string());
            return Some((
                start,
                format!(
                    "{export_prefix}function __{component}__{name}({params}): void {{\n{}    }}",
                    lower_extend_body(function.body.as_deref(), source_text, &component)
                ),
            ));
        }
        _ => lower_build_body_or_raw(function.body.as_deref(), source_text).0,
    };

    Some((
        start,
        format!("{export_prefix}function {name}({params}): void {{\n{body}    }}"),
    ))
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
    let inner = body_text
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
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

fn component_from_struct(
    statement: &StructStatement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
    is_exported: bool,
    is_default_export: bool,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    let has_component = statement.decorators.iter().any(|decorator| {
        matches!(
            decorator_name(decorator),
            "Component" | "ComponentV2" | "CustomDialog"
        )
    });
    if !has_component {
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
    let mut styles_methods: Vec<StylesMethodInfo> = Vec::new();
    let mut regular_members: Vec<String> = Vec::new();

    for element in &statement.body.body {
        match element {
            StructElement::PropertyDefinition(property) => {
                let decorator_names: Vec<&str> =
                    property.decorators.iter().map(decorator_name).collect();
                let Some(decorator) = primary_state_decorator(&decorator_names) else {
                    regular_members.push(strip_orphan_arkui_decorator_lines(
                        span_text(element.span(), source_text).trim(),
                    ));
                    continue;
                };
                let Some(name) = property.key.static_name() else {
                    continue;
                };
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
                    .and_then(|d| decorator_string_arg(d, source_text));
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
                    .and_then(|d| decorator_string_arg(d, source_text));
                state_fields.push(StateField {
                    name: name.into_owned(),
                    type_annotation,
                    decorator: decorator.to_string(),
                    initializer,
                    watch_handler,
                    alias,
                });
            }
            StructElement::MethodDefinition(method)
                if method.key.static_name().is_some_and(|name| name == "build") =>
            {
                let (body, hints) = lower_build_body(method.value.body.as_deref(), source_text)
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
                let params = extract_params_text(method.value.params.span(), source_text);
                // Lower the ArkUI DSL body; fall back to raw TS text when the
                // OXC ETS parser did not produce ArkUIComponentExpression nodes
                // (which happens for @Builder bodies — the parser only applies
                // ArkUI expression rules to the canonical build() method).
                let (lowered_body, _) =
                    lower_build_body_or_raw(method.value.body.as_deref(), source_text);
                builder_methods.push(BuilderMethodInfo {
                    name: name.into_owned(),
                    params,
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
            _ => {
                regular_members.push(strip_orphan_arkui_decorator_lines(
                    span_text(element.span(), source_text).trim(),
                ));
            }
        }
    }

    Ok(Some(ComponentInfo {
        name: statement.id.name.to_string(),
        is_entry,
        is_exported,
        is_default_export,
        state_fields,
        regular_members,
        initial_render_body,
        initial_render_span_hints,
        entry_registration: is_entry.then(|| entry_registration(id, options)),
        builder_methods,
        styles_methods,
    }))
}

fn primary_state_decorator(names: &[&str]) -> Option<&'static str> {
    // Mirrors upstream arkui-plugins classifyV1Property/classifyV2Property order.
    // Meta decorators such as @Watch and @Require are intentionally absent.
    [
        "State",
        "StorageLink",
        "LocalStorageLink",
        "Link",
        "ObjectLink",
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
        "Env",
        "Trace",
    ]
    .into_iter()
    .find(|candidate| names.iter().any(|name| name == candidate) && is_state_decorator(candidate))
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
pub fn transform_ets_viewpu_with_source_map(
    source: &ModuleSource,
    options: &TransformOptions,
    ets_source: &str,
) -> Result<(String, Option<String>), EtsTransformError> {
    let content = &source.content;

    if !has_arkts_transform_syntax(content) {
        return Ok((content.clone(), None));
    }

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&source.id.id).unwrap_or_else(|_| SourceType::ets());
    let parsed = Parser::new(&allocator, content, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(EtsTransformError::Fatal {
            file: source.id.id.clone(),
            message: format!("ArkUI parse failed: {:?}", parsed.errors),
        });
    }

    // Collect (viewpu_code, span_hints) per transformed struct so we can
    // compute absolute line numbers after the full output is assembled.
    struct StructResult {
        viewpu_code: String,
        span_hints: Vec<crate::transform::build_fn::SpanHint>,
        static_span_hints: Vec<crate::transform::build_fn::SpanHint>,
        /// Byte range in the `out` string where this struct's ViewPU code starts.
        out_offset: usize,
    }

    let mut out = String::with_capacity(content.len() + 512);
    let mut last_end = 0usize;
    let mut struct_results: Vec<StructResult> = Vec::new();
    let mut transformed_any = false;
    let mut pending_function_decorator: Option<PendingFunctionDecorator> = None;

    for statement in &parsed.program.body {
        let span = statement.span();
        if let Some(decorator) = pending_function_decorator_from_statement(statement, content) {
            out.push_str(&content[last_end..span.start as usize]);
            pending_function_decorator = Some(decorator);
            transformed_any = true;
            last_end = span.end as usize;
            continue;
        }

        let pending_decorator = pending_function_decorator.take();

        if let Some((component, struct_statement)) =
            component_with_struct_from_statement(statement, content, &source.id.id, options)?
        {
            let start = component_replacement_start(statement, content);
            out.push_str(&content[last_end..start]);
            let out_offset = out.len();
            let span_hints = component.initial_render_span_hints.clone();
            let viewpu_code = generate_view_pu_class(&component);
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
            });
            transformed_any = true;
        } else if let Some((start, replacement)) =
            transform_non_component_statement(statement, content, pending_decorator.as_ref())
        {
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
            Ok((strip_orphan_arkui_decorator_lines(&out), None))
        } else {
            Ok((content.clone(), None))
        };
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out = strip_orphan_arkui_decorator_lines(&out);

    // Build cross-transform source map entries.  For each struct's span hints,
    // compute the absolute ViewPU line number of the `initialRender()` body and
    // translate the relative body-line offsets to absolute output lines.
    let mut entries: Vec<MapEntry> = Vec::new();

    for sr in &struct_results {
        // Line offset of this struct's ViewPU code within the full output.
        let struct_line_start = out[..sr.out_offset].bytes().filter(|&b| b == b'\n').count();

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

fn component_with_struct_from_statement<'a>(
    statement: &'a Statement<'a>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
) -> Result<Option<(ComponentInfo, &'a StructStatement<'a>)>, EtsTransformError> {
    match statement {
        Statement::StructStatement(statement) => {
            Ok(
                component_from_struct(statement, source_text, id, options, false, false)?
                    .map(|component| (component, statement.as_ref())),
            )
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::StructStatement(statement)) => {
                Ok(
                    component_from_struct(statement, source_text, id, options, true, false)?
                        .map(|component| (component, statement.as_ref())),
                )
            }
            _ => Ok(None),
        },
        Statement::ExportDefaultDeclaration(export) => {
            match &export.declaration {
                ExportDefaultDeclarationKind::StructStatement(statement) => Ok(
                    component_from_struct(statement, source_text, id, options, true, true)?
                        .map(|component| (component, statement.as_ref())),
                ),
                _ => Ok(None),
            }
        }
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

fn entry_registration(id: &str, options: &TransformOptions) -> EntryRegistrationInfo {
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
    let page_full_path = if id.contains("/src/main/ets/") {
        format!("{module_name}/src/main/ets/{page_path}")
    } else {
        id.trim_end_matches(".ets").to_string()
    };

    EntryRegistrationInfo {
        bundle_name: options.bundle_name.clone().unwrap_or_default(),
        module_name,
        page_path,
        page_full_path,
        integrated_hsp: "false".to_string(),
        module_type: "followWithHap".to_string(),
    }
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
fn decorator_string_arg(decorator: &Decorator<'_>, source_text: &str) -> Option<String> {
    if let Expression::CallExpression(call) = &decorator.expression {
        if let Some(arg) = call.arguments.first() {
            // Use `as_expression()` — the oxc fork uses `inherit_variants!` so
            // `Argument` does not have a direct `Expression` variant.
            if let Some(expr) = arg.as_expression() {
                if let Expression::StringLiteral(lit) = expr {
                    return Some(lit.value.to_string());
                }
                // Fallback: slice the span text and strip surrounding quotes.
                let raw = span_text(expr.span(), source_text).trim().to_string();
                let unquoted = raw
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .trim_start_matches('\'')
                    .trim_end_matches('\'');
                if !unquoted.is_empty() {
                    return Some(unquoted.to_string());
                }
            }
        }
    }
    None
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
mod tests {
    use super::*;
    use arkdown_core::{ModuleType, ResolvedId};

    fn make_source(id: &str, content: &str) -> ModuleSource {
        ModuleSource {
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ets,
                ..Default::default()
            },
            content: content.to_string(),
            source_map: None,
        }
    }

    #[test]
    fn test_non_component_passthrough() {
        let src = "import { something } from '@ohos/hilog';\nconst x = 1;";
        let result = transform_ets(&make_source("test.ets", src)).unwrap();
        assert_eq!(result, src);
    }

    #[test]
    fn test_transforms_component_struct_with_empty_build() {
        let src = "@Component\nstruct MyView {\n  build() {}\n}";
        let result =
            transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
        assert!(result.contains("class MyView extends ViewPU"));
        assert!(result.contains("initialRender()"));
        assert!(!result.contains("struct MyView"));
    }

    #[test]
    fn test_transforms_export_component_without_leaking_decorator() {
        let src = "import { Foo } from './foo';\n\n@Component\nexport struct MyView {\n  @State message: string = \"hi\";\n  build() {}\n}";
        let result =
            transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
        assert!(result.contains("class MyView extends ViewPU"));
        assert!(
            !result.contains("@Component"),
            "component decorator must be consumed with export struct, got:\n{}",
            result
        );
        assert!(!result.contains("export struct MyView"));
    }

    #[test]
    fn test_transforms_default_export_component() {
        let src = "@Component\nexport default struct MyView {\n  build() {}\n}";
        let result =
            transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
        assert!(result.contains("export default class MyView extends ViewPU"));
        assert!(!result.contains("export default struct MyView"));
    }

    #[test]
    fn test_transforms_component_v2_property_decorators() {
        let src = "@ComponentV2\nstruct MyView {\n  @Local count: number = 0;\n  @Param title: string = '';\n  @Provider('theme') theme: string = 'dark';\n  @Consumer('theme') incoming: string = '';\n  @Event onTap: () => void = () => {};\n  @Computed\n  get label(): string { return this.title; }\n  build() {}\n}";
        let result =
            transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
        assert!(result.contains("class MyView extends ViewPU"));
        assert!(result.contains("private __count: ObservedPropertySimplePU<number>"));
        assert!(result.contains("private __title: SynchedPropertySimpleOneWayPU<string>"));
        assert!(
            result.contains("this.__incoming = this.initializeConsume(\"theme\", \"incoming\")")
        );
        assert!(result.contains("onTap: () => void = () => {};"));
        assert!(result.contains("get label(): string { return this.title; }"));
        assert!(!result.contains("@ComponentV2"));
        assert!(!result.contains("@Local"));
        assert!(!result.contains("@Param"));
        assert!(!result.contains("@Provider"));
        assert!(!result.contains("@Consumer"));
        assert!(!result.contains("@Event"));
        assert!(!result.contains("@Computed"));
    }

    #[test]
    fn test_transforms_export_builder_function_decorator() {
        let src = "@Builder\nexport function Card(title: string) {\n  Column() {\n    Text(title)\n  }\n}";
        let result =
            transform_ets(&make_source("entry/src/main/ets/components/Card.ets", src)).unwrap();
        assert!(result.contains("export function Card(title: string): void"));
        assert!(result.contains("observeComponentCreation2"));
        assert!(!result.contains("@Builder"));
        assert!(!result.contains("Column() {"));
    }

    #[test]
    fn test_transforms_builder_before_export_function() {
        let src = "@Builder\nexport function LearningItem(learningItem: LearningPath) {\n  Column() {\n    Image(learningItem.thumbnail)\n      .width(LearningConstants.FULL_PERCENT)\n  }\n  .clip(true)\n}";
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/LearningItem.ets",
            src,
        ))
        .unwrap();
        assert!(!result.contains("@Builder"));
        assert!(!result.contains("Column() {"));
        assert!(result.contains(".clip(true);"));
    }

    #[test]
    fn test_transforms_entry_component_with_hvigor_route_registration() {
        let src = "@Entry\n@Component\nstruct Page {\n  build() {}\n}";
        let options = TransformOptions {
            bundle_name: Some("com.example.arkdown.test".to_string()),
            module_name: Some("entry".to_string()),
        };
        let result = transform_ets_with_options(
            &make_source("entry/src/main/ets/pages/Page.ets", src),
            &options,
        )
        .unwrap();
        assert!(result.contains("class Page extends ViewPU"));
        assert!(result.contains("registerNamedRoute(() => new Page(undefined, {})"));
        assert!(result.contains("bundleName: \"com.example.arkdown.test\""));
        assert!(result.contains("pagePath: \"pages/Page\""));
        assert!(!result.contains("@Entry"));
        assert!(!result.contains("struct Page"));
    }

    #[test]
    fn test_minimal_index_component_lowers_arkui_semantics() {
        let src = include_str!(
            "../../../../tests/fixtures/minimal-hap/entry/src/main/ets/pages/Index.ets"
        );
        let result =
            transform_ets(&make_source("entry/src/main/ets/pages/Index.ets", src)).unwrap();

        assert!(result.contains("class Index extends ViewPU"));
        assert!(result.contains("private __message: ObservedPropertySimplePU<string>"));
        assert!(result.contains("this.observeComponentCreation2((elmtId, isInitialRender) => {"));
        assert!(result.contains("Row.create();"));
        assert!(result.contains("Column.create();"));
        assert!(result.contains("Text.create(this.message);"));
        assert!(result.contains("Text.fontSize(50);"));
        assert!(result.contains("Text.fontWeight(FontWeight.Bold);"));
        assert!(result.contains("Text.pop();"));
        assert!(result.contains("Column.pop();"));
        assert!(result.contains("Row.pop();"));
        assert!(!result.contains("build() { return; }"));
        assert!(!result.contains("TODO: desugar"));

        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            &result,
            oxc_span::SourceType::ts().with_module(true),
        )
        .parse();
        assert!(
            parsed.errors.is_empty(),
            "lowered output must be valid TypeScript: {:?}\n{}",
            parsed.errors,
            result
        );
    }

    // ── @Link integration ─────────────────────────────────────────────────────

    #[test]
    fn test_link_decorator_lowers_to_synched_property_two_way_pu() {
        let src = r#"
@Component
struct ItemList {
  @Link items: string[];
  build() {}
}
"#;
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/ItemList.ets",
            src,
        ))
        .unwrap();
        assert!(
            result.contains("private __items: SynchedPropertySimpleTwoWayPU<string[]>"),
            "expected SynchedPropertySimpleTwoWayPU backing field, got:\n{}",
            result
        );
        assert!(
            result.contains("new SynchedPropertySimpleTwoWayPU(params.items, this, \"items\")"),
            "expected params.items init for @Link, got:\n{}",
            result
        );
        // @Link must NOT appear in updateStateVars
        assert!(
            !result.contains("this.__items.set(params.items)"),
            "@Link must not have an updateStateVars entry, got:\n{}",
            result
        );
    }

    // ── @ObjectLink integration ───────────────────────────────────────────────

    #[test]
    fn test_object_link_decorator_lowers_to_synched_nesed_object_pu() {
        let src = r#"
@Component
struct ItemRow {
  @ObjectLink item: Item;
  build() {}
}
"#;
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/ItemRow.ets",
            src,
        ))
        .unwrap();
        assert!(
            result.contains("private __item: SynchedPropertyNesedObjectPU<Item>"),
            "expected SynchedPropertyNesedObjectPU backing field, got:\n{}",
            result
        );
        assert!(
            result.contains("new SynchedPropertyNesedObjectPU(params.item, this, \"item\")"),
            "expected params.item init for @ObjectLink, got:\n{}",
            result
        );
    }

    // ── @Provide / @Consume integration ───────────────────────────────────────

    #[test]
    fn test_provide_decorator_lowers_to_observed_property_simple_pu() {
        let src = r#"
@Component
struct ThemeProvider {
  @Provide theme: string = "dark";
  build() {}
}
"#;
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/ThemeProvider.ets",
            src,
        ))
        .unwrap();
        assert!(
            result.contains("private __theme: ObservedPropertySimplePU<string>"),
            "expected ObservedPropertySimplePU for @Provide, got:\n{}",
            result
        );
        assert!(
            result.contains("new ObservedPropertySimplePU(\"dark\", this, \"theme\")"),
            "expected local init for @Provide, got:\n{}",
            result
        );
    }

    #[test]
    fn test_consume_decorator_lowers_to_initialize_consume() {
        let src = r#"
@Component
struct ThemedChild {
  @Consume theme: string;
  build() {}
}
"#;
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/ThemedChild.ets",
            src,
        ))
        .unwrap();
        assert!(
            result.contains("private __theme: ObservedPropertyAbstractPU<string>"),
            "expected ObservedPropertyAbstractPU for @Consume, got:\n{}",
            result
        );
        assert!(
            result.contains("this.initializeConsume(\"theme\", \"theme\")"),
            "expected initializeConsume for @Consume, got:\n{}",
            result
        );
    }

    // ── @Watch integration ────────────────────────────────────────────────────

    #[test]
    fn test_watch_decorator_generates_declare_watch_in_constructor() {
        let src = r#"
@Component
struct Counter {
  @State @Watch("onCountChange") count: number = 0;
  onCountChange() {}
  build() {}
}
"#;
        let result = transform_ets(&make_source(
            "entry/src/main/ets/components/Counter.ets",
            src,
        ))
        .unwrap();
        assert!(
            result.contains("this.declareWatch(\"count\", this.onCountChange)"),
            "expected declareWatch for @Watch, got:\n{}",
            result
        );
        // Backing field is still ObservedPropertySimplePU (from @State)
        assert!(
            result.contains("private __count: ObservedPropertySimplePU<number>"),
            "expected ObservedPropertySimplePU backing field, got:\n{}",
            result
        );
    }
}
