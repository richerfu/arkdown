use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arkdown_ets_checker::{NativeCompilerOptions, NativeTypeSession};

/// Grammar acceptance gate, not a replacement for semantic parity tests.
/// OXC tasks/coverage/misc/pass/arkts-* uses the legacy .ets source type.
/// Static ETS has separate, explicitly selected parser fixtures and is outside
/// this gate. Keep original corpus text, including declaration-file extensions.
#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH and ARKDOWN_OXC_SOURCE"]
fn native_accepts_oxc_arkts11_positive_syntax_corpus() {
    check_corpus("pass");
}

#[test]
#[ignore = "requires ARKDOWN_TYPESCRIPT_PATH and ARKDOWN_OXC_SOURCE"]
fn native_rejects_oxc_arkts11_negative_corpus() {
    check_corpus("fail");
}

fn check_corpus(kind: &str) {
    let root = PathBuf::from(std::env::var_os("ARKDOWN_OXC_SOURCE").expect("ARKDOWN_OXC_SOURCE"));
    let directory = root.join("tasks/coverage/misc").join(kind);
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "ets")
                && path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .is_some_and(|name| name.starts_with("arkts-"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "OXC legacy ArkTS corpus was not found");

    let project = tempfile::tempdir().unwrap();
    let executable = NativeTypeSession::executable().unwrap();
    let mut options = NativeCompilerOptions::arkts11(project.path());
    // Unmodified compilerOptions.ets from ets2bundle compiler/tsconfig.json
    // (70818d757b52). Grammar tables, not fabricated SDK declarations.
    options.ets = Arc::new(
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/compiler/ets-options.json"
        ))
        .unwrap(),
    );
    let mut failures = Vec::new();
    for path in &paths {
        let name = path.file_name().unwrap();
        let file = project.path().join(name);
        let source: Arc<str> = std::fs::read_to_string(path).unwrap().into();
        // Isolate programs: corpus files can deliberately redeclare the same
        // names and do not form one SDK project.
        let mut session = NativeTypeSession::start(
            &executable,
            project.path(),
            BTreeMap::from([(file.clone(), source)]),
            None,
        )
        .unwrap();
        let program = session.create_program(&[file], &options).unwrap();
        // OH checkAnnotationDeclaration owns 28024. Unlike OXC, OH accepts
        // this syntax before rejecting the non-top-level declaration. Do not
        // accept an unrelated missing-SDK diagnostic as proof of rejection.
        let nested_annotation = kind == "fail" && name == "arkts-annotation-nested.ets";
        let diagnostics = if nested_annotation {
            session.diagnostics(&program).unwrap()
        } else {
            session.syntactic_diagnostics(&program).unwrap()
        };
        session.release(program).unwrap();
        let matches_contract = if kind == "pass" {
            diagnostics.is_empty()
        } else if nested_annotation {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == 28024)
        } else {
            !diagnostics.is_empty()
        };
        if !matches_contract {
            failures.push(format!(
                "{}:\n{}",
                name.to_string_lossy(),
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.render())
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{} OXC ArkTS 1.1 {kind} fixtures failed:\n{}",
        failures.len(),
        paths.len(),
        failures.join("\n\n")
    );
}
