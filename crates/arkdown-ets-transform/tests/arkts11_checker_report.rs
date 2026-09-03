use std::{path::PathBuf, sync::Arc};

use arkdown_ets_transform::{
    error::EtsTransformError, transform::component_meta::ComponentMetadata, ArktsSourceChecker,
    EtsChecker, SystemApiCheckOptions, TransformSourceSnapshot,
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
fn arkts11_source_checker_validates_only_sdk_available_annotations() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let report = checker.check(
        "AvailableFormat.ets",
        "import { Available as SdkAvailable } from '@ohos.annotation';\n\
         @SdkAvailable({ minApiVersion: '25.0.0' }) class Feature {}",
    );
    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "11706016"
        )
    }));

    let collision = checker.check(
        "AvailableCollision.ets",
        "function Available(value: object): ClassDecorator { return () => {}; }\n\
         @Available({ minApiVersion: '25.0.0' }) class Feature {}",
    );
    assert!(collision.diagnostics().iter().all(|diagnostic| {
        !matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. }
                if matches!(code.as_str(), "11706016" | "11706017")
        )
    }));

    let shadowed = checker.check(
        "AvailableShadowed.ets",
        "import { Available } from '@ohos.annotation';\n\
         function scope(Available: any): void {\n\
           @Available({ minApiVersion: '25.0.0' }) class Feature {}\n\
         }",
    );
    assert!(shadowed.diagnostics().iter().all(|diagnostic| {
        !matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. }
                if matches!(code.as_str(), "11706016" | "11706017")
        )
    }));
}

#[test]
fn arkts11_source_checker_reports_available_os_and_parent_scope_contracts() {
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()));
    let os_report = checker.check(
        "AvailableOs.ets",
        "import { Available } from '@kit.BasicServicesKit';\n\
         @Available({ minApiVersion: 'HarmonyOS 6.0.0' }) class Feature {}",
    );
    assert!(os_report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "11706017"
        )
    }));

    let hierarchy = checker.check(
        "AvailableHierarchy.ets",
        "import { Available } from '@ohos.annotation';\n\
         @Available({ minApiVersion: '28' })\n\
         class Feature { @Available({ minApiVersion: '27' }) run(): void {} }",
    );
    assert!(hierarchy.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::Diagnostic {
                message,
                severity: arkdown_ets_transform::error::DiagnosticSeverity::Warning,
                ..
            } if message.contains("outer annotation") && message.contains("28")
        )
    }));
    assert_eq!(hierarchy.error_count(), 0);
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

#[test]
fn arkts11_system_api_inventory_resolves_sdk_namespace_since_contracts() {
    let sdk_root = PathBuf::from("/sdk/api");
    let declaration_path = sdk_root.join("@ohos.demo.d.ts");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        declaration_path,
        Arc::<str>::from(
            "declare namespace demo {\n\
             /** @since 13 */\n\
             export function modern(): void;\n\
             }\n\
             export default demo;\n",
        ),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import demo from '@ohos.demo';\ndemo.modern();\n",
    );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic {
                code,
                severity: arkdown_ets_transform::error::DiagnosticSeverity::Warning,
                ..
            } if code == "11706011"
        )
    }));
    assert_eq!(report.error_count(), 0);
}

#[test]
fn arkts11_system_api_since_uses_symbol_identity_and_source_suppressors() {
    let sdk_root = PathBuf::from("/sdk/api");
    let declaration_path = sdk_root.join("@ohos.demo.d.ts");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        declaration_path,
        Arc::<str>::from("/** @since 13 */ export declare function modern(): void;\n"),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { modern as sdkModern } from '@ohos.demo';\n\
         function modern(): void {}\n\
         modern();\n\
         try { sdkModern(); } catch (_) {}\n\
         if (sdkModern !== undefined) { sdkModern(); }\n\
         sdkModern();\n",
    );
    let since_diagnostics = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(since_diagnostics, 1);
}

#[test]
fn arkts11_system_api_permission_and_syscap_use_native_build_context() {
    let sdk_root = PathBuf::from("/sdk/api");
    let declaration_path = sdk_root.join("@ohos.demo.d.ts");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        declaration_path,
        Arc::<str>::from(
            "/**\n\
             * @permission ohos.permission.CAMERA and (ohos.permission.LOCATION or ohos.permission.APPROXIMATELY_LOCATION)\n\
             * @permission [since 13 - 15] ohos.permission.FUTURE\n\
             * @syscap SystemCapability.Camera.Core\n\
             */\n\
             export declare function capture(): void;\n",
        ),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let context = SystemApiCheckOptions::default()
        .with_compatible_sdk_version("12")
        .with_compile_sdk_version("12")
        .with_request_permissions(["ohos.permission.CAMERA", "ohos.permission.LOCATION"])
        .with_system_capabilities(
            ["SystemCapability.ArkUI.ArkUI.Full"],
            [
                "SystemCapability.ArkUI.ArkUI.Full",
                "SystemCapability.Camera.Core",
            ],
        );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_system_api_context(context);
    let warned = checker.check(
        "/project/Page.ets",
        "import { capture } from '@ohos.demo';\ncapture();\n",
    );
    assert!(warned.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::Diagnostic { message, .. }
                if message.contains("canIUse condition")
        )
    }));
    assert!(warned.diagnostics().iter().all(|diagnostic| {
        !matches!(
            diagnostic,
            EtsTransformError::Diagnostic { message, .. }
                if message.contains("apply for the permissions")
        )
    }));

    let guarded = checker.check(
        "/project/Page.ets",
        "import { capture } from '@ohos.demo';\n\
         if (canIUse('SystemCapability.Camera.Core')) { capture(); }\n",
    );
    assert!(guarded.diagnostics().iter().all(|diagnostic| {
        !matches!(
            diagnostic,
            EtsTransformError::Diagnostic { message, .. }
                if message.contains("system capacity")
        )
    }));
}

#[test]
fn arkts11_system_api_inventory_follows_sdk_barrels_and_validates_guard_identity() {
    let api_root = PathBuf::from("/sdk/api");
    let kit_root = PathBuf::from("/sdk/kits");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        api_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from("/** @since 13 */ export declare function modern(): void;\n"),
    );
    files.insert(
        kit_root.join("@kit.DemoKit.d.ets"),
        Arc::<str>::from("export { modern as newer } from '../api/@ohos.demo';\n"),
    );
    let sources =
        TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![api_root, kit_root]);
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_system_api_context(
            SystemApiCheckOptions::default()
                .with_compatible_sdk_version("12")
                .with_strict_api_compatibility(true),
        );
    let false_guard = checker.check(
        "/project/Page.ets",
        "import { newer as guarded } from '@kit.DemoKit';\n\
         const deviceInfo = { sdkApiVersion: 99 };\n\
         if (deviceInfo.sdkApiVersion >= 13) { guarded(); }\n",
    );
    assert_eq!(false_guard.error_count(), 1);

    let sdk_guard = checker.check(
        "/project/Page.ets",
        "import { newer as guarded } from '@kit.DemoKit';\n\
         import deviceInfo from '@ohos.deviceInfo';\n\
         if (deviceInfo.sdkApiVersion >= 13) { guarded(); }\n",
    );
    assert_eq!(sdk_guard.error_count(), 0);
    assert!(sdk_guard.diagnostics().iter().all(|diagnostic| {
        !matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
    }));
}
