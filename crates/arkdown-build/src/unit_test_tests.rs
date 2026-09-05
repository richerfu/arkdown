use tempfile::TempDir;

use crate::layout::ModuleBuildLayout;
use crate::unit_test::{
    inject_unit_test_ability, UnitTestArtifactWriter, UnitTestProfileWriter,
    UnitTestTemplateParams, UnitTestTemplateWriter, UNIT_TEST_REPLACE_PAGE,
};

const GOLDEN_UNIT_TEST_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/golden/unit-test"
);

#[test]
fn ohos_test_mock_config_uses_normal_build_target_resources() {
    let temp = TempDir::new().expect("create ohosTest mock fixture root");
    let layout = ModuleBuildLayout::for_target_product(temp.path(), "demo", "ohosTest");
    let input = layout.intermediates_mock_config_json();
    std::fs::create_dir_all(input.parent().expect("mock input parent"))
        .expect("create mock input directory");
    std::fs::write(&input, b"{\"mock\":true}").expect("write transformed mock config");

    UnitTestArtifactWriter::for_ohos_test(temp.path(), "demo", "ohosTest")
        .copy_mock_config_json()
        .expect("copy ohosTest mock config");

    let output = layout.test_mock_config_output_json();
    assert_eq!(
        output,
        temp.path()
            .join("build")
            .join("demo")
            .join("intermediates")
            .join("res")
            .join("ohosTest")
            .join("resources")
            .join("rawfile")
            .join("mock")
            .join("mock-config.json")
    );
    assert_eq!(
        std::fs::read(output).expect("read copied mock config"),
        b"{\"mock\":true}"
    );
}

#[test]
fn crossplatform_ohos_test_uses_sdk_nomock_runner_template() {
    let temp = TempDir::new().expect("create crossplatform ohosTest root");
    UnitTestTemplateWriter::new(temp.path(), "default")
        .generate_ohos_test(17, true)
        .expect("generate ArkUI-X ohosTest template");

    let layout = ModuleBuildLayout::for_ohos_test_template(temp.path(), "default");
    let runner = std::fs::read_to_string(
        layout
            .ohos_test_runner_dir()
            .join("OpenHarmonyTestRunner.ets"),
    )
    .expect("read generated no-mock runner");

    assert!(runner.contains("import { Want } from '@kit.AbilityKit';"));
    assert!(!runner.contains("import { UIAbility, common, application }"));
    assert!(!runner.contains("mockMockKit"));
    assert!(runner.contains("abilityDelegator.startAbility(want"));
}

#[test]
fn unit_test_template_matches_hvigor_generated_files_byte_for_byte() {
    let module_path = std::fs::canonicalize(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("minimal-hap")
            .join("entry"),
    )
    .expect("minimal-hap entry fixture must exist");
    let serialized_module_path =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module_path)
            .expect("fixture path must serialize");
    let index_golden_template =
        std::fs::read_to_string(format!("{GOLDEN_UNIT_TEST_DIR}/Index.ets.golden"))
            .expect("hvigor-generated Index.ets golden must exist");
    let index_golden = index_golden_template
        .strip_suffix('\n')
        .unwrap_or(&index_golden_template)
        .replace("__MODULE_PATH__", &serialized_module_path);
    let ability_golden = std::fs::read(format!("{GOLDEN_UNIT_TEST_DIR}/TestAbility.ets.golden"))
        .expect("hvigor-generated TestAbility.ets golden must exist");
    let phone_golden = std::fs::read(format!(
        "{GOLDEN_UNIT_TEST_DIR}/phoneSettingConfig_Phone.json.golden"
    ))
    .expect("hvigor-generated phoneSettingConfig golden must exist");

    let params = UnitTestTemplateParams {
        bundle_name: "com.example.arkdown.test",
        module_name: "entry",
        scope: "",
        coverage: false,
        module_path: &module_path,
    };
    let rendered = params.render_page_index().expect("render template");
    assert_eq!(
        rendered.as_bytes(),
        index_golden.as_bytes(),
        "rendered Index.ets must equal the hvigor-generated file"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    UnitTestTemplateWriter::new(dir.path(), "default")
        .generate_local(&params)
        .expect("template generation");
    assert_eq!(
        std::fs::read(dir.path().join(".test/testability/TestAbility.ets")).unwrap(),
        ability_golden,
        "TestAbility.ets must equal the hvigor-generated file"
    );
    assert_eq!(
        std::fs::read(
            dir.path()
                .join(".test/testability/phoneSettingConfig_Phone.json")
        )
        .unwrap(),
        phone_golden,
        "phoneSettingConfig must equal the hvigor-generated file"
    );
}

#[test]
fn unit_test_template_coverage_false_and_scope_are_substituted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let module_path = dir.path().join("example").join("entry");
    let params = UnitTestTemplateParams {
        bundle_name: "com.example.arkdown.test",
        module_name: "entry",
        scope: "ActsAbilityTest",
        coverage: false,
        module_path: &module_path,
    };
    let rendered = params.render_page_index().expect("render template");
    assert!(rendered.contains("\"-s coverage\": \"false\""));
    assert!(rendered.contains("\"-s class\": \"ActsAbilityTest\""));
    assert!(rendered.contains("abilityDelegatorArguments.testCaseNames = \"ActsAbilityTest\""));
    let serialized_module_path =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module_path)
            .expect("test module path must serialize");
    assert!(rendered.contains(&format!(
    "global[savePath] = \"{serialized_module_path}/.test/default/intermediates/test/coverage_data/js_coverage.json\""
  )));
}

#[test]
fn unit_test_process_profile_injects_test_ability_like_hvigor() {
    let process_profile: serde_json::Value = serde_json::from_str(
        r#"{
      "app": {"bundleName": "com.example.arkdown.test", "icon": "$media:layered_image"},
      "module": {
        "name": "entry",
        "type": "entry",
        "mainElement": "EntryAbility",
        "abilities": [{
          "name": "EntryAbility",
          "srcEntry": "./ets/entryability/EntryAbility.ets",
          "startWindowIcon": "$media:startIcon",
          "startWindowBackground": "$color:start_window_background"
        }]
      }
    }"#,
    )
    .unwrap();
    let mut document = process_profile.clone();
    inject_unit_test_ability(&mut document, "entry").expect("injection");
    let abilities = document["module"]["abilities"].as_array().unwrap();
    assert_eq!(abilities.len(), 2);
    assert_eq!(abilities[1]["name"], "TestAbility");
    assert_eq!(
        abilities[1]["srcEntrance"],
        "../../.test/testability/TestAbility.ets"
    );
    assert_eq!(abilities[1]["startWindowIcon"], "$media:startIcon");
    assert_eq!(
        abilities[1]["startWindowBackground"],
        "$color:start_window_background"
    );
    assert_eq!(
        document["module"]["deliveryWithInstall"],
        serde_json::Value::Null
    );
}

#[test]
fn unit_test_process_profile_har_rewrite_matches_hvigor() {
    let mut document = serde_json::json!({
      "app": {"icon": "$media:layered_image", "label": "$string:app_name"},
      "module": {"name": "library", "type": "har", "abilities": []}
    });
    inject_unit_test_ability(&mut document, "har").expect("injection");
    assert_eq!(document["module"]["type"], "entry");
    assert_eq!(document["module"]["deliveryWithInstall"], true);
    assert_eq!(document["app"]["icon"], "icon");
    assert_eq!(document["app"]["label"], "label");
    let ability = &document["module"]["abilities"][0];
    assert_eq!(ability["startWindowIcon"], "startIcon");
    assert_eq!(ability["startWindowBackground"], "#FFFFFF");
}

#[test]
fn replace_unit_test_index_file_writes_pages_and_main_pages() {
    let dir = tempfile::tempdir().unwrap();
    let res = dir.path().join("res");
    std::fs::create_dir_all(&res).unwrap();
    let res_module_json = res.join("module.json");
    let profile_dir = res.join("resources/base/profile");
    let process_profile = serde_json::json!({
      "module": {"pages": "$profile:main_pages"}
    });
    UnitTestProfileWriter::new(
        &res_module_json,
        &process_profile,
        &profile_dir,
        "entry",
        UNIT_TEST_REPLACE_PAGE,
    )
    .replace_index()
    .expect("replace");
    let module_doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&res_module_json).unwrap()).unwrap();
    assert_eq!(module_doc["module"]["pages"], "$profile:main_pages");
    let main_pages: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(profile_dir.join("main_pages.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(main_pages["src"][0], UNIT_TEST_REPLACE_PAGE);

    let res_module_json2 = res.join("module2.json");
    std::fs::write(
        &res_module_json2,
        r#"{"module": {"pages": "$profile:existing"}}"#,
    )
    .unwrap();
    UnitTestProfileWriter::new(
        &res_module_json2,
        &process_profile,
        &profile_dir,
        "entry",
        UNIT_TEST_REPLACE_PAGE,
    )
    .replace_index()
    .expect("replace keeps pages");
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&res_module_json2).unwrap()).unwrap();
    assert_eq!(kept["module"]["pages"], "$profile:existing");
    assert!(profile_dir.join("main_pages.json").is_file());
}

#[test]
fn unit_test_artifact_handoffs_use_the_typed_module_layout() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let compile_output = temp.path().join("compile-output");
    std::fs::create_dir_all(&compile_output).expect("compile output");
    let modules_abc = compile_output.join("modules.abc");
    let source_maps = compile_output.join("sourceMaps.map");
    std::fs::write(&modules_abc, b"abc").expect("ABC fixture");
    std::fs::write(
        &source_maps,
        br#"{"entry":{"sources":["src/main/ets/Index.ets"],"mappings":"old"}}"#,
    )
    .expect("source map fixture");

    let layout = ModuleBuildLayout::for_test_product(&module_root, "default");
    let writer = UnitTestArtifactWriter::new(&module_root, "default");
    writer
        .stage_compiled_assets(&modules_abc, Some(&source_maps))
        .expect("stage UnitTestArkTS assets");
    assert_eq!(
        std::fs::read(layout.test_assets_modules_abc()).expect("staged ABC"),
        b"abc"
    );
    assert!(layout.test_assets_source_maps().is_file());

    let mock_input = layout.intermediates_mock_config_json();
    std::fs::create_dir_all(mock_input.parent().expect("mock input parent"))
        .expect("mock input directory");
    std::fs::write(&mock_input, b"{}").expect("mock config fixture");
    writer.copy_mock_config_json().expect("copy mock config");
    assert_eq!(
        std::fs::read(layout.test_mock_config_output_json()).expect("copied mock config"),
        b"{}"
    );

    let replacement = layout.test_coverage_source_map();
    std::fs::create_dir_all(replacement.parent().expect("coverage map parent"))
        .expect("coverage map directory");
    std::fs::write(
        &replacement,
        br#""src/main/ets/Index.ets":{"mappings":"new"},"#,
    )
    .expect("coverage source map fixture");
    writer
        .replace_source_map_in_coverage_mode(true)
        .expect("replace mappings");
    let merged: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.test_assets_source_maps()).expect("merged source maps"),
    )
    .expect("parse merged source maps");
    assert_eq!(merged["entry"]["mappings"], "new");

    writer.write_hsp_build_config().expect("HSP buildConfig");
    let build_config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.test_build_config_json()).expect("HSP buildConfig output"),
    )
    .expect("parse HSP buildConfig");
    assert_eq!(
        build_config["aceModuleBuild"],
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&layout.test_assets_ets_dir())
            .expect("portable artifact path")
    );
}
