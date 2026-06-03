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
        target_api_version: 18,
        target_api_sub_version: None,
        merge_abc: true,
        module: true,
        cache_path: None,
        npm_entries: None,
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
            }],
            &output,
        )
        .expect("compile");
}
