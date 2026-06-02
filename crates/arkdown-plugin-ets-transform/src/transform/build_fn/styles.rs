use oxc_ast::ast::FunctionBody;
use oxc_span::GetSpan;

/// Lower a `@Styles` method body into valid TypeScript.
///
/// ArkUI `@Styles` bodies consist of attribute chains starting with `.`:
/// ```text
/// { .width(100).height(100) }
/// ```
/// These are transformed to `this.attr(x)` calls so the generated TypeScript
/// is valid. Lines that do not start with `.` are preserved as-is.
///
/// Returns the indented statements for insertion inside a `private methodName()` body.
pub fn lower_styles_body(body: Option<&FunctionBody<'_>>, source_text: &str) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let span = body.span();
    let body_text = source_text[span].to_string();
    // Strip the surrounding braces and process each line.
    let inner = strip_outer_block(&body_text);
    let mut out = String::new();
    let mut chain = String::new();
    let mut paren_depth = 0i32;
    for line in inner.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !chain.is_empty() {
            chain.push('\n');
            chain.push_str("        ");
            chain.push_str(trimmed);
            paren_depth += paren_delta(trimmed);
            if paren_depth <= 0 {
                push_styles_chain(&mut out, &chain);
                chain.clear();
                paren_depth = 0;
            }
            continue;
        }
        if trimmed.starts_with('.') {
            chain.push_str("this");
            chain.push_str(trimmed);
            paren_depth = paren_delta(trimmed);
            if paren_depth <= 0 {
                push_styles_chain(&mut out, &chain);
                chain.clear();
                paren_depth = 0;
            }
        } else {
            // Regular statement — emit as-is with indentation.
            out.push_str("        ");
            out.push_str(trimmed);
            if !trimmed.ends_with(';') && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                out.push(';');
            }
            out.push('\n');
        }
    }
    if !chain.is_empty() {
        push_styles_chain(&mut out, &chain);
    }
    out
}

fn push_styles_chain(out: &mut String, chain: &str) {
    out.push_str("        ");
    out.push_str(chain);
    if !chain.trim_end().ends_with(';') {
        out.push(';');
    }
    out.push('\n');
}

fn paren_delta(text: &str) -> i32 {
    text.chars().fold(0, |acc, ch| match ch {
        '(' => acc + 1,
        ')' => acc - 1,
        _ => acc,
    })
}

fn strip_outer_block(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}
