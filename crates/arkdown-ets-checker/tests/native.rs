use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use arkdown_ets_checker::{
    NativeArktsTransformTypeQuery, NativeCheckError, NativeCompilerOptions, NativeDiagnostic,
    NativeOhSdkCheckPlugin, NativeOhSdkClassCheckPlugin, NativeOhSdkPluginCompletion,
    NativeOhSdkPluginHost, NativeOhSdkPluginRequest, NativeOhSdkPluginResponse,
    NativeProgramFileChanges, NativeTypeQueryRange, NativeTypeSession,
};
use serde_json::json;

#[derive(Debug)]
struct DeferredSource {
    text: &'static str,
    reads: std::sync::atomic::AtomicUsize,
    fail_path: Option<PathBuf>,
}

impl arkdown_core::SourceTextHandle for DeferredSource {
    fn size_bytes(&self) -> u64 {
        self.text.len() as u64
    }

    fn read_text(&self) -> Result<Arc<str>, arkdown_core::SourceReadError> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(path) = &self.fail_path {
            return Err(arkdown_core::SourceReadError::ReloadIo {
                path: path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "snapshot reload denied",
                ),
            });
        }
        Ok(Arc::from(self.text))
    }
}

#[test]
fn source_handles_materialize_only_program_reads_and_do_not_fall_back_on_error() {
    use arkdown_core::SourceTextHandle;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let project = tempfile::tempdir().unwrap();
    let app = project.path().join("entry.ets");
    let unused = project.path().join("unused.ets");
    // An unchecked disk fallback would suppress the captured-text type error.
    std::fs::write(&app, "export {}; const n: number = 1;").unwrap();
    let make = |text, fail_path| {
        Arc::new(DeferredSource {
            text,
            reads: AtomicUsize::new(0),
            fail_path,
        })
    };
    let used = make("export {}; const n: number = 'wrong';", None);
    let unneeded = make("invalid unused source", None);
    let mut session = NativeTypeSession::start(project.path(), BTreeMap::new(), None)
        .unwrap()
        .with_source_handles(BTreeMap::from([
            (app.clone(), used.clone() as Arc<dyn SourceTextHandle>),
            (unused, unneeded.clone() as Arc<dyn SourceTextHandle>),
        ]));
    let options = NativeCompilerOptions::arkts11(project.path());
    let program = session
        .create_program(std::slice::from_ref(&app), &options)
        .unwrap();
    for _ in 0..2 {
        assert!(
            session
                .diagnostics(&program)
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic.code == 2322)
        );
    }
    assert_eq!(used.reads.load(Ordering::Relaxed), 1);
    assert_eq!(unneeded.reads.load(Ordering::Relaxed), 0);

    let failed = make("", Some(app.clone()));
    let mut session = NativeTypeSession::start(project.path(), BTreeMap::new(), None)
        .unwrap()
        .with_source_handles(BTreeMap::from([(
            app.clone(),
            failed as Arc<dyn SourceTextHandle>,
        )]));
    let error = match session.create_program(std::slice::from_ref(&app), &options) {
        Err(error) => error,
        Ok(_) => panic!(
            "a failed captured read must not turn into missing-source diagnostics or disk content"
        ),
    };
    assert!(
        matches!(error, NativeCheckError::Source(error) if matches!(&*error, arkdown_core::SourceReadError::ReloadIo { path, .. } if path == &app))
    );
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
    // `noEmit`; Arkdown must never turn tsrs into its code generator.
    assert_eq!(value["noEmit"], true);
    assert_eq!(
        value["checkers"],
        std::thread::available_parallelism()
            .map_or(4, |count| count.get())
            .min(4)
    );
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

#[test]
fn compiler_options_enable_sdk_callback_transport_only_for_declared_plugins() {
    let project = tempfile::tempdir().unwrap();
    let mut options = NativeCompilerOptions::arkts11(project.path());
    assert!(!options.uses_sdk_plugin_callbacks());
    options.oh_sdk_class_check_plugins = vec![NativeOhSdkClassCheckPlugin {
        tag_name: "syscap".into(),
        path: project.path().join("syscap-checker.js"),
        class_name: "SyscapChecker".into(),
    }];
    assert!(options.uses_sdk_plugin_callbacks());
}

#[test]
fn native_diagnostic_preserves_source_less_signed_ranges() {
    let diagnostic: NativeDiagnostic = serde_json::from_value(serde_json::json!({
        "pos": -1,
        "end": -1,
        "code": 9001,
        "category": 1,
        "text": "global diagnostic",
        "startPosition": null,
        "endPosition": null
    }))
    .unwrap();

    assert_eq!(diagnostic.pos, -1);
    assert_eq!(diagnostic.end, -1);
    assert_eq!(diagnostic.file_name, None);
}

#[test]
fn native_api_applies_openharmony_parse_and_inference_semantics() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let dependency_path = project.path().join("dependency.ts");
    let loader_path = project.path().join("ets-loader");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.ets_loader_path = loader_path;
    let source = r#"import "./dependency";"#;
    let dependency = r#"
interface BaseData { method: string; }
abstract class CanvasMethod<T = object> { abstract draw(value: T): void; }
class Clear extends CanvasMethod<number[]> { draw(value: number[]) {} }
class Clip extends CanvasMethod<BaseData[]> { draw(value: BaseData[]) {} }
const legacyEscape = "\033";
const methods: Map<string, CanvasMethod<any>> = new Map([
  ["base", new Clear() as CanvasMethod<any>],
  ["clear", new Clear()],
  ["clip", new Clip()],
]);
"#;
    let mut session = NativeTypeSession::start(
        project.path(),
        BTreeMap::from([
            (source_path.clone(), Arc::from(source)),
            (dependency_path, Arc::from(dependency)),
        ]),
        None,
    )
    .unwrap();
    let program = session.create_program(&[source_path], &options).unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    let source_graph = session.source_graph(&program).unwrap();
    assert!(
        source_graph
            .files
            .iter()
            .any(|file| file.file_name.ends_with("dependency.ts")),
        "{source_graph:#?}"
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    drop(program);
}

#[test]
fn native_build_diagnostics_preserve_linter_and_typescript_channels() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let source = Arc::<str>::from(
        "var value: string | undefined;\nconst copy: string = value;\nconst wrong: number = 'text';\n",
    );
    let mut session = NativeTypeSession::start(
        project.path(),
        BTreeMap::from([(source_path.clone(), source)]),
        None,
    )
    .unwrap();
    let program = session
        .create_program(
            std::slice::from_ref(&source_path),
            &NativeCompilerOptions::arkts11(project.path()),
        )
        .unwrap();
    let diagnostics = session.arkts_build_diagnostics(&program).unwrap();
    assert!(diagnostics.syntactic_diagnostics.is_empty());
    assert!(
        diagnostics
            .linter_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == 5)
    );
    assert!(
        diagnostics
            .semantic_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == 2322)
    );
    drop(program);
}

#[derive(Debug)]
struct SincePluginHost;

impl NativeOhSdkPluginHost for SincePluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
        requests
            .into_iter()
            .map(|request| {
                if request.operation == "releaseSession" {
                    assert!(request.args.is_empty());
                    return Ok(NativeOhSdkPluginResponse::default());
                }
                assert_eq!(request.operation, "value");
                if let Some(function) = request.function_name {
                    assert_eq!(function, "checkSinceValue");
                }
                assert!(request.plugin_id > 0);
                let required = request.args[0].as_str().unwrap().parse::<u32>().unwrap();
                assert_eq!(request.args[1], "5.0.5(17)");
                assert_eq!(request.args[2], 0);
                Ok(NativeOhSdkPluginResponse {
                    found: true,
                    result: required <= 17,
                    ..NativeOhSdkPluginResponse::default()
                })
            })
            .collect()
    }
}

/// `main.js::collectExternalApiCheckPlugin` loads the SDK-owned synchronous
/// JavaScript callback. tsrs owns selection and diagnostics but must delegate
/// that JavaScript ABI through its API client.
#[test]
fn native_delegates_sdk_since_callback_to_the_api_host() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.sample.d.ts");
    let source_path = project.path().join("entry.ets");
    let plugin_path = project.path().join("since-checker.js");
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
        Some(Arc::new(SincePluginHost)),
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
    drop(program);
}

#[derive(Default)]
struct CompactingPluginHost {
    operations: Mutex<Vec<String>>,
    compaction: Mutex<Option<NativeOhSdkPluginCompletion>>,
}

impl std::fmt::Debug for CompactingPluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactingPluginHost")
            .finish_non_exhaustive()
    }
}

impl NativeOhSdkPluginHost for CompactingPluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
        Ok(requests
            .into_iter()
            .map(|request| {
                assert_ne!(
                    request.operation, "compactSession",
                    "compaction must not wait on JS"
                );
                self.operations
                    .lock()
                    .unwrap()
                    .push(request.operation.clone());
                NativeOhSdkPluginResponse {
                    found: true,
                    // Missing plugins are nonfatal and uncached: strict must retry.
                    error: if request.operation == "syscap" {
                        "plugin unavailable".into()
                    } else {
                        String::new()
                    },
                    phase: "load".into(),
                    ..Default::default()
                }
            })
            .collect())
    }

    fn submit_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
        complete: NativeOhSdkPluginCompletion,
    ) {
        if requests.len() == 1 && requests[0].operation == "compactSession" {
            let request = &requests[0];
            assert!(request.session_id > 0);
            assert!(request.args.is_empty());
            self.operations
                .lock()
                .unwrap()
                .push(request.operation.clone());
            assert!(self.compaction.lock().unwrap().replace(complete).is_none());
        } else {
            complete(self.invoke_batch(requests));
        }
    }
}

#[test]
fn native_compacts_sdk_session_between_normal_and_strict_without_waiting() {
    let project = tempfile::tempdir().unwrap();
    let sdk = project.path().join("api.d.ets");
    let app = project.path().join("entry.ets");
    let normal_app = project.path().join("normal.ts");
    let sources = BTreeMap::from([
        (
            sdk.clone(),
            Arc::from("/** @syscap base */ declare function sdkCall(): void;"),
        ),
        (
            app.clone(),
            Arc::from("export {}; sdkCall(); const bad: number = 'bad';"),
        ),
        (
            normal_app.clone(),
            Arc::from("export {}; sdkCall(); const bad: number = 'bad';"),
        ),
    ]);
    let mut options = NativeCompilerOptions::arkts11(project.path());
    // tsrs enables strict SDK callbacks in strict-checker-only mode. TS files
    // still run in normal, while ETS files run in strict, exercising both sides.
    options.strict_checker_only = true;
    options.ets_loader_path = project.path().join("ets-loader");
    options.oh_compile_mode = "moduleJson".into();
    options.oh_device_types = vec!["phone".into()];
    options.oh_syscap_intersection = vec!["base".into()];
    options.oh_all_module_paths = vec![sdk.clone()];
    options.oh_sdk_class_check_plugins = vec![NativeOhSdkClassCheckPlugin {
        tag_name: "syscap".into(),
        path: project.path().join("check.cjs"),
        class_name: "Checker".into(),
    }];
    let roots = [sdk, app, normal_app];
    let baseline_host = Arc::new(CompactingPluginHost::default());
    let mut baseline_session =
        NativeTypeSession::start(project.path(), sources.clone(), Some(baseline_host)).unwrap();
    let baseline_program = baseline_session.create_program(&roots, &options).unwrap();
    let baseline = baseline_session
        .arkts_build_diagnostics(&baseline_program)
        .unwrap();
    assert!(!baseline.semantic_diagnostics.is_empty());

    let host = Arc::new(CompactingPluginHost::default());
    let mut session =
        NativeTypeSession::start(project.path(), sources, Some(host.clone())).unwrap();
    let program = session.create_program(&roots, &options).unwrap();
    let normal = session.arkts_normal_diagnostics(&program).unwrap();
    let normal_end = host.operations.lock().unwrap().len();
    assert!(
        host.operations
            .lock()
            .unwrap()
            .iter()
            .any(|op| op == "syscap")
    );
    let diagnostics = session.arkts_strict_diagnostics(&program, normal).unwrap();
    let operations = host.operations.lock().unwrap().clone();
    assert_eq!(
        operations
            .iter()
            .filter(|op| *op == "compactSession")
            .count(),
        1
    );
    assert_eq!(operations[normal_end], "compactSession");
    assert!(
        operations[normal_end + 1..].iter().any(|op| op == "syscap"),
        "{operations:?}"
    );
    assert_eq!(format!("{diagnostics:#?}"), format!("{baseline:#?}"));
    // The entire strict pass completes while JS compaction is still pending.
    let complete = host.compaction.lock().unwrap().take().unwrap();
    complete(Err(NativeCheckError::Compiler(
        "compaction unavailable".into(),
    )));
    drop(program);
    drop(session);
    assert_eq!(
        host.operations.lock().unwrap().last().unwrap(),
        "releaseSession"
    );
}

#[test]
fn native_skips_sdk_compaction_if_no_callback_initialized_the_session() {
    let project = tempfile::tempdir().unwrap();
    let app = project.path().join("entry.ets");
    let host = Arc::new(CompactingPluginHost::default());
    let mut session = NativeTypeSession::start(
        project.path(),
        BTreeMap::from([(app.clone(), Arc::from("export {}; const n: number = 1;"))]),
        Some(host.clone()),
    )
    .unwrap();
    let program = session
        .create_program(&[app], &NativeCompilerOptions::arkts11(project.path()))
        .unwrap();
    let normal = session.arkts_normal_diagnostics(&program).unwrap();
    session.arkts_strict_diagnostics(&program, normal).unwrap();
    drop(program);
    drop(session);
    assert!(host.operations.lock().unwrap().is_empty());
    assert!(host.compaction.lock().unwrap().is_none());
}

#[derive(Debug, Default)]
struct SessionRecordingPluginHost {
    session_ids: Mutex<Vec<u64>>,
    released: Mutex<Vec<u64>>,
}

impl NativeOhSdkPluginHost for SessionRecordingPluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
        requests
            .into_iter()
            .map(|request| {
                if request.operation == "releaseSession" {
                    self.released.lock().unwrap().push(request.session_id);
                    return Ok(NativeOhSdkPluginResponse::default());
                }
                self.session_ids.lock().unwrap().push(request.session_id);
                Ok(NativeOhSdkPluginResponse {
                    found: true,
                    result: true,
                    ..NativeOhSdkPluginResponse::default()
                })
            })
            .collect()
    }
}

/// Separate tsrs children restart their process-local session counters. The
/// Rust service boundary must still keep their JS source/config intern tables
/// disjoint when a build-scoped plugin host is shared by multiple modules.
#[test]
fn native_scopes_sdk_plugin_sessions_across_compiler_instances() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.sample.d.ts");
    let source_path = project.path().join("entry.ets");
    let plugin_path = project.path().join("since-checker.js");
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
    let sources = BTreeMap::from([
        (
            sdk_path.clone(),
            Arc::from(
                "/** @since 11 */ declare function current(): void;\n/** @since 20 */ declare function future(): void;",
            ),
        ),
        (source_path.clone(), Arc::from("current(); future();")),
    ]);

    let host = Arc::new(SessionRecordingPluginHost::default());

    for _ in 0..2 {
        let mut session =
            NativeTypeSession::start(project.path(), sources.clone(), Some(host.clone())).unwrap();
        let program = session
            .create_program(&[sdk_path.clone(), source_path.clone()], &options)
            .unwrap();
        assert!(
            session
                .arkts_build_diagnostics(&program)
                .unwrap()
                .semantic_diagnostics
                .is_empty()
        );
        let lease = program.clone();
        drop(program);
        drop(session);
        let before = host.released.lock().unwrap().len();
        assert_eq!(
            before,
            host.session_ids
                .lock()
                .unwrap()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                - 1,
            "SDK state must remain live while a snapshot lease exists"
        );
        assert!(!lease.source_graph().unwrap().files.is_empty());
        drop(lease);
        assert_eq!(host.released.lock().unwrap().len(), before + 1);
    }

    let session_ids = host
        .session_ids
        .lock()
        .unwrap()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(session_ids.len(), 2, "{session_ids:?}");
}

/// `apiAvailable_validate_utils.ts::validateApiAvailableArgument` is a
/// declaration-identity/type-checker contract. OXC lowers the runtime call,
/// while tsrs must publish the source-owned 11706013 diagnostic through the
/// TS28007 SDK diagnostic carrier.
#[test]
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
        None,
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
    drop(program);
}

/// `api_check_utils.ts::isSourceRetentionAnnotationContentValid` and
/// `checkFormatResult` resolve the SDK `Available` annotation before checking
/// OpenHarmony version format and runtime OS. This remains a tsrs diagnostic
/// contract rather than an OXC transform-time rule.
#[test]
fn native_validates_available_annotation_format_and_runtime_os() {
    let project = tempfile::tempdir().unwrap();
    let sdk_path = project.path().join("@ohos.annotation.d.ets");
    let source_path = project.path().join("entry.ets");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_runtime_os = "OpenHarmony".into();
    options.oh_all_module_paths = vec![sdk_path.clone()];
    let mut session = NativeTypeSession::start(

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
        None,
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
    drop(program);
}

/// OH parser/binder/checker annotation declarations and resolveAnnotation:
/// keep real type checking after accepting @interface, rather than erasing it.
#[test]
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
        project.path(),
        BTreeMap::from([(source_path.clone(), Arc::clone(&source))]),
        None,
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
    drop(program);
}

#[test]
fn native_batches_annotation_and_transform_type_facts() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("facts.ets");
    let source: Arc<str> = Arc::from(
        r#"
@interface Marker { value: number = 1; }
class MutableBuilder { builder(): void {} }
@ObservedV2 class ObservedValue {}
class PlainValue {}
type PrimitiveAlias = string | number;
enum Color { Red }
@Marker
class Facts {
  @Marker property: number = 1;
  simple: PrimitiveAlias = "value";
  content: MutableBuilder = new MutableBuilder();
  observed: ObservedValue = new ObservedValue();
  callback: Function = () => {};
  nullablePlain: PlainValue | null = null;
  @Marker method(): void {}
  build(): void { this.content.builder(); const color = Color.Red; }
}
"#,
    );
    let mut session = NativeTypeSession::start(
        project.path(),
        BTreeMap::from([(source_path.clone(), Arc::clone(&source))]),
        None,
    )
    .unwrap();
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.strict_null_checks = true;
    let program = session
        .create_program(std::slice::from_ref(&source_path), &options)
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

    let range = |text: &str| {
        let pos = source.find(text).unwrap();
        NativeTypeQueryRange {
            pos,
            end: pos + text.len(),
        }
    };
    let query = NativeArktsTransformTypeQuery {
        file: source_path.clone(),
        properties: [
            "property",
            "simple",
            "content",
            "observed",
            "callback",
            "nullablePlain",
        ]
        .into_iter()
        .map(range)
        .collect(),
        builder_accesses: vec![range("this.content.builder")],
        member_accesses: vec![range("Color.Red")],
    };
    let facts = session
        .arkts_transform_type_facts(&program, &[query], &[])
        .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].properties.len(), 6);
    let simple = facts[0].properties[1].r#type.as_ref().unwrap();
    assert!(simple.is_union);
    assert_eq!(simple.types.len(), 2);
    assert!(simple.types.iter().all(|part| part.is_basic));
    let observed = facts[0]
        .properties
        .iter()
        .filter_map(|property| property.r#type.as_ref())
        .find(|identity| identity.symbol_name == "ObservedValue")
        .unwrap();
    assert!(observed.is_observed_v2);
    let function = facts[0]
        .properties
        .iter()
        .filter_map(|property| property.r#type.as_ref())
        .find(|identity| identity.symbol_name == "Function")
        .unwrap();
    assert!(function.is_function);
    let nullable_plain = facts[0]
        .properties
        .iter()
        .filter_map(|property| property.r#type.as_ref())
        .find(|identity| {
            identity
                .types
                .iter()
                .any(|part| part.symbol_name == "PlainValue")
        })
        .unwrap();
    assert!(nullable_plain.types.iter().any(|part| part.is_nullable));
    assert_eq!(facts[0].builder_accesses.len(), 1);
    assert_eq!(
        facts[0].builder_accesses[0].receiver_type.symbol_name,
        "MutableBuilder"
    );
    assert_eq!(facts[0].member_accesses.len(), 1);
    assert!(facts[0].member_accesses[0].r#type.is_enum);
    drop(program);
}

/// ets2bundle
/// `api_check_utils.ts::checkCrossplatformValue/getApiPathFromNode` collects
/// the declaration identities after TypeScript has resolved namespace,
/// factory-return, and member declarations.
#[test]
fn native_collects_resolved_sdk_dependency_uses() {
    let project = tempfile::tempdir().unwrap();
    let source_path = project.path().join("entry.ets");
    let sdk_path = project.path().join("@ohos.sample.d.ts");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options.oh_crossplatform = true;
    options.oh_all_module_paths = vec![sdk_path.clone()];
    let mut session = NativeTypeSession::start(
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
        None,
    )
    .unwrap();
    let program = session
        .create_program(&[sdk_path, source_path.clone()], &options)
        .unwrap();
    session.diagnostics(&program).unwrap();
    let facts = session
        .arkts_transform_type_facts(&program, &[], &[source_path])
        .unwrap();
    let uses = &facts[0].sdk_api_uses;
    assert_eq!(uses.len(), 3, "{uses:#?}");
    assert_eq!(uses[0].api_module, "@ohos.sample");
    assert_eq!(uses[0].function, "Client#run");
    assert_eq!(uses[1].function, "create");
    assert_eq!(uses[2].function, "sample");
    drop(program);
}

/// TypeScript API: api/session_createprogram_test.go and
/// api/callbackfs.go. Real compiler test, with no fabricated diagnostics.
#[test]
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

    let mut session = NativeTypeSession::start(
        project.path(),
        BTreeMap::from([(source_path.clone(), source.clone())]),
        None,
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
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.pos == i64::from(position))
        .unwrap();
    assert_eq!(
        diagnostic.start_position.as_ref().unwrap().character,
        source[..byte_pos]
            .rsplit('\n')
            .next()
            .unwrap()
            .encode_utf16()
            .count() as u32
    );
    drop(program);
}

#[test]
fn native_checks_struct_members_and_exact_sdk_aliases() {
    let project = tempfile::tempdir().unwrap();
    let file = project.path().join("Index.ets");
    let sdk = project.path().join("@kit.TestKit.d.ts");
    let mut options = NativeCompilerOptions::arkts11(project.path());
    options
        .paths
        .insert("@kit.TestKit".into(), vec![sdk.clone()]);
    let mut session = NativeTypeSession::start(
 project.path(),
        BTreeMap::from([
            (sdk, Arc::from("export declare const sdkValue: number;")),
            (file.clone(), Arc::from("import { sdkValue } from '@kit.TestKit';\ndeclare const Component: ClassDecorator; @Component struct Demo { count: number = 'bad'; value(): string { return 1; } }\nconst bad: string = sdkValue;")),
        ]),
        None,
    ).unwrap();
    let program = session.create_program(&[file], &options).unwrap();
    let diagnostics = session.diagnostics(&program).unwrap();
    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert!(diagnostics.iter().all(|d| d.code == 2322));
    drop(program);
}

#[test]
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
        project.path(),
        BTreeMap::from([(file.clone(), source.clone())]),
        None,
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
    drop(program);
}

#[test]
fn native_incremental_program_tracks_created_and_deleted_modules() {
    let project = tempfile::tempdir().unwrap();
    let entry = project.path().join("index.ts");
    let dependency = project.path().join("dependency.ts");
    let entry_source: Arc<str> =
        Arc::from("import { value } from './dependency'; export const result: number = value;");
    let options = NativeCompilerOptions::arkts11(project.path());
    let initial_sources = BTreeMap::from([(entry.clone(), entry_source.clone())]);
    let mut session =
        NativeTypeSession::start(project.path(), initial_sources.clone(), None).unwrap();
    let program = session
        .create_program(std::slice::from_ref(&entry), &options)
        .unwrap();
    assert!(
        session
            .diagnostics(&program)
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic.code == 2307 })
    );

    let mut sources_with_dependency = initial_sources;
    sources_with_dependency.insert(
        dependency.clone(),
        Arc::from("export const value: number = 1;"),
    );
    let program = session
        .update_program_snapshot(
            program,
            std::slice::from_ref(&entry),
            &options,
            sources_with_dependency,
            &NativeProgramFileChanges {
                created: vec![dependency.clone()],
                ..NativeProgramFileChanges::default()
            },
        )
        .unwrap();
    assert!(session.diagnostics(&program).unwrap().is_empty());

    let program = session
        .update_program_snapshot(
            program,
            std::slice::from_ref(&entry),
            &options,
            BTreeMap::from([(entry.clone(), entry_source)]),
            &NativeProgramFileChanges {
                deleted: vec![dependency],
                ..NativeProgramFileChanges::default()
            },
        )
        .unwrap();
    assert!(
        session
            .diagnostics(&program)
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic.code == 2307 })
    );
    drop(program);
}

#[derive(Default)]
struct RecordingPreprocessor {
    calls: Mutex<BTreeMap<PathBuf, usize>>,
}
impl arkdown_ets_checker::NativeSourcePreprocessor for RecordingPreprocessor {
    fn preprocess(
        &self,
        path: &Path,
        source: Arc<str>,
    ) -> arkdown_ets_checker::NativeProcessedSource {
        *self
            .calls
            .lock()
            .unwrap()
            .entry(path.to_path_buf())
            .or_default() += 1;
        arkdown_ets_checker::NativeProcessedSource {
            text: Arc::from(source.replace("'replaceMe'", "1")),
            legacy_extend_names: BTreeSet::from(["used".into()]),
            ..Default::default()
        }
    }
}

#[test]
fn native_preprocesses_only_read_implementation_versions_and_keeps_incremental_diffs_raw() {
    let project = tempfile::tempdir().unwrap();
    let app = project.path().join("app.ts");
    let dependency = project.path().join("dependency.ts");
    let unused = project.path().join("unused.ets");
    let declaration = project.path().join("global.d.ts");
    let initial_sources = BTreeMap::from([
        (
            app.clone(),
            Arc::from("// 😀\nimport { value } from './dependency'; const actual: number = value;"),
        ),
        (
            dependency.clone(),
            Arc::from("export const value: number = 'replaceMe';"),
        ),
        (unused.clone(), Arc::from("@Extend Text.unused() {}")),
        (
            declaration.clone(),
            Arc::from("declare const declared: number;"),
        ),
    ]);
    let processor = Arc::new(RecordingPreprocessor::default());
    let mut session = NativeTypeSession::start(project.path(), initial_sources.clone(), None)
        .unwrap()
        .with_source_preprocessor(processor.clone());
    let options = NativeCompilerOptions::arkts11(project.path());
    let roots = [app.clone(), declaration.clone()];
    let old = session.create_program(&roots, &options).unwrap();
    assert!(session.diagnostics(&old).unwrap().is_empty());
    session.arkts_build_diagnostics(&old).unwrap();
    let read = session.read_sources();
    assert!(
        !read.contains_key(&unused),
        "Program must not preprocess unreferenced inventory files"
    );
    assert_eq!(
        read[&dependency].text.as_ref(),
        "export const value: number = 1;"
    );
    assert!(read[&declaration].legacy_extend_names.is_empty());
    assert_eq!(processor.calls.lock().unwrap().get(&app), Some(&1));
    assert_eq!(processor.calls.lock().unwrap().get(&dependency), Some(&1));
    assert!(!processor.calls.lock().unwrap().contains_key(&declaration));

    let mut changed = initial_sources.clone();
    changed.insert(
        app.clone(),
        Arc::from("// 😀\nimport { value } from './dependency'; const actual: string = value;"),
    );
    let next = session
        .update_program_snapshot(
            old.clone(),
            &roots,
            &options,
            changed.clone(),
            &NativeProgramFileChanges::default(),
        )
        .unwrap();
    assert!(
        session
            .diagnostics(&next)
            .unwrap()
            .iter()
            .any(|d| d.code == 2322)
    );
    assert!(
        session.diagnostics(&old).unwrap().is_empty(),
        "old Program retains its processed version"
    );
    assert_eq!(processor.calls.lock().unwrap().get(&app), Some(&2));
    assert_eq!(processor.calls.lock().unwrap().get(&dependency), Some(&1));

    changed.insert(
        dependency.clone(),
        Arc::from("export const value: string = 'new';"),
    );
    let updated = session
        .update_program_snapshot(
            next,
            &roots,
            &options,
            changed.clone(),
            &NativeProgramFileChanges::default(),
        )
        .unwrap();
    assert!(session.diagnostics(&updated).unwrap().is_empty());
    assert_eq!(
        session.read_sources()[&dependency].text.as_ref(),
        "export const value: string = 'new';"
    );
    assert_eq!(processor.calls.lock().unwrap().get(&app), Some(&2));
    assert_eq!(processor.calls.lock().unwrap().get(&dependency), Some(&2));
    let mut fresh = NativeTypeSession::start(project.path(), changed, None)
        .unwrap()
        .with_source_preprocessor(Arc::new(RecordingPreprocessor::default()));
    let rebuilt = fresh.create_program(&roots, &options).unwrap();
    assert_eq!(
        session.source_graph(&updated).unwrap().files.len(),
        fresh.source_graph(&rebuilt).unwrap().files.len()
    );
    assert!(fresh.diagnostics(&rebuilt).unwrap().is_empty());
}

#[test]
fn native_disk_preprocessing_reuses_versions_and_invalidates_observed_changes() {
    let project = tempfile::tempdir().unwrap();
    let app = project.path().join("app.ts");
    let dependency = project.path().join("dependency.ts");
    std::fs::write(&dependency, "export const value: number = 'replaceMe';").unwrap();
    let sources = BTreeMap::from([(
        app.clone(),
        Arc::from("import { value } from './dependency'; const actual: number = value;"),
    )]);
    let processor = Arc::new(RecordingPreprocessor::default());
    let mut session = NativeTypeSession::start(project.path(), sources.clone(), None)
        .unwrap()
        .with_source_preprocessor(processor.clone());
    let options = NativeCompilerOptions::arkts11(project.path());
    let roots = [app];
    let old = session.create_program(&roots, &options).unwrap();
    assert!(session.diagnostics(&old).unwrap().is_empty());
    assert_eq!(
        session.read_sources()[&dependency].text.as_ref(),
        "export const value: number = 1;"
    );
    let unchanged = session
        .update_program_snapshot(
            old.clone(),
            &roots,
            &options,
            sources.clone(),
            &NativeProgramFileChanges::default(),
        )
        .unwrap();
    assert!(session.diagnostics(&unchanged).unwrap().is_empty());
    assert_eq!(processor.calls.lock().unwrap().get(&dependency), Some(&1));
    std::fs::write(&dependency, "export const value: string = 'new';").unwrap();
    let next = session
        .update_program_snapshot(
            unchanged,
            &roots,
            &options,
            sources,
            &NativeProgramFileChanges {
                changed: vec![dependency.clone()],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        session
            .diagnostics(&next)
            .unwrap()
            .iter()
            .any(|d| d.code == 2322)
    );
    assert!(session.diagnostics(&old).unwrap().is_empty());
    assert_eq!(processor.calls.lock().unwrap().get(&dependency), Some(&2));
    assert_eq!(
        session.read_sources()[&dependency].text.as_ref(),
        "export const value: string = 'new';"
    );
}
