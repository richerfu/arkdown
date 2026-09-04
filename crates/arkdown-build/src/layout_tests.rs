use std::path::PathBuf;

use tempfile::TempDir;

use super::layout::{ModuleBuildLayout, RestoolLayout};

#[test]
fn ohos_test_template_layout_stays_under_test_root_for_named_target_builds() {
    let temp = TempDir::new().expect("create ohosTest layout root");
    let target = ModuleBuildLayout::for_target_product(temp.path(), "tablet", "ohosTest");
    let template = ModuleBuildLayout::for_ohos_test_template(temp.path(), "tablet");

    assert_eq!(
        target.res_module_json(),
        temp.path()
            .join("build/tablet/intermediates/res/ohosTest/module.json")
    );
    assert_eq!(
        template.ohos_test_resources_root(),
        temp.path()
            .join(".test/tablet/intermediates/src/ohosTest/resources")
    );
    assert_eq!(
        template.ohos_test_ets_root(),
        temp.path()
            .join(".test/tablet/intermediates/src/ohosTest/ets")
    );
}

#[test]
fn ohos_test_template_migration_detection_matches_hvigor_first_ability_rule() {
    let temp = TempDir::new().expect("create ohosTest source root");
    let source = ModuleBuildLayout::new(temp.path()).source();

    assert!(
        source.ohos_test_templates_after_migration(&serde_json::json!({
          "module": { "abilities": [{ "name": "TestAbility" }] }
        }))
    );
    assert!(
        !source.ohos_test_templates_after_migration(&serde_json::json!({
          "module": { "abilities": [
            { "name": "TestAbility", "srcEntry": "./ets/TestAbility.ets" },
            { "name": "Ignored", "srcEntry": "" }
          ] }
        }))
    );

    std::fs::create_dir_all(temp.path().join("src/ohosTest/ets/testrunner"))
        .expect("create legacy runner directory");
    std::fs::write(
        temp.path()
            .join("src/ohosTest/ets/testrunner/OpenHarmonyTestRunner.ts"),
        "export default class OpenHarmonyTestRunner {}",
    )
    .expect("write legacy runner");
    assert!(
        !source.ohos_test_templates_after_migration(&serde_json::json!({
          "module": {}
        }))
    );
}

#[test]
fn package_har_cache_path_matches_latest_target_scoped_task_cache() {
    let module_root = PathBuf::from("project").join("entry");
    let layout = ModuleBuildLayout::new(&module_root);

    assert_eq!(
        layout.package_har_cache_dir("default"),
        module_root
            .join("build/default/cache/default")
            .join("default@PackageHar")
    );
}

#[test]
fn module_layout_scopes_build_test_and_preview_outputs_by_selected_product() {
    let module_root = PathBuf::from("project").join("entry");
    let product_name = "paid";

    let build = ModuleBuildLayout::for_output_target_product(
        &arkdown_core::OutputTarget::Hap,
        &module_root,
        product_name,
        "phone",
    );
    assert_eq!(
        build.loader_json(),
        module_root
            .join("build")
            .join(product_name)
            .join("intermediates")
            .join("loader")
            .join("phone")
            .join("loader.json")
    );
    assert_eq!(
        build.obfuscation_export_rule_path(),
        module_root
            .join("build")
            .join(product_name)
            .join("intermediates")
            .join("obfuscation")
            .join("phone")
            .join("obfuscation.txt")
    );
    assert_eq!(
        build.output_pack_info(),
        module_root
            .join("build")
            .join(product_name)
            .join("outputs")
            .join("phone")
            .join("pack.info")
    );

    let test = ModuleBuildLayout::for_test_product(&module_root, product_name);
    assert_eq!(
        test.test_init_coverage_json(),
        module_root
            .join(".test")
            .join(product_name)
            .join("intermediates")
            .join("test")
            .join("init_coverage.json")
    );

    let preview = ModuleBuildLayout::for_preview_product(&module_root, product_name, "phone");
    assert_eq!(
        preview.generated_resource_table_typescript(),
        module_root
            .join(".preview")
            .join(product_name)
            .join("generated")
            .join("r")
            .join("phone")
            .join("ResourceTable.ts")
    );
}

#[test]
fn restool_layout_matches_hvigor_sdk_specific_image_transcoder_contract() {
    let temp = TempDir::new().expect("create SDK fixture root");

    let openharmony_toolchains = temp.path().join("openharmony-sdk").join("toolchains");
    std::fs::create_dir_all(&openharmony_toolchains).expect("create OpenHarmony toolchains");
    std::fs::write(openharmony_toolchains.join("oh-uni-package.json"), b"{}")
        .expect("write OpenHarmony component marker");
    let openharmony_layout =
        RestoolLayout::from_restool_path(&openharmony_toolchains.join("restool"));
    assert_eq!(
        openharmony_layout.image_transcoder_library(),
        None,
        "hvigor OhosSdkInfo.getLibimageTranscoderShared() returns an empty path"
    );
    assert_eq!(
        openharmony_layout.defined_sys_ids(),
        None,
        "hvigor ProcessResource omits definedSysIds for OpenHarmony SDKs"
    );

    let default_sdk = temp.path().join("deveco-sdk").join("default");
    let openharmony_restool = default_sdk
        .join("openharmony")
        .join("toolchains")
        .join("restool");
    let hms_toolchains = default_sdk.join("hms").join("toolchains");
    std::fs::create_dir_all(&hms_toolchains).expect("create HarmonyOS toolchains");
    std::fs::write(hms_toolchains.join("id_defined.json"), b"[]")
        .expect("write HarmonyOS system ID definitions");
    let harmony_layout = RestoolLayout::from_restool_path(&openharmony_restool);
    let library = harmony_layout
        .image_transcoder_library()
        .expect("HarmonyOS SDK must expose the image transcoder path");
    assert_eq!(library.parent(), Some(hms_toolchains.join("lib").as_path()));
    assert_eq!(
        library.file_name().and_then(|name| name.to_str()),
        Some(platform_image_transcoder_name())
    );
    assert_eq!(
        harmony_layout.defined_sys_ids(),
        Some(hms_toolchains.join("id_defined.json"))
    );
}

fn platform_image_transcoder_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "libimage_transcoder_shared.dll"
    } else if cfg!(target_os = "linux") {
        "libimage_transcoder_shared.so"
    } else {
        "libimage_transcoder_shared.dylib"
    }
}
