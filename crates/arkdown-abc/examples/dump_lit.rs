use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};

fn main() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("modules.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        target_api_version: 12,
        merge_abc: true,
        module: true,
        cache_path: None,
    });
    // Simple test with registerNamedRoute object
    compiler.compile(vec![
        AbcModule { record_name: "test/index".to_string(),
            source: r#"registerNamedRoute(() => {}, "", {bundleName: "com.example.test", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap"});"#.to_string(),
            source_map: None },
    ], &out).expect("compile");
    let bytes = std::fs::read(&out).expect("read");
    let num_lit = u32::from_le_bytes(bytes[0x2C..0x30].try_into().unwrap());
    let lit_idx_off = u32::from_le_bytes(bytes[0x30..0x34].try_into().unwrap()) as usize;
    eprintln!("NUM_LIT={}", num_lit);
    for i in 0..num_lit as usize {
        let off = u32::from_le_bytes(
            bytes[lit_idx_off + i * 4..lit_idx_off + i * 4 + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let count = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        eprintln!("LIT[{i}] count={count}");
    }
}
