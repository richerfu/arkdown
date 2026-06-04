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
            compatible_sdk_version_stage: None,
            runtime_os: "OpenHarmony".to_string(),
            signing_config: None,
            build_option: BuildOption {
                preload_so: vec![],
                remove_permissions: vec![],
                aot_compile_mode: None,
                ap_path: None,
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
            ets_loader_path: PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/ets-loader"
            )),
            es2abc_path: PathBuf::from("/unused"),
            restool_path: PathBuf::from("/unused"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }))
}

/// A `@Component` struct with a nested ArkUI `build()` body must produce a source
/// map that references original `.ets` line/column positions for both generated
/// ViewPU boilerplate and `initialRender()` ArkUI calls.
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

    let result = plugin
        .do_transform(ets_source, id)
        .unwrap()
        .expect("@Component transform must return Some");

    // Confirm the transform ran (ViewPU class emitted).
    assert!(
        result
            .code
            .as_deref()
            .unwrap_or("")
            .contains("class Hello extends ViewPU"),
        "must emit ViewPU class; got:\n{}",
        result.code.as_deref().unwrap_or("")
    );
    assert!(
        result
            .code
            .as_deref()
            .unwrap_or("")
            .contains("observeComponentCreation2"),
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
        sources
            .iter()
            .any(|s| s.as_str().is_some_and(|p| p.contains("Hello.ets"))),
        "sources must reference Hello.ets; got: {sources:?}"
    );

    let mappings = map["mappings"].as_str().expect("mappings must be a string");
    let decoded = decode_mappings(mappings);
    assert!(
        decoded
            .iter()
            .any(|entry| entry.generated_line < 10 && entry.source_line == 2),
        "ViewPU class boilerplate must map back to the original struct line; got: {decoded:?}"
    );

    let code = result.code.as_deref().unwrap_or("");
    let row_create_line = line_index(code, "Row.create();").expect("Row.create must be emitted");
    let text_create_line = line_index(code, "Text.create(\"hello\");")
        .expect("Text.create(\"hello\") must be emitted");
    assert!(
        decoded
            .iter()
            .any(|entry| entry.generated_line >= row_create_line && entry.source_line == 4),
        "Row.create() must map to the original Row() line; got: {decoded:?}"
    );
    assert!(
        decoded
            .iter()
            .any(|entry| entry.generated_line == text_create_line && entry.source_line == 5),
        "Text.create() must map to the original Text() line; got: {decoded:?}"
    );
}

/// Non-struct `.ets` files (no `@Component`) must still return per-token source
/// maps — the span-aware path must not activate for them.
#[tokio::test]
async fn test_non_component_ets_still_returns_source_map() {
    let plugin = make_plugin();
    let code = "const greeting: string = 'hello';";
    let id = "entry/src/main/ets/util/helpers.ets";

    let result = plugin
        .do_transform(code, id)
        .unwrap()
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

    let result = plugin
        .do_transform(ets_source, id)
        .unwrap()
        .expect("@Component transform must return Some");

    let map_json_str = match &result.map {
        rolldown_plugin::HookTransformOutputMap::Sourcemap(sm) => sm.to_json_string(),
        _ => panic!("source_map must be Some"),
    };
    let map: serde_json::Value = serde_json::from_str(&map_json_str).unwrap();
    let code = result.code.as_deref().unwrap_or("");
    let decoded = decode_mappings(map["mappings"].as_str().unwrap());

    for (needle, source_line) in [
        ("Row.create();", 4),
        ("Column.create();", 5),
        ("Text.create(\"hi\");", 6),
    ] {
        let generated_line = line_index(code, needle).expect("ArkUI create call must be emitted");
        assert!(
            decoded
                .iter()
                .any(|entry| entry.generated_line >= generated_line
                    && entry.source_line == source_line),
            "{needle} must map to original ETS line {source_line}; got: {decoded:?}"
        );
    }
}

#[derive(Debug)]
struct MappingEntry {
    generated_line: usize,
    source_line: i32,
}

fn line_index(code: &str, needle: &str) -> Option<usize> {
    code.lines().position(|line| line.contains(needle))
}

fn decode_mappings(mappings: &str) -> Vec<MappingEntry> {
    let mut entries = Vec::new();
    let mut previous_source = 0i32;
    let mut previous_source_line = 0i32;
    let mut previous_source_col = 0i32;

    for (generated_line, group) in mappings.split(';').enumerate() {
        for segment in group.split(',').filter(|segment| !segment.is_empty()) {
            let mut index = 0usize;
            let (_generated_col_delta, next) = decode_vlq(segment, index);
            index = next;

            if index >= segment.len() {
                continue;
            }

            let (source_delta, next) = decode_vlq(segment, index);
            index = next;
            previous_source += source_delta;

            let (source_line_delta, next) = decode_vlq(segment, index);
            index = next;
            previous_source_line += source_line_delta;

            let (source_col_delta, _) = decode_vlq(segment, index);
            previous_source_col += source_col_delta;

            let _ = previous_source;
            let _ = previous_source_col;
            entries.push(MappingEntry {
                generated_line,
                source_line: previous_source_line,
            });
        }
    }

    entries
}

fn decode_vlq(segment: &str, mut index: usize) -> (i32, usize) {
    let bytes = segment.as_bytes();
    let mut result = 0i32;
    let mut shift = 0u32;

    loop {
        let digit = base64_value(bytes[index]);
        index += 1;
        let continuation = (digit & 32) != 0;
        result += ((digit & 31) as i32) << shift;
        shift += 5;
        if !continuation {
            break;
        }
    }

    let negative = (result & 1) == 1;
    result >>= 1;
    if negative {
        (-result, index)
    } else {
        (result, index)
    }
}

fn base64_value(byte: u8) -> u8 {
    match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("invalid base64 VLQ byte: {byte}"),
    }
}
