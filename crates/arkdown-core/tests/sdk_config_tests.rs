use std::fs;
use std::path::PathBuf;

use arkdown_core::SdkConfig;

fn sdk_config(ets_loader_path: PathBuf) -> SdkConfig {
    SdkConfig {
        ets_loader_path,
        es2abc_path: PathBuf::new(),
        restool_path: PathBuf::new(),
        compile_api_version: 17,
        sdk_component_api_version: 24,
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

#[test]
fn local_test_components_are_derived_from_the_selected_sdk_layout() {
    let temp = tempfile::tempdir().expect("temporary IDE layout");
    let contents = temp.path().join("Contents");
    let sdk_default = contents.join("sdk").join("default");
    let component = sdk_default.join("openharmony");
    let loader = component
        .join("ets")
        .join("build-tools")
        .join("ets-loader");
    let previewer = component.join("previewer");
    let hms_previewer = sdk_default.join("hms").join("previewer");
    let coverage = contents
        .join("tools")
        .join("hvigor")
        .join("hvigor-ohos-plugin")
        .join("node_modules")
        .join("@ohos")
        .join("coverage");
    fs::create_dir_all(previewer.join("common").join("bin")).expect("OHOS Previewer");
    fs::create_dir_all(&hms_previewer).expect("HMS Previewer");
    fs::write(hms_previewer.join("uni-package.json"), b"{}").expect("HMS metadata");
    let coverage_command = coverage
        .join("lib")
        .join("src")
        .join("commandLine")
        .join("index.js");
    fs::create_dir_all(coverage_command.parent().expect("coverage parent"))
        .expect("coverage package");
    fs::write(&coverage_command, b"module.exports = {};").expect("coverage entry");

    let config = sdk_config(loader);
    assert_eq!(config.previewer_component_path(), Some(previewer));
    assert_eq!(config.hms_previewer_component_path(), Some(hms_previewer));
    assert_eq!(config.hvigor_coverage_package_path(), Some(coverage));
}

#[test]
fn local_test_coverage_supports_the_standalone_command_line_layout() {
    let temp = tempfile::tempdir().expect("temporary command-line tools layout");
    let distribution = temp.path().join("command-line-tools");
    let component = distribution
        .join("sdk")
        .join("default")
        .join("openharmony");
    let loader = component
        .join("ets")
        .join("build-tools")
        .join("ets-loader");
    let coverage = distribution
        .join("hvigor")
        .join("hvigor-ohos-plugin")
        .join("node_modules")
        .join("@ohos")
        .join("coverage");
    let coverage_command = coverage
        .join("lib")
        .join("src")
        .join("commandLine")
        .join("index.js");
    fs::create_dir_all(coverage_command.parent().expect("coverage parent"))
        .expect("coverage package");
    fs::write(&coverage_command, b"module.exports = {};").expect("coverage entry");

    assert_eq!(
        sdk_config(loader).hvigor_coverage_package_path(),
        Some(coverage)
    );
}
