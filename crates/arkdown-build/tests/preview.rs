use arkdown_build::preview::{PreviewAssetProcessor, PreviewAssetProcessorInput};
use arkdown_core::{CompileMode, OhosModuleType};

#[test]
fn preview_incremental_parameters_match_restool_command_maps() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    let module = project.join("entry");
    let app_resources = project.join("AppScope").join("resources");
    let module_main = module.join("src").join("main").join("resources");
    let module_target = module.join("src").join("phone").join("resources");
    let har_resources = project
        .join("oh_modules")
        .join("dep")
        .join("src")
        .join("main")
        .join("resources");
    for directory in [&app_resources, &module_main, &module_target, &har_resources] {
        std::fs::create_dir_all(directory).unwrap();
    }

    PreviewAssetProcessor::new(PreviewAssetProcessorInput {
        project_root: &project,
        module_root: &module,
        module_name: "entry",
        module_type: &OhosModuleType::Entry,
        product_name: "default",
        target_name: "phone",
        compile_mode: &CompileMode::EsModule,
        package_name: "com.example.entry",
    })
    .persist_incremental_resource_parameters(
        Some(&app_resources),
        &[module_main.clone(), module_target.clone()],
        std::slice::from_ref(&har_resources),
        &["entry".to_string(), "feature".to_string()],
    )
    .unwrap();

    let output = module.join(".preview").join("PreviewBuildParam.json");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    let module_map: serde_json::Value =
        serde_json::from_str(value["MODULE_RESOURCES"].as_str().unwrap()).unwrap();
    let har_map: serde_json::Value =
        serde_json::from_str(value["HAR_RESOURCES"].as_str().unwrap()).unwrap();

    assert_eq!(module_map[slash(&module_main)], "module");
    assert_eq!(module_map[slash(&module_target)], "module1");
    assert_eq!(har_map[slash(&har_resources)], "har");
    assert_eq!(value["APP_RESOURCES"], slash(&app_resources));
    assert_eq!(value["PREVIEW_COMPILE_MODULE_NAMES"], "entry,feature");
    assert_eq!(
        value["PREVIEW_GENERATE_SOURCE_DIR"],
        slash(
            &module
                .join(".preview")
                .join("default")
                .join("generated")
                .join("r")
                .join("phone")
        )
    );
}

fn slash(path: &std::path::Path) -> String {
    arkdown_path::ArtifactPathSerializer::path_to_slash_string(path).unwrap()
}
