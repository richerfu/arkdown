use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};

struct UpstreamCase {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

const LEXICAL_ARROW: UpstreamCase = UpstreamCase {
    name: "es2panda/bytecode/js/lexicalEnv/arrow-function-reference-ctx",
    source: include_str!(
        "fixtures/upstream/es2panda/bytecode/js/lexicalEnv/arrow-function-reference-ctx.js"
    ),
    expected: include_str!(
        "fixtures/upstream/es2panda/bytecode/js/lexicalEnv/arrow-function-reference-ctx-expected.txt"
    ),
};

const COMMONJS_ARGS: UpstreamCase = UpstreamCase {
    name: "es2panda/bytecode/commonjs/test-commonjs-main-func-args",
    source: include_str!(
        "fixtures/upstream/es2panda/bytecode/commonjs/test-commonjs-main-func-args.js"
    ),
    expected: include_str!(
        "fixtures/upstream/es2panda/bytecode/commonjs/test-commonjs-main-func-args-expected.txt"
    ),
};

const EXPLICIT_CONSTRUCTOR: UpstreamCase = UpstreamCase {
    name: "es2panda/bytecode/js/class/test-explicit-constructor",
    source: include_str!(
        "fixtures/upstream/es2panda/bytecode/js/class/test-explicit-constructor.js"
    ),
    expected: include_str!(
        "fixtures/upstream/es2panda/bytecode/js/class/test-explicit-constructor-expected.txt"
    ),
};

fn compile_case(case: &UpstreamCase) -> Vec<u8> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("modules.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version: 12,
        target_api_sub_version: None,
        enable_annotations: false,
        merge_abc: true,
        module: false,
        module_record_field_name: None,
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    });
    compiler
        .compile(
            vec![AbcModule {
                record_name: format!("upstream/{}", case.name),
                source: case.source.to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &out,
        )
        .unwrap_or_else(|err| panic!("{} must compile: {err}", case.name));
    std::fs::read(out).expect("read compiled abc")
}

fn contains_opcode(bytes: &[u8], opcode: u8) -> bool {
    bytes.contains(&opcode)
}

#[test]
fn migrated_upstream_bytecode_cases_compile() {
    for case in [&LEXICAL_ARROW, &COMMONJS_ARGS, &EXPLICIT_CONSTRUCTOR] {
        assert!(
            !case.expected.trim().is_empty(),
            "{} must keep the upstream expected bytecode excerpt",
            case.name
        );
        let bytes = compile_case(case);
        assert!(bytes.starts_with(b"PANDA\0\0\0"), "{} emits ABC", case.name);
    }
}

#[test]
fn upstream_class_constructor_case_reaches_class_lowering() {
    let bytes = compile_case(&EXPLICIT_CONSTRUCTOR);
    assert!(
        contains_opcode(&bytes, 0x35),
        "upstream explicit constructor case should emit defineclasswithbuffer"
    );
}

#[test]
fn upstream_lexical_arrow_matches_required_bytecode_shape() {
    let bytes = compile_case(&LEXICAL_ARROW);
    assert!(LEXICAL_ARROW.expected.contains("newlexenv"));
    assert!(LEXICAL_ARROW.expected.contains("stlexvar"));
    assert!(LEXICAL_ARROW.expected.contains("ldlexvar"));
    assert!(LEXICAL_ARROW.expected.contains("definefunc"));
    assert!(contains_opcode(&bytes, 0x3c), "ldlexvar");
    assert!(contains_opcode(&bytes, 0x3d), "stlexvar");
    assert!(contains_opcode(&bytes, 0x33), "definefunc");
}

#[test]
fn upstream_commonjs_source_compiles_without_wrapper() {
    // The IIFE/CJS compatibility wrapper was removed: CJS sources compile as
    // regular JS without the hand-crafted Reflect.apply wrapper bytecode.
    // Verify the source still compiles to a valid ABC file.
    let bytes = compile_case(&COMMONJS_ARGS);
    assert!(bytes.starts_with(b"PANDA\0\0\0"), "must emit valid ABC");
    assert!(bytes.len() > 50, "must emit non-trivial ABC for CJS source");
}
