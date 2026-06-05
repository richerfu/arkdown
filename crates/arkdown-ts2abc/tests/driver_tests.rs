use arkdown_abc::cache::{
    abc_input_cache_hash, source_cache_hash, AbcProgramsCacheFile, ProgramCacheFile,
    ProgramProtoFile,
};
use arkdown_abc::AbcReader;
use arkdown_ts2abc::{CompileContextInfo, Ts2abcError, Ts2abcInput, Ts2abcOptions};

#[test]
fn compile_to_abc_uses_rust_backend_and_writes_panda_file() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "(function () { var x = 1; return x; })();").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry|entry|1.0.0|src/main/ets/pages/Index".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, output);
    assert_eq!(result.compiled_records.len(), 1);
    let bytes = fs_err::read(result.abc_path).unwrap();
    assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
}

#[test]
fn compile_to_abc_writes_upstream_program_cache_proto_bin() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let output = tmp.path().join("modules.abc");
    let cache_file = tmp.path().join("Index.protoBin");
    let record_name = "entry|entry|1.0.0|src/main/ets/pages/Index";
    let source_file = "entry/src/main/ets/pages/Index.ts";
    let package_name = "entry";
    let source_code = "export const value = 1;";
    fs_err::write(&source, source_code).unwrap();

    Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source.clone(),
            output: None,
            cache_file: Some(cache_file.clone()),
            record_name: record_name.to_string(),
            source_file: Some(source_file.to_string()),
            package_name: Some(package_name.to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output,
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: Some(cache_file.clone()),
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let entry = ProgramCacheFile::read(&cache_file).unwrap().unwrap();
    assert_eq!(
        entry.hash_code,
        source_cache_hash(
            source_code,
            &source.to_string_lossy(),
            record_name,
            source_file,
            package_name
        )
    );
    assert_eq!(entry.program.record_table.len(), 3);
    assert!(entry
        .program
        .function_table
        .iter()
        .any(|function| function.key == b"entry|entry|1.0.0|src/main/ets/pages/Index.func_main_0"));
}

#[test]
fn compile_to_abc_writes_output_proto_and_returns_before_abc_emit() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let output = tmp.path().join("modules.abc");
    let output_proto = tmp.path().join("Index.protoBin");
    fs_err::write(&source, "export const value = 1;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry|entry|1.0.0|src/main/ets/pages/Index".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: Some(output_proto.clone()),
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.proto_path.as_ref(), Some(&output_proto));
    assert_eq!(result.abc_paths, Vec::<std::path::PathBuf>::new());
    assert!(
        !output.exists(),
        "--outputProto returns before ABC emission"
    );
    let program = ProgramProtoFile::read(&output_proto)
        .unwrap()
        .expect("output proto is a protoPanda Program");
    assert_eq!(program.record_table.len(), 3);
    assert!(program
        .function_table
        .iter()
        .any(|function| function.key == b"entry|entry|1.0.0|src/main/ets/pages/Index.func_main_0"));
}

#[test]
fn compile_to_abc_accepts_inactive_compile_context_for_source_input() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: CompileContextInfo {
            compile_entries: vec!["&myapp/hap-file-exec&".to_string()],
            ..Default::default()
        },
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, output);
    assert!(fs_err::read(result.abc_path)
        .unwrap()
        .starts_with(b"PANDA\0\0\0"));
}

#[test]
fn compile_to_abc_active_compile_context_prunes_redundant_records() {
    let tmp = tempfile::tempdir().unwrap();
    let main = tmp.path().join("main.js");
    let dep = tmp.path().join("dep.js");
    let unused = tmp.path().join("unused.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(
        &main,
        r#"import { value } from "@normalized:N&entry&bundle&pkg/dep&1.0.0"; export const result = value;"#,
    )
    .unwrap();
    fs_err::write(&dep, "export const value = 1;").unwrap();
    fs_err::write(&unused, "export const unused = 2;").unwrap();

    let input = |path: std::path::PathBuf, record_name: &str| Ts2abcInput {
        path,
        output: None,
        cache_file: None,
        record_name: record_name.to_string(),
        source_file: Some(format!("{record_name}.ts")),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        source_map: None,
    };

    let result = Ts2abcOptions {
        inputs: vec![
            input(main, "bundle&pkg/main&1.0.0"),
            input(dep, "bundle&pkg/dep&1.0.0"),
            input(unused, "bundle&pkg/unused&1.0.0"),
        ],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: CompileContextInfo {
            compile_entries: vec!["bundle&pkg/main&1.0.0".to_string()],
            ..Default::default()
        },
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: true,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, output);
    let bytes = fs_err::read(result.abc_path).unwrap();
    let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
    assert!(
        class_names
            .iter()
            .any(|name| name == "Lbundle&pkg/main&1.0.0;"),
        "expected main record, got {class_names:?}"
    );
    assert!(
        class_names
            .iter()
            .any(|name| name == "Lbundle&pkg/dep&1.0.0;"),
        "expected dep record, got {class_names:?}"
    );
    assert!(
        !class_names
            .iter()
            .any(|name| name == "Lbundle&pkg/unused&1.0.0;"),
        "removeRedundantFile must drop records outside resolved deps, got {class_names:?}"
    );
}

#[test]
fn compile_to_abc_reemits_abc_input_with_abc2program_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let input_abc = tmp.path().join("input.abc");
    let output = tmp.path().join("roundtrip.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source.clone(),
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: input_abc.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: input_abc,
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: None,
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, output);
    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
    assert!(
        class_names.iter().any(|name| name == "Lentry/main;"),
        "roundtrip output must keep the input record, got {class_names:?}"
    );
}

#[test]
fn compile_to_abc_writes_output_proto_for_abc_input() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let input_abc = tmp.path().join("input.abc");
    let output_proto = tmp.path().join("input.protoBin");
    fs_err::write(&source, "export const value = 1;").unwrap();

    Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source.clone(),
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: input_abc.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: input_abc,
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: None,
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: tmp.path().join("unused.abc"),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: Some(output_proto.clone()),
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.proto_path.as_ref(), Some(&output_proto));
    assert_eq!(result.abc_paths, Vec::<std::path::PathBuf>::new());
    let program = ProgramProtoFile::read(&output_proto)
        .unwrap()
        .expect("ABC input outputProto must be a protoPanda Program");
    assert_eq!(program.record_table.len(), 3);
    assert!(program
        .function_table
        .iter()
        .any(|function| function.key == b"entry/main.func_main_0"));
    assert!(
        !program.literal_array_table.is_empty(),
        "ABC input Program must retain module literal arrays"
    );
}

#[test]
fn compile_to_abc_writes_and_reads_abc_input_programs_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Index.js");
    let input_abc = tmp.path().join("input.abc");
    let cache_file = tmp.path().join("input.protoBin");
    let output = tmp.path().join("roundtrip.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source.clone(),
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: input_abc.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: input_abc.clone(),
            output: None,
            cache_file: Some(cache_file.clone()),
            record_name: "entry/main".to_string(),
            source_file: None,
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let cache_entry = AbcProgramsCacheFile::read(&cache_file)
        .unwrap()
        .expect("ABC input cache must use AbcProtoProgramsCache");
    assert_eq!(cache_entry.programs.len(), 1);
    assert!(cache_entry
        .programs
        .iter()
        .any(|(name, _)| name.contains(&input_abc.to_string_lossy().into_owned())));

    let corrupt_abc = tmp.path().join("corrupt.abc");
    let corrupt_cache = tmp.path().join("corrupt.protoBin");
    let corrupt_output = tmp.path().join("corrupt-hit.abc");
    let corrupt_bytes = b"PANDA\0\0\0not a complete abc file";
    fs_err::write(&corrupt_abc, corrupt_bytes).unwrap();
    let corrupt_hash = abc_input_cache_hash(
        corrupt_bytes,
        &corrupt_abc.to_string_lossy(),
        "entry/main",
        "",
        "entry",
    );
    AbcProgramsCacheFile::write(&corrupt_cache, corrupt_hash, cache_entry.programs).unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: corrupt_abc,
            output: None,
            cache_file: Some(corrupt_cache),
            record_name: "entry/main".to_string(),
            source_file: None,
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: corrupt_output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, corrupt_output);
    AbcReader::validate(&fs_err::read(result.abc_path).unwrap()).unwrap();
}

#[test]
fn compile_to_abc_extension_abc_applies_only_when_file_extension_is_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    let unknown = tmp.path().join("input.bytecode");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&unknown, b"PANDA\0\0\0").unwrap();

    let err = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: unknown,
            output: None,
            cache_file: None,
            record_name: String::new(),
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "abc".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap_err();

    assert!(matches!(err, Ts2abcError::Abc(_)));

    let source = tmp.path().join("input.js");
    fs_err::write(&source, "export const value = 1;").unwrap();
    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry/main".to_string(),
            source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
            package_name: Some("entry".to_string()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: "ets".to_string(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "abc".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: true,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();
    assert_eq!(result.abc_path, output);
}

#[test]
fn compile_to_abc_accepts_module_source_without_iife_wrapper() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("entry.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry/main".into(),
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
}

#[test]
fn compile_to_abc_accepts_non_merge_single_source() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("entry.js");
    let output = tmp.path().join("entry.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: String::new(),
            source_file: Some("entry.js".to_string()),
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: false,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_path, output);
    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
}

#[test]
fn compile_to_abc_writes_non_merge_output_files() {
    let tmp = tempfile::tempdir().unwrap();
    let source_a = tmp.path().join("a.js");
    let source_b = tmp.path().join("b.js");
    let output_a = tmp.path().join("a.abc");
    let output_b = tmp.path().join("b.abc");
    let global_output = tmp.path().join("unused.abc");
    fs_err::write(&source_a, "export const a = 1;").unwrap();
    fs_err::write(&source_b, "export const b = 2;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![
            Ts2abcInput {
                path: source_a,
                output: Some(output_a.clone()),
                cache_file: None,
                record_name: String::new(),
                source_file: Some("a.js".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
                source_map: None,
            },
            Ts2abcInput {
                path: source_b,
                output: Some(output_b.clone()),
                cache_file: None,
                record_name: String::new(),
                source_file: Some("b.js".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
                source_map: None,
            },
        ],
        output: global_output.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: false,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    assert_eq!(result.abc_paths, vec![output_a.clone(), output_b.clone()]);
    assert_eq!(result.abc_path, output_a);
    assert!(!global_output.exists());
    assert!(fs_err::read(output_b).unwrap().starts_with(b"PANDA\0\0\0"));
}

#[test]
fn compile_to_abc_accepts_release_opt_level_two() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("release.js");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "export const value = 2;").unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry/release".into(),
            source_file: Some("entry/src/main/ets/pages/Release.ts".to_string()),
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 17,
        target_api_sub_version: Some("Release".to_string()),
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();

    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
}

#[test]
fn compile_to_abc_lowers_npm_module_entry_list_records() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("entry.js");
    let entries = tmp.path().join("npmEntries.txt");
    let output = tmp.path().join("modules.abc");
    fs_err::write(&source, "export const value = 1;").unwrap();
    fs_err::write(
        &entries,
        "@normalized:N&entry/src/main/ets/pages/Index&:/pkg_modules/a/index\n",
    )
    .unwrap();

    let result = Ts2abcOptions {
        inputs: vec![Ts2abcInput {
            path: source,
            output: None,
            cache_file: None,
            record_name: "entry/main".into(),
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            source_map: None,
        }],
        output: output.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: Some(entries),
    }
    .compile()
    .unwrap();

    let bytes = fs_err::read(result.abc_path).unwrap();
    assert!(bytes
        .windows(b"@normalized".len())
        .any(|window| window == b"@normalized"));
    assert!(bytes
        .windows(b"N&entry/src/main/ets/pages/Index&:/pkg_modules/a/index".len())
        .any(|window| window == b"N&entry/src/main/ets/pages/Index&:/pkg_modules/a/index"));
}

#[test]
#[ignore = "requires fixture JS files to be generated by integration test first"]
fn check_minimal_hap_num_lit() {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/minimal-hap/entry/build/default/intermediates/loader_out/default/ets"
    );
    let ea_js = std::path::PathBuf::from(base).join("EntryAbility.js");
    let idx_js = std::path::PathBuf::from(base).join("Index.js");
    if !ea_js.exists() || !idx_js.exists() {
        eprintln!("JS files not found at {base}; run integration test first");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("modules.abc");
    Ts2abcOptions {
        inputs: vec![
            Ts2abcInput {
                path: ea_js,
                output: None,
                cache_file: None,
                record_name: "com.example.arkdown.test/entry/ets/entryability/EntryAbility"
                    .to_string(),
                source_file: Some("entry/src/main/ets/entryability/EntryAbility.ts".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
                source_map: None,
            },
            Ts2abcInput {
                path: idx_js,
                output: None,
                cache_file: None,
                record_name: "com.example.arkdown.test/entry/ets/pages/Index".to_string(),
                source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
                source_map: None,
            },
        ],
        output: out.clone(),
        target_api_version: 12,
        target_api_sub_version: None,
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        merge_abc: true,
        module: true,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: None,
        output_proto: None,
        compile_context_info: Default::default(),
        enable_abc_input: false,
        dump_deps_info: false,
        remove_redundant_file: false,
        npm_module_entry_list: None,
    }
    .compile()
    .unwrap();
    let data = std::fs::read(&out).unwrap();
    let num_lit = u32::from_le_bytes(data[0x2C..0x30].try_into().unwrap());
    let lit_idx = u32::from_le_bytes(data[0x30..0x34].try_into().unwrap()) as usize;
    eprintln!("num_lit={num_lit} size={}", data.len());
    let mut counts = vec![];
    for i in 0..num_lit as usize {
        let off = u32::from_le_bytes(
            data[lit_idx + i * 4..lit_idx + i * 4 + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        counts.push(count);
        eprintln!("  LIT[{i}] count={count}");
    }
    assert_eq!(
        num_lit, 10,
        "expected 10 literal arrays (target: {}, got {})",
        10, num_lit
    );
}
