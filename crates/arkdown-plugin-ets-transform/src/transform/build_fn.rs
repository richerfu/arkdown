use oxc_ast::ast::{
    Argument, ArkUIChild, ArkUIComponentExpression, CallExpression, Expression, FunctionBody,
    Statement,
};
use oxc_span::{GetSpan, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLoweringError {
    UnsupportedStatement,
    UnsupportedChild,
    UnsupportedCallee,
    UnsupportedChain,
}

/// Records the position where an ArkUI component call was emitted in the
/// lowered body text, along with the original `.ets` span of the component
/// expression.  Used to build a cross-transform source map.
#[derive(Debug, Clone)]
pub struct SpanHint {
    /// 0-based line index within the `initialRender()` body text.
    pub body_line: usize,
    /// Byte span of the ArkUI component expression in the original `.ets` source.
    pub original_span: Span,

/// Extracted representation of a leaf component call and its attribute chain.
struct LeafComponent {
    name: String,
    create_args: String,
    chain_attrs: Vec<(String, String)>,
}

/// Recursively unwrap a chained `CallExpression` into its base component name,
/// constructor arguments, and ordered attribute calls.
///
/// For `Button("OK").width(100).height(40)` the outer call is `.height(40)`.
/// Recursion reaches the base `Button("OK")` and then collects attrs in
/// source order on the way back up: `[("width", "100"), ("height", "40")]`.
fn extract_leaf_component(
    call: &CallExpression<'_>,
    source_text: &str,
) -> Result<LeafComponent, BuildLoweringError> {
    match &call.callee {
        Expression::Identifier(_) => {
            let name = component_name(&call.callee)?;
            let create_args = arguments_text(&call.arguments, source_text);
            Ok(LeafComponent { name, create_args, chain_attrs: vec![] })
        }
        Expression::StaticMemberExpression(member) => {
            let attr_name = member.property.name.to_string();
            let attr_args = arguments_text(&call.arguments, source_text);
            match &member.object {
                Expression::CallExpression(inner) => {
                    let mut lc = extract_leaf_component(inner, source_text)?;
                    lc.chain_attrs.push((attr_name, attr_args));
                    Ok(lc)
                }
                _ => Err(BuildLoweringError::UnsupportedCallee),
            }
        }
        _ => Err(BuildLoweringError::UnsupportedCallee),
    }
}

/// Lower a leaf component `CallExpression` (no block body) to ViewPU JS.
///
/// Handles both plain calls (`Divider()`) and chained attribute calls
/// (`Button("OK").width(100).height(40)`). The OXC ETS parser only creates
/// `ArkUIComponentExpression` nodes for component calls that have a block
/// body `{ }`. Leaf components without blocks appear as regular
/// `CallExpression` nodes in the top-level build() statement list.
fn lower_call_expression(
    call: &CallExpression<'_>,
    source_text: &str,
    indent: &str,
    out: &mut String,
) -> Result<(), BuildLoweringError> {
    let lc = extract_leaf_component(call, source_text)?;
    out.push_str(&format!(
        "{indent}this.observeComponentCreation2((elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!("{indent}    {}.create({});\n", lc.name, lc.create_args));
    for (attr_name, attr_args) in &lc.chain_attrs {
        out.push_str(&format!("{indent}    {}.{attr_name}({attr_args});\n", lc.name));
    }
    out.push_str(&format!("{indent}}}, {});\n", lc.name));
    out.push_str(&format!("{indent}{}.pop();\n", lc.name));
    Ok(())
}

/// Lower an ArkUI `build()` body using the ohos-rs/oxc ArkUI AST.
///
/// This intentionally covers only the minimal fixture shape confirmed from
/// hvigor bytecode disassembly:
/// `Component(args) { children }.attr(args)` lowers to an
/// `observeComponentCreation2` callback containing `create` and chained
/// attribute calls, followed by child lowering and a post-order `pop`.
///
/// Returns the lowered text together with span hints that map each emitted
/// `observeComponentCreation2` call back to the original `.ets` component
/// expression.  The caller uses these to build a cross-transform source map.
pub fn lower_build_body(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
) -> Result<(String, Vec<SpanHint>), BuildLoweringError> {
    let Some(body) = body else {
        return Ok((String::new(), vec![]));
    };

    let mut out = String::new();
    let mut span_hints: Vec<SpanHint> = Vec::new();
    for statement in &body.statements {
        match statement {
            Statement::ExpressionStatement(statement) => match &statement.expression {
                Expression::ArkUIComponentExpression(component) => {
                    lower_component(component, source_text, "        ", &mut out, &mut span_hints)?;
                }
                Expression::CallExpression(call) => {
                    lower_call_expression(call, source_text, "        ", &mut out)?;
                }
                _ => return Err(BuildLoweringError::UnsupportedStatement),
            },
            _ => return Err(BuildLoweringError::UnsupportedStatement),
        }
    }

    Ok((out, span_hints))
}

fn lower_component(
    component: &ArkUIComponentExpression<'_>,
    source_text: &str,
    indent: &str,
    out: &mut String,
    span_hints: &mut Vec<SpanHint>,
) -> Result<(), BuildLoweringError> {
    let name = component_name(&component.callee)?;
    let args = arguments_text(&component.arguments, source_text);

    // Record span hint before emitting — current newline count is the body line.
    let body_line = out.bytes().filter(|&b| b == b'\n').count();
    span_hints.push(SpanHint { body_line, original_span: component.span() });

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
            ArkUIChild::Component(child) => {
                lower_component(child, source_text, indent, out, span_hints)?;
            }
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
///
/// Returns `(lowered_text, span_hints)`.  Span hints are empty on the raw
/// fallback path because the OXC parser did not produce ArkUI AST nodes.
pub fn lower_build_body_or_raw(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
) -> (String, Vec<SpanHint>) {
    match lower_build_body(body, source_text) {
        Ok(result) => result,
        Err(_) => {
            // Fall back to raw inner body text, re-indented for a class method.
            let Some(body) = body else {
                return (String::new(), vec![]);
            };
            let raw = span_text(body, source_text);
            // Strip the surrounding `{` / `}` and re-indent each non-empty line.
            let inner = raw.trim().trim_start_matches('{').trim_end_matches('}').trim();
            let text = inner
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| format!("        {}\n", line.trim()))
                .collect();
            (text, vec![])
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

    fn parse_and_lower(source: &str) -> Result<String, BuildLoweringError> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
        assert!(parsed.errors.is_empty(), "parse errors: {:?}", parsed.errors);
        let struct_statement = parsed
            .program
            .body
            .iter()
            .find_map(|s| match s {
                Statement::StructStatement(st) => Some(st.as_ref()),
                Statement::ExportNamedDeclaration(e) => match e.declaration.as_ref() {
                    Some(Declaration::StructStatement(st)) => Some(st.as_ref()),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected struct");
        let build = struct_statement
            .body
            .body
            .iter()
            .find_map(|el| match el {
                StructElement::MethodDefinition(m)
                    if m.key.static_name().is_some_and(|n| n == "build") =>
                {
                    Some(m)
                }
                _ => None,
            })
            .expect("expected build method");
        lower_build_body(build.value.body.as_deref(), source)
    }

    #[test]
    fn lowers_none_body_returns_empty_string() {
        let out = lower_build_body(None, "").unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn lowers_empty_build_body_returns_empty_string() {
        let source = "@Component\nstruct Foo {\n  build() {}\n}";
        let out = parse_and_lower(source).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn lowers_single_leaf_component_with_string_arg() {
        let source = r#"@Component
struct Foo {
  build() {
    Text("Hello")
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Text.create(\"Hello\");"), "got:\n{out}");
        assert!(out.contains("}, Text);"), "got:\n{out}");
        assert!(out.contains("Text.pop();"), "got:\n{out}");
        assert!(!out.contains(".pop();\n        this.observeComponentCreation2"), "got:\n{out}");
    }

    #[test]
    fn lowers_leaf_component_no_args_no_attrs() {
        let source = "@Component\nstruct Foo {\n  build() {\n    Divider()\n  }\n}";
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Divider.create();"), "got:\n{out}");
        assert!(out.contains("}, Divider);"), "got:\n{out}");
        assert!(out.contains("Divider.pop();"), "got:\n{out}");
        assert!(!out.contains("Divider.create(,"), "no empty-arg comma; got:\n{out}");
    }

    #[test]
    fn lowers_container_with_two_text_siblings() {
        let source = r#"@Component
struct Foo {
  build() {
    Column() {
      Text("first")
      Text("second")
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        let col_create = out.find("Column.create();").expect("Column.create missing");
        let first = out.find("Text.create(\"first\");").expect("first Text.create missing");
        let second = out.find("Text.create(\"second\");").expect("second Text.create missing");
        let col_pop = out.find("Column.pop();").expect("Column.pop missing");
        assert!(col_create < first, "Column.create must precede first Text.create");
        assert!(first < second, "first Text.create must precede second");
        assert!(second < col_pop, "second Text.create must precede Column.pop");
        assert_eq!(out.matches("Text.pop();").count(), 2, "two Text.pop() calls expected");
    }

    #[test]
    fn lowers_multiple_root_siblings_in_build() {
        let source = r#"@Component
struct Foo {
  build() {
    Text("a")
    Text("b")
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert_eq!(
            out.matches("this.observeComponentCreation2").count(),
            2,
            "expected two observeComponentCreation2 calls; got:\n{out}"
        );
        assert_eq!(out.matches("Text.pop();").count(), 2, "two Text.pop() calls; got:\n{out}");
    }

    #[test]
    fn lowers_component_with_member_expression_arg() {
        let source = "@Component\nstruct Foo {\n  @State msg: string = \"hi\";\n  build() {\n    Text(this.msg)\n  }\n}";
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Text.create(this.msg);"), "got:\n{out}");
    }

    #[test]
    fn lowers_component_with_multiple_chain_attrs() {
        let source = r#"@Component
struct Foo {
  build() {
    Button("OK")
      .width(100)
      .height(40)
      .borderRadius(8)
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Button.create(\"OK\");"), "got:\n{out}");
        assert!(out.contains("Button.width(100);"), "got:\n{out}");
        assert!(out.contains("Button.height(40);"), "got:\n{out}");
        assert!(out.contains("Button.borderRadius(8);"), "got:\n{out}");
        assert!(out.contains("Button.pop();"), "got:\n{out}");
        let create_pos = out.find("Button.create").unwrap();
        let pop_pos = out.find("Button.pop").unwrap();
        assert!(create_pos < pop_pos, "create must precede pop");
    }

    #[test]
    fn lowers_deeply_nested_four_levels() {
        let source = r#"@Component
struct Foo {
  build() {
    Flex() {
      Stack() {
        Column() {
          Row() {
          }
        }
      }
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        for name in ["Flex", "Stack", "Column", "Row"] {
            assert!(out.contains(&format!("{name}.create();")), "{name}.create missing; got:\n{out}");
            assert!(out.contains(&format!("{name}.pop();")), "{name}.pop missing; got:\n{out}");
        }
        let flex_create = out.find("Flex.create();").unwrap();
        let row_create = out.find("Row.create();").unwrap();
        let row_pop = out.find("Row.pop();").unwrap();
        let flex_pop = out.find("Flex.pop();").unwrap();
        assert!(flex_create < row_create, "Flex.create must come before Row.create");
        assert!(row_create < row_pop, "Row.create before Row.pop");
        assert!(row_pop < flex_pop, "Row.pop before Flex.pop");
    }

    #[test]
    fn lowers_container_no_attrs_one_child() {
        let source = r#"@Component
struct Foo {
  build() {
    Row() {
      Spacer()
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        let row_create = out.find("Row.create();").expect("Row.create");
        let spacer_create = out.find("Spacer.create();").expect("Spacer.create");
        let spacer_pop = out.find("Spacer.pop();").expect("Spacer.pop");
        let row_pop = out.find("Row.pop();").expect("Row.pop");
        assert!(row_create < spacer_create);
        assert!(spacer_create < spacer_pop);
        assert!(spacer_pop < row_pop);
        assert!(!out.contains("Row.pop();\n        Row.pop()"), "no duplicate Row.pop");
    }

    #[test]
    fn lowers_component_with_enum_member_arg() {
        let source = r#"@Component
struct Foo {
  build() {
    Image($r("app.media.logo"))
      .objectFit(ImageFit.Cover)
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert!(out.contains("Image.create($r(\"app.media.logo\"));"), "got:\n{out}");
        assert!(out.contains("Image.objectFit(ImageFit.Cover);"), "got:\n{out}");
        assert!(out.contains("Image.pop();"), "got:\n{out}");
    }

    #[test]
    fn lowers_sibling_containers_with_children() {
        let source = r#"@Component
struct Foo {
  build() {
    Column() {
      Row() {
        Text("left")
      }
      Row() {
        Text("right")
      }
    }
  }
}"#;
        let out = parse_and_lower(source).unwrap();
        assert_eq!(
            out.matches("Row.create();").count(),
            2,
            "two Row.create() calls; got:\n{out}"
        );
        assert_eq!(
            out.matches("Row.pop();").count(),
            2,
            "two Row.pop() calls; got:\n{out}"
        );
        assert_eq!(
            out.matches("Text.create(").count(),
            2,
            "two Text.create() calls; got:\n{out}"
        );
        let col_create = out.find("Column.create();").expect("Column.create");
        let col_pop = out.find("Column.pop();").expect("Column.pop");
        assert!(col_create < col_pop);
    }

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

        let (out, span_hints) = lower_build_body(build.value.body.as_deref(), source).unwrap();
        assert_eq!(span_hints.len(), 3, "expected 3 span hints (Row, Column, Text)");
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
