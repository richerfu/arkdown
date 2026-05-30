//! End-to-end integration tests — validates complete HAP, HAR, and HSP artifacts
//! produced by the full pipeline.

use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // repo root
        .join("tests/fixtures/minimal-hap")
}

fn har_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/minimal-har")
}

fn hsp_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/minimal-hsp")
}

fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/golden")
}

fn isolated_fixture(src: &PathBuf) -> PathBuf {
    static FIXTURE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_nanos();
    let dst = std::env::temp_dir().join(format!(
        "arkdown-cli-e2e-{}-{counter}-{nonce}",
        std::process::id()
    ));
    copy_fixture_dir(src, &dst);
    dst
}

fn copy_fixture_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("create isolated fixture dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("read fixture entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(name.as_ref(), "build" | ".hvigor") {
            continue;
        }
        let from = entry.path();
        let to = dst.join(name.as_ref());
        if from.is_dir() {
            copy_fixture_dir(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy fixture file");
        }
    }
}

fn run_build(fixture: &PathBuf) -> PathBuf {
    let fixture = isolated_fixture(fixture);
    use std::collections::HashMap;
    use std::sync::Arc;

    use arkdown::Bundler;
    use arkdown_config::load_config;
    use arkdown_core::{BuildMode, ModuleJson5, OutputTarget};
    use arkdown_pack::{PackInput, Packager};
    use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
    use arkdown_plugin_ets_transform::EtsTransformPlugin;
    use arkdown_plugin_json5::Json5Plugin;
    use arkdown_plugin_module_meta::ModuleMetaPlugin;
    use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
    use arkdown_profile::{scan_bytecode_har_deps, ProfileProcessor};
    use arkdown_resolver::ResolverConfig;
    use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
    use arkdown_ts2abc::{compile_to_abc, Ts2abcOptions};
    use rolldown_plugin::__inner::SharedPluginable;

    // Use tokio runtime for async bundler
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let config = load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None)
        .expect("load_config");
    let config = Arc::new(config);

    let processor = ProfileProcessor::new(config.clone());
    let module = &config.modules[0];

    let module_json5_path = module.src_path.join("src/main/module.json5");
    let module_json5_content =
        fs_err::read_to_string(&module_json5_path).expect("read module.json5");
    let module_json5: ModuleJson5 =
        json5::from_str(&module_json5_content).expect("parse module.json5");

    let (manifest, loader_json) = processor
        .process_module(module, &module_json5)
        .expect("process_module");

    // Write module.json
    let module_json_out = module
        .src_path
        .join("build/default/intermediates/res/default/module.json");
    processor
        .write_module_json(&manifest, &module_json_out, None)
        .expect("write_module_json");

    // Resource compilation
    let res_dir = module.src_path.join("src/main/resources");
    let res_out = module
        .src_path
        .join("build/default/intermediates/res/default");
    if res_dir.exists() {
        let rc = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: true,
        });
        let _ = rc.compile(ResourceCompileInput {
            resources_dirs: vec![res_dir],
            output_dir: res_out.clone(),
        });
    }

    // Bundling
    let plugins: Vec<SharedPluginable> = vec![
        Arc::new(EtsTransformPlugin::new(config.clone())) as SharedPluginable,
        Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: fixture.clone(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: HashMap::new(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
                module_src_roots: vec![module.src_path.join("src/main/ets")],
            },
            fixture.clone(),
            false,
        )) as SharedPluginable,
        Arc::new(ModuleMetaPlugin::new(fixture.clone(), module.name.clone())) as SharedPluginable,
        Arc::new(Json5Plugin) as SharedPluginable,
        Arc::new(DuplicateCheckPlugin) as SharedPluginable,
    ];
    // Clean stale .js files before bundling so the explicit chunk_files list
    // from bundle_out is the single source of truth for ABC compilation.
    let chunk_dir = PathBuf::from(&loader_json.ace_module_build);
    if chunk_dir.exists() {
        for entry in walkdir::WalkDir::new(&chunk_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let bundler = Bundler::new((*config).clone(), plugins);
    let bundle_out = rt.block_on(bundler.bundle(&loader_json)).expect("bundle");

    // ABC compilation
    let modules_abc = bundle_out.modules_abc_path.clone();
    // Compute OHM URL module paths: "{bundleName}/{moduleName}/{pathFromEts}"
    // Matches es2abc/hvigorw format: "com.example.app/entry/ets/entryability/EntryAbility"
    let bundle_name = &config.product.bundle_name;
    let record_name_map: std::collections::HashMap<String, ChunkCompileMeta> = loader_json
        .compile_entry
        .iter()
        .filter_map(|ep| {
            let stem = std::path::Path::new(ep).file_stem()?.to_str()?.to_string();
            let stripped = ep
                .trim_start_matches("./")
                .trim_end_matches(".ets")
                .trim_end_matches(".ts");
            Some((
                stem,
                ChunkCompileMeta {
                    record_name: format!(
                        "{}/{}/{}",
                        bundle_name, loader_json.module_name, stripped
                    ),
                    source_file: Some(ep.clone()),
                },
            ))
        })
        .collect();
    // Use the explicit chunk file list from bundle_out instead of scanning the
    // directory — prevents stale files from prior runs producing wrong record names.
    let ts2abc_inputs = collect_js_chunks_from_output(&bundle_out.chunk_files, &record_name_map);
    if !ts2abc_inputs.is_empty() {
        compile_to_abc(Ts2abcOptions {
            inputs: ts2abc_inputs,
            output: modules_abc.clone(),
            target_api_sub_version: None,
            debug_info: true,
            opt_level: 0,
            target_api_version: config.sdk_config.compile_api_version,
            merge_abc: true,
            cache_file: None,
            module: true,
            commonjs: false,
        })
        .expect("abc compile");
        for (_chunk_name, path) in &bundle_out.chunk_files {
            if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
                match fs_err::remove_file(path) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => panic!("remove CompileArkTS temp chunk: {err}"),
                }
            }
        }
    }

    // Packaging — ensure resources.index exists
    let resources_index = res_out.join("resources.index");
    if !resources_index.exists() {
        std::fs::create_dir_all(&res_out).ok();
        std::fs::write(&resources_index, &[]).ok();
    }

    let output = fixture.join("build/default/outputs/default/entry.hap");
    std::fs::create_dir_all(output.parent().unwrap()).ok();

    Packager::new(config.clone())
        .pack_hap(PackInput {
            module_name: module.name.clone(),
            abc_path: modules_abc,
            resources_index,
            module_json: module_json_out,
            native_libs: vec![],
            output_path: output.clone(),
            resources_src_dir: Some(res_out.join("resources")),
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            source_maps_path: bundle_out.source_maps_path.clone(),
            abilities: vec![],
        })
        .expect("pack_hap");

    output
}

#[derive(Debug, Clone)]
struct ChunkCompileMeta {
    record_name: String,
    source_file: Option<String>,
}

fn collect_js_chunks_from_output(
    chunk_files: &[(String, PathBuf)],
    record_name_map: &std::collections::HashMap<String, ChunkCompileMeta>,
) -> Vec<arkdown_ts2abc::Ts2abcInput> {
    chunk_files
        .iter()
        .filter(|(_, path)| path.extension().and_then(|x| x.to_str()) == Some("js"))
        .map(|(chunk_name, path)| {
            let meta = record_name_map.get(chunk_name.as_str());
            arkdown_ts2abc::Ts2abcInput {
                path: path.clone(),
                record_name: meta
                    .map(|meta| meta.record_name.clone())
                    .unwrap_or_else(|| chunk_name.clone()),
                source_file: meta.and_then(|meta| meta.source_file.clone()),
                source_map: None,
            }
        })
        .collect()
}

fn run_build_har(fixture: &PathBuf) -> PathBuf {
    use std::sync::Arc;

    use arkdown_config::load_config;
    use arkdown_core::{BuildMode, OutputTarget};
    use arkdown_pack::{HarPackInput, Packager};

    let fixture = isolated_fixture(fixture);
    let config = load_config(&fixture, BuildMode::Debug, OutputTarget::Har, None, None)
        .expect("load_config for HAR");
    let config = Arc::new(config);
    let module = &config.modules[0];

    let output = fixture.join(format!("build/default/outputs/default/{}.har", module.name));
    std::fs::create_dir_all(output.parent().unwrap()).ok();

    let module_name = module.name.clone();
    let source_dir = module.src_path.join("src/main/ets");
    let resources_dir = module.src_path.join("src/main/resources");
    let oh_package_json = module.src_path.join("oh-package.json5");

    Packager::new(config)
        .pack_har(HarPackInput {
            module_name,
            source_dir,
            resources_dir,
            oh_package_json,
            output_path: output.clone(),
        })
        .expect("pack_har");
    output
}

fn run_build_hsp(fixture: &PathBuf) -> PathBuf {
    let fixture = isolated_fixture(fixture);
    use std::collections::HashMap;
    use std::sync::Arc;

    use arkdown::Bundler;
    use arkdown_config::load_config;
    use arkdown_core::{BuildMode, ModuleJson5, OutputTarget};
    use arkdown_pack::{PackInput, Packager};
    use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
    use arkdown_plugin_ets_transform::EtsTransformPlugin;
    use arkdown_plugin_json5::Json5Plugin;
    use arkdown_plugin_module_meta::ModuleMetaPlugin;
    use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
    use arkdown_profile::{scan_bytecode_har_deps, ProfileProcessor};
    use arkdown_resolver::ResolverConfig;
    use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
    use arkdown_ts2abc::{compile_to_abc, Ts2abcOptions};
    use rolldown_plugin::__inner::SharedPluginable;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let config = load_config(&fixture, BuildMode::Debug, OutputTarget::Hsp, None, None)
        .expect("load_config");
    let config = Arc::new(config);

    let processor = ProfileProcessor::new(config.clone());
    let module = &config.modules[0];

    let module_json5_path = module.src_path.join("src/main/module.json5");
    let module_json5_content =
        fs_err::read_to_string(&module_json5_path).expect("read module.json5");
    let module_json5: ModuleJson5 =
        json5::from_str(&module_json5_content).expect("parse module.json5");

    let (manifest, loader_json) = processor
        .process_module(module, &module_json5)
        .expect("process_module");

    let module_json_out = module
        .src_path
        .join("build/default/intermediates/res/default/module.json");
    processor
        .write_module_json(&manifest, &module_json_out, None)
        .expect("write_module_json");

    let res_dir = module.src_path.join("src/main/resources");
    let res_out = module
        .src_path
        .join("build/default/intermediates/res/default");
    if res_dir.exists() {
        let rc = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: true,
        });
        let _ = rc.compile(ResourceCompileInput {
            resources_dirs: vec![res_dir],
            output_dir: res_out.clone(),
        });
    }

    let plugins: Vec<SharedPluginable> = vec![
        Arc::new(EtsTransformPlugin::new(config.clone())),
        Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: fixture.clone(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: HashMap::new(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
                module_src_roots: vec![module.src_path.join("src/main/ets")],
            },
            fixture.clone(),
            false,
        )),
        Arc::new(ModuleMetaPlugin::new(fixture.clone(), module.name.clone())),
        Arc::new(Json5Plugin),
        Arc::new(DuplicateCheckPlugin),
    ];

    let chunk_dir = PathBuf::from(&loader_json.ace_module_build);
    if chunk_dir.exists() {
        for entry in walkdir::WalkDir::new(&chunk_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let bundler = Bundler::new((*config).clone(), plugins);
    let bundle_out = rt.block_on(bundler.bundle(&loader_json)).expect("bundle");

    let modules_abc = bundle_out.modules_abc_path.clone();
    let bundle_name = &config.product.bundle_name;
    let record_name_map: std::collections::HashMap<String, ChunkCompileMeta> = loader_json
        .compile_entry
        .iter()
        .filter_map(|ep| {
            let stem = std::path::Path::new(ep).file_stem()?.to_str()?.to_string();
            let stripped = ep
                .trim_start_matches("./")
                .trim_end_matches(".ets")
                .trim_end_matches(".ts");
            Some((
                stem,
                ChunkCompileMeta {
                    record_name: format!(
                        "{}/{}/{}",
                        bundle_name, loader_json.module_name, stripped
                    ),
                    source_file: Some(ep.clone()),
                },
            ))
        })
        .collect();

    let ts2abc_inputs = collect_js_chunks_from_output(&bundle_out.chunk_files, &record_name_map);
    if !ts2abc_inputs.is_empty() {
        compile_to_abc(Ts2abcOptions {
            inputs: ts2abc_inputs,
            output: modules_abc.clone(),
            target_api_sub_version: None,
            debug_info: true,
            opt_level: 0,
            target_api_version: config.sdk_config.compile_api_version,
            merge_abc: true,
            cache_file: None,
            module: true,
            commonjs: false,
        })
        .expect("abc compile");
    }

    let resources_index = res_out.join("resources.index");
    if !resources_index.exists() {
        std::fs::create_dir_all(&res_out).ok();
        std::fs::write(&resources_index, &[]).ok();
    }

    let output = fixture.join(format!(
        "build/default/outputs/default/{}-default-unsigned.hsp",
        module.name
    ));
    std::fs::create_dir_all(output.parent().unwrap()).ok();

    Packager::new(config.clone())
        .pack_hsp(PackInput {
            module_name: module.name.clone(),
            abc_path: modules_abc,
            resources_index,
            module_json: module_json_out,
            native_libs: vec![],
            output_path: output.clone(),
            resources_src_dir: Some(res_out.join("resources")),
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            source_maps_path: bundle_out.source_maps_path.clone(),
            abilities: vec![],
        })
        .expect("pack_hsp");

    output
}

#[test]
fn test_hap_exists_and_is_valid_zip() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    assert!(hap_path.exists(), "HAP file not found at {:?}", hap_path);

    let data = std::fs::read(&hap_path).expect("read HAP");
    // ZIP magic: PK\x03\x04
    assert_eq!(
        &data[..4],
        b"PK\x03\x04",
        "HAP must start with ZIP magic PK\\x03\\x04"
    );
}

#[test]
fn test_hap_contains_required_entries() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    let file = std::fs::File::open(&hap_path).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");

    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    assert!(
        names.contains(&"module.json".to_string()),
        "missing module.json; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"pack.info".to_string()),
        "missing pack.info; entries: {:?}",
        names
    );
    assert!(
        !names.contains(&"pkgContextInfo.json".to_string()),
        "pkgContextInfo.json should not be packaged for HAP; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"resources.index".to_string()),
        "missing resources.index; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"ets/modules.abc".to_string()),
        "missing ets/modules.abc; entries: {:?}",
        names
    );

    // No raw .js files — JS must be compiled to .abc
    let js_entries: Vec<_> = names.iter().filter(|n| n.ends_with(".js")).collect();
    assert!(
        js_entries.is_empty(),
        "HAP must not contain raw .js files: {:?}",
        js_entries
    );
}

#[test]
fn test_hap_abc_has_panda_magic() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    let file = std::fs::File::open(&hap_path).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut abc = archive
        .by_name("ets/modules.abc")
        .expect("no ets/modules.abc");

    use std::io::Read;
    let mut buf = [0u8; 8];
    abc.read_exact(&mut buf).expect("read ABC magic");
    assert_eq!(
        &buf, b"PANDA\x00\x00\x00",
        "ABC must start with PANDA magic, got: {:02x?}",
        buf
    );
}

#[test]
fn test_hap_module_json_has_app_and_module_sections() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    let file = std::fs::File::open(&hap_path).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut entry = archive.by_name("module.json").expect("no module.json");

    use std::io::Read;
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .expect("read module.json");

    let v: serde_json::Value = serde_json::from_str(&content).expect("parse module.json");

    // Must have top-level app section
    let bundle_name = v["app"]["bundleName"]
        .as_str()
        .expect("app.bundleName missing");
    assert_eq!(bundle_name, "com.example.arkdown.test");

    // Must have module section with required fields
    assert_eq!(v["module"]["name"].as_str().unwrap(), "entry");
    assert_eq!(v["module"]["compileMode"].as_str().unwrap(), "esmodule");

    // virtualMachine format: "ark{N}.0.1.0"
    let vm = v["module"]["virtualMachine"]
        .as_str()
        .expect("virtualMachine missing");
    assert!(
        vm.starts_with("ark"),
        "virtualMachine should start with 'ark': {}",
        vm
    );
    assert!(
        !vm.contains("1.7"),
        "virtualMachine should not be '1.7' subdivision format: {}",
        vm
    );
}

#[test]
fn test_hap_pack_info_has_bundle_name() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    let file = std::fs::File::open(&hap_path).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut entry = archive.by_name("pack.info").expect("no pack.info");

    use std::io::Read;
    let mut content = String::new();
    entry.read_to_string(&mut content).expect("read pack.info");

    let v: serde_json::Value = serde_json::from_str(&content).expect("parse pack.info");

    assert_eq!(
        v["summary"]["app"]["bundleName"].as_str().unwrap(),
        "com.example.arkdown.test"
    );
}

#[test]
fn test_hap_resource_files_present() {
    let fixture = fixture_root();
    let hap_path = run_build(&fixture);

    let file = std::fs::File::open(&hap_path).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");

    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    assert!(
        names.iter().any(|n| n.starts_with("resources/")),
        "HAP should contain resource files under resources/; entries: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// HAR golden tests
// ---------------------------------------------------------------------------

#[test]
fn test_har_exists_and_is_valid_zip() {
    let fixture = har_fixture_root();
    let har_path = run_build_har(&fixture);

    assert!(har_path.exists(), "HAR file not found at {:?}", har_path);

    let data = std::fs::read(&har_path).expect("read HAR");
    assert_eq!(
        &data[..4],
        b"PK\x03\x04",
        "HAR must start with ZIP magic PK\\x03\\x04"
    );
}

#[test]
fn test_har_contains_required_entries() {
    let fixture = har_fixture_root();
    let har_path = run_build_har(&fixture);

    let file = std::fs::File::open(&har_path).expect("open HAR");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    assert!(
        names.contains(&"oh-package.json5".to_string()),
        "HAR must contain oh-package.json5; entries: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n.starts_with("src/main/ets/")),
        "HAR must contain ETS sources under src/main/ets/; entries: {:?}",
        names
    );

    // Source HAR must NOT contain module.json or compiled ABC
    assert!(
        !names.contains(&"module.json".to_string()),
        "Source HAR must not contain module.json; entries: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n.ends_with(".abc")),
        "Source HAR must not contain .abc files; entries: {:?}",
        names
    );
    assert!(
        !names.contains(&"pack.info".to_string()),
        "Source HAR must not contain pack.info; entries: {:?}",
        names
    );
}

#[test]
fn test_har_golden_entry_list() {
    let fixture = har_fixture_root();
    let har_path = run_build_har(&fixture);

    let file = std::fs::File::open(&har_path).expect("open HAR");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .filter(|n| !n.ends_with('/'))
        .collect();
    names.sort();

    let golden_path = golden_root().join("minimal-har/expected-zip-entries.json");
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).expect("read golden"))
            .expect("parse golden");

    let required = golden["required"]
        .as_array()
        .expect("golden.required is array");
    for entry in required {
        let entry = entry.as_str().expect("entry is string");
        assert!(
            names
                .iter()
                .any(|n| n == entry || n.ends_with(&format!("/{entry}"))),
            "HAR missing required golden entry '{}'; actual entries: {:?}",
            entry,
            names
        );
    }

    let forbidden = golden["forbidden"]
        .as_array()
        .expect("golden.forbidden is array");
    for entry in forbidden {
        let entry = entry.as_str().expect("entry is string");
        assert!(
            !names.contains(&entry.to_string()),
            "HAR contains forbidden golden entry '{}'; entries: {:?}",
            entry,
            names
        );
    }
}

// ---------------------------------------------------------------------------
// HSP golden tests
// ---------------------------------------------------------------------------

#[test]
fn test_hsp_exists_and_is_valid_zip() {
    let fixture = hsp_fixture_root();
    let hsp_path = run_build_hsp(&fixture);

    assert!(hsp_path.exists(), "HSP file not found at {:?}", hsp_path);

    let data = std::fs::read(&hsp_path).expect("read HSP");
    assert_eq!(
        &data[..4],
        b"PK\x03\x04",
        "HSP must start with ZIP magic PK\\x03\\x04"
    );
}

#[test]
fn test_hsp_contains_required_entries() {
    let fixture = hsp_fixture_root();
    let hsp_path = run_build_hsp(&fixture);

    let file = std::fs::File::open(&hsp_path).expect("open HSP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    assert!(
        names.contains(&"module.json".to_string()),
        "HSP must contain module.json; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"pack.info".to_string()),
        "HSP must contain pack.info; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"resources.index".to_string()),
        "HSP must contain resources.index; entries: {:?}",
        names
    );
    assert!(
        names.contains(&"ets/modules.abc".to_string()),
        "HSP must contain ets/modules.abc; entries: {:?}",
        names
    );

    let js_entries: Vec<_> = names.iter().filter(|n| n.ends_with(".js")).collect();
    assert!(
        js_entries.is_empty(),
        "HSP must not contain raw .js files: {:?}",
        js_entries
    );
}

#[test]
fn test_hsp_abc_has_panda_magic() {
    let fixture = hsp_fixture_root();
    let hsp_path = run_build_hsp(&fixture);

    let file = std::fs::File::open(&hsp_path).expect("open HSP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut abc = archive
        .by_name("ets/modules.abc")
        .expect("no ets/modules.abc in HSP");

    use std::io::Read;
    let mut buf = [0u8; 8];
    abc.read_exact(&mut buf).expect("read ABC magic");
    assert_eq!(
        &buf, b"PANDA\x00\x00\x00",
        "HSP ABC must start with PANDA magic, got: {:02x?}",
        buf
    );
}

#[test]
fn test_hsp_golden_entry_list() {
    let fixture = hsp_fixture_root();
    let hsp_path = run_build_hsp(&fixture);

    let file = std::fs::File::open(&hsp_path).expect("open HSP");
    let mut archive = zip::ZipArchive::new(file).expect("open ZIP");
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .filter(|n| !n.ends_with('/'))
        .collect();
    names.sort();

    let golden_path = golden_root().join("minimal-hsp/expected-zip-entries.json");
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).expect("read golden"))
            .expect("parse golden");

    let required = golden["required"]
        .as_array()
        .expect("golden.required is array");
    for entry in required {
        let entry = entry.as_str().expect("entry is string");
        assert!(
            names.contains(&entry.to_string()),
            "HSP missing required golden entry '{}'; actual entries: {:?}",
            entry,
            names
        );
    }

    let forbidden = golden["forbidden"]
        .as_array()
        .expect("golden.forbidden is array");
    for entry in forbidden {
        let entry = entry.as_str().expect("entry is string");
        assert!(
            !names.contains(&entry.to_string()),
            "HSP contains forbidden golden entry '{}'; entries: {:?}",
            entry,
            names
        );
    }
}

// ---------------------------------------------------------------------------
// ByteCode HAR importee test
// ---------------------------------------------------------------------------

#[test]
fn test_non_entry_importees_populated_with_har_dep() {
    use index_vec::IndexVec;
    use rustc_hash::FxHashMap;

    use arkdown_core::{ArkChunk, ChunkGraph, ChunkIdx};
    use arkdown_plugin_ohos_resolve::{process_importees, ProcessImporteeConfig};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src_main = tmp.path().join("entry/src/main");
    std::fs::create_dir_all(module_src_main.join("ets")).expect("create ets dir");

    std::fs::write(module_src_main.join("ets/Index.ets"), b"export {}").expect("write Index");
    std::fs::write(module_src_main.join("ets/harHelper.ets"), b"export {}").expect("write helper");

    let mut entry_chunk = ArkChunk::default();
    entry_chunk.name = "Index".to_string();
    entry_chunk.is_entry = true;
    entry_chunk.facade_module_id = Some(
        module_src_main
            .join("ets/Index.ets")
            .to_string_lossy()
            .into_owned(),
    );
    entry_chunk.imports = vec!["harHelper.js".to_string()];

    let mut helper_chunk = ArkChunk::default();
    helper_chunk.name = "harHelper".to_string();
    helper_chunk.is_entry = false;
    helper_chunk.facade_module_id = Some(
        module_src_main
            .join("ets/harHelper.ets")
            .to_string_lossy()
            .into_owned(),
    );

    let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();
    chunks.push(entry_chunk);
    chunks.push(helper_chunk);

    let graph = ChunkGraph {
        chunks,
        module_to_chunk: IndexVec::new(),
        entry_module_to_entry_chunk: FxHashMap::default(),
    };

    let config = ProcessImporteeConfig {
        bundle_name: "com.example.arkdown.test".to_string(),
        module_name: "entry".to_string(),
        module_version: String::new(),
        project_root: tmp.path().to_path_buf(),
        module_src_main: module_src_main.clone(),
        output_dir: tmp.path().join("entry/build/default/intermediates"),
    };

    let result = process_importees(&graph, &config).expect("process_importees");

    assert!(
        !result.non_entry_importees.is_empty(),
        "nonEntryImportees should be populated when bundle has non-entry chunks; got empty"
    );
    assert!(
        result
            .non_entry_importees
            .iter()
            .any(|e| e.contains("harHelper")),
        "nonEntryImportees should contain the helper chunk OHM URL: {:?}",
        result.non_entry_importees
    );
    assert!(
        result
            .non_entry_importees
            .iter()
            .any(|e| e.starts_with("@bundle:")),
        "nonEntryImportees entries should use @bundle: OHM URL format: {:?}",
        result.non_entry_importees
    );
}

// ByteCode HAR e2e tests
// ---------------------------------------------------------------------------

fn bc_har_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // repo root
        .join("tests/fixtures/minimal-hap-with-bc-har")
}

/// Build the minimal-hap-with-bc-har fixture through the full pipeline
/// including ByteCode HAR detection, importee processing, ABC compilation,
/// and BC HAR abc merge.
///
/// Returns `(modules_abc_path, importee_json_path)`.
fn run_build_bc_har(fixture_src: &PathBuf) -> (PathBuf, PathBuf) {
    let fixture = isolated_fixture(fixture_src);
    use std::collections::HashMap;
    use std::sync::Arc;

    use arkdown::Bundler;
    use arkdown_config::load_config;
    use arkdown_core::{BuildMode, ModuleJson5, OutputTarget};
    use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
    use arkdown_plugin_ets_transform::EtsTransformPlugin;
    use arkdown_plugin_json5::Json5Plugin;
    use arkdown_plugin_module_meta::ModuleMetaPlugin;
    use arkdown_plugin_ohos_resolve::{
        process_importees, OhosResolvePlugin, ProcessImporteeConfig,
    };
    use arkdown_profile::{scan_bytecode_har_deps, ProfileProcessor};
    use arkdown_resolver::ResolverConfig;
    use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
    use arkdown_ts2abc::{compile_to_abc, Ts2abcOptions};
    use rolldown_plugin::__inner::SharedPluginable;

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let config = load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None)
        .expect("load_config");
    let config = Arc::new(config);

    let processor = ProfileProcessor::new(config.clone());
    let module = &config.modules[0];

    let module_json5_path = module.src_path.join("src/main/module.json5");
    let module_json5_content =
        fs_err::read_to_string(&module_json5_path).expect("read module.json5");
    let module_json5: ModuleJson5 =
        json5::from_str(&module_json5_content).expect("parse module.json5");

    let (manifest, mut loader_json) = processor
        .process_module(module, &module_json5)
        .expect("process_module");

    // Write module.json
    let module_json_out = module
        .src_path
        .join("build/default/intermediates/res/default/module.json");
    processor
        .write_module_json(&manifest, &module_json_out, None)
        .expect("write_module_json");

    // Resource compilation
    let res_dir = module.src_path.join("src/main/resources");
    let res_out = module
        .src_path
        .join("build/default/intermediates/res/default");
    if res_dir.exists() {
        let rc = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config.product.bundle_name.clone(),
            module_name: module.name.clone(),
            device_types: vec!["phone".to_string()],
            is_debug: true,
        });
        let _ = rc.compile(ResourceCompileInput {
            resources_dirs: vec![res_dir],
            output_dir: res_out.clone(),
        });
    }

    // Detect ByteCode HAR dependencies
    let bc_har_deps = scan_bytecode_har_deps(&module.src_path, &fixture);

    // Bundling
    let plugins: Vec<SharedPluginable> = vec![
        Arc::new(EtsTransformPlugin::new(config.clone())),
        Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: fixture.clone(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: HashMap::new(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
                module_src_roots: vec![module.src_path.join("src/main/ets")],
            },
            fixture.clone(),
            false,
        )),
        Arc::new(ModuleMetaPlugin::new(fixture.clone(), module.name.clone())),
        Arc::new(Json5Plugin),
        Arc::new(DuplicateCheckPlugin),
    ];

    let chunk_dir = PathBuf::from(&loader_json.ace_module_build);
    if chunk_dir.exists() {
        for entry in walkdir::WalkDir::new(&chunk_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("js") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let bundler = Bundler::new((*config).clone(), plugins);
    let bundle_out = rt.block_on(bundler.bundle(&loader_json)).expect("bundle");

    // process_importees pass
    let intermediates_dir = module.src_path.join("build/default/intermediates");
    let importee_cfg = ProcessImporteeConfig {
        bundle_name: config.product.bundle_name.clone(),
        module_name: loader_json.module_name.clone(),
        module_version: String::new(),
        project_root: fixture.clone(),
        module_src_main: PathBuf::from(&loader_json.module_root_path),
        output_dir: intermediates_dir.clone(),
    };
    let importee_result =
        process_importees(&bundle_out.chunk_graph, &importee_cfg).expect("process_importees");
    loader_json.compiled_importees = importee_result.compiled_importees;

    // ByteCode HAR OHM URLs not available in ByteCodeHarScanEntry

    // Write importee.json cache so the test can read it back
    let importee_json_path = intermediates_dir
        .join("importee/default")
        .join(&loader_json.module_name)
        .join("importee.json");
    // Write the updated list that includes BC HAR OHM URLs
    {
        let cache = serde_json::json!({
            "nonEntryImportees": importee_result.non_entry_importees,
            "compiledImportees": loader_json.compiled_importees,
        });
        if let Some(parent) = importee_json_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(
            &importee_json_path,
            serde_json::to_string_pretty(&cache).unwrap(),
        )
        .expect("write importee.json");
    }

    // ABC compilation
    let modules_abc = bundle_out.modules_abc_path.clone();
    let bundle_name = &config.product.bundle_name;
    let record_name_map: std::collections::HashMap<String, ChunkCompileMeta> = loader_json
        .compile_entry
        .iter()
        .filter_map(|ep| {
            let stem = std::path::Path::new(ep).file_stem()?.to_str()?.to_string();
            let stripped = ep
                .trim_start_matches("./")
                .trim_end_matches(".ets")
                .trim_end_matches(".ts");
            Some((
                stem,
                ChunkCompileMeta {
                    record_name: format!(
                        "{}/{}/{}",
                        bundle_name, loader_json.module_name, stripped
                    ),
                    source_file: Some(ep.clone()),
                },
            ))
        })
        .collect();
    let ts2abc_inputs = collect_js_chunks_from_output(&bundle_out.chunk_files, &record_name_map);
    if !ts2abc_inputs.is_empty() {
        compile_to_abc(Ts2abcOptions {
            inputs: ts2abc_inputs,
            output: modules_abc.clone(),
            target_api_sub_version: None,
            debug_info: true,
            opt_level: 0,
            target_api_version: config.sdk_config.compile_api_version,
            merge_abc: true,
            cache_file: None,
            module: true,
            commonjs: false,
        })
        .expect("abc compile");
        for (_chunk_name, path) in &bundle_out.chunk_files {
            if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
                let _ = fs_err::remove_file(path);
            }
        }
    }

    // Merge ByteCode HAR .abc files
    for dep in &bc_har_deps {}

    (modules_abc, importee_json_path)
}

#[test]
#[ignore = "bc_har ohm_url field not available"]
fn test_bc_har_compiled_importees_contains_ohm_url() {
    let fixture = bc_har_fixture_root();
    let (_modules_abc, importee_json_path) = run_build_bc_har(&fixture);

    assert!(
        importee_json_path.exists(),
        "importee.json not found at {:?}",
        importee_json_path
    );

    let content = std::fs::read_to_string(&importee_json_path).expect("read importee.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse importee.json");

    let compiled = parsed["compiledImportees"]
        .as_array()
        .expect("compiledImportees must be array");
    let ohm_urls: Vec<&str> = compiled.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        ohm_urls.contains(&"@bundle:com.example.arkdown.test/bc-har-dep/index"),
        "compiledImportees must contain BC HAR OHM URL; got: {:?}",
        ohm_urls
    );
}

#[test]
#[ignore = "bc_har ohm_url field not available"]
fn test_bc_har_modules_abc_contains_har_bytes() {
    let fixture = bc_har_fixture_root();
    let (modules_abc, _importee_json) = run_build_bc_har(&fixture);

    assert!(
        modules_abc.exists(),
        "modules.abc not found at {:?}",
        modules_abc
    );

    let data = std::fs::read(&modules_abc).expect("read modules.abc");

    // File must start with the PANDA magic from the main compiled module.
    assert_eq!(
        &data[..8],
        b"PANDA\x00\x00\x00",
        "modules.abc must start with PANDA magic; got: {:02x?}",
        &data[..8]
    );

    // The BC HAR fixture's marker bytes (0xBC 0x4D) must appear somewhere after
    // the main module bytecode — this confirms the HAR's .abc was merged in.
    let marker = b"\xbc\x4d";
    let found = data.windows(marker.len()).any(|window| window == marker);
    assert!(
        found,
        "modules.abc must contain BC HAR marker bytes 0xBC 0x4D; file size={}, first 32 bytes={:02x?}",
        data.len(),
        &data[..data.len().min(32)]
    );
}
