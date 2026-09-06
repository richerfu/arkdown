use std::{path::PathBuf, sync::Arc};

use arkdown_core::ArktsLanguageVersion;
use arkdown_ets_transform::{
    error::{DiagnosticSeverity, EtsTransformError},
    transform::component_meta::ComponentMetadata,
    ArktsSourceChecker, EtsChecker, SystemApiCheckOptions, TransformSourceSnapshot,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

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
         const capability = 'SystemCapability.Camera.Core';\n\
         if (canIUse(capability)) { capture(); }\n",
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
         const minimum = 13;\n\
         if (deviceInfo.sdkApiVersion >= minimum) { guarded(); }\n",
    );
    assert_eq!(sdk_guard.error_count(), 0);
    assert!(sdk_guard.diagnostics().iter().all(|diagnostic| {
        !matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
    }));
}

#[test]
fn arkts11_available_usage_is_resolved_across_project_exports() {
    let feature_path = PathBuf::from("/project/Feature.ets");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        feature_path,
        Arc::<str>::from(
            "import { Available } from '@ohos.annotation';\n\
             @Available({ minApiVersion: '13' })\n\
             export function modern(): void {}\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files),
            vec![("entry".to_string(), PathBuf::from("/project"))],
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { modern } from './Feature';\n\
         try { modern(); } catch (_) {}\n\
         modern();\n",
    );
    let available = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706012")
        })
        .count();

    // `available_warning_suppressor.ts` intentionally does not accept a
    // try/catch guard, unlike the @since suppressor.
    assert_eq!(available, 2);
}

#[test]
fn arkts11_system_api_enforces_stage_form_atomic_and_test_tags() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "/**\n\
             * @form\n\
             * @atomicservice\n\
             */ export declare function supported(): void;\n\
             export declare function unsupported(): void;\n\
             /** @famodelonly */ export declare function faOnly(): void;\n\
             /** @test */ export declare function testOnly(): void;\n",
        ),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let context = SystemApiCheckOptions::default()
        .with_compatible_sdk_version("12")
        .with_compile_sdk_version("12")
        .with_form_source(true)
        .with_atomic_service(true);
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_system_api_context(context);
    let report = checker.check(
        "/project/Page.ets",
        "import { supported, unsupported, faOnly, testOnly } from '@ohos.demo';\n\
         supported(); unsupported(); faOnly(); testOnly();\n",
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
        ["11706006", "11706010", "11706006", "11706010", "11706008", "11706006", "11706010",]
    );
    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::Diagnostic { message, .. }
                if message.contains("testing directories")
        )
    }));

    let test_report = checker.check(
        "/project/src/ohosTest/Test.ets",
        "import { testOnly } from '@ohos.demo';\ntestOnly();\n",
    );
    assert!(test_report.diagnostics().iter().all(|diagnostic| {
        !matches!(
            diagnostic,
            EtsTransformError::Diagnostic { message, .. }
                if message.contains("testing directories")
        )
    }));
}

#[test]
fn arkts11_system_api_enforces_crossplatform_tag_and_version_range() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "/** @crossplatform */ export declare function supported(): void;\n\
             /** @crossplatform [since 10 - 12] */ export declare function supportedAtTwelve(): void;\n\
             /** @crossplatform [since 13 - 15] */ export declare function future(): void;\n\
             export declare function unsupported(): void;\n",
        ),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let source =
        "import { supported, supportedAtTwelve, future, unsupported } from '@ohos.demo';\n\
                  supported(); supportedAtTwelve(); future(); unsupported();\n";
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources.clone(), Vec::new())
        .with_system_api_context(
            SystemApiCheckOptions::default()
                .with_compile_sdk_version("12")
                .with_crossplatform(true, false),
        );
    let report = checker.check("/project/Page.ets", source);
    let diagnostics = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            EtsTransformError::CodedDiagnostic {
                code,
                severity,
                message,
                ..
            } if code == "11706007" => Some((severity, message.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2, "{:?}", report.diagnostics());
    assert!(diagnostics
        .iter()
        .all(|(severity, _)| **severity == DiagnosticSeverity::Error));
    assert!(diagnostics
        .iter()
        .any(|(_, message)| message.contains("future")));
    assert!(diagnostics
        .iter()
        .any(|(_, message)| message.contains("unsupported")));

    let ignored = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_system_api_context(
            SystemApiCheckOptions::default()
                .with_compile_sdk_version("12")
                .with_crossplatform(true, true),
        )
        .check("/project/Page.ets", source);
    assert_eq!(ignored.error_count(), 0);
    assert_eq!(
        ignored
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic,
                    EtsTransformError::CodedDiagnostic {
                        code,
                        severity: DiagnosticSeverity::Warning,
                        ..
                    } if code == "11706007"
                )
            })
            .count(),
        2
    );
}

#[test]
fn arkts11_system_api_resolves_recursive_sdk_types_without_unbounded_expansion() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut declaration = String::from(
        "export interface Transform {\n\
         /** @since 13 */ rotateY: Transform;\n\
         }\n\
         export declare function currentTransform(): Transform;\n",
    );
    // A real SDK declaration inventory contains thousands of symbols. The
    // source compiler resolves its type graph, so a recursive type must not be
    // expanded once per unrelated declaration in that inventory.
    for index in 0..2_048 {
        declaration.push_str(&format!("interface Unrelated{index} {{ value: number }}\n"));
    }
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(declaration),
    );
    let sources = TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]);
    let report = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(sources, Vec::new())
        .with_system_api_context(
            SystemApiCheckOptions::default()
                .with_compatible_sdk_version("12")
                .with_strict_api_compatibility(true),
        )
        .check(
            "/project/Page.ets",
            "import { currentTransform } from '@ohos.demo'; currentTransform().rotateY.rotateY.rotateY;",
        );

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011"
        )
    }));
}

#[test]
fn arkts11_system_api_resolves_return_and_local_value_type_flow() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "export interface Manager {\n\
             /** @since 13 */\n\
             run(): void;\n\
             }\n\
             export declare function getManager(): Manager;\n\
             export declare const directManager: Manager;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { getManager, directManager } from '@ohos.demo';\n\
         directManager.run();\n\
         getManager().run();\n\
         const manager = getManager();\n\
         manager.run();\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(since, 3);
}

#[test]
fn arkts11_system_api_selects_overloads_and_unwraps_awaited_generic_results() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "export interface Manager {\n\
             /** @since 13 */\n\
             run(): void;\n\
             }\n\
             export declare function getManager(): Promise<Manager>;\n\
             export declare function selectManager(callback: (manager: Manager) => void): void;\n\
             export declare function selectManager(): Promise<Manager>;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { getManager, selectManager } from '@ohos.demo';\n\
         async function useManagers(): Promise<void> {\n\
           const direct = await getManager();\n\
           direct.run();\n\
           (await getManager()).run();\n\
           const selected = await selectManager();\n\
           selected.run();\n\
           const callbackResult = selectManager(() => {});\n\
           callbackResult.run();\n\
         }\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since, 3,
        "zero-argument Promise overloads must expose the awaited Manager shape, while the callback overload returns void: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_selects_overload_contracts_by_arity() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "/** @since 13 */\n\
             export declare function choose(value: string): void;\n\
             /** @since 10 */\n\
             export declare function choose(value: string, mode: number): void;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { choose } from '@ohos.demo';\n\
         choose('future');\n\
         choose('supported', 1);\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        1,
        "the selected overload owns its JSDoc contract: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_selects_same_arity_overloads_by_argument_type() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "export interface Options { mode?: string; }\n\
             export interface Callback<T> { (value: T): void; }\n\
             export type Text = string;\n\
             interface FutureManager {\n\
               /** @since 13 */\n\
               run(): void;\n\
             }\n\
             interface LegacyManager {}\n\
             /** @since 13 */\n\
             export declare function select(value: string): void;\n\
             /** @since 10 */\n\
             export declare function select(value: number): void;\n\
             /** @since 13 */\n\
             export declare function load(value: Options): void;\n\
             /** @since 10 */\n\
             export declare function load(value: Callback<void>): void;\n\
             /** @since 10 */\n\
             export declare function make(value: string): FutureManager;\n\
             /** @since 10 */\n\
             export declare function make(value: number): LegacyManager;\n\
             /** @since 13 */\n\
             export declare function event(type: 'future'): void;\n\
             /** @since 10 */\n\
             export declare function event(type: 'stable'): void;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { select, load, make, event, type Options, type Callback, type Text } from '@ohos.demo';\n\
         const label: string = 'future';\n\
         const labelAlias = label;\n\
         const typedAlias: Text = 'future';\n\
         const count: number = 1;\n\
         const options: Options = {};\n\
         const callback: Callback<void> = () => {};\n\
         select('future');\n\
         select(1);\n\
         select(label);\n\
         select(labelAlias);\n\
         select(typedAlias);\n\
         select(count);\n\
         load({});\n\
         load(() => {});\n\
         load(options);\n\
         load(callback);\n\
         make('future').run();\n\
         make(1).run();\n\
         event('future');\n\
         event('stable');\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        8,
        "literal, annotated, object, and callable arguments must select matching overload contracts and return shapes: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_resolves_imported_callback_shapes_for_overloads() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.base.d.ts"),
        Arc::<str>::from("export interface Callback<T> { (value: T): void; }\n"),
    );
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "import type { Callback } from './@ohos.base';\n\
             /** @since 13 */\n\
             export declare function route<T>(value: T): void;\n\
             /** @since 10 */\n\
             export declare function route(value: Callback<void>): void;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { route } from '@ohos.demo';\n\
         import type { Callback as RenamedCallback } from '@ohos.base';\n\
         type LocalCallback = RenamedCallback<void>;\n\
         const importedCallback: RenamedCallback<void> = () => {};\n\
         const localCallback: LocalCallback = importedCallback;\n\
         route(() => {});\n\
         route(importedCallback);\n\
         route(localCallback);\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        0,
        "the imported callback declaration must outrank an unconstrained generic overload: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_converges_local_overload_result_types() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "interface FutureInput {}\n\
             interface LegacyInput {}\n\
             export declare function makeInput(value: string): FutureInput;\n\
             export declare function makeInput(value: number): LegacyInput;\n\
             export declare function forward(value: FutureInput): FutureInput;\n\
             export declare function forward(value: LegacyInput): LegacyInput;\n\
             /** @since 13 */\n\
             export declare function dispatch(value: FutureInput): void;\n\
             /** @since 10 */\n\
             export declare function dispatch(value: LegacyInput): void;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { makeInput, forward, dispatch } from '@ohos.demo';\n\
         const first = makeInput('future');\n\
         const second = forward(first);\n\
         const third = forward(second);\n\
         dispatch(third);\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        1,
        "local call-result types must converge before downstream overload selection: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_propagates_generic_arguments_through_named_wrappers() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "export interface Manager {\n\
             /** @since 13 */\n\
             run(): void;\n\
             }\n\
             export interface Legacy {}\n\
             interface Envelope<T> { value: T; }\n\
             type EnvelopeAlias<T> = Envelope<T>;\n\
             interface DerivedEnvelope<T> extends EnvelopeAlias<T> {}\n\
             interface DefaultEnvelope<T = Manager> { value: T; }\n\
             export declare function wrapped<T>(): Promise<DerivedEnvelope<T>>;\n\
             export declare function defaultWrapped(): DefaultEnvelope;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { wrapped, defaultWrapped, Manager, Legacy } from '@ohos.demo';\n\
         async function useManagers(): Promise<void> {\n\
           (await wrapped<Manager>()).value.run();\n\
           (await wrapped<Legacy>()).value.run();\n\
           defaultWrapped().value.run();\n\
         }\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        2,
        "generic aliases, inheritance, and declaration defaults must retain the instantiated SDK type: {:?}",
        report.diagnostics()
    );
}

#[test]
fn arkts11_system_api_applies_explicit_function_and_method_type_arguments() {
    let sdk_root = PathBuf::from("/sdk/api");
    let mut files = rustc_hash::FxHashMap::default();
    files.insert(
        sdk_root.join("@ohos.demo.d.ts"),
        Arc::<str>::from(
            "export interface Manager {\n\
             /** @since 13 */\n\
             run(): void;\n\
             }\n\
             export interface Legacy {}\n\
             interface Factory {\n\
               select<T>(): Promise<T>;\n\
             }\n\
             export declare const factory: Factory;\n\
             export declare function select<T>(): Promise<T>;\n\
             export declare function defaultSelect<T = Manager>(): Promise<T>;\n",
        ),
    );
    let checker = ArktsSourceChecker::new(Arc::new(ComponentMetadata::default()))
        .with_project_sources(
            TransformSourceSnapshot::new(files).with_sdk_declaration_roots(vec![sdk_root]),
            Vec::new(),
        )
        .with_api_available_context("12", "OpenHarmony");
    let report = checker.check(
        "/project/Page.ets",
        "import { factory, select, defaultSelect, Manager, Legacy } from '@ohos.demo';\n\
         async function useManagers(): Promise<void> {\n\
           (await select<Manager>()).run();\n\
           (await factory.select<Manager>()).run();\n\
           (await defaultSelect()).run();\n\
           (await select<Legacy>()).run();\n\
         }\n",
    );
    let since = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            matches!(diagnostic, EtsTransformError::CodedDiagnostic { code, .. } if code == "11706011")
        })
        .count();

    assert_eq!(
        since,
        3,
        "explicit generic arguments must select the instantiated SDK return shape: {:?}",
        report.diagnostics()
    );
}
