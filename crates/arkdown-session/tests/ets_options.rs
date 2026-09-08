use std::{fs, path::PathBuf};

use arkdown_core::SdkConfig;
use arkdown_session::SdkSnapshot;

#[test]
fn sdk_ets_tables_keep_source_merge_order_and_snapshot_identity() {
    // ets_checker.ts reads tsconfig once; concatenateEtsOptions changes only
    // components and extend.components, with JS object-identity Set behavior.
    let temp = tempfile::tempdir().unwrap();
    let primary = temp.path().join("openharmony/ets");
    let external = temp.path().join("vendor/ets");
    let loader = primary.join("build-tools/ets-loader");
    let external_components = external.join("build-tools/ets-loader/components");
    for path in [&loader, &external_components] {
        fs::create_dir_all(path).unwrap();
    }
    fs::write(
        external.join("sdkConfig.json"),
        r#"{"apiPath":["./api"],"prefix":"@vendor"}"#,
    )
    .unwrap();
    fs::write(loader.join("tsconfig.json"), r#"{
        // Loader configurations are JSONC, including trailing commas.
        "compilerOptions":{"ets":{
            "render":{"decorator":[]},
            "syntaxComponents":{"paramsUICallback":["ForEach","LazyForEach"],"attrUICallback":[{"name":"Repeat","attributes":["each"]}]},
            "components":["Text","Text"],
            "extend":{"components":[{"name":"Text","type":"TextAttribute","instance":"TextInstance"}]},
        }}
    }"#).unwrap();
    fs::write(
        external_components.join("externalconfig.json"),
        r#"{"compilerOptions":{"ets":{
        "components":["Text","Widget"],"render":{"decorator":["NotMerged"]},
        "extend":{"components":[{"name":"Text","type":"TextAttribute","instance":"TextInstance"}]}
    }}}"#,
    )
    .unwrap();
    let config = SdkConfig {
        ets_loader_path: loader.clone(),
        es2abc_path: PathBuf::new(),
        restool_path: PathBuf::new(),
        compile_api_version: 20,
        sdk_component_api_version: 20,
        has_rollup_plugin: true,
        compile_sdk_version: "6.0.0.1".into(),
    };
    let snapshot = SdkSnapshot::from_config_for_runtime(&config, "HarmonyOS").unwrap();
    let options = snapshot.ets_options.as_ref().unwrap();
    assert_eq!(
        options.components.as_deref().unwrap(),
        [Box::<str>::from("Text"), Box::<str>::from("Widget")]
    );
    assert_eq!(options.extend.components.as_ref().unwrap().len(), 2);
    assert_eq!(options.render.decorator.as_deref(), Some([].as_slice()));
    // EtsOptions spells UI as an acronym. Generic camelCase would silently
    // drop both tables before the native parser ever sees the configuration.
    let wire = serde_json::to_value(options.as_ref()).unwrap();
    assert_eq!(
        wire["syntaxComponents"]["paramsUICallback"],
        serde_json::json!(["ForEach", "LazyForEach"])
    );
    assert_eq!(
        wire["syntaxComponents"]["attrUICallback"],
        serde_json::json!([{"name":"Repeat", "attributes":["each"]}])
    );
    fs::write(
        loader.join("tsconfig.json"),
        r#"{"compilerOptions":{"ets":{"components":["Changed"]}}}"#,
    )
    .unwrap();
    assert_eq!(options.components.as_ref().unwrap()[0].as_ref(), "Text");

    let error = SdkSnapshot::from_config_for_runtime(&config, "HarmonyOS").unwrap_err();
    assert!(error
        .to_string()
        .contains("extend.components must be an array"));
}
