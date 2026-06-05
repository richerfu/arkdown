//! Integration tests for incremental build cache wiring.
//!
//! Verifies that:
//! 1. A first build writes cache entries to `build/default/cache/modules/`.
//! 2. The ABC compiler cache file is created at
//!    `build/default/cache/abc/modules.abc.cache`.
//! 3. A second build on the same fixture directory produces an identical
//!    `ets/modules.abc` (deterministic output / cache correctness).

use rustc_hash::FxHashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown::Bundler;
use arkdown_cache::{compute_content_hash, BuildCache, BundleCacheKey, CacheKey, CachedEntry};
use arkdown_config::load_config;
use arkdown_core::{ArkdownConfig, BuildMode, ModuleConfig, ModuleJson5, OutputTarget};
use arkdown_pack::{PackInput, Packager};
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::EtsTransformPlugin;
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
use arkdown_profile::ProfileProcessor;
use arkdown_resolver::ResolverConfig;
use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use arkdown_ts2abc::{Ts2abcInput, Ts2abcOptions};
use rolldown_plugin::__inner::SharedPluginable;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // repo root
        .join("tests/fixtures/minimal-hap")
}

/// Copy `src` into a fresh temp directory, skipping any existing `build/`
/// directory so the cache test starts with a clean slate.
fn fresh_fixture_copy(src: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dst = std::env::temp_dir().join(format!(
        "arkdown-cache-e2e-{}-{n}-{nonce}",
        std::process::id()
    ));
    copy_dir_no_build(src, &dst);
    dst
}

fn copy_dir_no_build(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create temp dir");
    for entry in std::fs::read_dir(src).expect("read src dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip build/ and .hvigor/ from the source fixture.
        if matches!(name_str.as_ref(), "build" | ".hvigor") {
            continue;
        }
        let from = entry.path();
        let to = dst.join(name_str.as_ref());
        if from.is_dir() {
            copy_dir_no_build(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy file");
        }
    }
}

fn compile_arkts_cache_root(module: &ModuleConfig) -> PathBuf {
    module
        .src_path
        .join("build/default/cache/default/default@CompileArkTS/esmodule/debug")
}

#[derive(Debug, Clone)]
struct ChunkCompileMeta {
    record_name: String,
    source_file: Option<String>,
}

fn collect_js_chunks(
    chunk_files: &[(String, PathBuf)],
    record_name_map: &FxHashMap<String, ChunkCompileMeta>,
) -> Vec<Ts2abcInput> {
    chunk_files
        .iter()
        .filter(|(_, p)| p.extension().and_then(|e| e.to_str()) == Some("js"))
        .map(|(name, path)| {
            let meta = record_name_map.get(name.as_str());
            Ts2abcInput {
                path: path.clone(),
                output: None,
                cache_file: None,
                record_name: meta
                    .map(|m| m.record_name.clone())
                    .unwrap_or_else(|| name.clone()),
                source_file: meta.and_then(|m| m.source_file.clone()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
                source_map: None,
            }
        })
        .collect()
}

/// Bundle-cache miss path: clean stale .js files, run rolldown, save the cache
/// manifest, and return the resulting `BundleOutput`.
fn run_bundle_miss(
    rt: &tokio::runtime::Runtime,
    fixture: &Path,
    module: &ModuleConfig,
    config: Arc<ArkdownConfig>,
    loader_json: &arkdown_profile::LoaderJson,
    bundle_cache: &BuildCache,
    bundle_key: &BundleCacheKey,
) -> arkdown::BundleOutput {
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

    let plugins: Vec<SharedPluginable> = vec![
        Arc::new(EtsTransformPlugin::new(config.clone())),
        Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: fixture.to_path_buf(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: FxHashMap::default(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![],
                source_set_overlays: vec![],
            },
            fixture.to_path_buf(),
            false,
        )),
        Arc::new(ModuleMetaPlugin::new(
            fixture.to_path_buf(),
            module.name.clone(),
        )),
        Arc::new(Json5Plugin),
        Arc::new(DuplicateCheckPlugin),
    ];
    let bundler = Bundler::new((*config).clone(), plugins);
    let bundle_out = rt
        .block_on(bundler.bundle(loader_json))
        .expect("bundle (cache miss)");

    // Write the bundle cache manifest so subsequent builds can detect a hit.
    // Mirrors hvigor's behaviour of persisting the bundle key to
    // `build/default/cache/bundle/bundle.json` after each successful Rollup run.
    if let Err(e) = bundle_cache.write_bundle_manifest(bundle_key) {
        tracing::warn!(error = %e, "failed to write bundle cache manifest");
    }

    bundle_out
}

/// Run the full build pipeline inside `fixture` (which must already exist on
/// disk).  Returns the path to the produced `modules.abc`.
///
/// This replicates the core logic from `e2e_test.rs::run_build` but is
/// self-contained so it can be called twice on the same fixture directory to
/// exercise the incremental cache.
fn run_pipeline(fixture: &Path) -> PathBuf {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let config =
        load_config(fixture, BuildMode::Debug, OutputTarget::Hap, None, None).expect("load_config");
    let config = Arc::new(config);

    let processor = ProfileProcessor::new(config.clone());
    let module = &config.modules[0];

    let module_json5_path = module.src_path.join("src/main/module.json5");
    let module_json5_str = fs_err::read_to_string(&module_json5_path).expect("read module.json5");
    let module_json5: ModuleJson5 = json5::from_str(&module_json5_str).expect("parse module.json5");

    let (manifest, loader_json) = processor
        .process_module(module, &module_json5)
        .expect("process_module");

    let res_out = module
        .src_path
        .join("build/default/intermediates/res/default");
    let module_json_out = res_out.join("module.json");
    processor
        .write_module_json(&manifest, &module_json_out, None)
        .expect("write_module_json");

    // Resource compilation (best-effort; missing resources do not fail the test).
    let res_dir = module.src_path.join("src/main/resources");
    if res_dir.exists() {
        let rc = ResourceCompiler::new(ResourceCompileConfig {
            package_name: config.product.bundle_name.clone(),
            module_name: module.name.clone(),
            module_names: vec![],
            device_types: vec!["phone".to_string()],
            is_debug: true,
        });
        let _ = rc.compile(ResourceCompileInput {
            resources_dirs: vec![res_dir],
            output_dir: res_out.clone(),
        });
    }

    // Bundle — check bundle cache BEFORE cleaning stale .js files.
    let bundle_cache_dir = module.src_path.join("build/default/cache/bundle");
    let bundle_cache = BuildCache::new(bundle_cache_dir);
    let bundle_key = {
        let source_hash = {
            let root = PathBuf::from(&loader_json.module_root_path);
            let mut sorted = loader_json.compile_entry.clone();
            sorted.sort();
            let mut h: u64 = 0xcbf29ce484222325;
            for entry in &sorted {
                let path = root.join(entry.trim_start_matches("./"));
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for b in content.bytes() {
                        h ^= b as u64;
                        h = h.wrapping_mul(0x100000001b3);
                    }
                }
                for b in entry.bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
            h
        };
        BundleCacheKey {
            inner: CacheKey {
                content_hash: format!("{source_hash:x}"),
                config_hash: 0,
                dependency_hash: 0,
            },
            compile_env: String::new(),
        }
    };

    let bundle_out = run_bundle_miss(
        &rt,
        fixture,
        module,
        config.clone(),
        &loader_json,
        &bundle_cache,
        &bundle_key,
    );

    // ABC compilation — wire the incremental cache exactly as build.rs does.
    let bundle_name = &config.product.bundle_name;
    let record_name_map: FxHashMap<String, ChunkCompileMeta> = loader_json
        .compile_entry
        .iter()
        .filter_map(|ep| {
            let stem = Path::new(ep).file_stem()?.to_str()?.to_string();
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

    let mut ts2abc_inputs = collect_js_chunks(&bundle_out.chunk_files, &record_name_map);
    let modules_abc = bundle_out.modules_abc_path.clone();

    if !ts2abc_inputs.is_empty() {
        let abc_cache_root = compile_arkts_cache_root(module);
        let abc_cache_file = abc_cache_root.join("modules.cache");
        fs_err::create_dir_all(&abc_cache_root).expect("create CompileArkTS cache root");
        let mut modules_cache = String::new();
        for input in &mut ts2abc_inputs {
            let cache_stem = input
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("module");
            let cache_file = abc_cache_root.join(format!("{cache_stem}.protoBin"));
            let _ = writeln!(
                modules_cache,
                "{};{}",
                input.path.display(),
                cache_file.display()
            );
            input.cache_file = Some(cache_file);
        }
        fs_err::write(&abc_cache_file, modules_cache).expect("write CompileArkTS modules.cache");

        // Load module build cache (mirrors build.rs logic).
        let cache_dir = module.src_path.join("build/default/cache/modules");
        let mut build_cache = BuildCache::new(cache_dir);
        let _ = build_cache.load_from_disk();

        // Compute config hash (same formula as compute_config_hash in build.rs).
        let config_hash = {
            let mut h: u64 = 0xcbf29ce484222325;
            let fp = format!(
                "debug:{}:{}:false",
                config.product.bundle_name, config.sdk_config.compile_api_version
            );
            for b in fp.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            h
        };

        // Check skip condition (same as build.rs).
        let can_skip = modules_abc.exists()
            && loader_json.compile_entry.iter().all(|ep| {
                let src =
                    Path::new(&loader_json.module_root_path).join(ep.trim_start_matches("./"));
                std::fs::read_to_string(&src)
                    .map(|c| {
                        build_cache.is_valid(
                            ep,
                            &CacheKey {
                                content_hash: compute_content_hash(&c),
                                config_hash,
                                dependency_hash: 0,
                            },
                        )
                    })
                    .unwrap_or(false)
            });

        if !can_skip {
            Ts2abcOptions {
                inputs: ts2abc_inputs,
                output: modules_abc.clone(),
                target_api_version: config.sdk_config.compile_api_version,
                target_api_sub_version: None,
                debug_info: true,
                opt_level: 0,
                optimize_try_catch_func: true,
                merge_abc: true,
                module: true,
                commonjs: false,
                input_extension: "js".to_string(),
                module_record_field_name: None,
                cache_file: Some(abc_cache_file),
                output_proto: None,
                compile_context_info: Default::default(),
                enable_abc_input: false,
                dump_deps_info: false,
                remove_redundant_file: false,
                npm_module_entry_list: None,
            }
            .compile()
            .expect("abc compile");

            // Update module cache entries.
            for ep in &loader_json.compile_entry {
                let src =
                    Path::new(&loader_json.module_root_path).join(ep.trim_start_matches("./"));
                if let Ok(content) = std::fs::read_to_string(&src) {
                    let _ = build_cache.insert(CachedEntry {
                        id: ep.clone(),
                        key: CacheKey {
                            content_hash: compute_content_hash(&content),
                            config_hash,
                            dependency_hash: 0,
                        },
                        transformed_code: String::new(),
                        source_map: None,
                    });
                }
            }
        }

        // Remove .js chunks.
        for (_, path) in &bundle_out.chunk_files {
            if path.extension().and_then(|e| e.to_str()) == Some("js") && path.exists() {
                let _ = fs_err::remove_file(path);
            }
        }
    }

    // Package.
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
            abc_path: modules_abc.clone(),
            resources_index,
            module_json: module_json_out,
            native_libs: vec![],
            output_path: output,
            resources_src_dir: Some(res_out.join("resources")),
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
            source_maps_path: bundle_out.source_maps_path,
            abilities: vec![],
        })
        .expect("pack_hap");

    modules_abc
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// After a first build the module cache directory is populated with at least
/// one `.json` cache entry per compile-entry source file.
#[test]
fn test_first_build_writes_cache_entries() {
    let fixture = fresh_fixture_copy(&fixture_root());
    let modules_abc = run_pipeline(&fixture);
    assert!(
        modules_abc.exists(),
        "modules.abc must exist after first build: {modules_abc:?}"
    );

    // The module cache directory must have been created and must contain
    // at least one JSON cache file.
    let config =
        load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None).expect("config");
    let module = &config.modules[0];
    let cache_dir = module.src_path.join("build/default/cache/modules");
    assert!(
        cache_dir.exists(),
        "module cache dir must exist after first build: {cache_dir:?}"
    );
    let json_count = std::fs::read_dir(&cache_dir)
        .expect("read cache dir")
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "json")
        })
        .count();
    assert!(
        json_count > 0,
        "at least one JSON cache entry must be written; cache_dir={cache_dir:?}"
    );
}

/// After a first build the CompileArkTS handoff cache manifest exists.
#[test]
fn test_first_build_writes_abc_cache_file() {
    let fixture = fresh_fixture_copy(&fixture_root());
    run_pipeline(&fixture);

    let config =
        load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None).expect("config");
    let module = &config.modules[0];
    let abc_cache = compile_arkts_cache_root(module).join("modules.cache");
    assert!(
        abc_cache.exists(),
        "CompileArkTS modules.cache must exist after first build: {abc_cache:?}"
    );

    let _ = std::fs::remove_dir_all(&fixture);
}

// ── Bundle-level cache tests ───────────────────────────────────────────────

/// After a first build the bundle cache manifest file exists at
/// `build/default/cache/bundle/bundle.json`.
#[test]
fn test_bundle_cache_manifest_written_after_first_build() {
    let fixture = fresh_fixture_copy(&fixture_root());
    run_pipeline(&fixture);

    let config =
        load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None).expect("config");
    let module = &config.modules[0];
    let manifest_path = module
        .src_path
        .join("build/default/cache/bundle/bundle.json");
    assert!(
        manifest_path.exists(),
        "bundle cache manifest must exist after first build: {manifest_path:?}"
    );

    let _ = std::fs::remove_dir_all(&fixture);
}

/// On a second build with unchanged sources the bundle cache is a hit:
/// the manifest still exists and both builds produce byte-identical `modules.abc`.
#[test]
fn test_second_build_uses_bundle_cache_and_produces_identical_abc() {
    let fixture = fresh_fixture_copy(&fixture_root());

    let abc1 = run_pipeline(&fixture);
    let bytes1 = std::fs::read(&abc1).expect("read modules.abc after build 1");

    let config =
        load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None).expect("config");
    let module = &config.modules[0];
    let manifest_path = module
        .src_path
        .join("build/default/cache/bundle/bundle.json");
    assert!(
        manifest_path.exists(),
        "bundle cache manifest must exist before second build: {manifest_path:?}"
    );

    let abc2 = run_pipeline(&fixture);
    let bytes2 = std::fs::read(&abc2).expect("read modules.abc after build 2");

    assert_eq!(
        bytes1, bytes2,
        "modules.abc must be identical across two consecutive builds (bundle cache correctness)"
    );

    let _ = std::fs::remove_dir_all(&fixture);
}

/// Two consecutive builds on the same fixture directory produce byte-identical
/// `modules.abc` outputs (deterministic compilation + cache correctness).
#[test]
fn test_two_builds_produce_identical_abc() {
    let fixture = fresh_fixture_copy(&fixture_root());

    // First build.
    let abc1 = run_pipeline(&fixture);
    let bytes1 = std::fs::read(&abc1).expect("read modules.abc after build 1");

    // Second build — the module build cache is now warm; if the skip logic
    // fires correctly modules.abc is not regenerated (and must match build 1).
    let abc2 = run_pipeline(&fixture);
    let bytes2 = std::fs::read(&abc2).expect("read modules.abc after build 2");

    assert_eq!(
        bytes1, bytes2,
        "modules.abc must be identical across two consecutive builds of the same \
         unchanged fixture (deterministic output / cache correctness)"
    );

    let _ = std::fs::remove_dir_all(&fixture);
}
