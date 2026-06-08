use rustc_hash::FxHashSet;

use arkdown_core::ModuleSource;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Statement, StructElement, StructStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::error::EtsTransformError;
use crate::source_map::{build_ets_source_map, offset_to_line_col, MapEntry};
use crate::transform::build_fn::{CustomComponentUpdateProps, SpanHint};
use crate::transform::component::{generate_view_pu_class, ComponentInfo};
use crate::transform::component_analysis::{
    collect_builder_function_names, collect_custom_component_names,
    collect_imported_component_names, collect_local_component_update_props, SimpleTypeContext,
};
use crate::transform::entry::EntryRegistrationBlock;
use crate::transform::viewpu_finalize::finalize_viewpu_output;
use crate::{apply_source_patches_with_offsets, SourcePatch};

use super::{
    component_from_struct, component_replacement_start, hvigor_source_path,
    program_has_arkts_transform_syntax, span_text, transform_non_component_statement,
    TransformOptions,
};

/// Byte range and source-map hints for one generated ViewPU class before finalization.
struct StructResult {
    viewpu_code: String,
    span_hints: Vec<SpanHint>,
    static_span_hints: Vec<SpanHint>,
    patch_index: usize,
    out_offset: usize,
    class_line_in_viewpu: usize,
    class_line_text: String,
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
    let mut custom_components = FxHashSet::default();
    let mut custom_component_update_props = CustomComponentUpdateProps::default();
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

    let mut patches = Vec::new();
    let mut struct_results: Vec<StructResult> = Vec::new();
    let mut entry_registrations = EntryRegistrationBlock::default();
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
            let span_hints = component.initial_render_span_hints.clone();
            if let Some(registration) = component.entry_registration_code() {
                entry_registrations.push(registration);
            }
            let viewpu_code = generate_view_pu_class(&component);
            let (class_line_in_viewpu, class_line_text) =
                viewpu_class_line_marker(&viewpu_code).unwrap_or((0, String::new()));
            let static_span_hints = collect_component_static_span_hints(
                &component,
                struct_statement,
                content,
                &viewpu_code,
            );
            let patch_index = patches.len();
            patches.push(SourcePatch {
                start,
                end: span.end as usize,
                replacement: viewpu_code.clone(),
            });
            struct_results.push(StructResult {
                viewpu_code,
                span_hints,
                static_span_hints,
                patch_index,
                out_offset: 0,
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
            patches.push(SourcePatch {
                start,
                end: span.end as usize,
                replacement,
            });
            transformed_any = true;
        }
    }
    let patched_source = apply_source_patches_with_offsets(content, patches);
    let mut out = patched_source.code;
    for result in &mut struct_results {
        result.out_offset = patched_source
            .output_starts
            .get(result.patch_index)
            .and_then(|offset| *offset)
            .expect("component patch must be applied");
    }

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
    entry_registrations.append_to(&mut out);
    out = finalize_viewpu_output(&out);

    let mut entries: Vec<MapEntry> = Vec::new();

    for sr in &struct_results {
        let struct_line_start = final_struct_line_start(&out, sr)
            .unwrap_or_else(|| count_newlines_before_byte(&out, sr.out_offset));

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

        let initial_render_line_in_struct = sr
            .viewpu_code
            .lines()
            .enumerate()
            .find(|(_, line)| line.trim_start().starts_with("initialRender()"))
            .map(|(i, _)| i)
            .unwrap_or(0);
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
        if is_viewpu_class_line(trimmed) {
            Some((index, trimmed.to_string()))
        } else {
            None
        }
    })
}

fn is_viewpu_class_line(line: &str) -> bool {
    (line.starts_with("class ")
        || line.starts_with("export class ")
        || line.starts_with("export default class "))
        && line.contains(" extends ViewPU")
}

fn count_newlines_before_byte(source: &str, offset: usize) -> usize {
    let end = previous_char_boundary(source, offset.min(source.len()));
    source[..end].bytes().filter(|&b| b == b'\n').count()
}

fn previous_char_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
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
    custom_components: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    builder_functions: &FxHashSet<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_newlines_before_mid_utf8_byte_offset() {
        let source = "alpha\n情况\nomega";
        let mid_char = source.find("况").expect("test fixture contains char") + 1;

        assert_eq!(count_newlines_before_byte(source, mid_char), 1);
    }

    #[test]
    fn viewpu_class_marker_accepts_exported_classes() {
        let named = "export class CommonAlertView extends ViewPU {";
        let default = "export default class Page extends ViewPU {";

        assert!(is_viewpu_class_line(named));
        assert!(is_viewpu_class_line(default));
    }
}

fn collect_component_static_span_hints(
    component: &ComponentInfo,
    statement: &StructStatement<'_>,
    source_text: &str,
    viewpu_code: &str,
) -> Vec<SpanHint> {
    let mut hints = Vec::new();
    let lines: Vec<&str> = viewpu_code.lines().collect();
    let generated = GeneratedViewPuLines::new(&lines);

    if let Some(line) = generated.class_declaration() {
        push_hint_for_text(&mut hints, &lines, line, &component.name, statement.id.span);
    }

    if let Some(line) = generated.constructor() {
        push_hint(
            &mut hints,
            line,
            leading_spaces(lines[line]),
            statement.span,
        );
    }

    if let Some(line) = generated
        .finalize_construction()
        .and_then(|line| find_next_line(&lines, line + 1, |line| line.trim() == "}"))
    {
        push_hint(
            &mut hints,
            line,
            lines[line].trim_end().len(),
            statement.span,
        );
    }

    if let Some(line) = generated.initial_render() {
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

        if let Some(line) = generated.field_declaration(name) {
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
        if let Some(line) = generated.field_initializer(name) {
            if let Some(init_span) = init_span {
                let initializer = span_text(init_span, source_text).trim();
                push_hint_for_text(&mut hints, &lines, line, initializer, init_span);
            }
        }
        if let Some(line) = generated.backing_field(name) {
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
        if let Some(line) = generated.getter(name) {
            push_hint_for_text(&mut hints, &lines, line, name, name_span);
        }
        if let Some(line) = generated.setter(name) {
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

struct GeneratedViewPuLines<'a> {
    lines: &'a [&'a str],
}

impl<'a> GeneratedViewPuLines<'a> {
    fn new(lines: &'a [&'a str]) -> Self {
        Self { lines }
    }

    fn class_declaration(&self) -> Option<usize> {
        self.find(|line| line.trim_start().starts_with("class "))
    }

    fn constructor(&self) -> Option<usize> {
        self.find(|line| line.trim_start().starts_with("constructor("))
    }

    fn finalize_construction(&self) -> Option<usize> {
        self.find(|line| line.contains("this.finalizeConstruction()"))
    }

    fn initial_render(&self) -> Option<usize> {
        self.find(|line| line.trim_start().starts_with("initialRender()"))
    }

    fn field_declaration(&self, name: &str) -> Option<usize> {
        self.find(|line| line.trim_start().starts_with(name))
    }

    fn field_initializer(&self, name: &str) -> Option<usize> {
        let needle = GeneratedFieldNeedle::initializer(name);
        self.find(|line| line.contains(&needle))
    }

    fn backing_field(&self, name: &str) -> Option<usize> {
        let needle = GeneratedFieldNeedle::backing_field(name);
        self.find(|line| line.contains(&needle))
    }

    fn getter(&self, name: &str) -> Option<usize> {
        let needle = GeneratedFieldNeedle::getter(name);
        self.find(|line| line.trim_start().starts_with(&needle))
    }

    fn setter(&self, name: &str) -> Option<usize> {
        let needle = GeneratedFieldNeedle::setter(name);
        self.find(|line| line.trim_start().starts_with(&needle))
    }

    fn find(&self, pred: impl Fn(&str) -> bool) -> Option<usize> {
        self.lines.iter().position(|line| pred(line))
    }
}

struct GeneratedFieldNeedle;

impl GeneratedFieldNeedle {
    fn initializer(name: &str) -> String {
        format!("this.__{name} = new ")
    }

    fn backing_field(name: &str) -> String {
        format!("private __{name}:")
    }

    fn getter(name: &str) -> String {
        format!("get {name}(")
    }

    fn setter(name: &str) -> String {
        format!("set {name}(")
    }
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

fn push_hint(hints: &mut Vec<SpanHint>, body_line: usize, body_col: usize, original_span: Span) {
    hints.push(SpanHint {
        body_line,
        body_col: body_col as u32,
        original_span,
    });
}

fn push_hint_for_text(
    hints: &mut Vec<SpanHint>,
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
