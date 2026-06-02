use super::*;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, StructElement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::FxHashSet;
use std::path::Path;

fn test_component_metadata() -> ComponentMetadata {
    ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata")
}

fn parse_and_lower(source: &str) -> Result<String, BuildLoweringError> {
    parse_and_lower_with_raw_object_fields(source, &FxHashSet::default())
}

fn parse_and_lower_with_raw_object_fields(
    source: &str,
    raw_object_fields: &FxHashSet<String>,
) -> Result<String, BuildLoweringError> {
    parse_and_lower_with_raw_object_fields_and_reusable(source, raw_object_fields, false)
}

fn parse_and_lower_with_raw_object_fields_and_reusable(
    source: &str,
    raw_object_fields: &FxHashSet<String>,
    current_component_reusable: bool,
) -> Result<String, BuildLoweringError> {
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
    lower_build_body_with_component_context(
        build.value.body.as_deref(),
        source,
        "",
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
        raw_object_fields,
        current_component_reusable,
    )
    .map(|(code, _)| code)
}

#[test]
fn lowers_none_body_returns_empty_string() {
    let (out, _) = lower_build_body(None, "").unwrap();
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
    assert!(
        !out.contains(".pop();\n        this.observeComponentCreation2"),
        "got:\n{out}"
    );
}

#[test]
fn lowers_leaf_component_no_args_no_attrs() {
    let source = "@Component\nstruct Foo {\n  build() {\n    Divider()\n  }\n}";
    let out = parse_and_lower(source).unwrap();
    assert!(out.contains("Divider.create();"), "got:\n{out}");
    assert!(out.contains("}, Divider);"), "got:\n{out}");
    assert!(!out.contains("Divider.pop();"), "got:\n{out}");
    assert!(
        !out.contains("Divider.create(,"),
        "no empty-arg comma; got:\n{out}"
    );
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
    let first = out
        .find("Text.create(\"first\");")
        .expect("first Text.create missing");
    let second = out
        .find("Text.create(\"second\");")
        .expect("second Text.create missing");
    let col_pop = out.find("Column.pop();").expect("Column.pop missing");
    assert!(
        col_create < first,
        "Column.create must precede first Text.create"
    );
    assert!(first < second, "first Text.create must precede second");
    assert!(
        second < col_pop,
        "second Text.create must precede Column.pop"
    );
    assert_eq!(
        out.matches("Text.pop();").count(),
        2,
        "two Text.pop() calls expected"
    );
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
    assert_eq!(
        out.matches("Text.pop();").count(),
        2,
        "two Text.pop() calls; got:\n{out}"
    );
}

#[test]
fn lowers_component_with_member_expression_arg() {
    let source = "@Component\nstruct Foo {\n  @State msg: string = \"hi\";\n  build() {\n    Text(this.msg)\n  }\n}";
    let out = parse_and_lower(source).unwrap();
    assert!(out.contains("Text.create(this.msg);"), "got:\n{out}");
}

#[test]
fn lowers_component_create_keeps_raw_object_top_level_arg() {
    let source = r#"class Item {}
@Component
struct Foo {
  @State item: Item = new Item();
  build() {
    Image(this.item)
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(out.contains("Image.create(this.item);"), "got:\n{out}");
    assert!(
        !out.contains("Image.create(ObservedObject.GetRawObject(this.item));"),
        "component create arg must match hvigor and stay unwrapped; got:\n{out}"
    );
}

#[test]
fn lowers_attribute_callback_wraps_raw_object_call_arg() {
    let source = r#"class Item {}
@Component
struct Foo {
  @State item: Item = new Item();
  handle(item: Item) {}
  build() {
    Button("OK").onClick(() => this.handle(this.item))
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(
        out.contains("Button.onClick(() => this.handle(ObservedObject.GetRawObject(this.item)));"),
        "attribute callback call argument should still match upstream raw-object lowering; got:\n{out}"
    );
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
    assert!(
        out.contains("Button.createWithLabel(\"OK\");"),
        "got:\n{out}"
    );
    assert!(out.contains("Button.width(100);"), "got:\n{out}");
    assert!(out.contains("Button.height(40);"), "got:\n{out}");
    assert!(out.contains("Button.borderRadius(8);"), "got:\n{out}");
    assert!(out.contains("Button.pop();"), "got:\n{out}");
    let create_pos = out.find("Button.create").unwrap();
    let pop_pos = out.find("Button.pop").unwrap();
    assert!(create_pos < pop_pos, "create must precede pop");
}

#[test]
fn lowers_button_with_children_using_create_with_child() {
    let source = r#"@Component
struct Foo {
  build() {
    Button("OK") {
      Text("child")
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("Button.createWithChild(\"OK\");"),
        "got:\n{out}"
    );
    assert!(out.contains("Text.create(\"child\");"), "got:\n{out}");
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
        assert!(
            out.contains(&format!("{name}.create();")),
            "{name}.create missing; got:\n{out}"
        );
        assert!(
            out.contains(&format!("{name}.pop();")),
            "{name}.pop missing; got:\n{out}"
        );
    }
    let flex_create = out.find("Flex.create();").unwrap();
    let row_create = out.find("Row.create();").unwrap();
    let row_pop = out.find("Row.pop();").unwrap();
    let flex_pop = out.find("Flex.pop();").unwrap();
    assert!(
        flex_create < row_create,
        "Flex.create must come before Row.create"
    );
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
    assert!(
        !out.contains("Row.pop();\n        Row.pop()"),
        "no duplicate Row.pop"
    );
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
    assert!(
        out.contains("Image.create($r(\"app.media.logo\"));"),
        "got:\n{out}"
    );
    assert!(
        out.contains("Image.objectFit(ImageFit.Cover);"),
        "got:\n{out}"
    );
    assert!(!out.contains("Image.pop();"), "got:\n{out}");
}

#[test]
fn lowers_reusable_regular_attrs_into_initial_render_block() {
    let source = r#"@Reusable
@Component
struct Foo {
  build() {
    Row() {
    }
      .width($r("app.float.card_width"))
      .alignItems(VerticalAlign.Top)
      .height(100)
      .linearGradient(this.selected ? { angle: "90deg" } : { colors: [] })
  }
}"#;
    let out =
        parse_and_lower_with_raw_object_fields_and_reusable(source, &FxHashSet::default(), true)
            .unwrap();
    let width = out
        .find("Row.width($r(\"app.float.card_width\"));")
        .unwrap();
    let gradient = out.find("Row.linearGradient(").unwrap();
    let initial = out.find("if (isInitialRender) {").unwrap();
    let align = out.find("Row.alignItems(VerticalAlign.Top);").unwrap();
    let height = out.find("Row.height(100);").unwrap();
    assert!(
        width < initial && gradient < initial,
        "dynamic/non-regular attrs stay in update path; got:\n{out}"
    );
    assert!(
        initial < align && align < height,
        "regular reusable attrs move into initial render block in source order; got:\n{out}"
    );
}

#[test]
fn prints_regular_function_calls_inside_attribute_arrow_with_codegen() {
    let source = r#"@Component
struct Foo {
  build() {
    Row() {
    }.onClick(() => {
      postCardAction(this, { action: "router" });
    })
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(out.contains("postCardAction(this, { action: \"router\" });"));
    assert!(
        !out.contains("postCardAction.create"),
        "ordinary function calls in attribute callbacks are not ArkUI components; got:\n{out}"
    );
}

#[test]
fn lowers_if_statement_children() {
    let source = r#"@Component
struct Foo {
  @State show: boolean = true;
  build() {
    Column() {
      if (this.show) {
        Text("shown")
      } else {
        Text("hidden")
      }
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    let if_pos = out.find("if (this.show) {").expect("if missing");
    let shown = out
        .find("Text.create(\"shown\");")
        .expect("shown Text.create missing");
    let else_pos = out.find("} else {").expect("else missing");
    let hidden = out
        .find("Text.create(\"hidden\");")
        .expect("hidden Text.create missing");
    let col_pop = out.find("Column.pop();").expect("Column.pop missing");
    assert!(if_pos < shown, "then branch lowered inside if; got:\n{out}");
    assert!(shown < else_pos, "then branch before else; got:\n{out}");
    assert!(else_pos < hidden, "else branch lowered; got:\n{out}");
    assert!(hidden < col_pop, "children before parent pop; got:\n{out}");
    assert_eq!(
        out.matches("Text.pop();").count(),
        2,
        "both branch Text components are popped; got:\n{out}"
    );
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

    let (out, span_hints) = lower_build_body_with_custom_components(
        build.value.body.as_deref(),
        source,
        "",
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
    )
    .unwrap();
    assert!(
        span_hints.len() >= 18,
        "expected token-level span hints for Row, Column, and Text"
    );
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

fn parse_and_lower_with_module(
    source: &str,
    source_id: &str,
    module_name: &str,
) -> Result<String, BuildLoweringError> {
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
    lower_build_body_with_component_context_and_module(
        build.value.body.as_deref(),
        source,
        source_id,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
        &FxHashSet::default(),
        Some(module_name),
        false,
    )
    .map(|(code, _)| code)
}

#[test]
fn nav_destination_emits_builder_closure_and_metadata() {
    let source = r#"@Component
struct Foo {
  build() {
    NavDestination() {
      Text("content")
    }
  }
}"#;
    let out = parse_and_lower_with_module(
        source,
        "features/challenge/ChallengeDetailView.ets",
        "phone",
    )
    .unwrap();
    // create() must include the builder closure placeholder and metadata object
    assert!(
        out.contains("NavDestination.create("),
        "missing NavDestination.create; got:\n{out}"
    );
    assert!(
        out.contains("moduleName: \"phone\""),
        "missing moduleName; got:\n{out}"
    );
    assert!(
        out.contains("pagePath: \"features/challenge/ChallengeDetailView\""),
        "missing pagePath (ets stripped); got:\n{out}"
    );
    // Children must still be lowered
    assert!(
        out.contains("Text.create(\"content\")"),
        "children not lowered; got:\n{out}"
    );
}

fn parse_and_lower_with_custom_components(
    source: &str,
    custom_components: &[&str],
) -> Result<String, BuildLoweringError> {
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
    let cc_set: FxHashSet<String> = custom_components.iter().map(|s| s.to_string()).collect();
    lower_build_body_with_custom_components(
        build.value.body.as_deref(),
        source,
        "",
        &cc_set,
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
    )
    .map(|(code, _)| code)
}

#[test]
fn custom_component_with_chain_attrs_emits_common_wrapper() {
    // CallExpression path: leaf custom component with chained attributes.
    let source = r#"@Component
struct Foo {
  build() {
    MyCard({ title: "hello" })
      .width("100%")
      .height(200)
  }
}"#;
    let out = parse_and_lower_with_custom_components(source, &["MyCard"]).unwrap();
    // __Common__.create() must be emitted in an observeComponentCreation2 block.
    assert!(
        out.contains("__Common__.create();"),
        "missing __Common__.create; got:\n{out}"
    );
    // Attribute calls must be on __Common__, not MyCard.
    assert!(
        out.contains("__Common__.width(\"100%\");"),
        "missing __Common__.width; got:\n{out}"
    );
    assert!(
        out.contains("__Common__.height(200);"),
        "missing __Common__.height; got:\n{out}"
    );
    // Must be wrapped with observeComponentCreation2 closing with __Common__.
    assert!(
        out.contains("}, __Common__);"),
        "missing }}, __Common__); got:\n{out}"
    );
    // The custom component itself must still be emitted.
    assert!(
        out.contains("new MyCard("),
        "missing MyCard constructor; got:\n{out}"
    );
    // __Common__.pop() must be emitted after the custom component.
    assert!(
        out.contains("__Common__.pop();"),
        "missing __Common__.pop; got:\n{out}"
    );
    // Order: __Common__.create before MyCard constructor before __Common__.pop.
    let common_create = out.find("__Common__.create();").unwrap();
    let my_card = out.find("new MyCard(").unwrap();
    let common_pop = out.find("__Common__.pop();").unwrap();
    assert!(
        common_create < my_card,
        "__Common__.create must precede MyCard constructor; got:\n{out}"
    );
    assert!(
        my_card < common_pop,
        "MyCard constructor must precede __Common__.pop; got:\n{out}"
    );
}

#[test]
fn custom_component_without_chain_attrs_does_not_emit_common_wrapper() {
    // Custom component with no chained attributes must NOT emit __Common__.
    let source = r#"@Component
struct Foo {
  build() {
    MyCard({ title: "hello" })
  }
}"#;
    let out = parse_and_lower_with_custom_components(source, &["MyCard"]).unwrap();
    assert!(
        !out.contains("__Common__"),
        "__Common__ must not be emitted when there are no chain attrs; got:\n{out}"
    );
}

#[test]
fn navigation_emits_builder_closure_and_metadata_with_is_user_create_stack() {
    let source = r#"@Component
struct Foo {
  build() {
    Navigation(new NavPathStack()) {
      Column() {}
    }
  }
}"#;
    let out = parse_and_lower_with_module(source, "features/nav/NavPage.ets", "entry").unwrap();
    assert!(
        out.contains("Navigation.create("),
        "missing Navigation.create; got:\n{out}"
    );
    assert!(
        out.contains("moduleName: \"entry\""),
        "missing moduleName; got:\n{out}"
    );
    assert!(
        out.contains("pagePath: \"features/nav/NavPage\""),
        "missing pagePath; got:\n{out}"
    );
    assert!(
        out.contains("isUserCreateStack:"),
        "missing isUserCreateStack; got:\n{out}"
    );
}

#[test]
fn lowers_gesture_attr_to_gesture_create_pop_sequence() {
    let source = r#"@Component
struct Foo {
  build() {
    Image($r("app.media.photo"))
      .objectFit(ImageFit.Contain)
      .gesture(
        LongPressGesture({ duration: 500 })
          .onAction((event) => {})
      )
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    // Regular attribute must still be emitted.
    assert!(
        out.contains("Image.objectFit(ImageFit.Contain);"),
        "objectFit missing; got:\n{out}"
    );
    // Gesture wrapper must be emitted.
    assert!(
        out.contains("Gesture.create(GesturePriority.Low);"),
        "Gesture.create missing; got:\n{out}"
    );
    // Inner gesture type must be emitted.
    assert!(
        out.contains("LongPressGesture.create("),
        "LongPressGesture.create missing; got:\n{out}"
    );
    // Gesture attribute call must be emitted.
    assert!(
        out.contains("LongPressGesture.onAction("),
        "LongPressGesture.onAction missing; got:\n{out}"
    );
    // Inner pop must come before outer pop.
    assert!(
        out.contains("LongPressGesture.pop();"),
        "LongPressGesture.pop missing; got:\n{out}"
    );
    assert!(
        out.contains("Gesture.pop();"),
        "Gesture.pop missing; got:\n{out}"
    );
    let inner_pop = out.find("LongPressGesture.pop();").unwrap();
    let outer_pop = out.find("Gesture.pop();").unwrap();
    assert!(
        inner_pop < outer_pop,
        "LongPressGesture.pop must precede Gesture.pop; got:\n{out}"
    );
    // The raw .gesture() call must NOT appear in the output.
    assert!(
        !out.contains("Image.gesture("),
        "Image.gesture() must not appear verbatim; got:\n{out}"
    );
}

#[test]
fn lowers_multiple_gesture_attrs_in_source_order() {
    let source = r#"@Component
struct Foo {
  build() {
    Row() {}
      .gesture(PinchGesture().onActionEnd(() => {}))
      .gesture(PanGesture().onActionUpdate((e) => {}))
      .visibility(Visibility.Visible)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    let pinch_create = out
        .find("PinchGesture.create(")
        .expect("PinchGesture.create");
    let pan_create = out.find("PanGesture.create(").expect("PanGesture.create");
    let vis = out.find("Row.visibility(").expect("Row.visibility");
    assert!(
        pinch_create < pan_create,
        "PinchGesture before PanGesture; got:\n{out}"
    );
    assert!(
        pan_create < vis,
        "gestures before visibility attr; got:\n{out}"
    );
    assert_eq!(
        out.matches("Gesture.create(GesturePriority.Low);").count(),
        2,
        "two Gesture.create; got:\n{out}"
    );
    // Count only standalone "Gesture.pop()" lines, not "XxxGesture.pop()".
    let gesture_pop_count = out.match_indices("\n            Gesture.pop();").count();
    assert_eq!(
        gesture_pop_count, 2,
        "two standalone Gesture.pop; got:\n{out}"
    );
}

#[test]
fn lowers_gesture_group_with_child_gestures() {
    // GestureGroup(mode, child1, child2) must expand each child gesture into
    // its own create/attrs/pop sequence between GestureGroup.create and
    // GestureGroup.pop.  The outer Gesture.create/Gesture.pop wrapper is still
    // emitted once for the whole group.
    let source = r#"@Component
struct Foo {
  build() {
    Column() {}
      .gesture(
        GestureGroup(GestureMode.Parallel,
          SwipeGesture({ direction: SwipeDirection.Vertical })
            .onAction((event) => {}),
          PanGesture({ direction: PanDirection.Vertical })
            .onActionStart(() => {})
            .onActionEnd((event) => {})
        )
      )
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    // Outer wrapper.
    assert!(
        out.contains("Gesture.create(GesturePriority.Low);"),
        "Gesture.create missing; got:\n{out}"
    );
    assert!(
        out.contains("Gesture.pop();"),
        "Gesture.pop missing; got:\n{out}"
    );
    // GestureGroup create must only contain the mode arg, not the child gestures.
    assert!(
        out.contains("GestureGroup.create(GestureMode.Parallel);"),
        "GestureGroup.create(mode) missing; got:\n{out}"
    );
    assert!(
        out.contains("GestureGroup.pop();"),
        "GestureGroup.pop missing; got:\n{out}"
    );
    // SwipeGesture child.
    assert!(
        out.contains("SwipeGesture.create("),
        "SwipeGesture.create missing; got:\n{out}"
    );
    assert!(
        out.contains("SwipeGesture.onAction("),
        "SwipeGesture.onAction missing; got:\n{out}"
    );
    assert!(
        out.contains("SwipeGesture.pop();"),
        "SwipeGesture.pop missing; got:\n{out}"
    );
    // PanGesture child.
    assert!(
        out.contains("PanGesture.create("),
        "PanGesture.create missing; got:\n{out}"
    );
    assert!(
        out.contains("PanGesture.onActionStart("),
        "PanGesture.onActionStart missing; got:\n{out}"
    );
    assert!(
        out.contains("PanGesture.onActionEnd("),
        "PanGesture.onActionEnd missing; got:\n{out}"
    );
    assert!(
        out.contains("PanGesture.pop();"),
        "PanGesture.pop missing; got:\n{out}"
    );
    // Order: GestureGroup.create -> SwipeGesture.create -> SwipeGesture.pop ->
    //        PanGesture.create -> PanGesture.pop -> GestureGroup.pop -> Gesture.pop
    let gg_create = out.find("GestureGroup.create(").unwrap();
    let swipe_create = out.find("SwipeGesture.create(").unwrap();
    let swipe_pop = out.find("SwipeGesture.pop();").unwrap();
    let pan_create = out.find("PanGesture.create(").unwrap();
    let pan_pop = out.find("PanGesture.pop();").unwrap();
    let gg_pop = out.find("GestureGroup.pop();").unwrap();
    // Find the standalone "Gesture.pop();" that is NOT part of "GestureGroup.pop();"
    let gesture_pop = out
        .match_indices("Gesture.pop();")
        .find(|(pos, _)| {
            // Ensure the character before "Gesture" is not a word char (i.e. not "Group")
            pos.checked_sub(1)
                .map(|p| !out.as_bytes()[p].is_ascii_alphabetic())
                .unwrap_or(true)
        })
        .map(|(pos, _)| pos)
        .expect("standalone Gesture.pop() not found");
    assert!(
        gg_create < swipe_create,
        "GestureGroup.create before SwipeGesture.create; got:\n{out}"
    );
    assert!(
        swipe_create < swipe_pop,
        "SwipeGesture.create before SwipeGesture.pop; got:\n{out}"
    );
    assert!(
        swipe_pop < pan_create,
        "SwipeGesture.pop before PanGesture.create; got:\n{out}"
    );
    assert!(
        pan_create < pan_pop,
        "PanGesture.create before PanGesture.pop; got:\n{out}"
    );
    assert!(
        pan_pop < gg_pop,
        "PanGesture.pop before GestureGroup.pop; got:\n{out}"
    );
    assert!(
        gg_pop < gesture_pop,
        "GestureGroup.pop before Gesture.pop; got:\n{out}"
    );
    // Only one Gesture.create/Gesture.pop pair (not one per child).
    assert_eq!(
        out.matches("Gesture.create(GesturePriority.Low);").count(),
        1,
        "one Gesture.create; got:\n{out}"
    );
    // The raw .gesture() call must not appear verbatim.
    assert!(
        !out.contains("Column.gesture("),
        "Column.gesture() must not appear verbatim; got:\n{out}"
    );
}
