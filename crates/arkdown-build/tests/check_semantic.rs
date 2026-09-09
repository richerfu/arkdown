//! Real native TypeScript check application regression. Uses the native TSGO
//! executable shipped in arkdown's current platform package and the configured
//! OpenHarmony SDK.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_build::check::{CheckApplication, CheckRequest};
use arkdown_ets_checker::{
    NativeCheckError, NativeOhSdkPluginHost, NativeOhSdkPluginRequest, NativeOhSdkPluginResponse,
};
use walkdir::WalkDir;

#[derive(Debug)]
struct UnavailableSdkPluginHost;

impl NativeOhSdkPluginHost for UnavailableSdkPluginHost {
    fn invoke(
        &self,
        _request: NativeOhSdkPluginRequest,
    ) -> Result<NativeOhSdkPluginResponse, NativeCheckError> {
        Ok(NativeOhSdkPluginResponse::default())
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
        typescript_executable: Some(packaged_typescript_executable()),
        sdk_plugin_host: sdk_plugin_host(),
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
        typescript_executable: Some(packaged_typescript_executable()),
        sdk_plugin_host: sdk_plugin_host(),
    })
    .execute()
    .expect("native TypeScript check must accept the SDK fixture without Node");
}

#[test]
fn check_uses_module_target_and_its_source_owned_standard_library() {
    // ets_checker.ts::getTargetESVersionLib pairs ES2017 with ES2020. A
    // module-level override must affect the Go program, not only OXC emit.
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
            typescript_executable: Some(packaged_typescript_executable()),
            sdk_plugin_host: sdk_plugin_host(),
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
            typescript_executable: Some(packaged_typescript_executable()),
            sdk_plugin_host: sdk_plugin_host(),
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

fn packaged_typescript_executable() -> PathBuf {
    let (platform, executable) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => ("darwin-arm64", "tsc"),
        ("linux", "x86_64") => ("linux-x64-gnu", "tsc"),
        ("windows", "x86_64") => ("win32-x64-msvc", "tsc.exe"),
        (os, arch) => panic!("arkdown has no native TypeScript platform package for {os}/{arch}"),
    };
    let path = repo_root()
        .join("packages/arkdown/npm")
        .join(platform)
        .join(executable);
    assert!(
        path.is_file(),
        "native TypeScript platform artifact is missing: {}",
        path.display()
    );
    path
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
