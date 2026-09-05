use std::fs;

use arkdown_ets_checker::{CheckOptions, check};

#[test]
fn checks_arkts_1_1_roots_and_component_members() {
    let project = tempfile::tempdir().unwrap();
    let source = project.path().join("Index.ets");
    fs::write(
        &source,
        r#"
@Component
struct Demo {
  count: number = "bad";

  value(): string {
    return 1;
  }
}

const bad: string = 1;
"#,
    )
    .unwrap();

    let result = check(CheckOptions {
        project_root: project.path().to_path_buf(),
        root_files: vec![source],
        tsconfig_path: None,
        module_paths: Vec::new(),
        aliases: Vec::new(),
        strict_null_checks: false,
        skip_lib_check: true,
        report_isolated_declaration_diagnostics: false,
    })
    .unwrap();

    let messages = result
        .files
        .iter()
        .flat_map(|file| file.diagnostics.iter())
        .map(|diagnostic| &*diagnostic.message)
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 3, "{messages:#?}");
    assert!(
        messages
            .iter()
            .all(|message| message.contains("is not assignable to type")),
        "{messages:#?}"
    );
}

#[test]
fn resolves_exact_sdk_declaration_aliases() {
    let project = tempfile::tempdir().unwrap();
    let declaration = project.path().join("@kit.TestKit.d.ts");
    fs::write(&declaration, "export declare const sdkValue: number;\n").unwrap();
    let source = project.path().join("Index.ets");
    fs::write(
        &source,
        "import { sdkValue } from '@kit.TestKit';\nconst bad: string = sdkValue;\n",
    )
    .unwrap();

    let result = check(CheckOptions {
        project_root: project.path().to_path_buf(),
        root_files: vec![source.clone()],
        tsconfig_path: None,
        module_paths: Vec::new(),
        aliases: vec![("@kit.TestKit$".to_string(), declaration)],
        strict_null_checks: false,
        skip_lib_check: true,
        report_isolated_declaration_diagnostics: false,
    })
    .unwrap();

    assert_eq!(result.error_count(), 1);
    let source_result = result
        .files
        .iter()
        .find(|file| file.path == source)
        .expect("source must be part of the checked program");
    assert!(
        source_result.diagnostics[0]
            .message
            .contains("is not assignable to type 'string'")
    );
}
