use super::transform_collector::HvigorModuleTransformRecord;
use super::*;
use arkdown_cache::CacheKeyMaterial;
use arkdown_ets_transform::{CrossplatformSourceCollection, InsightIntentSourceMetadata};

#[test]
fn transform_collector_excludes_rollup_virtual_runtime_source_maps() {
    assert!(
        HvigorModuleTransformCollector::is_runtime_javascript_source(
            "/project/oh_modules/runtime/index.js"
        )
    );
    assert!(!HvigorModuleTransformCollector::is_runtime_javascript_source("\0rolldown/runtime.js"));
}

#[test]
fn transform_collector_restores_sdk_javascript_mapping_line_boundaries() {
    let restored = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
        r#"{"version":3,"sources":["ResourceTable.ts"],"names":[],"mappings":""}"#.to_string(),
        true,
    );
    let value: serde_json::Value =
        serde_json::from_str(&restored).expect("restored source map JSON");
    assert_eq!(value["mappings"], ";");

    let populated = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
        r#"{"version":3,"sources":["Index.ts"],"names":[],"mappings":"AAAA"}"#.to_string(),
        true,
    );
    let value: serde_json::Value =
        serde_json::from_str(&populated).expect("populated source map JSON");
    assert_eq!(value["mappings"], "AAAA");

    let emitted = HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
        r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AAAA"}"#.to_string(),
        false,
    );
    let value: serde_json::Value = serde_json::from_str(&emitted).expect("emitted source map JSON");
    assert_eq!(value["mappings"], "AAAA;");

    let emitted_with_boundary =
        HvigorModuleTransformCollector::restore_hvigor_javascript_mapping_lines(
            r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AAAA;"}"#
                .to_string(),
            false,
        );
    let value: serde_json::Value =
        serde_json::from_str(&emitted_with_boundary).expect("source map JSON with boundary");
    assert_eq!(value["mappings"], "AAAA;");
}

#[test]
fn runtime_javascript_map_keeps_rollup_trailing_line_without_source_root() {
    let normalized = HvigorModuleTransformCollector::normalize_runtime_javascript_source_map(
        r#"{"version":3,"sources":["index.js"],"names":[],"mappings":"AAAA","sourceRoot":""}"#
            .to_string(),
        true,
    );
    let value: serde_json::Value =
        serde_json::from_str(&normalized).expect("runtime JavaScript source map JSON");

    assert_eq!(value["mappings"], "AAAA;");
    assert!(value.get("sourceRoot").is_none());
}

#[test]
fn collector_restores_transform_map_only_for_an_unchanged_projection() {
    let code = "const value = 1;\n";
    let code_hash = CacheKeyMaterial::compute_content_hash(code);
    let transform = r#"{"version":3,"sources":["index.ts"],"names":[],"mappings":"AAAA,CAAC"}"#;
    let projection =
        r#"{"version":3,"sources":["index.ts"],"names":[],"mappings":"AAAA"}"#.to_string();

    assert_eq!(
        HvigorModuleTransformCollector::select_transform_source_map(
            code,
            &code_hash,
            Some(transform),
            projection,
        ),
        transform,
    );

    let remapped =
        r#"{"version":3,"sources":["index.ts"],"names":[],"mappings":"AACA"}"#.to_string();
    assert_eq!(
        HvigorModuleTransformCollector::select_transform_source_map(
            code,
            &code_hash,
            Some(transform),
            remapped.clone(),
        ),
        remapped,
    );

    let changed =
        r#"{"version":3,"sources":["index.ts"],"names":[],"mappings":"AAAA"}"#.to_string();
    assert_eq!(
        HvigorModuleTransformCollector::select_transform_source_map(
            "const value = 2;\n",
            &code_hash,
            Some(transform),
            changed.clone(),
        ),
        changed,
    );
}

#[test]
fn transform_store_hands_crossplatform_facts_to_bundle_state() {
    let store = HvigorModuleTransformStore::default();
    store.record(HvigorModuleTransformRecord {
        module_id: "/project/entry/src/main/ets/model/Router.js".to_string(),
        module_source_code: None,
        source_map: None,
        emits_javascript: false,
        is_shared_module: false,
        insight_intent: InsightIntentSourceMetadata::default(),
        crossplatform_collection: CrossplatformSourceCollection {
            modules: vec!["ohos.router".to_string()],
            components: Vec::new(),
        },
    });

    let snapshot = store.finish();
    assert_eq!(
        snapshot
            .crossplatform_collections
            .get("/project/entry/src/main/ets/model/Router.js")
            .expect("crossplatform facts")
            .modules,
        ["ohos.router".to_string()]
    );
}
