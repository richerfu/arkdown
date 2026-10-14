//! In-process tsrs check application regression. Uses linked compiler crates and the configured
//! OpenHarmony SDK.

#![recursion_limit = "256"]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_build::check::{CheckApplication, CheckRequest};
use arkdown_ets_checker::{
    NativeCheckError, NativeCompilerOptions, NativeOhSdkClassCheckPlugin,
    NativeOhSdkPluginCompletion, NativeOhSdkPluginHost, NativeOhSdkPluginRequest,
    NativeOhSdkPluginResponse, NativeTypeSession,
};
use walkdir::WalkDir;

#[derive(Debug)]
struct UnavailableSdkPluginHost;

impl NativeOhSdkPluginHost for UnavailableSdkPluginHost {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
        Ok(vec![NativeOhSdkPluginResponse::default(); requests.len()])
    }
}

fn sdk_plugin_host() -> Option<Arc<dyn NativeOhSdkPluginHost>> {
    Some(Arc::new(UnavailableSdkPluginHost))
}

#[test]
fn check_reports_native_typescript_semantic_diagnostics() {
    let fixture = repo_root().join("tests/fixtures/minimal-hap");
    let temp = tempfile::tempdir().expect("temporary semantic-check fixture");
    copy_fixture(&fixture, temp.path()).expect("copy minimal HAP fixture");
    fs::write(
        temp.path().join("oh-package.json5"),
        r#"{
  "modelVersion": "6.0.0",
  "description": "Isolated semantic-check fixture.",
  "dependencies": {},
  "devDependencies": {}
}
"#,
    )
    .expect("remove unrelated test-runner dependency from semantic-check fixture");
    let source = temp.path().join("entry/src/main/ets/pages/Index.ets");
    let original = fs::read_to_string(&source).expect("read copied ArkTS source");
    fs::write(
        &source,
        format!("{original}\nconst semanticTypeError: number = \"not-a-number\";\n"),
    )
    .expect("inject TypeScript semantic error");

    let error = CheckApplication::new(CheckRequest {
        target: "hap".to_string(),
        module: None,
        project: Some(temp.path().to_path_buf()),
        is_crossplatform: false,
        ignore_crossplatform_check: false,
        sdk_plugin_host: sdk_plugin_host(),
        heap_reclaimer: None,
    })
    .execute()
    .expect_err("native TypeScript checker must reject a TypeScript type mismatch");
    let diagnostics = format!("{error:#}");

    assert!(
        diagnostics.contains("ARKDOWN_ETS_CHECK_FAILED"),
        "check must report its semantic-check failure boundary; output:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("not assignable to type 'number'"),
        "failure must originate from the native semantic checker; output:\n{diagnostics}"
    );
}

#[test]
fn check_defaults_to_native_typescript_without_a_node_runtime() {
    let fixture = repo_root().join("tests/fixtures/minimal-hap");
    CheckApplication::new(CheckRequest {
        target: "hap".to_string(),
        module: None,
        project: Some(fixture.clone()),
        is_crossplatform: false,
        ignore_crossplatform_check: false,
        sdk_plugin_host: sdk_plugin_host(),
        heap_reclaimer: None,
    })
    .execute()
    .expect("native TypeScript check must accept the SDK fixture without Node");
}

#[test]
fn check_uses_module_target_and_its_source_owned_standard_library() {
    // ets_checker.ts::getTargetESVersionLib pairs ES2017 with ES2020. A
    // module-level override must affect the tsrs program, not only OXC emit.
    for target in ["ES2017", "ES2021"] {
        let temp = tempfile::tempdir().expect("module target fixture");
        copy_fixture(&repo_root().join("tests/fixtures/minimal-hap"), temp.path()).unwrap();
        fs::write(
            temp.path().join("oh-package.json5"),
            r#"{"modelVersion":"6.0.0","dependencies":{},"devDependencies":{}}"#,
        )
        .unwrap();
        fs::write(temp.path().join("entry/build-profile.json5"), format!(
            r#"{{"apiType":"stageMode","buildOption":{{"arkOptions":{{"tscConfig":{{"targetESVersion":"{target}"}}}}}},"targets":[{{"name":"default"}}]}}"#
        )).unwrap();
        fs::write(temp.path().join("entry/src/main/ets/Target.ets"),
            "export const value: bigint = 1n;\nexport const promise: Promise<number> = Promise.any([Promise.resolve(1)]);\n").unwrap();
        prepend_entry_import(temp.path(), "../Target");
        let result = CheckApplication::new(CheckRequest {
            target: "hap".into(),
            module: None,
            project: Some(temp.path().to_path_buf()),
            is_crossplatform: false,
            ignore_crossplatform_check: false,
            sdk_plugin_host: sdk_plugin_host(),
            heap_reclaimer: None,
        })
        .execute();
        if target == "ES2017" {
            let diagnostic = format!("{:#}", result.expect_err("ES2017 target restrictions"));
            assert!(
                diagnostic.contains("BigInt literals are not available"),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains("Property 'any' does not exist"),
                "{diagnostic}"
            );
        } else {
            result.expect("ES2021 supports bigint literals and Promise.any");
        }
    }
}

#[test]
fn check_applies_source_owned_flow_limit_and_invalid_range_default() {
    // ets_checker.ts::getMaxFlowDepth normalizes an out-of-range module value
    // to 2000. Go checker.ts::getTypeAtFlowNode must consume the chosen limit.
    for limit in [1999, 65535] {
        let temp = tempfile::tempdir().expect("flow limit fixture");
        copy_fixture(&repo_root().join("tests/fixtures/minimal-hap"), temp.path()).unwrap();
        fs::write(
            temp.path().join("oh-package.json5"),
            r#"{"modelVersion":"6.0.0","dependencies":{},"devDependencies":{}}"#,
        )
        .unwrap();
        fs::write(temp.path().join("entry/build-profile.json5"), format!(
            r#"{{"apiType":"stageMode","buildOption":{{"arkOptions":{{"tscConfig":{{"maxFlowDepth":{limit}}}}}}},"targets":[{{"name":"default"}}]}}"#
        )).unwrap();
        fs::write(
            temp.path().join("entry/src/main/ets/Flow.ets"),
            format!(
                "export function f(x: string | number, condition: boolean) {{\n{}return x; }}",
                "if (condition) { x = 1; }\n".repeat(2050)
            ),
        )
        .unwrap();
        prepend_entry_import(temp.path(), "../Flow");
        let result = CheckApplication::new(CheckRequest {
            target: "hap".into(),
            module: None,
            project: Some(temp.path().to_path_buf()),
            is_crossplatform: false,
            ignore_crossplatform_check: false,
            sdk_plugin_host: sdk_plugin_host(),
            heap_reclaimer: None,
        })
        .execute();
        if limit == 1999 {
            let diagnostic = format!("{:#}", result.expect_err("out-of-range defaults to 2000"));
            assert!(
                diagnostic.contains("too large for control flow analysis"),
                "{diagnostic}"
            );
        } else {
            result.expect("configured flow depth reaches the native checker");
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("repository root")
        .to_path_buf()
}

fn prepend_entry_import(project: &Path, specifier: &str) {
    let entry = project.join("entry/src/main/ets/pages/Index.ets");
    let source = fs::read_to_string(&entry).expect("read entry source");
    fs::write(&entry, format!("import \"{specifier}\";\n{source}"))
        .expect("make test source reachable from the configured page entry");
}

fn copy_fixture(source: &Path, destination: &Path) -> std::io::Result<()> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("fixture entry is nested");
        if relative.as_os_str().is_empty() || is_generated_fixture_path(relative) {
            continue;
        }
        let output = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(output)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), output)?;
        }
    }
    Ok(())
}

fn is_generated_fixture_path(relative: &Path) -> bool {
    relative.components().any(|component| {
    matches!(component, Component::Normal(name) if name == "build" || name == ".hvigor")
  })
}

#[derive(Default)]
struct RecordingSdkHost {
    operations: std::sync::Mutex<Vec<String>>,
    pending: std::sync::Mutex<Vec<NativeOhSdkPluginCompletion>>,
}

impl std::fmt::Debug for RecordingSdkHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordingSdkHost").finish_non_exhaustive()
    }
}

impl NativeOhSdkPluginHost for RecordingSdkHost {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
        Ok(requests
            .into_iter()
            .map(|request| {
                assert!(
                    !matches!(
                        request.operation.as_str(),
                        "compactSession" | "releaseSources"
                    ),
                    "session maintenance must not wait on JS"
                );
                self.operations
                    .lock()
                    .unwrap()
                    .push(request.operation.clone());
                NativeOhSdkPluginResponse {
                    found: true,
                    // An uncached load error makes strict-only retry syscap.
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
        if requests.len() == 1
            && matches!(
                requests[0].operation.as_str(),
                "compactSession" | "releaseSources"
            )
        {
            self.operations
                .lock()
                .unwrap()
                .push(requests[0].operation.clone());
            self.pending.lock().unwrap().push(complete);
        } else {
            complete(self.invoke_batch(requests));
        }
    }
}

#[derive(Debug, Default)]
struct RecordingReclaimer {
    boundaries: std::sync::Mutex<Vec<arkdown_build::request::BuildHeapBoundary>>,
    sdk: Arc<RecordingSdkHost>,
}

impl arkdown_build::request::BuildHeapReclaimer for RecordingReclaimer {
    fn reclaim(&self, boundary: arkdown_build::request::BuildHeapBoundary) {
        self.boundaries.lock().unwrap().push(boundary);
        if boundary == arkdown_build::request::BuildHeapBoundary::NormalCheckerReleased {
            assert_eq!(
                self.sdk.operations.lock().unwrap().last().unwrap(),
                "releaseSources",
                "one-shot source release must be queued before pre-strict reclaim"
            );
        }
    }
}

#[tokio::test]
async fn build_request_reclaims_after_checker_bundle_before_bytecode_and_pack() {
    use arkdown_build::request::{BuildHeapBoundary, BuildRequest};
    let temp = tempfile::tempdir().unwrap();
    copy_fixture(&repo_root().join("tests/fixtures/minimal-hap"), temp.path()).unwrap();
    fs::write(
        temp.path().join("oh-package.json5"),
        r#"{"modelVersion":"6.0.0","dependencies":{},"devDependencies":{}}"#,
    )
    .unwrap();
    let reclaimer = Arc::new(RecordingReclaimer::default());
    arkdown_build::BuildService::new(BuildRequest {
        project: Some(temp.path().into()),
        target: "hap".into(),
        mode: "debug".into(),
        sdk_plugin_host: Some(reclaimer.sdk.clone()),
        heap_reclaimer: Some(reclaimer.clone()),
        ..Default::default()
    })
    .execute()
    .await
    .unwrap();
    assert_eq!(
        *reclaimer.boundaries.lock().unwrap(),
        [
            BuildHeapBoundary::NormalCheckerReleased,
            BuildHeapBoundary::CheckerReleased,
            BuildHeapBoundary::BundleCompleted,
            BuildHeapBoundary::BeforeBytecode,
            BuildHeapBoundary::BeforePack,
        ]
    );
    let operations = reclaimer.sdk.operations.lock().unwrap();
    assert_eq!(
        operations
            .iter()
            .filter(|op| *op == "releaseSources")
            .count(),
        1
    );
    assert_eq!(operations.last().unwrap(), "releaseSession");
}

#[test]
fn one_shot_sdk_source_release_respects_strict_only_and_disabled_linter_options() {
    // Build configuration fixes strictCheckerOnly=false. Drive the same native
    // boundary with explicit options to protect callers that enable strict-only.
    for (linter, strict_only, release) in [
        (true, false, true),
        (true, true, false),
        (false, false, false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let sdk = temp.path().join("api.d.ets");
        let app = temp.path().join("entry.ets");
        let normal_app = temp.path().join("entry.ts");
        let sources = std::collections::BTreeMap::from([
            (
                sdk.clone(),
                Arc::from("/** @syscap base */ declare function sdkCall(): void;"),
            ),
            (app.clone(), Arc::from("export {}; sdkCall();")),
            (normal_app.clone(), Arc::from("export {}; sdkCall();")),
        ]);
        let mut options = NativeCompilerOptions::arkts11(temp.path());
        options.need_do_ark_ts_linter = linter;
        options.strict_checker_only = strict_only;
        options.ets_loader_path = temp.path().join("ets-loader");
        options.oh_compile_mode = "moduleJson".into();
        options.oh_device_types = vec!["phone".into()];
        options.oh_syscap_intersection = vec!["base".into()];
        options.oh_all_module_paths = vec![sdk.clone()];
        options.oh_sdk_class_check_plugins = vec![NativeOhSdkClassCheckPlugin {
            tag_name: "syscap".into(),
            path: temp.path().join("checker.cjs"),
            class_name: "Checker".into(),
        }];
        let host = Arc::new(RecordingSdkHost::default());
        let mut session =
            NativeTypeSession::start(temp.path(), sources, Some(host.clone())).unwrap();
        let program = session
            .create_program(&[sdk, app, normal_app], &options)
            .unwrap();
        let normal = session.arkts_normal_diagnostics(&program).unwrap();
        assert!(host
            .operations
            .lock()
            .unwrap()
            .iter()
            .any(|op| op == "syscap"));
        session.release_arkts_build_checkers(&program).unwrap();
        session.release_sdk_sources_before_strict(&options);
        let strict_start = host.operations.lock().unwrap().len();
        session.arkts_strict_diagnostics(&program, normal).unwrap();
        drop(program);
        drop(session);
        let operations = host.operations.lock().unwrap();
        assert_eq!(
            operations
                .iter()
                .filter(|op| *op == "releaseSources")
                .count(),
            usize::from(release)
        );
        assert_eq!(
            operations[strict_start..].iter().any(|op| op == "syscap"),
            strict_only
        );
        assert_eq!(operations.last().unwrap(), "releaseSession");
    }
}
