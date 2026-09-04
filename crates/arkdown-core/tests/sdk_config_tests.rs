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
fn local_test_components_are_derived_from_the_selected_sdk_layout() {
    let temp = tempfile::tempdir().expect("temporary IDE layout");
    let contents = temp.path().join("Contents");
    let sdk_default = contents.join("sdk").join("default");
    let component = sdk_default.join("openharmony");
    let loader = component.join("ets").join("build-tools").join("ets-loader");
    let previewer = component.join("previewer");
    let hms_previewer = sdk_default.join("hms").join("previewer");
    let hms_ets = sdk_default.join("hms").join("ets");
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
    fs::create_dir_all(&hms_ets).expect("HMS ETS");
    fs::write(hms_ets.join("uni-package.json"), b"{}").expect("HMS ETS metadata");
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
    assert_eq!(config.hms_ets_component_path(), Some(hms_ets));
    assert_eq!(config.hvigor_coverage_package_path(), Some(coverage));
}

#[test]
fn harmonyos_modulecheck_uses_hms_schema_owners_like_hvigor() {
    let temp = tempfile::tempdir().expect("temporary SDK layout");
    let sdk_default = temp.path().join("sdk").join("default");
    let ohos_component = sdk_default.join("openharmony");
    let loader = ohos_component
        .join("ets")
        .join("build-tools")
        .join("ets-loader");
    let ohos_schemas = ohos_component.join("toolchains").join("modulecheck");
    let hms_schemas = sdk_default
        .join("hms")
        .join("toolchains")
        .join("modulecheck");
    for schema_root in [&ohos_schemas, &hms_schemas] {
        fs::create_dir_all(schema_root).expect("modulecheck schema directory");
        for file_name in ["app.json", "module.json", "systemTheme.json", "pages.json"] {
            fs::write(schema_root.join(file_name), b"{}").expect("modulecheck schema");
        }
    }

    let config = sdk_config(loader);
    for file_name in ["app.json", "module.json", "systemTheme.json"] {
        assert_eq!(
            config.modulecheck_schema_path_for_runtime("HarmonyOS", file_name),
            Some(hms_schemas.join(file_name)),
            "HosSdkInfo overrides {file_name} with the HMS toolchains component"
        );
        assert_eq!(
            config.modulecheck_schema_path_for_runtime("OpenHarmony", file_name),
            Some(ohos_schemas.join(file_name))
        );
    }
    assert_eq!(
        config.modulecheck_schema_path_for_runtime("HarmonyOS", "pages.json"),
        Some(ohos_schemas.join("pages.json")),
        "HosSdkInfo inherits page schema lookup from CommonSdkInfo"
    );
}

#[test]
fn local_test_coverage_supports_the_standalone_command_line_layout() {
    let temp = tempfile::tempdir().expect("temporary command-line tools layout");
    let distribution = temp.path().join("command-line-tools");
    let component = distribution.join("sdk").join("default").join("openharmony");
    let loader = component.join("ets").join("build-tools").join("ets-loader");
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
