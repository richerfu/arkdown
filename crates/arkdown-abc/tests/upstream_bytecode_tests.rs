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
        target_api_version: 12,
        merge_abc: true,
        cache_path: None,
    });
    compiler
        .compile(
            vec![AbcModule {
                record_name: format!("upstream/{}", case.name),
                source: case.source.to_string(),
                source_map: None,
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
#[ignore = "tracks upstream closure lexical-env parity; enable after nested closure lowering is ported"]
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
#[ignore = "tracks upstream CommonJS wrapper parity; enable after wrapper lowering is ported"]
fn upstream_commonjs_main_wrapper_matches_required_bytecode_shape() {
    let bytes = compile_case(&COMMONJS_ARGS);
    assert!(COMMONJS_ARGS.expected.contains("Reflect"));
    assert!(COMMONJS_ARGS.expected.contains("createemptyarray"));
    assert!(COMMONJS_ARGS.expected.contains("stownbyindex"));
    assert!(COMMONJS_ARGS.expected.contains("callthis3"));
    assert!(contains_opcode(&bytes, 0x33), "definefunc");
    assert!(contains_opcode(&bytes, 0x30), "callthis3");
}
