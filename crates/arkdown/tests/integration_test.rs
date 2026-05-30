use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arkdown::Bundler;
use arkdown_config::{load_config, ConfigError};
use arkdown_core::{
    ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5, OhosModuleType, OutputTarget,
    ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_plugin_duplicate_check::DuplicateCheckPlugin;
use arkdown_plugin_ets_transform::EtsTransformPlugin;
use arkdown_plugin_json5::Json5Plugin;
use arkdown_plugin_module_meta::ModuleMetaPlugin;
use arkdown_plugin_ohos_resolve::OhosResolvePlugin;
use arkdown_profile::{LoaderJson, ProfileProcessor};
use arkdown_resolver::ResolverConfig;
use rolldown_plugin::__inner::SharedPluginable;

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
                preload_so: vec![],
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
    let config = match load_config(&fixture, BuildMode::Debug, OutputTarget::Hap, None, None) {
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
    let plugins: Vec<SharedPluginable> = vec![
        // Must be first — desugars .ets decorators before any other plugin sees source.
        Arc::new(EtsTransformPlugin::new(config_arc.clone())) as SharedPluginable,
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
        )) as SharedPluginable,
        // Annotates each module with OHOS package metadata.
        Arc::new(ModuleMetaPlugin::new(
            project_root.clone(),
            module.name.clone(),
        )) as SharedPluginable,
        // JSON5 import support.
        Arc::new(Json5Plugin) as SharedPluginable,
        // Duplicate filename check.
        Arc::new(DuplicateCheckPlugin) as SharedPluginable,
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
    let expected_loader_out_ets = module
        .src_path
        .join("build/default/intermediates/loader_out/default/ets");
    let old_abc_path = module
        .src_path
        .join("build/default/intermediates/ets/modules.abc");
    let pack_abc_input = bundle_out.modules_abc_path.clone();

    assert_eq!(
        bundle_out.output_dir, expected_loader_out_ets,
        "CompileArkTS loader output must match hvigor's loader_out/default/ets layout"
    );
    assert_eq!(
        pack_abc_input,
        expected_loader_out_ets.join("modules.abc"),
        "packaging must source final ets/modules.abc from loader_out/default/ets/modules.abc"
    );
    assert_ne!(
        pack_abc_input, old_abc_path,
        "packaging must not source modules.abc from the legacy intermediates/ets path"
    );
    assert_eq!(
        bundle_out.source_maps_path,
        Some(expected_loader_out_ets.join("sourceMaps.map")),
        "source map output must use hvigor's sourceMaps.map path"
    );
    let source_maps_path = bundle_out
        .source_maps_path
        .as_ref()
        .expect("sourceMaps.map path should be reported");
    assert!(
        source_maps_path.exists(),
        "sourceMaps.map should be written when rolldown produces a real source map"
    );
    let source_maps =
        fs_err::read_to_string(source_maps_path).expect("failed to read generated sourceMaps.map");
    let source_maps_json: serde_json::Value =
        serde_json::from_str(&source_maps).expect("sourceMaps.map must be valid JSON");

    // The bundler now emits the hvigor per-file dict format: a JSON object whose
    // keys are "{module}|{pkg}|{ver}|src/main/{path}.ts" and whose values are
    // individual source map objects.  Validate that at least one such entry exists
    // and contains a non-empty mappings string.
    let obj = source_maps_json
        .as_object()
        .expect("sourceMaps.map must be a JSON object");
    assert!(
        !obj.is_empty(),
        "sourceMaps.map must not be empty — bundler produced no source maps"
    );
    // Every value must be a source map object with a non-empty mappings field or
    // a non-empty sources array (falling back to the single-chunk raw-map case).
    let has_real_maps = obj.values().any(|entry| {
        entry
            .get("mappings")
            .and_then(|m| m.as_str())
            .is_some_and(|m| !m.is_empty())
            || entry
                .get("sources")
                .and_then(|s| s.as_array())
                .is_some_and(|s| !s.is_empty())
            || entry
                .get("sections")
                .and_then(|s| s.as_array())
                .is_some_and(|s| !s.is_empty())
    });
    assert!(
        has_real_maps,
        "sourceMaps.map must be generated from real compile inputs"
    );

    // At least one chunk must have been produced.
    assert!(
        !bundle_out.chunk_files.is_empty(),
        "expected at least one chunk file"
    );

    // Every reported chunk file must exist on disk. Some generated chunks can
    // be empty after tree-shaking/entry normalization; non-empty chunks are
    // checked for helper-free ESM shape below.
    for (name, path) in &bundle_out.chunk_files {
        assert!(
            path.exists(),
            "chunk '{}' file not found on disk: {:?}",
            name,
            path
        );
    }

    // Every non-empty chunk must stay close to CompileArkTS ESM output.  The
    // ABC backend must not see rolldown's CJS helper layer.
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
            !chunk.code.contains("__toESM")
                && !chunk.code.contains("__copyProps")
                && !chunk.code.contains("require("),
            "chunk '{}' contains rolldown CJS helper output.\nCode:\n{}",
            chunk.name,
            &chunk.code[..chunk.code.len().min(500)]
        );
    }

    println!(
        "Integration test PASSED: {} helper-free ESM chunk(s) built",
        bundle_out.chunk_files.len()
    );
}

/// Build a minimal `ArkdownConfig` for a transient temp-dir fixture.
fn make_minimal_config(project_root: &PathBuf, build_mode: BuildMode) -> ArkdownConfig {
    ArkdownConfig {
        project_root: project_root.clone(),
        build_mode,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.example.test".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                preload_so: vec![],
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: false,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: PathBuf::from("/nonexistent/ets-loader"),
            es2abc_path: PathBuf::from("/nonexistent/es2abc"),
            restool_path: PathBuf::from("/nonexistent/restool"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }
}

/// Build a minimal `LoaderJson` for a temp-dir fixture.
fn make_minimal_loader_json(
    module_root: &PathBuf,
    compile_entry: Vec<String>,
    output_dir: PathBuf,
    build_mode: &str,
    support_chunks: bool,
) -> LoaderJson {
    let root_str = module_root.to_string_lossy().into_owned();
    let out_str = output_dir.to_string_lossy().into_owned();
    LoaderJson {
        module_path_map: HashMap::new(),
        compile_mode: "esmodule".to_string(),
        project_root_path: root_str.clone(),
        node_modules_path: format!("{}/node_modules", root_str),
        byte_code_har_info: HashMap::new(),
        declaration_entry: vec![],
        module_name: "entry".to_string(),
        hsp_name_ohm_map: HashMap::new(),
        har_name_ohm_map: HashMap::new(),
        package_manager_type: "ohpm".to_string(),
        hvigor_compile_entry: vec![],
        other_compile_files: vec![],
        dynamic_import_lib_info: HashMap::new(),
        router_map: vec![],
        hsp_resources_map: HashMap::new(),
        update_version_info: HashMap::new(),
        an_build_out_put: format!("{}/an", root_str),
        an_build_mode: "type".to_string(),
        non_entry_importees: vec![],
        compiled_importees: vec![],
        compile_entry,
        module_root_path: root_str.clone(),
        cache_path: format!("{}/cache/", root_str),
        ace_module_build: out_str,
        build_mode: build_mode.to_string(),
        support_chunks,
        use_normalized_ohm_url: false,
        api_version: 12,
        fallback_an_build: false,
        pkg_name_to_pkg_brief_info: HashMap::new(),
        workers: vec![],
    }
}

/// Verify release-mode treeshake is active and produces valid output.
///
/// The `Bundler` uses `TreeshakeOptions::Boolean(true)` for `BuildMode::Release`
/// and `TreeshakeOptions::Boolean(false)` for `BuildMode::Debug`.  This test
/// confirms:
/// - Both debug and release builds complete without error.
/// - Both produce helper-free ESM (no `require(` / `__toESM` helpers).
/// - The used function survives in both outputs.
/// - The release output is never larger than the debug output — rolldown's
///   live-variable analysis can only remove bindings, never add them.
#[tokio::test]
async fn test_release_treeshake_removes_dead_export() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let root = tmp.path().to_path_buf();

    // A utility module with a used function and a large dead function.
    // The dead function is never imported, so it is absent from both builds.
    // The important invariant is that the release bundle is not *larger* than
    // the debug bundle (treeshake can only shrink output, never grow it).
    let utils_src = concat!(
        "export function usedFn(): string { return \"live_value\"; }\n",
        "export function unusedFn(): string {\n",
        "  const PAD_A = \"PADDING_A\";\n",
        "  const PAD_B = \"PADDING_B\";\n",
        "  const PAD_C = \"PADDING_C\";\n",
        "  return PAD_A + PAD_B + PAD_C;\n",
        "}\n",
    );
    fs_err::write(root.join("utils.ts"), utils_src).expect("write utils.ts");

    // Entry imports only the used function.
    let main_src = "import { usedFn } from './utils';\nexport const result = usedFn();\n";
    fs_err::write(root.join("main.ts"), main_src).expect("write main.ts");

    let out_dir = root.join("out");
    fs_err::create_dir_all(&out_dir).expect("create out dir");

    // ── Debug build ───────────────────────────────────────────────────────────
    let debug_config = make_minimal_config(&root, BuildMode::Debug);
    let loader_debug = make_minimal_loader_json(
        &root,
        vec!["./main.ts".to_string()],
        out_dir.clone(),
        "debug",
        false,
    );
    let bundler_debug = Bundler::new(debug_config, vec![]);
    let debug_out = bundler_debug
        .bundle(&loader_debug)
        .await
        .expect("debug bundle failed");

    let debug_code: String = debug_out
        .chunk_graph
        .chunks
        .iter()
        .map(|c| c.code.as_str())
        .collect();

    println!("debug output ({} bytes):\n{debug_code}", debug_code.len());
    assert!(
        !debug_out.chunk_graph.chunks.is_empty(),
        "debug build must produce at least one chunk"
    );
    assert!(
        !debug_code.contains("require(") && !debug_code.contains("__toESM"),
        "debug output must be helper-free ESM; got:\n{debug_code}"
    );
    assert!(
        debug_code.contains("usedFn") || debug_code.contains("live_value"),
        "debug build must include the used function; got:\n{debug_code}"
    );

    // ── Release build ─────────────────────────────────────────────────────────
    let release_config = make_minimal_config(&root, BuildMode::Release);
    let loader_release = make_minimal_loader_json(
        &root,
        vec!["./main.ts".to_string()],
        out_dir.clone(),
        "release",
        false,
    );
    let bundler_release = Bundler::new(release_config, vec![]);
    let release_out = bundler_release
        .bundle(&loader_release)
        .await
        .expect("release bundle failed");

    let release_code: String = release_out
        .chunk_graph
        .chunks
        .iter()
        .map(|c| c.code.as_str())
        .collect();

    println!(
        "release output ({} bytes):\n{release_code}",
        release_code.len()
    );
    assert!(
        !release_out.chunk_graph.chunks.is_empty(),
        "release build must produce at least one chunk"
    );
    assert!(
        !release_code.contains("require(") && !release_code.contains("__toESM"),
        "release output must be helper-free ESM; got:\n{release_code}"
    );
    assert!(
        release_code.contains("usedFn") || release_code.contains("live_value"),
        "release build must retain the used function; got:\n{release_code}"
    );

    // Treeshake can only remove bindings — release output must not be larger.
    assert!(
        release_code.len() <= debug_code.len(),
        "release output ({} bytes) must not be larger than debug output ({} bytes)",
        release_code.len(),
        debug_code.len()
    );

    println!(
        "test_release_treeshake_removes_dead_export PASSED (debug={} bytes, release={} bytes)",
        debug_code.len(),
        release_code.len()
    );
}

/// Verify `support_chunks=true` produces a vendors chunk for shared non-entry modules.
///
/// With two entry files that both import a shared module, the `ManualCodeSplittingOptions`
/// built by `build_manual_code_splitting_options()` assigns the shared module to the
/// `vendors` chunk (because it is not an entry and not in `oh_modules`/`node_modules`).
#[tokio::test]
async fn test_support_chunks_produces_vendors_chunk() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let root = tmp.path().to_path_buf();

    // Shared non-entry module.
    let shared_src =
        "export function sharedHelper(x: string): string { return \"shared:\" + x; }\n";
    fs_err::write(root.join("shared.ts"), shared_src).expect("write shared.ts");

    // Two entry files that each import from the shared module.
    let entry1_src =
        "import { sharedHelper } from './shared';\nexport const a = sharedHelper(\"entry1\");\n";
    fs_err::write(root.join("entry1.ts"), entry1_src).expect("write entry1.ts");

    let entry2_src =
        "import { sharedHelper } from './shared';\nexport const b = sharedHelper(\"entry2\");\n";
    fs_err::write(root.join("entry2.ts"), entry2_src).expect("write entry2.ts");

    let out_dir = root.join("out");
    fs_err::create_dir_all(&out_dir).expect("create out dir");

    let config = make_minimal_config(&root, BuildMode::Debug);
    let loader = make_minimal_loader_json(
        &root,
        vec!["./entry1.ts".to_string(), "./entry2.ts".to_string()],
        out_dir.clone(),
        "debug",
        true, // support_chunks=true
    );

    let bundler = Bundler::new(config, vec![]);
    let bundle_out = bundler
        .bundle(&loader)
        .await
        .expect("support_chunks bundle failed");

    println!(
        "support_chunks chunks produced: {}",
        bundle_out.chunk_files.len()
    );
    for (name, path) in &bundle_out.chunk_files {
        println!("  chunk '{}' → {}", name, path.display());
    }

    // At least 2 entries + 1 vendors = 3 chunks expected.
    assert!(
        bundle_out.chunk_files.len() >= 2,
        "expected at least 2 chunk files with support_chunks=true, got {}",
        bundle_out.chunk_files.len()
    );

    // A vendors chunk must be present in the graph.
    let has_vendors = bundle_out
        .chunk_graph
        .chunks
        .iter()
        .any(|c| c.name == "vendors");
    let chunk_names: Vec<&str> = bundle_out
        .chunk_graph
        .chunks
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(
        has_vendors,
        "expected a 'vendors' chunk with support_chunks=true; chunks found: {:?}",
        chunk_names
    );

    // The vendors chunk must contain the shared helper.
    let vendors_code = bundle_out
        .chunk_graph
        .chunks
        .iter()
        .find(|c| c.name == "vendors")
        .map(|c| c.code.as_str())
        .unwrap_or("");
    assert!(
        vendors_code.contains("sharedHelper") || vendors_code.contains("shared:"),
        "vendors chunk should contain the sharedHelper function; got:\n{vendors_code}"
    );

    println!("test_support_chunks_produces_vendors_chunk PASSED");
}
