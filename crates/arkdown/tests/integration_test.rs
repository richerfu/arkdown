use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arkdown::Bundler;
use arkdown_config::{load_config, ConfigError};
use arkdown_core::{
    ArkdownConfig, ArkdownPlugin, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
    OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_profile::ProfileProcessor;
use arkdown_plugin_chunk_wrapper::ChunkWrapperPlugin;
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::EtsTransformPlugin;
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
use arkdown_resolver::ResolverConfig;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // repo root
        .join("tests/fixtures/minimal-hap")
}

/// Construct a synthetic `ArkdownConfig` for CI environments that don't have an OHOS SDK.
///
/// This mirrors what `load_config` returns for this fixture, minus real SDK paths.
fn make_test_config(fixture: &PathBuf) -> ArkdownConfig {
    ArkdownConfig {
        project_root: fixture.clone(),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.example.arkdown.test".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: false,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
        },
        modules: vec![ModuleConfig {
            name: "entry".to_string(),
            src_path: fixture.join("entry"),
            module_type: OhosModuleType::Entry,
        }],
        sdk_config: SdkConfig {
            // Bundler does not invoke SDK binaries — fake paths are fine.
            ets_loader_path: PathBuf::from("/nonexistent/ets-loader"),
            es2abc_path: PathBuf::from("/nonexistent/es2abc"),
            restool_path: PathBuf::from("/nonexistent/restool"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }
}

#[tokio::test]
async fn test_build_minimal_hap_fixture() {
    let fixture = fixture_root();
    assert!(fixture.exists(), "fixture not found: {:?}", fixture);

    // ── 1. Config ────────────────────────────────────────────────────────────
    // Try the real config parser first.  On machines without an OHOS SDK the
    // discover_sdk step fails; fall back to a synthetic config so the bundler
    // pipeline can still be exercised.
    let config = match load_config(
        &fixture,
        BuildMode::Debug,
        OutputTarget::Hap,
        None,
        None,
    ) {
        Ok(cfg) => {
            assert_eq!(
                cfg.product.bundle_name, "com.example.arkdown.test",
                "bundleName mismatch — check AppScope/app.json5"
            );
            println!("[OK] load_config succeeded (SDK present)");
            cfg
        }
        Err(ConfigError::SdkNotFound) => {
            eprintln!("[INFO] OHOS SDK not found — falling back to synthetic ArkdownConfig");
            make_test_config(&fixture)
        }
        Err(e) => panic!("load_config failed unexpectedly: {:?}", e),
    };

    let config_arc = Arc::new(config);

    // ── 2. Parse module.json5 ────────────────────────────────────────────────
    let module = &config_arc.modules[0];
    let module_json5_path = module.src_path.join("src/main/module.json5");
    let content = fs_err::read_to_string(&module_json5_path)
        .expect("failed to read entry/src/main/module.json5");
    let module_json5: ModuleJson5 =
        json5::from_str(&content).expect("failed to parse module.json5");

    // ── 3. Profile → LoaderJson ──────────────────────────────────────────────
    let processor = ProfileProcessor::new(config_arc.clone());
    let (_manifest, loader_json) = processor
        .process_module(module, &module_json5)
        .expect("process_module failed");

    println!("compile_entry:    {:?}", loader_json.compile_entry);
    println!("module_root_path: {}", loader_json.module_root_path);
    println!("ace_module_build: {}", loader_json.ace_module_build);

    assert!(
        !loader_json.compile_entry.is_empty(),
        "compile_entry must not be empty — check abilities[].srcEntry in module.json5"
    );

    // Verify every compile_entry path resolves to a real file.
    let module_root = PathBuf::from(&loader_json.module_root_path);
    for entry in &loader_json.compile_entry {
        let entry_path = module_root.join(entry.trim_start_matches("./"));
        assert!(
            entry_path.exists(),
            "compile_entry '{}' → file not found: {:?}",
            entry,
            entry_path
        );
    }

    // ── 4. Plugins (same order as CLI build.rs) ──────────────────────────────
    let project_root = fixture.clone();
    let plugins: Vec<Arc<dyn ArkdownPlugin>> = vec![
        // Must be first — desugars .ets decorators before any other plugin sees source.
        Arc::new(EtsTransformPlugin::new(config_arc.clone())),
        // Maps specifiers to OHM URLs / local paths.
        Arc::new(OhosResolvePlugin::new(
            ResolverConfig {
                project_root: project_root.clone(),
                sdk_api_dirs: vec![],
                sdk_kits_dirs: vec![],
                pkg_name_to_pkg_info: HashMap::new(),
                use_normalized_ohm_url: false,
                case_sensitive_check: false,
                oh_modules_roots: vec![
                    module.src_path.join("oh_modules"),
                    project_root.join("oh_modules"),
                ],
                module_src_roots: vec![module.src_path.join("src/main/ets")],
            },
            project_root.clone(),
            false,
        )),
        // Annotates each module with OHOS package metadata.
        Arc::new(ModuleMetaPlugin::new(
            project_root.clone(),
            module.name.clone(),
        )),
        // JSON5 import support.
        Arc::new(Json5Plugin),
        // Duplicate filename check.
        Arc::new(DuplicateCheckPlugin),
        // IIFE chunk wrapper — must be last so it wraps final CJS code.
        Arc::new(ChunkWrapperPlugin::new(loader_json.support_chunks)),
    ];

    // ── 5. Bundle ────────────────────────────────────────────────────────────
    let bundler = Bundler::new((*config_arc).clone(), plugins);
    let result = bundler.bundle(&loader_json).await;

    match &result {
        Err(e) => eprintln!("Bundle error: {:?}", e),
        Ok(out) => {
            println!("chunks produced: {}", out.chunk_files.len());
            for (name, path) in &out.chunk_files {
                println!("  '{}' → {}", name, path.display());
            }
        }
    }

    let bundle_out = result.expect("bundler.bundle() failed");

    // ── 6. Assertions ────────────────────────────────────────────────────────

    // At least one chunk must have been produced.
    assert!(
        !bundle_out.chunk_files.is_empty(),
        "expected at least one chunk file"
    );

    // Every reported chunk file must exist on disk and be non-empty.
    for (name, path) in &bundle_out.chunk_files {
        assert!(
            path.exists(),
            "chunk '{}' file not found on disk: {:?}",
            name,
            path
        );
        let size = std::fs::metadata(path)
            .unwrap_or_else(|_| panic!("stat failed for chunk '{}'", name))
            .len();
        assert!(size > 0, "chunk '{}' output file is empty", name);
    }

    // Every non-empty chunk must be IIFE-wrapped.
    for chunk in bundle_out.chunk_graph.chunks.iter() {
        if chunk.code.is_empty() {
            continue;
        }
        let preview_len = chunk.code.len().min(300);
        println!(
            "chunk '{}' code (first {} chars):\n{}",
            chunk.name,
            preview_len,
            &chunk.code[..preview_len]
        );
        assert!(
            chunk.code.contains("(function ()") || chunk.code.contains("(function()"),
            "chunk '{}' is missing IIFE wrapper — check ChunkWrapperPlugin is last in plugin list.\nCode:\n{}",
            chunk.name,
            &chunk.code[..chunk.code.len().min(500)]
        );
    }

    println!(
        "Integration test PASSED: {} chunk(s) built with IIFE wrapping",
        bundle_out.chunk_files.len()
    );
}
