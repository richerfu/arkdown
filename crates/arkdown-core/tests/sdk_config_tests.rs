use std::fs;
use std::path::PathBuf;

use arkdown_core::SdkConfig;

fn sdk_config(ets_loader_path: PathBuf) -> SdkConfig {
    SdkConfig {
        ets_loader_path,
        es2abc_path: PathBuf::new(),
        restool_path: PathBuf::new(),
        compile_api_version: 17,
        has_rollup_plugin: true,
        compile_sdk_version: "6.0.1.112".to_string(),
    }
}

#[test]
fn arkguard_api_dirs_follow_loader_global_module_paths_and_component_metadata() {
    let temp = tempfile::tempdir().expect("temporary SDK");
    let sdk_root = temp.path().join("default");
    let ets_root = sdk_root.join("openharmony/ets");
    let loader = ets_root.join("build-tools/ets-loader");
    for path in [
        ets_root.join("api"),
        ets_root.join("arkts"),
        ets_root.join("kits"),
        sdk_root.join("openharmony/js/api"),
        sdk_root.join("vendor/ets/api"),
        sdk_root.join("vendor/ets/kits"),
    ] {
        fs::create_dir_all(path).expect("SDK directory");
    }
    fs::create_dir_all(&loader).expect("ets-loader");
    fs::write(
        sdk_root.join("vendor/ets/sdkConfig.json"),
        r#"{"apiPath":["./api","./kits"],"prefix":"@vendor"}"#,
    )
    .expect("external SDK metadata");

    let config = sdk_config(loader);
    assert_eq!(
        config.arkguard_api_dirs("HarmonyOS"),
        vec![
            ets_root.join("api"),
            ets_root.join("arkts"),
            ets_root.join("kits"),
            sdk_root.join("vendor/ets/api"),
            sdk_root.join("vendor/ets/kits"),
        ]
    );
    assert_eq!(
        config.arkguard_api_dirs("OpenHarmony"),
        vec![
            ets_root.join("api"),
            ets_root.join("arkts"),
            ets_root.join("kits"),
        ],
        "OpenHarmony targets must not inherit sibling HarmonyOS API components"
    );
}
