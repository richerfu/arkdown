use std::{path::PathBuf, sync::Arc};

use arkdown_ets_transform::{
    error::EtsTransformError, transform::component_meta::ComponentMetadata, ArktsSourceChecker,
    EtsChecker, TransformSourceSnapshot,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

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
fn arkts11_source_checker_aggregates_sdk_api_diagnostics_without_transforming() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let report = checker.check(
        "ApiAvailable.ets",
        "import deviceInfo from '@ohos.deviceInfo';\n\
         deviceInfo.apiAvailable(0);\n\
         deviceInfo.apiAvailable('25.0.0');",
    );
    let codes = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic { code, .. } => Some(code.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(codes, ["11706013", "11706013"]);
    assert_eq!(report.error_count(), 2);
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
