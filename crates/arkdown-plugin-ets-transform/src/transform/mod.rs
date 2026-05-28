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
use crate::transform::build_fn::lower_build_body;
use crate::transform::component::{generate_view_pu_class, ComponentInfo, StateField};
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
                state_fields.push(StateField {
                    name: name.into_owned(),
                    type_annotation,
                    decorator: decorator.to_string(),
                    initializer,
                });
            }
            StructElement::MethodDefinition(method)
                if method.key.static_name().is_some_and(|name| name == "build") =>
            {
                initial_render_body = lower_build_body(method.value.body.as_deref(), source_text)
                    .map_err(|err| EtsTransformError::Fatal {
                    file: id.to_string(),
                    message: format!("unsupported ArkUI build() DSL: {err:?}"),
                })?;
            }
            _ => {}
        }
    }

    Ok(Some(ComponentInfo {
        name: statement.id.name.to_string(),
        is_entry,
        state_fields,
        initial_render_body,
        entry_registration: is_entry.then(|| entry_registration(id, options)),
    }))
}

fn entry_registration(id: &str, options: &TransformOptions) -> EntryRegistrationInfo {
    let page_full_path = id.trim_end_matches(".ets").to_string();
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
}
