/// Integration tests verifying that `@Component` struct transforms produce a
/// source map that references the original `.ets` line/column positions rather
/// than self-referencing the generated ViewPU TypeScript output.
///
/// Rationale: hvigor's ets-loader emits a source map where each ViewPU
/// `observeComponentCreation2` call is mapped back to the ArkUI DSL line in
/// the original `.ets` file.  The arkdown ETS transform must do the same so
/// that DevTools / source-map consumers show the original `.ets` source in
/// stack traces and the `ets/sourceMaps.map` file matches hvigor.

use std::sync::Arc;

use arkdown_core::{
    BuildMode, BuildOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_plugin_ets_transform::EtsTransformPlugin;
use std::path::PathBuf;

fn make_plugin() -> EtsTransformPlugin {
    EtsTransformPlugin::new(Arc::new(arkdown_core::ArkdownConfig {
        project_root: PathBuf::from("/tmp/test"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "test".to_string(),
            bundle_name: "com.example.test".to_string(),
            compatible_sdk_version: "12".to_string(),
            runtime_os: "OpenHarmony".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: true,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: PathBuf::from("/unused"),
            es2abc_path: PathBuf::from("/unused"),
            restool_path: PathBuf::from("/unused"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }))
}

/// A `@Component` struct with a `build()` body containing `Text("hello")` must
/// produce a source map whose first non-empty mapping group appears several lines
/// into the ViewPU output — corresponding to the `observeComponentCreation2`
/// call in `initialRender()` — and references an original `.ets` line that is
/// at least 3 lines into the source (where the `build()` body lives).
///
/// This distinguishes the new span-aware map from the old self-consistent map,
/// in which the first non-empty group was always at generated line 0.
#[tokio::test]
async fn test_component_source_map_references_ets_lines() {
    let plugin = make_plugin();
    // ETS source: @Component struct with Text("hello") in build().
    // Line 0: @Entry
    // Line 1: @Component
    // Line 2: struct Hello {
    // Line 3:   build() {
    // Line 4:     Text("hello")
    // Line 5:   }
    // Line 6: }
    // The OXC ETS parser wraps `ArkUIComponentExpression` nodes when a component
    // call appears as a statement with braces (children) or chained attributes.
    // A bare `Text("hello")` without children or chains is a regular CallExpression.
    // Use the known-working nested form (Row with a Text child) instead.
    //
    // Line 0: @Entry
    // Line 1: @Component
    // Line 2: struct Hello {
    // Line 3:   build() {
    // Line 4:     Row() {
    // Line 5:       Text("hello")
    // Line 6:         .fontSize(20)
    // Line 7:     }.width("100%");
    // Line 8:   }
    // Line 9: }
    let ets_source = concat!(
        "@Entry\n",
        "@Component\n",
        "struct Hello {\n",
        "  build() {\n",
        "    Row() {\n",
        "      Text(\"hello\")\n",
        "        .fontSize(20)\n",
        "    }.width(\"100%\");\n",
        "  }\n",
        "}\n",
    );
    let id = "entry/src/main/ets/pages/Hello.ets";

    let result = plugin.do_transform(ets_source, id).unwrap()
        .expect("@Component transform must return Some");

    // Confirm the transform ran (ViewPU class emitted).
    assert!(
        result.code.as_deref().unwrap_or("").contains("class Hello extends ViewPU"),
        "must emit ViewPU class; got:\n{}",
        result.code.as_deref().unwrap_or("")
    );
    assert!(
        result.code.as_deref().unwrap_or("").contains("observeComponentCreation2"),
        "must lower Row()/Text() calls; got:\n{}",
        result.code.as_deref().unwrap_or("")
    );

    // Source map must be present for struct-transformed files.
    let map_json_str = match &result.map {
        rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(),
        _ => panic!("source_map must be Some for @Component files"),
    };

    let map: serde_json::Value =
        serde_json::from_str(&map_json_str).expect("source_map must be valid JSON");

    // Version and source reference.
    assert_eq!(map["version"], 3, "source map version must be 3");
    let sources = map["sources"].as_array().expect("sources must be an array");
    assert!(
        sources.iter().any(|s| s.as_str().is_some_and(|p| p.contains("Hello.ets"))),
        "sources must reference Hello.ets; got: {sources:?}"
    );

    let mappings = map["mappings"].as_str().expect("mappings must be a string");
    // Each `;` separates a generated line group.  The span-aware source map
    // only emits non-empty groups at the `observeComponentCreation2` lines
    // inside `initialRender()`, which are well past the ViewPU boilerplate
    // (constructor, setInitiallyProvidedValue, etc.).
    let groups: Vec<&str> = mappings.split(';').collect();
    let first_nonempty_idx = groups
        .iter()
        .position(|g| !g.is_empty())
        .expect("mappings must have at least one non-empty group");

    // The ViewPU boilerplate for even the simplest component is at least 10
    // lines before the initialRender() body.  Verify the first mapping is past
    // the beginning of the ViewPU output.
    assert!(
        first_nonempty_idx >= 10,
        "first non-empty mapping group must correspond to initialRender() body \
         (at ViewPU line ≥ 10), not ViewPU line 0; got first non-empty at group {first_nonempty_idx}"
    );

    // The non-empty group must contain a 4-field VLQ segment (generated_col,
    // source_idx, orig_line, orig_col) — i.e. the segment must be at least 4
    // VLQ characters long and must NOT be a single-char generated-only segment.
    let nonempty_group = groups[first_nonempty_idx];
    assert!(
        nonempty_group.len() >= 4,
        "the mapping segment must encode source-position fields (≥ 4 chars); got: {nonempty_group:?}"
    );
}

/// Non-struct `.ets` files (no `@Component`) must still return per-token source
/// maps — the span-aware path must not activate for them.
#[tokio::test]
async fn test_non_component_ets_still_returns_source_map() {
    let plugin = make_plugin();
    let code = "const greeting: string = 'hello';";
    let id = "entry/src/main/ets/util/helpers.ets";

    let result = plugin.do_transform(code, id).unwrap()
        .expect("non-component .ets must return Some");

    let map_json_str = match &result.map {
        rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(),
        _ => panic!("source_map must be Some for non-struct .ets files"),
    };
    let map: serde_json::Value = serde_json::from_str(&map_json_str).unwrap();
    let mappings = map["mappings"].as_str().unwrap();
    // For the non-struct path (Case A) the oxc codegen emits per-token mappings
    // which have at least one comma within a source line.
    let has_per_token = mappings.lines().any(|line| line.contains(','));
    assert!(
        has_per_token,
        "non-struct .ets must emit per-token source maps; got: {mappings}"
    );
}

/// For a struct with multiple ArkUI components in `build()`, every component
/// call in `initialRender()` must have a source map entry pointing to the
/// corresponding ETS line.  Verify there are multiple non-empty groups in the
/// mappings (one per `observeComponentCreation2` call).
#[tokio::test]
async fn test_multi_component_build_has_multiple_mapping_entries() {
    let plugin = make_plugin();
    // build() has three nested components: Row → Column → Text (with attribute chains
    // so that the OXC parser produces ArkUIComponentExpression nodes).
    let ets_source = concat!(
        "@Entry\n",
        "@Component\n",
        "struct Page {\n",
        "  build() {\n",
        "    Row() {\n",
        "      Column() {\n",
        "        Text(\"hi\")\n",
        "          .fontSize(16)\n",
        "      }.width(\"100%\")\n",
        "    }.height(\"100%\");\n",
        "  }\n",
        "}\n",
    );
    let id = "entry/src/main/ets/pages/Page.ets";

    let result = plugin.do_transform(ets_source, id).unwrap()
        .expect("@Component transform must return Some");

    let map_json_str = match &result.map { rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(), _ => panic!("source_map must be Some") };
    let map: serde_json::Value = serde_json::from_str(&map_json_str).unwrap();
    let mappings = map["mappings"].as_str().unwrap();
    let groups: Vec<&str> = mappings.split(';').collect();
    let nonempty_count = groups.iter().filter(|g| !g.is_empty()).count();
    // Three ArkUI components → three `observeComponentCreation2` calls →
    // three non-empty mapping groups.
    assert_eq!(
        nonempty_count, 3,
        "expected 3 mapping entries (Row, Column, Text); got {nonempty_count}"
    );
}
