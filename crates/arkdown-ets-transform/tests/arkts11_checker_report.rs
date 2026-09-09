use std::{path::PathBuf, sync::Arc};

use arkdown_core::ArktsLanguageVersion;
use arkdown_ets_transform::{
    error::EtsTransformError, transform::component_meta::ComponentMetadata, ArktsFileTypeFacts,
    ArktsSourceChecker, ArktsTypeFactsSnapshot, EtsChecker, PropertyTypeFacts,
    ResolvedTypeIdentity, SpannedPropertyTypeFacts, TransformSourceSnapshot,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

#[test]
fn arkts12_checker_accepts_source_owned_use_static_before_imports() {
    let report = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_arkts_language_version(ArktsLanguageVersion::V1_2)
        .check(
            "StaticEntry.ets",
            "'use static';\nimport UIAbility from '@ohos.app.ability.UIAbility';\nclass EntryAbility extends UIAbility {}\n",
        );

    assert!(
        report.diagnostics().is_empty(),
        "static source must parse without import-order diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_checker_report_preserves_all_source_order_diagnostics() {
    let allocator = Allocator::default();
    let source = r#"
@Component
struct First {
  @Link value: number = 0
}
@Observed
class Model {
  @Trace value: number = 0
}
"#;
    let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

    let report = EtsChecker::new(&parsed.program, "AllDiagnostics.ets").check();
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["10905103", "10905304", "10905343"]);
    assert_eq!(report.error_count(), 3);
    assert_eq!(report.into_diagnostics().len(), 3);
}

#[test]
fn arkts11_extend_and_animatable_extend_report_both_source_phases() {
    let allocator = Allocator::default();
    let source = "@Extend(Text) @AnimatableExtend(Text) function sharedStyle() {}";
    let parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

    let report = EtsChecker::new(&parsed.program, "ExtendConflict.ets").check();
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["10905117", "10905111"]);
}

#[test]
fn arkts11_component_v2_property_contracts_keep_source_codes_and_order() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let report = checker.check(
        "ComponentV2Contracts.ets",
        r#"
@ComponentV2
struct ComponentV2Contracts {
  @Param param: string
  @Require @Local required: string = ''
  @Once @Local once: string = ''
  @Env(SystemProperties.BREAK_POINT) env: WindowSizeLayoutBreakpointInfo = new WindowSizeLayoutBreakpointInfo()
  @Local inferred = 1
  build() {}
}
"#,
    );
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        ["10905327", "10905325", "10905326", "10905362", "10905328"]
    );
}

#[test]
fn arkts11_custom_component_arguments_use_the_shared_component_inventory() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let report = checker.check(
        "CustomComponentArguments.ets",
        r#"
@ComponentV2
struct Child {
  @Require @Param required: string
  @Local local: string = ''
  build() {}
}
@Component
struct Parent {
  build() { Child({ local: 'override' }) }
}
"#,
    );
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["10905324", "10905321"]);
}

#[test]
fn arkts11_custom_component_contracts_follow_imported_exports() {
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        PathBuf::from("/project/Child.ets"),
        Arc::<str>::from(
            "@ComponentV2\nexport struct Child {\n  @Require @Param value: string\n  build() {}\n}\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(TransformSourceSnapshot::new(files), Vec::new());
    let report = checker.check(
        "/project/Parent.ets",
        "import { Child } from './Child';\n@Component struct Parent { build() { Child() } }",
    );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905321"
        )
    }));
}

#[test]
fn arkts11_v1_custom_component_contracts_follow_imported_exports() {
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        PathBuf::from("/project/Child.ets"),
        Arc::<str>::from(
            "@Component\nexport struct Child {\n  @StorageProp('stored') stored: number = 0\n  @Require @Prop required: number\n  @Link linked: number\n  build() {}\n}\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(TransformSourceSnapshot::new(files), Vec::new());
    let report = checker.check(
        "/project/Parent.ets",
        "import { Child } from './Child';\n@Component struct Parent { build() { Child({ stored: 1 }) } }",
    );
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["10905317", "10905359", "10905316"]);
}

#[test]
fn arkts11_v1_to_v2_assignment_uses_parent_property_type_identity() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let report = checker.check(
        "V1ToV2Assignment.ets",
        r#"
@ObservedV2
class Model { @Trace value: number = 0 }
@ComponentV2
struct Child { @Param data: Model = new Model(); build() {} }
@Component
struct Parent {
  @State data: Model = new Model()
  build() { Child({ data: this.data }) }
}
"#,
    );
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["10905348", "10905323"]);
}

#[test]
fn arkts11_source_checker_uses_sdk_component_inventory_for_name_collisions() {
    let loader = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ets-loader");
    let metadata = ComponentMetadata::from_ets_loader_path(&loader).expect("fixture metadata");
    let report = ArktsSourceChecker::new(Arc::new(metadata))
        .check("BuiltinName.ets", "@Component struct Text { build() {} }");

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905227"
        )
    }));
}

#[test]
fn arkts11_esmodule_imports_do_not_run_legacy_builtin_name_validation() {
    let loader = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ets-loader");
    let metadata = ComponentMetadata::from_ets_loader_path(&loader).expect("component metadata");
    let report = ArktsSourceChecker::new(Arc::new(metadata)).check(
        "EsmoduleImport.ets",
        "import { List } from './models';\n@Component struct Page { build() {} }",
    );

    assert!(!report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        EtsTransformError::CodedDiagnostic { code, .. } if code == "10905235"
    )));
}

#[test]
fn arkts11_entry_root_uses_sdk_container_metadata() {
    let loader = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ets-loader");
    let metadata = ComponentMetadata::from_ets_loader_path(&loader).expect("fixture metadata");
    let checker = ArktsSourceChecker::new(Arc::new(metadata));
    let report = checker.check(
        "AtomicEntry.ets",
        "@Entry @Component struct AtomicEntry { build() { Image('not a container') } }",
    );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905210"
        )
    }));

    let custom_report = checker.check(
        "CustomEntry.ets",
        "@Component struct Child { build() {} } @Entry @Component struct CustomEntry { build() { Child() } }",
    );
    assert!(custom_report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905210"
        )
    }));
}

#[test]
fn arkts11_state_controller_types_use_source_forbidden_type_inventory() {
    let loader = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ets-loader");
    let metadata = ComponentMetadata::from_ets_loader_path(&loader).expect("fixture metadata");
    let report = ArktsSourceChecker::new(Arc::new(metadata)).check(
        "ControllerState.ets",
        "@Component struct ControllerState { @State controller: Scroller = new Scroller(); build() {} }",
    );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905308"
        )
    }));
}

#[test]
fn arkts11_source_checker_reuses_project_export_closure_for_imported_inheritance() {
    let base_path = PathBuf::from("/project/model/Base.ets");
    let barrel_path = PathBuf::from("/project/model/Index.ets");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        base_path,
        Arc::<str>::from("@Observed\nexport class Base {}\n"),
    );
    files.insert(
        barrel_path,
        Arc::<str>::from("export { Base as LegacyBase } from './Base';\n"),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(TransformSourceSnapshot::new(files), Vec::new());
    let report = checker.check(
        "/project/Page.ets",
        "import { LegacyBase as Parent } from './model/Index';\n@ObservedV2\nclass Child extends Parent {}\n",
    );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905225"
        )
    }));
}

#[test]
fn arkts11_object_link_uses_resolved_typescript_facts_after_nullable_filtering() {
    let source_id = "ObjectLink.ets";
    let source = r#"
class Item {}
@Component
struct Child {
  @ObjectLink item: Item | null;
  build() {}
}
@Component
struct Parent {
  local: Item = new Item();
  build() { Child({ item: local }) }
}
"#;
    let plain = ResolvedTypeIdentity {
        is_nullable: false,
        is_union: false,
        is_enum: false,
        is_basic: false,
        is_observed_v2: false,
        is_function: false,
        symbol_name: "Item".to_string(),
        types: Vec::new(),
    };
    let nullable = ResolvedTypeIdentity {
        is_nullable: true,
        is_union: false,
        is_enum: false,
        is_basic: false,
        is_observed_v2: false,
        is_function: false,
        symbol_name: String::new(),
        types: Vec::new(),
    };
    let facts = ArktsFileTypeFacts {
        properties: vec![SpannedPropertyTypeFacts {
            span: Span::new(0, source.len() as u32),
            facts: PropertyTypeFacts {
                declared_type: Some(ResolvedTypeIdentity {
                    is_nullable: false,
                    is_union: true,
                    is_enum: false,
                    is_basic: false,
                    is_observed_v2: false,
                    is_function: false,
                    symbol_name: String::new(),
                    types: vec![plain, nullable],
                }),
            },
        }],
        ..ArktsFileTypeFacts::default()
    };
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default())).with_type_facts(
        ArktsTypeFactsSnapshot::new(rustc_hash::FxHashMap::from_iter([(
            PathBuf::from(source_id),
            facts,
        )])),
    );
    let report = checker.check(source_id, source);

    assert!(
        !report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "10905307" || code == "10905315"
        )),
        "{:#?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_object_link_rejects_observed_v2_from_typescript_identity() {
    let source_id = "ObservedObjectLink.ets";
    let source = "@ObservedV2 class Model {}\n@Component struct Child { @ObjectLink model: Model; build() {} }";
    let facts = ArktsFileTypeFacts {
        properties: vec![SpannedPropertyTypeFacts {
            span: Span::new(0, source.len() as u32),
            facts: PropertyTypeFacts {
                declared_type: Some(ResolvedTypeIdentity {
                    is_nullable: false,
                    is_union: false,
                    is_enum: false,
                    is_basic: false,
                    is_observed_v2: true,
                    is_function: false,
                    symbol_name: "Model".to_string(),
                    types: Vec::new(),
                }),
            },
        }],
        ..ArktsFileTypeFacts::default()
    };
    let report = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_type_facts(ArktsTypeFactsSnapshot::new(
            rustc_hash::FxHashMap::from_iter([(PathBuf::from(source_id), facts)]),
        ))
        .check(source_id, source);

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        EtsTransformError::CodedDiagnostic { code, .. } if code == "10905307"
    )));
}
