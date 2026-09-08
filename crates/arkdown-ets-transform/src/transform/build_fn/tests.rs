use super::*;
use oxc_allocator::Allocator;
use oxc_allocator::Vec as OxcVec;
use oxc_ast::ast::{Declaration, StructElement};
use oxc_ast::builder::AstBuilder;
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span, SPAN};
use rustc_hash::FxHashSet;
use std::path::Path;

use crate::{ResolvedTypeIdentity, SpannedTypeIdentity};

fn test_component_metadata() -> ComponentMetadata {
    ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata")
}

fn lower_build_body(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
) -> Result<String, BuildLoweringError> {
    lower_build_body_with_custom_components(
        body,
        source_text,
        "",
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &ComponentMetadata::default(),
    )
}

fn lower_build_body_with_custom_components(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
) -> Result<String, BuildLoweringError> {
    lower_build_body_with_component_context_and_module(
        body,
        source_text,
        source_id,
        custom_components,
        builder_methods,
        custom_component_update_props,
        component_metadata,
        &FxHashSet::default(),
        None,
        false,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_build_body_with_component_context(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    builder_param_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
    current_component_reusable: bool,
    type_facts: Option<Arc<ArktsFileTypeFacts>>,
) -> Result<String, BuildLoweringError> {
    let mut visible_builder_methods = builder_methods.clone();
    visible_builder_methods.extend(builder_param_methods.iter().cloned());
    lower_build_body_with_component_context_and_module(
        body,
        source_text,
        source_id,
        custom_components,
        &visible_builder_methods,
        custom_component_update_props,
        component_metadata,
        raw_object_fields,
        None,
        current_component_reusable,
        type_facts,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_build_body_with_component_context_and_module(
    body: Option<&FunctionBody<'_>>,
    source_text: &str,
    source_id: &str,
    custom_components: &FxHashSet<String>,
    builder_methods: &FxHashSet<String>,
    custom_component_update_props: &CustomComponentUpdateProps,
    component_metadata: &ComponentMetadata,
    raw_object_fields: &FxHashSet<String>,
    module_name: Option<&str>,
    current_component_reusable: bool,
    type_facts: Option<Arc<ArktsFileTypeFacts>>,
) -> Result<String, BuildLoweringError> {
    let plan = BuildBodyPlan::component_with_shared_inputs(
        body,
        BuildBodySharedInputs {
            source_text: Arc::from(source_text),
            source_id: Arc::from(source_id),
            custom_components,
            builder_methods,
            custom_component_update_props,
            component_metadata: Arc::new(component_metadata.clone()),
            raw_object_fields,
            module_name: module_name.map(Arc::from),
        },
        current_component_reusable,
        GeneratedViewpuName::VIEWPU_BASE,
    )
    .with_type_facts(type_facts, 11);
    let allocator = Allocator::default();
    let ast = AstBuilder::new(&allocator);
    let statements = plan.statements(&allocator)?;
    let program = oxc_ast::ast::Program::new(
        SPAN,
        SourceType::ts().with_module(true),
        "",
        OxcVec::new_in(&ast),
        None,
        OxcVec::new_in(&ast),
        statements,
        &ast,
    );
    let code = Codegen::new()
        .with_options(CodegenOptions {
            indent_char: IndentChar::Space,
            indent_width: 4,
            initial_indent: 2,
            ..CodegenOptions::default()
        })
        .build(&program)
        .code;
    Ok(code)
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
    parse_and_lower_with_builder_param_methods(
        source,
        raw_object_fields,
        current_component_reusable,
        &FxHashSet::default(),
    )
}

fn parse_and_lower_with_builder_param_methods(
    source: &str,
    raw_object_fields: &FxHashSet<String>,
    current_component_reusable: bool,
    builder_param_methods: &FxHashSet<String>,
) -> Result<String, BuildLoweringError> {
    let mut builder_methods = FxHashSet::default();
    builder_methods.extend(builder_param_methods.iter().cloned());
    parse_and_lower_with_visible_builder_methods(
        source,
        raw_object_fields,
        current_component_reusable,
        &builder_methods,
        builder_param_methods,
    )
}

fn parse_and_lower_with_visible_builder_methods(
    source: &str,
    raw_object_fields: &FxHashSet<String>,
    current_component_reusable: bool,
    builder_methods: &FxHashSet<String>,
    builder_param_methods: &FxHashSet<String>,
) -> Result<String, BuildLoweringError> {
    parse_and_lower_with_visible_builder_methods_and_facts(
        source,
        raw_object_fields,
        current_component_reusable,
        builder_methods,
        builder_param_methods,
        None,
    )
}

fn parse_and_lower_with_visible_builder_methods_and_facts(
    source: &str,
    raw_object_fields: &FxHashSet<String>,
    current_component_reusable: bool,
    builder_methods: &FxHashSet<String>,
    builder_param_methods: &FxHashSet<String>,
    type_facts: Option<Arc<ArktsFileTypeFacts>>,
) -> Result<String, BuildLoweringError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "parse errors: {:?}",
        parsed.diagnostics
    );
    let struct_statement = parsed
        .program
        .body
        .iter()
        .find_map(|s| match s {
            Statement::StructStatement(st) => Some(st.as_ref()),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(st) => Some(st.as_ref()),
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
        builder_methods,
        builder_param_methods,
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
        raw_object_fields,
        current_component_reusable,
        type_facts,
    )
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
fn test_xcomponent_pop_matches_sdk_static_container_type_detection() {
    let source = r#"@Component
struct Foo {
  build() {
    Column() {
      XComponent({ id: "surface", type: resolveType() })
      XComponent({ id: "string", type: "component" }) {}
      XComponent({ id: "number", type: 1 }) {}
      XComponent({ id: "enum", type: XComponentType.COMPONENT }) {}
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert_eq!(
        out.matches("XComponent.pop();").count(),
        3,
        "SDK checkContainer only pops the three statically recognized component variants:\n{out}"
    );
}

#[test]
fn lowers_builder_param_object_argument_with_hvigor_proxy() {
    let source = r#"@Component
struct Foo {
  @BuilderParam builderPage: (payload: ESObject) => void = this.emptyBuilder
  routerContext: ESObject | null = null

  @Builder emptyBuilder() {}

  build() {
    this.builderPage({ routerContext: this.routerContext, title: "privacy" })
  }
}"#;
    let out = parse_and_lower_with_builder_param_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["builderPage".to_string()]),
    )
    .unwrap();
    assert!(
        out.contains("this.builderPage.bind(this)(makeBuilderParameterProxy(\"builderPage\""),
        "got:\n{out}"
    );
    assert!(
        out.contains(
            "routerContext: () => this[\"__routerContext\"] ? this[\"__routerContext\"] : this[\"routerContext\"]"
        ),
        "got:\n{out}"
    );
    assert!(out.contains("title: () => \"privacy\""), "got:\n{out}");
}

#[test]
fn lowers_optional_builder_param_invocation_with_hvigor_bind_chain() {
    let source = r#"@Component
struct Foo {
  @BuilderParam trailing?: () => void

  build() {
    Column() {
      this.trailing?.()
    }
  }
}"#;
    let out = parse_and_lower_with_builder_param_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["trailing".to_string()]),
    )
    .unwrap();
    assert!(
        out.contains("this.trailing?.bind(this)?.();"),
        "SDK transferBuilderCall preserves both optional-chain boundaries around bind:\n{out}"
    );
}

#[test]
fn lowers_repeat_callbacks_through_nested_parentheses() {
    let source = r#"@ComponentV2
struct Foo {
  @Local items: string[] = []

  build() {
    List() {
      Repeat(this.items)
        .template('default', ((((item) => {
          Text(item.item)
        }))))
        .each(((item) => {
          ListItem() {
            Text(item.item)
          }
        }))
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("Repeat(this.items, this).template(\"default\", (item) =>"),
        "Repeat.template must accept arbitrarily parenthesized ArkUI callbacks:\n{out}"
    );
    assert!(
        out.contains(".each((item) =>"),
        "Repeat.each must accept parenthesized ArkUI callbacks:\n{out}"
    );
    assert_eq!(
        out.matches("Text.create(item.item);").count(),
        2,
        "both callbacks must lower their ArkUI bodies:\n{out}"
    );
}

#[test]
fn repeat_preserves_direct_state_data_member() {
    let source = r#"@Component
struct Foo {
  @State items: object[] = []

  build() {
    Repeat(this.items).each(item => Text(item.item))
  }
}"#;
    let out = parse_and_lower_with_raw_object_fields(
        source,
        &FxHashSet::from_iter(["items".to_string()]),
    )
    .unwrap();

    assert!(out.contains("Repeat(this.items, this)"), "{out}");
    assert!(
        !out.contains("Repeat(ObservedObject.GetRawObject(this.items)"),
        "Repeat data must follow the SDK's source-node reuse semantics:\n{out}"
    );
}

#[test]
fn lowers_regular_builder_object_argument_with_hvigor_proxy() {
    let source = r#"@Component
struct Foo {
  routerContext: ESObject | null = null

  @Builder localBuilder(payload: ESObject) {}

  build() {
    this.localBuilder({ routerContext: this.routerContext })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["localBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains(
            "this.localBuilder.bind(this)(makeBuilderParameterProxy(\"localBuilder\", { routerContext: () => this[\"__routerContext\"] ? this[\"__routerContext\"] : this[\"routerContext\"] }))"
        ),
        "SDK process_component_build.js::transferBuilderCall proxies customBuilderMethod object arguments; got:\n{out}"
    );
    assert!(
        !out.contains("this.localBuilder.bind(this)({ routerContext: this.routerContext });"),
        "got:\n{out}"
    );
}

#[test]
fn create_builder_properties_follow_sdk_bind_object_metadata() {
    let source = r#"@Component
struct Foo {
  @Builder Footer() {}

  build() {
    WaterFlow({ footer: this.Footer() }) {}
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["Footer".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("WaterFlow.create({ footer: this.Footer.bind(this) });"),
        "WaterFlow.create must consume SDK BIND_OBJECT_PROPERTY metadata; got:\n{out}"
    );
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
fn lowers_free_function_call_keeps_raw_object_argument() {
    let source = r#"class Item {}
@Component
struct Foo {
  @State item: Item = new Item();
  build() {
    Text(formatItem(this.item))
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(
        out.contains("Text.create(formatItem(this.item));")
            && !out.contains("formatItem(ObservedObject.GetRawObject(this.item))"),
        "SDK isNeedGetRawObject only rewrites arguments of property-access method calls:\n{out}"
    );
}

#[test]
fn lowers_builder_arguments_using_the_source_callee_shape() {
    let source = r#"class Item {}
@Component
struct Foo {
  @State item: Item = new Item();
  @Builder Render(item: Item) {}
  build() {
    Render(this.item)
    this.Render(this.item)
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let builder_methods = FxHashSet::from_iter(["Render".to_string()]);
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &raw_object_fields,
        false,
        &builder_methods,
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Render.bind(this)(this.item);"),
        "an unqualified builder has an identifier callee before transferBuilderCall:\n{out}"
    );
    assert!(
        out.contains("this.Render.bind(this)(ObservedObject.GetRawObject(this.item));"),
        "a member builder retains its property-access callee eligibility:\n{out}"
    );
}

#[test]
fn lowers_optional_state_field_method_argument_to_raw_object() {
    let source = r#"class Item {}
@Component
struct Foo {
  @State item: Item | null = null;
  build() {
    Button("OK").onClick(() => Reporter.get().send(this?.item))
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(
        out.contains("Reporter.get().send(ObservedObject.GetRawObject(this?.item))"),
        "optional property-access arguments follow SDK checkStateName semantics:\n{out}"
    );
}

#[test]
fn lowers_gesture_callback_wraps_raw_object_call_arg() {
    let source = r#"@Component
struct Foo {
  @State item: object = {};
  build() {
    Stack()
      .gesture(
        TapGesture()
          .onAction((event: GestureEvent) => {
            this.handle(this.item, event)
          })
      )
  }
  handle(item: object, event: GestureEvent) {}
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(
        out.contains("TapGesture.onAction((event: GestureEvent) => {")
            && out.contains(
                "this.handle(ObservedObject.GetRawObject(this.item), event);"
            ),
        "SDK processBuildMember visits call arguments nested inside gesture callback attributes:\n{out}"
    );
}

#[test]
fn lowers_if_condition_wraps_raw_object_call_arg() {
    let source = r#"class Item {}
@Component
struct Foo {
  @ObjectLink item: Item;
  isVisible(item: Item): boolean {
    return true
  }
  build() {
    if (this.isVisible(this.item)) {
      Text("visible")
    }
  }
}"#;
    let raw_object_fields = FxHashSet::from_iter(["item".to_string()]);
    let out = parse_and_lower_with_raw_object_fields(source, &raw_object_fields).unwrap();
    assert!(
        out.contains("if (this.isVisible(ObservedObject.GetRawObject(this.item)))"),
        "SDK processBuildMember rewrites state-object call arguments in UI conditions:\n{out}"
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
fn lowers_component_animation_attribute_to_context_animation() {
    let source = r#"@Component
struct Foo {
  build() {
    Row() {
      Text("child")
    }
      .width(100)
      .height(40)
      .animation({ duration: 300 })
      .onAppear(() => {})
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("globalThis.Context.animation({ duration: 300 });"),
        "missing animation context start; got:\n{out}"
    );
    assert!(
        out.contains("globalThis.Context.animation(null);"),
        "missing animation context reset; got:\n{out}"
    );
    assert!(
        !out.contains("Row.animation("),
        "animation must not be emitted as a component attribute; got:\n{out}"
    );
    let animation = out
        .find("globalThis.Context.animation({ duration: 300 });")
        .unwrap();
    let width = out.find("Row.width(100);").unwrap();
    let height = out.find("Row.height(40);").unwrap();
    let clear = out.find("globalThis.Context.animation(null);").unwrap();
    let appear = out.find("Row.onAppear(").unwrap();
    assert!(
        animation < width && width < height && height < clear && clear < appear,
        "animation context must wrap the preceding attribute segment; got:\n{out}"
    );
}

#[test]
fn first_animation_attribute_only_emits_hvigor_clear_boundary() {
    // hvigor process_component_build.js::addComponentAttr defers the animation
    // configuration until an earlier source attribute makes the segment active.
    let source = r#"@Component
struct Foo {
  build() {
    Row()
      .animation({ duration: 300 })
      .width(100)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        !out.contains("Context.animation({ duration: 300 });"),
        "the first attribute must not activate its animation configuration; got:\n{out}"
    );
    assert_eq!(
        out.matches("Context.animation(null);").count(),
        1,
        "a non-null first animation still emits its clear boundary; got:\n{out}"
    );
    let clear = out.find("Context.animation(null);").unwrap();
    let width = out.find("Row.width(100);").unwrap();
    assert!(
        clear < width,
        "the clear boundary must precede width; got:\n{out}"
    );
}

#[test]
fn terminal_null_animation_does_not_emit_a_second_clear_boundary() {
    let source = r#"@Component
struct Foo {
  build() {
    Row()
      .width(100)
      .animation(null)
      .height(40)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert_eq!(
        out.matches("Context.animation(null);").count(),
        1,
        "hvigor uses the null configuration itself as the terminal boundary; got:\n{out}"
    );
    let animation = out.find("Context.animation(null);").unwrap();
    let width = out.find("Row.width(100);").unwrap();
    let height = out.find("Row.height(40);").unwrap();
    assert!(
        animation < width && width < height,
        "the null animation must precede its attribute segment; got:\n{out}"
    );
}

#[test]
fn consecutive_animation_attributes_flush_the_later_configuration() {
    let source = r#"@Component
struct Foo {
  build() {
    Row()
      .animation({ duration: 100 })
      .animation({ duration: 200 })
      .width(100)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        !out.contains("Context.animation({ duration: 100 });"),
        "the first source attribute remains deferred; got:\n{out}"
    );
    assert!(
        out.contains("Context.animation({ duration: 200 });"),
        "the preceding animation must flush the later configuration; got:\n{out}"
    );
    assert_eq!(
        out.matches("Context.animation(null);").count(),
        2,
        "each non-null animation emits a clear boundary; got:\n{out}"
    );
}

#[test]
fn lowers_leaf_animation_attribute_to_context_animation() {
    let source = r#"@Component
struct Foo {
  build() {
    Text("Hello")
      .fontSize(18)
      .animation({ duration: 120 })
      .onClick(() => {})
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("Context.animation({ duration: 120 });"),
        "missing animation context start; got:\n{out}"
    );
    assert!(
        out.contains("Context.animation(null);"),
        "missing animation context reset; got:\n{out}"
    );
    assert!(
        !out.contains("Text.animation("),
        "animation must not be emitted as a leaf component attribute; got:\n{out}"
    );
}

#[test]
fn lowers_modifier_attributes_with_bound_component_method() {
    let source = r#"const columnItemModifier = {}
const contentModifier = {}
const menuItemContentModifier = {}
@Component
struct Foo {
  build() {
    Column() {
    }
      .attributeModifier(columnItemModifier)
      .contentModifier(contentModifier)
      .menuItemContentModifier(menuItemContentModifier)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("Column.attributeModifier.bind(this)(columnItemModifier);"),
        "attributeModifier must match hvigor's bound component method call; got:\n{out}"
    );
    assert!(
        out.contains("Column.contentModifier.bind(this)(contentModifier);"),
        "contentModifier must match hvigor's bound component method call; got:\n{out}"
    );
    assert!(
        out.contains("Column.menuItemContentModifier.bind(this)(menuItemContentModifier);"),
        "menuItemContentModifier must match hvigor's bound component method call; got:\n{out}"
    );
    assert!(
        !out.contains("Column.attributeModifier(columnItemModifier);"),
        "modifier attributes must not be emitted as unbound calls; got:\n{out}"
    );
}

#[test]
fn lowers_create_object_double_dollar_to_value_change_event_object() {
    let source = r#"@Component
struct Foo {
  @State value: string = ""

  build() {
    TextInput({ text: $$this.value })
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("TextInput.create({ text: {"),
        "create object $$ must become hvigor value/changeEvent object; got:\n{out}"
    );
    assert!(out.contains("value: this.value"), "got:\n{out}");
    assert!(out.contains("changeEvent: (newValue) => {"), "got:\n{out}");
    assert!(
        out.contains("this.value = newValue;"),
        "create object $$ must assign back to the target; got:\n{out}"
    );
    assert!(
        !out.contains("$$this.value"),
        "$$ marker must not leak into generated TS; got:\n{out}"
    );
}

#[test]
fn lowers_create_object_double_exclamation_to_value_and_dollar_property() {
    let source = r#"@ComponentV2
struct Foo {
  @Local value: string = ""

  build() {
    TextInput({ text: this.value!! })
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("TextInput.create({")
            && out.contains("text: this.value,")
            && out.contains("$text: (newValue) => {"),
        "create object !! must emit the paired $property callback; got:\n{out}"
    );
    assert!(out.contains("this.value = newValue;"), "got:\n{out}");
    assert!(
        !out.contains("this.value!!"),
        "!! marker must not leak into generated TypeScript; got:\n{out}"
    );
}

#[test]
fn lowers_attribute_double_dollar_to_value_and_change_event_arguments() {
    let source = r#"@Component
struct Foo {
  @State index: number = 0

  build() {
    Tabs() {
    }.index($$this.index)
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("Tabs.index(this.index, (newValue) => {"),
        "attribute $$ must become value plus changeEvent argument; got:\n{out}"
    );
    assert!(
        out.contains("this.index = newValue;"),
        "attribute $$ must assign back to the target; got:\n{out}"
    );
}

#[test]
fn lowers_style_double_dollar_with_extra_args_to_value_change_event_object() {
    let source = r#"@Component
struct Foo {
  @State show: boolean = false

  @Builder SheetBuilder() {}

  build() {
    Column() {
    }.bindSheet($$this.show, this.SheetBuilder, { detents: [1] })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["SheetBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Column.bindSheet({"),
        "style $$ with extra args must wrap first arg in value/changeEvent object; got:\n{out}"
    );
    assert!(out.contains("value: this.show"), "got:\n{out}");
    assert!(out.contains("changeEvent: (newValue) => {"), "got:\n{out}");
    assert!(
        out.contains("this.show = newValue;"),
        "style $$ must assign back to target; got:\n{out}"
    );
    assert!(
        out.contains("{ builder: this.SheetBuilder.bind(this) }"),
        "bindSheet builder arg must still be hvigor builder object after $$ rewrite; got:\n{out}"
    );
}

#[test]
fn lowers_style_double_exclamation_to_value_and_dollar_value_object() {
    let source = r#"@ComponentV2
struct Foo {
  @Local show: boolean = false

  @Builder MenuBuilder() {}

  build() {
    Button("menu").bindMenu(this.show!!, this.MenuBuilder)
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["MenuBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(out.contains("Button.bindMenu({"), "got:\n{out}");
    assert!(out.contains("value: this.show"), "got:\n{out}");
    assert!(out.contains("$value: (newValue) => {"), "got:\n{out}");
    assert!(out.contains("this.show = newValue;"), "got:\n{out}");
    assert!(
        out.contains("this.MenuBuilder.bind(this)"),
        "builder argument must retain its binding rewrite; got:\n{out}"
    );
}

#[test]
fn lowers_style_double_dollar_after_builder_call_argument_rewrite() {
    let source = r#"@Builder
function JSExceptionSheet(message: string) {}

@Component
struct Foo {
  @State show: boolean = false
  exception: string = "boom"

  build() {
    Stack().bindSheet($$this.show, JSExceptionSheet(this.exception!), { height: "70%" })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["JSExceptionSheet".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Stack.bindSheet({"),
        "bindSheet $$ first arg must become hvigor value/changeEvent object; got:\n{out}"
    );
    assert!(out.contains("value: this.show"), "got:\n{out}");
    assert!(out.contains("changeEvent: (newValue) => {"), "got:\n{out}");
    assert!(
        out.contains("this.show = newValue;"),
        "bindSheet $$ must assign back to the target; got:\n{out}"
    );
    assert!(
        out.contains("JSExceptionSheet.call(this, this.exception!)"),
        "builder call argument must keep hvigor call wrapper; got:\n{out}"
    );
    assert!(
        !out.contains("$$this.show"),
        "$$ marker must not leak into generated TS; got:\n{out}"
    );
}

#[test]
fn lowers_builder_object_properties_recursively() {
    let source = r#"@Component
struct Foo {
  @Builder StartBuilder() {}
  @Builder EndBuilder() {}

  build() {
    ListItem() {
      Text("item")
    }.swipeAction({
      start: { start: this.StartBuilder },
      end: this.EndBuilder(),
      plain: { start: this.StartBuilder }
    })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["StartBuilder".to_string(), "EndBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("start: { start: this.StartBuilder.bind(this) }"),
        "object builder property object values must recurse through hvigor builder keys; got:\n{out}"
    );
    assert!(
        out.contains("end: this.EndBuilder.bind(this)"),
        "object builder property call must become bound callee; got:\n{out}"
    );
    assert!(
        out.contains("plain: { start: this.StartBuilder }"),
        "non-builder object keys must not be recursively transformed; got:\n{out}"
    );
}

#[test]
fn lowers_empty_list_item_with_sdk_lazy_item_creation() {
    let source = r#"@Component
struct Foo {
  build() {
    List() {
      ListItem() {}.width(8)
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("ListItem.create(deepRenderFunction, true);"),
        "SDK processItemComponent lazily creates every ordinary ListItem, including empty items:\n{out}"
    );
    assert!(
        out.contains("const deepRenderFunction = (elmtId, isInitialRender) => {"),
        "ordinary ListItem must define the SDK deep-render callback:\n{out}"
    );
    assert_eq!(
        out.matches("ListItem.pop();").count(),
        3,
        "lazy item creation, deep render, and outer creation each close the ListItem:\n{out}"
    );
}

#[test]
fn lazy_list_item_uses_hvigor_first_animation_boundary() {
    let source = r#"@Component
struct Foo {
  build() {
    List() {
      ListItem() {}
        .animation({ duration: 90 })
        .width(8)
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        !out.contains("Context.animation({ duration: 90 });"),
        "ListItem itemCreation2 must defer a first animation attribute like bindComponentAttr; got:\n{out}"
    );
    assert_eq!(
        out.matches("Context.animation(null);").count(),
        1,
        "ListItem itemCreation2 must retain the non-null animation clear boundary; got:\n{out}"
    );
}

#[test]
fn lowers_grid_item_with_sdk_observed_deep_render() {
    let source = r#"@Component
struct Foo {
  build() {
    Grid() {
      GridItem() {
        Text("item")
      }.id("cell")
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("GridItem.create(() => {}, false);"),
        "SDK createItemCreate uses the non-lazy item contract for GridItem:\n{out}"
    );
    assert!(
        out.contains("const observedDeepRender = () => {"),
        "GridItem children must be hosted by observedDeepRender:\n{out}"
    );
    assert!(
        out.contains("this.observeComponentCreation2(itemCreation2, GridItem);"),
        "GridItem itemCreation2 must be observed inside the deep-render callback:\n{out}"
    );
}

#[test]
fn list_item_string_false_disables_lazy_creation_like_sdk() {
    let source = r#"@Component
struct Foo {
  build() {
    List() {
      ListItem("false") {
        Text("item")
      }
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("ListItem.create(() => {}, false, \"false\");"),
        "SDK checkLazyCreate recognizes the string literal 'false' and preserves it after the item flags:\n{out}"
    );
    assert!(
        out.contains("const observedDeepRender = () => {"),
        "non-lazy ListItem must use observedDeepRender:\n{out}"
    );
    assert!(
        !out.contains("ListItem.create(deepRenderFunction, true"),
        "string 'false' must disable ListItem lazy creation:\n{out}"
    );
}

#[test]
fn lowers_conditional_builder_attribute_branches_like_hvigor() {
    let source = r#"@Component
struct Foo {
  viewModel: ESObject = {}

  @Builder KeyboardBuilder() {}

  build() {
    TextInput({ text: "" })
      .customKeyboard(this.viewModel.keyboardView ? this.KeyboardBuilder() : undefined)
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["KeyboardBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("TextInput.customKeyboard(this.viewModel.keyboardView ? { builder: () => {"),
        "conditional custom builder true branch must become hvigor builder object; got:\n{out}"
    );
    assert!(
        out.contains("this.KeyboardBuilder.call(this);"),
        "builder call branch must call through Function.call(this); got:\n{out}"
    );
    assert!(
        out.contains("} } : undefined);"),
        "conditional false branch must be preserved; got:\n{out}"
    );
}

#[test]
fn lowers_builder_call_object_args_with_hvigor_proxy() {
    let source = r#"@Component
struct Foo {
  item: ESObject = {}

  @Builder TitleBuilder(payload: ESObject) {}

  build() {
    Navigation() {
    }.title(this.TitleBuilder({ item: this.item, label: "x" }))
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["TitleBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Navigation.title({ builder: () => {"),
        "builder call attr must be wrapped in hvigor builder object; got:\n{out}"
    );
    assert!(
        out.contains("this.TitleBuilder.call(this, makeBuilderParameterProxy(\"TitleBuilder\""),
        "builder object arg must be converted through makeBuilderParameterProxy; got:\n{out}"
    );
    assert!(
        out.contains("item: () => this[\"__item\"] ? this[\"__item\"] : this[\"item\"]"),
        "proxy must preserve hvigor backing-field fallback for this.item; got:\n{out}"
    );
    assert!(out.contains("label: () => \"x\""), "got:\n{out}");
}

#[test]
fn lowers_without_key_builder_attributes_as_bound_function() {
    let source = r#"@Component
struct Foo {
  @Builder ItemBuilder() {}

  build() {
    MenuItem().create(this.ItemBuilder)
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["ItemBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("MenuItem.create(this.ItemBuilder.bind(this));"),
        "CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY attributes must not wrap {{ builder }}; got:\n{out}"
    );
}

#[test]
fn menu_item_create_wraps_builder_invocations_like_hvigor() {
    let source = r#"@Component
struct Foo {
  @Builder ItemBuilder() {}

  build() {
    MenuItem(this.ItemBuilder())
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["ItemBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("MenuItem.create(() => {")
            && out.contains("this.ItemBuilder.call(this);")
            && !out.contains("MenuItem.create(this.ItemBuilder());"),
        "ets-loader checkCreateArgumentBuilder wraps a builder invocation in a lexical callback; got:\n{out}"
    );
}

#[test]
fn lowers_bind_popup_builder_config_property() {
    let source = r#"@Component
struct Foo {
  @State show: boolean = false
  @Builder PopupBuilder() {}

  build() {
    Column() {
    }.bindPopup(this.show, { builder: this.PopupBuilder })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["PopupBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Column.bindPopup(this.show, { builder: { builder: this.PopupBuilder.bind(this) } });"),
        "bindPopup config builder property must follow hvigor nested builder object shape; got:\n{out}"
    );
}

#[test]
fn bind_menu_keeps_non_builder_arguments_unchanged() {
    let source = r#"@Component
struct Foo {
  @State show: boolean = false
  @Builder MenuBuilder() {}

  build() {
    Button("menu").bindMenu(this.show, { builder: this.MenuBuilder })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["MenuBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();

    assert!(
        out.contains("Button.bindMenu(this.show, { builder: this.MenuBuilder.bind(this) });"),
        "SDK isBuilderChangeNode transforms only expressions proven to be builders:\n{out}"
    );
    assert!(
        !out.contains("{ builder: this.show.bind(this) }"),
        "a regular state member must not be rebound as a builder:\n{out}"
    );
}

#[test]
fn lowers_drag_return_builder_property() {
    let source = r#"@Component
struct Foo {
  @Builder DragBuilder() {}

  build() {
    Column() {
    }.onDragStart(() => {
      return { builder: this.DragBuilder }
    })
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["DragBuilder".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("return { builder: { builder: this.DragBuilder.bind(this) } };"),
        "drag return builder property must be transformed through processInitializer; got:\n{out}"
    );
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
    let enum_access = "VerticalAlign.Top";
    let start = source.find(enum_access).unwrap() as u32;
    let type_facts = ArktsFileTypeFacts {
        member_accesses: vec![SpannedTypeIdentity {
            span: Span::new(start, start + enum_access.len() as u32),
            identity: ResolvedTypeIdentity {
                is_nullable: false,
                is_enum: true,
                is_basic: true,
                symbol_name: "Top".to_string(),
                types: Vec::new(),
            },
        }],
        ..ArktsFileTypeFacts::default()
    };
    let out = parse_and_lower_with_visible_builder_methods_and_facts(
        source,
        &FxHashSet::default(),
        true,
        &FxHashSet::default(),
        &FxHashSet::default(),
        Some(Arc::new(type_facts)),
    )
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
fn lowers_else_if_chain_with_terminal_empty_branch() {
    let source = r#"@Component
struct Foo {
  @State value: number = 0;
  build() {
    if (this.value === 0) {
      Text("zero")
    } else if (this.value === 1) {
      Text("one")
    }
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert_eq!(
        out.matches("this.ifElseBranchUpdateFunction(").count(),
        3,
        "SDK processElseStatement emits update functions for both source branches and the terminal empty branch:\n{out}"
    );
    for branch_index in 0..=2 {
        assert!(
            out.contains(&format!("this.ifElseBranchUpdateFunction({branch_index},")),
            "missing update function for branch {branch_index}:\n{out}"
        );
    }
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
fn lowers_for_each_expression_body_item_generator() {
    let source = r#"@Component
struct Foo {
  items: string[] = [];

  build() {
    ForEach(this.items, item => Text(item))
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    assert!(
        out.contains("const forEachItemGenFunction = (_item) => {"),
        "expected ForEach item generator; got:\n{out}"
    );
    assert!(
        out.contains("Text.create(item);"),
        "expected expression-bodied item component to be emitted; got:\n{out}"
    );
    assert!(
        !out.contains("return undefined;"),
        "expression-bodied item generator must not be lowered to an empty return; got:\n{out}"
    );
}

#[test]
fn lowers_for_each_chain_attributes_before_the_item_generator() {
    let source = r#"@Component
struct Foo {
  items: string[] = [];

  build() {
    ForEach(this.items, item => {
      Text(item)
    }, item => item).onMove((from, to) => {})
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    let create = out.find("ForEach.create();").expect("ForEach.create");
    let on_move = out.find("ForEach.onMove(").expect("ForEach.onMove");
    let generator = out
        .find("const forEachItemGenFunction")
        .expect("ForEach item generator");
    let text = out.find("Text.create(item);").expect("item child");

    assert!(
        create < on_move && on_move < generator && generator < text,
        "ForEach attributes must be emitted between create and item generation:\n{out}"
    );
    assert!(
        !out.contains("ForEach.create(this.items"),
        "ForEach data and callbacks belong to the update call, not create:\n{out}"
    );
}

#[test]
fn lowers_lazy_for_each_chain_attributes_after_create() {
    let source = r#"@Component
struct Foo {
  data: Object;

  build() {
    LazyForEach(this.data, item => {
      ListItem() {
        Text(item)
      }
    }, item => item).onMove((from, to) => {})
  }
}"#;
    let out = parse_and_lower(source).unwrap();
    let create = out.find("LazyForEach.create(").expect("LazyForEach.create");
    let on_move = out.find("LazyForEach.onMove(").expect("LazyForEach.onMove");
    let pop = out.find("LazyForEach.pop();").expect("LazyForEach.pop");

    assert!(
        create < on_move && on_move < pop,
        "LazyForEach attributes must be emitted after create and before pop:\n{out}"
    );
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
        parsed.diagnostics.is_empty(),
        "parse errors: {:?}",
        parsed.diagnostics
    );

    let struct_statement = parsed
        .program
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::StructStatement(statement) => Some(statement.as_ref()),
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::StructStatement(statement) => Some(statement.as_ref()),
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

    let out = lower_build_body_with_custom_components(
        build.value.body.as_deref(),
        source,
        "",
        &FxHashSet::default(),
        &FxHashSet::default(),
        &CustomComponentUpdateProps::default(),
        &test_component_metadata(),
    )
    .unwrap();
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
        parsed.diagnostics.is_empty(),
        "parse errors: {:?}",
        parsed.diagnostics
    );
    let struct_statement = parsed
        .program
        .body
        .iter()
        .find_map(|s| match s {
            Statement::StructStatement(st) => Some(st.as_ref()),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(st) => Some(st.as_ref()),
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
        None,
    )
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

#[test]
fn nav_destination_places_children_inside_create_builder() {
    let source = r#"@Component
struct Foo {
  show: boolean = true

  build() {
    NavDestination() {
      Stack() {
        if (this.show) {
          this.buildContentWithEnableHalfPage()
        }
      }
    }
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["buildContentWithEnableHalfPage".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        !out.contains("NavDestination.create(() => {},"),
        "SDK processTabAndNav emits NavDestination children inside the create builder; got:\n{out}"
    );
    let create = out
        .find("NavDestination.create(() => {")
        .expect("NavDestination create builder closure");
    let builder_call = out[create..]
        .find("this.buildContentWithEnableHalfPage.bind(this)()")
        .map(|offset| create + offset)
        .expect("builder method call inside NavDestination create builder");
    let metadata = out[create..]
        .find("}, {")
        .map(|offset| create + offset)
        .expect("NavDestination metadata follows create builder");
    assert!(
        builder_call < metadata,
        "builder method call must be inside NavDestination create builder per SDK processTabAndNav; got:\n{out}"
    );
}

fn parse_and_lower_with_custom_components(
    source: &str,
    custom_components: &[&str],
) -> Result<String, BuildLoweringError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "parse errors: {:?}",
        parsed.diagnostics
    );
    let struct_statement = parsed
        .program
        .body
        .iter()
        .find_map(|s| match s {
            Statement::StructStatement(st) => Some(st.as_ref()),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::StructStatement(st) => Some(st.as_ref()),
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
fn custom_component_with_only_id_uses_retake_aware_common_create() {
    let source = r#"@Component
struct Foo {
  build() {
    MyCard({ title: "hello" }).id("stable-card")
  }
}"#;
    let out = parse_and_lower_with_custom_components(source, &["MyCard"]).unwrap();

    assert!(out.contains("__Common__.create(true);"), "{out}");
    assert!(out.contains("__Common__.id(\"stable-card\");"), "{out}");
}

#[test]
fn custom_component_animation_attribute_uses_context_animation() {
    let source = r#"@Component
struct Foo {
  build() {
    MyCard({ title: "hello" })
      .width("100%")
      .animation({ duration: 200 })
      .height(200)
  }
}"#;
    let out = parse_and_lower_with_custom_components(source, &["MyCard"]).unwrap();
    assert!(
        out.contains("Context.animation({ duration: 200 });"),
        "missing animation context start; got:\n{out}"
    );
    assert!(
        out.contains("Context.animation(null);"),
        "missing animation context reset; got:\n{out}"
    );
    assert!(
        !out.contains("__Common__.animation("),
        "animation must not be emitted through __Common__; got:\n{out}"
    );
    let animation = out.find("Context.animation({ duration: 200 });").unwrap();
    let width = out.find("__Common__.width(\"100%\");").unwrap();
    let clear = out.find("Context.animation(null);").unwrap();
    let height = out.find("__Common__.height(200);").unwrap();
    assert!(
        animation < width && width < clear && clear < height,
        "animation context must wrap the preceding __Common__ segment; got:\n{out}"
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
fn navigation_with_explicit_stack_and_empty_body_marks_user_created_stack() {
    let source = r#"@Component
struct Foo {
  @State stack: NavPathStack = new NavPathStack()

  build() {
    Navigation(this.stack) {
    }
  }
}"#;
    let out =
        parse_and_lower_with_module(source, "features/nav/EmptyNavPage.ets", "entry").unwrap();
    assert!(
        out.contains("Navigation.create(this.stack,"),
        "SDK process_component_build/process_component_class keep Navigation stack create arguments unwrapped; got:\n{out}"
    );
    assert!(
        !out.contains("Navigation.create(ObservedObject.GetRawObject(this.stack)"),
        "Navigation.create must not raw-object wrap its top-level stack argument; got:\n{out}"
    );
    assert!(
        out.contains("isUserCreateStack: true"),
        "Navigation(this.stack) must mark a user-created stack even when the body is empty; got:\n{out}"
    );
}

#[test]
fn navigation_without_stack_synthesizes_nav_path_stack() {
    let source = r#"@Component
struct Foo {
  build() {
    Navigation() {}
  }
}"#;
    let out =
        parse_and_lower_with_module(source, "features/nav/DefaultNavPage.ets", "entry").unwrap();

    assert!(
        out.contains("Navigation.create(new NavPathStack(),"),
        "{out}"
    );
    assert!(out.contains("isUserCreateStack: false"), "{out}");
    assert!(!out.contains("Navigation.create(() => {}"), "{out}");
}

#[test]
fn navigation_nav_destination_binds_page_map_builder_attribute() {
    let source = r#"@Component
struct Foo {
  @State stack: NavPathStack = new NavPathStack()

  @Builder
  PageMap(name: string) {}

  build() {
    Navigation(this.stack) {
    }.navDestination(this.PageMap)
  }
}"#;
    let out = parse_and_lower_with_visible_builder_methods(
        source,
        &FxHashSet::default(),
        false,
        &FxHashSet::from_iter(["PageMap".to_string()]),
        &FxHashSet::default(),
    )
    .unwrap();
    assert!(
        out.contains("Navigation.navDestination({ builder: this.PageMap.bind(this) });"),
        "Navigation.navDestination must receive an hvigor-style builder object; got:\n{out}"
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
        out.contains("globalThis.Gesture.create(GesturePriority.Low);"),
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
        out.contains("globalThis.Gesture.pop();"),
        "Gesture.pop missing; got:\n{out}"
    );
    let inner_pop = out.find("LongPressGesture.pop();").unwrap();
    let outer_pop = out.find("globalThis.Gesture.pop();").unwrap();
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
        out.matches("globalThis.Gesture.create(GesturePriority.Low);")
            .count(),
        2,
        "two Gesture.create; got:\n{out}"
    );
    // Latest process_component_build.js::createFunction emits standalone
    // generated Gesture calls via globalThis, never XxxGesture calls.
    let gesture_pop_count = out
        .match_indices("\n            globalThis.Gesture.pop();")
        .count();
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
        out.contains("globalThis.Gesture.create(GesturePriority.Low);"),
        "Gesture.create missing; got:\n{out}"
    );
    assert!(
        out.contains("globalThis.Gesture.pop();"),
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
    let gesture_pop = out
        .find("globalThis.Gesture.pop();")
        .expect("standalone globalThis.Gesture.pop() not found");
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
        out.matches("globalThis.Gesture.create(GesturePriority.Low);")
            .count(),
        1,
        "one Gesture.create; got:\n{out}"
    );
    // The raw .gesture() call must not appear verbatim.
    assert!(
        !out.contains("Column.gesture("),
        "Column.gesture() must not appear verbatim; got:\n{out}"
    );
}
