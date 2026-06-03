use std::path::PathBuf;

use arkdown_ts2abc::{
    ts2abc_script_target_bc_version, BytecodeVersion, Ts2abcCommand, Ts2abcError,
};

#[test]
fn target_bc_version_matches_ts2abc_wrapper_api8_compat() {
    let command =
        Ts2abcCommand::parse(["--target-bc-version", "--target-api-version", "8"]).unwrap();

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
    let command = Ts2abcCommand::parse([
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
    let command = Ts2abcCommand::parse([
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
    let err = Ts2abcCommand::parse(["--dump-assembly"]).unwrap_err();
    assert!(matches!(err, Ts2abcError::UnsupportedFlag(flag) if flag == "--dump-assembly"));
}

#[test]
fn rejects_missing_output_for_compile() {
    let err = Ts2abcCommand::parse(["input.js"]).unwrap_err();
    assert!(matches!(err, Ts2abcError::MissingOutput));
}

/// Upstream es2abc defaults --target-api-version to 24 (options.cpp:571).
/// Our previous default was 20, which caused wrong bytecode-version selection.
#[test]
fn default_target_api_version_is_24() {
    let command = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--output",
        "modules.abc",
        "input.js",
    ])
    .unwrap();
    match command {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(
                opts.target_api_version, 24,
                "default target API version must be 24 to match upstream es2abc"
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

/// Upstream options.cpp rejects --module and --commonjs together.
#[test]
fn rejects_module_and_commonjs_together() {
    let err = Ts2abcCommand::parse(["--module", "--commonjs", "--output", "x.abc", "x.js"])
        .unwrap_err();
    assert!(
        matches!(err, Ts2abcError::ConflictingFlags),
        "expected ConflictingFlags, got {err:?}"
    );
    // Same if commonjs comes first.
    let err2 = Ts2abcCommand::parse(["--commonjs", "--module", "--output", "x.abc", "x.js"])
        .unwrap_err();
    assert!(
        matches!(err2, Ts2abcError::ConflictingFlags),
        "expected ConflictingFlags, got {err2:?}"
    );
}

/// @filelist should expand each non-empty line as a source input.
#[test]
fn filelist_expands_lines_as_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let list_path = dir.path().join("sources.txt");
    let a = dir.path().join("a.js");
    let b = dir.path().join("b.js");
    std::fs::write(&a, "").unwrap();
    std::fs::write(&b, "").unwrap();
    std::fs::write(
        &list_path,
        format!("{}\n\n{}\n", a.display(), b.display()),
    )
    .unwrap();

    let command = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--output",
        "modules.abc",
        &format!("@{}", list_path.display()),
    ])
    .unwrap();
    match command {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(
                opts.inputs.len(),
                2,
                "filelist with 2 paths (and 1 blank line) must produce 2 inputs"
            );
            assert_eq!(opts.inputs[0].path, a);
            assert_eq!(opts.inputs[1].path, b);
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

/// @filelist that cannot be read must return an Io error, not UnsupportedFlag.
#[test]
fn filelist_missing_file_returns_io_error() {
    let err =
        Ts2abcCommand::parse(["@/nonexistent/path/sources.txt", "--output", "x.abc"]).unwrap_err();
    assert!(
        matches!(err, Ts2abcError::Io(_)),
        "missing @filelist must be an Io error, got {err:?}"
    );
}

/// Flags added for hvigor compatibility must be accepted without error.
#[test]
fn accepts_hvigor_compat_flags() {
    // These flags must not produce UnsupportedFlag even if their effect is no-op.
    let result = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--extension",
        "ets",
        "--module-record-field-name",
        "moduleRecordField",
        "--enable-release-column",
        "--use-define-semantic",
        "--output",
        "modules.abc",
        "input.js",
    ]);
    assert!(
        result.is_ok(),
        "hvigor compat flags must be accepted: {result:?}"
    );
}
