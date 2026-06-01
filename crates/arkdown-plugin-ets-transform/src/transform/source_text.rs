use oxc_ast::ast::{Declaration, Decorator, ExportDefaultDeclarationKind, Expression, Statement};
use oxc_span::{GetSpan, Span};

pub(super) fn span_text(span: Span, source_text: &str) -> &str {
    &source_text[span]
}

pub(super) fn type_annotation_text(span: Span, source_text: &str) -> String {
    span_text(span, source_text)
        .trim()
        .trim_start_matches(':')
        .trim()
        .to_string()
}

/// Extract the parameter list text from a `FormalParameters` span, stripped of
/// the surrounding parentheses.
pub(super) fn extract_params_text(params_span: Span, source_text: &str) -> String {
    span_text(params_span, source_text)
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

pub(super) fn decorator_first_arg_text(
    decorator: &Decorator<'_>,
    source_text: &str,
) -> Option<String> {
    let Expression::CallExpression(call) = &decorator.expression else {
        return None;
    };
    call.arguments
        .first()
        .map(|arg| span_text(arg.span(), source_text).trim().to_string())
}

pub(super) fn component_replacement_start(statement: &Statement<'_>, source_text: &str) -> usize {
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

pub(super) fn leading_decorator_start(source_text: &str, start: usize) -> usize {
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

pub(super) fn member_text_without_decorators(
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

pub(super) fn balanced_method_text(
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
