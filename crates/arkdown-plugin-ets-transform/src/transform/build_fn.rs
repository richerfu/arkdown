use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, CallExpression, Expression, FunctionBody,
    Statement,
};
use oxc_span::GetSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLoweringError {
    UnsupportedStatement,
    UnsupportedChild,
    UnsupportedCallee,
    UnsupportedChain,
}

/// Lower an ArkUI `build()` body using the ohos-rs/oxc ArkUI AST.
///
/// This intentionally covers only the minimal fixture shape confirmed from
/// hvigor bytecode disassembly:
/// `Component(args) { children }.attr(args)` lowers to an
/// `observeComponentCreation2` callback containing `create` and chained
/// attribute calls, followed by child lowering and a post-order `pop`.
pub fn lower_build_body(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
) -> Result<String, BuildLoweringError> {
    let Some(body) = body else {
        return Ok(String::new());
    };

    let mut out = String::new();
    for statement in &body.statements {
        match statement {
            Statement::ExpressionStatement(statement) => match &statement.expression {
                Expression::ArkUIComponentExpression(component) => {
                    lower_component(component, source_text, "        ", &mut out)?;
                }
                _ => return Err(BuildLoweringError::UnsupportedStatement),
            },
            _ => return Err(BuildLoweringError::UnsupportedStatement),
        }
    }

    Ok(out)
}

fn lower_component(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    indent: &str,
    out: &mut String,
) -> Result<(), BuildLoweringError> {
    let name = component_name(&component.callee)?;
    let args = arguments_text(&component.arguments, source_text);

    out.push_str(&format!(
        "{indent}this.observeComponentCreation2((elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!("{indent}    {name}.create({args});\n"));
    for chain in &component.chain_expressions {
        let chain_name = chain_name(chain)?;
        let chain_args = arguments_text(&chain.arguments, source_text);
        out.push_str(&format!("{indent}    {name}.{chain_name}({chain_args});\n"));
    }
    out.push_str(&format!("{indent}}}, {name});\n"));

    for child in &component.children {
        match child {
            ArkUIChild::Component(child) => lower_component(child, source_text, indent, out)?,
            ArkUIChild::Expression(_) | ArkUIChild::Statement(_) => {
                return Err(BuildLoweringError::UnsupportedChild);
            }
        }
    }

    out.push_str(&format!("{indent}{name}.pop();\n"));
    Ok(())
}

fn component_name(callee: &Expression<'_>) -> Result<String, BuildLoweringError> {
    match callee {
        Expression::Identifier(identifier) => Ok(identifier.name.to_string()),
        _ => Err(BuildLoweringError::UnsupportedCallee),
    }
}

fn chain_name(call: &CallExpression<'_>) -> Result<String, BuildLoweringError> {
    match &call.callee {
        Expression::StaticMemberExpression(member) => Ok(member.property.name.to_string()),
        _ => Err(BuildLoweringError::UnsupportedChain),
    }
}

fn arguments_text(arguments: &[Argument<'_>], source_text: &str) -> String {
    arguments
        .iter()
        .map(|argument| span_text(argument, source_text).trim().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn span_text(node: &impl GetSpan, source_text: &str) -> String {
    let span = node.span();
    source_text[span].to_string()
}

/// Lower an ArkUI `build()` body, falling back to the raw source text on failure.
///
/// The OXC ETS parser applies `ArkUIComponentExpression` parsing only to
/// the canonical `build()` method.  `@Builder` method bodies are parsed as
/// regular TypeScript, so `lower_build_body` returns
/// `Err(UnsupportedStatement)` for them.
///
/// On failure this function extracts the inner body text from the source span
/// and re-indents it for a class method body.  The raw TypeScript is valid
/// (e.g. `Text('hi')` is a regular function call) even if not fully desugared.
pub fn lower_build_body_or_raw(body: Option<&FunctionBody<'_>>, source_text: &str) -> String {
    match lower_build_body(body, source_text) {
        Ok(lowered) => lowered,
        Err(_) => {
            // Fall back to raw inner body text, re-indented for a class method.
            let Some(body) = body else {
                return String::new();
            };
            let raw = span_text(body, source_text);
            // Strip the surrounding `{` / `}` and re-indent each non-empty line.
            let inner = raw.trim().trim_start_matches('{').trim_end_matches('}').trim();
            inner
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| format!("        {}\n", line.trim()))
                .collect()
        }
    }
}

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
    let body_text = span_text(body, source_text);
    // Strip the surrounding braces and process each line.
    let inner = body_text.trim().trim_start_matches('{').trim_end_matches('}').trim();
    let mut out = String::new();
    for line in inner.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('.') {
            // @Styles attribute chain: .attr(x) → this.attr(x);
            out.push_str("        this");
            out.push_str(trimmed);
            if !trimmed.ends_with(';') {
                out.push(';');
            }
            out.push('\n');
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Declaration, StructElement};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    #[test]
    fn lowers_minimal_fixture_build_body_from_oxc_arkui_ast() {
        let source = r#"
@Entry
@Component
struct Index {
  @State message: string = "Hello World";

  build() {
    Row() {
      Column() {
        Text(this.message)
          .fontSize(50)
          .fontWeight(FontWeight.Bold)
      }.width("100%")
    }.height("100%");
  }
}
"#;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );

        let struct_statement = parsed
            .program
            .body
            .iter()
            .find_map(|statement| match statement {
                Statement::StructStatement(statement) => Some(statement.as_ref()),
                Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                    Some(Declaration::StructStatement(statement)) => Some(statement.as_ref()),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected ArkUI struct");

        let build = struct_statement
            .body
            .body
            .iter()
            .find_map(|element| match element {
                StructElement::MethodDefinition(method)
                    if method.key.static_name().is_some_and(|name| name == "build") =>
                {
                    Some(method)
                }
                _ => None,
            })
            .expect("expected build method");

        let out = lower_build_body(build.value.body.as_deref(), source).unwrap();
        let expected = concat!(
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Row.create();\n",
            "            Row.height(\"100%\");\n",
            "        }, Row);\n",
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Column.create();\n",
            "            Column.width(\"100%\");\n",
            "        }, Column);\n",
            "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
            "            Text.create(this.message);\n",
            "            Text.fontSize(50);\n",
            "            Text.fontWeight(FontWeight.Bold);\n",
            "        }, Text);\n",
            "        Text.pop();\n",
            "        Column.pop();\n",
            "        Row.pop();\n",
        );
        assert_eq!(out, expected);
    }
}
