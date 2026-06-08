use std::env;
use std::path::PathBuf;

use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};

fn main() {
    let mut args = env::args().skip(1);
    let input = PathBuf::from(args.next().expect("input path"));
    let output = PathBuf::from(args.next().expect("output path"));
    let record_name = args.next().expect("record name");
    let source_file = args.next();

    let source = std::fs::read_to_string(&input).expect("read input");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 2,
        optimize_try_catch_func: true,
        target_api_version: 18,
        target_api_sub_version: None,
        enable_annotations: false,
        use_define_semantic: false,
        merge_abc: true,
        module: true,
        module_record_field_name: None,
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    });
    compiler
        .compile(
            vec![AbcModule {
                record_name,
                source,
                source_map: None,
                source_file,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &output,
        )
        .expect("compile");
}
