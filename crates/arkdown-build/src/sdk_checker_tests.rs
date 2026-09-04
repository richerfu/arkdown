use super::sdk_checker::ArkLinterEntryResolver;

#[test]
fn ark_linter_uses_package_main_before_conventional_entries() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(
        temp.path().join("oh-package.json5"),
        "{ main: './src/Public.ets' }",
    )
    .unwrap();
    std::fs::write(temp.path().join("src/Public.ets"), "export const value = 1").unwrap();
    std::fs::write(temp.path().join("index.ets"), "export const fallback = 1").unwrap();

    assert_eq!(
        ArkLinterEntryResolver::new(temp.path()).resolve().unwrap(),
        temp.path().join("src/Public.ets")
    );
}

#[test]
fn ark_linter_probes_ets_then_ts_then_js_and_rejects_missing_entries() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("oh-package.json5"), "{}").unwrap();
    std::fs::write(temp.path().join("index.ts"), "export const value = 1").unwrap();
    std::fs::write(temp.path().join("index.js"), "exports.value = 1").unwrap();

    assert_eq!(
        ArkLinterEntryResolver::new(temp.path()).resolve().unwrap(),
        temp.path().join("index.ts")
    );
    std::fs::remove_file(temp.path().join("index.ts")).unwrap();
    std::fs::remove_file(temp.path().join("index.js")).unwrap();
    let error = ArkLinterEntryResolver::new(temp.path())
        .resolve()
        .unwrap_err()
        .to_string();
    assert!(error.contains("ARKDOWN_ARK_LINTER_MAIN_NOT_FOUND"));
}
