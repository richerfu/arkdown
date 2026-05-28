use arkdown_ts2abc::{compile_to_abc, Ts2abcInput, Ts2abcOptions};

#[test]
fn compile_to_abc_uses_rust_backend_and_writes_panda_file() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "(function () { var x = 1; return x; })();").unwrap();

    let result = compile_to_abc(Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            record_name: "entry|entry|1.0.0|src/main/ets/pages/Index".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        merge_abc: true,
        module: true,
        commonjs: false,
        cache_file: None,
    })
    .unwrap();

    assert_eq!(result.abc_path, output);
    assert_eq!(result.compiled_records.len(), 1);
    let bytes = fs_err::read(result.abc_path).unwrap();
    assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
}

#[test]
fn compile_to_abc_accepts_module_source_without_iife_wrapper() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("entry.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    let result = compile_to_abc(Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            record_name: "entry/main".into(),
            source_file: None,
        }],
        output: output.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        merge_abc: true,
        module: true,
        commonjs: false,
        cache_file: None,
    })
    .unwrap();

    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
}
