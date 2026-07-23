use tempfile::TempDir;

use super::intermediates::IntermediateArtifactWriter;
use super::layout::RestoolLayout;

#[test]
fn release_source_maps_move_out_of_loader_output_like_hvigor() {
  let temp = TempDir::new().expect("create source-map fixture root");
  let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
  let intermediate = temp.path().join("source_map/default/sourceMaps.map");
  std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
  std::fs::write(&origin, b"source-map").expect("write source map");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .stage_source_maps(&origin, &intermediate, &arkdown_core::BuildMode::Release)
    .expect("stage release source map");

  assert_eq!(
    std::fs::read(&intermediate).expect("read staged source map"),
    b"source-map"
  );
  assert!(!origin.exists());
}

#[test]
fn debug_source_maps_remain_in_loader_output_after_staging() {
  let temp = TempDir::new().expect("create source-map fixture root");
  let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
  let intermediate = temp.path().join("source_map/default/sourceMaps.map");
  std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
  std::fs::write(&origin, b"source-map").expect("write source map");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .stage_source_maps(&origin, &intermediate, &arkdown_core::BuildMode::Debug)
    .expect("stage debug source map");

  assert!(origin.is_file());
  assert!(intermediate.is_file());
}

#[test]
fn compile_resource_metadata_is_added_after_profile_intermediates() {
  let temp = TempDir::new().expect("create resource profile fixture root");
  let module_json = temp.path().join("res/default/module.json");
  let insight_intent = temp
    .path()
    .join("res/default/resources/base/profile/insight_intent.json");
  std::fs::create_dir_all(insight_intent.parent().unwrap()).expect("create profile directory");
  std::fs::write(&module_json, r#"{"app":{},"module":{"name":"entry"}}"#)
    .expect("write resource module profile");
  std::fs::write(
    &insight_intent,
    r#"{"insightIntents":[{"intentName":"Search"}]}"#,
  )
  .expect("write insight intent profile");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .apply_compile_resource_module_metadata(&module_json, true, &insight_intent)
    .expect("apply compile-resource metadata");

  let profile: serde_json::Value =
    serde_json::from_slice(&std::fs::read(module_json).expect("read resource module profile"))
      .expect("parse resource module profile");
  assert_eq!(
    profile["module"]["routerMap"],
    "$profile:default-router-map"
  );
  assert_eq!(profile["module"]["hasInsightIntent"], true);
}

#[test]
fn merge_profile_precedes_dependency_extension_ability_collection() {
  let temp = TempDir::new().expect("create profile stage fixture root");
  let module_root = temp.path().join("entry");
  let final_module_json = temp.path().join("final-module.json");
  std::fs::write(
    &final_module_json,
    r#"{
      "app": {},
      "module": {
        "name": "entry",
        "virtualMachine": "ark12.0.6.0",
        "compileMode": "esmodule",
        "extensionAbilities": [
          { "name": "CurrentOne" },
          { "name": "CurrentTwo" },
          { "name": "DependencyHarExtension" }
        ]
      }
    }"#,
  )
  .expect("write final module profile");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .write_profile_intermediates(&module_root, &final_module_json, 2)
    .expect("write staged profiles");

  let layout = super::layout::ModuleBuildLayout::new(&module_root);
  let merge: serde_json::Value = serde_json::from_slice(
    &std::fs::read(layout.merge_profile_module_json()).expect("read merge profile"),
  )
  .expect("parse merge profile");
  let process: serde_json::Value = serde_json::from_slice(
    &std::fs::read(layout.process_profile_module_json()).expect("read process profile"),
  )
  .expect("parse process profile");
  let resource_ark: serde_json::Value = serde_json::from_slice(
    &std::fs::read(layout.res_ark_module_json()).expect("read resource ark profile"),
  )
  .expect("parse resource ark profile");

  assert_eq!(
    merge["module"]["extensionAbilities"]
      .as_array()
      .unwrap()
      .len(),
    2
  );
  assert_eq!(
    process["module"]["extensionAbilities"]
      .as_array()
      .unwrap()
      .len(),
    3
  );
  assert!(merge["module"].get("virtualMachine").is_none());
  assert_eq!(process["module"]["virtualMachine"], "ark12.0.6.0");
  assert_eq!(
    resource_ark["module"]["extensionAbilities"]
      .as_array()
      .unwrap()
      .len(),
    2
  );
  assert_eq!(resource_ark["module"]["virtualMachine"], "ark12.0.6.0");
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
