//! Regression coverage for the native OXC check application.

use std::fs;
use std::path::{Component, Path, PathBuf};

use arkdown_build::check::{CheckApplication, CheckRequest};
use walkdir::WalkDir;

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
    })
    .execute()
    .expect_err("native OXC checker must reject a TypeScript type mismatch");
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
fn check_defaults_to_native_oxc_without_a_node_runtime() {
    let fixture = repo_root().join("tests/fixtures/minimal-hap");
    CheckApplication::new(CheckRequest {
        target: "hap".to_string(),
        module: None,
        project: Some(fixture.clone()),
        is_crossplatform: false,
        ignore_crossplatform_check: false,
    })
    .execute()
    .expect("default OXC check must not require Node");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .parent()
        .expect("repository root")
        .to_path_buf()
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
