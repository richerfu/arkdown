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
            assert!(options.optimize_try_catch_func);
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
fn parses_opt_try_catch_func_flag_like_upstream_bool() {
    let command = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--opt-try-catch-func=false",
        "--output",
        "modules.abc",
        "Index.js",
    ])
    .unwrap();

    match command {
        Ts2abcCommand::Compile(options) => {
            assert!(!options.optimize_try_catch_func);
        }
        other => panic!("expected compile command, got {other:?}"),
    }

    let command = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--opt-try-catch-func",
        "--output",
        "modules.abc",
        "Index.js",
    ])
    .unwrap();

    match command {
        Ts2abcCommand::Compile(options) => {
            assert!(options.optimize_try_catch_func);
            assert_eq!(options.output, PathBuf::from("modules.abc"));
            assert_eq!(options.inputs[0].path, PathBuf::from("Index.js"));
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn non_merge_command_ignores_record_name_like_upstream() {
    let command = Ts2abcCommand::parse([
        "--module",
        "--record-name",
        "entry|entry|1.0.0|src/main/ets/pages/Index",
        "--output",
        "Index.abc",
        "Index.js",
    ])
    .unwrap();

    match command {
        Ts2abcCommand::Compile(options) => {
            assert!(!options.merge_abc);
            assert_eq!(options.inputs.len(), 1);
            assert_eq!(options.inputs[0].record_name, "");
            assert_eq!(options.inputs[0].source_file.as_deref(), Some("Index.js"));
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
fn derives_default_output_for_compile_like_upstream() {
    let command = Ts2abcCommand::parse(["input.js"]).unwrap();
    match command {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.output, PathBuf::from("input.abc"));
            assert_eq!(opts.inputs[0].record_name, "");
            assert_eq!(opts.inputs[0].source_file.as_deref(), Some("input.js"));
        }
        other => panic!("expected compile command, got {other:?}"),
    }

    let command = Ts2abcCommand::parse(["--module", "--merge-abc", "input.js"]).unwrap();
    match command {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.output, PathBuf::from("input.abc"));
            assert_eq!(opts.inputs[0].record_name, "input");
        }
        other => panic!("expected compile command, got {other:?}"),
    }
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
            assert_eq!(
                opts.inputs[0].record_name, "modules",
                "merge-abc default record name comes from compiler output basename"
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

/// Upstream options.cpp rejects --module and --commonjs together.
#[test]
fn rejects_module_and_commonjs_together() {
    let err =
        Ts2abcCommand::parse(["--module", "--commonjs", "--output", "x.abc", "x.js"]).unwrap_err();
    assert!(
        matches!(err, Ts2abcError::ConflictingFlags),
        "expected ConflictingFlags, got {err:?}"
    );
    // Same if commonjs comes first.
    let err2 =
        Ts2abcCommand::parse(["--commonjs", "--module", "--output", "x.abc", "x.js"]).unwrap_err();
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
    std::fs::write(&list_path, format!("{}\n\n{}\n", a.display(), b.display())).unwrap();

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

#[test]
fn filelist_parses_non_merge_output_files() {
    let dir = tempfile::tempdir().unwrap();
    let list_path = dir.path().join("sources.txt");
    let a = dir.path().join("a.js");
    let b = dir.path().join("b.js");
    let a_out = dir.path().join("a.abc");
    let b_out = dir.path().join("b.abc");
    std::fs::write(&a, "").unwrap();
    std::fs::write(&b, "").unwrap();
    std::fs::write(
        &list_path,
        format!(
            "{};ignored_a;module;src/a.ts;{};false;ets\n{};ignored_b;module;src/b.ts;{};true;ts\n",
            a.display(),
            a_out.display(),
            b.display(),
            b_out.display()
        ),
    )
    .unwrap();

    let command = Ts2abcCommand::parse(["--module", &format!("@{}", list_path.display())]).unwrap();

    match command {
        Ts2abcCommand::Compile(opts) => {
            assert!(!opts.merge_abc);
            assert_eq!(opts.output, a_out);
            assert_eq!(opts.inputs.len(), 2);
            assert_eq!(opts.inputs[0].record_name, "");
            assert_eq!(opts.inputs[0].source_file.as_deref(), Some("src/a.ts"));
            assert_eq!(
                opts.inputs[0].output.as_deref(),
                Some(opts.output.as_path())
            );
            assert!(!opts.inputs[0].is_shared_module);
            assert_eq!(opts.inputs[0].origin_source_lang, "ets");
            assert_eq!(opts.inputs[1].output.as_deref(), Some(b_out.as_path()));
            assert!(opts.inputs[1].is_shared_module);
            assert_eq!(opts.inputs[1].origin_source_lang, "ts");
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn filelist_parses_merge_package_names() {
    let dir = tempfile::tempdir().unwrap();
    let list_path = dir.path().join("sources.txt");
    let source = dir.path().join("entry.js");
    std::fs::write(&source, "").unwrap();
    std::fs::write(
        &list_path,
        format!(
            "{};entry|entry|1.0.0|src/main/ets/pages/Index;module;src/main/ets/pages/Index.ts;entryPkg;true;ets\n",
            source.display()
        ),
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
            assert!(opts.merge_abc);
            assert_eq!(opts.output, PathBuf::from("modules.abc"));
            assert_eq!(
                opts.inputs[0].record_name,
                "entry|entry|1.0.0|src/main/ets/pages/Index"
            );
            assert_eq!(opts.inputs[0].package_name.as_deref(), Some("entryPkg"));
            assert_eq!(opts.inputs[0].output, None);
            assert!(opts.inputs[0].is_shared_module);
            assert_eq!(opts.inputs[0].origin_source_lang, "ets");
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

#[test]
fn cache_file_bare_path_maps_to_single_input_proto_bin() {
    let cmd = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--cache-file",
        "entry.protoBin",
        "--output",
        "modules.abc",
        "entry.js",
    ])
    .unwrap();

    match cmd {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.cache_file, Some(PathBuf::from("entry.protoBin")));
            assert_eq!(
                opts.inputs[0].cache_file,
                Some(PathBuf::from("entry.protoBin"))
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn cache_file_manifest_maps_source_files_to_proto_bins() {
    let tmp = tempfile::tempdir().unwrap();
    let source_a = tmp.path().join("a.js");
    let source_b = tmp.path().join("b.js");
    fs_err::write(&source_a, "export const a = 1;").unwrap();
    fs_err::write(&source_b, "export const b = 2;").unwrap();
    let manifest = tmp.path().join("modules.cache");
    let proto_a = tmp.path().join("a.protoBin");
    let proto_b = tmp.path().join("b.protoBin");
    fs_err::write(
        &manifest,
        format!(
            "{};{}\n{};{}\n",
            source_a.display(),
            proto_a.display(),
            source_b.display(),
            proto_b.display()
        ),
    )
    .unwrap();

    let cmd = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--cache-file",
        &format!("@{}", manifest.display()),
        "--output",
        "modules.abc",
        source_a.to_str().unwrap(),
        source_b.to_str().unwrap(),
    ])
    .unwrap();

    match cmd {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.inputs[0].cache_file, Some(proto_a));
            assert_eq!(opts.inputs[1].cache_file, Some(proto_b));
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

/// Flags added for hvigor compatibility must be accepted without error.
#[test]
fn accepts_hvigor_compat_flags() {
    let result = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--extension",
        "ts",
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
    match result.unwrap() {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.input_extension, "ts");
            assert_eq!(
                opts.module_record_field_name.as_deref(),
                Some("moduleRecordField")
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

#[test]
fn rejects_invalid_extension_like_upstream() {
    let err = Ts2abcCommand::parse(["--extension", "ets", "--output", "modules.abc", "input.js"])
        .unwrap_err();
    assert!(
        matches!(err, Ts2abcError::InvalidValue { ref flag, ref value } if flag == "--extension" && value == "ets"),
        "expected InvalidValue for native --extension domain, got {err:?}"
    );
}

/// Upstream `--outputProto` changes the observable product: for a single
/// program it writes a protobuf Program snapshot and returns before ABC emit.
#[test]
fn parses_output_proto_snapshot_path() {
    let cmd = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--output-proto",
        "input.protoBin",
        "--output",
        "modules.abc",
        "input.js",
    ])
    .unwrap();

    match cmd {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.output_proto, Some(PathBuf::from("input.protoBin")));
            assert_eq!(opts.output, PathBuf::from("modules.abc"));
        }
        other => panic!("expected compile command, got {other:?}"),
    }

    let cmd = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--outputProto",
        "input.protoBin",
        "--output",
        "modules.abc",
        "input.js",
    ])
    .unwrap();

    match cmd {
        Ts2abcCommand::Compile(opts) => {
            assert_eq!(opts.output_proto, Some(PathBuf::from("input.protoBin")));
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}

/// Upstream `--compile-context-info` is parsed during option handling. The
/// content becomes active only when the ABC-input/dependency path is enabled.
#[test]
fn parses_compile_context_info_like_upstream_options() {
    let dir = tempfile::tempdir().unwrap();
    let context = dir.path().join("compileContextInfo.json");
    fs_err::write(
        &context,
        r#"{
            "compileEntries": ["&myapp/hap-file-exec&"],
            "hspPkgNames": ["lib", "@ohos/lib"],
            "updateVersionInfo": {
                "bytecodehar": {
                    "lib": "2.0.0",
                    "@ohos/lib": "2.0.0"
                }
            },
            "needModifyRecord": true,
            "bundleName": "com.inter_app.hsp"
        }"#,
    )
    .unwrap();

    let cmd = Ts2abcCommand::parse([
        "--module",
        "--merge-abc",
        "--enable-abc-input",
        "--dump-deps-info",
        "--remove-redundant-file",
        "--compile-context-info",
        context.to_str().unwrap(),
        "--output",
        "modules.abc",
        "input.js",
    ])
    .unwrap();

    match cmd {
        Ts2abcCommand::Compile(opts) => {
            assert!(opts.enable_abc_input);
            assert!(opts.dump_deps_info);
            assert!(opts.remove_redundant_file);
            assert_eq!(
                opts.compile_context_info.compile_entries,
                vec!["&myapp/hap-file-exec&"]
            );
            assert!(opts.compile_context_info.hsp_pkg_names.contains("lib"));
            assert!(opts
                .compile_context_info
                .hsp_pkg_names
                .contains("@ohos/lib"));
            assert!(opts.compile_context_info.need_modify_record);
            assert_eq!(opts.compile_context_info.bundle_name, "com.inter_app.hsp");
            assert_eq!(
                opts.compile_context_info.update_version_info["bytecodehar"]["lib"].version,
                "2.0.0"
            );
            assert_eq!(
                opts.compile_context_info.update_version_info["bytecodehar"]["lib"].package_name,
                "lib"
            );
        }
        other => panic!("expected compile command, got {other:?}"),
    }
}
