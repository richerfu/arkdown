use std::{collections::BTreeMap, sync::Arc};

use arkdown_ets_checker::{
    NativeCompilerOptions, NativeOhSdkCheckPlugin, NativeOhSdkClassCheckPlugin, NativeTypeSession,
};
use serde_json::json;

#[test]
fn native_typescript_uses_packaged_service_without_mandatory_environment_configuration() {
    let package = tempfile::tempdir().unwrap();
    let packaged = package.path().join("tsc");
    std::fs::write(&packaged, b"native compiler fixture").unwrap();
    let resolved = NativeTypeSession::resolve_executable(Some(&packaged)).unwrap();
    let expected = std::env::var_os("ARKDOWN_TYPESCRIPT_PATH")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or(packaged)
        .canonicalize()
        .unwrap();
    assert_eq!(resolved, expected);
}

#[test]
fn compiler_options_preserve_oh_acronym_wire_names_and_checker_mode() {
    let project = tempfile::tempdir().unwrap();
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_runtime_os = "OpenHarmony".into();
    options.skip_ark_ts_static_blocks_check = true;
    options.enable_strict_check_oh_module = true;
    options.oh_ark_ui_declaration_dirs = vec![project.path().join("component")];
    options.oh_sdk_check_plugins = vec![NativeOhSdkCheckPlugin {
        os_name: "HarmonyOS".into(),
        tag: "since".into(),
        plugin_type: "CompatibilityCheck".into(),
        path: project.path().join("since-checker.js"),
        function_name: "checkSinceValue".into(),
    }];
    options.oh_sdk_class_check_plugins = vec![NativeOhSdkClassCheckPlugin {
        tag_name: "syscap".into(),
        path: project.path().join("syscap-checker.js"),
        class_name: "SyscapChecker".into(),
    }];
    options.oh_sdk_plugin_project_config = Some(serde_json::Map::from_iter([
        ("runtimeOS".into(), json!("OpenHarmony")),
        (
            "strictMode".into(),
            json!({ "apiCompatibilityCheck": "warning" }),
        ),
        ("customSdkPluginValue".into(), json!({ "nested": true })),
    ]));
    let value = serde_json::to_value(options).unwrap();
    // ets2bundle/compiler/src/ets_checker.ts::setCompilerOptions fixes
    // `noEmit`; Arkdown must never turn TSGO into its code generator.
    assert_eq!(value["noEmit"], true);
    assert_eq!(value["module"], 1);
    assert_eq!(value["ohRuntimeOS"], "OpenHarmony");
    assert_eq!(value["skipArkTSStaticBlocksCheck"], true);
    assert_eq!(value["enableStrictCheckOHModule"], true);
    assert!(value.get("ohArkUIDeclarationDirs").is_some());
    assert!(value.get("ohRuntimeOs").is_none());
    assert_eq!(value["ohSdkCheckPlugins"][0]["osName"], "HarmonyOS");
    assert_eq!(value["ohSdkCheckPlugins"][0]["type"], "CompatibilityCheck");
    assert_eq!(value["ohSdkClassCheckPlugins"][0]["tagName"], "syscap");
    assert_eq!(
        value["ohSdkClassCheckPlugins"][0]["className"],
        "SyscapChecker"
    );
    assert_eq!(
        value["ohSdkPluginProjectConfig"]["strictMode"]["apiCompatibilityCheck"],
        "warning"
    );
    assert_eq!(
        value["ohSdkPluginProjectConfig"]["customSdkPluginValue"]["nested"],
        true
    );
}

#[test]
fn compiler_options_omit_absent_sdk_plugin_project_config() {
    let project = tempfile::tempdir().unwrap();
    let value = serde_json::to_value(NativeCompilerOptions::arkts11(project.path())).unwrap();
    assert!(value.get("ohSdkPluginProjectConfig").is_none());
}

/// `main.js::collectExternalApiCheckPlugin` loads the SDK-owned synchronous
/// JavaScript callback. A HarmonyOS point version must therefore be compared
/// by the SDK callback instead of the built-in integer comparator.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler and Node.js"]
fn native_executes_sdk_since_callback_with_node_module_semantics() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.sample.d.ts");
    let source_path = project.path().join("entry.ets");
    let plugin_path = project.path().join("since-checker.js");
    std::fs::write(
        &plugin_path,
        r#"
const fs = require('fs');
const path = require('path');
function checkSinceValue(required, target, scene) {
  fs.statSync(path.join(__dirname, 'since-checker.js'));
  const match = target.match(/\((\d+)\)$/);
  return { result: scene === 0 && match !== null && Number(required) <= Number(match[1]) };
}
module.exports = { checkSinceValue };
"#,
    )
    .unwrap();
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.compile_sdk_version = 17;
    options.compatible_sdk_version = Some(17.0);
    options.oh_origin_compatible_sdk_version = "5.0.5(17)".into();
    options.oh_runtime_os = "HarmonyOS".into();
    options.oh_compile_mode = "moduleJson".into();
    options.oh_project_root_path = project.path().to_path_buf();
    options.oh_module_path = project.path().to_path_buf();
    options.oh_all_module_paths = vec![sdk_path.clone()];
    options.oh_global_module_paths = vec![project.path().to_path_buf()];
    options.oh_sdk_check_plugins = vec![NativeOhSdkCheckPlugin {
        os_name: "HarmonyOS".into(),
        tag: "since".into(),
        plugin_type: "CompatibilityCheck".into(),
        path: plugin_path,
        function_name: "checkSinceValue".into(),
    }];
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([
            (
                sdk_path.clone(),
                Arc::from(
                    "/** @since 11 */ declare function current(): void;\n/** @since 20 */ declare function future(): void;",
                ),
            ),
            (source_path.clone(), Arc::from("current(); future();")),
        ]),
    )
    .unwrap();
    let program = session
        .create_program(&[sdk_path, source_path], &options)
        .unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    let sdk_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == 28007)
        .collect::<Vec<_>>();
    assert_eq!(sdk_diagnostics.len(), 1, "{diagnostics:#?}");
    assert!(
        sdk_diagnostics[0].text.contains("future"),
        "{diagnostics:#?}"
    );
    let linter = session.arkts_linter_diagnostics(&program).unwrap();
    let sdk_linter = linter
        .iter()
        .filter(|diagnostic| diagnostic.code == 28007)
        .collect::<Vec<_>>();
    assert!(sdk_linter.is_empty(), "{linter:#?}");
    session.release(program).unwrap();
}

/// `apiAvailable_validate_utils.ts::validateApiAvailableArgument` is a
/// declaration-identity/type-checker contract. OXC lowers the runtime call,
/// while TSGO must publish the source-owned 11706013 diagnostic through the
/// TS28007 SDK diagnostic carrier.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_validates_api_available_against_the_resolved_sdk_declaration() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.deviceInfo.d.ts");
    let source_path = project.path().join("entry.ets");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_runtime_os = "OpenHarmony".into();
    options.oh_sdk_configs = vec![arkdown_ets_checker::NativeOhSdkConfig {
        api_paths: vec![project.path().to_path_buf()],
    }];
    options.oh_system_modules = vec!["@ohos.deviceInfo.d.ts".into()];
    options.oh_all_module_paths = vec![sdk_path.clone()];
    options.oh_global_module_paths = vec![project.path().to_path_buf()];
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([
            (
                sdk_path.clone(),
                Arc::from(
                    "declare namespace deviceInfo { function apiAvailable(version: string | number): boolean; } export default deviceInfo;",
                ),
            ),
            (
                source_path.clone(),
                Arc::from(
                    "import deviceInfo from '@ohos.deviceInfo'; const available = deviceInfo.apiAvailable('25.0.0');",
                ),
            ),
        ]),
    )
    .unwrap();
    let program = session
        .create_program(&[sdk_path, source_path], &options)
        .unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    let sdk_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == 28007)
        .collect();
    assert_eq!(sdk_diagnostics.len(), 1, "{diagnostics:#?}");
    assert!(
        sdk_diagnostics[0].text.starts_with("11706013#"),
        "{diagnostics:#?}"
    );
    session.release(program).unwrap();
}

/// `api_check_utils.ts::isSourceRetentionAnnotationContentValid` and
/// `checkFormatResult` resolve the SDK `Available` annotation before checking
/// OpenHarmony version format and runtime OS. This remains a TSGO diagnostic
/// contract rather than an OXC transform-time rule.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_validates_available_annotation_format_and_runtime_os() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.annotation.d.ets");
    let source_path = project.path().join("entry.ets");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_runtime_os = "OpenHarmony".into();
    options.oh_all_module_paths = vec![sdk_path.clone()];
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([
            (
                sdk_path.clone(),
                Arc::from("@interface Available { minApiVersion: string; }"),
            ),
            (
                source_path.clone(),
                Arc::from(
                    "@Available({ minApiVersion: '25.0.0' }) class InvalidFormat {}\n@Available({ minApiVersion: 'HarmonyOS 6.0.0' }) class InvalidRuntime {}",
                ),
            ),
        ]),
    )
    .unwrap();
    let program = session
        .create_program(&[sdk_path, source_path], &options)
        .unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    let sdk_texts: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == 28007)
        .map(|diagnostic| diagnostic.text.as_str())
        .collect();
    assert_eq!(sdk_texts.len(), 2, "{diagnostics:#?}");
    assert!(
        sdk_texts.iter().any(|text| text.starts_with("11706016#")),
        "{diagnostics:#?}"
    );
    assert!(
        sdk_texts.iter().any(|text| text.starts_with("11706017#")),
        "{diagnostics:#?}"
    );
    session.release(program).unwrap();
}

/// OH parser/binder/checker annotation declarations and resolveAnnotation:
/// keep real type checking after accepting @interface, rather than erasing it.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_checks_annotations_without_erasing_their_type_rules() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let source: Arc<str> = Arc::from(
        r#"
@interface Options { value: number; label: string = "default"; }
@Options({ value: 1 }) class Valid {}
@Options({ value: "wrong" }) class Invalid {}
let wrongType: Options;
const wrongValue = Options;
"#,
    );
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([(source_path.clone(), source)]),
    )
    .unwrap();
    let program = session
        .create_program(
            &[source_path],
            &NativeCompilerOptions::arkts11(project.path()),
        )
        .unwrap();
    assert!(session.syntactic_diagnostics(&program).unwrap().is_empty());
    let diagnostics = session.diagnostics(&program).unwrap();
    let mut codes: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    codes.sort_unstable();
    assert_eq!(codes, [2322, 28027, 28028], "{diagnostics:#?}");
    session.release(program).unwrap();
}

#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_batches_annotation_and_transform_type_facts() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("facts.ets");
    let source: Arc<str> = Arc::from(
        r#"
@interface Marker { value: number = 1; }
class MutableBuilder { builder(): void {} }
type PrimitiveAlias = string | number;
enum Color { Red }
@Marker
class Facts {
  @Marker property: number = 1;
  simple: PrimitiveAlias = "value";
  content: MutableBuilder = new MutableBuilder();
  @Marker method(): void {}
  build(): void { this.content.builder(); const color = Color.Red; }
}
"#,
    );
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([(source_path.clone(), source)]),
    )
    .unwrap();
    let program = session
        .create_program(
            std::slice::from_ref(&source_path),
            &NativeCompilerOptions::arkts11(project.path()),
        )
        .unwrap();

    let annotation = session
        .annotation_transform_infos(&program, std::slice::from_ref(&source_path))
        .unwrap();
    assert_eq!(annotation.len(), 1);
    assert_eq!(annotation[0].declarations.len(), 1);
    assert_eq!(annotation[0].uses.len(), 3);
    assert_eq!(
        annotation[0]
            .uses
            .iter()
            .filter(|use_info| use_info.runtime_retained)
            .count(),
        2
    );

    let facts = session
        .arkts_transform_type_facts(&program, std::slice::from_ref(&source_path))
        .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].properties.len(), 3);
    let simple = facts[0].properties[1].r#type.as_ref().unwrap();
    assert_eq!(simple.types.len(), 2);
    assert!(simple.types.iter().all(|part| part.is_basic));
    assert_eq!(facts[0].builder_accesses.len(), 1);
    assert_eq!(
        facts[0].builder_accesses[0].receiver_type.symbol_name,
        "MutableBuilder"
    );
    assert_eq!(facts[0].member_accesses.len(), 1);
    assert!(facts[0].member_accesses[0].r#type.is_enum);
    session.release(program).unwrap();
}

/// ets2bundle
/// `api_check_utils.ts::checkCrossplatformValue/getApiPathFromNode` collects
/// the declaration identities after TypeScript has resolved namespace,
/// factory-return, and member declarations.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_collects_resolved_sdk_dependency_uses() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let sdk_path = project.path().join("@ohos.sample.d.ts");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_crossplatform = true;
    options.oh_all_module_paths = vec![sdk_path.clone()];
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([
            (
                sdk_path.clone(),
                Arc::from(
                    r#"declare namespace sample {
/** @crossplatform */ interface Client { /** @crossplatform */ run(): void; }
/** @crossplatform */ function create(): Client;
}"#,
                ),
            ),
            (source_path.clone(), Arc::from("sample.create().run();")),
        ]),
    )
    .unwrap();
    let program = session
        .create_program(&[sdk_path, source_path.clone()], &options)
        .unwrap();
    let facts = session
        .arkts_transform_type_facts(&program, &[source_path])
        .unwrap();
    let uses = &facts[0].sdk_api_uses;
    assert_eq!(uses.len(), 3, "{uses:#?}");
    assert_eq!(uses[0].api_module, "@ohos.sample");
    assert_eq!(uses[0].function, "Client#run");
    assert_eq!(uses[1].function, "create");
    assert_eq!(uses[2].function, "sample");
    session.release(program).unwrap();
}

/// TypeScript API: api/session_createprogram_test.go and
/// api/callbackfs.go. Real compiler test, with no fabricated diagnostics.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_checks_complex_types_and_snapshot_sources() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let source: Arc<str> = Arc::from(
        r#"
type Keys<T> = { [K in keyof T]: T[K] };
type PickType<T> = T extends string ? number : boolean;
const bad: Keys<{ value: number }> = { value: "wrong" };
const conditional: PickType<string> = false;
const unicode = "中文😀"; const wrong: number = "bad";
"#,
    );
    let executable = NativeTypeSession::executable().unwrap();
    let mut session = NativeTypeSession::start(
        &executable,
        project.path(),
        BTreeMap::from([(source_path.clone(), source.clone())]),
    )
    .unwrap();
    let options = NativeCompilerOptions::arkts11(project.path());
    let program = session
        .create_program(std::slice::from_ref(&source_path), &options)
        .unwrap();
    assert!(session.syntactic_diagnostics(&program).unwrap().is_empty());
    let diagnostics = session.diagnostics(&program).unwrap();
    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert!(diagnostics.iter().all(|d| d.code == 2322 && d.is_error()));
    let byte_pos = source.find("wrong:").unwrap();
    let position = source[..byte_pos].encode_utf16().count() as u32;
    let types = session
        .types_at_positions(&program, &source_path, &[position])
        .unwrap();
    assert!(types[0].is_some());
    let diagnostic = diagnostics.iter().find(|d| d.pos == position).unwrap();
    assert_eq!(
        diagnostic.start_position.as_ref().unwrap().character,
        source[..byte_pos]
            .rsplit('\n')
            .next()
            .unwrap()
            .encode_utf16()
            .count() as u32
    );
    session.release(program).unwrap();
}

#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_checks_struct_members_and_exact_sdk_aliases() {
    let project = tempfile::tempdir().unwrap();
    let file = project.path().join("Index.ets");
    let sdk = project.path().join("@kit.TestKit.d.ts");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options
        .paths
        .insert("@kit.TestKit".into(), vec![sdk.clone()]);
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(), project.path(),
        BTreeMap::from([
            (sdk, Arc::from("export declare const sdkValue: number;")),
            (file.clone(), Arc::from("import { sdkValue } from '@kit.TestKit';\ndeclare const Component: ClassDecorator; @Component struct Demo { count: number = 'bad'; value(): string { return 1; } }\nconst bad: string = sdkValue;")),
        ])
    ).unwrap();
    let program = session.create_program(&[file], &options).unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert!(diagnostics.iter().all(|d| d.code == 2322));
    session.release(program).unwrap();
}

#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH native compiler"]
fn native_updates_program_and_resolves_aliased_factory_types() {
    let project = tempfile::tempdir().unwrap();
    let file = project.path().join("builder.ts");
    let source: Arc<str> = Arc::from(
        "class MutableBuilder { builder(): void {} }\ntype Alias = MutableBuilder;\nfunction make(): Alias { return new MutableBuilder(); }\nconst value = make();\nconst bad: number = 'wrong';",
    );
    let position = source[..source.find("value =").unwrap()]
        .encode_utf16()
        .count() as u32;
    let options = NativeCompilerOptions::arkts11(project.path());
    let mut session = NativeTypeSession::start(
        &NativeTypeSession::executable().unwrap(),
        project.path(),
        BTreeMap::from([(file.clone(), source.clone())]),
    )
    .unwrap();
    let program = session
        .create_program(std::slice::from_ref(&file), &options)
        .unwrap();
    let ty = session
        .types_at_positions(&program, &file, &[position])
        .unwrap()
        .remove(0)
        .unwrap();
    let ty = session.non_nullable_type(&program, &ty).unwrap();
    assert_eq!(
        session.type_symbol(&program, &ty).unwrap().unwrap().name,
        "MutableBuilder"
    );
    assert_eq!(session.diagnostics(&program).unwrap().len(), 1);
    let program = session
        .update_program(
            program,
            std::slice::from_ref(&file),
            &options,
            BTreeMap::from([(file.clone(), Arc::from(source.replace("'wrong'", "42")))]),
            &[],
        )
        .unwrap();
    assert!(session.diagnostics(&program).unwrap().is_empty());
    session.release(program).unwrap();
}
