use std::path::PathBuf;

use tempfile::TempDir;

use super::intermediates::IntermediateArtifactWriter;
use super::layout::RestoolLayout;

#[test]
fn release_source_maps_move_out_of_loader_output_like_hvigor() {
  let temp = TempDir::new().expect("create source-map fixture root");
  let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
  let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
  let intermediate = temp.path().join("source_map/default/sourceMaps.map");
  std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
  std::fs::write(&origin, b"source-map").expect("write source map");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .stage_source_maps(
      &origin,
      &compile_cache,
      &intermediate,
      &arkdown_core::BuildMode::Release,
    )
    .expect("stage release source map");

  assert_eq!(
    std::fs::read(&intermediate).expect("read staged source map"),
    b"source-map"
  );
  assert!(!origin.exists());
  assert_eq!(
    std::fs::read(&compile_cache).expect("read CompileArkTS source map"),
    b"source-map"
  );
}

#[test]
fn debug_source_maps_remain_in_loader_output_after_staging() {
  let temp = TempDir::new().expect("create source-map fixture root");
  let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
  let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
  let intermediate = temp.path().join("source_map/default/sourceMaps.map");
  std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
  std::fs::write(&origin, b"source-map").expect("write source map");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .stage_source_maps(
      &origin,
      &compile_cache,
      &intermediate,
      &arkdown_core::BuildMode::Debug,
    )
    .expect("stage debug source map");

  assert!(origin.is_file());
  assert!(intermediate.is_file());
  assert!(compile_cache.is_file());
}

#[test]
fn package_har_publishes_source_maps_to_outputs_mapping_like_latest_hvigor() {
  let temp = TempDir::new().expect("create PackageHar source-map fixture root");
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
  let intermediate = module_layout.source_maps_intermediate();
  std::fs::create_dir_all(intermediate.parent().unwrap()).expect("create source-map directory");
  std::fs::write(&intermediate, b"package-har-source-map").expect("write source map");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .publish_package_source_maps(
      &intermediate,
      None,
      &module_layout.output_mapping_source_maps(),
    )
    .expect("publish PackageHar source map");

  assert_eq!(
    std::fs::read(module_layout.output_mapping_source_maps())
      .expect("read published PackageHar source map"),
    b"package-har-source-map"
  );
}

#[test]
fn empty_router_map_writes_latest_process_router_map_task_outputs() {
  let temp = TempDir::new().expect("create empty router-map fixture root");
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .write_empty_router_map_intermediates(temp.path())
    .expect("write empty ProcessRouterMap outputs");

  for output in [
    module_layout.router_map_loader(),
    module_layout.router_map_temp(),
  ] {
    assert_eq!(
      std::fs::read_to_string(&output).expect("read empty router-map output"),
      r#"{"routerMap":[]}"#
    );
  }
}

#[test]
fn module_info_writer_matches_latest_conditional_output_and_stale_cleanup() {
  let temp = TempDir::new().expect("create module-info fixture root");
  let module_root = temp.path().join("entry");
  let output = module_root.join("build/default/generated/profile/default/ModuleInfo.ts");
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .write_module_info(&module_root, "default", true)
    .expect("write enabled ModuleInfo.ts");
  assert_eq!(
    std::fs::read_to_string(&output).expect("read ModuleInfo.ts"),
    "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n"
  );

  writer
    .write_module_info(&module_root, "default", false)
    .expect("remove stale disabled ModuleInfo.ts");
  assert!(!output.exists());
}

#[test]
fn package_har_cache_path_matches_latest_target_scoped_task_cache() {
  let module_root = PathBuf::from("project").join("entry");
  let layout = super::layout::ModuleBuildLayout::new(&module_root);

  assert_eq!(
    layout.package_har_cache_dir("default"),
    module_root
      .join("build/default/cache/default")
      .join("default@PackageHar")
  );
}

#[test]
fn resource_table_outputs_match_latest_process_resource_contract() {
  let temp = TempDir::new().expect("create resource table fixture root");
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  let hap = writer.resource_table_outputs(
    &module_layout,
    &arkdown_core::OhosModuleType::Entry,
    &arkdown_core::OutputTarget::Hap,
  );
  assert_eq!(
    hap
      .iter()
      .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
      .collect::<Vec<_>>(),
    ["ResourceTable.h", "ResourceTable.ts"]
  );

  let har = writer.resource_table_outputs(
    &module_layout,
    &arkdown_core::OhosModuleType::Har,
    &arkdown_core::OutputTarget::Har,
  );
  assert_eq!(har.len(), 1);
  let preview = writer.resource_table_outputs(
    &module_layout,
    &arkdown_core::OhosModuleType::Entry,
    &arkdown_core::OutputTarget::Preview,
  );
  assert_eq!(preview.len(), 1);
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

#[tokio::test]
async fn partial_hvigor_execution_stops_only_after_every_selected_root() {
  use super::args::NativeTaskRoot;
  use super::coordinator::{BuildTaskLifecycle, NativeTaskCutoffReached};

  let mut lifecycle = BuildTaskLifecycle::new(
    None,
    vec![
      NativeTaskRoot {
        module_name: "entry".to_string(),
        target_name: Some("default".to_string()),
        task_name: "CreateBuildProfile".to_string(),
      },
      NativeTaskRoot {
        module_name: "entry".to_string(),
        target_name: Some("default".to_string()),
        task_name: "CompileArkTS".to_string(),
      },
    ],
  );

  lifecycle
    .after("entry", Some("default"), "CreateBuildProfile")
    .await
    .expect("the first selected root must not stop a multi-root command");
  lifecycle
    .after("entry", Some("default"), "GenerateLoaderJson")
    .await
    .expect("an unrelated dependency boundary must not stop the command");
  let cutoff = lifecycle
    .after("entry", Some("default"), "CompileArkTS")
    .await
    .expect_err("the final selected root must stop the native pipeline");

  assert!(cutoff.is::<NativeTaskCutoffReached>());
}

#[tokio::test]
async fn unqualified_module_hook_root_matches_a_targetless_boundary() {
  use super::args::NativeTaskRoot;
  use super::coordinator::{BuildTaskLifecycle, NativeTaskCutoffReached};

  let mut lifecycle = BuildTaskLifecycle::new(
    None,
    vec![NativeTaskRoot {
      module_name: "entry".to_string(),
      target_name: None,
      task_name: "GenerateBuildProfile".to_string(),
    }],
  );

  lifecycle
    .after("entry", Some("default"), "CreateBuildProfile")
    .await
    .expect("the qualified dependency must not complete the unqualified hook root");
  let cutoff = lifecycle
    .after("entry", None, "GenerateBuildProfile")
    .await
    .expect_err("the unqualified module hook boundary must stop the partial command");

  assert!(cutoff.is::<NativeTaskCutoffReached>());
}
