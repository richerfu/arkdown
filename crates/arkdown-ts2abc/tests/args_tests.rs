use std::path::PathBuf;

use arkdown_ts2abc::{
    parse_ts2abc_args, ts2abc_script_target_bc_version, BytecodeVersion, Ts2abcCommand, Ts2abcError,
};

#[test]
fn target_bc_version_matches_ts2abc_wrapper_api8_compat() {
    let command = parse_ts2abc_args(["--target-bc-version", "--target-api-version", "8"]).unwrap();

    match command {
        Ts2abcCommand::PrintTargetBcVersion(query) => {
            assert_eq!(query.bytecode_version, BytecodeVersion::new(0, 0, 0, 2));
        }
        other => panic!("expected version query, got {other:?}"),
    }
}

#[test]
fn target_bc_version_matches_api17_release() {
    assert_eq!(
        ts2abc_script_target_bc_version(17, Some("Release")).to_string(),
        "12.0.6.0"
    );
}

#[test]
fn target_bc_version_matches_api18_current_generation() {
    assert_eq!(
        ts2abc_script_target_bc_version(18, None),
        BytecodeVersion::new(13, 0, 1, 0)
    );
}

#[test]
fn parses_minimal_compile_command() {
    let command = parse_ts2abc_args([
        "--module",
        "--merge-abc",
        "--debug-info",
        "--opt-level=0",
        "--target-api-version=17",
        "--target-api-sub-version",
        "Release",
        "--record-name",
        "entry|entry|1.0.0|src/main/ets/pages/Index",
        "--source-file",
        "entry/src/main/ets/pages/Index.ts",
        "--output",
        "modules.abc",
        "Index.js",
    ])
    .unwrap();

    match command {
        Ts2abcCommand::Compile(options) => {
            assert!(options.module);
            assert!(options.merge_abc);
            assert!(options.debug_info);
            assert_eq!(options.opt_level, 0);
            assert_eq!(options.target_api_version, 17);
            assert_eq!(options.target_api_sub_version.as_deref(), Some("Release"));
            assert_eq!(options.output, PathBuf::from("modules.abc"));
            assert_eq!(options.inputs.len(), 1);
            assert_eq!(
                options.inputs[0].record_name,
                "entry|entry|1.0.0|src/main/ets/pages/Index"
            );
            assert_eq!(
                options.inputs[0].source_file.as_deref(),
                Some("entry/src/main/ets/pages/Index.ts")
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn parses_npm_module_entry_list_flag() {
    let command = parse_ts2abc_args([
        "--module",
        "--merge-abc",
        "--npm-module-entry-list",
        "npmEntries.txt",
        "--output",
        "modules.abc",
        "Index.js",
    ])
    .unwrap();

    match command {
        Ts2abcCommand::Compile(options) => {
            assert_eq!(
                options.npm_module_entry_list,
                Some(PathBuf::from("npmEntries.txt"))
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn rejects_unknown_native_backend_flag_explicitly() {
    let err = parse_ts2abc_args(["--dump-assembly"]).unwrap_err();
    assert!(matches!(err, Ts2abcError::UnsupportedFlag(flag) if flag == "--dump-assembly"));
}

#[test]
fn rejects_missing_output_for_compile() {
    let err = parse_ts2abc_args(["input.js"]).unwrap_err();
    assert!(matches!(err, Ts2abcError::MissingOutput));
}
