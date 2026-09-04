use super::*;
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
fn transform_store_hands_crossplatform_facts_to_bundle_state() {
    let store = HvigorModuleTransformStore::default();
    store.record(
        "/project/entry/src/main/ets/model/Router.js".to_string(),
        None,
        None,
        false,
        false,
        InsightIntentSourceMetadata::default(),
        CrossplatformSourceCollection {
            modules: vec!["ohos.router".to_string()],
            components: Vec::new(),
        },
    );

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
