pub mod build_fn;
pub mod builder;
pub mod component;
pub mod decorator;
pub mod entry;

use arkdown_core::ModuleSource;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, Decorator, Expression, Statement, StructElement, StructStatement};
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
    if !content.contains("@Component") && !content.contains("@Entry") {
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

    for statement in &parsed.program.body {
        let span = statement.span();
        out.push_str(&content[last_end..span.start as usize]);

        if let Some(component) =
            component_from_statement(statement, content, &source.id.id, options)?
        {
            out.push_str(&generate_view_pu_class(&component));
            transformed_any = true;
        } else {
            out.push_str(span_text(span, content));
        }

        last_end = span.end as usize;
    }

    out.push_str(&content[last_end..]);

    if transformed_any {
        while out.ends_with("\n\n") {
            out.pop();
        }
        Ok(out)
    } else {
        Ok(content.clone())
    }
}

fn component_from_statement(
    statement: &Statement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    match statement {
        Statement::StructStatement(statement) => {
            component_from_struct(statement, source_text, id, options)
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::StructStatement(statement)) => {
                component_from_struct(statement, source_text, id, options)
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn component_from_struct(
    statement: &StructStatement<'_>,
    source_text: &str,
    id: &str,
    options: &TransformOptions,
) -> Result<Option<ComponentInfo>, EtsTransformError> {
    let has_component = statement
        .decorators
        .iter()
        .any(|decorator| decorator_name(decorator) == "Component");
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

    for element in &statement.body.body {
        match element {
            StructElement::PropertyDefinition(property) => {
                let Some(decorator) = property
                    .decorators
                    .iter()
                    .map(decorator_name)
                    .find(|name| is_state_decorator(name))
                else {
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
                    .find(|d| matches!(decorator_name(d), "Provide" | "Consume"))
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
            _ => {}
        }
    }

    Ok(Some(ComponentInfo {
        name: statement.id.name.to_string(),
        is_entry,
        state_fields,
        initial_render_body,
        initial_render_span_hints,
        entry_registration: is_entry.then(|| entry_registration(id, options)),
        builder_methods,
        styles_methods,
    }))
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

    if !content.contains("@Component") && !content.contains("@Entry") {
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
        /// Byte range in the `out` string where this struct's ViewPU code starts.
        out_offset: usize,
    }

    let mut out = String::with_capacity(content.len() + 512);
    let mut last_end = 0usize;
    let mut struct_results: Vec<StructResult> = Vec::new();

    for statement in &parsed.program.body {
        let span = statement.span();
        out.push_str(&content[last_end..span.start as usize]);

        if let Some(component) =
            component_from_statement(statement, content, &source.id.id, options)?
        {
            let out_offset = out.len();
            let span_hints = component.initial_render_span_hints.clone();
            let viewpu_code = generate_view_pu_class(&component);
            out.push_str(&viewpu_code);
            struct_results.push(StructResult {
                viewpu_code,
                span_hints,
                out_offset,
            });
        } else {
            out.push_str(span_text(span, content));
        }

        last_end = span.end as usize;
    }
    out.push_str(&content[last_end..]);

    if struct_results.is_empty() {
        return Ok((content.clone(), None));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }

    // Build cross-transform source map entries.  For each struct's span hints,
    // compute the absolute ViewPU line number of the `initialRender()` body and
    // translate the relative body-line offsets to absolute output lines.
    let mut entries: Vec<MapEntry> = Vec::new();

    for sr in &struct_results {
        // Line offset of this struct's ViewPU code within the full output.
        let struct_line_start = out[..sr.out_offset].bytes().filter(|&b| b == b'\n').count();

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
                src_line,
                src_col,
            });
        }
    }

    let total_lines = out.lines().count();
    let source_map = build_ets_source_map(&source.id.id, total_lines.max(1), &entries);

    Ok((out, Some(source_map)))
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
