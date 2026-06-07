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
fn test_strips_arkui_decorators_from_regular_class() {
    let src = "@Observed\nexport class UserModel {\n  @Trace name: string = 'guest';\n}";
    let result = transform_ets(&make_source("entry/src/main/ets/model/UserModel.ets", src))
        .expect("regular observed class must transform");

    assert!(result.contains("export class UserModel"));
    assert!(result.contains("name: string = 'guest';"));
    assert!(
        !result.contains("@Observed"),
        "@Observed must be consumed before ABC:\n{result}"
    );
    assert!(
        !result.contains("@Trace"),
        "@Trace must be consumed before ABC:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.errors.is_empty(),
        "decorator-stripped class must be standard TypeScript: {:?}\n{}",
        parsed.errors,
        result
    );
}

#[test]
fn test_strips_arkui_decorators_from_default_export_class() {
    let src = "@Observed\nexport default class LazyDataSource<T> {\n  dataArray: T[] = [];\n}";
    let result = transform_ets(&make_source(
        "commons/utils/src/main/ets/utils/LazyDataSource.ets",
        src,
    ))
    .expect("default export observed class must transform");

    assert!(result.contains("export default class LazyDataSource<T>"));
    assert!(result.contains("dataArray: T[] = [];"));
    assert!(
        !result.contains("@Observed"),
        "@Observed must be consumed before ABC:\n{result}"
    );

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.errors.is_empty(),
        "decorator-stripped default export class must be standard TypeScript: {:?}\n{}",
        parsed.errors,
        result
    );
}

#[test]
fn test_transforms_component_struct_with_empty_build() {
    let src = "@Component\nstruct MyView {\n  build() {}\n}";
    let result = transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
    assert!(result.contains("class MyView extends ViewPU"));
    assert!(result.contains("initialRender()"));
    assert!(!result.contains("struct MyView"));
}

#[test]
fn test_transforms_export_component_without_leaking_decorator() {
    let src = "import { Foo } from './foo';\n\n@Component\nexport struct MyView {\n  @State message: string = \"hi\";\n  build() {}\n}";
    let result = transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
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
    let result = transform_ets(&make_source("entry/src/main/ets/pages/Multi.ets", src)).unwrap();
    assert!(result.starts_with("if (!(\"finalizeConstruction\" in ViewPU.prototype))"));
    let second = result.find("interface Second_Params").unwrap();
    let first = result.find("interface First_Params").unwrap();
    let import = result.find("import { Foo }").unwrap();
    let first_class = result.find("export class First extends ViewPU").unwrap();
    assert!(second < first);
    assert!(first < import);
    assert!(import < first_class);
    assert!(!result.contains("Copyright header"));
}

#[test]
fn test_transforms_default_export_component() {
    let src = "@Component\nexport default struct MyView {\n  build() {}\n}";
    let result = transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
    assert!(result.contains("export default class MyView extends ViewPU"));
    assert!(!result.contains("export default struct MyView"));
}

#[test]
fn test_transforms_component_v2_property_decorators() {
    let src = "@ComponentV2\nstruct MyView {\n  @Local count: number = 0;\n  @Param title: string = '';\n  @Provider('theme') theme: string = 'dark';\n  @Consumer('theme') incoming: string = '';\n  @Event onTap: () => void = () => {};\n  @Computed\n  get label(): string { return this.title; }\n  build() {}\n}";
    let result = transform_ets(&make_source("entry/src/main/ets/pages/MyView.ets", src)).unwrap();
    assert!(result.contains("class MyView extends ViewPU"));
    assert!(result.contains("private __count: ObservedPropertySimplePU<number>"));
    assert!(result.contains("private __title: SynchedPropertySimpleOneWayPU<string>"));
    assert!(result.contains("this.__incoming = this.initializeConsume(\"theme\", \"incoming\")"));
    assert!(result.contains("this.onTap = () => {};"));
    assert!(result.contains("if (params.onTap !== undefined)"));
    assert!(result.contains("private onTap: () => void;"));
    assert!(result.contains("get label(): string { return this.title; }"));
    assert!(!result.contains("@ComponentV2"));
    assert!(!result.contains("@Local"));
    assert!(!result.contains("@Param"));
    assert!(!result.contains("@Provider"));
    assert!(!result.contains("@Consumer"));
    assert!(!result.contains("@Event"));
    assert!(!result.contains("@Computed"));
}

#[test]
fn test_rejects_struct_without_component_decorator_like_hvigor_checker() {
    let src = "struct Plain {\n  build() {}\n}";
    let err = transform_ets(&make_source("entry/src/main/ets/pages/Plain.ets", src))
        .expect_err("plain ArkTS struct must fail in etsChecker");
    assert!(
        err.to_string()
            .contains("Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_rejects_component_v1_v2_decorator_mix() {
    let src = "@Component\n@ComponentV2\nstruct Mixed {\n  build() {}\n}";
    let err = transform_ets(&make_source("entry/src/main/ets/pages/Mixed.ets", src))
        .expect_err("@Component and @ComponentV2 must be mutually exclusive");
    assert!(
        err.to_string()
            .contains("can not be decorated with '@ComponentV2'"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_lowers_all_oxc_arkts_extra_nodes_to_standard_ts() {
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
    let result = transform_ets(&make_source("entry/src/main/ets/pages/Page.ets", src)).unwrap();
    assert!(result.contains("interface RouteMeta"));
    assert!(result.contains("import LazyPage, { Helper } from './lazy';"));
    assert!(result.contains("class Page extends ViewPU"));
    assert!(!result.contains("@interface"));
    assert!(!result.contains("import lazy"));
    assert!(!result.contains("struct Page"));
    assert!(!result.contains("Column() {"));

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &result,
        oxc_span::SourceType::ts().with_module(true),
    )
    .parse();
    assert!(
        parsed.errors.is_empty(),
        "lowered output must be standard TypeScript: {:?}\n{}",
        parsed.errors,
        result
    );
}

#[test]
fn test_transforms_export_builder_function_decorator() {
    let src =
        "@Builder\nexport function Card(title: string) {\n  Column() {\n    Text(title)\n  }\n}";
    let result =
        transform_ets(&make_source("entry/src/main/ets/components/Card.ets", src)).unwrap();
    assert!(result.contains("export function Card(title: string, parent = null)"));
    assert!(result.contains("observeComponentCreation2"));
    assert!(!result.contains("@Builder"));
    assert!(!result.contains("Column() {"));
}

#[test]
fn test_top_level_builder_preserves_explicit_return_type() {
    let src = "@Builder\nexport function Card(): void {\n  Text('card')\n}";
    let result =
        transform_ets(&make_source("entry/src/main/ets/components/Card.ets", src)).unwrap();
    assert!(result.contains("export function Card(parent = null): void"));
}

#[test]
fn test_transforms_builder_before_export_function() {
    let src = "@Builder\nexport function LearningItem(learningItem: LearningPath) {\n  Column() {\n    Image(learningItem.thumbnail)\n      .width(LearningConstants.FULL_PERCENT)\n  }\n  .clip(true)\n}";
    let result = transform_ets(&make_source(
        "entry/src/main/ets/components/LearningItem.ets",
        src,
    ))
    .unwrap();
    assert!(!result.contains("@Builder"));
    assert!(!result.contains("Column() {"));
    assert!(result.contains(".clip(true);"));
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
    let result = transform_ets(&make_source(
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
        parsed.errors.is_empty(),
        "lowered output must be valid TypeScript: {:?}\n{}",
        parsed.errors,
        result
    );
}

#[test]
fn test_transforms_entry_component_with_hvigor_route_registration() {
    let src = "@Entry\n@Component\nstruct Page {\n  build() {}\n}";
    let options = TransformOptions {
        bundle_name: Some("com.example.arkdown.test".to_string()),
        module_name: Some("phone".to_string()),
        project_root: None,
        module_roots: Vec::new(),
        component_metadata: ComponentMetadata::default(),
    };
    let result = transform_ets_with_options(
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
fn test_minimal_index_component_lowers_arkui_semantics() {
    let src =
        include_str!("../../../../tests/fixtures/minimal-hap/entry/src/main/ets/pages/Index.ets");
    let result = transform_ets(&make_source("entry/src/main/ets/pages/Index.ets", src)).unwrap();

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

#[test]
fn test_imported_reexported_enum_state_is_simple_like_hvigor_enum_collection() {
    let temp = tempfile::tempdir().unwrap();
    let utils_root = temp.path().join("commons/utils");
    let enum_dir = utils_root.join("src/main/ets/common");
    std::fs::create_dir_all(&enum_dir).unwrap();
    std::fs::write(
        utils_root.join("Index.ets"),
        r#"
export {
  EventTypeEnum,
  LoadingStatus,
  WebpageStatus,
  ResourcesType
} from './src/main/ets/common/CommonEnums';
"#,
    )
    .unwrap();
    std::fs::write(
            enum_dir.join("CommonEnums.ets"),
            "export enum LoadingStatus { OFF, LOADING, SUCCESS, FAILED }\nexport enum EventTypeEnum { A }\nexport class ResourcesType {}\n",
        )
        .unwrap();

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
    let options = TransformOptions {
        module_roots: vec![("utils".to_string(), utils_root)],
        ..Default::default()
    };
    let result = transform_ets_with_options(
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
    let result = transform_ets(&make_source(
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
    let result = transform_ets(&make_source(
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
    let result = transform_ets(&make_source(
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
    let result = transform_ets(&make_source(
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
fn test_prop_decorator_lowers_to_synched_property_one_way_pu() {
    let src = r#"
@Component
struct CardItem {
  @Prop itemData: string;
  build() {}
}
"#;
    let result = transform_ets(&make_source(
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
    let result = transform_ets(&make_source(
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
    let result = transform_ets(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("let componentCall = new Child(this, {")
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
    let result = transform_ets(&make_source(
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
    assert!(
        !result.contains("item: ObservedObject.GetRawObject(this.item)"),
        "GetRawObject must be limited to call arguments, got:\n{}",
        result
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
    let result = transform_ets(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
            result.contains("let componentCall = new Child(this, {\n        value: this.__value"),
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
fn test_for_each_wraps_raw_observed_state_data_argument() {
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
    let result = transform_ets(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
            result.contains(
                "this.forEachUpdateFunction(elmtId, ObservedObject.GetRawObject(this.items), forEachItemGenFunction)"
            ),
            "expected ForEach data argument to match upstream GetRawObject lowering, got:\n{}",
            result
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
    let result = transform_ets(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains(
            "this.forEachUpdateFunction(elmtId, ObservedObject.GetRawObject(this.items), forEachItemGenFunction, undefined, true, false)"
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
    Child({ itemData: this.value }).reuseId('child')
  }
}
"#;
    let result = transform_ets(&make_source(
        "products/phone/src/main/ets/pages/Parent.ets",
        src,
    ))
    .unwrap();
    assert!(
        result.contains("this.observeRecycleComponentCreation('child'")
            && result.contains("ViewPU.createRecycle(componentCall, recycleNode !== null, 'child'")
            && result.contains("new Child(this, {")
            && result.contains("itemData: this.value")
            && result.contains("recycleNode.aboutToReuse({"),
        "expected @Reusable custom component recycle path, got:\n{}",
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
fn test_entry_with_storage_lowers_to_load_ets_card() {
    let src = r#"
let storage = new LocalStorage();

@Entry(storage)
@Component
struct WidgetCard {
  build() {}
}
"#;
    let result = transform_ets_with_options(
        &make_source("phone/src/main/ets/widget/pages/WidgetCard.ets", src),
        &TransformOptions {
            bundle_name: Some("com.example.hmosworld".to_string()),
            module_name: Some("phone".to_string()),
            project_root: None,
            module_roots: vec![],
            component_metadata: ComponentMetadata::default(),
        },
    )
    .unwrap();
    assert!(
            result.contains("loadEtsCard(new WidgetCard(undefined, {}, storage), \"com.example.hmosworld/phone/ets/widget/pages/WidgetCard\")"),
            "expected widget entry to use loadEtsCard, got:\n{}",
            result
        );
    assert!(
        !result.contains("registerNamedRoute(() => new WidgetCard"),
        "widget entry must not use page route registration, got:\n{}",
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
    let result = transform_ets(&make_source(
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
    let names =
        collect_arkui_component_names(src, "entry/src/main/ets/widget/pages/WidgetCard.ets")
            .unwrap();
    assert_eq!(
        names,
        vec!["Row", "Image", "Column", "Text", "List", "ListItem", "If"]
    );
}
