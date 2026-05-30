/// Integration tests for `@Builder` and `@Styles` decorator lowering.
///
/// Each test takes representative ETS source input and asserts on the
/// generated TypeScript output, verifying both structural and semantic
/// properties of the lowered code.
///
/// # @Builder body lowering note
///
/// The OXC ETS parser applies `ArkUIComponentExpression` parsing only to the
/// canonical `build()` method — not to `@Builder` method bodies.  In those
/// bodies `Text('hi')` is parsed as a regular TypeScript `CallExpression`.
/// `lower_build_body_or_raw` therefore falls back to emitting the raw body
/// source text re-indented as a method body.  The output is still syntactically
/// valid TypeScript, and the `@Builder` decorator is stripped either way.
/// Full `observeComponentCreation2` desugaring in `@Builder` bodies depends on
/// the OXC ETS parser adding ArkUI expression support there in a future update.
use arkdown_core::{ModuleSource, ModuleType, ResolvedId};
use arkdown_plugin_ets_transform::transform::transform_ets;

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

// ── @Builder ──────────────────────────────────────────────────────────────────

/// `@Builder` decorator is stripped and the method appears in the ViewPU class.
///
/// The OXC ETS parser does not apply ArkUI expression rules to `@Builder`
/// bodies, so the body is emitted as raw TypeScript source text.
#[test]
fn test_builder_method_decorator_stripped_and_method_present() {
    let src = r#"
@Component
struct MyCard {
  @Builder
  headerBuilder() {
    Text('header')
  }
  build() {}
}
"#;
    let result = transform_ets(&make_source(
        "entry/src/main/ets/components/MyCard.ets",
        src,
    ))
    .unwrap();

    // The @Builder decorator must be removed from the output.
    assert!(
        !result.contains("@Builder"),
        "@Builder decorator must be stripped; got:\n{}",
        result
    );
    // The method name is preserved (no __headerBuilder__ rename).
    assert!(
        result.contains("headerBuilder(): void {"),
        "expected 'headerBuilder(): void {{'; got:\n{}",
        result
    );
    // The body content is present (raw fallback path: Text(...) as a call).
    assert!(
        result.contains("Text"),
        "body content must be preserved; got:\n{}",
        result
    );
    // The ViewPU class must be generated.
    assert!(
        result.contains("class MyCard extends ViewPU"),
        "ViewPU class must be generated; got:\n{}",
        result
    );
    // The @Builder method must come after initialRender in the class body.
    let initial_render_pos = result
        .find("initialRender()")
        .expect("missing initialRender");
    let builder_pos = result
        .find("headerBuilder():")
        .expect("missing headerBuilder");
    assert!(
        builder_pos > initial_render_pos,
        "@Builder method must come after initialRender in class body"
    );
}

/// `@Builder` method with parameters: the signature is preserved exactly.
#[test]
fn test_builder_method_with_params_preserves_signature() {
    let src = r#"
@Component
struct Banner {
  @Builder
  labelRow(text: string) {
    Text(text)
  }
  build() {}
}
"#;
    let result = transform_ets(&make_source(
        "entry/src/main/ets/components/Banner.ets",
        src,
    ))
    .unwrap();

    // Full method signature including parameter type must appear.
    assert!(
        result.contains("labelRow(text: string): void {"),
        "expected 'labelRow(text: string): void {{'; got:\n{}",
        result
    );
    // The raw body is forwarded.
    assert!(
        result.contains("text"),
        "body content must reference the parameter; got:\n{}",
        result
    );
    assert!(
        !result.contains("@Builder"),
        "@Builder must be stripped; got:\n{}",
        result
    );
}

/// A component with `@State` fields and a `@Builder` method: both are present.
#[test]
fn test_builder_alongside_state_field() {
    let src = r#"
@Component
struct Counter {
  @State count: number = 0;

  @Builder
  counterDisplay() {
    Text(`${this.count}`)
  }
  build() {}
}
"#;
    let result = transform_ets(&make_source(
        "entry/src/main/ets/components/Counter.ets",
        src,
    ))
    .unwrap();

    assert!(
        result.contains("private __count: ObservedPropertySimplePU<number>"),
        "missing @State backing field; got:\n{}",
        result
    );
    assert!(
        result.contains("counterDisplay(): void {"),
        "missing @Builder method; got:\n{}",
        result
    );
    // Body content preserved.
    assert!(
        result.contains("this.count"),
        "body must reference this.count; got:\n{}",
        result
    );
}

/// Multiple `@Builder` methods: every method appears exactly once.
#[test]
fn test_multiple_builder_methods() {
    let src = r#"
@Component
struct Page {
  @Builder
  header() {
    Text('Header')
  }

  @Builder
  footer() {
    Text('Footer')
  }

  build() {}
}
"#;
    let result =
        transform_ets(&make_source("entry/src/main/ets/components/Page.ets", src)).unwrap();

    assert!(
        result.contains("header(): void {"),
        "first @Builder method missing; got:\n{}",
        result
    );
    assert!(
        result.contains("footer(): void {"),
        "second @Builder method missing; got:\n{}",
        result
    );
    // Both methods' body content appears: Text('Header') and Text('Footer').
    assert!(
        result.contains("'Header'"),
        "header body content missing; got:\n{}",
        result
    );
    assert!(
        result.contains("'Footer'"),
        "footer body content missing; got:\n{}",
        result
    );
}

// ── @Styles ───────────────────────────────────────────────────────────────────

/// `@Styles` decorator is stripped and the method becomes `private`.
/// Attribute chain bodies are lowered via `ComponentInfo` unit-test path to
/// avoid depending on the ETS parser's handling of bare `.attr()` syntax.
#[test]
fn test_styles_method_becomes_private_with_lowered_body() {
    use arkdown_plugin_ets_transform::transform::component::{
        generate_view_pu_class, ComponentInfo, StylesMethodInfo,
    };

    let info = ComponentInfo {
        name: "StyledComp".to_string(),
        styles_methods: vec![StylesMethodInfo {
            name: "myStyle".to_string(),
            params: String::new(),
            lowered_body: concat!("        this.width(100);\n", "        this.height(50);\n",)
                .to_string(),
        }],
        ..Default::default()
    };

    let out = generate_view_pu_class(&info);

    // The method must be private.
    assert!(
        out.contains("private myStyle(): void {"),
        "expected 'private myStyle(): void {{'; got:\n{}",
        out
    );
    // The attribute chain must be lowered to this.attr() calls.
    assert!(
        out.contains("this.width(100);"),
        "expected this.width(100) in @Styles body; got:\n{}",
        out
    );
    assert!(
        out.contains("this.height(50);"),
        "expected this.height(50) in @Styles body; got:\n{}",
        out
    );
    // The @Styles decorator text must not appear.
    assert!(
        !out.contains("@Styles"),
        "@Styles decorator must not appear in output; got:\n{}",
        out
    );
}

/// `@Styles` with multiple attribute calls in the lowered body.
#[test]
fn test_styles_body_with_multiple_attributes() {
    use arkdown_plugin_ets_transform::transform::component::{
        generate_view_pu_class, ComponentInfo, StylesMethodInfo,
    };

    let info = ComponentInfo {
        name: "Comp".to_string(),
        styles_methods: vec![StylesMethodInfo {
            name: "bigStyle".to_string(),
            params: String::new(),
            lowered_body: concat!(
                "        this.fontSize(20);\n",
                "        this.fontColor(Color.Red);\n",
                "        this.width(300);\n",
            )
            .to_string(),
        }],
        ..Default::default()
    };
    let out = generate_view_pu_class(&info);

    assert!(out.contains("private bigStyle(): void {"));
    assert!(out.contains("this.fontSize(20);"));
    assert!(out.contains("this.fontColor(Color.Red);"));
    assert!(out.contains("this.width(300);"));
}

/// `@Styles` method appears after `initialRender` in the class body.
#[test]
fn test_styles_method_position_in_class() {
    use arkdown_plugin_ets_transform::transform::component::{
        generate_view_pu_class, ComponentInfo, StylesMethodInfo,
    };

    let info = ComponentInfo {
        name: "Pos".to_string(),
        styles_methods: vec![StylesMethodInfo {
            name: "card".to_string(),
            params: String::new(),
            lowered_body: "        this.borderRadius(8);\n".to_string(),
        }],
        ..Default::default()
    };
    let out = generate_view_pu_class(&info);

    let initial_pos = out.find("initialRender()").expect("missing initialRender");
    let styles_pos = out.find("private card()").expect("missing @Styles method");
    let rerender_pos = out.find("rerender()").expect("missing rerender");
    assert!(
        initial_pos < styles_pos,
        "@Styles method must come after initialRender"
    );
    assert!(
        styles_pos < rerender_pos,
        "@Styles method must come before rerender"
    );
}

/// Component with both `@Builder` methods and `@Styles` methods: all are present.
#[test]
fn test_builder_and_styles_together() {
    use arkdown_plugin_ets_transform::transform::component::{
        generate_view_pu_class, BuilderMethodInfo, ComponentInfo, StylesMethodInfo,
    };

    let info = ComponentInfo {
        name: "Combined".to_string(),
        builder_methods: vec![BuilderMethodInfo {
            name: "titleArea".to_string(),
            params: String::new(),
            lowered_body: concat!(
                "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
                "            Text.create('title');\n",
                "        }, Text);\n",
                "        Text.pop();\n",
            )
            .to_string(),
        }],
        styles_methods: vec![StylesMethodInfo {
            name: "boxStyle".to_string(),
            params: String::new(),
            lowered_body: "        this.padding(16);\n".to_string(),
        }],
        ..Default::default()
    };
    let out = generate_view_pu_class(&info);

    // @Builder method is present and not private.
    assert!(
        out.contains("titleArea(): void {"),
        "missing @Builder method; got:\n{}",
        out
    );
    assert!(
        !out.contains("private titleArea"),
        "@Builder method must NOT be private; got:\n{}",
        out
    );
    // @Styles method is present and private.
    assert!(
        out.contains("private boxStyle(): void {"),
        "missing @Styles method; got:\n{}",
        out
    );
    // Both bodies are present.
    assert!(out.contains("Text.create('title');"));
    assert!(out.contains("this.padding(16);"));
}

// ── End-to-end TypeScript validity ────────────────────────────────────────────

/// The output of a component with `@Builder` methods must be valid TypeScript.
#[test]
fn test_builder_output_is_valid_typescript() {
    let src = r#"
@Component
struct Widget {
  @Builder
  content() {
    Row() {
      Text('hello')
    }
  }
  build() {}
}
"#;
    let result = transform_ets(&make_source(
        "entry/src/main/ets/components/Widget.ets",
        src,
    ))
    .unwrap();

    // Parse the generated TypeScript with the standard oxc TS parser to
    // ensure the output is syntactically valid.
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.errors.is_empty(),
        "@Builder lowered output must be valid TypeScript: {:?}\n{}",
        parsed.errors,
        result
    );
}
