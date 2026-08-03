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

struct TransformHarness;

impl TransformHarness {
    fn transform(source: &ModuleSource) -> Result<String, EtsTransformError> {
        ArktsTransformer::new(source).transform()
    }

    fn transform_with_options(
        source: &ModuleSource,
        options: &TransformOptions,
    ) -> Result<String, EtsTransformError> {
        ArktsTransformer::with_options(source, options).transform()
    }

    fn collect_component_names(content: &str, id: &str) -> Result<Vec<String>, EtsTransformError> {
        ArkuiComponentNameCollector::new(content, id).collect()
    }

    fn collect_component_names_with_options(
        content: &str,
        id: &str,
        options: &TransformOptions,
    ) -> Result<Vec<String>, EtsTransformError> {
        ArkuiComponentNameCollector::with_options(content, id, options).collect()
    }
}

#[test]
fn test_non_component_passthrough() {
    let src = "import { something } from '@ohos/hilog';\nconst x = 1;";
    let result = TransformHarness::transform(&make_source("test.ets", src)).unwrap();
    assert_eq!(result, src);
}

#[test]
fn test_component_transform_preserves_decimal_literal_spelling() {
    let src = r#"
@Component
struct DecimalPrinter {
  build() {
    Text(`${0.5}`)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/pages/DecimalPrinter.ets",
        src,
    ))
    .expect("component source must transform");

    // Latest ets-loader delegates retained source nodes to TypeScript's
    // printer, which preserves ordinary decimal spellings while emitting
    // generated ViewPU statements around them.
    assert!(result.contains("0.5"), "{result}");
    assert!(!result.contains("${.5}"), "{result}");
}

#[test]
fn test_concurrent_function_inserts_directive_and_strips_decorator() {
    let src = "@Concurrent\nexport async function reportAppStorage(context: Context) {\n  await run(context);\n}";
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/StorageMetricsReporter.ets",
        src,
    ))
    .expect("@Concurrent function must transform like hvigor processConcurrent");

    assert!(result.contains("export async function reportAppStorage"));
    assert!(
        result.contains("\"use concurrent\";"),
        "@Concurrent must become function body directive:\n{result}"
    );
    assert!(
        !result.contains("@Concurrent"),
        "@Concurrent decorator must not leak to rolldown:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "concurrent transform must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_concurrent_function_inside_namespace_inserts_directive() {
    let src = r#"
namespace taskqueue {
  @Concurrent
  function taskWrapper(func: Function, ...args: ESObject[]): ESObject {
    let ret: ESObject = func(...args)
    return ret
  }
}

export default taskqueue
"#;
    let result = TransformHarness::transform(&make_source(
        "oh_modules/@example/concurrent/src/main/ets/TaskQueue.ets",
        src,
    ))
    .expect("namespace @Concurrent function must transform like hvigor processConcurrent");

    assert!(
        result.contains("namespace taskqueue")
            && result.contains("function taskWrapper")
            && result.contains("\"use concurrent\";")
            && !result.contains("@Concurrent"),
        "namespace @Concurrent must lower in-place:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "namespace concurrent transform must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_concurrent_function_inside_export_namespace_inserts_directive() {
    let src = r#"
export namespace FontRegister {
  @Concurrent
  async function registerFontAsync(): Promise<void> {
    await run()
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "oh_modules/@example/mtflexbox/src/main/ets/utils/font/FontRegister.ets",
        src,
    ))
    .expect("export namespace @Concurrent function must transform like hvigor processConcurrent");

    assert!(
        result.contains("export namespace FontRegister")
            && result.contains("async function registerFontAsync")
            && result.contains("\"use concurrent\";")
            && !result.contains("@Concurrent"),
        "export namespace @Concurrent must lower in-place:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "export namespace concurrent transform must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_preserves_arkui_decorators_on_regular_class_like_hvigor() {
    let src = "@Observed\nexport class UserModel {\n  @Trace name: string = 'guest';\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/model/UserModel.ets", src))
            .expect("regular observed class must transform");

    assert!(result.contains("export class UserModel"));
    assert!(result.contains("name: string"));
    assert!(result.contains("@Observed"), "{result}");
    assert!(result.contains("@Trace"), "{result}");

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "hvigor-compatible decorated class must remain valid ArkTS: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_preserves_observed_v2_and_trace_like_hvigor() {
    let src = "@ObservedV2\nexport class Counter {\n  @Trace count: number = 0;\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/model/Counter.ets", src))
            .expect("ObservedV2 class must use the dedicated lowerer");

    assert!(result.contains("export class Counter"));
    assert!(result.contains("@ObservedV2"), "{result}");
    assert!(result.contains("@Trace"), "{result}");
    assert!(!result.contains("@Track"), "{result}");
}

#[test]
fn test_preserves_track_from_oh_modules_regular_class_like_hvigor() {
    let src = "export class KNBWebViewStateTracker {\n  @Track showErrorView: boolean = false;\n  @Track backgroundColor?: string;\n}";
    let result = TransformHarness::transform(&make_source(
        "oh_modules/@knb/core/src/main/ets/runtime/KNBWebViewStateTracker.ets",
        src,
    ))
    .expect("oh_modules @Track checker diagnostics are demoted and syntax is preserved");

    assert!(result.contains("export class KNBWebViewStateTracker"));
    assert!(result.contains("showErrorView: boolean = false;"));
    assert!(result.contains("backgroundColor?: string;"));
    assert!(result.contains("@Track"), "{result}");

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "non-ObservedV2 @Track output must remain valid ArkTS: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_preserves_custom_decorators_on_regular_class_like_hvigor() {
    let src = "@RegisterClass(KEY)\nexport class Model {\n  @memo value: string = 'guest';\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/model/Model.ets", src))
            .expect("regular decorated class must transform");

    let decorator = result.find("@RegisterClass(KEY)").expect("class decorator");
    let export = result.find("export class Model").expect("exported class");
    assert!(
        decorator < export,
        "hvigor keeps ArkTS class decorators before the export keyword:\n{result}"
    );
    assert!(result.contains("@memo"), "{result}");
    assert!(!result.contains("Model = RegisterClass"), "{result}");

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "custom decorated class must remain valid ArkTS: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_preserves_arkui_decorators_on_default_export_class_like_hvigor() {
    let src = "@Observed\nexport default class LazyDataSource<T> {\n  dataArray: T[] = [];\n}";
    let result = TransformHarness::transform(&make_source(
        "commons/utils/src/main/ets/utils/LazyDataSource.ets",
        src,
    ))
    .expect("default export observed class must transform");

    let decorator = result.find("@Observed").expect("class decorator");
    let export = result
        .find("export default class LazyDataSource<T>")
        .expect("default exported class");
    assert!(
        decorator < export,
        "hvigor keeps ArkTS class decorators before the default export keyword:\n{result}"
    );
    assert!(result.contains("dataArray: T[] = [];"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "decorated default export class must remain valid ArkTS: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_component_finalizer_preserves_regular_class_decorator_export_order_like_hvigor() {
    let src = r#"@Observed
export class SwitchModel {
  @Track flag: boolean = false
}

@Component
export struct SwitchView {
  build() {
    Row() {}
  }
}"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/SwitchView.ets",
        src,
    ))
    .expect("component file with a decorated regular class must transform");

    assert!(
        result.contains("@Observed\nexport class SwitchModel"),
        "ViewPU finalization must retain hvigor's line-oriented ArkTS decorator order:\n{result}"
    );
    assert!(!result.contains("export @Observed class"), "{result}");
    assert!(
        result.contains("@Track\n    flag: boolean = false;"),
        "{result}"
    );
}

#[test]
fn test_sendable_class_matches_sdk_process_sendable_class() {
    let src = "@Sendable\nexport class Payload {\n  label?: string;\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/model/Payload.ets", src))
            .expect("@Sendable class must follow SDK process_sendable class lowering");

    assert!(!result.contains("@Sendable"), "{result}");
    assert!(result.contains("constructor()"), "{result}");
    assert!(result.contains("\"use sendable\";"), "{result}");
    assert!(
        result.contains("label: string | undefined = undefined;"),
        "{result}"
    );
}

#[test]
fn test_sendable_derived_class_forwards_constructor_arguments() {
    let src = "@Sendable\nexport class Payload extends BasePayload {}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/model/Payload.ets", src))
            .expect("derived @Sendable class must get hvigor's forwarding constructor");

    assert!(result.contains("constructor(...args)"), "{result}");
    assert!(result.contains("\"use sendable\";"), "{result}");
    assert!(result.contains("super(...args);"), "{result}");
}

#[test]
fn test_transforms_component_struct_with_empty_build() {
    let src = "@Component\nstruct MyView {\n  build() {}\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    assert!(result.contains("class MyView extends ViewPU"));
    assert!(result.contains("initialRender()"));
    assert!(!result.contains("struct MyView"));
}

#[test]
fn test_transforms_export_component_without_leaking_decorator() {
    let src = "import { Foo } from './foo';\n\n@Component\nexport struct MyView {\n  @State message: string = \"hi\";\n  build() {}\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    assert!(result.contains("class MyView extends ViewPU"));
    assert!(
        !result.contains("@Component"),
        "component decorator must be consumed with export struct, got:\n{}",
        result
    );
    assert!(!result.contains("export struct MyView"));
}

#[test]
fn test_hoists_component_params_like_hvigor_interface_node_set() {
    let src = r#"
/*
 * Copyright header
 */
import { Foo } from './foo';

@Component
export struct First {
  @State message: string = '';
  build() {}
}

@Component
struct Second {
  @State count: number = 0;
  build() {}
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Multi.ets", src))
            .unwrap();
    assert!(result.starts_with("if (!(\"finalizeConstruction\" in ViewPU.prototype))"));
    let second = result.find("interface Second_Params").unwrap();
    let first = result.find("interface First_Params").unwrap();
    let import = result.find("import { Foo }").unwrap();
    let first_class = result.find("export class First extends ViewPU").unwrap();
    assert!(second < first);
    assert!(first < import);
    assert!(import < first_class);
    assert!(!result.contains("Copyright header"));
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &result, SourceType::ts().with_module(true)).parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "hoisted ViewPU output must remain valid TypeScript:\n{result}"
    );
}

#[test]
fn test_transforms_default_export_component() {
    let src = "@Component\nexport default struct MyView {\n  build() {}\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    assert!(result.contains("export default class MyView extends ViewPU"));
    assert!(!result.contains("export default struct MyView"));
}

#[test]
fn test_regular_field_initializer_does_not_run_before_state_backing_fields() {
    let src = r#"
@Component
struct WebShell {
  @Prop webTag: string = '';
  controller = new WebController(this.webTag);
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/WebShell.ets",
        src,
    ))
    .unwrap();
    assert_eq!(
        result.matches("new WebController(this.webTag)").count(),
        1,
        "regular field initializer must only be emitted in the constructor:\n{result}"
    );
    let prop_init = result
        .find("this.__webTag = new SynchedPropertySimpleOneWayPU")
        .unwrap();
    let regular_init = result
        .find("this.controller = new WebController(this.webTag)")
        .unwrap();
    assert!(
        prop_init < regular_init,
        "regular initializer must run after the @Prop backing slot exists:\n{result}"
    );
}

#[test]
fn test_static_component_property_is_not_lowered_to_instance_field() {
    let src = r#"
@Component
struct RecReasonComponent {
  private static readonly FONT_SIZE: number = 12;
  private static readonly FONT_WEIGHT: number = 400;
  private componentWidth: number = 0;

  updateValidDataList() {
    getTextWidth("x", RecReasonComponent.FONT_SIZE, RecReasonComponent.FONT_WEIGHT);
  }

  build() {
    Text("x")
      .fontSize(RecReasonComponent.FONT_SIZE)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/RecReasonComponent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("static readonly FONT_SIZE: number = 12"),
        "hvigor keeps static struct properties as static class properties:\n{result}"
    );
    assert!(
        result.contains("static readonly FONT_WEIGHT: number = 400"),
        "hvigor keeps all static struct properties out of constructor lowering:\n{result}"
    );
    assert!(
        !result.contains("this.FONT_SIZE = 12"),
        "static properties must not become instance constructor assignments:\n{result}"
    );
    assert!(
        !result.contains("FONT_SIZE?:"),
        "static properties must not be added to the generated params interface:\n{result}"
    );
    assert!(
        result.contains(
            "getTextWidth(\"x\", RecReasonComponent.FONT_SIZE, RecReasonComponent.FONT_WEIGHT)"
        ),
        "static property reads must continue to resolve through the class:\n{result}"
    );
}

#[test]
fn test_component_preserves_new_oxc_struct_member_variants() {
    let src = r#"
@Component
struct SyntaxMembers {
  static { SyntaxMembers.initialized = true; }
  [key: string]: unknown;
  accessor current: number = 1;
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/SyntaxMembers.ets",
        src,
    ))
    .expect("OXC struct member variants must lower to equivalent class members");

    assert!(
        result.contains("static {"),
        "static blocks must survive component lowering:\n{result}"
    );
    assert!(
        result.contains("[key: string]: unknown"),
        "index signatures must survive component lowering:\n{result}"
    );
    assert!(
        result.contains("accessor current: number = 1"),
        "auto-accessor properties must survive component lowering:\n{result}"
    );
}

#[test]
fn test_component_v2_preserves_new_oxc_struct_member_variants() {
    let src = r#"
@ComponentV2
struct SyntaxMembersV2 {
  static { SyntaxMembersV2.initialized = true; }
  [key: string]: unknown;
  accessor current: number = 1;
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/SyntaxMembersV2.ets",
        src,
    ))
    .expect("OXC struct member variants must lower to equivalent ViewV2 class members");

    assert!(result.contains("class SyntaxMembersV2 extends ViewV2"));
    assert!(result.contains("static {"));
    assert!(result.contains("[key: string]: unknown"));
    assert!(result.contains("accessor current: number = 1"));
}

#[test]
fn test_transforms_component_v2_property_decorators() {
    let src = "@ComponentV2\nstruct MyView {\n  @Local count: number = 0;\n  @Require\n  @Param title: string = '';\n  @Provider('theme') theme: string = 'dark';\n  @Consumer('theme') incoming: string = '';\n  @Event onTap: () => void = () => {};\n  @Computed\n  get label(): string { return this.title; }\n  @Monitor('count')\n  onCountChange(): void {}\n  build() {}\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    assert!(result.contains("class MyView extends ViewV2"));
    assert!(result.contains("@Local"));
    assert!(result.contains("count: number;"));
    assert!(result.contains("this.count = 0;"));
    assert!(result.contains("@Param"));
    assert!(result.contains("title: string;"));
    assert!(result.contains("this.initParam(\"title\""));
    assert!(result.contains("this.updateParam(\"title\", params.title);"));
    assert!(result.contains("this.resetParam(\"title\""));
    assert!(result.contains("@Provider('theme')"));
    assert!(result.contains("theme: string;"));
    assert!(result.contains("this.theme = 'dark';"));
    assert!(result.contains("@Consumer('theme')"));
    assert!(result.contains("incoming: string;"));
    assert!(
        result.contains("this.resetConsumer(\"incoming\", '');"),
        "@Consumer reset must preserve the source literal spelling:\n{result}"
    );
    assert!(result.contains("@Event"));
    assert!(result.contains("onTap: () => void;"));
    assert!(result.contains("this.onTap ="));
    assert!(result.contains("\"onTap\" in params"));
    assert!(result.contains("params.onTap"));
    assert!(result.contains("this.resetComputed(\"label\");"));
    assert!(result.contains("this.resetMonitorsOnReuse();"));
    assert!(!result.contains("@ComponentV2"));
    assert!(!result.contains("@Require"));
    assert!(!result.contains("@Computed"));
    assert!(!result.contains("@Monitor"));
    assert!(!result.contains("SyncedProperty"));
    assert!(!result.contains("EventEmitter"));
    assert!(!result.contains("__initializeStruct"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "ComponentV2 property output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_component_v2_rejects_non_ui_build_statements_like_hvigor() {
    for statement in [
        "let x = 1;",
        "for (let i = 0; i < 1; i++) {}",
        "switch (1) { default: break; }",
        "try {} catch (err) {}",
        "return;",
    ] {
        let src =
            format!("@ComponentV2\nstruct BadView {{\n  build() {{\n    {statement}\n  }}\n}}");
        let err =
            TransformHarness::transform(&make_source("entry/src/main/ets/pages/BadView.ets", &src))
                .expect_err("hvigor rejects non-UI statements in build()");
        assert!(
            err.to_string()
                .contains("Only UI component syntax can be written here."),
            "unexpected error for `{statement}`: {err}"
        );
    }
}

#[test]
fn test_component_v2_skips_direct_build_block_but_lowers_if_block_like_hvigor() {
    let src = r#"@ComponentV2
struct MyView {
  build() {
    {
      Text('skipped')
    }
    if (true) {
      Text('kept')
    }
  }
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .expect("direct build block and if block must lower with hvigor block semantics");

    assert!(
        !result.contains("Text.create('skipped')"),
        "direct build block must be skipped like hvigor processComponentChild:\n{result}"
    );
    assert!(
        result.contains("Text.create('kept')"),
        "if branch block must still lower ArkUI children:\n{result}"
    );
}

#[test]
fn test_component_v2_reusable_v2_emits_hvigor_inner_decorator_once() {
    let src = r#"@ReusableV2
@ComponentV2
struct FirstCell {
  build() {}
}

@ReusableV2
@ComponentV2
struct SecondCell {
  build() {}
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Cells.ets", src))
            .expect("@ReusableV2 @ComponentV2 must lower to hvigor reusable V2 marker");

    assert_eq!(
        result
            .matches("function __ReusableV2_Inner_Decorator__(BaseClass)")
            .count(),
        1,
        "reusable V2 helper must be emitted once per file:\n{result}"
    );
    assert_eq!(
        result.matches("@__ReusableV2_Inner_Decorator__").count(),
        2,
        "each reusable V2 class must carry the inner decorator:\n{result}"
    );
    assert!(
        result.contains("Reflect.defineProperty(BaseClass.prototype, \"isReusable_\""),
        "helper must define BaseClass.prototype.isReusable_ like hvigor:\n{result}"
    );
    assert!(!result.contains("@ReusableV2"));
    assert!(!result.contains("@ComponentV2"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered ReusableV2 output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_reusable_v2_call_uses_sdk_reuse_or_create_protocol() {
    let src = r#"
@ReusableV2
@ComponentV2
struct ReusableCell {
  @Param value: string = "";
  build() {
    Text(this.value)
  }
}

@ComponentV2
struct Host {
  @Local value: string = "value";
  build() {
    ReusableCell({ value: this.value })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/pages/ReusableCell.ets",
        src,
    ))
    .expect("@ReusableV2 call site must lower to the SDK reuse protocol");

    assert!(
        result.contains("this.reuseOrCreateNewComponent({")
            && result.contains("componentClass: ReusableCell")
            && result.contains("getParams: () => ({ value: this.value })")
            && result.contains("getReuseId: () => \"ReusableCell\"")
            && result.contains(
                "extraInfo: { page: \"entry/src/main/ets/pages/ReusableCell.ets\""
            ),
        "SDK process_custom_component.createCustomComponent uses reuseOrCreateNewComponent for @ReusableV2 children:\n{result}"
    );
    assert_eq!(
        result.matches("ViewV2.create(componentCall);").count(),
        0,
        "a reusable V2 child call must not allocate through the ordinary ViewV2 observer path:\n{result}"
    );
}

#[test]
fn test_component_v2_rejects_once_without_param_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @Once title: string = '';
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("@Once without @Param must not lower as a standalone ComponentV2 param");
    assert!(
        err.to_string().contains(
            "When a variable decorated with '@Once', it must also be decorated with '@Param'."
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_rejects_param_without_default_or_require_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @Param title: string;
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("@Param without initializer or @Require must match hvigor ComponentV2 error");
    assert!(
        err.to_string().contains(
            "When a variable decorated with '@Param' is not assigned a default value, it must also be decorated with '@Require'."
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_rejects_require_without_param_or_builder_param_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @Require
  count: number;
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("@Require must only combine with @Param/@BuilderParam in ComponentV2");
    assert!(
        err.to_string().contains(
            "In a struct decorated with '@ComponentV2', '@Require' can only be used with '@Param' and '@BuilderParam'."
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_rejects_decorated_property_without_type_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @Local count = 0;
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("ComponentV2 decorated properties must specify a type like hvigor");
    assert!(
        err.to_string()
            .contains("The property 'count' must specify a type."),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_rejects_builder_param_non_builder_initializer_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @BuilderParam content: () => void = () => {};
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("@BuilderParam initializer must be an @Builder function like hvigor");
    assert!(
        err.to_string()
            .contains("'@BuilderParam' property can only initialized by '@Builder' function."),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_allows_builder_param_builder_initializer() {
    let src = r#"@ComponentV2
struct MyView {
  @BuilderParam content: () => void = this.empty;
  @Builder
  empty() {}
  build() {}
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .expect("@BuilderParam may be initialized by a @Builder method");
    assert!(
        result.contains("this.content = \"content\" in params ? params.content : this.empty;"),
        "@BuilderParam initializer must be preserved as direct assignment:\n{result}"
    );
}

#[test]
fn test_component_v2_lowers_build_and_builder_bodies() {
    let src = r#"@ComponentV2
struct MyView {
  @Param title: string = '';

  @Builder
  header() {
    Row() {
      Text(this.title)
    }
  }

  build() {
    Column() {
      this.header()
    }
    .width('100%')
  }
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    assert!(result.contains("class MyView extends ViewV2"));
    assert!(result.contains("observeComponentCreation2"));
    assert!(result.contains("Column.create();"));
    assert!(result.contains("Row.create();"));
    assert!(result.contains("Text.create(this.title);"));
    assert!(result.contains("Column.width('100%');"));
    assert!(
        !result.contains("Column() {") && !result.contains("Row() {"),
        "@ComponentV2 build/@Builder bodies must not leak ArkUI block DSL:\n{result}"
    );
    assert!(!result.contains("@Builder"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered ComponentV2 output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_component_v2_lowers_repeat_each_body_like_hvigor() {
    let src = r#"
type Item = { title: string };

@ComponentV2
struct MyView {
  @Param items: Item[] = [];

  build() {
    Column() {
      Repeat<Item>(this.items).each((repeatItem: RepeatItem<Item>) => {
        Text(repeatItem.item.title)
      })
      .templateId((item: Item, index: number) => {
        return item.title
      })
      .template('card', (repeatItem: RepeatItem<Item>) => {
        Row() {
          Text(repeatItem.item.title)
        }
      }, { cachedCount: 5 })
      .key((item: Item, index: number) => {
        return item.title
      })
      .virtualScroll({ totalCount: this.items.length })
    }
  }
}
"#;
    let component_metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata");
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(component_metadata),
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &options,
    )
    .expect("ComponentV2 Repeat.each must lower like hvigor processRepeatComponent");

    assert!(
        result.contains("Repeat<Item>(this.items, this).each"),
        "Repeat base call must receive this like hvigor:\n{result}"
    );
    assert!(
        result.contains(".render(isInitialRender);"),
        "Repeat chain must render inside observeComponentCreation2:\n{result}"
    );
    assert!(
        result.contains("Text.create(repeatItem.item.title);"),
        "Repeat.each callback ArkUI body must be lowered:\n{result}"
    );
    assert!(
        result.contains(".template('card', (repeatItem: RepeatItem<Item>) => {")
            && result.contains("Row.create();"),
        "Repeat.template callback ArkUI body must be lowered while preserving chain:\n{result}"
    );
    assert!(
        !result.contains("Text(repeatItem.item.title)")
            && !result.contains("Repeat<Item>(this.items).each"),
        "raw Repeat/ArkUI DSL must not leak after transform:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered Repeat output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_component_v2_lowers_lazy_for_each_call_body() {
    let src = r#"
type Item = { title: string };

@ComponentV2
struct MyView {
  @Param items: Item[] = [];
  @Param enabled: boolean = true;

  build() {
    if (this.enabled) {
      LazyForEach(this.items, (item: Item, index: number) => {
        FlowItem() {
          Text(item.title)
        }
      })
    }
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .expect("LazyForEach call callback must lower like hvigor processForEachComponentNew");

    assert!(result.contains("LazyForEach.create("), "got:\n{result}");
    assert!(result.contains("FlowItem.create();"), "got:\n{result}");
    assert!(
        result.contains("Text.create(item.title);"),
        "got:\n{result}"
    );
    assert!(
        !result.contains("FlowItem()") && !result.contains("Text(item.title)"),
        "raw LazyForEach callback ArkUI DSL must not leak:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered LazyForEach output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_component_v2_lowers_styles_method_body() {
    let src = r#"
@ComponentV2
struct MyView {
  @Styles
  cardStyle() {
    .padding({ left: 8 })
      .backgroundColor(Color.White)
      .borderRadius(10)
  }

  build() {
    Text('title').cardStyle()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .expect("@ComponentV2 @Styles methods must lower leading-dot chains");

    assert!(
        result.contains("Text.padding({ left: 8 });")
            && result.contains("Text.backgroundColor(Color.White);")
            && result.contains("Text.borderRadius(10);"),
        "got:\n{result}"
    );
    assert!(
        !result.contains("Text.cardStyle()")
            && !result.contains("cardStyle() {")
            && !result.contains("\n    .padding"),
        "SDK must expand and remove local @Styles declarations:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered @Styles output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_component_styles_expand_at_attribute_call_and_are_removed_from_class() {
    let src = r#"
@Component
struct StyledCard {
  @State inset: number = 8

  @Styles
  cardStyle() {
    .padding({ left: this.inset })
      .backgroundColor(Color.White)
      .borderRadius(10)
  }

  build() {
    Text('title')
      .cardStyle()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/StyledCard.ets", src))
            .expect("SDK INNER_STYLE_FUNCTION calls must expand into component attributes");

    assert!(
        result.contains("Text.padding({ left: this.inset });"),
        "local @Styles must inline its first attribute:\n{result}"
    );
    assert!(
        result.contains("Text.backgroundColor(Color.White);")
            && result.contains("Text.borderRadius(10);"),
        "local @Styles chained attributes must preserve declaration order:\n{result}"
    );
    assert!(
        !result.contains("Text.cardStyle()") && !result.contains("cardStyle() {"),
        "SDK removes local @Styles declarations after expanding their calls:\n{result}"
    );
}

#[test]
fn test_top_level_styles_expand_and_declaration_is_removed() {
    let src = r#"
@Styles
function sharedCardStyle() {
  .padding(12)
    .backgroundColor(Color.White)
}

@Component
struct StyledCard {
  build() {
    Row() {
      Text('title')
    }
    .sharedCardStyle()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/StyledCard.ets", src))
            .expect("SDK GLOBAL_STYLE_FUNCTION calls must expand into component attributes");

    assert!(
        result.contains("Row.padding(12);") && result.contains("Row.backgroundColor(Color.White);"),
        "top-level @Styles attributes must expand at the call site:\n{result}"
    );
    assert!(
        !result.contains("Row.sharedCardStyle()") && !result.contains("function sharedCardStyle()"),
        "SDK removes top-level @Styles declarations after indexing them:\n{result}"
    );
}

#[test]
fn test_top_level_styles_expand_for_lazy_list_item_creation() {
    let src = r#"
@Styles
function itemStyle() {
  .padding(12)
    .borderRadius(8)
}

@Component
struct StyledList {
  build() {
    List() {
      ListItem() {
        Text('row')
      }
      .itemStyle()
    }
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/StyledList.ets", src))
            .expect("SDK lazy ListItem creation must expand top-level @Styles");

    assert!(
        result.contains("ListItem.padding(12);") && result.contains("ListItem.borderRadius(8);"),
        "lazy ListItem must receive expanded style attributes:\n{result}"
    );
    assert!(
        !result.contains("ListItem.itemStyle()"),
        "raw @Styles call must not leak through lazy ListItem creation:\n{result}"
    );
}

#[test]
fn test_component_source_methods_keep_hvigor_member_order() {
    let src = r#"
@Component
struct OrderedPage {
  aboutToAppear() {}

  @Builder
  contentArea() {
    Text('content')
  }

  build() {
    this.contentArea()
  }

  afterBuildMethod() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/pages/OrderedPage.ets",
        src,
    ))
    .expect("component members must lower");
    let about = result.find("aboutToAppear()").expect("aboutToAppear");
    let builder = result
        .find("contentArea(parent = null)")
        .expect("@Builder method");
    let initial = result.find("initialRender()").expect("initialRender");
    let after = result
        .find("afterBuildMethod()")
        .expect("method after source build");

    assert!(
        about < builder && builder < initial && initial < after,
        "SDK processMembers transforms members in source order:\n{result}"
    );
}

#[test]
fn test_component_v2_source_methods_keep_hvigor_member_order() {
    let src = r#"
@ComponentV2
struct OrderedPage {
  @Param count: number = 0;

  beforeBuildMethod() {}

  @Builder
  contentArea() {
    Text('content')
  }

  build() {
    this.contentArea()
  }

  afterBuildMethod() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/pages/OrderedPageV2.ets",
        src,
    ))
    .expect("ComponentV2 members must lower");
    let before = result
        .find("beforeBuildMethod()")
        .expect("method before source build");
    let builder = result
        .find("contentArea(parent = null)")
        .expect("@Builder method");
    let initial = result.find("initialRender()").expect("initialRender");
    let after = result
        .find("afterBuildMethod()")
        .expect("method after source build");
    let update = result
        .find("updateStateVars(params)")
        .expect("synthetic updateStateVars");
    let rerender = result.find("rerender()").expect("synthetic rerender");

    assert!(
        before < builder
            && builder < initial
            && initial < after
            && after < update
            && update < rerender,
        "SDK processStructMembersV2 preserves source members before synthetic suffixes:\n{result}"
    );
}

#[test]
fn test_if_branch_component_id_uses_hvigor_can_retake_guard() {
    let src = r#"
@Component
struct Child {
  build() {
    Text('child')
  }
}

@Component
struct Parent {
  @State visible: boolean = true;

  build() {
    if (this.visible) {
      Column() {
        Text('content')
      }
      .id('stable-column')
      Child().id('stable-child')
    }
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/RetakePage.ets", src))
            .expect("if branch component IDs must lower");

    assert!(
        result.contains("if (!If.canRetake('stable-column'))")
            && result.contains("if (!If.canRetake('stable-child'))"),
        "SDK process_component_build.ifRetakeId must guard system and custom components in If branches:\n{result}"
    );
    assert_eq!(
        result.matches("If.canRetake(").count(),
        2,
        "only source components carrying .id(...) should receive retake guards:\n{result}"
    );
}

#[test]
fn test_global_animate_calls_rewrite_to_context_for_entire_ets_program() {
    let src = r#"
export function animateLater(): void {
  animateTo({ duration: 120 }, () => {});
}

export function animateNow(): void {
  animateToImmediately({ duration: 0 }, () => {});
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/utils/AnimationUtils.ets",
        src,
    ))
    .expect("SDK global animation calls must lower without requiring a component");

    assert!(
        result.contains("Context.animateTo({ duration: 120 }, () =>")
            && result.contains("Context.animateToImmediately({ duration: 0 }, () =>"),
        "SDK processAnimateToOrImmediately must rewrite both global APIs across the ETS program:\n{result}"
    );
    assert!(
        !result.contains("\n    animateTo(") && !result.contains("\n    animateToImmediately("),
        "unqualified animation globals must not remain after UI syntax transform:\n{result}"
    );
}

#[test]
fn test_component_v2_lifecycle_methods_emitted_with_decorators_stripped() {
    // @ComponentV2 methods decorated with @ComponentAppear, @ComponentDisappear etc.
    // must be emitted into the ViewV2 class with their lifecycle decorators stripped.
    // The ArkRuntime registers them at load time — no decorator syntax needed.
    let src = r#"@ComponentV2
struct MyView {
  @Local count: number = 0;
  @ComponentAppear
  onAppear(): void { this.count = 1; }
  @ComponentDisappear
  onDisappear(): void { this.count = 0; }
  build() {}
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .unwrap();
    // Class should be generated
    assert!(
        result.contains("class MyView extends ViewV2"),
        "class not found: {result}"
    );
    // Lifecycle methods must be preserved in the output
    assert!(result.contains("onAppear"), "onAppear missing: {result}");
    assert!(
        result.contains("onDisappear"),
        "onDisappear missing: {result}"
    );
    // Lifecycle decorators must be stripped
    assert!(
        !result.contains("@ComponentAppear"),
        "@ComponentAppear must be stripped: {result}"
    );
    assert!(
        !result.contains("@ComponentDisappear"),
        "@ComponentDisappear must be stripped: {result}"
    );
}

#[test]
fn test_component_v2_rejects_lifecycle_method_params_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @ComponentAppear
  onAppear(value: number): void {}
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("ComponentV2 lifecycle methods with params must match hvigor syntax error");
    assert!(
        err.to_string()
            .contains("Methods decorated with '@ComponentAppear' cannot have input parameters."),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_rejects_component_reuse_params_even_when_checker_off() {
    let src = r#"@ComponentV2
struct MyView {
  @ComponentReuse
  reuse(params: Record<string, Object>): void {}
  build() {}
}"#;
    let err = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/MyView.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect_err("ComponentV2 @ComponentReuse must not accept params");
    assert!(
        err.to_string().contains(
            "Methods decorated with '@ComponentReuse' in '@ComponentV2' cannot have input parameters."
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn test_component_v2_sync_monitor_does_not_reset_monitors_without_monitor() {
    let src = r#"@ComponentV2
struct MyView {
  @Local count: number = 0;
  @SyncMonitor('count')
  onCountChange(): void {}
  build() {}
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/MyView.ets", src))
            .expect("@SyncMonitor method must lower without monitor reset side effect");
    assert!(
        !result.contains("this.resetMonitorsOnReuse();"),
        "@SyncMonitor alone must not trigger resetMonitorsOnReuse like hvigor:\n{result}"
    );
}

#[test]
fn test_component_v2_plain_helper_method_emitted() {
    // Non-decorated helper methods must also be emitted unchanged.
    let src = r#"@ComponentV2
struct Card {
  @Local title: string = '';
  getLabel(): string { return this.title + '!'; }
  build() {}
}"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Card.ets", src))
            .unwrap();
    assert!(
        result.contains("class Card extends ViewV2"),
        "class: {result}"
    );
    assert!(
        result.contains("getLabel"),
        "helper method missing: {result}"
    );
}

#[test]
fn test_rejects_struct_without_component_decorator_like_hvigor_checker() {
    let src = "struct Plain {\n  build() {}\n}";
    let err = TransformHarness::transform(&make_source("entry/src/main/ets/pages/Plain.ets", src))
        .expect_err("plain ArkTS struct must fail in etsChecker");
    assert!(
        err.to_string()
            .contains("Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_checker_off_skips_ets_checker_diagnostics() {
    let src = "struct Plain {\n  build() {}\n}";
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Plain.ets", src),
        &TransformOptions {
            checker_mode: CheckerMode::Off,
            ..Default::default()
        },
    )
    .expect("checker=off must skip etsChecker diagnostics");
    assert!(result.contains("struct Plain") || result.contains("class Plain"));
}

#[test]
fn test_rejects_component_v1_v2_decorator_mix() {
    let src = "@Component\n@ComponentV2\nstruct Mixed {\n  build() {}\n}";
    let err = TransformHarness::transform(&make_source("entry/src/main/ets/pages/Mixed.ets", src))
        .expect_err("@Component and @ComponentV2 must be mutually exclusive");
    assert!(
        err.to_string()
            .contains("can not be decorated with '@ComponentV2'"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_lowers_arkui_nodes_but_preserves_hvigor_lazy_import() {
    let src = r#"
@interface RouteMeta {
  value: string;
  minApiVersion: string;
}

import lazy LazyPage, { Helper } from './lazy';

@Component
struct Page {
  @State title: string = 'hello';
  build() {
    Column() {
      Text(this.title)
    }
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Page.ets", src))
            .unwrap();
    assert!(result.contains("interface RouteMeta"));
    assert!(result.contains("import lazy LazyPage, { Helper } from './lazy';"));
    assert!(result.contains("class Page extends ViewPU"));
    assert!(!result.contains("@interface"));
    assert!(!result.contains("struct Page"));
    assert!(!result.contains("Column() {"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &result, oxc_span::SourceType::ets()).parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered output must be valid hvigor ArkTS: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_transforms_export_builder_function_decorator() {
    let src =
        "@Builder\nexport function Card(title: string) {\n  Column() {\n    Text(title)\n  }\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Card.ets", src))
            .unwrap();
    assert!(result.contains("export function Card(title: string, parent = null)"));
    assert!(
        result.contains("const __title__ = title;")
            && result.contains(
                "(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, title = __title__) => {"
            ),
        "SDK processUISyntax/processComponentBlock must preserve the top-level builder receiver and single-parameter capture:\n{result}"
    );
    assert!(!result.contains("@Builder"));
    assert!(!result.contains("Column() {"));
    assert!(
        !result.contains("NavigationBuilderRegister"),
        "an ordinary global @Builder must not be registered without a routerMap source record:\n{result}"
    );
}

#[test]
fn test_router_map_registers_only_the_source_local_global_builder_like_hvigor() {
    let source_id = "entry/src/main/ets/pages/NavigationPage.ets";
    let src = r#"
@Builder
export function PageBuilder(): void {
  Text("page")
}
"#;
    let options = TransformOptions {
        navigation_routes: NavigationRouteMap::from_routes([NavigationRoute::new(
            source_id,
            "NavigationPage",
            "PageBuilder",
        )]),
        ..Default::default()
    };

    let result = TransformHarness::transform_with_options(&make_source(source_id, src), &options)
        .expect("router-map global @Builder must transform");

    assert!(
        result.contains("if (typeof NavigationBuilderRegister === \"function\")"),
        "SDK createNavigationInit must retain its runtime availability guard:\n{result}"
    );
    assert!(
        result.contains("NavigationBuilderRegister(\"NavigationPage\", wrapBuilder(PageBuilder));"),
        "SDK createNavigationRegister must preserve the routerMap name and builder function:\n{result}"
    );
    assert!(
        result.trim_end().ends_with("})();"),
        "SDK appends the guarded navigation registration IIFE after transformed source:\n{result}"
    );
}

#[test]
fn test_router_map_missing_local_builder_is_a_typed_hvigor_parity_error() {
    let source_id = "entry/src/main/ets/pages/NavigationPage.ets";
    let src = r#"
@Builder
export function DifferentBuilder(): void {
  Text("page")
}
"#;
    let options = TransformOptions {
        navigation_routes: NavigationRouteMap::from_routes([NavigationRoute::new(
            source_id,
            "NavigationPage",
            "MissingBuilder",
        )]),
        ..Default::default()
    };

    let error = TransformHarness::transform_with_options(&make_source(source_id, src), &options)
        .expect_err("routerMap must reject an unavailable source-local @Builder");
    assert!(matches!(error, EtsTransformError::Fatal { .. }));
    assert!(
        error.to_string().contains(
            "The buildFunction 'MissingBuilder' configured in the routerMap json file does not exist."
        ),
        "must match ets-loader process_ui_syntax.js::createNavigationInit diagnostic: {error}"
    );
}

#[test]
fn test_global_builder_custom_component_common_wrapper_matches_hvigor_block_shape() {
    let src = r#"
@Component
struct Child {
  build() {
    Text("child")
  }
}

@Builder
export function Card(title: string) {
  Child()
    .width(60)
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Card.ets", src))
            .unwrap();
    assert!(
        result.contains("__Common__.width(60);")
            && result.contains(
                "    {\n        (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, title = __title__) => {"
            ),
        "SDK processCustomComponent appends the block returned by createCustomComponent directly:\n{result}"
    );
    assert!(
        !result.contains(
            "    {\n        {\n            (parent ? parent : this).observeComponentCreation2"
        ),
        "custom component common attributes must not add a second block around createCustomComponent:\n{result}"
    );
}

#[test]
fn test_top_level_builder_preserves_explicit_return_type() {
    let src = "@Builder\nexport function Card(): void {\n  Text('card')\n}";
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Card.ets", src))
            .unwrap();
    assert!(result.contains("export function Card(parent = null): void"));
}

#[test]
fn test_global_builder_trailing_lambda_uses_lexical_this_like_hvigor() {
    let src = r#"
@Component
struct Wrapper {
  @BuilderParam content: () => void;
  build() {
    this.content()
  }
}

@Builder
function GlobalBuilder(title: string, subtitle: string) {
  Wrapper() {
    Text(title + subtitle)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/Global.ets",
        src,
    ))
    .unwrap();
    assert_eq!(
        result
            .matches("this.observeComponentCreation2((elmtId, isInitialRender) => {")
            .count(),
        2,
        "SDK processBlockToExpression lowers both constructor and params-lambda trailing bodies against lexical this:\n{result}"
    );
    assert!(
        !result.contains(
            "content: () => {\n                (parent ? parent : this).observeComponentCreation2"
        ),
        "a custom component trailing lambda must not inherit the global builder parent receiver:\n{result}"
    );
}

#[test]
fn test_global_builder_terminal_empty_if_branch_uses_lexical_this_like_hvigor() {
    let src = r#"
@Builder
function GlobalBuilder(show: boolean) {
  if (show) {
    Text("shown")
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/Global.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("(parent ? parent : this).ifElseBranchUpdateFunction(0, () => {"),
        "source-backed global-builder branches use createConditionParent():\n{result}"
    );
    assert!(
        result.contains("this.ifElseBranchUpdateFunction(1, () => {"),
        "SDK processElseStatement synthesizes its empty branch against lexical this:\n{result}"
    );
}

#[test]
fn test_builder_param_backing_field_preserves_required_marker() {
    let src = r#"
@Component
struct Panel {
  @BuilderParam required: () => void;
  @BuilderParam optional?: () => void;

  build() {
    Column() {}
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Panel.ets", src))
            .unwrap();
    assert!(
        result.contains("private __required;") && !result.contains("private __required?;"),
        "required @BuilderParam backing field must remain required like SDK createPropertyDeclaration:\n{result}"
    );
    assert!(
        result.contains("private __optional?;"),
        "optional @BuilderParam backing field must preserve the source question token:\n{result}"
    );
}

#[test]
fn test_transforms_builder_before_export_function() {
    let src = "@Builder\nexport function LearningItem(learningItem: LearningPath) {\n  Column() {\n    Image(learningItem.thumbnail)\n      .width(LearningConstants.FULL_PERCENT)\n  }\n  .clip(true)\n}";
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/LearningItem.ets",
        src,
    ))
    .unwrap();
    assert!(!result.contains("@Builder"));
    assert!(!result.contains("Column() {"));
    assert!(result.contains(".clip(true);"));
}

#[test]
fn test_extend_functions_use_source_function_name_like_hvigor() {
    let src = r#"
@Extend(Text)
function warningText() {
  .fontColor(Color.Red)
}

@Extend(Text)
function primaryText() {
  .fontSize(20)
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/ExtendPage.ets", src))
            .expect("@Extend functions must transform");

    assert!(
        result.contains("function __Text__warningText()"),
        "@Extend helper must include source function name:\n{result}"
    );
    assert!(
        result.contains("function __Text__primaryText()"),
        "@Extend helper must include source function name:\n{result}"
    );
    assert!(
        !result.contains("__Text__Extend"),
        "@Extend helper must not use decorator name as function identity:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered @Extend output must be valid TypeScript: {:?}\n{result}",
        parsed.diagnostics
    );
}

#[test]
fn test_extend_attribute_call_uses_generated_helper_like_hvigor() {
    let src = r#"
@Extend(Text)
function textExtend(action: () => void) {
  .fontSize(13)
  .fontColor('#575859')
  .height('100%')
  .onVisibleAreaChange([0], (isVisible: boolean) => {
    if (isVisible) {
      action()
    }
  })
}

@Component
struct SearchBarItem {
  onKeywordLxViewAction: () => void

  build() {
    Text('keyword')
      .textExtend(this.onKeywordLxViewAction)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/SearchBarItem.ets",
        src,
    ))
    .expect("@Extend attribute invocation must transform");

    assert!(
        result.contains("__Text__textExtend(this.onKeywordLxViewAction);"),
        "SDK process_component_build.js::isExtendFunctionNode rewrites the attribute to the generated helper:\n{result}"
    );
    assert!(
        !result.contains("Text.textExtend("),
        "@Extend attributes do not exist on the runtime component object:\n{result}"
    );
    assert!(
        result.contains("Text.fontSize(13);\n    Text.fontColor('#575859');\n    Text.height('100%');"),
        "SDK process_component_build.js emits each @Extend attribute as an independent component call:\n{result}"
    );
    assert!(
        !result.contains("Text.fontSize(13).fontColor"),
        "ArkUI attribute calls cannot be chained by their JavaScript return values:\n{result}"
    );
}

#[test]
fn test_keeps_regular_method_body_before_top_level_builder() {
    let src = r#"
@Component
export struct ConfirmView {
  @Consume('appPathStack') appPathStack: NavPathStack;
  @State uri: string = '';
  @StorageProp('currentBreakpoint') currentBreakpoint: string = BreakpointTypeEnum.MD;

  async savePhoto() {
    const imageSource: image.ImageSource = image.createImageSource(ProfileConstants.getInstance().LOCAL_PROFILE_PATH);
    imageSource.createPixelMap((err, pixelMap) => {
      Logger.error('tag', `createPixelMap error: ${err}`)
    });
    await imageSource.release();
  }

  checkBackAction(isCenter?: boolean) {
    AlertDialog.show(
      {
        message: $r('app.string.sure_cancel'),
        autoCancel: false,
        offset: isCenter ? { dx: 0, dy: 0 } : {
          dx: $r('app.float.xxl_padding_margin'),
          dy: AppStorage.get<number>('statusBarHeight') || 0
        },
        primaryButton: {
          value: $r('app.string.cancel'),
          action: () => {
            Logger.info('tag', 'cancel');
          }
        },
        secondaryButton: {
          value: $r('app.string.sure'),
          action: () => {
            Logger.info('tag', 'sure');
            this.uri = '';
          }
        }
      }
    )
  }

  build() {}
}

@Builder
export function PhotoConfirmBuilder() {
  ConfirmView();
}
"#;
    let result = TransformHarness::transform(&make_source(
        "features/mine/src/main/ets/views/ConfirmView.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("async savePhoto()"),
        "regular method modifiers must be preserved, got:\n{result}"
    );
    assert!(
        result.contains("secondaryButton"),
        "regular method body must be preserved, got:\n{result}"
    );
    assert!(
        !result.contains("AlertDialog.show(\nexport function"),
        "method body was truncated before top-level builder, got:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered output must be valid TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_transforms_entry_component_with_hvigor_route_registration() {
    let src = "@Entry\n@Component\nstruct Page {\n  build() {}\n}";
    let options = TransformOptions {
        bundle_name: Some("com.example.arkdown.test".to_string()),
        module_name: Some("phone".to_string()),
        project_root: Some(PathBuf::from("/project")),
        project_path: Some(PathBuf::from("/project/products/phone/src/main/ets")),
        module_roots: Vec::new(),
        component_metadata: std::sync::Arc::new(ComponentMetadata::default()),
        sources: crate::source::TransformSourceSnapshot::default(),
        module_exports: ModuleExportIndex::default(),
        navigation_routes: NavigationRouteMap::default(),
        form_entry: false,
        partial_update: true,
        checker_mode: crate::CheckerMode::Oxc,
    };
    let result = TransformHarness::transform_with_options(
        &make_source("products/phone/src/main/ets/pages/Page.ets", src),
        &options,
    )
    .unwrap();
    assert!(result.contains("class Page extends ViewPU"));
    assert!(result.contains("registerNamedRoute(() => new Page(undefined, {})"));
    assert!(result.contains("bundleName: \"com.example.arkdown.test\""));
    assert!(result.contains("pagePath: \"pages/Page\""));
    assert!(result.contains("pageFullPath: \"products/phone/src/main/ets/pages/Page\""));
    assert!(!result.contains("@Entry"));
    assert!(!result.contains("struct Page"));
}

#[test]
fn test_xcomponent_create_appends_hvigor_host_module_identity() {
    let src = r#"
@Component
struct Player {
  build() {
    XComponent({ id: "player", type: resolveXComponentType(), libraryname: "player" })
  }
}
"#;
    let options = TransformOptions {
        bundle_name: Some("com.example.app".to_string()),
        module_name: Some("entry".to_string()),
        ..TransformOptions::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Player.ets", src),
        &options,
    )
    .unwrap();
    assert!(
        result.contains(
            "XComponent.create({ id: \"player\", type: resolveXComponentType(), libraryname: \"player\" }, \"com.example.app/entry\")"
        ),
        "SDK checkArguments appends bundleName/moduleName to XComponent.create:\n{result}"
    );
    assert!(
        !result.contains("XComponent.pop();"),
        "a dynamic XComponent type is not statically a container in SDK checkContainer:\n{result}"
    );
}

#[test]
fn test_viewv2_entry_lowers_external_custom_component_and_route_like_hvigor() {
    let src = r#"
import { LaunchProtectPage, LaunchProtect } from "launch_protect";

LaunchProtect.setAppIcon($r("app.media.app_icon"));

@Entry
@ComponentV2
export struct SecureModePage {
  build() {
    Column() {
      LaunchProtectPage()
    }
  }
}
"#;
    let options = TransformOptions {
        bundle_name: Some("com.example.hexample".to_string()),
        module_name: Some("example".to_string()),
        project_root: Some(PathBuf::from("/project")),
        project_path: Some(PathBuf::from("/project/example/src/main/ets")),
        module_roots: Vec::new(),
        component_metadata: std::sync::Arc::new(ComponentMetadata::default()),
        sources: crate::source::TransformSourceSnapshot::default(),
        module_exports: ModuleExportIndex::default(),
        navigation_routes: NavigationRouteMap::default(),
        form_entry: false,
        partial_update: true,
        checker_mode: crate::CheckerMode::Oxc,
    };
    let result = TransformHarness::transform_with_options(
        &make_source("example/src/main/ets/pages/SecureModePage.ets", src),
        &options,
    )
    .unwrap();

    assert!(
        result.starts_with("if (!(\"finalizeConstruction\" in ViewPU.prototype))"),
        "ViewV2 @Entry must keep hvigor's ViewPU finalizeConstruction shim:\n{}",
        result
    );
    assert!(
        result.contains("export class SecureModePage extends ViewV2"),
        "@ComponentV2 entry must lower to ViewV2 class:\n{}",
        result
    );
    assert!(
        result.contains(
            "constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo)"
        ),
        "ViewV2 constructor parameters must match hvigor without paramsLambda/extraInfo defaults:\n{}",
        result
    );
    assert!(
        result.contains("new LaunchProtectPage("),
        "unresolved external ArkUI component imports must lower as custom components like hvigor:\n{}",
        result
    );
    assert!(
        result.contains("ViewV2.create(componentCall)"),
        "ViewV2 custom component creation must use ViewV2.create like hvigor:\n{}",
        result
    );
    assert!(
        !result.contains("LaunchProtectPage.create()"),
        "external custom component must not be emitted as SDK component create/pop:\n{}",
        result
    );
    assert!(
        result.contains("componentCall.paramsGenerator_ = paramsLambda"),
        "custom component params generator must be retained:\n{}",
        result
    );
    assert!(
        result.contains("static getEntryName()"),
        "ViewV2 @Entry must emit getEntryName like hvigor:\n{}",
        result
    );
    assert!(
        result.contains("registerNamedRoute(() => new SecureModePage(undefined, {})"),
        "ViewV2 @Entry must still register the page route:\n{}",
        result
    );
    assert!(result.contains("pagePath: \"pages/SecureModePage\""));
}

#[test]
fn test_viewv2_entry_lowers_resolved_reexported_componentv2_like_hvigor() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let package_root = project_root.join("oh_modules/launch_protect");
    let index_path = package_root.join("Index.ets");
    let page_path = package_root.join("src/main/ets/pages/LaunchProtectPage.ets");
    let index_source = r#"
export { LaunchProtectPage } from './src/main/ets/pages/LaunchProtectPage'
"#;
    let page_source = r#"
@Entry
@ComponentV2
export struct LaunchProtectPage {
  build() {}
}
"#;
    let src = r#"
import { LaunchProtectPage, LaunchProtect } from "launch_protect";

LaunchProtect.setAppIcon($r("app.media.app_icon"));

@Entry
@ComponentV2
export struct SecureModePage {
  build() {
    Column() {
      LaunchProtectPage()
    }
  }
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from(index_source),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&page_path),
        std::sync::Arc::<str>::from(page_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let options = TransformOptions {
        bundle_name: Some("com.example.hexample".to_string()),
        module_name: Some("example".to_string()),
        project_root: Some(project_root.clone()),
        project_path: Some(project_root.join("example/src/main/ets")),
        module_roots: Vec::new(),
        component_metadata: std::sync::Arc::new(ComponentMetadata::default()),
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        sources,
        navigation_routes: NavigationRouteMap::default(),
        form_entry: false,
        partial_update: true,
        checker_mode: crate::CheckerMode::Oxc,
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            project_root
                .join("example/src/main/ets/pages/SecureModePage.ets")
                .to_str()
                .unwrap(),
            src,
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("let componentCall = new LaunchProtectPage(this, {}, undefined"),
        "resolved re-exported @ComponentV2 imports must lower as custom components like hvigor:\n{}",
        result
    );
    assert!(
        result.contains("ViewV2.create(componentCall)"),
        "resolved re-exported @ComponentV2 imports must use ViewV2.create:\n{}",
        result
    );
    assert!(
        !result.contains("LaunchProtectPage.create()"),
        "resolved re-exported @ComponentV2 imports must not lower as SDK components:\n{}",
        result
    );
}

#[test]
fn test_custom_component_create_uses_child_generation_like_hvigor() {
    let temp = tempfile::tempdir().unwrap();
    let page_dir = temp.path().join("entry/src/main/ets/pages");
    let legacy_path = page_dir.join("Legacy.ets");
    let modern_path = page_dir.join("Modern.ets");
    let host_path = page_dir.join("Host.ets");
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&legacy_path),
        std::sync::Arc::<str>::from("@Component\nexport struct Legacy {\n  build() {}\n}"),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&modern_path),
        std::sync::Arc::<str>::from("@ComponentV2\nexport struct Modern {\n  build() {}\n}"),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let options = TransformOptions {
        sources: sources.clone(),
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        ..Default::default()
    };
    let source = r#"
import { Legacy } from "./Legacy";
import { Modern } from "./Modern";

@Builder
function ModernBuilder() {
  Modern()
}

@ComponentV2
struct Host {
  build() {
    Legacy()
  }
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source(host_path.to_str().unwrap(), source),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("let componentCall = new Modern(")
            && result.contains("ViewV2.create(componentCall);"),
        "SDK process_custom_component/createNewComponent selects ViewV2 from the child struct even in a global builder:\n{result}"
    );
    assert!(
        result.contains("let componentCall = new Legacy(")
            && result.contains("ViewPU.create(componentCall);"),
        "a ComponentV2 parent must still create a V1 child through ViewPU:\n{result}"
    );
    assert_eq!(
        result.matches("ViewV2.create(componentCall);").count(),
        1,
        "only the V2 child uses ViewV2.create:\n{result}"
    );
    assert_eq!(
        result.matches("ViewPU.create(componentCall);").count(),
        1,
        "only the V1 child uses ViewPU.create:\n{result}"
    );
}

#[test]
fn test_global_builder_uses_local_component_v2_generation_like_hvigor() {
    let source = r#"
@Builder
function ModernBuilder() {
  Modern()
}

@ComponentV2
struct Modern {
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/Modern.ets",
        source,
    ))
    .unwrap();

    assert!(
        result.contains("let componentCall = new Modern(")
            && result.contains("ViewV2.create(componentCall);"),
        "SDK createNewComponent selects ViewV2 from a local child declaration even when the caller is a preceding global builder:\n{result}"
    );
    assert!(
        !result.contains("ViewPU.create(componentCall);"),
        "local @ComponentV2 metadata must be available before statement lowering:\n{result}"
    );
}

#[test]
fn test_custom_component_name_wins_over_local_builder_method_for_unqualified_call() {
    let source = r#"
@Component
struct LogPanel {
  build() {
    Text("log")
  }
}

@Component
struct Host {
  @Builder
  LogPanel() {
    LogPanel()
  }

  build() {
    this.LogPanel()
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/Host.ets",
        source,
    ))
    .unwrap();

    assert!(
        result.contains("let componentCall = new LogPanel(this, {}")
            && result.contains("ViewPU.create(componentCall);"),
        "SDK process_component_build.getComponentType resolves a custom component before a same-named @Builder method:\n{result}"
    );
    assert!(
        !result.contains("\n        LogPanel.bind(this)();"),
        "the unqualified component call inside the builder must not dispatch to the same-named method:\n{result}"
    );
    assert!(
        result.contains("this.LogPanel.bind(this)();"),
        "the explicit this.LogPanel() call remains a builder-method dispatch:\n{result}"
    );
}

#[test]
fn test_resolved_package_builder_is_not_reclassified_by_external_component_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let package_root = temp.path().join("oh_modules/@scope/ui");
    let index_path = package_root.join("Index.ets");
    let host_path = temp.path().join("entry/src/main/ets/components/Host.ets");
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from(
            "@Builder\nexport function LoadingBuilder() {\n  Text(\"loading\")\n}",
        ),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let module_roots = vec![("ui".to_string(), package_root)];
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            host_path.to_str().unwrap(),
            "import { LoadingBuilder } from \"@scope/ui\";\n@Component\nstruct Host {\n  build() {\n    LoadingBuilder()\n  }\n}",
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("LoadingBuilder.bind(this)();")
            && !result.contains("new LoadingBuilder(")
            && !result.contains("ViewPU.create(componentCall);"),
        "a resolved @Builder export must not be promoted by the unresolved-package custom-component usage fallback:\n{result}"
    );
}

#[test]
fn test_imported_builder_object_argument_uses_hvigor_parameter_proxy() {
    let temp = tempfile::tempdir().unwrap();
    let builder_path = temp.path().join("components/PicassoBuilder.ets");
    let host_path = temp.path().join("components/AnimationView.ets");
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&builder_path),
        std::sync::Arc::<str>::from(
            "@Builder\nexport function PicassoBuilder($$: { vm: object }) {\n  Text($$.vm)\n}",
        ),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            host_path.to_str().unwrap(),
            "import { PicassoBuilder } from './PicassoBuilder';\n@Component\nstruct AnimationView {\n  build() {\n    ForEach([], (vm: object) => PicassoBuilder({ vm: vm }))\n  }\n}",
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains(
            "PicassoBuilder.bind(this)(makeBuilderParameterProxy(\"PicassoBuilder\", { vm: () => vm }))"
        ),
        "SDK process_component_build.js::transferBuilderCall proxies an imported @Builder object argument:\n{result}"
    );
}

#[test]
fn test_global_builder_proxy_uses_dollar_parameter_not_this_backing_state() {
    let src = r#"
interface BuilderParams {
  value: string;
}

@Builder
function TargetBuilder(params: object) {}

@Builder
export function HostBuilder($$: BuilderParams) {
  TargetBuilder({ fromParam: $$.value, fromThis: this.value })
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/Builders.ets",
        src,
    ))
    .unwrap();

    assert!(
        result.contains(
            "fromParam: () => $$[\"__value\"] ? $$[\"__value\"] : $$[\"value\"]"
        ),
        "SDK traverseBuilderParams uses the $$ backing property while processing a global @Builder:\n{result}"
    );
    assert!(
        result.contains("fromThis: () => this.value")
            && !result.contains("fromThis: () => this[\"__value\"]"),
        "SDK suppresses this.x backing-property fallback while processGlobalBuilder is active:\n{result}"
    );
}

#[test]
fn test_default_identifier_export_is_classified_as_custom_component_like_hvigor() {
    let temp = tempfile::tempdir().unwrap();
    let component_path = temp.path().join("components/AiTab.ets");
    let component_source = r#"
@Component
struct AiTab {
  @Prop title: string = '';

  build() {
    Text(this.title)
  }
}

export default AiTab;
"#;
    let src = r#"
import AiTab from './components/AiTab';

@Component
struct Page {
  build() {
    AiTab({ title: 'AI' }).height(32)
  }
}
"#;
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&component_path),
        std::sync::Arc::<str>::from(component_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let metadata = ComponentMetadata::default();
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(metadata.clone()),
        module_exports: ModuleExportIndex::build(&sources, &[], &metadata),
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(temp.path().join("Page.ets").to_str().unwrap(), src),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("new AiTab("),
        "hvigor componentCollection follows `export default AiTab`; got:\n{result}"
    );
    assert!(
        result.contains("__Common__.height(32);"),
        "attributes on a default-exported custom component must use __Common__: got:\n{result}"
    );
    assert!(
        !result.contains("AiTab.create("),
        "default identifier exports must not fall back to inner components: got:\n{result}"
    );
}

#[test]
fn test_resolved_external_declaration_ui_usage_is_custom_component_like_hvigor() {
    let temp = tempfile::tempdir().unwrap();
    let kit_root = temp.path().join("sdk/AccountKit");
    let declaration_path = kit_root.join("Index.d.ets");
    let declaration_source = r#"
export declare class LoginWithHuaweiIDButton {
  constructor(params: object);
}
"#;
    let src = r#"
import { LoginWithHuaweiIDButton } from '@kit.AccountKit';

@Component
struct Page {
  build() {
    LoginWithHuaweiIDButton({ params: {} }).height(44).width('100%')
  }
}
"#;
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&declaration_path),
        std::sync::Arc::<str>::from(declaration_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let module_roots = vec![("@kit.AccountKit".to_string(), kit_root)];
    let metadata = ComponentMetadata::default();
    let options = TransformOptions {
        module_roots: module_roots.clone(),
        component_metadata: std::sync::Arc::new(metadata.clone()),
        module_exports: ModuleExportIndex::build(&sources, &module_roots, &metadata),
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(temp.path().join("Page.ets").to_str().unwrap(), src),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("new LoginWithHuaweiIDButton("),
        "hvigor type-checks imported SDK UI declarations as custom components: got:\n{result}"
    );
    assert!(
        result.contains("__Common__.height(44);") && result.contains("__Common__.width('100%');"),
        "SDK custom component common attributes must use __Common__: got:\n{result}"
    );
    assert!(
        !result.contains("LoginWithHuaweiIDButton.create("),
        "resolved declaration-only imports must not fall back to inner components: got:\n{result}"
    );
}

#[test]
fn test_export_star_cycle_converges_to_complete_component_exports() {
    let temp = tempfile::tempdir().unwrap();
    let a_path = temp.path().join("components/A.ets");
    let b_path = temp.path().join("components/B.ets");
    let c_path = temp.path().join("components/C.ets");
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&a_path),
        std::sync::Arc::<str>::from("export * from './B';\nexport * from './C';\n"),
    );
    source_map.insert(
        arkdown_path::lexical_normalize(&b_path),
        std::sync::Arc::<str>::from("export * from './A';\n"),
    );
    source_map.insert(
        arkdown_path::lexical_normalize(&c_path),
        std::sync::Arc::<str>::from(
            "@Component\nexport struct ExportedCard {\n  build() { Text('card') }\n}\n",
        ),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let metadata = ComponentMetadata::default();
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(metadata.clone()),
        module_exports: ModuleExportIndex::build(&sources, &[], &metadata),
        sources,
        ..Default::default()
    };
    let src = r#"
import { ExportedCard } from './components/B';

@Component
struct Page {
  build() {
    ExportedCard().height(40)
  }
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source(temp.path().join("Page.ets").to_str().unwrap(), src),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("new ExportedCard("),
        "hvigor componentCollection resolves complete exports across export-star cycles: got:\n{result}"
    );
    assert!(
        result.contains("__Common__.height(40);") && !result.contains("ExportedCard.create("),
        "cycle resolution must retain custom-component common attribute lowering: got:\n{result}"
    );
}

#[test]
fn test_entry_route_registration_is_emitted_after_sibling_components() {
    let src = r#"
@Entry
@Component
struct SplashPage {
  build() {
    ImgItem()
  }
}

@Component
struct ImgItem {
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("products/phone/src/main/ets/pages/SplashPage.ets", src),
        &TransformOptions {
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/products/phone/src/main/ets")),
            ..TransformOptions::default()
        },
    )
    .unwrap();
    let splash = result.find("class SplashPage extends ViewPU").unwrap();
    let img = result.find("class ImgItem extends ViewPU").unwrap();
    let route = result
        .find("registerNamedRoute(() => new SplashPage")
        .unwrap();

    assert!(
        splash < img,
        "component class order must follow source order"
    );
    assert!(
        img < route,
        "entry route registration must be emitted after sibling component classes like hvigor"
    );
}

#[test]
fn test_dependency_entry_transform_emits_named_route_registration() {
    let src = r#"
export const ACTION_SHEET_ENTRY_NAME: string = "MSIActionSheetApiEntryPage"

@Entry({ routeName: ACTION_SHEET_ENTRY_NAME })
@Component
export struct MSIActionSheetApiEntryPage {
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source(
            "oh_modules/@msi/runtime/src/main/ets/window/ActionSheetPage.ets",
            src,
        ),
        &TransformOptions {
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/entry/src/main/ets")),
            ..TransformOptions::default()
        },
    )
    .unwrap();

    assert!(
        result.contains("export const ACTION_SHEET_ENTRY_NAME"),
        "route name constants must remain importable in normal module context:\n{}",
        result
    );
    assert!(
        result.contains("export class MSIActionSheetApiEntryPage extends ViewPU"),
        "@Entry component still lowers to a ViewPU class:\n{}",
        result
    );
    assert!(
        result.contains("static getEntryName()"),
        "@Entry class metadata must remain available:\n{}",
        result
    );
    assert!(
        result.contains("registerNamedRoute(() => new MSIActionSheetApiEntryPage(undefined, {})"),
        "SDK processUISyntax/createEntryNode registers dependency @Entry sources:\n{}",
        result
    );
}

#[test]
fn test_minimal_index_component_lowers_arkui_semantics() {
    let src =
        include_str!("../../../../tests/fixtures/minimal-hap/entry/src/main/ets/pages/Index.ets");
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Index.ets", src),
        &TransformOptions {
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/entry/src/main/ets")),
            ..TransformOptions::default()
        },
    )
    .unwrap();

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
        parsed.diagnostics.is_empty(),
        "lowered output must be valid TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_tab_content_children_lower_into_create_builder_like_hvigor() {
    // ets-loader `process_component_build.js::processTabAndNav` gives
    // TabContent's ArkUI body to `TabContent.create(() => { ... })`. Keeping
    // it outside the create callback changes generated anonymous functions
    // and ViewPU mounting semantics in large applications.
    let src = r#"
@Component
struct TabHost {
  build() {
    Tabs() {
      TabContent() {
        Text('inside')
      }
    }
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/TabHost.ets", src))
            .expect("TabContent builder body must lower like latest ets-loader");

    let create = result
        .find("TabContent.create(() => {")
        .expect("TabContent.create must receive a builder callback");
    let text = result
        .find("Text.create")
        .unwrap_or_else(|| panic!("TabContent callback must contain its lowered child:\n{result}"));
    let pop = result
        .find("TabContent.pop()")
        .expect("TabContent must still close after the creation observer");
    assert!(
        create < text && text < pop,
        "TabContent callback and pop order must match hvigor:\n{result}"
    );
}

#[test]
fn test_viewpu_delete_uses_hvigor_subscriber_manager_get_name() {
    let src = r#"
@Component
struct Counter {
  @State count: number = 0;

  build() {
    Text(`${this.count}`)
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Counter.ets", src))
            .unwrap();

    assert!(
        result.contains("SubscriberManager.Get().delete(this.id__());"),
        "ViewPU deletion must call hvigor's SubscriberManager.Get(), got:\n{}",
        result
    );
    assert!(
        !result.contains("SubscriberManager.get()"),
        "SubscriberManager must not reuse state property get() naming:\n{}",
        result
    );
    assert!(
        result.contains("return this.__count.get();"),
        "state field accessors must still use lowercase get():\n{}",
        result
    );
}

#[test]
fn test_state_conditional_initializer_is_valid_constructor_argument() {
    let src = r#"
@Component
struct ConditionalStateView {
  @State value: string = AppStorage.get<boolean>('enabled') ? 'on' : 'off';

  build() {
    Text(this.value)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/pages/ConditionalStateView.ets",
        src,
    ))
    .expect("conditional @State initializer must transform");

    assert!(
        result.contains("new ObservedPropertySimplePU("),
        "expected @State backing property, got:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "lowered output must be valid TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_imported_reexported_enum_state_is_simple_like_hvigor_enum_collection() {
    let temp = tempfile::tempdir().unwrap();
    let utils_root = temp.path().join("commons/utils");
    let enum_dir = utils_root.join("src/main/ets/common");
    let index_path = utils_root.join("Index.ets");
    let index_source = r#"
import {
  EventTypeEnum,
  LoadingStatus,
  ResourcesType
} from './src/main/ets/common/CommonEnums';
export {
  EventTypeEnum,
  LoadingStatus,
  ResourcesType
};
"#;
    let enum_path = enum_dir.join("CommonEnums.ets");
    let enum_source = "export enum LoadingStatus { OFF, LOADING, SUCCESS, FAILED }\nexport enum EventTypeEnum { A }\nexport class ResourcesType {}\n";

    let src = r#"
import {
  EventTypeEnum,
  LoadingStatus,
  ResourcesType
} from '@ohos/utils';

@Component
struct Page {
    @State loadingStatus: LoadingStatus = LoadingStatus.Loading;
  build() {}
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from(index_source),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&enum_path),
        std::sync::Arc::<str>::from(enum_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let module_roots = vec![("utils".to_string(), utils_root)];
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            temp.path()
                .join("features/discover/src/main/ets/pages/Page.ets")
                .to_str()
                .unwrap(),
            src,
        ),
        &options,
    )
    .unwrap();
    assert!(
        result.contains("private __loadingStatus: ObservedPropertySimplePU<LoadingStatus>"),
        "imported enum state must use SimplePU like hvigor enumCollection, got:\n{result}"
    );
}

#[test]
fn test_imported_reexported_basic_union_alias_is_simple_like_hvigor_type_checker() {
    let temp = tempfile::tempdir().unwrap();
    let package_root = temp.path().join("payment/recce");
    let alias_path = package_root.join("src/main/ets/Mode.ets");
    let index_path = package_root.join("Index.ets");
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&alias_path),
        std::sync::Arc::<str>::from(
            "export type OfflineMode = \"specified_only\" | \"cache_only\";",
        ),
    );
    source_map.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from("export * from './src/main/ets/Mode';"),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let module_roots = vec![("recce".to_string(), package_root)];
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            temp.path()
                .join("entry/src/main/ets/pages/Page.ets")
                .to_str()
                .unwrap(),
            "import { OfflineMode } from '@ohos/recce';\n@Component\nstruct Page {\n  @State mode: OfflineMode = \"specified_only\";\n  build() {}\n}",
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("private __mode: ObservedPropertySimplePU<OfflineMode>"),
        "SDK getDeclarationType follows re-exported basic-union aliases through the TypeScript checker:\n{result}"
    );
}

#[test]
fn test_sdk_enum_and_nullable_state_types_are_simple_like_hvigor_type_checker() {
    let component_metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata");
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(component_metadata),
        ..Default::default()
    };
    let source = r#"
@Component
struct Page {
  @State weight: FontWeight | null = null;
  @State visible: boolean | undefined = undefined;
  @State label: string | null = null;
  @State exact: "text" = "text";
  @State normalizedString: "toast" | "loading" | string = "toast";
  @State qualifiedEnum: arkui.FontWeight = arkui.FontWeight.Normal;
  @State qualifiedObject: cips.Channel = {} as cips.Channel;
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Page.ets", source),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("private __weight: ObservedPropertySimplePU<FontWeight | null>")
            && result.contains(
                "private __visible: ObservedPropertySimplePU<boolean | undefined>"
            )
            && result
                .contains("private __qualifiedEnum: ObservedPropertySimplePU<arkui.FontWeight>"),
        "SDK enumCollection and findNonNullType keep enum/nullable primitive unions on SimplePU:\n{result}"
    );
    assert!(
        result.contains("private __label: ObservedPropertyObjectPU<string | null>")
            && result.contains("private __exact: ObservedPropertyObjectPU<\"text\">")
            && result.contains(
                "private __normalizedString: ObservedPropertyObjectPU<\"toast\" | \"loading\" | string>"
            )
            && result
                .contains("private __qualifiedObject: ObservedPropertyObjectPU<cips.Channel>"),
        "SDK getDeclarationType applies TypeScript union normalization before inspecting TypeFlags:\n{result}"
    );
}

#[test]
fn test_external_sdk_builder_function_is_lowered_as_hvigor_builder() {
    let temp = tempfile::tempdir().unwrap();
    let external_ets = temp.path().join("hms").join("ets");
    let api_root = external_ets.join("api");
    let kit_root = external_ets
        .join("build-tools")
        .join("ets-loader")
        .join("kit_configs");
    std::fs::create_dir_all(&api_root).unwrap();
    std::fs::create_dir_all(&kit_root).unwrap();
    std::fs::write(
        external_ets.join("sdkConfig.json"),
        r#"{"apiPath":"./api"}"#,
    )
    .unwrap();
    std::fs::write(
        api_root.join("@hms.collaboration.service.d.ets"),
        r#"@Builder
declare function createCollaborationServiceMenuItems(): void;
export { createCollaborationServiceMenuItems };"#,
    )
    .unwrap();
    std::fs::write(
        kit_root.join("@kit.ServiceCollaborationKit.json"),
        r#"{
  "symbols": {
    "createCollaborationServiceMenuItems": {
      "source": "@hms.collaboration.service",
      "bindings": "createCollaborationServiceMenuItems"
    }
  }
}"#,
    )
    .unwrap();
    let component_metadata =
        ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/ets-loader"
            )),
            &[api_root],
        )
        .unwrap();
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(component_metadata),
        ..Default::default()
    };
    let source = r#"
import { createCollaborationServiceMenuItems } from '@kit.ServiceCollaborationKit';

@Component
struct Page {
  build() {
    Menu() {
      createCollaborationServiceMenuItems()
    }
  }
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Page.ets", source),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("createCollaborationServiceMenuItems.bind(this)();"),
        "HMS @Builder declarations from ets-loader externalApiPaths must use transferBuilderCall: {result}"
    );
    assert!(
        !result.contains("createCollaborationServiceMenuItems.create("),
        "an SDK @Builder must not be lowered as an ArkUI component: {result}"
    );
}

#[test]
fn test_kit_namespace_enum_prop_is_simple_like_typescript_checker() {
    let component_metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata");
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(component_metadata),
        ..Default::default()
    };
    let source = r#"
import { photoAccessHelper } from "@kit.MediaLibraryKit";

@Component
struct Page {
  @Prop mimeType: photoAccessHelper.PhotoViewMIMETypes;
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Page.ets", source),
        &options,
    )
    .unwrap();

    assert!(
        result.contains(
            "this.__mimeType = new SynchedPropertySimpleOneWayPU(params.mimeType, this, \"mimeType\")"
        ),
        "SDK process_component_member.js delegates qualified namespace enums to the TypeScript checker:\n{result}"
    );
}

#[test]
fn test_unrelated_enum_name_does_not_reclassify_local_interface() {
    let temp = tempfile::tempdir().unwrap();
    let enum_path = temp.path().join("feature/SharedEnums.ets");
    let page_path = temp.path().join("entry/src/main/ets/pages/Page.ets");
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&enum_path),
        std::sync::Arc::<str>::from("export enum Position { Start, End }"),
    );
    source_map.insert(
        arkdown_path::lexical_normalize(&page_path),
        std::sync::Arc::<str>::from(
            "interface Position { x: number; y: number }\n@Component\nstruct Page {\n  @State point: Position = { x: 0, y: 0 };\n  build() {}\n}",
        ),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let options = TransformOptions {
        sources: sources.clone(),
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(
            page_path.to_str().unwrap(),
            "interface Position { x: number; y: number }\n@Component\nstruct Page {\n  @State point: Position = { x: 0, y: 0 };\n  build() {}\n}",
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("private __point: ObservedPropertyObjectPU<Position>"),
        "an unrelated enum must not reclassify a local interface according to parallel source scheduling:\n{result}"
    );
}

#[test]
fn test_prior_unrelated_enum_name_does_not_reclassify_local_interface() {
    let temp = tempfile::tempdir().unwrap();
    let enum_path = temp.path().join("a-feature/SharedEnums.ets");
    let page_path = temp.path().join("z-entry/src/main/ets/pages/Page.ets");
    let page_source = "interface Position { x: number; y: number }\n@Component\nstruct Page {\n  @State point: Position = { x: 0, y: 0 };\n  build() {}\n}";
    let mut source_map = rustc_hash::FxHashMap::default();
    source_map.insert(
        arkdown_path::lexical_normalize(&enum_path),
        std::sync::Arc::<str>::from("export enum Position { Start, End }"),
    );
    source_map.insert(
        arkdown_path::lexical_normalize(&page_path),
        std::sync::Arc::<str>::from(page_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(source_map);
    let options = TransformOptions {
        sources: sources.clone(),
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        ..Default::default()
    };
    let result = TransformHarness::transform_with_options(
        &make_source(page_path.to_str().unwrap(), page_source),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("private __point: ObservedPropertyObjectPU<Position>"),
        "source inventory order must not let an unrelated enum reclassify a local interface:\n{result}"
    );
}

#[test]
fn test_sdk_declaration_component_import_uses_custom_component_common_wrapper() {
    let component_metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ets-loader"
    )))
    .expect("test ets-loader metadata");
    let options = TransformOptions {
        component_metadata: std::sync::Arc::new(component_metadata),
        ..Default::default()
    };
    let src = r#"
import { AVVolumePanel } from '@kit.AudioKit';

enum HitTestMode { Transparent }

@Component
struct Page {
  @State volumeLevel: number = 0;

  build() {
    AVVolumePanel({ volumeLevel: this.volumeLevel })
      .hitTestBehavior(HitTestMode.Transparent)
  }
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("entry/src/main/ets/pages/Index.ets", src),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("__Common__.hitTestBehavior(HitTestMode.Transparent);"),
        "SDK declaration component chained common attrs must be emitted on __Common__; got:\n{result}"
    );
    assert!(
        result.contains("new AVVolumePanel("),
        "SDK declaration component must be emitted as a custom component constructor; got:\n{result}"
    );
    assert!(
        !result.contains("AVVolumePanel.hitTestBehavior("),
        "SDK declaration component must not be treated as an inner component; got:\n{result}"
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
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/ItemList.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("private __items: SynchedPropertySimpleOneWayPU<string[]>"),
        "expected SynchedPropertySimpleOneWayPU declaration for non-simple @Link type, got:\n{}",
        result
    );
    assert!(
        result.contains("new SynchedPropertyObjectTwoWayPU(params.items, this, \"items\")"),
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
    let result = TransformHarness::transform(&make_source(
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
    assert_eq!(
        result.matches("this.__item.set(params.item)").count(),
        2,
        "@ObjectLink must update its backing property in both setInitiallyProvidedValue and updateStateVars:\n{result}"
    );
    assert!(
        !result.contains("set item("),
        "@ObjectLink is immutable and must not emit a field setter:\n{result}"
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
    let result = TransformHarness::transform(&make_source(
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
    let result = TransformHarness::transform(&make_source(
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

#[test]
fn test_consume_decorator_with_default_lowers_to_initialize_consume_default() {
    let src = r#"
enum HitTestMode { Default }

@Component
struct DynamicContainer {
  @Consume('mrnHitTestMode') mrnHitTestMode: HitTestMode = HitTestMode.Default;
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/DynamicContainer.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains(
            "this.initializeConsume(\"mrnHitTestMode\", \"mrnHitTestMode\", HitTestMode.Default)"
        ),
        "SDK process_component_member.js::updateConsumeProperty passes the initializer as initializeConsume's third argument; got:\n{}",
        result
    );
}

#[test]
fn test_prop_decorator_lowers_to_synched_property_one_way_pu() {
    let src = r#"
@Component
struct CardItem {
  @Prop itemData: string;
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/widget/pages/WidgetCard.ets",
        src,
    ))
    .unwrap();
    assert!(
            result.contains("this.__itemData = new SynchedPropertySimpleOneWayPU(params.itemData, this, \"itemData\")"),
            "expected @Prop constructor init from params, got:\n{}",
            result
        );
    assert!(
        result.contains("get itemData()"),
        "expected @Prop getter, got:\n{}",
        result
    );
    assert!(
        result.contains("this.__itemData.reset(params.itemData)"),
        "expected @Prop updateStateVars reset, got:\n{}",
        result
    );
}

#[test]
fn test_prop_with_default_value_emits_undefined_fallback() {
    // Upstream ref: process_component_member.js createUpdateParamsWithIf(e, true, initializer)
    // When @Prop has an initializer, generate:
    //   if (params.ratio === undefined) { this.__ratio.set(0); }
    let src = r#"
@Component
struct BuildListItem {
  @Prop buildingImgRatio: number = 0;
  @Prop title: string;
  build() {}
}
"#;
    let result = TransformHarness::transform(&make_source(
        "features/challenge/src/main/ets/components/BuildListItem.ets",
        src,
    ))
    .unwrap();
    // @Prop with initializer: must have undefined fallback
    assert!(
        result.contains("if (params.buildingImgRatio === undefined)"),
        "@Prop with initializer must emit undefined check, got:\n{result}"
    );
    assert!(
        result.contains("this.__buildingImgRatio.set(0)"),
        "@Prop undefined fallback must call .set(defaultVal), got:\n{result}"
    );
    // @Prop without initializer: must NOT have undefined fallback
    assert!(
        !result.contains("if (params.title === undefined)"),
        "@Prop without initializer must NOT emit undefined check, got:\n{result}"
    );
}

#[test]
fn test_custom_component_call_lowers_like_hvigor_partial_update() {
    let src = r#"
@Component
struct Child {
  @Prop itemData: string;
  build() {}
}

@Component
struct Parent {
  @State value: string = "a";
  build() {
    Column() {
      Child({ itemData: this.value })
    }
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("let componentCall = new Child(this,")
            && result.contains("itemData: this.value")
            && result.contains("undefined, elmtId, () => { }"),
        "expected custom component constructor path, got:\n{}",
        result
    );
    assert!(
        result.contains("ViewPU.create(componentCall);"),
        "expected ViewPU.create for custom component, got:\n{}",
        result
    );
    assert!(
        result.contains("let paramsLambda = () =>"),
        "expected paramsLambda for custom component, got:\n{}",
        result
    );
    assert!(
        result.contains("this.updateStateVarsOfChildByElmtId(elmtId, {")
            && result.contains("itemData: this.value"),
        "expected child updateStateVars branch, got:\n{}",
        result
    );
    assert!(
        !result.contains("Child.create"),
        "custom component must not lower to built-in create/pop path, got:\n{}",
        result
    );
}

#[test]
fn test_imported_custom_component_trailing_lambda_populates_sole_builder_param() {
    // Upstream refs:
    // - ets-loader process_component_build.js processExpressionStatementChange
    // - ets-loader process_component_build.js processBlockToExpression
    // A trailing UI block is passed through the target component's sole
    // @BuilderParam in both constructor params and paramsGenerator_.
    let temp = tempfile::tempdir().unwrap();
    let package_path = temp.path().join("oh_modules/@scope/widgets/Index.ets");
    let package_source = r#"
@Component
export struct Panel {
  @Prop enabled: boolean;
  @BuilderParam content: () => void;
  build() {
    this.content()
  }
}
"#;
    let src = r#"
import { Panel } from '@scope/widgets'

@Component
struct Parent {
  build() {
    Panel({ enabled: true }) {
      Stack() {
        Text('child')
      }
    }
  }
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&package_path),
        std::sync::Arc::<str>::from(package_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let module_roots = Vec::new();
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };

    let result = TransformHarness::transform_with_options(
        &make_source(
            temp.path()
                .join("entry/src/main/ets/pages/Parent.ets")
                .to_str()
                .unwrap(),
            src,
        ),
        &options,
    )
    .expect("sole @BuilderParam trailing lambda must lower like hvigor");

    assert_eq!(
        result.matches("content: () => {").count(),
        2,
        "constructor params and paramsLambda must both receive the trailing builder:\n{result}"
    );
    assert!(
        result.contains("Stack.create()")
            && result.contains("Text.create('child')")
            && result.contains("Stack.pop()"),
        "the trailing builder must contain the lowered ArkUI child body:\n{result}"
    );
    assert!(
        result.contains("this.updateStateVarsOfChildByElmtId(elmtId, { enabled: true })")
            && !result.contains(
                "this.updateStateVarsOfChildByElmtId(elmtId, {\n                            content:"
            ),
        "@BuilderParam must not be emitted as an updateStateVars @Prop payload:\n{result}"
    );
}

#[test]
fn test_custom_component_trailing_lambda_rejects_missing_builder_param_like_hvigor() {
    // ets-loader processExpressionStatementChange reports 10905102 when the
    // target does not have exactly one @BuilderParam property.
    let src = r#"
@Component
struct Child {
  build() {}
}

@Component
struct Parent {
  build() {
    Child() {
      Text('child')
    }
  }
}
"#;
    let error =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Parent.ets", src))
            .expect_err("trailing lambda without a sole @BuilderParam must be rejected");
    assert!(
        error.to_string().contains(
            "In the trailing lambda case, 'Child' must have one and only one property decorated with '@BuilderParam'"
        ),
        "expected the hvigor trailing-lambda diagnostic, got: {error}"
    );
}

#[test]
fn test_custom_component_callback_wraps_raw_observed_state_argument() {
    let src = r#"
class Item {}

@Component
struct Child {
  @ObjectLink item: Item;
  @Prop onClicked: () => void;
  build() {}
}

@Component
struct Parent {
  @ObjectLink item: Item;
  handle(item: Item) {}
  build() {
    Child({
      item: this.item,
      onClicked: () => this.handle(this.item)
    })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("item: this.item"),
        "direct custom component param must stay unchanged, got:\n{}",
        result
    );
    assert!(
        result.contains("onClicked: () => this.handle(ObservedObject.GetRawObject(this.item))"),
        "expected callback call argument to match upstream GetRawObject lowering, got:\n{}",
        result
    );
    assert_eq!(
        result
            .matches("onClicked: () => this.handle(ObservedObject.GetRawObject(this.item))")
            .count(),
        3,
        "SDK process_component_build.js rewrites the source parameter before cloning it into the constructor, paramsGenerator_, and updateStateVars branches:\n{result}"
    );
    assert!(
        !result.contains("item: ObservedObject.GetRawObject(this.item)"),
        "GetRawObject must be limited to call arguments, got:\n{}",
        result
    );
}

#[test]
fn test_component_v2_method_arguments_do_not_use_v1_raw_object_protocol() {
    let src = r#"
class Item {}

@ComponentV2
struct Parent {
  @Local item: Item = new Item();
  handle(item: Item) {}
  build() {
    Button("send").onClick(() => this.handle(this.item))
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("this.handle(this.item)")
            && !result.contains("ObservedObject.GetRawObject(this.item)"),
        "SDK process_component_class.js disables isNeedGetRawObject in ComponentV2 context:\n{result}"
    );
}

#[test]
fn test_custom_component_link_param_constructor_uses_backing_field() {
    let src = r#"
@Component
struct Child {
  @Link value: string;
  build() {}
}

@Component
struct Parent {
  @State value: string = "";
  build() {
    Child({ value: this.value })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("let componentCall = new Child(this,")
            && result.contains("value: this.__value"),
        "expected @Link constructor param to use backing field like upstream createReference(), got:\n{}",
        result
    );
    assert!(
        result.contains("let paramsLambda = () =>")
            && result.contains("value: this.value")
            && !result.contains("this.updateStateVarsOfChildByElmtId(elmtId, {\n        value:"),
        "expected paramsLambda to use getter and updateStateVars to skip @Link, got:\n{}",
        result
    );
}

#[test]
fn test_custom_component_link_param_through_imported_reexport_uses_backing_field() {
    let temp = tempfile::tempdir().unwrap();
    let package_root = temp.path().join("oh_modules/@example/mrouter");
    let index_path = package_root.join("Index.ets");
    let router_map_path = package_root.join("src/main/ets/components/RouterMap.ets");
    let index_source = r#"
import RouterMap from './src/main/ets/components/RouterMap'

export {
  RouterMap
}
"#;
    let router_map_source = r#"
@Component
export default struct RouterMap {
  @Link stack: NavPathStack;
  @Prop name: string;
  build() {}
}
"#;
    let src = r#"
import { RouterMap } from '@example/mrouter'

@Component
struct Parent {
  @State stack: NavPathStack = new NavPathStack();

  @Builder
  PageMap(name: string) {
    RouterMap({
      name,
      stack: this.stack
    })
  }

  build() {
    this.PageMap('privacy')
  }
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from(index_source),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&router_map_path),
        std::sync::Arc::<str>::from(router_map_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let module_roots = Vec::new();
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };

    let result = TransformHarness::transform_with_options(
        &make_source(
            temp.path()
                .join("example/src/main/ets/pages/PrivacyPage.ets")
                .to_str()
                .unwrap(),
            src,
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("let componentCall = new RouterMap(this,")
            && result.contains("stack: this.__stack"),
        "expected imported re-exported @Link constructor param to use backing field like hvigor linkCollection, got:\n{}",
        result
    );
    assert!(
        result.contains("this.observeComponentCreation2((elmtId, isInitialRender) => {")
            && !result.contains("parent ? parent : this"),
        "SDK processComponentMethod/addCustomComponentId keep ordinary @Builder custom component receivers on this; got:\n{}",
        result
    );
    assert!(
        result.contains("let paramsLambda = () =>")
            && result.contains("stack: this.stack")
            && !result.contains("this.updateStateVarsOfChildByElmtId(elmtId, {\n        stack:"),
        "expected paramsLambda to keep getter and updateStateVars to skip @Link, got:\n{}",
        result
    );
}

#[test]
fn test_custom_component_link_param_through_package_reexport_uses_backing_field() {
    let temp = tempfile::tempdir().unwrap();
    let reexport_root = temp.path().join("oh_modules/@scope/reexport");
    let child_root = temp.path().join("oh_modules/@scope/child");
    let reexport_path = reexport_root.join("Index.ets");
    let child_path = child_root.join("Index.ets");
    let reexport_source = r#"
export {
  Child
} from '@scope/child'
"#;
    let child_source = r#"
@Component
export struct Child {
  @Link stack: NavPathStack;
  build() {}
}
"#;
    let src = r#"
import { Child } from '@scope/reexport'

@Component
struct Parent {
  @State stack: NavPathStack = new NavPathStack();

  build() {
    Child({
      stack: this.stack
    })
  }
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&reexport_path),
        std::sync::Arc::<str>::from(reexport_source),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&child_path),
        std::sync::Arc::<str>::from(child_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let module_roots = Vec::new();
    let options = TransformOptions {
        module_exports: ModuleExportIndex::build(
            &sources,
            &module_roots,
            &ComponentMetadata::default(),
        ),
        module_roots,
        sources,
        ..Default::default()
    };

    let result = TransformHarness::transform_with_options(
        &make_source(
            temp.path()
                .join("entry/src/main/ets/pages/Index.ets")
                .to_str()
                .unwrap(),
            src,
        ),
        &options,
    )
    .unwrap();

    assert!(
        result.contains("let componentCall = new Child(")
            && result.contains("stack: this.__stack"),
        "expected package re-exported @Link constructor param to use backing field like hvigor processImportModule, got:\n{}",
        result
    );
}

#[test]
fn test_for_each_keeps_direct_observed_state_data_argument() {
    let src = r#"
class Item {}

@Component
struct Parent {
  @State items: Item[] = [];
  build() {
    ForEach(this.items, (item: Item) => {
      Text("item")
    })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("this.forEachUpdateFunction(elmtId, this.items, forEachItemGenFunction)")
            && !result.contains("ObservedObject.GetRawObject(this.items)"),
        "SDK keeps the original ForEach identifier-call parent on a direct data argument:\n{}",
        result
    );
}

#[test]
fn test_for_each_wraps_raw_observed_state_nested_in_data_call() {
    let src = r#"
interface Metrics {
  [name: string]: number
}

@Component
struct Parent {
  @State metrics: Metrics = {};
  build() {
    ForEach(Object.keys(this.metrics), (name: string) => {
      Text(name)
    }, (name: string) => name)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains(
            "this.forEachUpdateFunction(elmtId, Object.keys(ObservedObject.GetRawObject(this.metrics)), forEachItemGenFunction, (name: string) => name, false, false)"
        ),
        "ForEach call data must apply hvigor's nested @State raw-object lowering:\n{}",
        result
    );
}

#[test]
fn test_lazy_for_each_list_item_uses_observed_deep_render_contract() {
    let src = r#"
class DataSource implements IDataSource {}

@Component
struct Parent {
  private dataSource: DataSource = new DataSource();
  build() {
    List() {
      LazyForEach(this.dataSource, (item: string, index: number) => {
        ListItem() {
          Text(item)
        }
        .width("100%")
      }, (item: string) => item)
    }
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("const __lazyForEachItemGenFunction = (_item, index: number) => {")
            && result.contains("const item = _item;"),
        "LazyForEach must use hvigor's untyped _item binding contract:\n{}",
        result
    );
    let generated_lazy_for_each = result
        .find("const __lazyForEachItemGenFunction")
        .expect("LazyForEach generator must be present");
    assert!(
        result[..generated_lazy_for_each].trim_end().ends_with('{'),
        "SDK process_component_build.js scopes generated LazyForEach declarations in a block:\n{}",
        result
    );
    assert!(
        result.contains("ListItem.create(() => { }, false);")
            && result.contains("const observedDeepRender = () => {")
            && result.contains("this.observeComponentCreation2(itemCreation2, ListItem);")
            && result.contains("observedDeepRender();"),
        "ListItem inside LazyForEach must use hvigor's observedDeepRender path:\n{}",
        result
    );
    assert!(
        result.contains("ListItem.width(\"100%\");"),
        "LazyForEach ListItem attributes must remain on itemCreation2:\n{}",
        result
    );
    assert!(
        !result.contains("ListItem.create(deepRenderFunction, true);"),
        "LazyForEach must not use the eager/ForEach ListItem deepRenderFunction path:\n{}",
        result
    );
}

#[test]
fn test_global_builder_list_item_observer_uses_lexical_this() {
    let src = r#"
@Builder
function RenderItems() {
  List() {
    ForEach(["one"], (item: string) => {
      ListItem() {
        Text(item)
      }
    })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/RenderItems.ets",
        src,
    ))
    .expect("global builder ListItem lowering must match process_component_build.js");

    assert!(
        result.contains("this.observeComponentCreation2(itemCreation2, ListItem);")
            && !result.contains(
                "(parent ? parent : this).observeComponentCreation2(itemCreation2, ListItem);"
            ),
        "the ListItem itemCreation2 observer is lexical-this scoped even inside a global @Builder:\n{result}"
    );
}

#[test]
fn test_for_each_item_and_index_no_keyfn_emits_six_arg_update() {
    // When itemFn uses (item, index) but no keyFn is provided,
    // forEachUpdateFunction must be called with (elmtId, data, fn, undefined, true, false).
    let src = r#"
class Item {}

@Component
struct Parent {
  @State items: Item[] = [];
  build() {
    ForEach(this.items, (item: Item, index: number) => {
      Text("item")
    })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains(
            "this.forEachUpdateFunction(elmtId, this.items, forEachItemGenFunction, undefined, true, false)"
        ),
        "expected 6-arg forEachUpdateFunction with undefined keyFn when itemFn uses index, got:\n{}",
        result
    );
}

#[test]
fn test_reusable_component_reuseid_uses_recycle_creation() {
    let src = r#"
@Reusable
@Component
struct Child {
  @State itemData: string = "";
  build() {}
}

@Component
struct Parent {
  @State value: string = "a";
  build() {
    Child({ itemData: this.value }).reuseId('child').width(100)
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("this.observeRecycleComponentCreation(")
            && result.contains("ViewPU.createRecycle(componentCall, recycleNode !== null,")
            && result.contains("new Child(this,")
            && result.contains("itemData: this.value")
            && result.contains("recycleNode.aboutToReuse({")
            && result.contains(
                "recycleNode && typeof recycleNode.aboutToReuseInternal === \"function\""
            )
            && result.contains(
                "recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === \"function\""
            )
            && !result.contains("recycleNode && typeof recycleNode.aboutToReuse === \"function\""),
        "expected @Reusable custom component recycle path, got:\n{}",
        result
    );
    let common_pop = result.find("__Common__.pop();").expect("__Common__.pop");
    let recycle_pop = result.find("__Recycle__.pop();").expect("__Recycle__.pop");
    assert!(
        common_pop < recycle_pop,
        "process_custom_component.js closes __Common__ before reusable __Recycle__:\n{}",
        result
    );
    assert!(
        !result.contains("new Child(this, 'child'")
            && !result.contains("return 'child'")
            && !result.contains("Child.create"),
        "reuseId must not replace constructor params or lower as a built-in component, got:\n{}",
        result
    );
}

#[test]
fn test_non_reusable_custom_component_reuseid_does_not_emit_common_reuseid() {
    let src = r#"
@Component
struct VideoCardContent {
  @Prop title: string = "";
  build() {}
}

@Component
struct Parent {
  build() {
    VideoCardContent({ title: "video" }).reuseId('video')
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("new VideoCardContent(this,")
            && result.contains("title: \"video\"")
            && !result.contains("__Common__.reuseId")
            && !result.contains("VideoCardContent.reuseId"),
        "SDK custom component reuseId must not lower to a nonexistent Common or static component method; got:\n{}",
        result
    );
}

#[test]
fn test_form_profile_entry_lowers_to_load_ets_card_without_storage_argument() {
    let src = r#"
@Entry
@Component
struct WidgetCard {
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("phone/src/main/ets/widget/pages/WidgetCard.ets", src),
        &TransformOptions {
            bundle_name: Some("com.example.hmosworld".to_string()),
            module_name: Some("phone".to_string()),
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/phone/src/main/ets")),
            module_roots: vec![],
            component_metadata: std::sync::Arc::new(ComponentMetadata::default()),
            sources: crate::source::TransformSourceSnapshot::default(),
            module_exports: ModuleExportIndex::default(),
            navigation_routes: NavigationRouteMap::default(),
            form_entry: true,
            partial_update: true,
            checker_mode: crate::CheckerMode::Oxc,
        },
    )
    .unwrap();
    assert!(
        result.contains(
            "loadEtsCard(new WidgetCard(undefined, {}), \"com.example.hmosworld/phone/ets/widget/pages/WidgetCard\")"
        ) && result.contains(
            "ViewStackProcessor.StartGetAccessRecordingFor(ViewStackProcessor.AllocateNewElmetIdForNextComponent())"
        ) && result.contains("ViewStackProcessor.StopGetAccessRecording()"),
        "SDK readCardForm/cardObj entries must use the card registration sequence:\n{result}"
    );
    assert!(
        !result.contains("registerNamedRoute(() => new WidgetCard"),
        "form profile entry must not use page route registration:\n{result}"
    );
}

#[test]
fn test_regular_entry_with_storage_is_not_misclassified_as_card() {
    let src = r#"
let storage = new LocalStorage();

@Entry(storage)
@Component
struct Page {
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("phone/src/main/ets/pages/Page.ets", src),
        &TransformOptions {
            bundle_name: Some("com.example.hmosworld".to_string()),
            module_name: Some("phone".to_string()),
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/phone/src/main/ets")),
            ..TransformOptions::default()
        },
    )
    .unwrap();
    assert!(
        result.contains("registerNamedRoute(() => new Page(undefined, {}, storage)")
            && !result.contains("loadEtsCard"),
        "SDK createEntryNode selects card behavior from cardObj, not from the presence of an @Entry argument:\n{result}"
    );
}

#[test]
fn test_identifier_entry_options_use_hvigor_runtime_dispatch() {
    let src = r#"
declare const entryOptions: object;

@Entry(entryOptions)
@Component
struct Page {
  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("phone/src/main/ets/pages/Page.ets", src),
        &TransformOptions {
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/phone/src/main/ets")),
            ..TransformOptions::default()
        },
    )
    .unwrap();

    assert!(
        result.contains(
            "entryOptions && entryOptions.routeName != undefined && entryOptions.storage != undefined"
        ) && result.contains(
            "entryOptions && entryOptions.routeName != undefined && entryOptions.storage == undefined"
        ) && result.contains(
            "entryOptions && entryOptions.routeName == undefined && entryOptions.storage != undefined"
        ) && result.contains(
            "entryOptions && entryOptions.useSharedStorage != undefined"
        ),
        "SDK judgeRouteNameAndStorageForIdentifier runtime branches must be preserved:\n{result}"
    );
    assert!(
        result.contains(
            "entryOptions.useSharedStorage ? LocalStorage.getShared() : entryOptions.storage"
        ) && result.contains("new Page(undefined, {}, entryOptions)"),
        "identifier entry options must preserve shared-storage selection and scalar-storage fallback:\n{result}"
    );
}

#[test]
fn test_form_entry_rejects_object_entry_options_like_hvigor() {
    let src = r#"
@Entry({ storage: LocalStorage.getShared() })
@Component
struct WidgetCard {
  build() {}
}
"#;
    let error = TransformHarness::transform_with_options(
        &make_source("phone/src/main/ets/widget/pages/WidgetCard.ets", src),
        &TransformOptions {
            form_entry: true,
            ..TransformOptions::default()
        },
    )
    .expect_err("SDK rejects object-literal @Entry options for card sources");
    assert!(
        error
            .to_string()
            .contains("'@Entry' doesn't support {} parameter in card"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn test_entry_object_storage_and_route_use_hvigor_entry_options() {
    let src = r#"
const ModalEntryName: string = 'PicassoModalEntryPage';

@Entry({ storage: LocalStorage.getShared(), routeName: ModalEntryName })
@Component
struct ModalPage {
  @LocalStorageProp('ModalParam') modalParam: object | undefined = undefined

  build() {}
}
"#;
    let result = TransformHarness::transform_with_options(
        &make_source("example/src/main/ets/page/ModalPage.ets", src),
        &TransformOptions {
            bundle_name: Some("com.example.hexample".to_string()),
            module_name: Some("example".to_string()),
            project_root: Some(PathBuf::from("/project")),
            project_path: Some(PathBuf::from("/project/example/src/main/ets")),
            module_roots: vec![],
            component_metadata: std::sync::Arc::new(ComponentMetadata::default()),
            sources: crate::source::TransformSourceSnapshot::default(),
            module_exports: ModuleExportIndex::default(),
            navigation_routes: NavigationRouteMap::default(),
            form_entry: false,
            partial_update: true,
            checker_mode: crate::CheckerMode::Oxc,
        },
    )
    .unwrap();

    assert!(
        result.contains("let routeNameNode = ModalEntryName"),
        "object-literal @Entry routeName must be extracted like hvigor, got:\n{}",
        result
    );
    assert!(
        result.contains("new ModalPage(undefined, {}, LocalStorage.getShared())"),
        "object-literal @Entry storage must pass only the storage expression, got:\n{}",
        result
    );
    assert_eq!(
        result
            .matches("new ModalPage(undefined, {}, LocalStorage.getShared())")
            .count(),
        2,
        "routeName undefined fallback must retain object-literal storage like hvigor:\n{result}"
    );
    assert!(
        result.contains("routeNameNode"),
        "routeNameNode guard/route argument must be emitted, got:\n{}",
        result
    );
    assert!(
        !result.contains("new ModalPage(undefined, {}, {"),
        "the whole @Entry options object must not be passed as localStorage, got:\n{}",
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
    let result = TransformHarness::transform(&make_source(
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

#[test]
fn test_component_source_constructor_does_not_emit_second_constructor() {
    let src = r#"
@Component
struct Panel {
  @State title: string = "x";
  constructor() {
    super();
  }
  build() {
    Text(this.title)
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Panel.ets", src))
            .unwrap();
    assert_eq!(
        result.matches("constructor(").count(),
        1,
        "source constructor must not be cloned beside generated ViewPU constructor:\n{}",
        result
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "component constructor lowering must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_custom_dialog_emits_single_set_controller_for_multiple_controller_fields() {
    let src = r#"
@CustomDialog
struct DebugPanelDialogComponent {
  controller?: CustomDialogController
  private logPanelDialogController?: CustomDialogController
  private devConfigPanelDialogController?: CustomDialogController
  build() {
    Column()
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/DebugPanelDialogComponent.ets",
        src,
    ))
    .unwrap();
    assert_eq!(
        result.matches("setController(").count(),
        1,
        "@CustomDialog must generate one setController method:\n{}",
        result
    );
    assert!(
        result.contains("this.controller = ctr;"),
        "setController should bind the first controller field in source order:\n{}",
        result
    );
}

#[test]
fn test_custom_dialog_controller_builder_is_lazy_dialog_factory() {
    let src = r#"
@CustomDialog
struct DismissDialog {
  controller?: CustomDialogController
  title: string = ""
  build() {
    Column()
  }
}

@Component
struct Host {
  dialogController: CustomDialogController = new CustomDialogController({
    builder: DismissDialog({ title: "privacy" }),
    autoCancel: true
  })

  build() {
    Column()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Host.ets", src))
            .unwrap();
    assert!(
        !result.contains("builder: DismissDialog("),
        "CustomDialogController builder must not eagerly call generated dialog class:\n{}",
        result
    );
    assert!(
        result.contains("builder: () => {"),
        "CustomDialogController builder must be emitted as a lazy function:\n{}",
        result
    );
    assert!(
        result.contains("this.dialogController = new CustomDialogController({")
            && result.contains("builder: () => {"),
        "CustomDialogController constructor must keep hvigor's lazy builder object:\n{}",
        result
    );
    assert!(
        result.contains("}, this);"),
        "CustomDialogController constructor must receive the owning component as second argument:\n{}",
        result
    );
    assert!(
        result.contains(
            "let jsDialog = new DismissDialog(this, { title: \"privacy\" }, undefined, -1, () => { }, { page: \"entry/src/main/ets/pages/Host.ets\", line: 14, col: 14 });"
        ),
        "lazy builder should instantiate the dialog with the SDK source location argument:\n{}",
        result
    );
    assert!(
        result.contains("jsDialog.setController(this.dialogController);"),
        "lazy builder should attach the CustomDialogController field:\n{}",
        result
    );
    assert!(
        result.contains("ViewPU.create(jsDialog);"),
        "lazy builder should register the custom dialog ViewPU instance:\n{}",
        result
    );
    assert!(
        result.contains("jsDialog.paramsGenerator_ = paramsLambda;"),
        "lazy builder should preserve hvigor paramsGenerator_ wiring:\n{}",
        result
    );
    assert!(
        result.contains("autoCancel: true"),
        "non-builder CustomDialogController options must be preserved:\n{}",
        result
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "custom dialog controller lowering must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_custom_dialog_controller_builder_uses_link_backing_property() {
    let src = r#"
@CustomDialog
struct TimeDialog {
  @Link selectedTime: number
  build() {
    Column()
  }
}

@Component
struct Host {
  @State selectedTime: number = 10
  controller: CustomDialogController = new CustomDialogController({
    builder: TimeDialog({ selectedTime: this.selectedTime })
  })

  build() {
    Column()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Host.ets", src))
            .expect("custom dialog @Link parameters must lower");

    assert!(
        result.contains("let jsDialog = new TimeDialog(this, { selectedTime: this.__selectedTime },")
            && result.contains("return { selectedTime: this.__selectedTime };"),
        "custom dialog constructor and params lambda must receive the @Link backing property:\n{result}"
    );
    assert!(
        !result.contains("TimeDialog(this, { selectedTime: this.selectedTime }")
            && !result.contains("return { selectedTime: this.selectedTime };"),
        "the public @State getter cannot preserve custom-dialog two-way linkage:\n{result}"
    );
}

#[test]
fn test_custom_dialog_controller_builder_in_method_assignment_is_lazy_dialog_factory() {
    let src = r#"
@CustomDialog
struct DismissDialog {
  controller?: CustomDialogController
  title: string = ""
  build() {
    Column()
  }
}

@Component
struct Host {
  controller!: CustomDialogController

  aboutToAppear(): void {
    this.controller = new CustomDialogController({
      builder: DismissDialog({ title: "privacy" }),
      autoCancel: true
    })
  }

  build() {
    Column()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/Host.ets", src))
            .unwrap();
    assert!(
        !result.contains("builder: DismissDialog("),
        "method assignment builder must not eagerly call generated dialog class:\n{}",
        result
    );
    assert!(
        result.contains("this.controller = new CustomDialogController({")
            && result.contains("builder: () => {"),
        "method assignment must keep hvigor's lazy builder object:\n{}",
        result
    );
    assert!(
        result.contains("let jsDialog = new DismissDialog(this, { title: \"privacy\" }, undefined, -1, () => { }, { page: \"entry/src/main/ets/pages/Host.ets\", line: 17, col: 16 });"),
        "method assignment lazy builder should preserve the exact SDK source location:\n{}",
        result
    );
    assert!(
        result.contains("jsDialog.setController(this.controller);"),
        "method assignment lazy builder should attach the assignment target controller:\n{}",
        result
    );
    assert!(
        result.contains("}, this);"),
        "method assignment CustomDialogController constructor must receive the owning component:\n{}",
        result
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "method assignment custom dialog controller lowering must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_custom_dialog_controller_program_pass_covers_v2_fields_and_local_variables() {
    let src = r#"
@CustomDialog
struct DismissDialog {
  controller?: CustomDialogController
  title: string = ""
  build() {
    Column()
  }
}

@ComponentV2
struct Host {
  fieldController: CustomDialogController = new CustomDialogController({
    builder: DismissDialog({ title: "field" })
  })

  aboutToAppear(): void {
    const localController = new CustomDialogController({
      builder: DismissDialog({ title: "local" })
    })
    localController.open()
  }

  build() {
    Column()
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/pages/HostV2.ets", src))
            .unwrap();

    assert_eq!(
        result.matches("ViewPU.create(jsDialog);").count(),
        2,
        "SDK process_ui_syntax visits ComponentV2 property and variable declarations before class lowering:\n{result}"
    );
    assert!(
        result.contains("jsDialog.setController(this.fieldController);")
            && result.contains("jsDialog.setController(this.localController);"),
        "SDK createCustomComponentBuilderArrowFunction addresses property and variable declarations through this.<name>:\n{result}"
    );
    assert_eq!(
        result.matches("new CustomDialogController({").count(),
        2,
        "both controller constructors must remain present:\n{result}"
    );
    assert_eq!(
        result.matches("}, this);").count(),
        2,
        "both controller constructors must receive the SDK owner argument:\n{result}"
    );
}

#[test]
fn test_custom_dialog_controller_program_pass_covers_regular_class_assignment() {
    let src = r#"
@CustomDialog
struct DismissDialog {
  controller?: CustomDialogController
  build() {
    Column()
  }
}

class DialogManager {
  private static controller: CustomDialogController | null = null

  static reset(): void {
    DialogManager.controller = new CustomDialogController({
      builder: DismissDialog()
    })
  }
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/dialog/DialogManager.ets",
        src,
    ))
    .unwrap();

    assert!(
        result.contains("jsDialog.setController(DialogManager.controller);")
            && result.contains("ViewPU.create(jsDialog);"),
        "SDK process_ui_syntax visits CustomDialogController assignments in ordinary classes:\n{result}"
    );
    assert!(
        result.contains("DialogManager.controller = new CustomDialogController({")
            && result.contains("}, this);"),
        "ordinary class assignments preserve the constructor and add the SDK owner argument:\n{result}"
    );
}

#[test]
fn test_state_styles_inline_object_lowers_to_visual_state_sequence() {
    let src = r#"
@Component
struct Demo {
  build() {
    Text("x")
      .stateStyles({
        pressed: {
          .backgroundColor(Color.Transparent)
        }
      })
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Demo.ets", src))
            .unwrap();
    assert!(
        result.contains(
            "ViewStackProcessor.visualState(\"pressed\");\n            Text.backgroundColor(Color.Transparent);\n            ViewStackProcessor.visualState();"
        ),
        "stateStyles must lower to hvigor's visual-state sequence:\n{}",
        result
    );
    assert!(
        !result.contains("Text.stateStyles("),
        "stateStyles must not remain as a component runtime call:\n{}",
        result
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "stateStyles lowering must emit standard TypeScript: {:?}\n{}",
        parsed.diagnostics,
        result
    );
}

#[test]
fn test_top_level_builder_expands_file_level_styles() {
    let src = r#"
@Styles
function topicStyle() {
  .height(24)
  .backgroundColor(Color.White)
}

@Builder
export function LoadingRow() {
  Row()
    .topicStyle()
}
"#;
    let result = TransformHarness::transform(&make_source(
        "entry/src/main/ets/components/LoadingRow.ets",
        src,
    ))
    .expect("top-level @Builder must receive file-level @Styles metadata");

    assert!(
        result.contains("Row.height(24);")
            && result.contains("Row.backgroundColor(Color.White);")
            && !result.contains("Row.topicStyle();"),
        "global @Styles must expand in a top-level builder like process_component_build.js:\n{result}"
    );
}

#[test]
fn test_state_styles_expand_local_and_global_styles_in_source_order() {
    let src = r#"
@Styles
function disabledStyle() {
  .opacity(0.5)
}

@Component
struct Demo {
  @Styles normalStyle() {
    .backgroundColor(Color.White)
  }

  @Styles pressedStyle() {
    .backgroundColor('#eeeeee')
    .scale({ x: 0.98, y: 0.98 })
  }

  build() {
    Text("x")
      .stateStyles({
        normal: this.normalStyle,
        pressed: this.pressedStyle,
        disabled: disabledStyle
      })
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Demo.ets", src))
            .unwrap();
    let normal = result
        .find("ViewStackProcessor.visualState(\"normal\");")
        .expect("normal visual state");
    let normal_attr = result[normal..]
        .find("Text.backgroundColor(Color.White);")
        .map(|offset| normal + offset)
        .expect("normal style expansion");
    let pressed = result
        .find("ViewStackProcessor.visualState(\"pressed\");")
        .expect("pressed visual state");
    let pressed_color = result[pressed..]
        .find("Text.backgroundColor('#eeeeee');")
        .map(|offset| pressed + offset)
        .expect("pressed color expansion");
    let pressed_scale = result[pressed..]
        .find("Text.scale(")
        .map(|offset| pressed + offset)
        .expect("pressed scale expansion");
    let disabled = result
        .find("ViewStackProcessor.visualState(\"disabled\");")
        .expect("disabled visual state");
    let disabled_attr = result[disabled..]
        .find("Text.opacity(0.5);")
        .map(|offset| disabled + offset)
        .expect("global style expansion");
    let reset = result[disabled..]
        .find("ViewStackProcessor.visualState();")
        .map(|offset| disabled + offset)
        .expect("visual state reset");
    assert!(
        normal < normal_attr
            && normal_attr < pressed
            && pressed < pressed_color
            && pressed_color < pressed_scale
            && pressed_scale < disabled
            && disabled < disabled_attr
            && disabled_attr < reset,
        "stateStyles states and expanded attributes must retain hvigor source order:\n{}",
        result
    );
    assert!(
        !result.contains("normalStyle()")
            && !result.contains("pressedStyle()")
            && !result.contains("disabledStyle()"),
        "@Styles declarations and references must not leak into generated output:\n{}",
        result
    );
}

#[test]
fn test_custom_component_state_styles_lower_on_common_component() {
    let src = r#"
@Component
struct Child {
  build() {
    Text("child")
  }
}

@Component
struct Demo {
  @Styles pressedStyle() {
    .backgroundColor(Color.Transparent)
  }

  build() {
    Child()
      .stateStyles({ pressed: this.pressedStyle })
  }
}
"#;
    let result =
        TransformHarness::transform(&make_source("entry/src/main/ets/components/Demo.ets", src))
            .unwrap();
    assert!(
        result.contains(
            "ViewStackProcessor.visualState(\"pressed\");\n            __Common__.backgroundColor(Color.Transparent);\n            ViewStackProcessor.visualState();"
        ),
        "custom-component stateStyles must target hvigor's __Common__ wrapper:\n{}",
        result
    );
    assert!(
        !result.contains("__Common__.stateStyles("),
        "custom-component stateStyles must not remain as a runtime attribute:\n{}",
        result
    );
}

#[test]
fn test_collect_arkui_component_names_skips_custom_components_and_appends_if() {
    let src = r#"
@Component
struct CardItem {
  getImage() {
    if (this.imageFd != '') {
      return this.imageFd;
    }
    return '';
  }
  build() {
    Row() {
      Image('')
    }
  }
}

@Component
struct WidgetCard {
  build() {
    Column() {
      Row() {
        Text('title')
      }
      List() {
        ListItem() {
          CardItem()
        }
      }
    }
  }
}
"#;
    let names = TransformHarness::collect_component_names(
        src,
        "entry/src/main/ets/widget/pages/WidgetCard.ets",
    )
    .unwrap();
    assert_eq!(
        names,
        vec!["Row", "Image", "Column", "Text", "List", "ListItem", "If"]
    );
}

#[test]
fn test_collect_arkui_component_names_skips_imported_custom_widget_like_hvigor() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let package_root = project_root.join("oh_modules/@example/card_subscribe");
    let index_path = package_root.join("index.ets");
    let widget_path = package_root.join("src/main/ets/widget/pages/Member2_2Widget.ets");
    let index_source =
        "export { Member2_2Widget } from './src/main/ets/widget/pages/Member2_2Widget';";
    let widget_source = r#"
@Component
export struct Member2_2Widget {
  build() {}
}
"#;
    let src = r#"
import { Member2_2Widget } from "@example/card_subscribe";

@Component
struct MemberForm {
  build() {
    Column() {
      Member2_2Widget()
    }
  }
}
"#;
    let mut sources = rustc_hash::FxHashMap::default();
    sources.insert(
        arkdown_path::lexical_normalize(&index_path),
        std::sync::Arc::<str>::from(index_source),
    );
    sources.insert(
        arkdown_path::lexical_normalize(&widget_path),
        std::sync::Arc::<str>::from(widget_source),
    );
    let sources = crate::source::TransformSourceSnapshot::new(sources);
    let options = TransformOptions {
        project_root: Some(project_root.clone()),
        sources: sources.clone(),
        module_exports: ModuleExportIndex::build(&sources, &[], &ComponentMetadata::default()),
        ..Default::default()
    };
    let names = TransformHarness::collect_component_names_with_options(
        src,
        project_root
            .join("example/src/main/ets/widget/pages/member2_2.ets")
            .to_str()
            .unwrap(),
        &options,
    )
    .unwrap();

    assert_eq!(names, vec!["Column"]);
}
